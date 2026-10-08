//! Loading the config of a directory: discovery (`semoxide.toml`, then `.config/semoxide.toml`),
//! the layer merge with the `--set` flags, and validation (CONFIG.md §1).

use std::fmt;
use std::path::{Path, PathBuf};

use semoxide_error::{ErrorCode, ErrorInfo};
use semoxide_schema::config::{Config, ConfigError, MergeMode};
use toml::Table;

use super::flag::{FlagError, parse_flag};
use super::merge::{Layer, Merged, Source, merge};
use crate::codes::{CONFIG_INVALID_TOML, CONFIG_UNREADABLE};

#[cfg(test)]
mod mutation_tests;
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

/// Config files looked for in a directory, in order; the first one found is used.
const CANDIDATES: [&str; 2] = ["semoxide.toml", ".config/semoxide.toml"];

/// Loads the config of `dir` with the `--set` flags, in order.
///
/// # Errors
///
/// Returns [`LoadError`] if a flag is malformed, the config file can't be read or isn't TOML,
/// or the merged config is invalid.
pub fn load(dir: &Path, flags: &[String]) -> Result<Loaded, LoadError> {
    let flags = flags
        .iter()
        .enumerate()
        .map(|(index, flag)| parse_flag(index + 1, flag))
        .collect::<Result<Vec<_>, _>>()
        .map_err(LoadError::Flag)?;
    let mut found = CANDIDATES
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.exists());
    let file = found.next();
    let ignored: Vec<PathBuf> = found.collect();
    let layers = match &file {
        Some(path) => vec![Layer::new(Source::File(path.clone()), read(path)?)],
        None => Vec::new(),
    };
    // With a single user layer, shallow and deep give the same result; `config.merge` takes effect
    // once `extends` adds a second one.
    let merged = merge(&Config::defaults_table(), &layers, &flags, MergeMode::Deep);
    let config = Config::from_table(merged.table().clone()).map_err(LoadError::Config)?;
    Ok(Loaded {
        config,
        merged,
        file,
        ignored,
    })
}

fn read(path: &Path) -> Result<Table, LoadError> {
    let file_error = |code: ErrorCode, message: String| {
        LoadError::File(FileError {
            code,
            path: path.to_path_buf(),
            message,
        })
    };
    let text = std::fs::read_to_string(path)
        .map_err(|error| file_error(CONFIG_UNREADABLE, error.to_string()))?;
    text.parse::<Table>()
        .map_err(|error| file_error(CONFIG_INVALID_TOML, error.to_string().trim_end().to_owned()))
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
