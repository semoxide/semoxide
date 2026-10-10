//! `semoxide.toml` as typed configuration, one type per domain (CONFIG.md).
//!
//! [`Config::from_table`] reads a merged TOML table by hand, domain by domain, so every error
//! carries its code and exact key path (`branches.rules[2].prerelease`).

use std::collections::BTreeMap;

use toml::{Table, Value};

mod branches;
mod commits;
mod error;
mod layering;
mod plugins;
mod reader;
mod secrets;
mod steps;
mod suggest;
mod tags;
mod values;
mod version;

pub use branches::{
    BranchRule, Branches, Channel, MaintenanceRule, Prerelease, PrereleaseRule, ReleaseRule,
};
pub use commits::{Commits, Preset};
pub use error::ConfigError;
pub use layering::{ConfigDomain, MergeMode};
pub use plugins::PluginConfig;
pub use secrets::Secrets;
pub use steps::{Steps, SuccessErrors};
pub use tags::Tags;
pub use values::{EnvName, PluginName, Step, TagFormat, Template};
pub use version::{Level, VersionDomain, ZeroLevels};

use plugins::parse_plugins;
use reader::{DOMAINS, Fields, index, key};

/// A validated configuration: the merged layers with every default applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    layering: ConfigDomain,
    commits: Commits,
    version: VersionDomain,
    branches: Branches,
    tags: Tags,
    steps: Steps,
    plugins: BTreeMap<PluginName, PluginConfig>,
    secrets: Secrets,
}

impl Config {
    /// Validates a merged TOML table (CONFIG.md).
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] with the code and key path of the first problem.
    pub fn from_table(table: Table) -> Result<Self, ConfigError> {
        let mut fields = Fields::from_table("", table);
        let layering = ConfigDomain::parse(fields.domain("config")?)?;
        let commits = Commits::parse(fields.domain("commits")?)?;
        let version = VersionDomain::parse(fields.domain("version")?)?;
        let mut branch_fields = fields.domain("branches")?;
        let branches = Branches::parse(&mut branch_fields)?;
        branch_fields.finish(&branches::KEYS)?;
        let tags = Tags::parse(fields.domain("tags")?)?;
        let steps = Steps::parse(fields.domain("steps")?)?;
        let plugins = parse_plugins(fields.domain("plugins")?, &steps)?;
        let secrets = Secrets::parse(fields.domain("secrets")?)?;
        if let Some((path, _)) = fields.take("packages") {
            return Err(ConfigError::unsupported(&path));
        }
        fields.finish(&DOMAINS)?;
        Ok(Self {
            layering,
            commits,
            version,
            branches,
            tags,
            steps,
            plugins,
            secrets,
        })
    }

    /// The built-in defaults as the lowest config layer: [`Config::default`] as a table, without
    /// `[config]` (set only by `semoxide.toml`) and `[plugins]` (per-plugin defaults depend on
    /// `steps.plugins`, known only after merging).
    #[must_use]
    pub fn defaults_table() -> Table {
        let mut table = Self::default().to_table();
        table.remove("config");
        table.remove("plugins");
        table
    }

    /// The configuration as a TOML table that [`Config::from_table`] reads back unchanged.
    #[must_use]
    pub fn to_table(&self) -> Table {
        let mut table = Table::new();
        let mut insert = |name: &str, domain: Table| {
            table.insert(name.to_owned(), Value::Table(domain));
        };
        insert("config", self.layering.to_table());
        insert("commits", self.commits.to_table());
        insert("version", self.version.to_table());
        insert(
            "branches",
            Table::from_iter([(String::from("rules"), self.branches.to_value())]),
        );
        insert("tags", self.tags.to_table());
        insert("steps", self.steps.to_table());
        insert(
            "plugins",
            self.plugins
                .iter()
                .map(|(name, settings)| (name.to_string(), Value::Table(settings.to_table())))
                .collect(),
        );
        insert("secrets", self.secrets.to_table());
        table
    }

    /// `[config]`
    #[must_use]
    pub fn config(&self) -> &ConfigDomain {
        &self.layering
    }

    /// `[commits]`
    #[must_use]
    pub fn commits(&self) -> &Commits {
        &self.commits
    }

    /// `[version]`
    #[must_use]
    pub fn version(&self) -> &VersionDomain {
        &self.version
    }

    /// `[branches]`
    #[must_use]
    pub fn branches(&self) -> &Branches {
        &self.branches
    }

    /// `[tags]`
    #[must_use]
    pub fn tags(&self) -> &Tags {
        &self.tags
    }

    /// `[steps]`
    #[must_use]
    pub fn steps(&self) -> &Steps {
        &self.steps
    }

    /// The settings of an enabled plugin (its `[plugins.<name>]` table, or defaults without
    /// one); `None` for a plugin missing from `steps.plugins`.
    #[must_use]
    pub fn plugin(&self, name: &PluginName) -> Option<&PluginConfig> {
        self.plugins.get(name)
    }

    /// `[secrets]`
    #[must_use]
    pub fn secrets(&self) -> &Secrets {
        &self.secrets
    }
}

