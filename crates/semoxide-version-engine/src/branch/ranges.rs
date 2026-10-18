//! [`ReleaseRanges`]: the versions each release branch may release, and the current branch.

use super::{BranchError, BranchModel, ReleaseBranch};
use crate::Range;

/// The range of each release branch of a [`BranchModel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRanges<'a> {
    model: &'a BranchModel,
    /// One per release branch, in order.
    ranges: Vec<Range>,
}

/// What the current branch is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Current<'a> {
    /// A release branch and its range.
    Release {
        /// The branch.
        branch: &'a ReleaseBranch,
        /// The versions it may release.
        range: &'a Range,
    },
    /// No rule claims the branch.
    NotConfigured,
}

impl<'a> ReleaseRanges<'a> {
    pub(super) fn new(model: &'a BranchModel, ranges: Vec<Range>) -> Self {
        Self { model, ranges }
    }

    /// The range of a release branch; `None` for any other name.
    #[must_use]
    pub fn range(&self, branch: &str) -> Option<&Range> {
        let index = self
            .model
            .release()
            .iter()
            .position(|release| release.name() == branch)?;
        self.ranges.get(index)
    }

    /// What `branch`, the branch being released, is.
    ///
    /// # Errors
    ///
    /// Returns [`BranchError`] for a prerelease or maintenance branch, which can't release yet.
    pub fn current(&self, _branch: &str) -> Result<Current<'_>, BranchError> {
        Ok(Current::NotConfigured)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rstest::rstest;

    use super::super::test_support::model;
    use super::*;
    use crate::Version;

    fn version(input: &str) -> Version {
        input.parse().unwrap()
    }

    fn describe(range: &Range) -> String {
        match range.max_exclusive() {
            Some(max) => format!(">={} <{max}", range.min()),
            None => format!(">={}", range.min()),
        }
    }

    /// Each existing release branch of the default rules with its range, from the versions on
    /// each branch.
    fn ranges_with(branches: &[(&str, &[&str])], initial: &str) -> Vec<(String, Option<String>)> {
        let names: Vec<&str> = branches.iter().map(|(name, _)| *name).collect();
        let versions: HashMap<String, Vec<Version>> = branches
            .iter()
            .map(|(name, versions)| {
                (
                    (*name).to_owned(),
                    versions.iter().map(|input| version(input)).collect(),
                )
            })
            .collect();
        let model = model("", &names).unwrap();
        let ranges = model.ranges(&versions, &version(initial));
        model
            .release()
            .iter()
            .map(|branch| {
                (
                    branch.name().to_owned(),
                    ranges.range(branch.name()).map(describe),
                )
            })
            .collect()
    }

    fn ranges(branches: &[(&str, &[&str])]) -> Vec<(String, Option<String>)> {
        ranges_with(branches, "1.0.0")
    }

    fn expected(ranges: &[(&str, &str)]) -> Vec<(String, Option<String>)> {
        ranges
            .iter()
            .map(|(name, range)| ((*name).to_owned(), Some((*range).to_owned())))
            .collect()
    }

    #[test]
    fn an_untagged_main_line_starts_at_the_initial_version() {
        assert_eq!(
            ranges(&[("main", &[][..])]),
            expected(&[("main", ">=1.0.0")])
        );
    }

    #[test]
    fn the_initial_version_is_configurable() {
        assert_eq!(
            ranges_with(&[("main", &[][..])], "0.1.0"),
            expected(&[("main", ">=0.1.0")])
        );
    }

    #[test]
    fn the_floor_is_the_latest_release_not_prerelease() {
        assert_eq!(
            ranges(&[("main", &["1.0.0", "1.1.0", "2.0.0-beta.1", "1.2.0"][..])]),
            expected(&[("main", ">=1.2.0")])
        );
    }

    #[test]
    fn the_initial_version_counts_only_while_the_main_line_has_no_release() {
        assert_eq!(
            ranges_with(&[("main", &["1.0.0"][..])], "2.0.0"),
            expected(&[("main", ">=1.0.0")])
        );
    }

    #[test]
    fn the_initial_version_doesnt_raise_a_later_untagged_branch() {
        assert_eq!(
            ranges_with(&[("main", &["1.0.0"][..]), ("next", &[][..])], "2.0.0"),
            expected(&[("main", ">=1.0.0"), ("next", ">=1.0.0")])
        );
    }

    #[test]
    fn a_main_line_with_only_prereleases_starts_at_the_initial_version() {
        assert_eq!(
            ranges(&[("main", &["1.0.0-beta.1"][..])]),
            expected(&[("main", ">=1.0.0")])
        );
    }

