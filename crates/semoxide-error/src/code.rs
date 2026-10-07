//! Namespaced error codes such as `core::no_git_repo` (OBSERVABILITY §8).

use std::borrow::Cow;
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
///
/// A crate declares its codes as constants named after the full code:
///
/// ```
/// use semoxide_error::ErrorCode;
///
/// const CORE_NO_GIT_REPO: ErrorCode = ErrorCode::from_static("core::no_git_repo");
/// assert_eq!(CORE_NO_GIT_REPO.slug(), "core/no-git-repo");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorCode {
    code: Cow<'static, str>,
}

impl ErrorCode {
    /// A code known at compile time. In a `const` item an invalid code is a build error:
    ///
    /// ```compile_fail
    /// use semoxide_error::ErrorCode;
    ///
    /// const CORE_BAD: ErrorCode = ErrorCode::from_static("Core::bad");
    /// let _ = CORE_BAD;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `code` is not a valid error code (only reachable outside `const` items).
    #[must_use]
    pub const fn from_static(code: &'static str) -> Self {
        assert!(
            is_valid(code.as_bytes()),
            "invalid error code: expected `<namespace>::<name>`"
        );
        Self {
            code: Cow::Borrowed(code),
        }
    }

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
            .unwrap_or((self.as_str(), ""))
    }
}

impl FromStr for ErrorCode {
    type Err = InvalidErrorCode;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        if !is_valid(code.as_bytes()) {
            return Err(InvalidErrorCode {
                input: code.to_owned(),
            });
        }
        Ok(Self {
            code: Cow::Owned(code.to_owned()),
        })
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.code)
    }
}

/// `<namespace>::<name>` with `[a-z][a-z0-9]*(-[a-z0-9]+)*` and `[a-z][a-z0-9_]*`, at most
/// [`MAX_LEN`] bytes. A `const fn` so constants are checked at compile time; it walks the
/// bytes with slice patterns because iterators and `get` aren't available in `const fn`.
const fn is_valid(code: &[u8]) -> bool {
    if code.len() > MAX_LEN {
        return false;
    }
    let mut rest = code;
    let mut first_byte = true;
    let mut in_name = false;
    // At the start of a part: the namespace, a dash-separated namespace part, or the name.
    let mut part_start = true;
    while let [byte, tail @ ..] = rest {
        let byte = *byte;
        rest = tail;
        if part_start {
            // A namespace part after a dash may start with a digit; the code and the name may not.
            let digit_ok = !in_name && !first_byte;
            if !(byte.is_ascii_lowercase() || (digit_ok && byte.is_ascii_digit())) {
                return false;
            }
            part_start = false;
        } else if byte == b':' {
            let [b':', after @ ..] = rest else {
                return false;
            };
            if in_name {
                return false;
            }
            rest = after;
            in_name = true;
            part_start = true;
        } else if byte == b'-' {
            if in_name {
                return false;
            }
            part_start = true;
        } else if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || (in_name && byte == b'_'))
        {
            return false;
        }
        first_byte = false;
    }
    in_name && !part_start
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
