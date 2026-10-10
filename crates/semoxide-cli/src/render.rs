//! How the CLI shows an error: miette in a terminal, plain text in CI, a JSON object with
//! `--output=json` (OBSERVABILITY §8, CLI.md JSON contract).

use std::fmt;

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme};
use semoxide::ErrorInfo;
use serde::Serialize;

/// Version of the JSON output format (CLI.md: fields are only added within a version).
const SCHEMA_VERSION: u32 = 1;

/// How text output is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextStyle {
    /// miette's graphical report, for a terminal.
    Graphical { colors: Colors },
    /// One `error[code]: message` line plus indented help and docs lines, for CI logs.
    Plain,
}

/// Whether graphical output uses colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Colors {
    On,
    Off,
}

/// The error as text for stderr.
pub(crate) fn text(error: &dyn ErrorInfo, style: TextStyle) -> String {
    match style {
        TextStyle::Plain => plain(error),
        TextStyle::Graphical { colors } => graphical(error, colors),
    }
}

fn plain(error: &dyn ErrorInfo) -> String {
    let help = error
        .help()
        .map(|help| format!("  help: {help}\n"))
        .unwrap_or_default();
    format!(
        "error[{}]: {error}\n{help}  docs: {}\n",
        error.code(),
        error.url()
    )
}

fn graphical(error: &dyn ErrorInfo, colors: Colors) -> String {
    let theme = match colors {
        Colors::On => GraphicalTheme::unicode(),
        Colors::Off => GraphicalTheme::unicode_nocolor(),
    };
    let mut text = String::new();
    // Writing into a String can't fail.
    let _ =
        GraphicalReportHandler::new_themed(theme).render_report(&mut text, &AsDiagnostic(error));
    text
}

/// `ErrorInfo` seen through miette's `Diagnostic` trait; miette stays inside the CLI.
struct AsDiagnostic<'a>(&'a dyn ErrorInfo);

impl fmt::Debug for AsDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0, f)
    }
}

impl fmt::Display for AsDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0, f)
    }
}

impl std::error::Error for AsDiagnostic<'_> {}

impl Diagnostic for AsDiagnostic<'_> {
    fn code<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        Some(Box::new(self.0.code()))
    }

    fn help<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        self.0
            .help()
            .map(|help| Box::new(help) as Box<dyn fmt::Display>)
    }

    fn url<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        Some(Box::new(self.0.url()))
    }
}

/// The `--output=json` document for an error.
#[derive(Debug, Serialize)]
pub(crate) struct JsonError {
    schema_version: u32,
    error: JsonErrorObject,
}

#[derive(Debug, Serialize)]
struct JsonErrorObject {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<String>,
    url: String,
    retryable: bool,
    remote_writes_happened: bool,
}

