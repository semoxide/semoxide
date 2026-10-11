//! `HEAD`: the commit it points to (G14) and the local branch, unless detached (G7).

use tracing::debug;

use crate::{CommitId, GitError, Repo};

/// Where `HEAD` is.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Head {
    commit: CommitId,
    branch: Option<String>,
}

impl Head {
    /// The commit `HEAD` points to.
    #[must_use]
    pub fn commit(&self) -> &CommitId {
        &self.commit
    }

    /// The local branch `HEAD` is on, without `refs/heads/`; `None` when detached.
    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }
}

impl Repo {
    /// Where `HEAD` is, like `git rev-parse HEAD` and `git rev-parse --abbrev-ref HEAD`. A
    /// branch name that isn't UTF-8 is decoded with replacement characters.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if `HEAD` points at a branch with no commits yet, or libgit2 fails
    /// to read it.
    pub fn head(&self) -> Result<Head, GitError> {
        let read = |error: git2::Error| GitError::read_failed(&error);
        let reference = self.git().head().map_err(|error| {
            if error.code() == git2::ErrorCode::UnbornBranch {
                GitError::no_commits(self.workdir())
            } else {
                read(error)
            }
        })?;
        // Through an annotated tag to its commit.
        let commit = reference.peel_to_commit().map_err(read)?;
        // Only a local branch counts; detached, the name is `HEAD`.
        let branch = reference
            .name_bytes()
            .strip_prefix(b"refs/heads/")
            .map(|name| String::from_utf8_lossy(name).into_owned());
        let head = Head {
            commit: CommitId::from(commit.id()),
            branch,
        };
        debug!(
            target: "semoxide::git",
            op = "G7/G14",
            commit = %head.commit,
            branch = head.branch.as_deref(),
            "HEAD read"
        );
        Ok(head)
    }
}

#[cfg(test)]
mod tests {
    use semoxide_error::ErrorInfo;
    use semoxide_test_support::git_fixture::GitFixture;

    use super::*;
    use crate::codes::{GIT_NO_COMMITS, GIT_READ_FAILED};

    fn fixture() -> GitFixture {
        GitFixture::builder()
            .commit("feat: first")
            .tag("v1.0.0")
            .commit("fix: second")
            .branch("feature/x")
            .commit("feat: on a branch with a slash")
            .checkout("main")
            .build()
            .unwrap()
    }

    /// `HEAD` as `(commit, branch)`.
    fn ours(fixture: &GitFixture) -> Result<(String, Option<String>), String> {
        Repo::discover(&fixture.path())
            .and_then(|repo| repo.head())
            .map(|head| (head.commit().to_string(), head.branch().map(str::to_owned)))
            .map_err(|error| error.to_string())
    }

    /// `HEAD` as git sees it: `--abbrev-ref` says `HEAD` when detached.
    fn by_git(fixture: &GitFixture) -> (String, Option<String>) {
        let branch = fixture.git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
        (
            fixture.rev_parse("HEAD").unwrap(),
            (branch != "HEAD").then_some(branch),
        )
    }

    #[test]
    fn on_a_branch_head_matches_git() {
        let fixture = fixture();
        let on_main = (ours(&fixture), by_git(&fixture));
        fixture.git(&["checkout", "feature/x"]).unwrap();
        let on_feature = (ours(&fixture), by_git(&fixture));

        assert_eq!((on_main.0, on_feature.0), (Ok(on_main.1), Ok(on_feature.1)));
    }

    #[test]
    fn a_detached_head_has_no_branch() {
        let fixture = fixture();
        fixture.git(&["checkout", "v1.0.0"]).unwrap();
        let on_tag = ours(&fixture);
        let sha = fixture.rev_parse("main").unwrap();
        fixture.git(&["checkout", &sha]).unwrap();
        let on_commit = ours(&fixture);

        assert_eq!(
            (on_tag, on_commit),
            (
                Ok((fixture.rev_parse("v1.0.0^{commit}").unwrap(), None)),
                Ok((sha, None))
            )
        );
    }

