//! The layer merge: defaults always fill key by key underneath; user layers merge deep, or
//! domain by domain with `config.merge = "shallow"`; `--set` flags override single keys on top.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use semoxide_schema::config::MergeMode;
use toml::Table;

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
        let _ = f;
        Ok(())
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
    let _ = (defaults, layers, flags, mode);
    Merged {
        table: Table::new(),
        sources: BTreeMap::new(),
    }
}
