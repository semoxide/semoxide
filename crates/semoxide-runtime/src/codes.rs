//! Error codes of this crate, and the registry of every code in the workspace.

use semoxide_error::ErrorCode;

/// A `--set` flag that can't be read as `<key>=<value>`.
pub const CONFIG_INVALID_FLAG: ErrorCode = ErrorCode::from_static("config::invalid_flag");

/// A config file that isn't valid TOML.
pub const CONFIG_INVALID_TOML: ErrorCode = ErrorCode::from_static("config::invalid_toml");
/// A config file that exists but can't be read.
pub const CONFIG_UNREADABLE: ErrorCode = ErrorCode::from_static("config::unreadable");

/// Every error code this crate can produce.
pub const ALL: &[ErrorCode] = &[CONFIG_INVALID_FLAG, CONFIG_INVALID_TOML, CONFIG_UNREADABLE];

/// Every error code of semoxide's crates, for the docs-page check and `semoxide schema`.
#[must_use]
pub fn all() -> Vec<ErrorCode> {
    [
        semoxide_schema::codes::ALL,
        semoxide_version_engine::codes::ALL,
        semoxide_git::codes::ALL,
        ALL,
    ]
    .concat()
}
