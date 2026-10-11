//! How a crate defines an error with `ErrorInfo`: a `const` code and only the methods it needs.

use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};

const GIT_NO_REPOSITORY: ErrorCode = ErrorCode::from_static("git::no_repository");
const GIT_PUSH_TIMED_OUT: ErrorCode = ErrorCode::from_static("git::push_timed_out");

#[derive(Debug)]
struct NoRepository;

impl fmt::Display for NoRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a git repository")
    }
}

impl std::error::Error for NoRepository {}

impl ErrorInfo for NoRepository {
    fn code(&self) -> ErrorCode {
        GIT_NO_REPOSITORY
    }
}

#[derive(Debug)]
struct PushTimedOut;

impl fmt::Display for PushTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("push timed out")
    }
}

impl std::error::Error for PushTimedOut {}

impl ErrorInfo for PushTimedOut {
    fn code(&self) -> ErrorCode {
        GIT_PUSH_TIMED_OUT
    }

    fn help(&self) -> Option<String> {
        Some(String::from("check the network, then rerun"))
    }

    fn retryable(&self) -> bool {
        true
    }
}

#[test]
fn defaults_need_only_a_code() {
    let error = NoRepository;

    assert_eq!(error.code().as_str(), "git::no_repository");
    assert_eq!(error.help(), None);
    assert!(!error.retryable());
    assert!(!error.remote_writes_happened());
}

#[test]
fn url_points_to_the_docs_page_named_by_the_slug() {
    assert_eq!(
        NoRepository.url(),
        "https://github.com/semoxide/semoxide/blob/main/docs/errors/git/no-repository.md"
    );
}

#[test]
fn overridden_methods_win() {
    let error = PushTimedOut;

    assert_eq!(
        error.help().as_deref(),
        Some("check the network, then rerun")
    );
    assert!(error.retryable());
    assert!(!error.remote_writes_happened());
}

#[test]
fn static_code_equals_the_parsed_code() {
    let parsed: ErrorCode = "git::no_repository".parse().unwrap();

    assert_eq!(GIT_NO_REPOSITORY, parsed);
}

#[test]
#[should_panic(expected = "invalid error code")]
fn invalid_static_code_panics() {
    let _ = ErrorCode::from_static("Core::bad");
}
