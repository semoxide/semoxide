//! The layer merge: defaults always fill key by key underneath; user layers merge deep, or
//! domain by domain with `config.merge = "shallow"`; `--set` flags override single keys on top.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use semoxide_schema::config::MergeMode;
use toml::{Table, Value};

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use semoxide_schema::config::{Config, MergeMode};
    use toml::{Table, Value};

    use super::*;

    fn table(text: &str) -> Table {
        text.parse().unwrap()
    }

    fn file(name: &str, text: &str) -> Layer {
        Layer::new(Source::File(PathBuf::from(name)), table(text))
    }

    fn flag(position: usize, text: &str) -> Layer {
        Layer::new(Source::Flag(position), table(text))
    }

    fn defaults() -> Table {
        table(
            r#"
[version]
initial = "1.0.0"
zero = { breaking = "minor", feature = "patch", fix = "patch" }

[tags]
format = "v{version}"

[steps]
plugins = ["commit-analyzer", "release-notes"]
success = { errors = "warn" }
"#,
        )
    }

    /// The value at a dotted key path.
    fn value<'a>(merged: &'a Merged, path: &str) -> Option<&'a Value> {
        let mut parts = path.split('.');
        let first = merged.table().get(parts.next()?)?;
        parts.try_fold(first, |value, part| value.as_table()?.get(part))
    }

    fn semoxide_toml() -> Source {
        Source::File(PathBuf::from("semoxide.toml"))
    }

    // --- Defaults ---

    #[test]
    fn defaults_alone_give_the_defaults_with_their_source() {
        let merged = merge(&defaults(), &[], &[], MergeMode::Deep);

        assert_eq!(merged.table(), &defaults());
        assert_eq!(
            merged
                .sources()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "steps.plugins",
                "steps.success.errors",
                "tags.format",
                "version.initial",
                "version.zero.breaking",
                "version.zero.feature",
                "version.zero.fix",
            ]
        );
        assert!(
            merged
                .sources()
                .values()
                .all(|source| *source == Source::Default),
            "{:?}",
            merged.sources()
        );
    }

    // --- The file over the defaults ---

    #[test]
    fn file_overrides_a_default_and_keeps_its_neighbours() {
        let merged = merge(
            &defaults(),
            &[file("semoxide.toml", "version.initial = \"0.1.0\"")],
            &[],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "version.initial"),
            Some(&Value::from("0.1.0"))
        );
        assert_eq!(merged.source("version.initial"), Some(&semoxide_toml()));
        assert_eq!(
            value(&merged, "version.zero.breaking"),
            Some(&Value::from("minor"))
        );
        assert_eq!(
            merged.source("version.zero.breaking"),
            Some(&Source::Default)
        );
    }

    #[test]
    fn file_array_replaces_the_default_array() {
        let merged = merge(
            &defaults(),
            &[file("semoxide.toml", "steps.plugins = [\"git\"]")],
            &[],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "steps.plugins"),
            Some(&Value::Array(vec![Value::from("git")]))
        );
        assert_eq!(merged.source("steps.plugins"), Some(&semoxide_toml()));
    }

    #[test]
    fn file_adds_keys_the_defaults_dont_have() {
        let merged = merge(
            &defaults(),
            &[file("semoxide.toml", "[plugins.github]\ndraft = true")],
            &[],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "plugins.github.draft"),
            Some(&Value::from(true))
        );
        assert_eq!(
            merged.source("plugins.github.draft"),
            Some(&semoxide_toml())
        );
    }

    #[test]
    fn file_table_replaces_a_default_scalar_whole() {
        let merged = merge(
            &defaults(),
            &[file("semoxide.toml", "[tags.format]\nprefix = \"v\"")],
            &[],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "tags.format.prefix"),
            Some(&Value::from("v"))
        );
        assert_eq!(merged.source("tags.format"), None);
        assert_eq!(merged.source("tags.format.prefix"), Some(&semoxide_toml()));
    }

    // --- Flags on top ---

    #[test]
    fn flag_overrides_the_file() {
        let merged = merge(
            &defaults(),
            &[file("semoxide.toml", "tags.format = \"release-{version}\"")],
            &[flag(1, "tags.format = \"v{version}-rc\"")],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "tags.format"),
            Some(&Value::from("v{version}-rc"))
        );
        assert_eq!(merged.source("tags.format"), Some(&Source::Flag(1)));
    }

    #[test]
    fn later_flag_wins() {
        let merged = merge(
            &defaults(),
            &[],
            &[
                flag(1, "tags.format = \"a-{version}\""),
                flag(2, "tags.format = \"b-{version}\""),
            ],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "tags.format"),
            Some(&Value::from("b-{version}"))
        );
        assert_eq!(merged.source("tags.format"), Some(&Source::Flag(2)));
    }

    #[test]
    fn flag_in_a_table_keeps_the_files_other_keys() {
        let merged = merge(
            &defaults(),
            &[file(
                "semoxide.toml",
                "[plugins.github]\ndraft = true\nassets = [\"dist/*\"]",
            )],
            &[flag(1, "plugins.github.draft = false")],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "plugins.github.draft"),
            Some(&Value::from(false))
        );
        assert_eq!(
            merged.source("plugins.github.draft"),
            Some(&Source::Flag(1))
        );
        assert_eq!(
            value(&merged, "plugins.github.assets"),
            Some(&Value::Array(vec![Value::from("dist/*")]))
        );
        assert_eq!(
            merged.source("plugins.github.assets"),
            Some(&semoxide_toml())
        );
    }

    #[test]
    fn flag_array_replaces_the_files_array() {
        let merged = merge(
            &defaults(),
            &[file(
                "semoxide.toml",
                "steps.plugins = [\"git\", \"github\"]",
            )],
            &[flag(1, "steps.plugins = [\"npm\"]")],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "steps.plugins"),
            Some(&Value::Array(vec![Value::from("npm")]))
        );
        assert_eq!(merged.source("steps.plugins"), Some(&Source::Flag(1)));
    }

    // --- Deep and shallow between user layers ---

    fn preset() -> Layer {
        file(
            "preset.toml",
            "[plugins.github]\nassets = [\"dist/*\"]\ndraft = true",
        )
    }

    #[test]
    fn deep_merge_keeps_the_earlier_layers_keys_in_a_domain() {
        let merged = merge(
            &defaults(),
            &[
                preset(),
                file("semoxide.toml", "[plugins.github]\ndraft = false"),
            ],
            &[],
            MergeMode::Deep,
        );

        assert_eq!(
            value(&merged, "plugins.github.draft"),
            Some(&Value::from(false))
        );
        assert_eq!(
            merged.source("plugins.github.assets"),
            Some(&Source::File(PathBuf::from("preset.toml")))
        );
    }

    #[test]
    fn shallow_merge_replaces_the_earlier_layers_domain() {
        let merged = merge(
            &defaults(),
            &[
                preset(),
                file("semoxide.toml", "[plugins.github]\ndraft = false"),
            ],
            &[],
            MergeMode::Shallow,
        );

        assert_eq!(
            value(&merged, "plugins.github.draft"),
            Some(&Value::from(false))
        );
        assert_eq!(value(&merged, "plugins.github.assets"), None);
        assert_eq!(merged.source("plugins.github.assets"), None);
    }

    #[test]
    fn shallow_merge_keeps_domains_the_later_layer_doesnt_set() {
        let merged = merge(
            &defaults(),
            &[
                preset(),
                file("semoxide.toml", "tags.format = \"r{version}\""),
            ],
            &[],
            MergeMode::Shallow,
        );

        assert_eq!(
            value(&merged, "plugins.github.draft"),
            Some(&Value::from(true))
        );
        assert_eq!(
            merged.source("plugins.github.draft"),
            Some(&Source::File(PathBuf::from("preset.toml")))
        );
    }

    #[test]
    fn shallow_merge_never_drops_defaults() {
        let merged = merge(
            &defaults(),
            &[file("semoxide.toml", "version.initial = \"0.1.0\"")],
            &[],
            MergeMode::Shallow,
        );

        assert_eq!(
            value(&merged, "version.initial"),
            Some(&Value::from("0.1.0"))
        );
        assert_eq!(
            value(&merged, "version.zero.breaking"),
            Some(&Value::from("minor"))
        );
        assert_eq!(
            merged.source("version.zero.breaking"),
            Some(&Source::Default)
        );
    }

    #[test]
    fn flags_set_single_keys_even_when_shallow() {
        let merged = merge(
            &defaults(),
            &[preset()],
            &[flag(1, "plugins.github.draft = false")],
            MergeMode::Shallow,
        );

        assert_eq!(
            value(&merged, "plugins.github.draft"),
            Some(&Value::from(false))
        );
        assert_eq!(
            value(&merged, "plugins.github.assets"),
            Some(&Value::Array(vec![Value::from("dist/*")]))
        );
    }

    // --- Into the typed config ---

    #[test]
    fn merged_layers_load_as_a_config() {
        let merged = merge(
            &Config::defaults_table(),
            &[file(
                "semoxide.toml",
                "version.initial = \"0.1.0\"\nsteps.plugins = [\"commit-analyzer\", \"github\"]",
            )],
            &[flag(1, "tags.format = \"release-{version}\"")],
            MergeMode::Deep,
        );

        let config = Config::from_table(merged.into_table());

        assert!(config.is_ok(), "{config:?}");
        let config = config.unwrap();
        assert_eq!(config.version().initial().to_string(), "0.1.0");
        assert_eq!(config.tags().format().as_str(), "release-{version}");
        assert_eq!(config.version().zero(), Config::default().version().zero());
    }

    // --- Source display ---

    #[test]
    fn sources_name_the_layer() {
        assert_eq!(Source::Default.to_string(), "default");
        assert_eq!(semoxide_toml().to_string(), "file semoxide.toml");
        assert_eq!(Source::Flag(2).to_string(), "--set #2");
    }
}
