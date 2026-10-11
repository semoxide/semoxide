//! [`Repo`]: the repository a run releases from, found by walking up from its directory
//! (ARCHITECTURE §6).

use std::path::Path;

use tracing::debug;

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
    pub(crate) fn discover_below(cwd: &Path, ceilings: &[&Path]) -> Result<Self, GitError> {
        // No `FROM_ENV`: libgit2 would read `GIT_DIR` and the like from the process env.
        let repo = git2::Repository::open_ext(cwd, git2::RepositoryOpenFlags::empty(), ceilings)
            .map_err(|error| open_error(&error, cwd))?;
        if repo.is_bare() {
            return Err(GitError::bare_repository(cwd));
        }
        let repo = Self { repo };
        debug!(
            target: "semoxide::git",
            op = "G3",
            workdir = %repo.workdir().display(),
            "repository found"
        );
        Ok(repo)
    }

    /// The working tree's top directory.
    #[must_use]
    pub fn workdir(&self) -> &Path {
        // Bare repositories are rejected when opened, so there is always a working tree.
        self.repo.workdir().unwrap_or_else(|| self.repo.path())
    }

    /// The URL of the remote `name`, with `url.<base>.insteadOf` applied.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if there is no such remote or libgit2 fails to read it.
    pub fn remote_url(&self, name: &str) -> Result<String, GitError> {
        let remote = self
            .repo
            .find_remote(name)
            .map_err(|error| match error.code() {
                git2::ErrorCode::NotFound | git2::ErrorCode::InvalidSpec => {
                    GitError::no_remote(name)
                }
                _ => GitError::read_failed(&error),
            })?;
        let url = remote
            .url()
            .map_err(|error| GitError::read_failed(&error))?;
        // The URL may carry credentials, so only the remote's name is logged.
        debug!(target: "semoxide::git", op = "G2", remote = name, "remote URL read");
        Ok(url.to_owned())
    }
}

/// The error for libgit2 failing to open the repository found from `cwd`.
fn open_error(error: &git2::Error, cwd: &Path) -> GitError {
    match error.code() {
        git2::ErrorCode::NotFound => GitError::no_repository(cwd),
        git2::ErrorCode::Owner => GitError::unsafe_repository(cwd),
        _ => GitError::read_failed(error),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use semoxide_error::{ErrorCode, ErrorInfo};
    use semoxide_test_support::git_fixture::GitFixture;

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

    /// The walk stops at the ceiling, so the fixture's repository above it is never found.
    #[test]
    fn no_repository_at_or_above_is_an_error() {
        let fixture = fixture();
        let ceiling = fixture.path().join("outside");
        let cwd = ceiling.join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        assert_eq!(
            failure(Repo::discover_below(&cwd, &[&ceiling])),
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
    fn a_linked_worktree_is_its_own_working_tree() {
        let fixture = fixture();
        fixture.git(&["worktree", "add", "../worktree"]).unwrap();
        let worktree = fixture.path().with_file_name("worktree");

        assert_eq!(
            Repo::discover(&worktree).map(|repo| canonical(repo.workdir())),
            Ok(canonical(&worktree))
        );
    }

    #[test]
    fn a_repository_libgit2_cannot_open_is_a_read_failure() {
        let fixture = fixture();
        let config = fixture.path().join(".git").join("config");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(
            &config,
            text.replace(
                "repositoryformatversion = 0",
                "repositoryformatversion = 99",
            ),
        )
        .unwrap();

        // libgit2's wording isn't ours to pin.
        assert_eq!(
            failure(Repo::discover(&fixture.path())).map(|(code, _, help)| (code, help)),
            Some((GIT_READ_FAILED, None))
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
    fn a_not_found_error_is_no_repository() {
        let cwd = Path::new("/work/repo");
        let missing = git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Repository,
            "could not find repository at '/work/repo'",
        );

        assert_eq!(open_error(&missing, cwd), GitError::no_repository(cwd));
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
    fn remote_urls_match_git_including_insteadof_and_not_the_push_url() {
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
        // The push URL must not replace the URL.
        fixture
            .git(&[
                "remote",
                "set-url",
                "--push",
                "origin",
                "https://example.invalid/push.git",
            ])
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
