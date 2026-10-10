//! Where a config key path came from: the spot in the config file, or the `--set` flag that set
//! it (OBSERVABILITY §8).

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::PathBuf;

use semoxide_error::{ErrorInfo, FileLocation, Location};
use semoxide_schema::codes::CONFIG_INVALID_VALUE;
use semoxide_schema::config::ConfigError;
use toml::de::{DeTable, DeValue};

use super::flag::flag_location;
use super::merge::{Merged, Source};

/// A config file and the text it was parsed from.
#[derive(Debug, Clone)]
pub(super) struct SourceFile {
    path: PathBuf,
    text: String,
}

impl SourceFile {
    pub(super) fn new(path: PathBuf, text: String) -> Self {
        Self { path, text }
    }
}

/// Where `error` points: at the value for an invalid value, at the key otherwise; in the file or
/// the flag its key path came from. `None` for a value from the defaults.
pub(super) fn locate(
    error: &ConfigError,
    merged: &Merged,
    file: Option<&SourceFile>,
    flags: &[String],
) -> Option<Location> {
    match source(merged.sources(), error.path())? {
        Source::Default => None,
        Source::Flag(position) => {
            let flag = flags.get(position.checked_sub(1)?)?;
            Some(flag_location(*position, flag))
        }
        Source::File(path) => {
            let file = file.filter(|file| file.path == *path)?;
            let point = if error.code() == CONFIG_INVALID_VALUE {
                Point::Value
            } else {
                Point::Key
            };
            Some(Location::File(
                match span(&file.text, error.path(), point) {
                    Some(span) => FileLocation::at(file.path.clone(), file.text.clone(), span),
                    None => FileLocation::new(file.path.clone()),
                },
            ))
        }
    }
}

/// The source of `path`, of its nearest ancestor (`branches.rules` for
/// `branches.rules[0].name`: an array is one leaf), or of a key under it (`packages.a` for
/// `packages`).
fn source<'a>(sources: &'a BTreeMap<String, Source>, path: &str) -> Option<&'a Source> {
    let mut current = path;
    loop {
        if let Some(source) = sources.get(current) {
            return Some(source);
        }
        let Some(end) = current.rfind(['.', '[']) else {
            break;
        };
        current = current.get(..end)?;
    }
    sources
        .iter()
        .find(|(key, _)| {
            key.strip_prefix(path)
                .is_some_and(|rest| rest.starts_with(['.', '[']))
        })
        .map(|(_, source)| source)
}

/// Which part of a `key = value` pair an error points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Point {
    Key,
    Value,
}

/// The byte range of the key path in `text`, or of the deepest part of it that exists. An
/// array item has no key, so it points at the item.
fn span(text: &str, path: &str, point: Point) -> Option<Range<usize>> {
    let document = DeTable::parse(text).ok()?;
    let mut table = document.get_ref();
    let mut rest = path;
    let mut found = None;
    // The longest key that matches, so a key named `a.b` wins over `a` for `a.b.c`.
    while let Some((key, value)) = table
        .iter()
        .filter(|(key, _)| starts_with_part(rest, key.get_ref()))
        .max_by_key(|(key, _)| key.get_ref().len())
    {
        rest = rest.get(key.get_ref().len()..)?;
        found = Some(match point {
            Point::Key => key.span(),
            Point::Value => value.span(),
        });
        let mut value = value;
        while let DeValue::Array(items) = value.get_ref()
            && let Some((position, after)) = array_index(rest)
            && let Some(item) = items.get(position)
        {
            rest = after;
            found = Some(item.span());
            value = item;
        }
        let DeValue::Table(inner) = value.get_ref() else {
            break;
        };
        let Some(after) = rest.strip_prefix('.') else {
            break;
        };
        table = inner;
        rest = after;
    }
    found
}

/// Whether `path` starts with the key part `name`, followed by its end, `.` or `[`.
fn starts_with_part(path: &str, name: &str) -> bool {
    path.strip_prefix(name)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', '[']))
}

/// The leading `[n]` of `path` and what follows it.
fn array_index(path: &str) -> Option<(usize, &str)> {
    let (index, after) = path.strip_prefix('[')?.split_once(']')?;
    Some((index.parse().ok()?, after))
}
