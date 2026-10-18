//! [`BranchError`]: the existing branches don't fit `branches.rules`, or the current branch can't
//! release yet.

use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};

use crate::codes::BRANCH_RELEASE_BRANCHES;

/// The existing branches don't fit `branches.rules`, or the current branch can't release yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchError {
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// The release branches, when there are none or more than 3.
    ReleaseBranches {
        branches: Vec<String>,
    },
    InvalidPrereleaseId {
        branch: String,
        id: String,
    },
    DuplicatePrereleaseIds {
        id: String,
        branches: Vec<String>,
    },
    DuplicateMaintenanceRanges {
        line: String,
        branches: Vec<String>,
    },
    NotYetSupported {
        branch: String,
        kind: Unsupported,
    },
}

/// A kind of branch that can't release yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Unsupported {
    Prerelease,
    Maintenance,
}

impl fmt::Display for BranchError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl std::error::Error for BranchError {}

impl ErrorInfo for BranchError {
    fn code(&self) -> ErrorCode {
        BRANCH_RELEASE_BRANCHES
    }

    fn help(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rstest::rstest;

    use super::super::test_support::model;
    use super::*;
    use crate::Version;
    use crate::codes::{
        BRANCH_DUPLICATE_MAINTENANCE_RANGES, BRANCH_DUPLICATE_PRERELEASE_IDS,
        BRANCH_INVALID_PRERELEASE_ID, BRANCH_NOT_YET_SUPPORTED,
    };

    /// The code, message and help of the error, or `Ok` if there was none.
    fn failure<T>(result: Result<T, BranchError>) -> Result<(), (ErrorCode, String, String)> {
        result.map(|_| ()).map_err(|error| {
            (
                error.code(),
                error.to_string(),
                error.help().unwrap_or_default(),
            )
        })
    }

    #[rstest]
    #[case::no_release_branch(
        "",
        &["beta", "1.x"],
        BRANCH_RELEASE_BRANCHES,
        "no release branch: no release rule matches an existing branch",
        "Create a branch a release rule names, such as `main`, or add a rule for an existing branch."
    )]
    #[case::too_many_release_branches(
        "",
        &["master", "main", "next", "next-major"],
        BRANCH_RELEASE_BRANCHES,
        "4 release branches (`master`, `main`, `next`, `next-major`); at most 3 are allowed",
        "Remove release rules, or make some of these branches prerelease branches."
    )]
    #[case::invalid_prerelease_id_from_the_name(
        r#"branches.rules = ["main", { name = "pre/*", prerelease = true }]"#,
        &["main", "pre/a"],
        BRANCH_INVALID_PRERELEASE_ID,
        "branch `pre/a` gives the prerelease identifier `pre/a`, which is not valid",
        "Set `prerelease` on its rule to a valid identifier (letters, digits, `-` and `.`, no leading zeros)."
    )]
    #[case::invalid_prerelease_id_from_a_template(
        r#"branches.rules = ["main", { name = "x*", prerelease = "{name}" }]"#,
        &["main", "x_y"],
        BRANCH_INVALID_PRERELEASE_ID,
        "branch `x_y` gives the prerelease identifier `x_y`, which is not valid",
        "Set `prerelease` on its rule to a valid identifier (letters, digits, `-` and `.`, no leading zeros)."
    )]
    #[case::invalid_prerelease_id_differing_from_the_name(
        r#"branches.rules = ["main", { name = "x*", prerelease = "pre-{name}" }]"#,
        &["main", "x_y"],
        BRANCH_INVALID_PRERELEASE_ID,
        "branch `x_y` gives the prerelease identifier `pre-x_y`, which is not valid",
        "Set `prerelease` on its rule to a valid identifier (letters, digits, `-` and `.`, no leading zeros)."
    )]
    #[case::duplicate_prerelease_ids(
        r#"branches.rules = ["main", { name = "beta", prerelease = "rc" }, { name = "alpha", prerelease = "rc" }]"#,
        &["main", "beta", "alpha"],
        BRANCH_DUPLICATE_PRERELEASE_IDS,
        "branches `beta`, `alpha` share the prerelease identifier `rc`",
        "Give each prerelease branch its own identifier with `prerelease`."
    )]
    #[case::duplicate_prerelease_ids_from_a_glob(
        r#"branches.rules = ["main", { name = "b*", prerelease = "rc" }]"#,
        &["main", "b2", "b1"],
        BRANCH_DUPLICATE_PRERELEASE_IDS,
        "branches `b1`, `b2` share the prerelease identifier `rc`",
        "Give each prerelease branch its own identifier with `prerelease`."
    )]
    #[case::duplicate_maintenance_ranges(
        "",
        &["main", "1.x", "1.x.x"],
        BRANCH_DUPLICATE_MAINTENANCE_RANGES,
        "branches `1.x`, `1.x.x` maintain the same range `1.x`",
        "Keep one maintenance branch per range."
    )]
    #[case::duplicate_maintenance_ranges_by_case(
        "",
        &["main", "1.x", "1.X"],
        BRANCH_DUPLICATE_MAINTENANCE_RANGES,
        "branches `1.X`, `1.x` maintain the same range `1.x`",
        "Keep one maintenance branch per range."
    )]
    #[case::duplicate_maintenance_ranges_with_an_explicit_range(
        r#"branches.rules = [{ maintenance = "legacy", range = "1.x" }, { maintenance = "N.x" }, "main"]"#,
        &["legacy", "1.x.x", "main"],
        BRANCH_DUPLICATE_MAINTENANCE_RANGES,
        "branches `legacy`, `1.x.x` maintain the same range `1.x`",
        "Keep one maintenance branch per range."
    )]
    fn a_model_that_doesnt_fit_is_an_error(
        #[case] text: &str,
        #[case] branches: &[&str],
        #[case] code: ErrorCode,
        #[case] message: &str,
        #[case] help: &str,
    ) {
        assert_eq!(
            failure(model(text, branches)),
            Err((code, message.to_owned(), help.to_owned()))
        );
    }

    #[rstest]
    #[case::prerelease(
        "beta",
        "releasing from the prerelease branch `beta` is not supported yet"
    )]
    #[case::maintenance(
        "1.x",
        "releasing from the maintenance branch `1.x` is not supported yet"
    )]
    fn releasing_from_a_prerelease_or_maintenance_branch_is_not_supported_yet(
        #[case] branch: &str,
        #[case] message: &str,
    ) {
        let model = model("", &["main", "beta", "1.x"]).unwrap();
        let ranges = model.ranges(
            &HashMap::new(),
            &Version::from_parts(1, 0, 0, semver::Prerelease::EMPTY),
        );

        assert_eq!(
            failure(ranges.current(branch)),
            Err((
                BRANCH_NOT_YET_SUPPORTED,
                message.to_owned(),
                String::from(
                    "Release from a release branch; prerelease and maintenance releases come in a later version."
                )
            ))
        );
    }
}
