//! [`GitError`]: what went wrong reading the repository.

use std::fmt;
use std::path::{Path, PathBuf};

use semoxide_error::{ErrorCode, ErrorInfo};

use crate::codes::{
    GIT_BARE_REPOSITORY, GIT_NO_COMMITS, GIT_NO_REMOTE, GIT_NO_REPOSITORY, GIT_READ_FAILED,
    GIT_UNSAFE_REPOSITORY,
};

/// An error reading the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError {
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// Nothing found walking up from the directory.
    NoRepository {
        cwd: PathBuf,
    },
    /// libgit2's ownership check refused the repository found from the directory.
    UnsafeRepository {
        cwd: PathBuf,
    },
    /// The repository found from the directory has no working tree.
    BareRepository {
        cwd: PathBuf,
    },
    NoRemote {
        name: String,
    },
    /// `HEAD` points at a branch with no commits yet.
    NoCommits {
        workdir: PathBuf,
    },
    /// libgit2's message.
    ReadFailed {
        message: String,
    },
}

impl GitError {
    pub(crate) fn no_repository(cwd: &Path) -> Self {
        Self::new(Kind::NoRepository {
            cwd: cwd.to_owned(),
        })
    }

    pub(crate) fn unsafe_repository(cwd: &Path) -> Self {
        Self::new(Kind::UnsafeRepository {
            cwd: cwd.to_owned(),
        })
    }

    pub(crate) fn bare_repository(cwd: &Path) -> Self {
        Self::new(Kind::BareRepository {
            cwd: cwd.to_owned(),
        })
    }

    pub(crate) fn no_commits(workdir: &Path) -> Self {
        Self::new(Kind::NoCommits {
            workdir: workdir.to_owned(),
        })
    }

    pub(crate) fn no_remote(name: &str) -> Self {
        Self::new(Kind::NoRemote {
            name: name.to_owned(),
        })
    }

    pub(crate) fn read_failed(error: &git2::Error) -> Self {
        Self::new(Kind::ReadFailed {
            message: error.message().to_owned(),
        })
    }

    fn new(kind: Kind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Kind::NoRepository { cwd } => {
                write!(f, "no git repository at or above `{}`", cwd.display())
            }
            Kind::UnsafeRepository { cwd } => write!(
                f,
                "the repository at or above `{}` is owned by another user",
                cwd.display()
            ),
            Kind::BareRepository { cwd } => {
                write!(f, "the repository at or above `{}` is bare", cwd.display())
            }
            Kind::NoRemote { name } => write!(f, "no remote `{name}`"),
            Kind::NoCommits { workdir } => write!(
                f,
                "the repository at `{}` has no commits",
                workdir.display()
            ),
            Kind::ReadFailed { message } => write!(f, "reading the repository: {message}"),
        }
    }
}

impl std::error::Error for GitError {}

impl ErrorInfo for GitError {
    fn code(&self) -> ErrorCode {
        match self.kind {
            Kind::NoRepository { .. } => GIT_NO_REPOSITORY,
            Kind::UnsafeRepository { .. } => GIT_UNSAFE_REPOSITORY,
            Kind::BareRepository { .. } => GIT_BARE_REPOSITORY,
            Kind::NoRemote { .. } => GIT_NO_REMOTE,
            Kind::NoCommits { .. } => GIT_NO_COMMITS,
            Kind::ReadFailed { .. } => GIT_READ_FAILED,
        }
    }

    fn help(&self) -> Option<String> {
        let help = match &self.kind {
            Kind::NoRepository { .. } => {
                String::from("Run semoxide in a clone, or point `--cwd` at one.")
            }
            Kind::UnsafeRepository { .. } => String::from(
                "Run `git config --global --add safe.directory <path>` with the repository's \
                 path if you trust it.",
            ),
            Kind::BareRepository { .. } => {
                String::from("Run semoxide in a clone with a working tree.")
            }
            Kind::NoRemote { name } => format!(
                "Add it with `git remote add {name} <url>`, or set `git.remote` or `git.url`."
            ),
            Kind::NoCommits { .. } => String::from("Make a first commit, then run semoxide again."),
            Kind::ReadFailed { .. } => return None,
        };
        Some(help)
    }
}
