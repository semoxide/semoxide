//! Tags: the ones merged into a commit (G11) and one by name (G15), each with the commit it
//! points to.

use std::collections::HashSet;

use tracing::{debug, warn};

use crate::id::{commit_id, to_oid};
use crate::{CommitId, GitError, ObjectId, Repo};

const PREFIX: &str = "refs/tags/";

/// A tag that points at a commit, directly or through an annotated tag.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Tag {
    name: String,
    commit: CommitId,
    annotation: Option<ObjectId>,
}

impl Tag {
    /// The name, without `refs/tags/`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The commit the tag points to, through an annotated tag if there is one.
    #[must_use]
    pub fn commit(&self) -> &CommitId {
        &self.commit
    }

    /// The tag object of an annotated tag; `None` for a lightweight tag. Upstream's channel
    /// notes can sit on it instead of the commit (UPSTREAM-CORE, G18).
    #[must_use]
    pub fn annotation(&self) -> Option<&ObjectId> {
        self.annotation.as_ref()
    }
}

impl Repo {
    /// The tags whose commit is reachable from `commit`, like `git tag --merged <commit>`,
    /// sorted by name. Tags that don't point at a commit, or whose name isn't UTF-8, are
    /// skipped.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if `commit` doesn't exist or libgit2 fails to read the history.
    pub fn merged_tags(&self, commit: &CommitId) -> Result<Vec<Tag>, GitError> {
        let read = |error: git2::Error| GitError::read_failed(&error);
        let repo = self.git();
        // One walk, then a set lookup per tag: checking each tag's ancestry separately is slow
        // with many tags.
        let mut walk = repo.revwalk().map_err(read)?;
        walk.push(to_oid(commit).map_err(read)?).map_err(read)?;
        let reachable: HashSet<git2::Oid> = walk.collect::<Result<_, _>>().map_err(read)?;
        let mut tags = Vec::new();
        for reference in repo.references_glob(&format!("{PREFIX}*")).map_err(read)? {
            // Like git, a broken ref is left out of the list.
            let reference = match reference {
                Ok(reference) => reference,
                Err(error) => {
                    warn!(target: "semoxide::git", op = "G11", error = error.message(), "skipping a broken tag");
                    continue;
                }
            };
            let tag = match to_tag(&reference) {
                Ok(tag) => tag,
                // A tag pointing at a missing object is broken too.
                Err(error) => {
                    warn!(target: "semoxide::git", op = "G11", %error, "skipping a broken tag");
                    continue;
                }
            };
            if let Some(tag) = tag
                && reachable.contains(&to_oid(&tag.commit).map_err(read)?)
            {
                tags.push(tag);
            }
        }
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        debug!(target: "semoxide::git", op = "G11", %commit, count = tags.len(), "merged tags read");
        Ok(tags)
    }

    /// The tag `name`; `None` if there is none or it doesn't point at a commit.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if libgit2 fails to read it.
    pub fn tag_commit(&self, name: &str) -> Result<Option<Tag>, GitError> {
        let reference = match self.git().find_reference(&format!("{PREFIX}{name}")) {
            Ok(reference) => reference,
            // An invalid name can't be a tag, so it is missing like any other.
            Err(error)
                if matches!(
                    error.code(),
                    git2::ErrorCode::NotFound | git2::ErrorCode::InvalidSpec
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(GitError::read_failed(&error)),
        };
        let tag = to_tag(&reference)?;
        debug!(target: "semoxide::git", op = "G15", tag = name, found = tag.is_some(), "tag read");
        Ok(tag)
    }
}

/// The tag `reference` names; `None`, logged, if its name isn't UTF-8 or it doesn't point at a
/// commit.
fn to_tag(reference: &git2::Reference<'_>) -> Result<Option<Tag>, GitError> {
    let Some(name) = reference
        .name()
        .ok()
        .and_then(|full| full.strip_prefix(PREFIX))
    else {
        debug!(target: "semoxide::git", "skipping a tag whose name isn't UTF-8");
        return Ok(None);
    };
    let commit = match reference.peel_to_commit() {
        Ok(commit) => commit.id(),
        Err(error)
            if matches!(
                error.code(),
                git2::ErrorCode::Peel | git2::ErrorCode::InvalidSpec
            ) =>
        {
            debug!(target: "semoxide::git", tag = name, "skipping a tag that doesn't point at a commit");
            return Ok(None);
        }
        Err(error) => return Err(GitError::read_failed(&error)),
    };
    // An annotated tag's ref points at its tag object; a lightweight one at the commit.
    let annotation = reference
        .target()
        .filter(|&target| target != commit)
        .map(ObjectId::from);
    Ok(Some(Tag {
        name: name.to_owned(),
        commit: commit_id(commit).map_err(|error| GitError::read_failed(&error))?,
        annotation,
    }))
}

#[cfg(test)]
mod tests {
    use semoxide_error::ErrorInfo;
    use semoxide_test_support::git_fixture::{GitFixture, Merge};

