//! [`ConfigError`]: an invalid configuration with its code and key path.

use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};
use toml::Value;

use super::{PluginName, key, suggest};
use crate::codes::{
    CONFIG_CONFLICTING_KEYS, CONFIG_INVALID_VALUE, CONFIG_PLUGIN_NOT_ENABLED, CONFIG_UNKNOWN_KEY,
    CONFIG_UNSUPPORTED_SECTION,
};

/// An invalid configuration: its code, the key path (`branches.rules[2].prerelease`) and what
/// is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    code: ErrorCode,
    path: String,
    message: String,
    help: Option<String>,
}

impl ConfigError {
    /// The key path, e.g. `tags.format`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    pub(super) fn invalid(path: &str, value: &Value, problem: &str) -> Self {
        Self {
            code: CONFIG_INVALID_VALUE,
            path: path.to_owned(),
            message: format!("`{path}` = {value}: {problem}"),
            help: None,
        }
    }

    /// An unknown key, with a hint when a valid one in `known` is close.
    pub(super) fn unknown(path: &str, known: &[&str]) -> Self {
        let (parent, name) = path.rsplit_once('.').unwrap_or(("", path));
        let help = suggest::closest(name, known)
            .map(|candidate| format!("Use `{}` instead.", key(parent, candidate)));
        Self {
            code: CONFIG_UNKNOWN_KEY,
            path: path.to_owned(),
            message: format!("unknown key `{path}`"),
            help,
        }
    }

    pub(super) fn conflicting(path: &str, problem: &str) -> Self {
        Self {
            code: CONFIG_CONFLICTING_KEYS,
            path: path.to_owned(),
            message: format!("`{path}`: {problem}"),
            help: None,
        }
    }

    pub(super) fn unsupported(path: &str) -> Self {
        Self {
            code: CONFIG_UNSUPPORTED_SECTION,
            path: path.to_owned(),
            message: format!("`{path}` isn't supported yet"),
            help: None,
        }
    }

    pub(super) fn not_enabled(path: &str, name: &PluginName) -> Self {
        Self {
            code: CONFIG_PLUGIN_NOT_ENABLED,
            path: path.to_owned(),
            message: format!("`{path}`: plugin `{name}` isn't in `steps.plugins`"),
            help: None,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

impl ErrorInfo for ConfigError {
    fn code(&self) -> ErrorCode {
        self.code.clone()
    }

    fn help(&self) -> Option<String> {
        self.help.clone()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::config::Config;
    use crate::config::test_support::parse;

    #[test]
    fn error_message_names_the_key_and_the_bad_value() {
        let error = parse(r#"tags.format = "release-tag""#);

        assert!(error.is_err());
        let message = error.unwrap_err().to_string();
        assert!(message.contains("`tags.format`"), "{message}");
        assert!(message.contains("release-tag"), "{message}");
    }

    /// The code and help of the error the config fails with.
    fn unknown_key_help(text: &str) -> Option<(String, Option<String>)> {
        let table = text.parse::<toml::Table>().unwrap();
        Config::from_table(table)
            .err()
            .map(|error| (error.code().to_string(), error.help()))
    }

    #[rstest]
    #[case::domain("[tgas]\nformat = \"v{version}\"", Some("Use `tags` instead."))]
    #[case::config_key(r#"config.marge = "deep""#, Some("Use `config.merge` instead."))]
    #[case::commits_key(r#"commits.prest = "angular""#, Some("Use `commits.preset` instead."))]
    #[case::version_key(r#"version.inittal = "1.0.0""#, Some("Use `version.initial` instead."))]
    #[case::zero_key(
        r#"version.zero.brekaing = "major""#,
        Some("Use `version.zero.breaking` instead.")
    )]
    #[case::tags_key(r#"tags.formta = "v{version}""#, Some("Use `tags.format` instead."))]
    #[case::release_rule_key(
        r#"branches.rules = [{ name = "main", chnanel = "stable" }]"#,
        Some("Use `branches.rules[0].channel` instead.")
    )]
    #[case::maintenance_rule_key(
        r#"branches.rules = [{ maintenance = "legacy", range = "1.x", chnanel = "old" }]"#,
        Some("Use `branches.rules[0].channel` instead.")
    )]
    #[case::step_name(
        r#"steps.publihs.order = ["release-notes"]"#,
        Some("Use `steps.publish` instead.")
    )]
    #[case::step_key(
        r#"steps.publish.orderr = ["release-notes"]"#,
        Some("Use `steps.publish.order` instead.")
    )]
    #[case::timeout_step(
        "[plugins.release-notes]\ntimeouts.publihs = \"1h\"",
        Some("Use `plugins.release-notes.timeouts.publish` instead.")
    )]
    #[case::secrets_key("secrets.mask_envs = []", Some("Use `secrets.mask_env` instead."))]
    #[case::nothing_close_key(r#"tags.prefix = "v{version}""#, None)]
    #[case::nothing_close_domain("[colour]\nname = \"red\"", None)]
    fn unknown_key_suggests_the_closest_valid_key(#[case] text: &str, #[case] help: Option<&str>) {
        assert_eq!(
            unknown_key_help(text),
            Some((CONFIG_UNKNOWN_KEY.to_string(), help.map(str::to_owned)))
        );
    }

    #[test]
    fn the_message_still_names_the_unknown_key() {
        let table = r#"tags.formta = "v{version}""#.parse::<toml::Table>().unwrap();

        let error = Config::from_table(table);

        assert!(error.is_err());
        assert_eq!(error.unwrap_err().to_string(), "unknown key `tags.formta`");
    }
}
