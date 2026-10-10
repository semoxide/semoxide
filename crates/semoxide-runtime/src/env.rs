//! [`Env`]: the snapshot of environment variables semoxide reads; nothing else reads the process
//! env (OBSERVABILITY P3).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};

use crate::codes::ENV_NOT_UNICODE;

/// The `SEMOXIDE_*` variables semoxide reads (CLI.md).
pub(crate) const KNOWN_VARS: [&str; 0] = [];

/// Whether variable names differ by letter case: they do on Unix, not on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
    Sensitive,
    Insensitive,
}

/// A snapshot of environment variables, built by the caller (the CLI from
/// `std::env::vars_os()`, tests from a map).
///
/// On Windows names compare ignoring ASCII case; the names semoxide reads are ASCII, so other
/// letters are left unspecified. Of names that differ only in case, the first in byte order wins.
///
/// ```
/// let env: semoxide_runtime::Env = [("SEMOXIDE_LOG", "debug")].into_iter().collect();
/// assert_eq!(env.get_str("SEMOXIDE_LOG"), Ok(Some("debug")));
/// ```
#[derive(Debug, Clone)]
pub struct Env {
    vars: BTreeMap<OsString, OsString>,
    case: Case,
}

impl Env {
    pub(crate) fn with_case<K, V>(case: Case, vars: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        Self {
            vars: vars
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
            case,
        }
    }

    /// The value of `name`, as set.
    #[must_use]
    pub fn get(&self, _name: &str) -> Option<&OsStr> {
        None
    }

    /// The value of `name` as text.
    ///
    /// # Errors
    ///
    /// Returns [`EnvError`] if the value isn't valid UTF-8.
    pub fn get_str(&self, _name: &str) -> Result<Option<&str>, EnvError> {
        Ok(None)
    }

    /// The `SEMOXIDE_*` variables semoxide doesn't know, in name order (likely typos).
    #[must_use]
    pub fn unknown_semoxide_vars(&self) -> Vec<UnknownVar> {
        Vec::new()
    }
}

impl<K, V> FromIterator<(K, V)> for Env
where
    K: Into<OsString>,
    V: Into<OsString>,
{
    /// Names compare as the operating system does: ignoring case on Windows.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(vars: I) -> Self {
        Self::with_case(Case::Sensitive, vars)
    }
}

/// A `SEMOXIDE_*` variable semoxide doesn't read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVar {
    name: String,
    suggestion: Option<&'static str>,
}

impl UnknownVar {
    /// The name, as set.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The closest known name, if one is close.
    #[must_use]
    pub fn suggestion(&self) -> Option<&'static str> {
        self.suggestion
    }
}

/// A variable whose value isn't valid UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvError {
    name: String,
}

