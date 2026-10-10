//! GitHub Actions: detected by `GITHUB_ACTIONS`; the event name tells pull request runs apart.

use super::{PullRequest, Run, number, value};
use crate::{Env, EnvError};

/// The events whose runs build a pull request before it merges.
const PULL_REQUEST_EVENTS: [&str; 5] = [
    "pull_request",
    "pull_request_target",
    "pull_request_review",
    "pull_request_review_comment",
    "merge_group",
];

/// The temporary branches a merge queue builds: `<prefix><base>/pr-<n>-<sha>`.
const MERGE_QUEUE_PREFIX: &str = "refs/heads/gh-readonly-queue/";

pub(super) fn read(env: &Env) -> Result<Run, EnvError> {
    let commit = value(env, "GITHUB_SHA")?.map(str::to_owned);
    let event = value(env, "GITHUB_EVENT_NAME")?;
    let git_ref = value(env, "GITHUB_REF")?;
    let Some(event) = event.filter(|event| PULL_REQUEST_EVENTS.contains(event)) else {
        let branch = git_ref.and_then(branch_name).map(str::to_owned);
        return Ok((branch, commit, PullRequest::No));
    };
    // `GITHUB_BASE_REF` is set for these two events only; review runs have no branch.
    let branch = match event {
        "pull_request" | "pull_request_target" => value(env, "GITHUB_BASE_REF")?,
        "merge_group" => git_ref.and_then(merge_queue_base),
        _ => None,
    };
    let number = git_ref.and_then(pull_request_number);
    Ok((
        branch.map(str::to_owned),
        commit,
        PullRequest::Yes { number },
    ))
}

/// The branch of `refs/heads/<name>` or of a bare name; no other ref is a branch.
fn branch_name(git_ref: &str) -> Option<&str> {
    match git_ref.strip_prefix("refs/heads/") {
        Some(name) => Some(name).filter(|name| !name.is_empty()),
        None => Some(git_ref).filter(|name| !name.starts_with("refs/")),
    }
}

/// The base branch of a merge queue ref, which ends at the last `/pr-`.
fn merge_queue_base(git_ref: &str) -> Option<&str> {
    let (base, _) = git_ref
        .strip_prefix(MERGE_QUEUE_PREFIX)?
        .rsplit_once("/pr-")?;
    Some(base).filter(|base| !base.is_empty())
}