/// The error as the JSON document printed on stdout with `--output=json`.
pub(crate) fn json(error: &dyn ErrorInfo) -> JsonError {
    JsonError {
        schema_version: SCHEMA_VERSION,
        error: JsonErrorObject {
            code: error.code().to_string(),
            message: error.to_string(),
            help: error.help(),
            url: error.url(),
            retryable: error.retryable(),
            remote_writes_happened: error.remote_writes_happened(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::path::PathBuf;

    use semoxide::{ErrorCode, ErrorInfo, FileLocation, FlagLocation, Location};
    use serde_json::json;

    use super::*;

    const CORE_NO_GIT_REPO: ErrorCode = ErrorCode::from_static("core::no_git_repo");
    const GIT_PUSH_TIMED_OUT: ErrorCode = ErrorCode::from_static("git::push_timed_out");
    const CONFIG_INVALID_VALUE: ErrorCode = ErrorCode::from_static("config::invalid_value");

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

    /// A config error pointing at the value on line 2 of `semoxide.toml`.
    #[derive(Debug)]
    struct InvalidTagFormat;

    impl fmt::Display for InvalidTagFormat {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("`tags.format` = \"release\": must contain `{version}`")
        }
    }

    impl std::error::Error for InvalidTagFormat {}

    impl ErrorInfo for InvalidTagFormat {
        fn code(&self) -> ErrorCode {
            CONFIG_INVALID_VALUE
        }

        fn location(&self) -> Option<Location> {
            Some(Location::File(FileLocation::at(
                PathBuf::from("semoxide.toml"),
                String::from("[tags]\nformat = \"release\"\n"),
                16..25,
            )))
        }
    }

    /// A config error pointing at the second `--set` flag.
    #[derive(Debug)]
    struct InvalidFlagValue;

    impl fmt::Display for InvalidFlagValue {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("`tags.format` = \"release\": must contain `{version}`")
        }
    }

    impl std::error::Error for InvalidFlagValue {}

    impl ErrorInfo for InvalidFlagValue {
        fn code(&self) -> ErrorCode {
            CONFIG_INVALID_VALUE
        }

        fn location(&self) -> Option<Location> {
            Some(Location::Flag(FlagLocation::new(
                2,
                String::from("--set tags.format=release"),
            )))
        }
    }

    /// An error pointing at a whole file, with no line.
    #[derive(Debug)]
    struct Unreadable;

    impl fmt::Display for Unreadable {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("`semoxide.toml`: Access is denied.")
        }
    }

    impl std::error::Error for Unreadable {}

    impl ErrorInfo for Unreadable {
        fn code(&self) -> ErrorCode {
            ErrorCode::from_static("config::unreadable")
        }

        fn location(&self) -> Option<Location> {
            Some(Location::File(FileLocation::new(PathBuf::from(
                "semoxide.toml",
            ))))
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

    // --- Locations ---

    fn graphical_no_colors(error: &dyn ErrorInfo) -> String {
        text(
            error,
            TextStyle::Graphical {
                colors: Colors::Off,
            },
        )
    }

    #[test]
    fn plain_text_points_at_the_line() {
        let plain = text(&InvalidTagFormat, TextStyle::Plain);

        assert!(plain.contains("\n  --> semoxide.toml:2:10\n"), "{plain}");
        insta::assert_snapshot!(plain);
    }

    #[test]
    fn plain_text_points_at_the_flag() {
        let plain = text(&InvalidFlagValue, TextStyle::Plain);

        assert!(
            plain.contains("\n  --> flag #2: --set tags.format=release\n"),
            "{plain}"
        );
        insta::assert_snapshot!(plain);
    }

    #[test]
    fn plain_text_points_at_the_whole_file() {
        let plain = text(&Unreadable, TextStyle::Plain);

        assert!(plain.contains("\n  --> semoxide.toml\n"), "{plain}");
        insta::assert_snapshot!(plain);
    }

    #[test]
    fn graphical_text_shows_the_line() {
        let graphical = graphical_no_colors(&InvalidTagFormat);

        assert!(graphical.contains("format = \"release\""), "{graphical}");
        insta::assert_snapshot!(graphical);
    }

    #[test]
    fn graphical_text_shows_the_flag() {
        let graphical = graphical_no_colors(&InvalidFlagValue);

        assert!(
            graphical.contains("--set tags.format=release"),
            "{graphical}"
        );
        insta::assert_snapshot!(graphical);
    }

    /// The `location` field of the JSON error object.
    fn json_location(error: &dyn ErrorInfo) -> serde_json::Value {
        serde_json::to_value(json(error)).unwrap()["error"]["location"].clone()
    }

    #[test]
    fn json_file_location() {
        assert_eq!(
            json_location(&InvalidTagFormat),
            json!({ "file": "semoxide.toml", "line": 2, "column": 10 })
        );
    }

    #[test]
    fn json_flag_location() {
        assert_eq!(
            json_location(&InvalidFlagValue),
            json!({ "flag": "--set tags.format=release", "position": 2 })
        );
    }

    #[test]
    fn json_whole_file_location() {
        assert_eq!(
            json_location(&Unreadable),
            json!({ "file": "semoxide.toml" })
        );
    }

    #[test]
    fn json_without_a_location_omits_the_field() {
        let value = serde_json::to_value(json(&InvalidTagFormat)).unwrap();
        let without = serde_json::to_value(json(&NoGitRepo)).unwrap();

        assert!(value["error"].get("location").is_some(), "{value}");
        assert!(without["error"].get("location").is_none(), "{without}");
    }
}
