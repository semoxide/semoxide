//! GitHub Actions: detected by `GITHUB_ACTIONS`; the event name tells pull request runs apart.

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
    // Ported from env-ci's tests, same results.
    #[case::push(&[], Some("master"), No)]
    #[case::push_with_short_branch_name(&[("GITHUB_REF", "master")], Some("master"), No)]
    #[case::push_with_incorrect_branch_name(&[("GITHUB_REF", "")], None, No)]
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
    // Review runs build the pull request's merge commit; env-ci calls them pushes.
    #[case::differs_pr_review(
        &[("GITHUB_EVENT_NAME", "pull_request_review"), ("GITHUB_REF", "refs/pull/10/merge")],
        None,
        pr(10)
    )]
    #[case::differs_pr_review_comment(
        &[
            ("GITHUB_EVENT_NAME", "pull_request_review_comment"),
            ("GITHUB_REF", "refs/pull/10/merge"),
        ],
        None,
        pr(10)
    )]
    // A merge queue run builds a temporary `gh-readonly-queue/<base>/…` branch before merging.
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
