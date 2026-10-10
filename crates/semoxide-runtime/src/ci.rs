//! CI detection from the [`Env`] snapshot: the vendor, the branch to release from, the commit
//! and whether the run is for a pull or merge request (ARCHITECTURE, CI context). Never reads
//! files; differences from upstream's env-ci are listed in DIFFERENCES.

mod github;
mod gitlab;

use crate::{Env, EnvError};

/// The CI service a run is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Vendor {
    /// GitHub Actions
    GitHubActions,
    /// GitLab CI/CD
    GitLabCi,
}

/// Whether the run is for a pull or merge request, which never releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PullRequest {
    /// A branch or tag run.
    No,
    /// A pull or merge request run, with the number users see when the environment has it.
    Yes {
        /// The pull request number, or the merge request's IID.
        number: Option<u64>,
    },
}

/// What the environment says about the CI run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CiContext {
    vendor: Option<Vendor>,
    ci_variable: bool,
    branch: Option<String>,
    commit: Option<String>,
    pull_request: PullRequest,
}

impl CiContext {
    /// The detected CI service, if any.
    #[must_use]
    pub fn vendor(&self) -> Option<Vendor> {
        self.vendor
    }

    /// Whether the run is on CI: a known service, or `CI` set.
    #[must_use]
    pub fn is_ci(&self) -> bool {
        self.vendor.is_some() || self.ci_variable
    }

    /// The branch to release from; for a pull request, its target branch.
    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    /// The commit the run is for.
    #[must_use]
    pub fn commit(&self) -> Option<&str> {
        self.commit.as_deref()
    }

    /// Whether the run is for a pull or merge request.
    #[must_use]
    pub fn pull_request(&self) -> PullRequest {
        self.pull_request
    }
}

/// The CI context of a run, from its environment.
///
/// # Errors
///
/// Returns [`EnvError`] if a variable it reads isn't valid UTF-8.
pub fn detect(env: &Env) -> Result<CiContext, EnvError> {
    // In env-ci's order; the first match wins and the others' variables are never read.
    for (vendor, variable, read) in VENDORS {
        if value(env, variable)?.is_some() {
            let (branch, commit, pull_request) = read(env)?;
            return Ok(CiContext {
                vendor: Some(vendor),
                ci_variable: false,
                branch,
                commit,
                pull_request,
            });
        }
    }
    Ok(CiContext {
        vendor: None,
        ci_variable: value(env, "CI")?.is_some(),
        branch: None,
        commit: None,
        pull_request: PullRequest::No,
    })
}

/// What a vendor reads about a run: the branch, the commit and the pull request state.
type Run = (Option<String>, Option<String>, PullRequest);

/// How a vendor reads a run it detected.
type Read = fn(&Env) -> Result<Run, EnvError>;

/// Each vendor, the variable that detects it and how its runs are read.
const VENDORS: [(Vendor, &str, Read); 2] = [
    (Vendor::GitHubActions, "GITHUB_ACTIONS", github::read),
    (Vendor::GitLabCi, "GITLAB_CI", gitlab::read),
];

/// The value of `name`, an empty one counting as unset.
fn value<'a>(env: &'a Env, name: &str) -> Result<Option<&'a str>, EnvError> {
    Ok(env.var(name)?.filter(|text| !text.is_empty()))
}

