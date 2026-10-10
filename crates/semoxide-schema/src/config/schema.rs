//! The JSON Schema of `semoxide.toml` (CONFIG.md §12), assembled from each table's schema next to
//! its parser.

/// The JSON Schema (draft-07) of `semoxide.toml`.
#[must_use]
pub fn json_schema() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use serde_json::Value as Json;
    use toml::{Table, Value};

    use super::super::reader::DOMAINS;
    use super::super::test_support::{DRAFT_07, schema_errors};
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

    /// Every table the schema describes under `path`: array items get `[]`, any key of an open
    /// table `<name>`.
    fn collect_tables(root: &Json, node: &Json, path: &str, found: &mut Vec<SchemaTable>) {
        let node = resolve(root, node);
        for variants in ["anyOf", "oneOf", "allOf"].map(|keyword| node.get(keyword)) {
            for variant in variants.and_then(Json::as_array).into_iter().flatten() {
                collect_tables(root, variant, path, found);
            }
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
}
