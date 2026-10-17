//! [`Version`]: a Semantic Versioning 2.0.0 version without build metadata, ordered by precedence.

use std::fmt;
use std::str::FromStr;

/// A Semantic Versioning 2.0.0 version: core and pre-release, never build metadata (which only
/// a git tag carries). Equality, ordering and hashing follow its precedence rules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(semver::Version);

impl Version {
    /// A version from its parts, without build metadata.
    pub(crate) fn from_parts(major: u64, minor: u64, patch: u64, pre: semver::Prerelease) -> Self {
        let mut version = semver::Version::new(major, minor, patch);
        version.pre = pre;
        Self(version)
    }

    pub(crate) fn as_semver(&self) -> &semver::Version {
        &self.0
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let version = semver::Version::parse(input).map_err(|error| VersionError {
            kind: Kind::Invalid {
                input: input.to_owned(),
                reason: error.to_string(),
            },
        })?;
        Self::try_from(version)
    }
}

impl TryFrom<semver::Version> for Version {
    type Error = VersionError;

    fn try_from(version: semver::Version) -> Result<Self, Self::Error> {
        if !version.build.is_empty() {
            return Err(VersionError {
                kind: Kind::BuildMetadata {
                    input: version.to_string(),
                },
            });
        }
        Ok(Self(version))
    }
}

/// A string or [`semver::Version`] that isn't a [`Version`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VersionError {
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// Not Semantic Versioning 2.0.0; `reason` is the `semver` crate's.
    Invalid {
        input: String,
        reason: String,
    },
    BuildMetadata {
        input: String,
    },
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Kind::Invalid { input, reason } => {
                write!(f, "`{input}` is not a SemVer version: {reason}")
            }
            Kind::BuildMetadata { input } => write!(
                f,
                "`{input}` has build metadata; a version doesn't carry it"
            ),
        }
    }
}

impl std::error::Error for VersionError {}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// What parsing gives: the version as displayed, or the kind of error.
    fn parse(input: &str) -> Result<String, &'static str> {
        input
            .parse::<Version>()
            .map(|version| version.to_string())
            .map_err(|error| match error.kind {
                Kind::Invalid { .. } => "invalid",
                Kind::BuildMetadata { .. } => "build metadata",
            })
    }

    fn version(input: &str) -> Version {
        input.parse().unwrap()
    }

    #[rstest]
    #[case::plain("1.2.3")]
    #[case::digit_then_letter("1.0.0-0a")]
    #[case::hyphen_only("1.0.0--")]
    #[case::hyphens("1.0.0-x-y-z.--")]
    #[case::numeric_identifiers("1.0.0-0.3.7")]
    #[case::largest_major("18446744073709551615.0.0")]
    #[case::numeric_identifier_beyond_u64("1.0.0-99999999999999999999999")]
    fn a_valid_version_displays_as_given(#[case] input: &str) {
        assert_eq!(parse(input), Ok(input.to_owned()));
    }

    /// Upstream rejects versions over 256 characters; the specification sets no limit.
    #[test]
    fn a_long_version_is_valid() {
        let input = format!("1.0.0-{}", "a".repeat(294));

        assert_eq!((input.len(), parse(&input)), (300, Ok(input.clone())));
    }

    #[rstest]
    #[case::v_prefix("v1.2.3")]
    #[case::equals_prefix("=1.2.3")]
    #[case::leading_space(" 1.2.3")]
    #[case::trailing_space("1.2.3 ")]
    #[case::leading_zero("01.2.3")]
    #[case::numeric_identifier_leading_zero("1.2.3-01")]
    #[case::empty_pre_release("1.2.3-")]
    #[case::empty_build("1.2.3+")]
    #[case::empty_identifier("1.2.3-a..b")]
    #[case::not_ascii("1.2.3-α")]
    #[case::underscore("1.2.3-a_b")]
    #[case::two_parts("1.2")]
    #[case::four_parts("1.2.3.4")]
    #[case::major_beyond_u64("18446744073709551616.0.0")]
    #[case::empty("")]
    fn an_invalid_version_is_rejected(#[case] input: &str) {
        assert_eq!(parse(input), Err("invalid"));
    }

    #[rstest]
    #[case::alphanumeric("1.2.3+meta")]
    #[case::leading_zeros("1.2.3+001")]
    #[case::with_pre_release("1.0.0-rc.1+b")]
    fn build_metadata_is_rejected(#[case] input: &str) {
        assert_eq!(parse(input), Err("build metadata"));
    }

    #[test]
    fn a_semver_version_with_build_metadata_is_rejected() {
        let with_build = semver::Version::parse("1.2.3+meta").unwrap();

        assert_eq!(
            Version::try_from(with_build).map_err(|error| error.to_string()),
            Err(String::from(
                "`1.2.3+meta` has build metadata; a version doesn't carry it"
            ))
        );
    }

    #[test]
    fn errors_name_the_input_and_the_reason() {
        let message = |input: &str| input.parse::<Version>().map_err(|error| error.to_string());

        assert_eq!(
            (message("v1.2.3"), message(" 1.2.3"), message("1.2.3+meta")),
            (
                Err(String::from(
                    "`v1.2.3` is not a SemVer version: unexpected character 'v' while parsing \
                     major version number"
                )),
                Err(String::from(
                    "` 1.2.3` is not a SemVer version: unexpected character ' ' while parsing \
                     major version number"
                )),
                Err(String::from(
                    "`1.2.3+meta` has build metadata; a version doesn't carry it"
                ))
            )
        );
    }

    #[test]
    fn parsing_and_converting_give_the_same_version() {
        let converted = Version::try_from(semver::Version::new(1, 2, 3));

        assert_eq!(converted, Ok(version("1.2.3")));
    }

    /// The specification's example (§11).
    #[test]
    fn the_canonical_chain_is_ascending() {
        let chain = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ]
        .map(version);

        assert!(
            chain.is_sorted_by(|lower, higher| lower < higher),
            "{chain:?}"
        );
    }

    #[rstest]
    #[case::numeric_core("1.9.0", "1.10.0")]
    #[case::numeric_before_alphanumeric("1.0.0-1", "1.0.0-a")]
    #[case::uppercase_before_lowercase("1.0.0-RC.1", "1.0.0-rc.1")]
    #[case::alphanumeric_by_bytes("1.0.0-pre12", "1.0.0-pre8")]
    #[case::z_before_a("1.0.0-Z", "1.0.0-a")]
    #[case::fewer_identifiers_first("1.0.0-0", "1.0.0-0.0")]
    #[case::numeric_identifier_first("1.0.0-a.1", "1.0.0-a.b")]
    #[case::numeric_beyond_u64("1.0.0-99999999999999999999999", "1.0.0-100000000000000000000000")]
    #[case::numeric_beyond_float_precision("1.0.0-9007199254740992", "1.0.0-9007199254740993")]
    #[case::numeric_beyond_u128(
        "1.0.0-340282366920938463463374607431768211456",
        "1.0.0-340282366920938463463374607431768211457"
    )]
    #[case::large_numeric_before_alphanumeric("1.0.0-9007199254740993", "1.0.0-a")]
    fn precedence(#[case] lower: &str, #[case] higher: &str) {
        let (lower, higher) = (version(lower), version(higher));

        assert_eq!(
            (lower < higher, lower == higher),
            (true, false),
            "{lower} < {higher}"
        );
    }
}
