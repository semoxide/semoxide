//! Orchestrator, config loading, CI context, plugin host integration and observability for semoxide.

#![forbid(unsafe_code)]

pub mod codes;
pub mod config;
mod env;

pub use env::{Env, EnvError, UnknownVar};
pub use semoxide_error::{ErrorCode, ErrorInfo, FileLocation, FlagLocation, Location};
