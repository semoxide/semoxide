//! Pure version engine: branch model, ranges, last and next version. No I/O.

#![forbid(unsafe_code)]

mod bump;
pub mod codes;
mod prerelease_id;
mod range;
mod tag_format;
mod version;

pub use bump::BumpError;
pub use prerelease_id::{PrereleaseId, PrereleaseIdError};
pub use range::Range;
pub use tag_format::TagPattern;
pub use version::{Version, VersionError};