    #[test]
    fn a_repository_without_commits_is_an_error() {
        let fixture = GitFixture::builder().build().unwrap();
        let repo = Repo::discover(&fixture.path()).unwrap();
        let workdir = repo.workdir().to_owned();

        assert_eq!(
            repo.head()
                .map_err(|error| (error.code(), error.to_string(), error.help())),
            Err((
                GIT_NO_COMMITS,
                format!("the repository at `{}` has no commits", workdir.display()),
                Some(String::from(
                    "Make a first commit, then run semoxide again."
                ))
            ))
        );
    }

    #[test]
    fn an_orphan_branch_has_no_commits() {
        let fixture = fixture();
        fixture.git(&["checkout", "--orphan", "fresh"]).unwrap();
        let repo = Repo::discover(&fixture.path()).unwrap();

        assert_eq!(
            repo.head().map_err(|error| error.code()),
            Err(GIT_NO_COMMITS)
        );
    }

    #[test]
    fn a_broken_head_is_a_read_failure() {
        let fixture = fixture();
        let repo = Repo::discover(&fixture.path()).unwrap();
        let branch = fixture
            .path()
            .join(".git")
            .join("refs")
            .join("heads")
            .join("main");
        let code = || repo.head().map_err(|error| error.code());
        std::fs::write(&branch, "garbage\n").unwrap();
        let unreadable = code();
        std::fs::write(&branch, "1111111111111111111111111111111111111111\n").unwrap();
        let missing = code();

        assert_eq!(
            (unreadable, missing),
            (Err(GIT_READ_FAILED), Err(GIT_READ_FAILED))
        );
    }

    /// Only a local branch counts: git's `--abbrev-ref` would say `origin/x`.
    #[test]
    fn head_on_a_remote_branch_has_no_branch() {
        let fixture = fixture();
        fixture
            .git(&["update-ref", "refs/remotes/origin/x", "feature/x"])
            .unwrap();
        fixture
            .git(&["symbolic-ref", "HEAD", "refs/remotes/origin/x"])
            .unwrap();

        assert_eq!(
            ours(&fixture),
            Ok((fixture.rev_parse("feature/x").unwrap(), None))
        );
    }

    /// The commit, not the tag object `git rev-parse HEAD` would give.
    #[test]
    fn head_on_an_annotated_tag_gives_its_commit() {
        let fixture = fixture();
        fixture
            .git(&["tag", "-a", "v2.0.0", "-m", "annotated", "main"])
            .unwrap();
        fixture
            .git(&["symbolic-ref", "HEAD", "refs/tags/v2.0.0"])
            .unwrap();

        assert_eq!(
            ours(&fixture),
            Ok((fixture.rev_parse("main").unwrap(), None))
        );
    }

    /// Windows can't hold such a branch name: libgit2 fails to map it to a file name before
    /// decoding it, so this runs on Unix only.
    #[cfg(unix)]
    #[test]
    fn a_branch_name_that_isnt_utf_8_is_decoded_lossily() {
        let fixture = fixture();
        fixture.git(&["pack-refs", "--all"]).unwrap();
        let git_dir = fixture.path().join(".git");
        let packed = git_dir.join("packed-refs");
        // The appended ref breaks the sort order the header promises.
        let text = std::fs::read(&packed).unwrap();
        let mut text = String::from_utf8(text)
            .unwrap()
            .replace(" sorted", "")
            .into_bytes();
        text.extend(fixture.rev_parse("feature/x").unwrap().as_bytes());
        text.extend(b" refs/heads/f\xff\n");
        std::fs::write(&packed, text).unwrap();
        std::fs::write(git_dir.join("HEAD"), b"ref: refs/heads/f\xff\n").unwrap();

        assert_eq!(
            ours(&fixture),
            Ok((
                fixture.rev_parse("feature/x").unwrap(),
                Some(String::from("f\u{fffd}"))
            ))
        );
    }
}
