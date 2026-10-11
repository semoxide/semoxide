//! Git access for semoxide via git2: reads, guarded pushes, SSH transports, credentials.

pub mod codes;
mod diff;
mod error;
mod id;
mod log;
mod repo;
mod tags;

pub use diff::RepoPath;
pub use error::GitError;
pub use id::{CommitId, ObjectId};
pub use log::{Commit, Signature, Time};
pub use repo::Repo;
pub use tags::Tag;
