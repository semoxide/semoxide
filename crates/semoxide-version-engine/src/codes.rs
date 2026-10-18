//! Error codes of this crate. `ALL` feeds the error-code registry check and `semoxide schema`.

use semoxide_error::ErrorCode;

/// Not 1 to 3 release branches among the existing branches.
pub const BRANCH_RELEASE_BRANCHES: ErrorCode = ErrorCode::from_static("branch::release_branches");
/// A prerelease branch whose identifier, from its name or a `{name}` template, is invalid.
pub const BRANCH_INVALID_PRERELEASE_ID: ErrorCode =
    ErrorCode::from_static("branch::invalid_prerelease_id");
/// Two prerelease branches with the same identifier.
pub const BRANCH_DUPLICATE_PRERELEASE_IDS: ErrorCode =
    ErrorCode::from_static("branch::duplicate_prerelease_ids");
/// Two maintenance branches with the same range.
pub const BRANCH_DUPLICATE_MAINTENANCE_RANGES: ErrorCode =
    ErrorCode::from_static("branch::duplicate_maintenance_ranges");
/// A release from a prerelease or maintenance branch, which this version can't make yet.
pub const BRANCH_NOT_YET_SUPPORTED: ErrorCode = ErrorCode::from_static("branch::not_yet_supported");

/// Every error code this crate can produce.
pub const ALL: &[ErrorCode] = &[
    BRANCH_RELEASE_BRANCHES,
    BRANCH_INVALID_PRERELEASE_ID,
    BRANCH_DUPLICATE_PRERELEASE_IDS,
    BRANCH_DUPLICATE_MAINTENANCE_RANGES,
    BRANCH_NOT_YET_SUPPORTED,
];
