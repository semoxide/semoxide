//! Git access for semoxide via git2: reads, guarded pushes, SSH transports, credentials.

pub mod codes;
mod error;
mod id;
mod repo;
mod tags;

pub use error::GitError;
pub use id::{CommitId, ObjectId};
pub use repo::Repo;
pub use tags::Tag;