    use super::*;
    use crate::codes::GIT_READ_FAILED;

    /// `main` with a lightweight and an annotated tag, a `feature` branch with its own tag
    /// merged into `main` with a merge commit (so `main^1`, its first parent, is before the
    /// merge), an annotated tag on the merge commit and a tag that points at a tree.
    fn fixture() -> GitFixture {
        let fixture = GitFixture::builder()
            .commit("feat: a")
            .tag("v1.0.0")
            .commit("fix: b")
            .annotated_tag("v1.0.1", "release 1.0.1")
            .branch("feature")
            .commit("feat: c")
            .tag("v1.1.0-feature.1")
            .checkout("main")
            .commit("fix: d")
            .tag("v1.0.2")
            .merge("feature", Merge::NoFastForward)
            .annotated_tag("v1.1.0", "release 1.1.0")
            .build()
            .unwrap();
        fixture.git(&["tag", "tree-tag", "HEAD^{tree}"]).unwrap();
        fixture
    }

    fn commit(fixture: &GitFixture, rev: &str) -> CommitId {
        CommitId::try_from(fixture.rev_parse(rev).unwrap().as_str()).unwrap()
    }

    fn repo(fixture: &GitFixture) -> Repo {
        Repo::discover(&fixture.path()).unwrap()
    }

    /// A tag as `(name, commit, annotation)`.
    type Described = (String, String, Option<String>);

    fn describe(tag: &Tag) -> Described {
        (
            tag.name().to_owned(),
            tag.commit().to_string(),
            tag.annotation().map(ToString::to_string),
        )
    }

    /// What git says about `name`: its commit (`git rev-list -1`) and, for an annotated tag,
    /// its own object.
    fn described_by_git(fixture: &GitFixture, name: &str) -> Described {
        let commit = fixture.git(&["rev-list", "-1", name]).unwrap();
        let kind = fixture.git(&["cat-file", "-t", name]).unwrap();
        let annotation = (kind == "tag").then(|| fixture.rev_parse(name).unwrap());
        (name.to_owned(), commit, annotation)
    }

    /// `git tag --merged <rev>`, each tag described by git.
    fn merged_by_git(fixture: &GitFixture, rev: &str) -> Vec<Described> {
        let sha = fixture.rev_parse(rev).unwrap();
        fixture
            .git(&["tag", "--merged", &sha])
            .unwrap()
            .lines()
            .map(|name| described_by_git(fixture, name))
            .collect()
    }

    #[test]
    fn merged_tags_match_git_before_and_after_a_merge() {
        let fixture = fixture();
        let names = |rev: &str| -> Vec<String> {
            merged_by_git(&fixture, rev)
                .into_iter()
                .map(|(name, ..)| name)
                .collect()
        };

        // The fixture has what the comparison needs: tags before and after the merge, and
        // a tree tag git leaves out.
        assert_eq!(
            (
                names("main^1"),
                names("main"),
                fixture.git(&["tag", "--list", "tree-tag"]).unwrap()
            ),
            (
                vec![
                    String::from("v1.0.0"),
                    String::from("v1.0.1"),
                    String::from("v1.0.2")
                ],
                vec![
                    String::from("v1.0.0"),
                    String::from("v1.0.1"),
                    String::from("v1.0.2"),
                    String::from("v1.1.0"),
                    String::from("v1.1.0-feature.1")
                ],
                String::from("tree-tag")
            )
        );

        let repo = repo(&fixture);
        let merged = |rev: &str| {
            repo.merged_tags(&commit(&fixture, rev))
                .map(|tags| tags.iter().map(describe).collect::<Vec<_>>())
        };

        assert_eq!(
            (merged("main^1"), merged("main")),
            (
                Ok(merged_by_git(&fixture, "main^1")),
                Ok(merged_by_git(&fixture, "main"))
            )
        );
    }

    #[test]
    fn a_tag_by_name_matches_git_and_is_none_if_missing_or_not_a_commit() {
        let fixture = fixture();
        let repo = repo(&fixture);
        let tag = |name: &str| repo.tag_commit(name).map(|tag| tag.as_ref().map(describe));

        assert_eq!(
            (tag("v1.0.0"), tag("v1.1.0"), tag("v9.9.9"), tag("tree-tag")),
            (
                Ok(Some(described_by_git(&fixture, "v1.0.0"))),
                Ok(Some(described_by_git(&fixture, "v1.1.0"))),
                Ok(None),
                Ok(None)
            )
        );
    }

