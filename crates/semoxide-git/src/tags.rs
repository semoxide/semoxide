//! Tags: the ones merged into a commit (G11) and one by name (G15), each with the commit it
//! points to.

use crate::{CommitId, GitError, ObjectId, Repo};

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
    pub fn merged_tags(&self, _commit: &CommitId) -> Result<Vec<Tag>, GitError> {
        Ok(Vec::new())
    }

    /// The tag `name`; `None` if there is none or it doesn't point at a commit.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if libgit2 fails to read it.
    pub fn tag_commit(&self, _name: &str) -> Result<Option<Tag>, GitError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use semoxide_error::ErrorInfo;
    use semoxide_test_support::git_fixture::{GitFixture, Merge};

    use super::*;
    use crate::codes::GIT_READ_FAILED;

    /// `main` with a lightweight and an annotated tag, a `feature` branch with its own tag
    /// merged into `main` (`main^1` is before the merge), an annotated tag on the merge commit
    /// and a tag that points at a tree.
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
        CommitId::from(git2::Oid::from_str(&fixture.rev_parse(rev).unwrap()).unwrap())
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

    #[test]
    fn an_unknown_commit_is_a_read_failure() {
        let fixture = fixture();
        let unknown = CommitId::from(
            git2::Oid::from_str("1111111111111111111111111111111111111111").unwrap(),
        );

        assert_eq!(
            repo(&fixture)
                .merged_tags(&unknown)
                .map_err(|error| error.code()),
            Err(GIT_READ_FAILED)
        );
    }
}
