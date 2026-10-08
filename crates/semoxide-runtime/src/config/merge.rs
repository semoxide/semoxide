//! The layer merge: defaults always fill key by key underneath; user layers merge deep, or
//! domain by domain with `config.merge = "shallow"`; `--set` flags override single keys on top.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use semoxide_schema::config::MergeMode;
use toml::{Table, Value};

#[cfg(test)]
mod tests;

/// Where a config value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The built-in defaults.
    Default,
    /// A config file, e.g. `semoxide.toml`.
    File(PathBuf),
    /// The `n`th `--set` flag, counting from 1.
    Flag(usize),
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::File(path) => write!(f, "file {}", path.display()),
            Self::Flag(position) => write!(f, "--set #{position}"),
        }
    }
}

/// One layer: a table and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    source: Source,
    table: Table,
}

impl Layer {
    /// A layer read from `source`.
    #[must_use]
    pub fn new(source: Source, table: Table) -> Self {
        Self { source, table }
    }

    /// Where the layer came from.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The layer's keys.
    #[must_use]
    pub fn table(&self) -> &Table {
        &self.table
    }
}

/// The merged config and the source of every leaf value, by key path (`tags.format`; an array is
/// one leaf, e.g. `steps.plugins`).
#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    table: Table,
    sources: BTreeMap<String, Source>,
}

impl Merged {
    /// The merged table, ready for `Config::from_table`.
    #[must_use]
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// The merged table, consuming the result.
    #[must_use]
    pub fn into_table(self) -> Table {
        self.table
    }

    /// Where the value at a key path came from.
    #[must_use]
    pub fn source(&self, path: &str) -> Option<&Source> {
        self.sources.get(path)
    }

    /// Every leaf key path with its source.
    #[must_use]
    pub fn sources(&self) -> &BTreeMap<String, Source> {
        &self.sources
    }
}

/// Merges `defaults` (always key by key, underneath), the user `layers` in order (deep, or
/// domain by domain when `mode` is shallow) and the `flags` (single keys, on top).
#[must_use]
pub fn merge(defaults: &Table, layers: &[Layer], flags: &[Layer], mode: MergeMode) -> Merged {
    let mut user = Merged::empty();
    for layer in layers {
        let source = |_: &str| layer.source.clone();
        match mode {
            MergeMode::Deep => overlay(
                &mut user.table,
                &mut user.sources,
                "",
                &layer.table,
                &source,
            ),
            MergeMode::Shallow => {
                for (domain, value) in &layer.table {
                    replace(
                        &mut user.table,
                        &mut user.sources,
                        "",
                        domain,
                        value,
                        &source,
                    );
                }
            }
        }
    }
    let mut merged = Merged::empty();
    overlay(
        &mut merged.table,
        &mut merged.sources,
        "",
        defaults,
        &|_| Source::Default,
    );
    let user_source = |path: &str| user.sources.get(path).cloned().unwrap_or(Source::Default);
    overlay(
        &mut merged.table,
        &mut merged.sources,
        "",
        &user.table,
        &user_source,
    );
    for flag in flags {
        let source = |_: &str| flag.source.clone();
        overlay(
            &mut merged.table,
            &mut merged.sources,
            "",
            &flag.table,
            &source,
        );
    }
    merged
}

impl Merged {
    fn empty() -> Self {
        Self {
            table: Table::new(),
            sources: BTreeMap::new(),
        }
    }
}

/// Merges `layer` into `target` (at key path `prefix`): tables key by key, anything else
/// replaces.
fn overlay(
    target: &mut Table,
    sources: &mut BTreeMap<String, Source>,
    prefix: &str,
    layer: &Table,
    source: &dyn Fn(&str) -> Source,
) {
    for (name, value) in layer {
        match (target.get_mut(name), value) {
            (Some(Value::Table(inner)), Value::Table(layer_inner)) => {
                overlay(inner, sources, &join(prefix, name), layer_inner, source);
            }
            _ => replace(target, sources, prefix, name, value, source),
        }
    }
}

/// Puts `value` at `name`, dropping whatever was there and its sources.
fn replace(
    target: &mut Table,
    sources: &mut BTreeMap<String, Source>,
    prefix: &str,
    name: &str,
    value: &Value,
    source: &dyn Fn(&str) -> Source,
) {
    let path = join(prefix, name);
    let nested = format!("{path}.");
    sources.retain(|key, _| *key != path && !key.starts_with(&nested));
    target.insert(name.to_owned(), value.clone());
    record_leaves(&path, value, source, sources);
}

fn record_leaves(
    path: &str,
    value: &Value,
    source: &dyn Fn(&str) -> Source,
    sources: &mut BTreeMap<String, Source>,
) {
    match value {
        Value::Table(table) => {
            for (name, inner) in table {
                record_leaves(&join(path, name), inner, source, sources);
            }
        }
        _ => {
            sources.insert(path.to_owned(), source(path));
        }
    }
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}.{name}")
    }
}
