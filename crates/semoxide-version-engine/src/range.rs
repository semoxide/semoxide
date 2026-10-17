//! [`Range`]: the versions a branch may release, `>=min <max`.

use crate::Version;

/// The versions from `min` up to, not including, `max_exclusive`; no `max_exclusive` means no
/// upper bound. A release as `max_exclusive` also keeps out its own prereleases: `<2.0.0` ends
/// below `2.0.0-0`, so a 1.x range never takes a 2.0.0 beta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    min: Version,
    max_exclusive: Option<Version>,
}

impl Range {
    /// The range `>=min <max_exclusive`, or `>=min` without `max_exclusive`. It is empty when
    /// `max_exclusive` isn't above `min`.
    #[must_use]
    pub fn new(min: Version, max_exclusive: Option<Version>) -> Self {
        Self { min, max_exclusive }
    }

    /// The lowest version in the range.
    #[must_use]
    pub fn min(&self) -> &Version {
        &self.min
    }

    /// The first version above the range, if it has an upper bound.
    #[must_use]
    pub fn max_exclusive(&self) -> Option<&Version> {
        self.max_exclusive.as_ref()
    }

    /// Whether `version` is in the range.
    #[must_use]
    pub fn contains(&self, _version: &Version) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(input: &str) -> Version {
        input.parse().unwrap()
    }

    fn range(min: &str, max_exclusive: Option<&str>) -> Range {
        Range::new(version(min), max_exclusive.map(version))
    }

    /// Which of `versions` the range contains.
    fn contained<'a, const N: usize>(
        range: &Range,
        versions: [&'a str; N],
    ) -> [(&'a str, bool); N] {
        versions.map(|input| (input, range.contains(&version(input))))
    }

    #[test]
    fn a_release_bound_keeps_its_prereleases_out() {
        let range = range("1.0.0", Some("2.0.0"));

        assert_eq!(
            contained(
                &range,
                [
                    "0.9.0",
                    "1.0.0-beta.1",
                    "1.0.0",
                    "1.5.0-beta.1",
                    "1.5.0",
                    "1.9.9",
                    "2.0.0-0",
                    "2.0.0-beta.1",
                    "2.0.0",
                    "2.0.1",
                ]
            ),
            [
                ("0.9.0", false),
                ("1.0.0-beta.1", false),
                ("1.0.0", true),
                ("1.5.0-beta.1", true),
                ("1.5.0", true),
                ("1.9.9", true),
                ("2.0.0-0", false),
                ("2.0.0-beta.1", false),
                ("2.0.0", false),
                ("2.0.1", false),
            ]
        );
    }

    #[test]
    fn a_prerelease_bound_is_compared_by_precedence() {
        let range = range("1.5.0", Some("2.0.0-beta.1"));

        assert_eq!(
            contained(
                &range,
                ["1.5.0", "2.0.0-0", "2.0.0-alpha.1", "2.0.0-beta.1", "2.0.0"]
            ),
            [
                ("1.5.0", true),
                ("2.0.0-0", true),
                ("2.0.0-alpha.1", true),
                ("2.0.0-beta.1", false),
                ("2.0.0", false),
            ]
        );
    }

    #[test]
    fn a_prerelease_min_is_compared_by_precedence() {
        let range = range("1.0.0-beta.1", Some("2.0.0"));

        assert_eq!(
            contained(
                &range,
                ["1.0.0-alpha", "1.0.0-beta.1", "1.0.0-beta.2", "1.0.0"]
            ),
            [
                ("1.0.0-alpha", false),
                ("1.0.0-beta.1", true),
                ("1.0.0-beta.2", true),
                ("1.0.0", true),
            ]
        );
    }

    #[test]
    fn without_a_max_there_is_no_upper_bound() {
        let range = range("1.5.0", None);

        assert_eq!(
            contained(&range, ["1.4.9", "1.5.0", "3.0.0-beta.1", "99.0.0"]),
            [
                ("1.4.9", false),
                ("1.5.0", true),
                ("3.0.0-beta.1", true),
                ("99.0.0", true),
            ]
        );
    }

    #[test]
    fn an_empty_range_contains_nothing() {
        let empty = range("1.0.0", Some("1.0.0"));
        let reversed = range("2.0.0", Some("1.0.0"));
        let below_min = range("1.0.0", Some("1.0.0-beta.1"));
        let all = ["0.9.0", "1.0.0-beta.1", "1.0.0", "1.5.0", "2.0.0"];

        assert_eq!(
            [&empty, &reversed, &below_min].map(|range| contained(range, all)),
            [all.map(|input| (input, false)); 3]
        );
    }

    #[test]
    fn the_bounds_are_kept_as_given() {
        let range = range("1.0.0", Some("2.0.0"));

        assert_eq!(
            (range.min(), range.max_exclusive()),
            (&version("1.0.0"), Some(&version("2.0.0")))
        );
    }
}
