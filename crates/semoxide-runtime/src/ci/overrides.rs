//! `SEMOXIDE_CI_BRANCH` and `SEMOXIDE_CI_IS_PR`: override what the vendor variables say, on any
//! CI or none (CLI.md).

use super::{CiContext, PullRequest};
use crate::{Env, EnvError};

/// `context` with the overrides set in `env` applied; empty ones count as unset.
pub(super) fn apply(env: &Env, mut context: CiContext) -> Result<CiContext, EnvError> {
    if let Some((_, branch)) = set(env, "SEMOXIDE_CI_BRANCH")? {
        context.branch = Some(branch.to_owned());
    }
    let Some((name, is_pr)) = set(env, "SEMOXIDE_CI_IS_PR")? else {
        return Ok(context);
    };
    context.pull_request = if is_pr.eq_ignore_ascii_case("true") {
        match context.pull_request {
            PullRequest::No => PullRequest::Yes { number: None },
            detected => detected,
        }
    } else if is_pr.eq_ignore_ascii_case("false") {
        PullRequest::No
    } else {
        return Err(EnvError::invalid(name, is_pr, "`true` or `false`"));
    };
    Ok(context)
}

/// The name as set and the value of `name`, if it is set and not empty.
fn set<'a>(env: &'a Env, name: &str) -> Result<Option<(&'a str, &'a str)>, EnvError> {
    Ok(env.var_named(name)?.filter(|(_, value)| !value.is_empty()))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use rstest::rstest;
    use semoxide_error::{ErrorCode, ErrorInfo};

    use super::super::test_support::{detected, env, not_unicode};
    use super::super::{PullRequest, Vendor, detect};
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

    #[test]
    fn the_branch_override_applies_to_pull_requests_and_runs_without_a_detected_branch() {
        let pull_request = context(GITHUB_PR, &[("SEMOXIDE_CI_BRANCH", "release")]);
        let review = context(
            &[
                ("GITHUB_ACTIONS", "true"),
                ("GITHUB_EVENT_NAME", "pull_request_review"),
                ("GITHUB_REF", "refs/pull/10/merge"),
            ],
            &[("SEMOXIDE_CI_BRANCH", "r")],
        );

        assert_eq!(
            (
                (pull_request.branch(), pull_request.pull_request()),
                review.branch()
            ),
            ((Some("release"), pr(10)), Some("r"))
        );
    }

    #[test]
    fn the_branch_override_keeps_spaces_case_and_refs_without_a_vendor() {
        let spaces = context(GITHUB_PUSH, &[("SEMOXIDE_CI_BRANCH", " Main ")]);
        let generic = context(
            &[("CI", "true")],
            &[("SEMOXIDE_CI_BRANCH", "refs/heads/main")],
        );

        assert_eq!(
            (spaces.branch(), generic.branch()),
            (Some(" Main "), Some("refs/heads/main"))
        );
    }

    #[test]
    fn overrides_leave_the_commit_and_vendor_alone() {
        let context = context(
            GITHUB_PUSH,
            &[("SEMOXIDE_CI_BRANCH", "r"), ("SEMOXIDE_CI_IS_PR", "true")],
        );

        assert_eq!(
            (
                context.branch(),
                context.commit(),
                context.vendor(),
                context.is_ci()
            ),
            (Some("r"), Some("1234"), Some(Vendor::GitHubActions), true)
        );
    }

    #[test]
    fn is_pr_applies_without_a_vendor_and_doesnt_make_a_run_ci() {
        let local = context(&[("PATH", "/bin")], &[("SEMOXIDE_CI_IS_PR", "true")]);
        let generic = context(&[("CI", "true")], &[("SEMOXIDE_CI_IS_PR", "false")]);

        assert_eq!(
            (
                (local.pull_request(), local.is_ci(), local.vendor()),
                (generic.pull_request(), generic.is_ci(), generic.vendor())
            ),
            (
                (PR_WITHOUT_NUMBER, false, None),
                (PullRequest::No, true, None)
            )
        );
    }

    const GITLAB_MR: &[(&str, &str)] = &[
        ("GITLAB_CI", "true"),
        ("CI_COMMIT_SHA", "5678"),
        ("CI_PIPELINE_SOURCE", "merge_request_event"),
        ("CI_MERGE_REQUEST_IID", "10"),
        ("CI_MERGE_REQUEST_TARGET_BRANCH_NAME", "main"),
    ];

    #[test]
    fn is_pr_keeps_a_gitlab_iid_or_turns_the_merge_request_off() {
        let kept = context(GITLAB_MR, &[("SEMOXIDE_CI_IS_PR", "true")]);
        let off = context(GITLAB_MR, &[("SEMOXIDE_CI_IS_PR", "false")]);

        assert_eq!(
            (kept.pull_request(), off.pull_request()),
            (pr(10), PullRequest::No)
        );
    }

    #[test]
    fn is_pr_ignores_mixed_letter_case() {
        let mixed_true = context(GITHUB_PUSH, &[("SEMOXIDE_CI_IS_PR", "tRuE")]);
        let mixed_false = context(GITHUB_PR, &[("SEMOXIDE_CI_IS_PR", "fAlSe")]);

        assert_eq!(
            (mixed_true.pull_request(), mixed_false.pull_request()),
            (PR_WITHOUT_NUMBER, PullRequest::No)
        );
    }

    /// The invalid-value message for `value`, as written in it.
    fn invalid_is_pr(shown: &str) -> (ErrorCode, String, Option<String>) {
        (
            ENV_INVALID_VALUE,
            format!("the value of `SEMOXIDE_CI_IS_PR` is `{shown}`: expected `true` or `false`"),
            Some(String::from(
                "Set `SEMOXIDE_CI_IS_PR` to `true` or `false`.",
            )),
        )
    }

    #[rstest]
    #[case::one("1")]
    #[case::zero("0")]
    #[case::on("on")]
    #[case::prefix("t")]
    #[case::leading_space(" true")]
    #[case::trailing_space("true ")]
    #[case::capitalised_yes("Yes")]
    fn only_true_or_false_is_accepted_wherever_it_is_set(#[case] value: &str) {
        let on_a_pr = env(&vars(
            GITHUB_PR,
            &[("SEMOXIDE_CI_BRANCH", "x"), ("SEMOXIDE_CI_IS_PR", value)],
        ));
        let without_a_vendor = env(&[("PATH", "/bin"), ("SEMOXIDE_CI_IS_PR", value)]);

        assert_eq!(
            (failure(&on_a_pr), failure(&without_a_vendor)),
            (Some(invalid_is_pr(value)), Some(invalid_is_pr(value)))
        );
    }

    #[rstest]
    #[case::newline("tr\nue", "tr\\nue")]
    #[case::tab("a\tb", "a\\tb")]
    #[case::quote("it's", "it\\'s")]
    #[case::non_ascii("да", "да")]
    #[case::long(
        "this value is forty characters long, no",
        "this value is forty characters long, no"
    )]
    fn an_invalid_value_is_shown_whole_with_control_characters_escaped(
        #[case] value: &str,
        #[case] shown: &str,
    ) {
        let env = env(&[("CI", "true"), ("SEMOXIDE_CI_IS_PR", value)]);

        assert_eq!(failure(&env), Some(invalid_is_pr(shown)));
    }

    #[test]
    fn an_invalid_value_names_the_variable_as_set() {
        let env = Env::with_case(
            Case::Insensitive,
            [("CI", "true"), ("Semoxide_Ci_Is_Pr", "yes")],
        );

        assert_eq!(
            failure(&env).map(|(_, message, help)| (message, help)),
            Some((
                String::from(
                    "the value of `Semoxide_Ci_Is_Pr` is `yes`: expected `true` or `false`"
                ),
                Some(String::from(
                    "Set `Semoxide_Ci_Is_Pr` to `true` or `false`."
                ))
            ))
        );
    }

    // --- Both ---

    #[test]
    fn is_pr_that_isnt_utf_8_is_an_error_without_a_vendor() {
        let env = Env::with_case(
            Case::Sensitive,
            [
                (OsString::from("PATH"), OsString::from("/bin")),
                (OsString::from("SEMOXIDE_CI_IS_PR"), not_unicode()),
            ],
        );

        assert_eq!(
            failure(&env).map(|(code, message, _)| (code, message)),
            Some((
                ENV_NOT_UNICODE,
                String::from("the value of `SEMOXIDE_CI_IS_PR` isn't valid UTF-8")
            ))
        );
    }

    #[test]
    fn the_branch_override_is_read_first() {
        let with_is_pr = |is_pr: &str| {
            Env::with_case(
                Case::Sensitive,
                [
                    (OsString::from("CI"), OsString::from("true")),
                    (OsString::from("SEMOXIDE_CI_BRANCH"), not_unicode()),
                    (OsString::from("SEMOXIDE_CI_IS_PR"), OsString::from(is_pr)),
                ],
            )
        };
        let branch_error = Some((
            ENV_NOT_UNICODE,
            String::from("the value of `SEMOXIDE_CI_BRANCH` isn't valid UTF-8"),
        ));

        assert_eq!(
            (
                failure(&with_is_pr("true")).map(|(code, message, _)| (code, message)),
                failure(&with_is_pr("yes")).map(|(code, message, _)| (code, message))
            ),
            (branch_error.clone(), branch_error.clone())
        );
        let both_not_unicode = Env::with_case(
            Case::Sensitive,
            [
                (OsString::from("CI"), OsString::from("true")),
                (OsString::from("SEMOXIDE_CI_BRANCH"), not_unicode()),
                (OsString::from("SEMOXIDE_CI_IS_PR"), not_unicode()),
            ],
        );
        assert_eq!(
            failure(&both_not_unicode).map(|(code, message, _)| (code, message)),
            branch_error
        );
    }

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
