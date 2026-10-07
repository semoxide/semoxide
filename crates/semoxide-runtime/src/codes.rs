//! Error codes of this crate, and the registry of every code in the workspace.

use semoxide_error::ErrorCode;

/// Every error code this crate can produce.
pub const ALL: &[ErrorCode] = &[];

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
