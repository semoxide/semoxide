//! Namespaced error codes such as `core::no_git_repo` (OBSERVABILITY §8).

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::core("core::no_git_repo", "core/no-git-repo")]
    #[case::git("git::push_rejected", "git/push-rejected")]
    #[case::plugin_namespace_with_dash(
        "commit-analyzer::invalid_rule",
        "commit-analyzer/invalid-rule"
    )]
    #[case::digits("github::release_exists_2", "github/release-exists-2")]
    fn valid_code_parses_with_its_slug(#[case] input: &str, #[case] slug: &str) {
        let code: ErrorCode = input.parse().unwrap();

        assert_eq!(code.as_str(), input);
        assert_eq!(code.to_string(), input);
        assert_eq!(code.slug(), slug);
        assert_eq!(ErrorCode::from_slug(slug).unwrap(), code);
    }

    #[rstest]
    #[case::single_colon("core:no_git_repo")]
    #[case::two_separators("core::no::git")]
    #[case::empty_namespace("::no_git_repo")]
    #[case::empty_name("core::")]
    #[case::uppercase_namespace("Core::no_git_repo")]
    #[case::uppercase_name("core::NoGitRepo")]
    #[case::dash_in_name("core::no-git-repo")]
    #[case::namespace_starts_with_digit("2core::x")]
    #[case::name_starts_with_underscore("core::_x")]
    #[case::upstream_mnemonic("ENOGITREPO")]
    #[case::namespace_trailing_dash("commit-analyzer-::x")]
    #[case::namespace_double_dash("commit--analyzer::x")]
    #[case::leading_whitespace(" core::x")]
    #[case::trailing_newline("core::x\n")]
    #[case::non_ascii_letter("core::n\u{f6}")]
    #[case::cyrillic_lookalike("\u{441}ore::x")]
    fn invalid_code_is_rejected(#[case] input: &str) {
        assert!(input.parse::<ErrorCode>().is_err());
    }

    #[test]
    fn length_limit_is_64_characters() {
        let at_limit = format!("core::{}", "a".repeat(58));
        let over_limit = format!("core::{}", "a".repeat(59));
        assert_eq!((at_limit.len(), over_limit.len()), (64, 65));

        assert!(at_limit.parse::<ErrorCode>().is_ok());
        assert!(over_limit.parse::<ErrorCode>().is_err());
    }

    #[rstest]
    #[case::no_separator("core")]
    #[case::uppercase("Core/x")]
    #[case::empty_name("core/")]
    #[case::not_the_canonical_slug("core/no_git_repo")]
    fn invalid_slug_is_rejected(#[case] slug: &str) {
        assert!(ErrorCode::from_slug(slug).is_err());
    }

    proptest! {
        #[test]
        fn slug_round_trips(
            namespace in "[a-z][a-z0-9]{0,9}(-[a-z0-9]{1,9}){0,2}",
            name in "[a-z][a-z0-9_]{0,20}",
        ) {
            let code: ErrorCode = format!("{namespace}::{name}").parse().unwrap();

            prop_assert_eq!(ErrorCode::from_slug(&code.slug()).unwrap(), code);
        }
    }

    fn message(input: &str) -> String {
        let error: InvalidErrorCode = input.parse::<ErrorCode>().unwrap_err();
        error.to_string()
    }

    #[test]
    fn message_names_the_bad_input_and_the_expected_shape() {
        assert_eq!(
            message("Core::bad"),
            "invalid error code `Core::bad`: expected `<namespace>::<name>`"
        );
    }

    #[test]
    fn message_follows_the_style_guide() {
        let text = message("core:x");

        assert!(text.starts_with(char::is_lowercase), "{text}");
        assert!(!text.ends_with(['.', '!', '?']), "{text}");
        assert!(!text.contains('\n'), "{text}");
    }
}
