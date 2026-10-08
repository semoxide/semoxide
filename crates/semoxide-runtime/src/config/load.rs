//! Loading the config of a directory: discovery (`semoxide.toml`, then `.config/semoxide.toml`),
//! the layer merge with the `--set` flags, and validation (CONFIG.md §1).

use std::fmt;
use std::path::{Path, PathBuf};

use semoxide_error::{ErrorCode, ErrorInfo};
use semoxide_schema::config::{Config, ConfigError};

use super::flag::FlagError;
use super::merge::Merged;
use crate::codes::CONFIG_UNREADABLE;

#[cfg(test)]
mod tests;

/// A loaded config: the validated [`Config`], the merged table with the source of every value,
/// the file used and any other config file that was ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    config: Config,
    merged: Merged,
    file: Option<PathBuf>,
    ignored: Vec<PathBuf>,
}

impl Loaded {
    /// The validated config.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The merged table and the source of every value.
    #[must_use]
    pub fn merged(&self) -> &Merged {
        &self.merged
    }

    /// The config file used, or `None` when none was found.
    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// Config files found but not used, e.g. `.config/semoxide.toml` next to `semoxide.toml`.
    #[must_use]
    pub fn ignored(&self) -> &[PathBuf] {
        &self.ignored
    }
}

/// Loads the config of `dir` with the `--set` flags, in order.
///
/// # Errors
///
/// Returns [`LoadError`] if a flag is malformed, the config file can't be read or isn't TOML,
/// or the merged config is invalid.
pub fn load(dir: &Path, flags: &[String]) -> Result<Loaded, LoadError> {
    let _ = flags;
    Err(LoadError::File(FileError {
        code: CONFIG_UNREADABLE,
        path: dir.to_path_buf(),
        message: String::from("not implemented"),
    }))
}

/// Why a config couldn't be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// A malformed `--set` flag.
    Flag(FlagError),
    /// The config file can't be read or isn't TOML.
    File(FileError),
    /// The merged config is invalid.
    Config(ConfigError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(error) => error.fmt(f),
            Self::File(error) => error.fmt(f),
            Self::Config(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LoadError {}

impl ErrorInfo for LoadError {
    fn code(&self) -> ErrorCode {
        match self {
            Self::Flag(error) => error.code(),
            Self::File(error) => error.code(),
            Self::Config(error) => error.code(),
        }
    }

    fn help(&self) -> Option<String> {
        match self {
            Self::Flag(error) => error.help(),
            Self::File(error) => error.help(),
            Self::Config(error) => error.help(),
        }
    }
}

/// A config file that can't be read or isn't TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError {
    code: ErrorCode,
    path: PathBuf,
    message: String,
}

impl FileError {
    /// The config file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for FileError {}

impl ErrorInfo for FileError {
    fn code(&self) -> ErrorCode {
        self.code.clone()
    }
}
