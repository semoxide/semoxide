//! GitLab CI/CD: detected by `GITLAB_CI`; merge request and external pull request pipelines
//! release nothing.

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::super::PullRequest::{self, No, Yes};
    use super::super::test_support::fields;

    /// env-ci's test environment for GitLab, as a branch pipeline (env-ci also sets a tag,
    /// which a branch pipeline never has).
    const BASE: &[(&str, &str)] = &[
        ("GITLAB_CI", "true"),
        ("CI_COMMIT_SHA", "5678"),
        ("CI_PIPELINE_ID", "91011"),
        ("CI_JOB_ID", "1213"),
        ("CI_PROJECT_URL", "https://gitlab.com/owner/repo"),
        ("CI_COMMIT_REF_NAME", "master"),
        ("CI_PROJECT_PATH", "owner/repo"),
        ("CI_PROJECT_DIR", "/"),
        ("CI_PIPELINE_SOURCE", "push"),
    ];

    const fn mr(number: u64) -> PullRequest {
        Yes {
            number: Some(number),
        }
    }

    #[rstest]
    // Ported from env-ci's tests, same results.
    #[case::push(&[("CI_COMMIT_BRANCH", "master")], Some("master"), No)]
    // Differences from env-ci (DIFFERENCES, CI detection).
    // The number is the IID users see (`!10`); env-ci reports the instance-wide ID.
    #[case::differs_pr(
        &[
            ("CI_PIPELINE_SOURCE", "merge_request_event"),
            ("CI_MERGE_REQUEST_ID", "1010"),
            ("CI_MERGE_REQUEST_IID", "10"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "master"),
            ("CI_MERGE_REQUEST_SOURCE_BRANCH_NAME", "pr-branch"),
            ("CI_COMMIT_REF_NAME", "pr-branch"),
        ],
        Some("master"),
        mr(10)
    )]
    // A merge train builds a commit that isn't on the target branch yet.
    #[case::differs_merge_train(
        &[
            ("CI_PIPELINE_SOURCE", "merge_request_event"),
            ("CI_MERGE_REQUEST_EVENT_TYPE", "merge_train"),
            ("CI_MERGE_REQUEST_ID", "1010"),
            ("CI_MERGE_REQUEST_IID", "10"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "main"),
            ("CI_COMMIT_REF_NAME", "feature"),
        ],
        Some("main"),
        mr(10)
    )]
    // The merge request's variables are gone once it is closed; the pipeline source stays.
    #[case::differs_closed_mr(
        &[
            ("CI_PIPELINE_SOURCE", "merge_request_event"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "main"),
            ("CI_COMMIT_REF_NAME", "feature"),
        ],
        Some("main"),
        Yes { number: None }
    )]
    // A GitHub pull request built by GitLab CI; env-ci treats it as its source branch.
    #[case::differs_external_pr(
        &[
            ("CI_PIPELINE_SOURCE", "external_pull_request_event"),
            ("CI_EXTERNAL_PULL_REQUEST_IID", "7"),
            ("CI_EXTERNAL_PULL_REQUEST_TARGET_BRANCH_NAME", "main"),
            ("CI_EXTERNAL_PULL_REQUEST_SOURCE_BRANCH_NAME", "feature"),
            ("CI_COMMIT_REF_NAME", "feature"),
            ("CI_COMMIT_BRANCH", "feature"),
        ],
        Some("main"),
        mr(7)
    )]
    // A tag is not a branch; env-ci returns the tag name.
    #[case::differs_tag_pipeline(
        &[("CI_COMMIT_TAG", "v1.0.0"), ("CI_COMMIT_REF_NAME", "v1.0.0")],
        None,
        No
    )]
    fn gitlab_ci(
        #[case] extra: &[(&str, &str)],
        #[case] branch: Option<&str>,
        #[case] pull_request: PullRequest,
    ) {
        assert_eq!(
            fields(BASE, extra),
            (
                branch.map(str::to_owned),
                Some(String::from("5678")),
                pull_request
            )
        );
    }
}