    /// Adds what the main fixture lacks: packed tags listed apart from loose ones, a tag
    /// sharing a commit with another, an uppercase name (byte order puts it first) and a tag
    /// of an annotated tag.
    fn awkward_fixture() -> GitFixture {
        let fixture = fixture();
        fixture.git(&["pack-refs", "--all"]).unwrap();
        fixture.git(&["tag", "v1.0.0-again", "v1.0.0"]).unwrap();
        fixture.git(&["tag", "V2.0.0", "main"]).unwrap();
        fixture
            .git(&["tag", "-a", "v1.0.1-nested", "-m", "nested", "v1.0.1"])
            .unwrap();
        fixture
    }

    #[test]
    fn merged_tags_match_git_for_packed_shared_uppercase_and_nested_tags() {
        let fixture = awkward_fixture();

        assert_eq!(
            repo(&fixture)
                .merged_tags(&commit(&fixture, "main"))
                .map(|tags| tags.iter().map(describe).collect::<Vec<_>>()),
            Ok(merged_by_git(&fixture, "main"))
        );
    }

    #[test]
    fn a_nested_tag_matches_git_and_a_branch_or_invalid_name_is_no_tag() {
        let fixture = awkward_fixture();
        let repo = repo(&fixture);
        let tag = |name: &str| repo.tag_commit(name).map(|tag| tag.as_ref().map(describe));

        assert_eq!(
            (tag("v1.0.1-nested"), tag("feature"), tag("bad name")),
            (
                Ok(Some(described_by_git(&fixture, "v1.0.1-nested"))),
                Ok(None),
                Ok(None)
            )
        );
    }

    /// The names of the tags merged into `main`, as the main fixture has them.
    fn merged_names() -> Vec<String> {
        ["v1.0.0", "v1.0.1", "v1.0.2", "v1.1.0", "v1.1.0-feature.1"]
            .map(String::from)
            .to_vec()
    }

    /// Like git, a broken tag is left out of the list; asked for by name, it is an error.
    #[test]
    fn a_broken_tag_is_skipped_in_the_list_and_an_error_by_name() {
        let fixture = fixture();
        std::fs::write(
            fixture
                .path()
                .join(".git")
                .join("refs")
                .join("tags")
                .join("broken"),
            "not a sha\n",
        )
        .unwrap();
        let repo = repo(&fixture);

        assert_eq!(
            (
                repo.merged_tags(&commit(&fixture, "main"))
                    .map(|tags| tags.iter().map(|tag| tag.name().to_owned()).collect()),
                repo.tag_commit("broken").map_err(|error| error.code())
            ),
            (Ok(merged_names()), Err(GIT_READ_FAILED))
        );
    }

    /// Like git, a tag pointing at a missing object is left out of the list; asked for by name,
    /// it is an error.
    #[test]
    fn a_dangling_tag_is_skipped_in_the_list_and_an_error_by_name() {
        let fixture = fixture();
        std::fs::write(
            fixture
                .path()
                .join(".git")
                .join("refs")
                .join("tags")
                .join("dangling"),
            "1111111111111111111111111111111111111111
",
        )
        .unwrap();
        let repo = repo(&fixture);

        assert_eq!(
            (
                repo.merged_tags(&commit(&fixture, "main"))
                    .map(|tags| tags.iter().map(|tag| tag.name().to_owned()).collect()),
                repo.tag_commit("dangling").map_err(|error| error.code())
            ),
            (Ok(merged_names()), Err(GIT_READ_FAILED))
        );
    }

    /// A name that isn't UTF-8 can't be a loose ref on every platform, but `packed-refs`
    /// holds raw bytes everywhere.
    #[test]
    fn a_tag_whose_name_isnt_utf_8_is_skipped() {
        let fixture = fixture();
        fixture.git(&["pack-refs", "--all"]).unwrap();
        let packed = fixture.path().join(".git").join("packed-refs");
        let mut text = std::fs::read(&packed).unwrap();
        text.extend(fixture.rev_parse("main").unwrap().as_bytes());
        text.extend(b" refs/tags/v\xff\n");
        std::fs::write(&packed, text).unwrap();

        assert_eq!(
            repo(&fixture)
                .merged_tags(&commit(&fixture, "main"))
                .map(|tags| tags.iter().map(|tag| tag.name().to_owned()).collect()),
            Ok(merged_names())
        );
    }

    #[test]
    fn an_unknown_commit_is_a_read_failure() {
        let fixture = fixture();
        let unknown = CommitId::try_from("1111111111111111111111111111111111111111").unwrap();

        assert_eq!(
            repo(&fixture)
                .merged_tags(&unknown)
                .map_err(|error| error.code()),
            Err(GIT_READ_FAILED)
        );
    }
}
