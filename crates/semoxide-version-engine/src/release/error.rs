//! [`ReleaseError`]: the last release is ambiguous, or the next version can't be released.

use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};

use crate::codes::{VERSION_DUPLICATE_TAGS, VERSION_OUT_OF_RANGE, VERSION_OVERFLOW};
use crate::{BumpError, Range, Version};

/// The last release is ambiguous, or the next version can't be released from the branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseError {
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    DuplicateTags {
        version: Version,
        tags: Vec<String>,
    },
    Overflow {
        bump: BumpError,
    },
    OutOfRange {
        version: Version,
        branch: String,
        range: String,
        /// Another release branch whose range contains the version.
        fits: Option<String>,
    },
    EmptyRange {
        branch: String,
        range: String,
        /// The next release branch, whose versions bound this one's range.
        higher: Option<String>,
    },
    NotReleaseBranch {
        branch: String,
    },
}

impl ReleaseError {
    pub(super) fn duplicate_tags(version: &Version, tags: Vec<String>) -> Self {
        Self::new(Kind::DuplicateTags {
            version: version.clone(),
            tags,
        })
    }

    pub(super) fn overflow(bump: BumpError) -> Self {
        Self::new(Kind::Overflow { bump })
    }

    pub(super) fn out_of_range(
        version: &Version,
        branch: &str,
        range: &Range,
        fits: Option<&str>,
    ) -> Self {
        Self::new(Kind::OutOfRange {
            version: version.clone(),
            branch: branch.to_owned(),
            range: describe(range),
            fits: fits.map(str::to_owned),
        })
    }

    pub(super) fn empty_range(branch: &str, range: &Range, higher: Option<&str>) -> Self {
        Self::new(Kind::EmptyRange {
            branch: branch.to_owned(),
            range: describe(range),
            higher: higher.map(str::to_owned),
        })
    }

    pub(super) fn not_release_branch(branch: &str) -> Self {
        Self::new(Kind::NotReleaseBranch {
            branch: branch.to_owned(),
        })
    }

    fn new(kind: Kind) -> Self {
        Self { kind }
    }
}

/// `>=min <max`, or `>=min` without an upper bound.
fn describe(range: &Range) -> String {
    match range.max_exclusive() {
        Some(max) => format!(">={} <{max}", range.min()),
        None => format!(">={}", range.min()),
    }
}

impl fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Kind::DuplicateTags { version, tags } => {
                let tags: Vec<String> = tags.iter().map(|tag| format!("`{tag}`")).collect();
                write!(
                    f,
                    "tags {} hold the last release `{version}` on different commits",
                    tags.join(", ")
                )
            }
            Kind::Overflow { bump } => bump.fmt(f),
            Kind::OutOfRange {
                version,
                branch,
                range,
                ..
            } => write!(
                f,
                "the release `{version}` on branch `{branch}` is outside its range `{range}`"
            ),
            Kind::EmptyRange { branch, range, .. } => write!(
                f,
                "branch `{branch}` can't release any version: its range `{range}` is empty"
            ),
            Kind::NotReleaseBranch { branch } => {
                write!(
                    f,
                    "branch `{branch}` is not a release branch of these ranges"
                )
            }
        }
    }
}

impl std::error::Error for ReleaseError {}

impl ErrorInfo for ReleaseError {
    fn code(&self) -> ErrorCode {
        match self.kind {
            Kind::DuplicateTags { .. } => VERSION_DUPLICATE_TAGS,
            Kind::Overflow { .. } => VERSION_OVERFLOW,
            Kind::OutOfRange { .. } | Kind::EmptyRange { .. } | Kind::NotReleaseBranch { .. } => {
                VERSION_OUT_OF_RANGE
            }
        }
    }

    fn help(&self) -> Option<String> {
        let help = match &self.kind {
            Kind::DuplicateTags { version, .. } => {
                format!("Delete the tags on the wrong commits, so one commit holds `{version}`.")
            }
            Kind::Overflow { .. } => {
                String::from("No higher version exists at this level; release a lower level.")
            }
            Kind::OutOfRange {
                fits: Some(fits), ..
            } => format!("Release it from `{fits}`, whose range contains it."),
            Kind::OutOfRange { fits: None, .. } => {
                String::from("Release it from a branch whose range contains it.")
            }
            Kind::EmptyRange {
                branch,
                higher: Some(higher),
                ..
            } => format!("Merge `{higher}` into `{branch}`, or release from `{higher}`."),
            Kind::EmptyRange {
                branch,
                higher: None,
                ..
            } => format!("Merge the next release branch into `{branch}`."),
            Kind::NotReleaseBranch { .. } => {
                String::from("Pass a release branch of the branch model the ranges come from.")
            }
        };
        Some(help)
    }
}
