//! [`CommitId`]: a commit's ID, without any git library.

use std::fmt;

/// A commit's ID: its 40-character lowercase hex SHA.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitId(String);

impl CommitId {
    /// The hex SHA.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for CommitId {
    type Error = CommitIdError;

    fn try_from(sha: &str) -> Result<Self, Self::Error> {
        let is_hex = |byte: u8| matches!(byte, b'0'..=b'9' | b'a'..=b'f');
        if sha.len() != 40 || !sha.bytes().all(is_hex) {
            return Err(CommitIdError {
                input: sha.to_owned(),
            });
        }
        Ok(Self(sha.to_owned()))
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A string that isn't a [`CommitId`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CommitIdError {
    input: String,
}

impl fmt::Display for CommitIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a commit ID: expected 40 lowercase hex digits",
            self.input
        )
    }
}

impl std::error::Error for CommitIdError {}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

    #[test]
    fn a_sha_is_kept_as_given() {
        let id = CommitId::try_from(SHA);

        assert_eq!(
            id.map(|id| (id.as_str().to_owned(), id.to_string())),
            Ok((SHA.to_owned(), SHA.to_owned()))
        );
    }

    #[rstest]
    #[case::uppercase("4B825DC642CB6EB9A060E54BF8D69288FBEE4904")]
    #[case::too_short("4b825dc642cb6eb9a060e54bf8d69288fbee490")]
    #[case::too_long("4b825dc642cb6eb9a060e54bf8d69288fbee49040")]
    #[case::not_hex("4b825dc642cb6eb9a060e54bf8d69288fbee490g")]
    #[case::sha256("6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321")]
    #[case::empty("")]
    fn anything_else_is_rejected(#[case] input: &str) {
        assert_eq!(
            CommitId::try_from(input).map_err(|error| error.to_string()),
            Err(format!(
                "`{input}` is not a commit ID: expected 40 lowercase hex digits"
            ))
        );
    }
}