/// A pull request number: plain ASCII digits.
fn number(text: &str) -> Option<u64> {
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Test helpers shared by the vendor modules.
#[cfg(test)]
mod test_support {
    use std::ffi::OsString;

    use super::{CiContext, PullRequest, detect};
    use crate::Env;
    use crate::env::Case;

    pub(super) fn env(vars: &[(&str, &str)]) -> Env {
        Env::with_case(Case::Sensitive, vars.iter().copied())
    }

    /// The context `vars` give, failing the test if detection fails.
    pub(super) fn detected(vars: &[(&str, &str)]) -> CiContext {
        let result = detect(&env(vars));
        assert!(result.is_ok(), "detection should succeed: {result:?}");
        result.unwrap()
    }

    /// The branch, commit and pull request state of `base` with `extra` set on top.
    pub(super) fn fields(
        base: &[(&str, &str)],
        extra: &[(&str, &str)],
    ) -> (Option<String>, Option<String>, PullRequest) {
        let vars: Vec<(&str, &str)> = base.iter().chain(extra).copied().collect();
        let context = detected(&vars);
        (
            context.branch().map(str::to_owned),
            context.commit().map(str::to_owned),
            context.pull_request(),
        )
    }

    pub(super) fn not_unicode() -> OsString {
        cfg_select! {
            unix => {
                use std::os::unix::ffi::OsStringExt;
                OsString::from_vec(vec![0xff])
            }
            windows => {
                use std::os::windows::ffi::OsStringExt;
                OsString::from_wide(&[0xD800])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use semoxide_error::{ErrorCode, ErrorInfo};

    use super::test_support::{detected, not_unicode};
    use super::*;
    use crate::codes::ENV_NOT_UNICODE;
    use crate::env::Case;

    #[test]
    fn ci_is_a_set_ci_variable_when_no_vendor_matches() {
        let outside = detected(&[("PATH", "/bin")]);
        let generic = detected(&[("CI", "true")]);

        assert_eq!(
            (
                (outside.is_ci(), outside.vendor()),
                (generic.is_ci(), generic.vendor())
            ),
            ((false, None), (true, None))
        );
    }

    #[test]
    fn a_detected_vendor_is_ci_without_the_ci_variable() {
        let context = detected(&[("GITLAB_CI", "true")]);

        assert_eq!(
            (context.is_ci(), context.vendor()),
            (true, Some(Vendor::GitLabCi))
        );
    }

    #[test]
    fn github_is_checked_before_gitlab() {
        let context = detected(&[
            ("GITHUB_ACTIONS", "true"),
            ("GITHUB_REF", "refs/heads/main"),
            ("GITLAB_CI", "true"),
            ("CI_COMMIT_BRANCH", "other"),
        ]);

        assert_eq!(
            (context.vendor(), context.branch()),
            (Some(Vendor::GitHubActions), Some("main"))
        );
    }

    #[test]
    fn an_empty_value_counts_as_unset() {
        let unset = detected(&[("GITHUB_ACTIONS", ""), ("GITLAB_CI", ""), ("CI", "")]);
        let github = detected(&[
            ("GITHUB_ACTIONS", "true"),
            ("GITHUB_SHA", ""),
            ("GITHUB_REF", ""),
        ]);

        assert_eq!(
            (
                (unset.vendor(), unset.is_ci()),
                (github.vendor(), github.commit(), github.branch())
            ),
            ((None, false), (Some(Vendor::GitHubActions), None, None))
        );
    }

    #[test]
    fn without_a_vendor_its_variables_are_ignored() {
        let context = detected(&[
            ("CI", "true"),
            ("GITHUB_SHA", "1234"),
            ("GITHUB_REF", "refs/heads/main"),
            ("CI_COMMIT_SHA", "5678"),
            ("CI_COMMIT_BRANCH", "main"),
        ]);

        assert_eq!(
            (
                context.is_ci(),
                context.vendor(),
                context.branch(),
                context.commit()
            ),
            (true, None, None, None)
        );
    }

    fn env_os(vars: &[(&str, OsString)]) -> Env {
        Env::with_case(
            Case::Sensitive,
            vars.iter()
                .map(|(name, value)| (OsString::from(name), value.clone())),
        )
    }

    /// The code and message of the error detection fails with, if it does.
    fn failure(env: &Env) -> Option<(ErrorCode, String)> {
        detect(env)
            .err()
            .map(|error| (error.code(), error.to_string()))
    }

    #[test]
    fn variables_that_arent_read_may_be_anything() {
        let env = env_os(&[
            ("GITHUB_ACTIONS", OsString::from("true")),
            ("GITHUB_REF", OsString::from("refs/heads/main")),
            ("GITHUB_BASE_REF", not_unicode()),
            ("CI", not_unicode()),
            ("GITLAB_CI", not_unicode()),
            ("CI_COMMIT_BRANCH", not_unicode()),
            ("HOME", not_unicode()),
        ]);

        let result = detect(&env);

        assert_eq!(
            result.map(|context| (context.vendor(), context.branch().map(str::to_owned))),
            Ok((Some(Vendor::GitHubActions), Some(String::from("main"))))
        );
    }

    #[test]
    fn the_merge_request_iid_isnt_read_on_an_external_pull_request() {
        let env = env_os(&[
            ("GITLAB_CI", OsString::from("true")),
            (
                "CI_PIPELINE_SOURCE",
                OsString::from("external_pull_request_event"),
            ),
            ("CI_EXTERNAL_PULL_REQUEST_IID", OsString::from("7")),
            (
                "CI_EXTERNAL_PULL_REQUEST_TARGET_BRANCH_NAME",
                OsString::from("main"),
            ),
            ("CI_MERGE_REQUEST_IID", not_unicode()),
        ]);

        let result = detect(&env);

        assert_eq!(
            result.map(|context| (context.branch().map(str::to_owned), context.pull_request())),
            Ok((
                Some(String::from("main")),
                PullRequest::Yes { number: Some(7) }
            ))
        );
    }

    #[test]
    fn a_variable_that_isnt_utf_8_is_an_error_when_read() {
        let no_vendor = env_os(&[("CI", not_unicode())]);
        let gitlab = env_os(&[
            ("GITLAB_CI", OsString::from("true")),
            ("CI_PIPELINE_SOURCE", OsString::from("merge_request_event")),
            ("CI_MERGE_REQUEST_IID", not_unicode()),
        ]);

        assert_eq!(
            (failure(&no_vendor), failure(&gitlab)),
            (
                Some((
                    ENV_NOT_UNICODE,
                    String::from("the value of `CI` isn't valid UTF-8")
                )),
                Some((
                    ENV_NOT_UNICODE,
                    String::from("the value of `CI_MERGE_REQUEST_IID` isn't valid UTF-8")
                ))
            )
        );
    }

    #[test]
    fn a_variable_that_isnt_utf_8_is_an_error() {
        let env = Env::with_case(
            Case::Sensitive,
            [
                (OsString::from("GITHUB_ACTIONS"), OsString::from("true")),
                (OsString::from("GITHUB_REF"), not_unicode()),
            ],
        );

        let result = detect(&env);

        assert_eq!(
            result.err().map(|error| (error.code(), error.to_string())),
            Some((
                ENV_NOT_UNICODE,
                String::from("the value of `GITHUB_REF` isn't valid UTF-8")
            ))
        );
    }
}
