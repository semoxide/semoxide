//! Namespaced error codes such as `core::no_git_repo` (OBSERVABILITY §8).

use std::fmt;
use std::str::FromStr;

#[cfg(test)]
mod tests;

/// A validated error code: `<namespace>::<name>`.
///
/// The namespace is lowercase letters and digits with single dashes between parts
/// (`[a-z][a-z0-9]*(-[a-z0-9]+)*`, a plugin's name is its namespace), the name is
/// `[a-z][a-z0-9_]*`, and the whole code is at most 64 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorCode {
    code: String,
}

impl ErrorCode {
    /// The code as written, e.g. `core::no_git_repo`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.code
    }

    /// The docs path, e.g. `core/no-git-repo`: `<namespace>/<name with _ replaced by ->`.
    #[must_use]
    pub fn slug(&self) -> String {
        // Stub (test commit): wrong but valid value.
        String::from("wrong/stub")
    }

    /// Parses a slug back into the code it was made from.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidErrorCode`] if the slug doesn't come from a valid code.
    pub fn from_slug(_slug: &str) -> Result<Self, InvalidErrorCode> {
        // Stub (test commit): wrong but valid value.
        Ok(Self {
            code: String::from("wrong::stub"),
        })
    }
}

impl FromStr for ErrorCode {
    type Err = InvalidErrorCode;

    fn from_str(_code: &str) -> Result<Self, Self::Err> {
        // Stub (test commit): wrong but valid value.
        Ok(Self {
            code: String::from("wrong::stub"),
        })
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.code)
    }
}

/// A string that is not a valid [`ErrorCode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidErrorCode {
    input: String,
}

impl fmt::Display for InvalidErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid error code `{}`: expected `<namespace>::<name>`",
            self.input
        )
    }
}

impl std::error::Error for InvalidErrorCode {}
