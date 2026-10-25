//! Bumps: the next version at a level, the next prerelease and the first prerelease of a line.

use std::fmt;

use semoxide_schema::config::Level;

use crate::{PrereleaseId, Version};

impl Version {
    /// The version a `level` release after this one. On a prerelease the level may already be
    /// pending in the core (`1.0.0-beta.1` is before major `1.0.0`): then the prerelease is only
    /// dropped.
    ///
    /// # Errors
    ///
    /// Returns [`BumpError`] if the bumped number would exceed `u64::MAX`.
    pub fn bump(&self, level: Level) -> Result<Self, BumpError> {
        let core = self.as_semver();
        let pending = !core.pre.is_empty()
            && match level {
                Level::Major => core.minor == 0 && core.patch == 0,
                Level::Minor => core.patch == 0,
                Level::Patch => true,
            };
        if pending {
            return Ok(Self::from_parts(
                core.major,
                core.minor,
                core.patch,
                semver::Prerelease::EMPTY,
            ));
        }
        let overflow = || BumpError {
            kind: Kind::Overflow {
                version: self.clone(),
                level,
            },
        };
        let (major, minor, patch) = match level {
            Level::Major => (core.major.checked_add(1).ok_or_else(overflow)?, 0, 0),
            Level::Minor => (
                core.major,
                core.minor.checked_add(1).ok_or_else(overflow)?,
                0,
            ),
            Level::Patch => (
                core.major,
                core.minor,
                core.patch.checked_add(1).ok_or_else(overflow)?,
            ),
        };
        Ok(Self::from_parts(
            major,
            minor,
            patch,
            semver::Prerelease::EMPTY,
        ))
    }

    /// The next prerelease of the same line: the rightmost numeric identifier plus one, at any
    /// size, or `.0` appended when there is none (`beta.1.x` → `beta.2.x`, `beta` → `beta.0`).
    ///
    /// # Errors
    ///
    /// Returns [`BumpError`] if this version isn't a prerelease.
    pub fn next_prerelease(&self) -> Result<Self, BumpError> {
        let core = self.as_semver();
        if core.pre.is_empty() {
            return Err(BumpError {
                kind: Kind::NotPrerelease {
                    version: self.clone(),
                },
            });
        }
        let mut ids: Vec<String> = core.pre.split('.').map(str::to_owned).collect();
        match ids.iter_mut().rev().find(|id| is_numeric(id)) {
            Some(id) => *id = increment(id),
            None => ids.push(String::from("0")),
        }
        let ids = ids.join(".");
        let pre = semver::Prerelease::new(&ids).map_err(|_| BumpError {
            kind: Kind::InvalidPrerelease { ids },
        })?;
        Ok(Self::from_parts(core.major, core.minor, core.patch, pre))
    }

    /// The first prerelease of this version's core: `1.2.3` with `beta` is `1.2.3-beta.1`. Any
    /// prerelease this version has is replaced.
    #[must_use]
    pub fn first_prerelease(&self, id: &PrereleaseId) -> Self {
        let core = self.as_semver();
        Self::from_parts(core.major, core.minor, core.patch, id.first().clone())
    }
}

fn is_numeric(id: &str) -> bool {
    id.bytes().all(|byte| byte.is_ascii_digit())
}

/// A decimal number plus one, at any size.
fn increment(digits: &str) -> String {
    let mut bytes = digits.as_bytes().to_vec();
    for byte in bytes.iter_mut().rev() {
        if *byte != b'9' {
            *byte += 1;
            return String::from_utf8_lossy(&bytes).into_owned();
        }
        *byte = b'0';
    }
    format!("1{}", String::from_utf8_lossy(&bytes))
}

/// A bump that has no result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BumpError {
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Overflow {
        version: Version,
        level: Level,
    },
    NotPrerelease {
        version: Version,
    },
    /// Identifiers built from valid ones are always valid; kept as an error, never a guess.
    InvalidPrerelease {
        ids: String,
    },
}

impl fmt::Display for BumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Kind::Overflow { version, level } => {
                let level = match level {
                    Level::Major => "major",
                    Level::Minor => "minor",
                    Level::Patch => "patch",
                };
                write!(
                    f,
                    "`{version}` can't get a {level} bump: the number would exceed {}",
                    u64::MAX
                )
            }
            Kind::NotPrerelease { version } => {
                write!(f, "`{version}` has no prerelease to increment")
            }
            Kind::InvalidPrerelease { ids } => write!(f, "`{ids}` is not a valid prerelease"),
        }
    }
}

