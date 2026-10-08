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