/// The number of `refs/pull/<n>/merge`.
fn pull_request_number(git_ref: &str) -> Option<u64> {
    number(git_ref.strip_prefix("refs/pull/")?.strip_suffix("/merge")?)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::super::PullRequest::{self, No, Yes};
    use super::super::test_support::fields;

    /// env-ci's test environment for GitHub Actions.
    const BASE: &[(&str, &str)] = &[
        ("GITHUB_ACTIONS", "true"),
        ("GITHUB_SHA", "1234"),
        ("GITHUB_REF", "refs/heads/master"),
        ("GITHUB_REPOSITORY", "owner/repo"),
        ("GITHUB_WORKSPACE", "/workspace"),
        ("GITHUB_RUN_ID", "1246789"),
        ("GITHUB_SERVER_URL", "https://github.com"),
    ];

    const fn pr(number: u64) -> PullRequest {
        Yes {
            number: Some(number),
        }
    }

    const PR_WITHOUT_NUMBER: PullRequest = Yes { number: None };

    #[rstest]
    // Same results as env-ci (the first three ported from its tests).
    #[case::push(&[], Some("master"), No)]
    #[case::push_with_short_branch_name(&[("GITHUB_REF", "master")], Some("master"), No)]
    #[case::push_with_incorrect_branch_name(&[("GITHUB_REF", "")], None, No)]
    #[case::push_of_a_branch_with_a_slash(
        &[("GITHUB_REF", "refs/heads/release/1.x")],
        Some("release/1.x"),
        No
    )]
    // `GITHUB_BASE_REF` belongs to pull request events only.
    #[case::push_ignores_a_base_ref(&[("GITHUB_BASE_REF", "dev")], Some("master"), No)]
    // Only the listed events are pull request runs.
    #[case::unlisted_pull_request_event(
        &[("GITHUB_EVENT_NAME", "pull_request_x"), ("GITHUB_REF", "refs/heads/main")],
        Some("main"),
        No
    )]
    // env-ci reads the event file; `GITHUB_BASE_REF` holds the same branch.
    #[case::pr(
        &[
            ("GITHUB_EVENT_NAME", "pull_request"),
            ("GITHUB_REF", "refs/pull/10/merge"),
            ("GITHUB_BASE_REF", "master"),
        ],
        Some("master"),
        pr(10)
    )]
    // Differences from env-ci (DIFFERENCES, CI detection).
    // The number comes from `refs/pull/<n>/merge`, not the event file.
    #[case::differs_pr_without_base_ref(
        &[("GITHUB_EVENT_NAME", "pull_request"), ("GITHUB_REF", "refs/pull/10/merge")],
        None,
        pr(10)
    )]
    // `GITHUB_REF` is the default branch here, so the number is only in the event file.
    #[case::differs_pr_target(
        &[
            ("GITHUB_EVENT_NAME", "pull_request_target"),
            ("GITHUB_REF", "refs/heads/main"),
            ("GITHUB_BASE_REF", "master"),
        ],
        Some("master"),
        PR_WITHOUT_NUMBER
    )]
    // A tag is not a branch; env-ci returns `refs/tags/v1.0.0`.
    #[case::differs_tag_push(&[("GITHUB_REF", "refs/tags/v1.0.0")], None, No)]
    #[case::differs_tag_push_ignores_a_base_ref(
        &[("GITHUB_REF", "refs/tags/v1.0.0"), ("GITHUB_BASE_REF", "dev")],
        None,
        No
    )]
    // `GITHUB_REF` is no pull request ref here, so there's no number; the base ref is the branch.
    #[case::differs_pr_target_on_a_numbered_branch(
        &[
            ("GITHUB_EVENT_NAME", "pull_request_target"),
            ("GITHUB_REF", "refs/heads/release/2"),
            ("GITHUB_BASE_REF", "release/2"),
        ],
        Some("release/2"),
        PR_WITHOUT_NUMBER
    )]
    // Without `GITHUB_BASE_REF` the branch is unknown; `GITHUB_REF` is the default branch.
    #[case::differs_pr_target_without_base_ref(
        &[("GITHUB_EVENT_NAME", "pull_request_target"), ("GITHUB_REF", "refs/heads/main")],
        None,
        PR_WITHOUT_NUMBER
    )]
    // Only `refs/pull/<n>/merge` carries the number, and only as plain digits.
    #[case::differs_pr_head_ref(
        &[
            ("GITHUB_EVENT_NAME", "pull_request"),
            ("GITHUB_REF", "refs/pull/10/head"),
            ("GITHUB_BASE_REF", "master"),
        ],
        Some("master"),
        PR_WITHOUT_NUMBER
    )]
    #[case::differs_pr_number_not_digits(
        &[
            ("GITHUB_EVENT_NAME", "pull_request"),
            ("GITHUB_REF", "refs/pull/+10/merge"),
            ("GITHUB_BASE_REF", "master"),
        ],
        Some("master"),
        PR_WITHOUT_NUMBER
    )]
    // The event name decides a pull request run, not the ref.
    #[case::differs_pull_ref_without_a_pr_event(
        &[("GITHUB_REF", "refs/pull/10/merge")],
        None,
        No
    )]
    // Review runs build the pull request's merge commit; env-ci calls them pushes. GitHub sets
    // no base ref for them, so the branch is unknown even if one is set.
    #[case::differs_pr_review(
        &[
            ("GITHUB_EVENT_NAME", "pull_request_review"),
            ("GITHUB_REF", "refs/pull/10/merge"),
            ("GITHUB_BASE_REF", "master"),
        ],
        None,
        pr(10)
    )]
    #[case::differs_pr_review_comment(
        &[
            ("GITHUB_EVENT_NAME", "pull_request_review_comment"),
            ("GITHUB_REF", "refs/pull/10/merge"),
            ("GITHUB_BASE_REF", "master"),
        ],
        None,
        pr(10)
    )]
    // A merge queue run builds a temporary `gh-readonly-queue/<base>/pr-…` branch before
    // merging; the base ends at the last `/pr-`.
    #[case::differs_merge_group(
        &[
            ("GITHUB_EVENT_NAME", "merge_group"),
            ("GITHUB_REF", "refs/heads/gh-readonly-queue/main/pr-10-0123abcd"),
        ],
        Some("main"),
        PR_WITHOUT_NUMBER
    )]
    #[case::differs_merge_group_base_with_a_slash(
        &[
            ("GITHUB_EVENT_NAME", "merge_group"),
            ("GITHUB_REF", "refs/heads/gh-readonly-queue/release/1.x/pr-10-0123abcd"),
        ],
        Some("release/1.x"),
        PR_WITHOUT_NUMBER
    )]
    #[case::differs_merge_group_base_containing_pr(
        &[
            ("GITHUB_EVENT_NAME", "merge_group"),
            ("GITHUB_REF", "refs/heads/gh-readonly-queue/team/pr-x/pr-10-0123abcd"),
        ],
        Some("team/pr-x"),
        PR_WITHOUT_NUMBER
    )]
    #[case::differs_merge_group_without_a_pr_part(
        &[
            ("GITHUB_EVENT_NAME", "merge_group"),
            ("GITHUB_REF", "refs/heads/gh-readonly-queue/main/other"),
        ],
        None,
        PR_WITHOUT_NUMBER
    )]
    #[case::differs_merge_group_pr_without_a_slash(
        &[
            ("GITHUB_EVENT_NAME", "merge_group"),
            ("GITHUB_REF", "refs/heads/gh-readonly-queue/main/xpr-10-0123abcd"),
        ],
        None,
        PR_WITHOUT_NUMBER
    )]
    #[case::differs_merge_group_empty_base(
        &[
            ("GITHUB_EVENT_NAME", "merge_group"),
            ("GITHUB_REF", "refs/heads/gh-readonly-queue//pr-10-0123abcd"),
        ],
        None,
        PR_WITHOUT_NUMBER
    )]
    #[case::differs_merge_group_other_ref(
        &[("GITHUB_EVENT_NAME", "merge_group"), ("GITHUB_REF", "refs/heads/main")],
        None,
        PR_WITHOUT_NUMBER
    )]
    // Only `refs/heads/<name>` or a bare name is a branch.
    #[case::differs_empty_branch_ref(&[("GITHUB_REF", "refs/heads/")], None, No)]
    #[case::differs_other_ref(&[("GITHUB_REF", "refs/remotes/origin/main")], None, No)]
    fn github_actions(
        #[case] extra: &[(&str, &str)],
        #[case] branch: Option<&str>,
        #[case] pull_request: PullRequest,
    ) {
        assert_eq!(
            fields(BASE, extra),
            (
                branch.map(str::to_owned),
                Some(String::from("1234")),
                pull_request
            )
        );
    }
}
