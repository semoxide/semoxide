//! Error codes of this crate. `ALL` feeds the error-code registry check and `semoxide schema`.

use semoxide_error::ErrorCode;

/// No git repository at or above the run's directory.
pub const GIT_NO_REPOSITORY: ErrorCode = ErrorCode::from_static("git::no_repository");
/// A repository owned by another user and not listed in `safe.directory`.
pub const GIT_UNSAFE_REPOSITORY: ErrorCode = ErrorCode::from_static("git::unsafe_repository");
/// A bare repository: no working tree to release from.
pub const GIT_BARE_REPOSITORY: ErrorCode = ErrorCode::from_static("git::bare_repository");
/// The configured remote doesn't exist.
pub const GIT_NO_REMOTE: ErrorCode = ErrorCode::from_static("git::no_remote");
/// Any other error libgit2 reports while reading the repository.
pub const GIT_READ_FAILED: ErrorCode = ErrorCode::from_static("git::read_failed");

/// Every error code this crate can produce.
pub const ALL: &[ErrorCode] = &[
    GIT_NO_REPOSITORY,
    GIT_UNSAFE_REPOSITORY,
    GIT_BARE_REPOSITORY,
    GIT_NO_REMOTE,
    GIT_READ_FAILED,
];
