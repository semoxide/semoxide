//! Git access for semoxide via git2: reads, guarded pushes, SSH transports, credentials.

pub mod codes;
mod error;
mod repo;

pub use error::GitError;
pub use repo::Repo;
