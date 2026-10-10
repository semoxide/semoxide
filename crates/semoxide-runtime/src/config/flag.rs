//! `--set <key>=<value>`: a dotted TOML key without array indexes, and a TOML value or a plain
//! string (CLI.md).

use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};
use toml::{Table, Value};

use super::merge::{Layer, Source};
use crate::codes::CONFIG_INVALID_FLAG;

/// Reads the `position`th `--set` flag (counting from 1) as a one-key layer.
///
/// # Errors
///
/// Returns [`FlagError`] if the flag isn't `<key>=<value>` as CLI.md describes.
pub fn parse_flag(position: usize, flag: &str) -> Result<Layer, FlagError> {
    let error = |problem: &str| FlagError {
        position,
        flag: flag.to_owned(),
        problem: problem.to_owned(),
    };
    let (key, value) = split_at_equals(flag).ok_or_else(|| error("expected `<key>=<value>`"))?;
    let path = key_path(key).map_err(|problem| error(&problem))?;
    let value = parse_value(value).map_err(|problem| error(&problem))?;
    let table = path.iter().rev().fold(value, |inner, part| {
        Value::Table(Table::from_iter([(part.clone(), inner)]))
    });
    match table {
        Value::Table(table) => Ok(Layer::new(Source::Flag(position), table)),
        _ => Err(error("expected `<key>=<value>`")),
    }
}

/// The key and value around the first `=` outside quotes.
fn split_at_equals(flag: &str) -> Option<(&str, &str)> {
    let mut quote = None;
    for (index, character) in flag.char_indices() {
        match (quote, character) {
            (None, '"' | '\'') => quote = Some(character),
            (Some(open), _) if character == open => quote = None,
            (None, '=') => return Some((flag.get(..index)?, flag.get(index + 1..)?)),
            _ => {}
        }
    }
    None
}

/// The parts of a dotted TOML key; array indexes are rejected because arrays are set whole.
fn key_path(key: &str) -> Result<Vec<String>, String> {
    if key.trim().is_empty() {
        return Err(String::from("the key is empty"));
    }
    if key.contains(['[', ']']) {
        return Err(String::from(
            "array indexes aren't allowed; set the whole array",
        ));
    }
    let parsed: Table = format!("{key} = 0")
        .parse()
        .map_err(|_| format!("`{}` isn't a dotted TOML key", key.trim()))?;
    let mut path = Vec::new();
    let mut current = &parsed;
    while let Some((name, value)) = current.iter().next() {
        path.push(name.clone());
        match value {
            Value::Table(inner) => current = inner,
            _ => break,
        }
    }
    Ok(path)
}

/// A TOML value, or the text as a string when it isn't TOML and doesn't start like a TOML
/// string, array or table.
fn parse_value(value: &str) -> Result<Value, String> {
    if let Ok(mut parsed) = format!("v = {value}").parse::<Table>()
        && parsed.len() == 1
        && let Some(parsed_value) = parsed.remove("v")
    {
        return Ok(parsed_value);
    }
    if value.trim_start().starts_with(['"', '\'', '[', '{']) {
        return Err(format!("`{value}` isn't a valid TOML value"));
    }
    Ok(Value::String(value.to_owned()))
}

/// A `--set` flag that can't be read as `<key>=<value>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagError {
    position: usize,
    flag: String,
    problem: String,
}

impl FlagError {
    /// Which `--set` flag it is, counting from 1.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }
}

impl fmt::Display for FlagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`--set {}`: {}", self.flag, self.problem)
    }
}

impl std::error::Error for FlagError {}

