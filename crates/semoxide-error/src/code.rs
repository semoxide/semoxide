//! Namespaced error codes such as `core::no_git_repo` (OBSERVABILITY §8).

use std::fmt;
use std::str::FromStr;

#[cfg(test)]
mod tests;

const MAX_LEN: usize = 64;
const SEPARATOR: &str = "::";

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
        let (namespace, name) = self.parts();
        format!("{namespace}/{}", name.replace('_', "-"))
    }

    /// Parses a slug back into the code it was made from.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidErrorCode`] unless the slug is exactly the slug of a valid code.
    pub fn from_slug(slug: &str) -> Result<Self, InvalidErrorCode> {
        let invalid = || InvalidErrorCode {
            input: slug.to_owned(),
        };
        let (namespace, name) = slug.split_once('/').ok_or_else(invalid)?;
        let code: Self = format!("{namespace}{SEPARATOR}{}", name.replace('-', "_"))
            .parse()
            .map_err(|_| invalid())?;
        // One code, one slug: `core/no_git_repo` must not also mean `core::no_git_repo`.
        if code.slug() != slug {
            return Err(invalid());
        }
        Ok(code)
    }

    fn parts(&self) -> (&str, &str) {
        // Validated on construction, so the separator is always present.
        self.code
            .split_once(SEPARATOR)
            .unwrap_or((self.code.as_str(), ""))
    }
}

impl FromStr for ErrorCode {
    type Err = InvalidErrorCode;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        let valid = code.len() <= MAX_LEN
            && code
                .split_once(SEPARATOR)
                .is_some_and(|(namespace, name)| is_namespace(namespace) && is_name(name));
        if !valid {
            return Err(InvalidErrorCode {
                input: code.to_owned(),
            });
        }
        Ok(Self {
            code: code.to_owned(),
        })
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.code)
    }
}

/// `[a-z][a-z0-9]*(-[a-z0-9]+)*`
fn is_namespace(namespace: &str) -> bool {
    namespace.starts_with(|c: char| c.is_ascii_lowercase())
        && namespace.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// `[a-z][a-z0-9_]*`
fn is_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
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
