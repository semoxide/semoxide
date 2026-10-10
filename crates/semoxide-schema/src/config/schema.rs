//! The JSON Schema of `semoxide.toml` (CONFIG.md §12), assembled from each table's schema next to
//! its parser.

use schemars::Schema;
use schemars::generate::SchemaSettings;
use toml::Table;

use super::Config;
use super::values::to_json;

/// The JSON Schema (draft-07) of `semoxide.toml`.
#[must_use]
pub fn json_schema() -> serde_json::Value {
    SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<Config>()
        .to_value()
}

/// One key of a [`closed_table`].
pub(super) struct Property {
    name: &'static str,
    description: Option<&'static str>,
    schema: Schema,
}

impl Property {
    /// A key with its description.
    pub(super) fn new(name: &'static str, description: &'static str, schema: Schema) -> Self {
        Self {
            name,
            description: Some(description),
            schema,
        }
    }

    /// A key whose schema describes itself: a table, or a `$ref` to one (a description next to
    /// a `$ref` would be dropped).
    pub(super) fn table(name: &'static str, schema: Schema) -> Self {
        Self {
            name,
            description: None,
            schema,
        }
    }
}

/// A table that rejects keys it doesn't list; a key with a value in `defaults` that isn't a
/// table gets it as its `default`.
pub(super) fn closed_table(
    description: &str,
    properties: impl IntoIterator<Item = Property>,
    defaults: &Table,
) -> Schema {
    let properties: serde_json::Map<String, serde_json::Value> = properties
        .into_iter()
        .map(|property| {
            let Property {
                name,
                description,
                mut schema,
            } = property;
            if let Some(description) = description {
                schema.insert(String::from("description"), description.into());
            }
            if let Some(default) = defaults
                .get(name)
                .filter(|value| !value.is_table())
                .and_then(|value| to_json(name, value).ok())
            {
                schema.insert(String::from("default"), default);
            }
            (name.to_owned(), schema.to_value())
        })
        .collect();
    schemars::json_schema!({
        "type": "object",
        "description": description,
        "properties": properties,
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};

    use serde_json::Value as Json;
    use toml::{Table, Value};

    use super::super::reader::DOMAINS;
    use super::super::test_support::{DRAFT_07, schema_errors};
    use super::super::values::from_json;
    use super::super::{
        Config, PluginName, Step, branches, commits, layering, secrets, steps, tags, version,
    };
    use super::json_schema;

    const UPDATE: &str = "SEMOXIDE_UPDATE_SCHEMA";

    fn committed_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/semoxide.schema.json")
    }

    /// The first line where the two texts differ.
    fn first_difference(committed: &str, generated: &str) -> String {
        let mut committed_lines = committed.lines();
        let mut generated_lines = generated.lines();
        let mut number = 1;
        loop {
            match (committed_lines.next(), generated_lines.next()) {
                (None, None) => return String::from("only line endings differ"),
                (old, new) if old != new => {
                    return format!("line {number}: committed {old:?}, generated {new:?}");
                }
                _ => number += 1,
            }
        }
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "the regeneration switch is a setting of the test run, like insta's `INSTA_UPDATE`"
    )]
    fn the_committed_schema_is_up_to_date() {
        let generated = format!(
            "{}\n",
            serde_json::to_string_pretty(&json_schema()).unwrap()
        );
        let path = committed_path();
        if std::env::var_os(UPDATE).is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &generated).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();

        assert!(
            committed == generated,
            "`schemas/semoxide.schema.json` is stale or missing ({}); run \
             `{UPDATE}=1 cargo nextest run -p semoxide-schema` and review the diff",
            first_difference(&committed, &generated)
        );
    }

    #[test]
    fn the_schema_declares_draft_07() {
        assert_eq!(
            json_schema().get("$schema").and_then(Json::as_str),
            Some(DRAFT_07)
        );
    }

    // --- Tables and their keys ---

    /// Whether a table rejects keys it doesn't list.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Others {
        Rejected,
        Allowed,
    }

    /// One table of the schema: its config path, its keys, and what happens to other keys.
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct SchemaTable {
        path: String,
        keys: Vec<String>,
        others: Others,
    }

    fn table(path: &str, keys: &[&str], others: Others) -> SchemaTable {
        let mut keys: Vec<String> = keys.iter().map(|key| (*key).to_owned()).collect();
        keys.sort();
        SchemaTable {
            path: path.to_owned(),
            keys,
            others,
        }
    }

    /// `path.name`, `name` at the top level.
    fn join(path: &str, name: &str) -> String {
        if path.is_empty() {
            name.to_owned()
        } else {
            format!("{path}.{name}")
        }
    }

    /// `node`, or the definition its `$ref` points at.
    fn resolve<'a>(root: &'a Json, node: &'a Json) -> &'a Json {
        match node
            .get("$ref")
            .and_then(Json::as_str)
            .and_then(|reference| reference.strip_prefix('#'))
        {
            Some(pointer) => root.pointer(pointer).unwrap(),
            None => node,
        }
    }

    /// The schemas `node` applies to the same value: `anyOf`/`oneOf`/`allOf` entries and the
    /// `then`/`else` branches (`if` is only a condition).
    fn variants(node: &Json) -> Vec<&Json> {
        let lists = ["anyOf", "oneOf", "allOf"]
            .into_iter()
            .filter_map(|keyword| node.get(keyword).and_then(Json::as_array))
            .flatten();
        let branches = ["then", "else"]
            .into_iter()
            .filter_map(|keyword| node.get(keyword));
        lists.chain(branches).collect()
    }

    /// Every table the schema describes under `path`: array items get `[]`, any key of an open
    /// table `<name>`.
    fn collect_tables(root: &Json, node: &Json, path: &str, found: &mut Vec<SchemaTable>) {
        let node = resolve(root, node);
        for variant in variants(node) {
            collect_tables(root, variant, path, found);
        }
        if let Some(items) = node.get("items") {
            collect_tables(root, items, &format!("{path}[]"), found);
        }
        let properties = node.get("properties").and_then(Json::as_object);
        let additional = node.get("additionalProperties");
        if properties.is_none() && additional.is_none() {
            return;
        }
        let keys: Vec<&str> = properties
            .into_iter()
            .flat_map(|properties| properties.keys().map(String::as_str))
            .collect();
        let others = if additional == Some(&Json::Bool(false)) {
            Others::Rejected
        } else {
            Others::Allowed
        };
        found.push(table(path, &keys, others));
        for (name, property) in properties.into_iter().flatten() {
            collect_tables(root, property, &join(path, name), found);
        }
        if let Some(additional @ Json::Object(_)) = additional {
            collect_tables(root, additional, &join(path, "<name>"), found);
        }
    }

    #[test]
    fn every_table_has_the_parsers_keys_and_only_plugin_tables_are_open() {
        let schema = json_schema();
        let mut found = Vec::new();
        collect_tables(&schema, &schema, "", &mut found);
        found.sort();
        let mut expected = vec![
            table("", &DOMAINS, Others::Rejected),
            table("config", &layering::KEYS, Others::Rejected),
            table("commits", &commits::KEYS, Others::Rejected),
            table("version", &version::KEYS, Others::Rejected),
            table("version.zero", &version::ZERO_KEYS, Others::Rejected),
            table("branches", &branches::KEYS, Others::Rejected),
            table("branches.rules[]", &branches::RULE_KEYS, Others::Rejected),
            table(
                "branches.rules[]",
                &branches::MAINTENANCE_KEYS,
                Others::Rejected,
            ),
            table("tags", &tags::KEYS, Others::Rejected),
            table("steps", &steps::step_keys(), Others::Rejected),
            table("plugins", &[], Others::Allowed),
            table(
                "plugins.<name>",
                &["version", "timeouts", "show_output"],
                Others::Allowed,
            ),
            table("plugins.<name>.timeouts", &Step::NAMES, Others::Rejected),
            table("secrets", &secrets::KEYS, Others::Rejected),
        ];
        for step in Step::NAMES {
            let keys: &[&str] = if step == "success" {
                &steps::SUCCESS_KEYS
            } else {
                &steps::STEP_KEYS
            };
            expected.push(table(&format!("steps.{step}"), keys, Others::Rejected));
        }
        expected.sort();

        assert_eq!(found, expected);
    }

    // --- Defaults ---

    /// The `default` of every key under `path`, by config path.
    fn collect_defaults(root: &Json, node: &Json, path: &str, found: &mut BTreeMap<String, Json>) {
        let resolved = resolve(root, node);
        if let Some(default) = node.get("default").or_else(|| resolved.get("default")) {
            found.insert(path.to_owned(), default.clone());
        }
        for (name, property) in resolved
            .get("properties")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            collect_defaults(root, property, &join(path, name), found);
        }
        if let Some(additional @ Json::Object(_)) = resolved.get("additionalProperties") {
            collect_defaults(root, additional, &join(path, "<name>"), found);
        }
    }

    /// Every value that isn't a table, by config path.
    fn collect_leaves(path: &str, table: &Table, found: &mut BTreeMap<String, Json>) {
        for (name, value) in table {
            match value {
                Value::Table(inner) => collect_leaves(&join(path, name), inner, found),
                leaf => {
                    found.insert(join(path, name), serde_json::to_value(leaf).unwrap());
                }
            }
        }
    }

    /// The defaults in the code; every plugin has the same ones, shown as `plugins.<name>`.
    fn code_defaults() -> BTreeMap<String, Json> {
        let config = Config::default();
        let mut table = config.to_table();
        table.remove("plugins");
        let mut found = BTreeMap::new();
        collect_leaves("", &table, &mut found);
        let plugin = config
            .plugin(&PluginName::bundled("commit-analyzer"))
            .unwrap();
        collect_leaves("plugins.<name>", &plugin.to_table(), &mut found);
        found
    }

    #[test]
    fn the_schema_defaults_are_the_config_defaults() {
        let schema = json_schema();
        let mut found = BTreeMap::new();
        collect_defaults(&schema, &schema, "", &mut found);

        assert_eq!(found, code_defaults());
    }

    #[test]
    fn the_defaults_table_validates() {
        assert_eq!(
            schema_errors(&Config::defaults_table()),
            Vec::<String>::new()
        );
    }

    // --- Descriptions ---

    fn has_description(node: &Json) -> bool {
        node.get("description")
            .and_then(Json::as_str)
            .is_some_and(|text| !text.trim().is_empty())
    }

    /// Every table and key under `path` without a `description`.
    fn collect_undescribed(root: &Json, node: &Json, path: &str, found: &mut Vec<String>) {
        let node = resolve(root, node);
        for variant in variants(node) {
            collect_undescribed(root, variant, path, found);
        }
        if let Some(items) = node.get("items") {
            collect_undescribed(root, items, &format!("{path}[]"), found);
        }
        if let Some(additional @ Json::Object(_)) = node.get("additionalProperties") {
            collect_undescribed(root, additional, &join(path, "<name>"), found);
        }
        let Some(properties) = node.get("properties").and_then(Json::as_object) else {
            return;
        };
        if !has_description(node) {
            found.push(format!(
                "table `{}`",
                if path.is_empty() { "(root)" } else { path }
            ));
        }
        for (name, property) in properties {
            let key = join(path, name);
            if !has_description(property) && !has_description(resolve(root, property)) {
                found.push(format!("key `{key}`"));
            }
            collect_undescribed(root, property, &key, found);
        }
    }

    #[test]
    fn every_table_and_key_has_a_description() {
        let schema = json_schema();
        let mut found = Vec::new();
        collect_undescribed(&schema, &schema, "", &mut found);

        assert_eq!(found, Vec::<String>::new());
    }

    // --- Key reference in CONFIG.md ---

    const REFERENCE_BEGIN: &str = "<!-- key reference: generated by the semoxide-schema tests; \
                                   SEMOXIDE_UPDATE_SCHEMA=1 rewrites it -->";
    const REFERENCE_END: &str = "<!-- end of key reference -->";

    fn config_md_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CONFIG.md")
    }

    /// One row of the key reference.
    #[derive(Debug, Clone, PartialEq)]
    struct Row {
        key: String,
        default: String,
        description: String,
    }

    /// The default as inline TOML, or nothing.
    fn render_default(property: &Json) -> String {
        property
            .get("default")
            .map(|default| format!("`{}`", from_json(default)))
            .unwrap_or_default()
    }

    /// The description, with an enum's values.
    fn render_description(property: &Json) -> String {
        let text = property
            .get("description")
            .and_then(Json::as_str)
            .unwrap_or_default();
        match property.get("enum").and_then(Json::as_array) {
            Some(values) => {
                let names: Vec<String> = values
                    .iter()
                    .filter_map(Json::as_str)
                    .map(|name| format!("`{name}`"))
                    .collect();
                format!("{text} One of {}.", names.join(", "))
            }
            None => text.to_owned(),
        }
    }

    /// A row for every key under `path` that isn't a table.
    fn collect_rows(root: &Json, node: &Json, path: &str, rows: &mut Vec<Row>) {
        let node = resolve(root, node);
        for variant in variants(node) {
            collect_rows(root, variant, path, rows);
        }
        if let Some(items) = node.get("items") {
            collect_rows(root, items, &format!("{path}[]"), rows);
        }
        if let Some(additional @ Json::Object(_)) = node.get("additionalProperties") {
            collect_rows(root, additional, &join(path, "<name>"), rows);
        }
        for (name, property) in node
            .get("properties")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            let key = join(path, name);
            if resolve(root, property).get("properties").is_none() {
                let row = Row {
                    key: key.clone(),
                    default: render_default(property),
                    description: render_description(property),
                };
                if !rows.contains(&row) {
                    rows.push(row);
                }
            }
            collect_rows(root, property, &key, rows);
        }
    }

    /// `key` with each step name as `<step>`.
    fn any_step(key: &str) -> String {
        key.split('.')
            .map(|part| {
                if Step::NAMES.contains(&part) {
                    "<step>"
                } else {
                    part
                }
            })
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Rows that are the same for every step as one `<step>` row.
    fn merge_steps(rows: &[Row]) -> Vec<Row> {
        let mut merged: Vec<Row> = Vec::new();
        for row in rows {
            let key = any_step(&row.key);
            let same_for_every_step = rows
                .iter()
                .filter(|other| any_step(&other.key) == key)
                .filter(|other| {
                    other.default == row.default && other.description == row.description
                })
                .count()
                == Step::NAMES.len();
            let row = if key != row.key && same_for_every_step {
                Row { key, ..row.clone() }
            } else {
                row.clone()
            };
            if !merged.contains(&row) {
                merged.push(row);
            }
        }
        merged
    }

    /// CONFIG.md's key reference: one row per key with its default and description, the
    /// domains in CONFIG.md's order.
    fn key_reference(schema: &Json) -> String {
        let mut rows = Vec::new();
        collect_rows(schema, schema, "", &mut rows);
        let mut rows = merge_steps(&rows);
        rows.sort_by_key(|row| {
            let domain = row.key.split(['.', '[']).next().unwrap_or_default();
            DOMAINS.iter().position(|known| *known == domain)
        });
        let mut table = String::from("\n\n| Key | Default | Description |\n| --- | --- | --- |\n");
        for row in rows {
            let default = if row.default.is_empty() {
                String::from(" ")
            } else {
                format!(" {} ", row.default)
            };
            writeln!(table, "| `{}` |{default}| {} |", row.key, row.description).unwrap();
        }
        table.push('\n');
        table
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "the regeneration switch is a setting of the test run, like insta's `INSTA_UPDATE`"
    )]
    fn the_config_md_key_reference_is_up_to_date() {
        let path = config_md_path();
        let text = std::fs::read_to_string(&path).unwrap();
        let generated = key_reference(&json_schema());
        let parts = text.split_once(REFERENCE_BEGIN).and_then(|(before, rest)| {
            rest.split_once(REFERENCE_END)
                .map(|(block, after)| (before, block, after))
        });
        if std::env::var_os(UPDATE).is_some() {
            let (before, _, after) = parts.unwrap();
            std::fs::write(
                &path,
                format!("{before}{REFERENCE_BEGIN}{generated}{REFERENCE_END}{after}"),
            )
            .unwrap();
            return;
        }
        let committed = parts.map(|(_, block, _)| block);

        assert!(
            committed == Some(generated.as_str()),
            "the key reference in `docs/CONFIG.md` is stale or missing ({}); run \
             `{UPDATE}=1 cargo nextest run -p semoxide-schema` and review the diff",
            committed.map_or_else(
                || String::from("its markers are missing"),
                |block| first_difference(block, &generated)
            )
        );
    }
}
