//! Error codes of this crate, and the registry of every code in the workspace.

use semoxide_error::ErrorCode;

/// A `--set` flag that can't be read as `<key>=<value>`.
pub const CONFIG_INVALID_FLAG: ErrorCode = ErrorCode::from_static("config::invalid_flag");

/// Every error code this crate can produce.
pub const ALL: &[ErrorCode] = &[CONFIG_INVALID_FLAG];

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
