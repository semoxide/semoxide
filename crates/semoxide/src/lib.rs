//! Library API of semoxide, a release tool: builder, run, read-only queries, `RunReport`.

#![forbid(unsafe_code)]
// Public types of the stable API are `#[non_exhaustive]` (CODE-ARCHITECTURE P14).
#![warn(clippy::exhaustive_enums, clippy::exhaustive_structs)]

/// The semoxide version, shared by the library and the CLI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use semoxide_runtime::{ErrorCode, ErrorInfo, FileLocation, FlagLocation, Location};

/// Every error code semoxide can report; each has a docs page at [`ErrorInfo::url`].
#[must_use]
pub fn error_codes() -> Vec<ErrorCode> {
    semoxide_runtime::codes::all()
}