impl fmt::Display for EnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`", self.name)
    }
}

impl std::error::Error for EnvError {}

impl ErrorInfo for EnvError {
    fn code(&self) -> ErrorCode {
        ENV_NOT_UNICODE
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use semoxide_error::ErrorInfo;

    use super::*;

    fn env(case: Case, vars: &[(&str, &str)]) -> Env {
        Env::with_case(case, vars.iter().copied())
    }

    fn os(text: &str) -> Option<&OsStr> {
        Some(OsStr::new(text))
    }

    /// A value that isn't valid UTF-8 on this platform.
    fn not_unicode() -> OsString {
        cfg_select! {
            unix => {
                use std::os::unix::ffi::OsStringExt;
                OsString::from_vec(vec![0xff])
            }
            windows => {
                use std::os::windows::ffi::OsStringExt;
                OsString::from_wide(&[0xD800])
            }
        }
    }

    // --- Lookup ---

    #[test]
    fn a_set_variable_is_found_and_a_missing_one_is_not() {
        let env = env(Case::Sensitive, &[("SEMOXIDE_LOG", "debug")]);

        assert_eq!(
            (env.get("SEMOXIDE_LOG"), env.get("SEMOXIDE_CI_BRANCH")),
            (os("debug"), None)
        );
    }

    #[test]
    fn case_sensitive_names_must_match_exactly() {
        let env = env(Case::Sensitive, &[("Path", "/bin")]);

        assert_eq!((env.get("Path"), env.get("PATH")), (os("/bin"), None));
    }

    #[test]
    fn case_insensitive_names_match_in_any_case() {
        let env = env(Case::Insensitive, &[("Path", "/bin")]);

        assert_eq!(
            (env.get("PATH"), env.get("path"), env.get("PATHS")),
            (os("/bin"), os("/bin"), None)
        );
    }

    #[test]
    fn collecting_compares_names_as_the_operating_system_does() {
        let env: Env = [("Path", "/bin")].into_iter().collect();

        assert_eq!(
            (env.get("Path"), env.get("PATH").is_some()),
            (os("/bin"), cfg!(windows))
        );
    }

    #[test]
    fn a_value_is_read_as_text() {
        let env = env(Case::Sensitive, &[("SEMOXIDE_CI_BRANCH", "main")]);

        assert_eq!(
            (
                env.get_str("SEMOXIDE_CI_BRANCH"),
                env.get_str("SEMOXIDE_CI_IS_PR")
            ),
            (Ok(Some("main")), Ok(None))
        );
    }

    #[test]
    fn a_value_that_isnt_utf_8_is_an_error_naming_the_variable() {
        let env = Env::with_case(
            Case::Sensitive,
            [(OsString::from("SEMOXIDE_CI_BRANCH"), not_unicode())],
        );

        let error = env.get_str("SEMOXIDE_CI_BRANCH").err();

        assert_eq!(
            error.map(|error| (error.code(), error.to_string())),
            Some((
                ENV_NOT_UNICODE,
                String::from("the value of `SEMOXIDE_CI_BRANCH` isn't valid UTF-8")
            ))
        );
    }

    #[test]
    fn the_error_says_to_set_utf_8_text() {
        let env = Env::with_case(
            Case::Sensitive,
            [(OsString::from("SEMOXIDE_CI_BRANCH"), not_unicode())],
        );

        let error = env.get_str("SEMOXIDE_CI_BRANCH").err();

        assert_eq!(
            error.and_then(|error| error.help()),
            Some(String::from("Set `SEMOXIDE_CI_BRANCH` to UTF-8 text."))
        );
    }

    #[test]
    fn a_name_that_only_starts_with_the_query_is_not_a_match() {
        let env = env(Case::Sensitive, &[("SEMOXIDE_LOG_X", "1")]);

        assert_eq!(
            (env.get("SEMOXIDE_LOG"), env.get("SEMOXIDE_LOG_X")),
            (None, os("1"))
        );
    }

    #[test]
    fn a_case_insensitive_name_that_only_starts_with_the_query_is_not_a_match() {
        let env = env(Case::Insensitive, &[("PATHS", "/bin")]);

        assert_eq!((env.get("path"), env.get("paths")), (None, os("/bin")));
    }

    #[test]
    fn the_error_names_the_variable_as_set() {
        let env = Env::with_case(
            Case::Insensitive,
            [(OsString::from("Semoxide_Ci_Branch"), not_unicode())],
        );

        let error = env.get_str("SEMOXIDE_CI_BRANCH").err();

        assert_eq!(
            error.map(|error| error.to_string()),
            Some(String::from(
                "the value of `Semoxide_Ci_Branch` isn't valid UTF-8"
            ))
        );
    }

    #[test]
    fn of_names_differing_only_in_case_the_first_in_byte_order_wins() {
        let env = env(Case::Insensitive, &[("Path", "b"), ("PATH", "a")]);

        assert_eq!(
            (env.get("path"), env.get_str("path")),
            (os("a"), Ok(Some("a")))
        );
    }

    // --- Unknown SEMOXIDE_* variables ---

    fn unknown(env: &Env) -> Vec<(String, Option<&'static str>)> {
        env.unknown_semoxide_vars()
            .iter()
            .map(|var| (var.name().to_owned(), var.suggestion()))
            .collect()
    }

    #[test]
    fn only_unknown_semoxide_vars_are_reported_in_name_order() {
        let env = env(
            Case::Sensitive,
            &[
                ("SEMOXIDE_LGO", "debug"),
                ("SEMOXIDE_LOG", "debug"),
                ("SEMOXIDE_COLOUR", "never"),
                ("SEMOXIDE_CI_BRANCH", "main"),
                ("SEMOXIDE_CI_IS_PR", "false"),
                ("SEMOXIDE_SSH_BACKEND", "exec"),
                ("PATH", "/bin"),
                ("semoxide_lgo", "debug"),
            ],
        );

        assert_eq!(
            unknown(&env),
            [
                (String::from("SEMOXIDE_COLOUR"), None),
                (String::from("SEMOXIDE_LGO"), Some("SEMOXIDE_LOG")),
            ]
        );
    }

    #[test]
    fn case_insensitive_names_are_known_and_reported_in_any_case() {
        let env = env(
            Case::Insensitive,
            &[("semoxide_log", "debug"), ("Semoxide_Lgo", "debug")],
        );

        assert_eq!(
            unknown(&env),
            [(String::from("Semoxide_Lgo"), Some("SEMOXIDE_LOG"))]
        );
    }

    #[test]
    fn case_sensitive_known_names_match_exactly_and_whole() {
        let env = env(
            Case::Sensitive,
            &[("SEMOXIDE_log", "debug"), ("SEMOXIDE_LOGS", "debug")],
        );

        assert_eq!(
            unknown(&env),
            [
                (String::from("SEMOXIDE_LOGS"), Some("SEMOXIDE_LOG")),
                (String::from("SEMOXIDE_log"), Some("SEMOXIDE_LOG")),
            ]
        );
    }

    #[test]
    fn only_the_full_prefix_counts_and_an_empty_suffix_is_unknown() {
        let env = env(
            Case::Sensitive,
            &[("SEMOXIDEX", "1"), ("SEMOXIDE", "1"), ("SEMOXIDE_", "1")],
        );

        assert_eq!(unknown(&env), [(String::from("SEMOXIDE_"), None)]);
    }

    #[test]
    fn names_that_arent_utf_8_are_skipped() {
        let mut name = OsString::from("SEMOXIDE_");
        name.push(not_unicode());
        let env = Env::with_case(
            Case::Sensitive,
            [
                (name, OsString::from("1")),
                (OsString::from("SEMOXIDE_LGO"), OsString::from("1")),
            ],
        );

        assert_eq!(
            unknown(&env),
            [(String::from("SEMOXIDE_LGO"), Some("SEMOXIDE_LOG"))]
        );
    }

    #[test]
    fn case_insensitive_names_are_reported_in_byte_order_each_as_set() {
        let env = env(
            Case::Insensitive,
            &[
                ("SEMOXIDE_a", "1"),
                ("SEMOXIDE_B", "1"),
                ("Semoxide_Lgo", "1"),
                ("SEMOXIDE_LGO", "1"),
            ],
        );

        assert_eq!(
            unknown(&env),
            [
                (String::from("SEMOXIDE_B"), None),
                (String::from("SEMOXIDE_LGO"), Some("SEMOXIDE_LOG")),
                (String::from("SEMOXIDE_a"), None),
                (String::from("Semoxide_Lgo"), Some("SEMOXIDE_LOG")),
            ]
        );
    }

    #[test]
    fn known_vars_match_cli_md() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CLI.md");
        let text = std::fs::read_to_string(path).unwrap();
        let section = text
            .split("## Environment variables")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .unwrap();
        let documented: Vec<&str> = section
            .lines()
            .filter_map(|line| {
                let cell = line.split('|').nth(1)?.trim();
                cell.strip_prefix('`')?.strip_suffix('`')
            })
            .filter(|name| name.starts_with("SEMOXIDE_"))
            .collect();

        assert_eq!(KNOWN_VARS.as_slice(), documented.as_slice());
    }
}
