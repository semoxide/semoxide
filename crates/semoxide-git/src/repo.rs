//! [`Repo`]: the repository a run releases from, found by walking up from its directory
//! (ARCHITECTURE §6).

use std::path::Path;

use crate::GitError;

/// An open repository with a working tree.
pub struct Repo {
    repo: git2::Repository,
}

impl Repo {
    /// The repository at or above `cwd`, like `git rev-parse --show-toplevel`.
    ///
    /// Ignores `GIT_DIR` and the other `GIT_*` variables: the process env is never read.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if there is no repository, it is owned by another user and not
    /// listed in `safe.directory`, it is bare, or libgit2 fails to open it.
    pub fn discover(cwd: &Path) -> Result<Self, GitError> {
        Self::discover_below(cwd, &[])
    }

    /// [`Self::discover`], stopping the walk at `ceilings`.
    pub(crate) fn discover_below(cwd: &Path, _ceilings: &[&Path]) -> Result<Self, GitError> {
        Err(GitError::no_repository(cwd))
    }

    /// The working tree's top directory.
    #[must_use]
    pub fn workdir(&self) -> &Path {
        self.repo.path()
    }

    /// The URL of the remote `name`, with `url.<base>.insteadOf` applied.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if there is no such remote or libgit2 fails to read it.
    pub fn remote_url(&self, name: &str) -> Result<String, GitError> {
        Err(GitError::no_remote(name))
    }
}

/// The error for libgit2 failing to open the repository found from `cwd`.
fn open_error(error: &git2::Error, _cwd: &Path) -> GitError {
    GitError::read_failed(error)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use semoxide_error::{ErrorCode, ErrorInfo};
    use semoxide_test_support::git_fixture::GitFixture;
    use tempfile::TempDir;

    use super::*;
    use crate::codes::{
        GIT_BARE_REPOSITORY, GIT_NO_REMOTE, GIT_NO_REPOSITORY, GIT_READ_FAILED,
        GIT_UNSAFE_REPOSITORY,
    };

    fn fixture() -> GitFixture {
        GitFixture::builder().commit("feat: first").build().unwrap()
    }

    fn canonical(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap()
    }

    /// The code, message and help of `result`'s error, if it failed.
    fn failure<T>(result: Result<T, GitError>) -> Option<(ErrorCode, String, Option<String>)> {
        result
            .err()
            .map(|error| (error.code(), error.to_string(), error.help()))
    }

    // --- Discovery ---

    #[test]
    fn the_repository_is_found_from_its_root_and_a_subfolder_as_git_finds_it() {
        let fixture = fixture();
        let subfolder = fixture.path().join("src").join("deep");
        std::fs::create_dir_all(&subfolder).unwrap();
        let toplevel = canonical(Path::new(
            &fixture.git(&["rev-parse", "--show-toplevel"]).unwrap(),
        ));

        let found = |cwd: &Path| Repo::discover(cwd).map(|repo| canonical(repo.workdir()));

        assert_eq!(
            (found(&fixture.path()), found(&subfolder)),
            (Ok(toplevel.clone()), Ok(toplevel))
        );
    }

    #[test]
    fn no_repository_at_or_above_is_an_error() {
        let outside = TempDir::new().unwrap();
        let cwd = outside.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        assert_eq!(
            failure(Repo::discover_below(&cwd, &[outside.path()])),
            Some((
                GIT_NO_REPOSITORY,
                format!("no git repository at or above `{}`", cwd.display()),
                Some(String::from(
                    "Run semoxide in a clone, or point `--cwd` at one."
                ))
            ))
        );
    }

    #[test]
    fn a_bare_repository_is_rejected() {
        let fixture = fixture();
        let cwd = fixture.remote_path();

        assert_eq!(
            failure(Repo::discover(&cwd)),
            Some((
                GIT_BARE_REPOSITORY,
                format!("the repository at or above `{}` is bare", cwd.display()),
                Some(String::from("Run semoxide in a clone with a working tree."))
            ))
        );
    }

    #[test]
    fn an_ownership_refusal_is_an_unsafe_repository() {
        let cwd = Path::new("/work/repo");
        let refused = git2::Error::new(
            git2::ErrorCode::Owner,
            git2::ErrorClass::Config,
            "repository path '/work/repo/' is not owned by current user",
        );

        let error = open_error(&refused, cwd);

        assert_eq!(
            (error.code(), error.to_string(), error.help()),
            (
                GIT_UNSAFE_REPOSITORY,
                format!(
                    "the repository at or above `{}` is owned by another user",
                    cwd.display()
                ),
                Some(String::from(
                    "Run `git config --global --add safe.directory <path>` with the \
                     repository's path if you trust it."
                ))
            )
        );
    }

    #[test]
    fn other_open_errors_keep_libgit2s_message() {
        let broken = git2::Error::new(
            git2::ErrorCode::GenericError,
            git2::ErrorClass::Odb,
            "object file is corrupt",
        );

        let error = open_error(&broken, Path::new("/work/repo"));

        assert_eq!(
            (error.code(), error.to_string(), error.help()),
            (
                GIT_READ_FAILED,
                String::from("reading the repository: object file is corrupt"),
                None
            )
        );
    }

    // --- Remote URL ---

    #[test]
    fn remote_urls_match_git_including_insteadof() {
        let fixture = fixture();
        fixture
            .git(&[
                "remote",
                "add",
                "upstream",
                "https://example.invalid/o/r.git",
            ])
            .unwrap();
        fixture
            .git(&["config", "url.https://example.invalid/.insteadOf", "short:"])
            .unwrap();
        fixture
            .git(&["remote", "add", "short", "short:owner/repo.git"])
            .unwrap();
        let repo = Repo::discover(&fixture.path());

        for name in ["origin", "upstream", "short"] {
            let expected = fixture.git(&["remote", "get-url", name]).unwrap();
            assert_eq!(
                repo.as_ref()
                    .map_err(Clone::clone)
                    .and_then(|repo| repo.remote_url(name)),
                Ok(expected),
                "{name}"
            );
        }
    }

    #[test]
    fn a_missing_or_invalid_remote_is_an_error() {
        let fixture = fixture();
        let repo = Repo::discover(&fixture.path());
        let failed = |name: &str| {
            failure(
                repo.as_ref()
                    .map_err(Clone::clone)
                    .and_then(|repo| repo.remote_url(name)),
            )
        };
        let no_remote = |name: &str| {
            Some((
                GIT_NO_REMOTE,
                format!("no remote `{name}`"),
                Some(format!(
                    "Add it with `git remote add {name} <url>`, or set `git.remote` or `git.url`."
                )),
            ))
        };

        assert_eq!(
            (failed("upstream"), failed("bad name")),
            (no_remote("upstream"), no_remote("bad name"))
        );
    }
}
