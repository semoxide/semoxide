//! GitLab CI/CD: detected by `GITLAB_CI`; merge request and external pull request pipelines
//! release nothing.

use super::{PullRequest, Run, number, value};
use crate::{Env, EnvError};

/// The variables of one kind of pull request pipeline.
struct Request {
    source: &'static str,
    iid: &'static str,
    target_branch: &'static str,
}

const MERGE_REQUEST: Request = Request {
    source: "merge_request_event",
    iid: "CI_MERGE_REQUEST_IID",
    target_branch: "CI_MERGE_REQUEST_TARGET_BRANCH_NAME",
};

/// A GitHub pull request built by GitLab CI.
const EXTERNAL_PULL_REQUEST: Request = Request {
    source: "external_pull_request_event",
    iid: "CI_EXTERNAL_PULL_REQUEST_IID",
    target_branch: "CI_EXTERNAL_PULL_REQUEST_TARGET_BRANCH_NAME",
};

pub(super) fn read(env: &Env) -> Result<Run, EnvError> {
    let commit = value(env, "CI_COMMIT_SHA")?.map(str::to_owned);
    let Some(request) = request(env)? else {
        let branch = value(env, "CI_COMMIT_BRANCH")?.map(str::to_owned);
        return Ok((branch, commit, PullRequest::No));
    };
    let number = value(env, request.iid)?.and_then(number);
    let branch = value(env, request.target_branch)?.map(str::to_owned);
    Ok((branch, commit, PullRequest::Yes { number }))
}

/// The kind of pull request pipeline: an external one by its source, otherwise a merge request
/// by its source or IID, otherwise an external one by its IID.
fn request(env: &Env) -> Result<Option<Request>, EnvError> {
    let source = value(env, "CI_PIPELINE_SOURCE")?;
    if source == Some(EXTERNAL_PULL_REQUEST.source) {
        return Ok(Some(EXTERNAL_PULL_REQUEST));
    }
    if source == Some(MERGE_REQUEST.source) || value(env, MERGE_REQUEST.iid)?.is_some() {
        return Ok(Some(MERGE_REQUEST));
    }
    Ok(value(env, EXTERNAL_PULL_REQUEST.iid)?.map(|_| EXTERNAL_PULL_REQUEST))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::super::PullRequest::{self, No, Yes};
    use super::super::test_support::fields;

    /// env-ci's test environment for GitLab, as a branch pipeline (env-ci also sets a tag,
    /// which a branch pipeline never has), plus `CI_DEFAULT_BRANCH`, which GitLab always sets.
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
        ("CI_DEFAULT_BRANCH", "main"),
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
    // The ref name isn't a branch without `CI_COMMIT_BRANCH`.
    #[case::differs_ref_name_is_not_the_branch(&[], None, No)]
    // Merge request variables make a merge request run whatever the pipeline source, e.g. a
    // child pipeline.
    #[case::differs_mr_variables_in_a_child_pipeline(
        &[
            ("CI_PIPELINE_SOURCE", "parent_pipeline"),
            ("CI_MERGE_REQUEST_IID", "10"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "main"),
        ],
        Some("main"),
        mr(10)
    )]
    #[case::differs_external_pr_variables_without_its_source(
        &[
            ("CI_EXTERNAL_PULL_REQUEST_IID", "7"),
            ("CI_EXTERNAL_PULL_REQUEST_TARGET_BRANCH_NAME", "main"),
        ],
        Some("main"),
        mr(7)
    )]
    // A pull request's branch is its target branch or unknown, never the commit's branch.
    #[case::differs_external_pr_without_target(
        &[
            ("CI_PIPELINE_SOURCE", "external_pull_request_event"),
            ("CI_EXTERNAL_PULL_REQUEST_IID", "7"),
            ("CI_COMMIT_BRANCH", "feature"),
        ],
        None,
        mr(7)
    )]
    // On an external pull request pipeline its own variables win over stray merge request ones.
    #[case::differs_external_pr_source_wins(
        &[
            ("CI_PIPELINE_SOURCE", "external_pull_request_event"),
            ("CI_MERGE_REQUEST_IID", "10"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "dev"),
            ("CI_EXTERNAL_PULL_REQUEST_IID", "7"),
            ("CI_EXTERNAL_PULL_REQUEST_TARGET_BRANCH_NAME", "main"),
        ],
        Some("main"),
        mr(7)
    )]
    // Otherwise the merge request's variables come first.
    #[case::differs_mr_variables_win_unless_the_source_is_external(
        &[
            ("CI_MERGE_REQUEST_IID", "10"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "dev"),
            ("CI_EXTERNAL_PULL_REQUEST_IID", "7"),
            ("CI_EXTERNAL_PULL_REQUEST_TARGET_BRANCH_NAME", "main"),
        ],
        Some("dev"),
        mr(10)
    )]
    #[case::differs_iid_with_a_leading_zero(
        &[
            ("CI_PIPELINE_SOURCE", "merge_request_event"),
            ("CI_MERGE_REQUEST_IID", "010"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "main"),
        ],
        Some("main"),
        mr(10)
    )]
    // The IID is plain digits or no number.
    #[case::differs_iid_not_digits(
        &[
            ("CI_PIPELINE_SOURCE", "merge_request_event"),
            ("CI_MERGE_REQUEST_IID", "+10"),
            ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "main"),
        ],
        Some("main"),
        Yes { number: None }
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
