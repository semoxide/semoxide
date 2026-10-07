use std::fmt;

use semoxide::{ErrorCode, ErrorInfo};

use super::{Colors, TextStyle, json, text};

const CORE_NO_GIT_REPO: ErrorCode = ErrorCode::from_static("core::no_git_repo");
const GIT_PUSH_TIMED_OUT: ErrorCode = ErrorCode::from_static("git::push_timed_out");

/// An error with a help line, like most real ones.
#[derive(Debug)]
struct NoGitRepo;

impl fmt::Display for NoGitRepo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a git repository")
    }
}

impl std::error::Error for NoGitRepo {}

impl ErrorInfo for NoGitRepo {
    fn code(&self) -> ErrorCode {
        CORE_NO_GIT_REPO
    }

    fn help(&self) -> Option<String> {
        Some(String::from(
            "Run semoxide inside a git repository, or pass `--cwd`.",
        ))
    }
}

/// An error without a help line, retryable, after a remote write.
#[derive(Debug)]
struct PushTimedOut;

impl fmt::Display for PushTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("push of `v1.2.0` to `origin` timed out")
    }
}

impl std::error::Error for PushTimedOut {}

impl ErrorInfo for PushTimedOut {
    fn code(&self) -> ErrorCode {
        GIT_PUSH_TIMED_OUT
    }

    fn retryable(&self) -> bool {
        true
    }

    fn remote_writes_happened(&self) -> bool {
        true
    }
}

#[test]
fn plain_text_with_help() {
    insta::assert_snapshot!(text(&NoGitRepo, TextStyle::Plain));
}

#[test]
fn plain_text_without_help() {
    insta::assert_snapshot!(text(&PushTimedOut, TextStyle::Plain));
}

#[test]
fn graphical_text_with_help() {
    insta::assert_snapshot!(text(
        &NoGitRepo,
        TextStyle::Graphical {
            colors: Colors::Off
        }
    ));
}

#[test]
fn graphical_text_without_help() {
    insta::assert_snapshot!(text(
        &PushTimedOut,
        TextStyle::Graphical {
            colors: Colors::Off
        }
    ));
}

#[test]
fn json_with_help() {
    insta::assert_snapshot!(serde_json::to_string_pretty(&json(&NoGitRepo)).unwrap());
}

#[test]
fn json_without_help_omits_the_field() {
    insta::assert_snapshot!(serde_json::to_string_pretty(&json(&PushTimedOut)).unwrap());
}

#[test]
fn graphical_text_with_colors_contains_color_codes() {
    let colored = text(&NoGitRepo, TextStyle::Graphical { colors: Colors::On });

    assert!(colored.contains('\u{1b}'), "{colored:?}");
}
