//! How the CLI shows an error: miette in a terminal, plain text in CI, a JSON object with
//! `--output=json` (OBSERVABILITY §8, CLI.md JSON contract).

use std::fmt;

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme};
use semoxide::ErrorInfo;
use serde::Serialize;

#[cfg(test)]
mod tests;

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
