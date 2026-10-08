//! `semoxide.toml` as typed configuration, one type per domain (CONFIG.md).
//!
//! [`Config::from_table`] reads a merged TOML table by hand, domain by domain, so every error
//! carries its code and exact key path (`branches.rules[2].prerelease`).

use std::collections::BTreeMap;

use toml::{Table, Value};

mod branches;
mod commits;
#[cfg(test)]
mod defaults_tests;
mod error;
mod layering;
#[cfg(test)]
mod message_tests;
mod plugins;
mod reader;
mod secrets;
mod steps;
mod suggest;
#[cfg(test)]
mod suggestion_mutation_tests;
#[cfg(test)]
mod suggestion_tests;
mod tags;
#[cfg(test)]
mod tests;
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
        branch_fields.finish(&["rules"])?;
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
