//! `ErrorInfo` trait and the namespaced error-code type shared by every semoxide crate.

#![forbid(unsafe_code)]

mod code;
mod info;

pub use code::{ErrorCode, InvalidErrorCode};
pub use info::{DOCS_BASE_URL, ErrorInfo};
