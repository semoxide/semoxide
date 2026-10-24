//! Object IDs as semoxide's own types, so git2 stays inside this crate.

use std::fmt;

pub use semoxide_version_engine::CommitId;

/// Any object's ID, e.g. an annotated tag's own object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectId(String);

/// The commit ID of `oid`.
pub(crate) fn commit_id(oid: git2::Oid) -> Result<CommitId, git2::Error> {
    CommitId::try_from(oid.to_string().as_str())
        .map_err(|error| git2::Error::from_str(&error.to_string()))
}

pub(crate) fn to_oid(commit: &CommitId) -> Result<git2::Oid, git2::Error> {
    git2::Oid::from_str(commit.as_str())
}

impl ObjectId {
    /// The hex SHA.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn to_oid(&self) -> Result<git2::Oid, git2::Error> {
        git2::Oid::from_str(&self.0)
    }
}

impl From<git2::Oid> for ObjectId {
    fn from(oid: git2::Oid) -> Self {
        Self(oid.to_string())
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

    #[test]
    fn ids_are_the_lowercase_hex_sha() {
        let oid = git2::Oid::from_str(&SHA.to_uppercase()).unwrap();

        let (commit, object) = (commit_id(oid).unwrap(), ObjectId::from(oid));

        assert_eq!(
            [
                commit.as_str(),
                &commit.to_string(),
                object.as_str(),
                &object.to_string()
            ],
            [SHA; 4]
        );
    }
}
