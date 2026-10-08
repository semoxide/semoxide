//! Reading a TOML table key by key, with the full key path of each value.

use toml::{Table, Value};

use super::ConfigError;

/// The keys of one table, taken one by one; [`Fields::finish`] rejects any left over.
pub(super) struct Fields {
    path: String,
    table: Table,
}

impl Fields {
    pub(super) fn from_table(path: &str, table: Table) -> Self {
        Self {
            path: path.to_owned(),
            table,
        }
    }

    pub(super) fn from_value(path: &str, value: Value) -> Result<Self, ConfigError> {
        match value {
            Value::Table(table) => Ok(Self::from_table(path, table)),
            other => Err(ConfigError::invalid(path, &other, "expected a table")),
        }
    }

    /// The key's full path and value, if present.
    pub(super) fn take(&mut self, name: &str) -> Option<(String, Value)> {
        self.table
            .remove(name)
            .map(|value| (key(&self.path, name), value))
    }

    /// A domain table, or an empty one when it isn't set.
    pub(super) fn domain(&mut self, name: &str) -> Result<Self, ConfigError> {
        match self.take(name) {
            Some((path, value)) => Self::from_value(&path, value),
            None => Ok(Self::from_table(&key(&self.path, name), Table::new())),
        }
    }

    /// Every remaining key with its full path, in key order.
    pub(super) fn drain(&mut self) -> Vec<(String, String, Value)> {
        std::mem::take(&mut self.table)
            .into_iter()
            .map(|(name, value)| (key(&self.path, &name), name, value))
            .collect()
    }

    /// Rejects the first key left over; `known` are the keys valid here, for the hint.
    pub(super) fn finish(self, known: &[&str]) -> Result<(), ConfigError> {
        match self.table.keys().next() {
            Some(name) => Err(ConfigError::unknown(&key(&self.path, name), known)),
            None => Ok(()),
        }
    }
}

/// The domains, in the order CONFIG.md lists them.
pub(super) const DOMAINS: [&str; 8] = [
    "config", "commits", "version", "branches", "tags", "steps", "plugins", "secrets",
];

/// `path.name`, or `name` at the top level.
pub(super) fn key(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}.{name}")
    }
}

/// `path[position]`
pub(super) fn index(path: &str, position: usize) -> String {
    format!("{path}[{position}]")
}
