//! Orchestrator, config loading, CI context, plugin host integration and observability for semoxide.

#![forbid(unsafe_code)]

pub mod codes;
pub mod config;

pub use semoxide_error::{ErrorCode, ErrorInfo, FileLocation, FlagLocation, Location};
