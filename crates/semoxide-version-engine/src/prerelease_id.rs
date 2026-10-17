//! [`PrereleaseId`]: the identifier a prerelease branch puts into its versions.

use std::fmt;

use crate::Version;

/// A prerelease identifier such as `beta` or `rc.1`: valid when `1.0.0-<id>.1` is a
/// [`Version`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrereleaseId {
    id: String,
    /// `<id>.1`, the prerelease of a line's first version.
    first: semver::Prerelease,
}

impl PrereleaseId {
    /// The identifier as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.id
    }

    pub(crate) fn first(&self) -> &semver::Prerelease {
        &self.first
    }
}

impl TryFrom<&str> for PrereleaseId {
    type Error = PrereleaseIdError;

    fn try_from(id: &str) -> Result<Self, Self::Error> {
        let version: Version = format!("1.0.0-{id}.1")
            .parse()
            .map_err(|_| PrereleaseIdError {
                input: id.to_owned(),
            })?;
        Ok(Self {
            id: id.to_owned(),
            first: version.as_semver().pre.clone(),
        })
    }
}

impl fmt::Display for PrereleaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

/// A string that isn't a [`PrereleaseId`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrereleaseIdError {
    input: String,
}

impl fmt::Display for PrereleaseIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a prerelease identifier (letters, digits, `-` and `.`, no leading zeros)",
            self.input
        )
    }
}

impl std::error::Error for PrereleaseIdError {}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::word("beta")]
    #[case::dotted("rc.1")]
    #[case::digit_then_letter("0a")]
    #[case::hyphen("-")]
    #[case::uppercase("RC")]
    fn a_valid_identifier_displays_as_given(#[case] id: &str) {
        assert_eq!(
            PrereleaseId::try_from(id).map(|id| id.to_string()),
            Ok(id.to_owned())
        );
    }

    #[rstest]
    #[case::empty("")]
    #[case::leading_zero("01")]
    #[case::empty_part("a..b")]
    #[case::build_metadata("beta+x")]
    #[case::not_ascii("α")]
    #[case::underscore("a_b")]
    #[case::surrounding_space(" beta ")]
    fn an_invalid_identifier_is_rejected(#[case] id: &str) {
        assert_eq!(
            PrereleaseId::try_from(id).map_err(|error| error.input),
            Err(id.to_owned())
        );
    }

    #[test]
    fn the_error_names_the_identifier_and_the_rule() {
        assert_eq!(
            PrereleaseId::try_from("01").map_err(|error| error.to_string()),
            Err(String::from(
                "`01` is not a prerelease identifier (letters, digits, `-` and `.`, no leading \
                 zeros)"
            ))
        );
    }
}
