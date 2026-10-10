//! How the CLI shows an error: miette in a terminal, plain text in CI, a JSON object with
//! `--output=json` (OBSERVABILITY §8, CLI.md JSON contract).

use std::fmt;

use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, SourceCode,
    SourceSpan,
};
use semoxide::{ErrorInfo, Location};
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
    let location = error
        .location()
        .and_then(|location| pointer(&location))
        .map(|pointer| format!("  --> {pointer}\n"))
        .unwrap_or_default();
    let help = error
        .help()
        .map(|help| format!("  help: {help}\n"))
        .unwrap_or_default();
    format!(
        "error[{}]: {error}\n{location}{help}  docs: {}\n",
        error.code(),
        error.url()
    )
}

/// The location as one line: `semoxide.toml:2:10`, `semoxide.toml` or `flag #2: --set …`.
fn pointer(location: &Location) -> Option<String> {
    match location {
        Location::File(file) => Some(match (file.line(), file.column()) {
            (Some(line), Some(column)) => format!("{}:{line}:{column}", file.path().display()),
            _ => file.path().display().to_string(),
        }),
        Location::Flag(flag) => Some(format!("flag #{}: {}", flag.position(), flag.flag())),
        _ => None,
    }
}

fn graphical(error: &dyn ErrorInfo, colors: Colors) -> String {
    let theme = match colors {
        Colors::On => GraphicalTheme::unicode(),
        Colors::Off => GraphicalTheme::unicode_nocolor(),
    };
    let mut text = String::new();
    // Writing into a String can't fail.
    let _ = GraphicalReportHandler::new_themed(theme)
        .render_report(&mut text, &AsDiagnostic::new(error));
    text
}

/// `ErrorInfo` seen through miette's `Diagnostic` trait, with the location as a code snippet;
/// miette stays inside the CLI.
#[derive(Debug)]
struct AsDiagnostic<'a> {
    error: &'a dyn ErrorInfo,
    snippet: Option<(NamedSource<String>, SourceSpan)>,
}

impl<'a> AsDiagnostic<'a> {
    fn new(error: &'a dyn ErrorInfo) -> Self {
        let snippet = error.location().and_then(|location| snippet(&location));
        Self { error, snippet }
    }
}

/// The text to show and the part to underline: the line in a file, or the whole flag.
fn snippet(location: &Location) -> Option<(NamedSource<String>, SourceSpan)> {
    match location {
        Location::File(file) => {
            let (text, span) = file.text().zip(file.span())?;
            let name = file.path().display().to_string();
            Some((NamedSource::new(name, text.to_owned()), span.into()))
        }
        Location::Flag(flag) => {
            let name = format!("flag #{}", flag.position());
            let span = (0..flag.flag().len()).into();
            Some((NamedSource::new(name, flag.flag().to_owned()), span))
        }
        _ => None,
    }
}

impl fmt::Display for AsDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.error, f)
    }
}

impl std::error::Error for AsDiagnostic<'_> {}

impl Diagnostic for AsDiagnostic<'_> {
    fn code<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        Some(Box::new(self.error.code()))
    }

    fn help<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        self.error
            .help()
            .map(|help| Box::new(help) as Box<dyn fmt::Display>)
    }

    fn url<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        Some(Box::new(self.error.url()))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.snippet
            .as_ref()
            .map(|(source, _)| source as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let (_, span) = self.snippet.as_ref()?;
        Some(Box::new(std::iter::once(LabeledSpan::underline(*span))))
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
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<JsonLocation>,
}

/// `{"file", "line", "column"}` (no line and column for a whole file) or `{"flag", "position"}`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum JsonLocation {
    File {
        file: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        column: Option<usize>,
    },
    Flag {
        flag: String,
        position: usize,
    },
}

impl JsonLocation {
    fn new(location: &Location) -> Option<Self> {
        match location {
            Location::File(file) => Some(Self::File {
                file: file.path().display().to_string(),
                line: file.line(),
                column: file.column(),
            }),
            Location::Flag(flag) => Some(Self::Flag {
                flag: flag.flag().to_owned(),
                position: flag.position(),
            }),
            _ => None,
        }
    }
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
            location: error
                .location()
                .and_then(|location| JsonLocation::new(&location)),
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
