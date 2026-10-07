//! Golden histories: a git history, its config and the expected release, one TOML file each
//! under `tests/histories/` (TESTING: fixtures).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::git_fixture::GitFixture;

/// One `tests/histories/*.toml` file.
#[derive(Debug)]
pub struct GoldenHistory {
    path: PathBuf,
    description: String,
    config: Option<toml::Table>,
    expected: Expected,
}

/// What semoxide should decide for a history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    /// A release.
    Release(ExpectedRelease),
    /// No release, with the `NoReleaseReason` in snake case (`no_relevant_commits`).
    NoRelease(String),
}

/// The expected release of a history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedRelease {
    version: String,
    release_type: ReleaseType,
    channel: Option<String>,
}

impl ExpectedRelease {
    /// The next version, e.g. `2.0.0-beta.1`.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The level the commits asked for.
    #[must_use]
    pub fn release_type(&self) -> ReleaseType {
        self.release_type
    }

    /// The channel, or `None` for the branch's default channel.
    #[must_use]
    pub fn channel(&self) -> Option<&str> {
        self.channel.as_deref()
    }
}

/// The level of a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseType {
    /// `major`
    Major,
    /// `minor`
    Minor,
    /// `patch`
    Patch,
}

impl GoldenHistory {
    /// Reads and parses one history file.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] if the file can't be read or isn't a valid history.
    pub fn load(path: &Path) -> Result<Self, HistoryError> {
        Err(HistoryError::new(path, "not implemented"))
    }

    /// Every `*.toml` history in `dir`, sorted by file name.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] for the first file that can't be loaded.
    pub fn all(dir: &Path) -> Result<Vec<Self>, HistoryError> {
        Err(HistoryError::new(dir, "not implemented"))
    }

    /// Parses `text` as the history file at `path` (used in error messages).
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] if `text` isn't a valid history.
    pub fn parse(path: &Path, text: &str) -> Result<Self, HistoryError> {
        let _ = text;
        Err(HistoryError::new(path, "not implemented"))
    }

    /// The file the history was loaded from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One line on what the history shows.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The `[config]` table, written as `semoxide.toml` by [`Self::build`]; `None` = defaults.
    #[must_use]
    pub fn config(&self) -> Option<&toml::Table> {
        self.config.as_ref()
    }

    /// What semoxide should decide.
    #[must_use]
    pub fn expected(&self) -> &Expected {
        &self.expected
    }

    /// Runs the steps with [`GitFixture`] and writes `[config]` as an untracked `semoxide.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] if a step fails or the config can't be written.
    pub fn build(&self) -> Result<GitFixture, HistoryError> {
        Err(HistoryError::new(&self.path, "not implemented"))
    }
}

/// A history file that can't be loaded or built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryError {
    path: PathBuf,
    message: String,
}

impl HistoryError {
    fn new(path: &Path, message: &str) -> Self {
        Self {
            path: path.to_path_buf(),
            message: message.to_owned(),
        }
    }
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for HistoryError {}
