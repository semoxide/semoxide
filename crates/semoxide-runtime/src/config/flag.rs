//! `--set <key>=<value>`: a dotted TOML key without array indexes, and a TOML value or a plain
//! string (CLI.md).

use std::fmt;

use semoxide_error::{ErrorCode, ErrorInfo};
use toml::{Table, Value};

use super::merge::{Layer, Source};
use crate::codes::CONFIG_INVALID_FLAG;

#[cfg(test)]
mod tests;

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