impl ErrorInfo for FlagError {
    fn code(&self) -> ErrorCode {
        CONFIG_INVALID_FLAG
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semoxide_error::{ErrorInfo, FlagLocation, Location};
    use toml::Table;

    use super::*;
    use crate::codes::CONFIG_INVALID_FLAG;
    use crate::config::Source;

    /// The flag's layer table, failing the test with an assertion if it doesn't parse.
    fn parsed(position: usize, flag: &str) -> (Source, Table) {
        let result = parse_flag(position, flag);
        assert!(result.is_ok(), "`{flag}` should parse: {result:?}");
        let layer = result.unwrap();
        (layer.source().clone(), layer.table().clone())
    }

    fn expected(text: &str) -> Table {
        text.parse().unwrap()
    }

    #[rstest]
    #[case::plain_string("tags.format=v{version}", r#"tags.format = "v{version}""#)]
    #[case::version_string("version.initial=0.1.0", r#"version.initial = "0.1.0""#)]
    #[case::word("steps.success.errors=fail", r#"steps.success.errors = "fail""#)]
    #[case::empty_value("tags.format=", r#"tags.format = """#)]
    #[case::boolean("plugins.github.draft=true", "plugins.github.draft = true")]
    #[case::integer("plugins.npm.tag=1", "plugins.npm.tag = 1")]
    #[case::quoted_forces_a_string(r#"plugins.npm.tag="1""#, r#"plugins.npm.tag = "1""#)]
    #[case::array(
        r#"steps.plugins=["git", "github"]"#,
        r#"steps.plugins = ["git", "github"]"#
    )]
    #[case::inline_table(
        "plugins.github.draft={ enabled = true }",
        "plugins.github.draft = { enabled = true }"
    )]
    #[case::dashed_key(
        "plugins.release-notes.locale=sv",
        r#"plugins.release-notes.locale = "sv""#
    )]
    #[case::quoted_key_part(r#"plugins."my.plugin".x=1"#, r#"plugins."my.plugin".x = 1"#)]
    #[case::equals_in_the_value("tags.format=a={version}", r#"tags.format = "a={version}""#)]
    fn valid_flag(#[case] flag: &str, #[case] table: &str) {
        assert_eq!(parsed(1, flag), (Source::Flag(1), expected(table)));
    }

    #[test]
    fn the_flags_position_is_its_source() {
        assert_eq!(parsed(3, "tags.format=v{version}").0, Source::Flag(3));
    }

    #[rstest]
    #[case::no_equals("tags.format")]
    #[case::empty_key("=v{version}")]
    #[case::empty_key_part("tags..format=x")]
    #[case::array_index("branches.rules[0].channel=next")]
    #[case::broken_array(r#"steps.plugins=["git""#)]
    #[case::unterminated_string(r#"tags.format="v{version}"#)]
    #[case::broken_inline_table("plugins.github.draft={ enabled = true")]
    #[case::single_quote_start("tags.format='v{version}")]
    fn invalid_flag_is_rejected_with_its_position(#[case] flag: &str) {
        let result = parse_flag(2, flag);

        assert_eq!(
            result
                .map(|_| ())
                .map_err(|error| (error.code(), error.position())),
            Err((CONFIG_INVALID_FLAG, 2)),
            "{flag}"
        );
    }

    #[test]
    fn error_message_quotes_the_flag() {
        let result = parse_flag(1, "tags.format");

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("`--set tags.format`"), "{message}");
        assert!(message.contains("expected `<key>=<value>`"), "{message}");
    }

    #[test]
    fn the_error_points_at_the_flag() {
        let result = parse_flag(2, "tags.format");

        assert_eq!(
            result.err().and_then(|error| error.location()),
            Some(Location::Flag(FlagLocation::new(
                2,
                String::from("--set tags.format")
            )))
        );
    }

    #[test]
    fn equals_inside_a_quoted_key_part_is_part_of_the_key() {
        let layer = parse_flag(1, r#"plugins."a=b".x=1"#);

        assert!(layer.is_ok(), "{layer:?}");
        let expected: Table = r#"plugins."a=b".x = 1"#.parse().unwrap();
        assert_eq!(layer.unwrap().table(), &expected);
    }
}