    #[rstest]
    #[case::next_ahead(
        &[("main", &["1.0.0", "1.1.0"][..]), ("next", &["1.0.0", "1.1.0", "1.3.0"][..])],
        &[("main", ">=1.1.0 <1.3.0"), ("next", ">=1.3.0")]
    )]
    #[case::three_lines(
        &[
            ("main", &["1.0.0", "1.0.1"][..]),
            ("next", &["1.0.0", "1.0.1", "1.1.0"][..]),
            ("next-major", &["1.0.0", "1.0.1", "1.1.0", "2.0.0"][..]),
        ],
        &[("main", ">=1.0.1 <1.1.0"), ("next", ">=1.1.0 <2.0.0"), ("next-major", ">=2.0.0")]
    )]
    #[case::next_level_with_main(
        &[("main", &["1.0.0", "1.1.0"][..]), ("next", &["1.0.0", "1.1.0"][..])],
        &[("main", ">=1.1.0"), ("next", ">=1.1.0")]
    )]
    #[case::next_behind_main(
        &[
            ("main", &["1.0.0", "2.0.0", "3.0.0"][..]),
            ("next", &["1.0.0", "1.1.0"][..]),
            ("next-major", &["1.0.0", "2.0.0"][..]),
        ],
        &[("main", ">=3.0.0"), ("next", ">=3.0.0"), ("next-major", ">=3.0.0")]
    )]
    #[case::prerelease_bound(
        &[("main", &["1.0.0"][..]), ("next", &["1.0.0", "2.0.0-rc.1"][..])],
        &[("main", ">=1.0.0 <2.0.0-rc.1"), ("next", ">=1.0.0")]
    )]
    #[case::bound_above_every_lower_prerelease(
        &[
            ("main", &["1.0.0", "1.1.0-rc.1"][..]),
            ("next", &["1.0.0", "1.1.0-rc.1", "1.1.0-rc.2", "1.1.0"][..]),
        ],
        &[("main", ">=1.0.0 <1.1.0-rc.2"), ("next", ">=1.1.0")]
    )]
    #[case::untagged_main_below_next(
        &[("main", &[][..]), ("next", &["1.0.0", "1.1.0"][..])],
        &[("main", ">=1.0.0 <1.0.0"), ("next", ">=1.1.0")]
    )]
    #[case::untagged_main_below_a_next_prerelease(
        &[("main", &[][..]), ("next", &["1.0.0-beta.1", "1.0.0"][..])],
        &[("main", ">=1.0.0 <1.0.0-beta.1"), ("next", ">=1.0.0")]
    )]
    #[case::versions_in_any_order(
        &[("main", &["1.1.0", "1.0.0"][..]), ("next", &["1.3.0", "1.0.0", "1.2.0", "1.1.0"][..])],
        &[("main", ">=1.1.0 <1.2.0"), ("next", ">=1.3.0")]
    )]
    fn release_ranges(
        #[case] branches: &[(&str, &[&str])],
        #[case] expected_ranges: &[(&str, &str)],
    ) {
        assert_eq!(ranges(branches), expected(expected_ranges));
    }

    #[test]
    fn a_branch_missing_from_the_versions_has_none() {
        let model = model("", &["main", "next"]).unwrap();
        let versions = HashMap::from([(String::from("next"), vec![version("1.0.0")])]);

        let ranges = model.ranges(&versions, &version("1.0.0"));

        assert_eq!(
            [ranges.range("main"), ranges.range("next")].map(|range| range.map(describe)),
            [
                Some(String::from(">=1.0.0 <1.0.0")),
                Some(String::from(">=1.0.0"))
            ]
        );
    }

    #[test]
    fn only_release_branches_have_a_range() {
        let model = model("", &["main", "beta", "1.x"]).unwrap();

        let ranges = model.ranges(&HashMap::new(), &version("1.0.0"));

        assert_eq!(
            [
                ranges.range("main").is_some(),
                ranges.range("beta").is_some(),
                ranges.range("1.x").is_some(),
                ranges.range("feature").is_some()
            ],
            [true, false, false, false]
        );
    }

    #[test]
    fn the_current_release_branch_comes_with_its_range() {
        let model = model("", &["main", "next"]).unwrap();
        let versions = HashMap::from([
            (String::from("main"), vec![version("1.0.0")]),
            (
                String::from("next"),
                vec![version("1.0.0"), version("1.1.0")],
            ),
        ]);
        let ranges = model.ranges(&versions, &version("1.0.0"));

        let current = ranges.current("next");

        assert!(
            matches!(
                current,
                Ok(Current::Release { branch, range })
                    if branch.name() == "next" && describe(range) == ">=1.1.0"
            ),
            "{current:?}"
        );
    }

    #[rstest]
    #[case::unknown("feature/x")]
    #[case::star_stops_at_slash("feature/a/b")]
    fn a_branch_no_rule_claims_is_not_configured(#[case] branch: &str) {
        let model = model(
            r#"branches.rules = ["main", { name = "feature/*", prerelease = "dev" }]"#,
            &["main", "feature/a/b"],
        )
        .unwrap();
        let ranges = model.ranges(&HashMap::new(), &version("1.0.0"));

        assert_eq!(ranges.current(branch), Ok(Current::NotConfigured));
    }
}
