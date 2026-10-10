//! `SEMOXIDE_CI_BRANCH` and `SEMOXIDE_CI_IS_PR`: override what the vendor variables say, on any
//! CI or none (CLI.md).

use super::CiContext;
use crate::{Env, EnvError};

/// `context` with the overrides set in `env` applied.
pub(super) fn apply(_env: &Env, context: CiContext) -> Result<CiContext, EnvError> {
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use semoxide_error::{ErrorCode, ErrorInfo};

    use super::super::test_support::{detected, env, not_unicode};
    use super::super::{PullRequest, detect};
    use crate::Env;
    use crate::codes::{ENV_INVALID_VALUE, ENV_NOT_UNICODE};
    use crate::env::Case;

    const GITHUB_PUSH: &[(&str, &str)] = &[
        ("GITHUB_ACTIONS", "true"),
        ("GITHUB_SHA", "1234"),
        ("GITHUB_REF", "refs/heads/master"),
    ];

    const GITHUB_PR: &[(&str, &str)] = &[
        ("GITHUB_ACTIONS", "true"),
        ("GITHUB_SHA", "1234"),
        ("GITHUB_EVENT_NAME", "pull_request"),
        ("GITHUB_REF", "refs/pull/10/merge"),
        ("GITHUB_BASE_REF", "master"),
    ];

    fn vars<'a>(
        base: &[(&'a str, &'a str)],
        extra: &[(&'a str, &'a str)],
    ) -> Vec<(&'a str, &'a str)> {
        base.iter().chain(extra).copied().collect()
    }

    /// The context of `base` with `extra` set on top.
    fn context(base: &[(&str, &str)], extra: &[(&str, &str)]) -> super::CiContext {
        detected(&vars(base, extra))
    }

    const fn pr(number: u64) -> PullRequest {
        PullRequest::Yes {
            number: Some(number),
        }
    }

    const PR_WITHOUT_NUMBER: PullRequest = PullRequest::Yes { number: None };

    /// The code, message and help of the error detection fails with, if it does.
    fn failure(env: &Env) -> Option<(ErrorCode, String, Option<String>)> {
        detect(env)
            .err()
            .map(|error| (error.code(), error.to_string(), error.help()))
    }

    // --- SEMOXIDE_CI_BRANCH ---

    #[test]
    fn the_branch_override_replaces_the_detected_branch() {
        let overridden = context(GITHUB_PUSH, &[("SEMOXIDE_CI_BRANCH", "release")]);
        let detected_branch = context(GITHUB_PUSH, &[]);

        assert_eq!(
            (overridden.branch(), detected_branch.branch()),
            (Some("release"), Some("master"))
        );
    }

    #[test]
    fn the_branch_override_applies_without_a_vendor_and_doesnt_make_a_run_ci() {
        let generic = context(&[("CI", "true")], &[("SEMOXIDE_CI_BRANCH", "main")]);
        let local = context(&[("PATH", "/bin")], &[("SEMOXIDE_CI_BRANCH", "main")]);

        assert_eq!(
            (
                (generic.branch(), generic.is_ci(), generic.vendor()),
                (local.branch(), local.is_ci(), local.vendor())
            ),
            ((Some("main"), true, None), (Some("main"), false, None))
        );
    }

    #[test]
    fn the_branch_override_is_used_as_given() {
        let context = context(GITHUB_PUSH, &[("SEMOXIDE_CI_BRANCH", "refs/heads/main")]);

        assert_eq!(context.branch(), Some("refs/heads/main"));
    }

    // --- SEMOXIDE_CI_IS_PR ---

    #[test]
    fn is_pr_true_makes_a_push_a_pull_request_and_keeps_a_detected_number() {
        let push = context(GITHUB_PUSH, &[("SEMOXIDE_CI_IS_PR", "true")]);
        let detected_pr = context(GITHUB_PR, &[("SEMOXIDE_CI_IS_PR", "true")]);

        assert_eq!(
            (push.pull_request(), detected_pr.pull_request()),
            (PR_WITHOUT_NUMBER, pr(10))
        );
    }

    #[test]
    fn is_pr_false_makes_a_pull_request_a_branch_run() {
        let overridden = context(GITHUB_PR, &[("SEMOXIDE_CI_IS_PR", "false")]);
        let detected_pr = context(GITHUB_PR, &[]);

        assert_eq!(
            (overridden.pull_request(), detected_pr.pull_request()),
            (PullRequest::No, pr(10))
        );
    }

    #[test]
    fn is_pr_ignores_letter_case() {
        let upper_true = context(GITHUB_PUSH, &[("SEMOXIDE_CI_IS_PR", "True")]);
        let upper_false = context(GITHUB_PR, &[("SEMOXIDE_CI_IS_PR", "FALSE")]);

        assert_eq!(
            (upper_true.pull_request(), upper_false.pull_request()),
            (PR_WITHOUT_NUMBER, PullRequest::No)
        );
    }

    #[test]
    fn is_pr_with_another_value_is_an_error() {
        let env = env(&vars(GITHUB_PUSH, &[("SEMOXIDE_CI_IS_PR", "yes")]));

        assert_eq!(
            failure(&env),
            Some((
                ENV_INVALID_VALUE,
                String::from(
                    "the value of `SEMOXIDE_CI_IS_PR` is `yes`: expected `true` or `false`"
                ),
                Some(String::from(
                    "Set `SEMOXIDE_CI_IS_PR` to `true` or `false`."
                ))
            ))
        );
    }

    // --- Both ---

    #[test]
    fn empty_overrides_count_as_unset() {
        let empty = context(
            GITHUB_PR,
            &[("SEMOXIDE_CI_BRANCH", ""), ("SEMOXIDE_CI_IS_PR", "")],
        );
        let one_set = context(
            GITHUB_PR,
            &[("SEMOXIDE_CI_BRANCH", ""), ("SEMOXIDE_CI_IS_PR", "false")],
        );

        assert_eq!(
            (
                (empty.branch(), empty.pull_request()),
                (one_set.branch(), one_set.pull_request())
            ),
            ((Some("master"), pr(10)), (Some("master"), PullRequest::No))
        );
    }

    #[test]
    fn an_override_that_isnt_utf_8_is_an_error() {
        let env = Env::with_case(
            Case::Sensitive,
            [
                (OsString::from("CI"), OsString::from("true")),
                (OsString::from("SEMOXIDE_CI_BRANCH"), not_unicode()),
            ],
        );

        assert_eq!(
            failure(&env).map(|(code, message, _)| (code, message)),
            Some((
                ENV_NOT_UNICODE,
                String::from("the value of `SEMOXIDE_CI_BRANCH` isn't valid UTF-8")
            ))
        );
    }
}
