use std::path::Path;

use toml::{Table, Value};

use super::{Config, PluginName};

fn empty() -> Result<Config, super::ConfigError> {
    Config::from_table(Table::new())
}

#[test]
fn default_is_what_an_empty_config_loads_to() {
    assert_eq!(empty(), Ok(Config::default()));
}

#[test]
fn defaults_table_loads_to_the_default() {
    assert_eq!(
        Config::from_table(Config::defaults_table()),
        Ok(Config::default())
    );
}

#[test]
fn defaults_table_has_every_domain_except_config_and_plugins() {
    let defaults = Config::defaults_table();
    let domains: Vec<&str> = defaults.keys().map(String::as_str).collect();

    assert_eq!(
        domains,
        ["branches", "commits", "secrets", "steps", "tags", "version"]
    );
}

/// The key and TOML value of each row of CONFIG.md's "Defaults" table.
fn documented_defaults() -> Vec<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CONFIG.md");
    let text = std::fs::read_to_string(path).unwrap();
    let section = text
        .split("### Defaults")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .unwrap();
    section
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let key = cells.get(1)?.strip_prefix('`')?.strip_suffix('`')?;
            let value = cells.get(2)?.strip_prefix('`')?.strip_suffix('`')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// The value at a dotted key path of a table.
fn lookup<'a>(table: &'a Table, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let first = table.get(parts.next()?)?;
    parts.try_fold(first, |value, part| value.as_table()?.get(part))
}

#[test]
fn config_md_defaults_match_the_code() {
    let defaults = Config::default().to_table();
    let documented = documented_defaults();
    assert!(documented.len() >= 10, "{documented:?}");

    for (key, value) in &documented {
        let documented: Table = format!("v = {value}").parse().unwrap();
        let documented = documented.get("v");
        let default_config = Config::default();
        let actual = if let Some(option) = key.strip_prefix("plugins.<name>.") {
            default_config
                .plugin(&PluginName::bundled("commit-analyzer"))
                .and_then(|settings| settings.to_table().get(option).cloned())
        } else {
            lookup(&defaults, key).cloned()
        };
        assert_eq!(actual.as_ref(), documented, "{key}");
    }
}

#[test]
fn every_default_is_documented() {
    let documented: Vec<String> = documented_defaults()
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let mut leaves = Vec::new();
    collect_leaves("", &Config::defaults_table(), &mut leaves);
    assert!(leaves.len() >= 10, "{leaves:?}");

    let undocumented: Vec<&String> = leaves
        .iter()
        .filter(|leaf| *leaf != "branches.rules" && !documented.contains(leaf))
        .collect();
    assert_eq!(undocumented, Vec::<&String>::new());
}

fn collect_leaves(prefix: &str, table: &Table, leaves: &mut Vec<String>) {
    for (name, value) in table {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        match value {
            Value::Table(inner) => collect_leaves(&path, inner, leaves),
            _ => leaves.push(path),
        }
    }
}

#[test]
fn config_md_default_branch_rules_match_the_code() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CONFIG.md");
    let text = std::fs::read_to_string(path).unwrap();
    let block = text
        .split("## 3. Branches")
        .nth(1)
        .and_then(|section| section.split("```toml\n").nth(1))
        .and_then(|rest| rest.split("```").next())
        .unwrap();
    let documented: Table = block.parse().unwrap();

    assert_eq!(
        lookup(&documented, "branches.rules"),
        lookup(&Config::default().to_table(), "branches.rules")
    );
}
