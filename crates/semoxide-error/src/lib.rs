//! `ErrorInfo` trait, the namespaced error-code type and error locations shared by every semoxide crate.

#![forbid(unsafe_code)]

mod code;
mod info;
mod location;

pub use code::{ErrorCode, InvalidErrorCode};
pub use info::{DOCS_BASE_URL, ErrorInfo};
pub use location::{FileLocation, FlagLocation, Location};
