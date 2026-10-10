//! [`Location`]: where an error's cause is, for a pointer in the error output (OBSERVABILITY §8).

use std::ops::Range;
use std::path::{Path, PathBuf};

/// Where the cause of an error is: a spot in a file, or a command-line flag.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Location {
    /// A file, with the spot in it when known.
    File(FileLocation),
    /// A command-line flag, e.g. the second `--set`.
    Flag(FlagLocation),
}

/// A file and, when known, a byte range in its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLocation {
    path: PathBuf,
    spot: Option<Spot>,
}

/// The file's text and a byte range in it, on character boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spot {
    text: String,
    span: Range<usize>,
}

impl FileLocation {
    /// The whole file, with no spot in it.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path, spot: None }
    }

    /// The byte range `span` in the file's `text`; a range outside the text or between the
    /// bytes of one character is narrowed to fit.
    #[must_use]
    pub fn at(path: PathBuf, _text: String, _span: Range<usize>) -> Self {
        Self { path, spot: None }
    }

    /// The file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's text, when there is a spot in it.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.spot.as_ref().map(|spot| spot.text.as_str())
    }

    /// The byte range in [`text`](Self::text).
    #[must_use]
    pub fn span(&self) -> Option<Range<usize>> {
        self.spot.as_ref().map(|spot| spot.span.clone())
    }

    /// The line where the span starts, counting from 1.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        None
    }

    /// The column where the span starts, in characters, counting from 1.
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        None
    }
}

/// A command-line flag: its position among flags of its kind, counting from 1, and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagLocation {
    position: usize,
    flag: String,
}

impl FlagLocation {
    /// The `position`th flag, e.g. `--set tags.format=release` given second.
    #[must_use]
    pub fn new(position: usize, flag: String) -> Self {
        Self { position, flag }
    }

    /// Its position, counting from 1.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// Its text, e.g. `--set tags.format=release`.
    #[must_use]
    pub fn flag(&self) -> &str {
        &self.flag
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn at(text: &str, span: Range<usize>) -> FileLocation {
        FileLocation::at(PathBuf::from("semoxide.toml"), text.to_owned(), span)
    }

    #[rstest]
    #[case::start_of_the_text("format = 1", 0..6, (1, 1))]
    #[case::inside_the_first_line("tags.format = 1", 5..11, (1, 6))]
    #[case::second_line("[tags]\nformat = \"release\"", 16..25, (2, 10))]
    #[case::after_crlf("[tags]\r\nformat = 1", 8..14, (2, 1))]
    #[case::empty_span_at_the_end("tags.format = \"v", 16..16, (1, 17))]
    #[case::characters_not_bytes("name = \"ü\" x", 12..13, (1, 12))]
    fn line_and_column_of_the_span_start(
        #[case] text: &str,
        #[case] span: Range<usize>,
        #[case] expected: (usize, usize),
    ) {
        let location = at(text, span);

        assert_eq!(
            (location.line(), location.column()),
            (Some(expected.0), Some(expected.1))
        );
    }

    #[test]
    fn the_spot_keeps_the_text_and_the_span() {
        let location = at("[tags]\nformat = 1", 7..13);

        assert_eq!(location.text(), Some("[tags]\nformat = 1"));
        assert_eq!(location.span(), Some(7..13));
    }

    #[rstest]
    #[case::past_the_end("a = 1", 3..40, 3..5)]
    #[case::entirely_past_the_end("a = 1", 9..12, 5..5)]
    #[case::reversed("a = 1", Range { start: 4, end: 2 }, 4..4)]
    #[case::inside_a_character("ü = 1", 1..3, 0..3)]
    fn a_span_that_does_not_fit_is_narrowed(
        #[case] text: &str,
        #[case] span: Range<usize>,
        #[case] fitted: Range<usize>,
    ) {
        assert_eq!(at(text, span).span(), Some(fitted));
    }
}
