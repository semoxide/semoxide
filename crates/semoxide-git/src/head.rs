//! `HEAD`: the commit it points to (G14) and the local branch, unless detached (G7).

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
        Err(GitError::no_remote("HEAD"))
    }
}

#[cfg(test)]
mod tests {
    use semoxide_error::ErrorInfo;
    use semoxide_test_support::git_fixture::GitFixture;

    use super::*;
    use crate::codes::GIT_NO_COMMITS;

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
}
