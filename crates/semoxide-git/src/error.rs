//! [`GitError`]: what went wrong reading the repository.

use std::fmt;
use std::path::{Path, PathBuf};

use semoxide_error::{ErrorCode, ErrorInfo};

use crate::codes::GIT_NO_REPOSITORY;

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
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl std::error::Error for GitError {}

impl ErrorInfo for GitError {
    fn code(&self) -> ErrorCode {
        GIT_NO_REPOSITORY
    }

    fn help(&self) -> Option<String> {
        None
    }
}
