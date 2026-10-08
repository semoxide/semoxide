//! `--set <key>=<value>`: a dotted TOML key without array indexes, and a TOML value or a plain
//! string (CLI.md).

use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};
use toml::Table;

use super::merge::{Layer, Source};
use crate::codes::CONFIG_INVALID_FLAG;

#[cfg(test)]
mod tests;

/// Reads the `position`th `--set` flag (counting from 1) as a one-key layer.
///
/// # Errors
///
/// Returns [`FlagError`] if the flag isn't `<key>=<value>` as CLI.md describes.
pub fn parse_flag(position: usize, flag: &str) -> Result<Layer, FlagError> {
    let _ = (position, Source::Flag(position), Table::new());
    Err(FlagError {
        position: 0,
        flag: flag.to_owned(),
        problem: String::from("not implemented"),
    })
}

/// A `--set` flag that can't be read as `<key>=<value>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagError {
    position: usize,
    flag: String,
    problem: String,
}

impl FlagError {
    /// Which `--set` flag it is, counting from 1.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }
}

impl fmt::Display for FlagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`--set {}`: {}", self.flag, self.problem)
    }
}

impl std::error::Error for FlagError {}

impl ErrorInfo for FlagError {
    fn code(&self) -> ErrorCode {
        CONFIG_INVALID_FLAG
    }
}