impl Default for Config {
    fn default() -> Self {
        let steps = Steps::defaults();
        let plugins = steps
            .plugins()
            .iter()
            .map(|name| (name.clone(), PluginConfig::default()))
            .collect();
        Self {
            layering: ConfigDomain::DEFAULT,
            commits: Commits::DEFAULT,
            version: VersionDomain::defaults(),
            branches: Branches::default_rules(),
            tags: Tags::defaults(),
            steps,
            plugins,
            secrets: Secrets::DEFAULT,
        }
    }
}

/// Helpers shared by the config modules' unit tests.
#[cfg(test)]
mod test_support {
    use semoxide_error::{ErrorCode, ErrorInfo};

    use super::{BranchRule, Channel, Config, ConfigError, PluginName, Prerelease};

    pub(super) fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::from_table(text.parse::<toml::Table>().unwrap())
    }

    /// The config, failing the test with an assertion if it doesn't load.
    pub(super) fn loaded(text: &str) -> Config {
        let result = parse(text);
        assert!(result.is_ok(), "the config should load: {result:?}");
        result.unwrap()
    }

    /// The code and key path of the error, or `Ok` if the config loaded.
    pub(super) fn rejection(text: &str) -> Result<(), (ErrorCode, String)> {
        parse(text)
            .map(|_| ())
            .map_err(|error| (error.code(), error.path().to_owned()))
    }

    pub(super) fn plugin(name: &str) -> PluginName {
        name.parse().unwrap()
    }

    pub(super) fn plugins(names: &[&str]) -> Vec<PluginName> {
        names.iter().map(|name| plugin(name)).collect()
    }

    /// Every field of a branch rule in one line, so a test can compare whole rules.
    pub(super) fn describe(rule: &BranchRule) -> String {
        let channel = |channel: Option<&Channel>| match channel {
            None => String::from("-"),
            Some(Channel::Default) => String::from("default"),
            Some(Channel::Named(name)) => format!("'{}'", name.as_str()),
        };
        match rule {
            BranchRule::Release(rule) => {
                format!(
                    "release {} channel={}",
                    rule.name(),
                    channel(rule.channel())
                )
            }
            BranchRule::Prerelease(rule) => {
                let id = match rule.prerelease() {
                    Prerelease::BranchName => String::from("<branch>"),
                    Prerelease::Id(id) => format!("'{}'", id.as_str()),
                };
                format!(
                    "prerelease {} id={id} channel={}",
                    rule.name(),
                    channel(rule.channel())
                )
            }
            BranchRule::Maintenance(rule) => format!(
                "maintenance {} range={} channel={}",
                rule.pattern(),
                rule.range().unwrap_or("-"),
                channel(rule.channel())
            ),
        }
    }

    pub(super) fn describe_rules(config: &Config) -> Vec<String> {
        config.branches().rules().iter().map(describe).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rstest::rstest;
    use semoxide_error::ErrorCode;
    use semver::Version;

    use super::test_support::{describe_rules, loaded, plugin, plugins, rejection};
    use super::*;
    use crate::codes::{CONFIG_UNKNOWN_KEY, CONFIG_UNSUPPORTED_SECTION};

    // --- Defaults (CONFIG.md) ---

    #[test]
    fn empty_table_gives_the_defaults() {
        let config = loaded("");

        assert_eq!(config.config().merge(), MergeMode::Deep);
        assert_eq!(config.commits().preset(), Preset::ConventionalCommits);
        assert_eq!(config.version().initial(), &Version::new(1, 0, 0));
        let zero = config.version().zero();
        assert_eq!(
            (zero.breaking(), zero.feature(), zero.fix()),
            (Level::Minor, Level::Patch, Level::Patch)
        );
        assert_eq!(config.tags().format().as_str(), "v{version}");
        assert_eq!(config.tags().metadata(), None);
        assert_eq!(
            config.steps().plugins(),
            plugins(&["commit-analyzer", "release-notes"])
        );
        for step in [Step::VerifyConditions, Step::Publish, Step::Success] {
            assert_eq!(config.steps().order(step), None, "{step:?}");
        }
        assert_eq!(config.steps().success_errors(), SuccessErrors::Warn);
        assert_eq!(config.secrets().mask_env(), &[] as &[EnvName]);
    }

    #[rstest]
    #[case::packages(
        "[packages.web]\npath = \"web\"",
        CONFIG_UNSUPPORTED_SECTION,
        "packages"
    )]
    // top level
    #[case::unknown_domain("[colour]\nname = \"red\"", CONFIG_UNKNOWN_KEY, "colour")]
    fn invalid_config_is_rejected(#[case] text: &str, #[case] code: ErrorCode, #[case] path: &str) {
        assert_eq!(rejection(text), Err((code, path.to_owned())));
    }

    // --- Examples and round trip ---

    /// CONFIG.md's full example, verbatim.
    const CONFIG_MD_EXAMPLE: &str = r#"
