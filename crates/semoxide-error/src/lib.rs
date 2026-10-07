//! `ErrorInfo` trait and the namespaced error-code type shared by every semoxide crate.

#![forbid(unsafe_code)]

mod code;

pub use code::{ErrorCode, InvalidErrorCode};
