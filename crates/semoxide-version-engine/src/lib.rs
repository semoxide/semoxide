//! Pure version engine: branch model, ranges, last and next version. No I/O.

#![forbid(unsafe_code)]

mod branch;
mod bump;
pub mod codes;
mod commit_id;
mod prerelease_id;
mod range;
mod release;
mod tag_format;
mod version;

pub use branch::{
    BranchError, BranchModel, Channel, Current, MaintenanceBranch, MaintenanceLine, Overlap,
    PrereleaseBranch, ReleaseBranch, ReleaseRanges,
};
pub use bump::BumpError;
pub use commit_id::{CommitId, CommitIdError};
pub use prerelease_id::{PrereleaseId, PrereleaseIdError};
pub use range::Range;
pub use release::{FirstReleaseWarning, LastRelease, NextRelease, ReleaseError, Releases};
pub use tag_format::TagPattern;
pub use version::{Version, VersionError};