[config]
extends = "preset:rust"
merge = "deep"

[commits]
preset = "conventionalcommits"

[version]
initial = "0.1.0"
zero = { breaking = "minor", feature = "patch", fix = "patch" }

[branches]
rules = [{ maintenance = "N.x" }, "main", { name = "beta", prerelease = true }]

[tags]
format = "v{version}"

[steps]
plugins = ["commit-analyzer", "release-notes", "git", "github"]
publish.order = ["github", "git"]
success.errors = "warn"

[plugins.github]
version = "1.4.2"

[secrets]
mask_env = ["DEPLOY_TOKEN"]
"#;

    #[test]
    fn config_md_example_is_rejected_until_extends_is_supported() {
        assert_eq!(
            rejection(CONFIG_MD_EXAMPLE),
            Err((CONFIG_UNSUPPORTED_SECTION, String::from("config.extends")))
        );
    }

    #[test]
    fn config_md_example_without_extends_loads() {
        let config = loaded(&CONFIG_MD_EXAMPLE.replace("extends = \"preset:rust\"\n", ""));

        assert_eq!(config.config().merge(), MergeMode::Deep);
        assert_eq!(config.commits().preset(), Preset::ConventionalCommits);
        assert_eq!(config.version().initial(), &Version::new(0, 1, 0));
        let zero = config.version().zero();
        assert_eq!(
            (zero.breaking(), zero.feature(), zero.fix()),
            (Level::Minor, Level::Patch, Level::Patch)
        );
        assert_eq!(
            describe_rules(&config),
            [
                "maintenance N.x range=- channel=-",
                "release main channel=-",
                "prerelease beta id=<branch> channel=-",
            ]
        );
        assert_eq!(config.tags().format().as_str(), "v{version}");
        assert_eq!(
            config.steps().plugins(),
            plugins(&["commit-analyzer", "release-notes", "git", "github"])
        );
        assert_eq!(
            config.steps().order(Step::Publish),
            Some(plugins(&["github", "git"]).as_slice())
        );
        assert_eq!(config.steps().success_errors(), SuccessErrors::Warn);
        assert_eq!(
            config
                .plugin(&plugin("github"))
                .and_then(|github| github.version()),
            Some(&Version::new(1, 4, 2))
        );
        let names: Vec<&str> = config
            .secrets()
            .mask_env()
            .iter()
            .map(EnvName::as_str)
            .collect();
        assert_eq!(names, ["DEPLOY_TOKEN"]);
    }

    /// Every key set to a value that differs from its default.
    const EVERY_KEY_CHANGED: &str = r#"
[config]
merge = "shallow"

[commits]
preset = "angular"

[version]
initial = "0.1.0"
zero = { breaking = "major", feature = "minor", fix = "minor" }

[branches]
rules = [
  { maintenance = "release/N.N.x", channel = "{name}" },
  { maintenance = "legacy", range = "1.x", channel = false },
  { name = "trunk", channel = "stable" },
  { name = "rc/*", prerelease = "rc-{name}", channel = "{name}" },
]

[tags]
format = "release-{version}"
metadata = "{{ commit.short_sha }}"

[steps]
plugins = ["release-notes", "github"]
verify_conditions.order = ["github", "release-notes"]
success.order = ["github"]
success.errors = "fail"

[plugins.github]
version = "1.4.2"
timeouts.publish = "30m"
show_output = true
assets = ["a", { path = "b", label = "B" }]

[secrets]
mask_env = ["DEPLOY_TOKEN"]
"#;

    #[rstest]
    #[case::defaults("")]
    #[case::every_key_changed(EVERY_KEY_CHANGED)]
    fn config_round_trips_through_toml(#[case] text: &str) {
        let config = loaded(text);

        assert_eq!(Config::from_table(config.to_table()), Ok(config));
    }

    #[test]
    fn every_key_changed_differs_from_the_defaults_in_every_domain() {
        let config = loaded(EVERY_KEY_CHANGED);
        let defaults = loaded("");

        assert_ne!(config.config(), defaults.config());
        assert_ne!(config.commits(), defaults.commits());
        assert_ne!(config.version(), defaults.version());
        assert_ne!(config.branches(), defaults.branches());
        assert_ne!(config.tags(), defaults.tags());
        assert_ne!(config.steps(), defaults.steps());
        assert_ne!(config.secrets(), defaults.secrets());
        assert_ne!(
            config.plugin(&plugin("github")),
            defaults.plugin(&plugin("github"))
        );
    }

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
}