impl std::error::Error for BumpError {}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;
    use semoxide_test_support::strategies;

    use super::*;

    fn version(input: &str) -> Version {
        input.parse().unwrap()
    }

    /// `[major, minor, patch]` bumps of `input`, displayed.
    fn bumps(input: &str) -> [Result<String, String>; 3] {
        [Level::Major, Level::Minor, Level::Patch].map(|level| {
            version(input)
                .bump(level)
                .map(|bumped| bumped.to_string())
                .map_err(|error| error.to_string())
        })
    }

    fn ok(versions: [&str; 3]) -> [Result<String, String>; 3] {
        versions.map(|bumped| Ok(bumped.to_owned()))
    }

    #[rstest]
    #[case::zero("0.0.0", ["1.0.0", "0.1.0", "0.0.1"])]
    #[case::zero_minor("0.1.0", ["1.0.0", "0.2.0", "0.1.1"])]
    #[case::first("1.0.0", ["2.0.0", "1.1.0", "1.0.1"])]
    #[case::patch_set("1.2.3", ["2.0.0", "1.3.0", "1.2.4"])]
    #[case::minor_set("1.2.0", ["2.0.0", "1.3.0", "1.2.1"])]
    fn a_release_bumps_its_core(#[case] input: &str, #[case] expected: [&str; 3]) {
        assert_eq!(bumps(input), ok(expected));
    }

    /// A level the core already carries only drops the prerelease.
    #[rstest]
    #[case::major_pending("1.0.0-beta.1", ["1.0.0", "1.0.0", "1.0.0"])]
    #[case::major_pending_numeric("1.0.0-0", ["1.0.0", "1.0.0", "1.0.0"])]
    #[case::minor_pending("1.2.0-beta.1", ["2.0.0", "1.2.0", "1.2.0"])]
    #[case::major_not_pending_with_patch("1.0.3-beta.1", ["2.0.0", "1.1.0", "1.0.3"])]
    #[case::patch_pending("1.2.3-beta.1", ["2.0.0", "1.3.0", "1.2.3"])]
    #[case::without_counter("1.2.3-beta", ["2.0.0", "1.3.0", "1.2.3"])]
    #[case::all_zero("0.0.0-0", ["0.0.0", "0.0.0", "0.0.0"])]
    fn a_prerelease_bumps_to_its_pending_release(#[case] input: &str, #[case] expected: [&str; 3]) {
        assert_eq!(bumps(input), ok(expected));
    }

    #[test]
    fn a_bump_beyond_u64_is_an_error() {
        let overflow = |input: &str, level: Level| {
            version(input)
                .bump(level)
                .map_err(|error| error.to_string())
        };

        assert_eq!(
            [
                overflow("18446744073709551615.0.0", Level::Major),
                overflow("1.18446744073709551615.0", Level::Minor),
                overflow("1.2.18446744073709551615", Level::Patch),
            ],
            [
                Err(String::from(
                    "`18446744073709551615.0.0` can't get a major bump: the number would exceed \
                     18446744073709551615"
                )),
                Err(String::from(
                    "`1.18446744073709551615.0` can't get a minor bump: the number would exceed \
                     18446744073709551615"
                )),
                Err(String::from(
                    "`1.2.18446744073709551615` can't get a patch bump: the number would exceed \
                     18446744073709551615"
                )),
            ]
        );
    }

    /// Only the bumped number overflows: the others may be at `u64::MAX`.
    #[test]
    fn a_bump_below_a_maximal_number_succeeds() {
        assert_eq!(
            [
                version("1.18446744073709551615.0").bump(Level::Major),
                version("1.2.18446744073709551615").bump(Level::Minor),
                version("18446744073709551615.0.0-rc.1").bump(Level::Major),
            ],
            [
                Ok(version("2.0.0")),
                Ok(version("1.3.0")),
                Ok(version("18446744073709551615.0.0")),
            ]
        );
    }

    #[rstest]
    #[case::appends_counter("1.2.3-beta", "1.2.3-beta.0")]
    #[case::counter("1.2.3-beta.1", "1.2.3-beta.2")]
    #[case::carry("1.2.3-beta.9", "1.2.3-beta.10")]
    #[case::carry_twice("1.2.3-beta.99", "1.2.3-beta.100")]
    #[case::rightmost_numeric("1.2.3-beta.1.2", "1.2.3-beta.1.3")]
    #[case::numeric_before_alphanumeric("1.2.3-beta.1.x", "1.2.3-beta.2.x")]
    #[case::zeros("1.2.3-beta.0.0", "1.2.3-beta.0.1")]
    #[case::no_numeric("1.2.3-alpha.beta", "1.2.3-alpha.beta.0")]
    #[case::numeric_only("1.2.3-1", "1.2.3-2")]
    #[case::numeric_zero("1.0.0-0", "1.0.0-1")]
    #[case::beyond_float_precision("1.2.3-beta.9007199254740992", "1.2.3-beta.9007199254740993")]
    #[case::beyond_u64("1.2.3-beta.18446744073709551615", "1.2.3-beta.18446744073709551616")]
    #[case::beyond_u128(
        "1.2.3-beta.340282366920938463463374607431768211455",
        "1.2.3-beta.340282366920938463463374607431768211456"
    )]
    #[case::digits_only_after_letters("1.2.3-beta.0a", "1.2.3-beta.0a.0")]
    fn the_next_prerelease_increments_the_rightmost_number(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(version(input).next_prerelease(), Ok(version(expected)));
    }

    #[test]
    fn a_release_has_no_next_prerelease() {
        assert_eq!(
            version("1.2.3")
                .next_prerelease()
                .map_err(|error| error.to_string()),
            Err(String::from("`1.2.3` has no prerelease to increment"))
        );
    }

    #[rstest]
    #[case::release("1.2.3", "beta", "1.2.3-beta.1")]
    #[case::first("1.0.0", "rc", "1.0.0-rc.1")]
    #[case::replaces_prerelease("2.0.0-alpha.3", "beta", "2.0.0-beta.1")]
    #[case::dotted_identifier("1.0.0", "rc.1", "1.0.0-rc.1.1")]
    fn the_first_prerelease_is_numbered_one(
        #[case] input: &str,
        #[case] id: &str,
        #[case] expected: &str,
    ) {
        let id = PrereleaseId::try_from(id).unwrap();

        assert_eq!(version(input).first_prerelease(&id), version(expected));
    }

    fn level() -> impl Strategy<Value = Level> {
        prop::sample::select(vec![Level::Major, Level::Minor, Level::Patch])
    }

    fn release(major: u64, minor: u64, patch: u64) -> Version {
        Version::from_parts(major, minor, patch, semver::Prerelease::EMPTY)
    }

    /// The least release above `version` whose parts below `level` are zero; `None` past
    /// `u64::MAX`.
    fn least_release_above(version: &Version, level: Level) -> Option<Version> {
        let semver::Version {
            major,
            minor,
            patch,
            pre,
            ..
        } = version.as_semver();
        let pre = !pre.is_empty();
        let candidates = match level {
            Level::Major => [
                (pre && *minor == 0 && *patch == 0).then(|| release(*major, 0, 0)),
                major.checked_add(1).map(|major| release(major, 0, 0)),
            ],
            Level::Minor => [
                (pre && *patch == 0).then(|| release(*major, *minor, 0)),
                minor.checked_add(1).map(|minor| release(*major, minor, 0)),
            ],
            Level::Patch => [
                pre.then(|| release(*major, *minor, *patch)),
                patch
                    .checked_add(1)
                    .map(|patch| release(*major, *minor, patch)),
            ],
        };
        candidates
            .into_iter()
            .flatten()
            .filter(|candidate| candidate > version)
            .min()
    }

    /// `digits` plus one, carried digit by digit.
    fn plus_one(digits: &str) -> String {
        let mut bytes = digits.as_bytes().to_vec();
        for digit in bytes.iter_mut().rev() {
            if *digit == b'9' {
                *digit = b'0';
            } else {
                *digit += 1;
                return String::from_utf8(bytes).unwrap();
            }
        }
        format!("1{}", String::from_utf8(bytes).unwrap())
    }

    proptest! {
        #[test]
        fn a_bump_is_the_least_release_above_at_its_level(
            input in strategies::version(),
            level in level(),
        ) {
            let version: Version = input.parse().unwrap();

            prop_assert_eq!(version.bump(level).ok(), least_release_above(&version, level));
        }

        #[test]
        fn a_higher_version_never_bumps_lower(
            a in strategies::close_version(),
            b in strategies::close_version(),
            level in level(),
        ) {
            let (a, b): (Version, Version) = (a.parse().unwrap(), b.parse().unwrap());
            let (lower, higher) = if a <= b { (a, b) } else { (b, a) };

            prop_assert!(lower.bump(level).unwrap() <= higher.bump(level).unwrap());
        }

        #[test]
        fn a_prerelease_bump_raises_its_rightmost_number(input in strategies::version()) {
            let version: Version = input.parse().unwrap();
            let semver = version.as_semver();
            let next = version.next_prerelease();
            if semver.pre.is_empty() {
                prop_assert!(next.is_err());
                return Ok(());
            }
            let next = next.unwrap();
            let mut ids: Vec<String> = semver.pre.split('.').map(str::to_owned).collect();
            let rightmost = ids
                .iter_mut()
                .rev()
                .find(|id| id.bytes().all(|byte| byte.is_ascii_digit()));
            match rightmost {
                Some(id) => *id = plus_one(id),
                None => ids.push(String::from("0")),
            }
            let bumped = next.as_semver();

            prop_assert_eq!(bumped.pre.as_str(), ids.join("."));
            prop_assert_eq!(
                (bumped.major, bumped.minor, bumped.patch),
                (semver.major, semver.minor, semver.patch)
            );
            prop_assert!(next > version);
            prop_assert!(next < release(semver.major, semver.minor, semver.patch));
        }
    }
}
