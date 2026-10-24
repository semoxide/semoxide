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
/// Tags of the last release's version on different commits.
pub const VERSION_DUPLICATE_TAGS: ErrorCode = ErrorCode::from_static("version::duplicate_tags");
/// A next version outside the range of the branch being released.
pub const VERSION_OUT_OF_RANGE: ErrorCode = ErrorCode::from_static("version::out_of_range");
/// A next version whose number would exceed `u64::MAX`.
pub const VERSION_OVERFLOW: ErrorCode = ErrorCode::from_static("version::overflow");

/// Every error code this crate can produce.
pub const ALL: &[ErrorCode] = &[
    BRANCH_RELEASE_BRANCHES,
    BRANCH_INVALID_PRERELEASE_ID,
    BRANCH_DUPLICATE_PRERELEASE_IDS,
    BRANCH_DUPLICATE_MAINTENANCE_RANGES,
    BRANCH_NOT_YET_SUPPORTED,
    VERSION_DUPLICATE_TAGS,
    VERSION_OUT_OF_RANGE,
    VERSION_OVERFLOW,
];
