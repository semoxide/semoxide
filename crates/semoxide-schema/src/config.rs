//! `semoxide.toml` as typed configuration, one type per domain (CONFIG.md).
//!
//! [`Config::from_table`] reads a merged TOML table by hand, domain by domain, so every error
//! carries its code and exact key path (`branches.rules[2].prerelease`).

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use semoxide_error::{ErrorCode, ErrorInfo};
use semver::Version;
use toml::{Table, Value};

use crate::codes::{
    CONFIG_CONFLICTING_KEYS, CONFIG_INVALID_VALUE, CONFIG_PLUGIN_NOT_ENABLED, CONFIG_UNKNOWN_KEY,
    CONFIG_UNSUPPORTED_SECTION,
};

mod branches;
#[cfg(test)]
mod message_tests;
mod suggest;
#[cfg(test)]
mod suggestion_mutation_tests;
#[cfg(test)]
mod suggestion_tests;
#[cfg(test)]
mod tests;
mod values;

pub use branches::{
    BranchRule, Branches, Channel, MaintenanceRule, Prerelease, PrereleaseRule, ReleaseRule,
};
pub use values::{EnvName, PluginName, Step, TagFormat, Template};

use values::{array, choice, choice_name, string};

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

/// `[config]`: how the configuration itself is assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDomain {
    merge: MergeMode,
}

const MERGE_MODES: [(&str, MergeMode); 2] =
    [("deep", MergeMode::Deep), ("shallow", MergeMode::Shallow)];

impl ConfigDomain {
    /// `config.merge`
    #[must_use]
    pub fn merge(&self) -> MergeMode {
        self.merge
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        if let Some((path, _)) = fields.take("extends") {
            return Err(ConfigError::unsupported(&path));
        }
        let merge = fields
            .take("merge")
            .map(|(path, value)| choice(&path, &value, &MERGE_MODES))
            .transpose()?
            .unwrap_or(MergeMode::Deep);
        fields.finish(&["merge"])?;
        Ok(Self { merge })
    }

    fn to_table(&self) -> Table {
        Table::from_iter([(
            String::from("merge"),
            Value::String(choice_name(self.merge, &MERGE_MODES).to_owned()),
        )])
    }
}

/// How a later layer combines with an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
    /// Tables merge key by key; arrays and scalars replace.
    Deep,
    /// A domain from a later layer replaces the whole earlier domain.
    Shallow,
}

/// `[commits]`: how commits are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commits {
    preset: Preset,
}

const PRESETS: [(&str, Preset); 2] = [
    ("conventionalcommits", Preset::ConventionalCommits),
    ("angular", Preset::Angular),
];

impl Commits {
    /// `commits.preset`
    #[must_use]
    pub fn preset(&self) -> Preset {
        self.preset
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let preset = fields
            .take("preset")
            .map(|(path, value)| choice(&path, &value, &PRESETS))
            .transpose()?
            .unwrap_or(Preset::ConventionalCommits);
        fields.finish(&["preset"])?;
        Ok(Self { preset })
    }

    fn to_table(&self) -> Table {
        Table::from_iter([(
            String::from("preset"),
            Value::String(choice_name(self.preset, &PRESETS).to_owned()),
        )])
    }
}

/// The commit convention shared by the analyzer and notes plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// `conventionalcommits`: `!` marks a breaking change.
    ConventionalCommits,
    /// `angular`
    Angular,
}

/// `[version]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDomain {
    initial: Version,
    zero: ZeroLevels,
}

impl VersionDomain {
    /// `version.initial`: the first release version.
    #[must_use]
    pub fn initial(&self) -> &Version {
        &self.initial
    }

    /// `version.zero`: how the analyzer's levels map while the version is 0.x.
    #[must_use]
    pub fn zero(&self) -> ZeroLevels {
        self.zero
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let initial = fields
            .take("initial")
            .map(|(path, value)| parse_version(&path, &value))
            .transpose()?
            .unwrap_or(Version::new(1, 0, 0));
        if !initial.build.is_empty() {
            return Err(ConfigError::invalid(
                "version.initial",
                &Value::String(initial.to_string()),
                "build metadata isn't allowed",
            ));
        }
        let zero = match fields.take("zero") {
            Some((path, value)) => ZeroLevels::parse(Fields::from_value(&path, value)?)?,
            None => ZeroLevels::DEFAULT,
        };
        fields.finish(&["initial", "zero"])?;
        Ok(Self { initial, zero })
    }

    fn to_table(&self) -> Table {
        Table::from_iter([
            (
                String::from("initial"),
                Value::String(self.initial.to_string()),
            ),
            (String::from("zero"), Value::Table(self.zero.to_table())),
        ])
    }
}

fn parse_version(path: &str, value: &Value) -> Result<Version, ConfigError> {
    let text = string(path, value)?;
    Version::parse(&text).map_err(|_| {
        ConfigError::invalid(
            path,
            value,
            "expected a full SemVer version such as `1.0.0`",
        )
    })
}

/// The release level each kind of change gets on 0.x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroLevels {
    breaking: Level,
    feature: Level,
    fix: Level,
}

const LEVELS: [(&str, Level); 3] = [
    ("major", Level::Major),
    ("minor", Level::Minor),
    ("patch", Level::Patch),
];

impl ZeroLevels {
    const DEFAULT: Self = Self {
        breaking: Level::Minor,
        feature: Level::Patch,
        fix: Level::Patch,
    };

    /// `version.zero.breaking`
    #[must_use]
    pub fn breaking(&self) -> Level {
        self.breaking
    }

    /// `version.zero.feature`
    #[must_use]
    pub fn feature(&self) -> Level {
        self.feature
    }

    /// `version.zero.fix`
    #[must_use]
    pub fn fix(&self) -> Level {
        self.fix
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let mut level = |name: &str, default: Level| {
            fields
                .take(name)
                .map(|(path, value)| choice(&path, &value, &LEVELS))
                .transpose()
                .map(|level| level.unwrap_or(default))
        };
        let levels = Self {
            breaking: level("breaking", Self::DEFAULT.breaking)?,
            feature: level("feature", Self::DEFAULT.feature)?,
            fix: level("fix", Self::DEFAULT.fix)?,
        };
        fields.finish(&["breaking", "feature", "fix"])?;
        Ok(levels)
    }

    fn to_table(self) -> Table {
        let name = |level: Level| Value::String(choice_name(level, &LEVELS).to_owned());
        Table::from_iter([
            (String::from("breaking"), name(self.breaking)),
            (String::from("feature"), name(self.feature)),
            (String::from("fix"), name(self.fix)),
        ])
    }
}

/// A release level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// `major`
    Major,
    /// `minor`
    Minor,
    /// `patch`
    Patch,
}

/// `[tags]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    format: TagFormat,
    metadata: Option<Template>,
}

impl Tags {
    /// `tags.format`
    #[must_use]
    pub fn format(&self) -> &TagFormat {
        &self.format
    }

    /// `tags.metadata`
    #[must_use]
    pub fn metadata(&self) -> Option<&Template> {
        self.metadata.as_ref()
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let format = fields
            .take("format")
            .map(|(path, value)| TagFormat::parse(&path, &value))
            .transpose()?
            .unwrap_or_else(TagFormat::default_format);
        let metadata = fields
            .take("metadata")
            .map(|(path, value)| string(&path, &value).map(Template::unchecked))
            .transpose()?;
        fields.finish(&["format", "metadata"])?;
        Ok(Self { format, metadata })
    }

    fn to_table(&self) -> Table {
        let mut table = Table::from_iter([(
            String::from("format"),
            Value::String(self.format.as_str().to_owned()),
        )]);
        if let Some(metadata) = &self.metadata {
            table.insert(
                String::from("metadata"),
                Value::String(metadata.as_str().to_owned()),
            );
        }
        table
    }
}

/// `[steps]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steps {
    plugins: Vec<PluginName>,
    orders: BTreeMap<Step, Vec<PluginName>>,
    success_errors: SuccessErrors,
}

const SUCCESS_ERRORS: [(&str, SuccessErrors); 2] =
    [("warn", SuccessErrors::Warn), ("fail", SuccessErrors::Fail)];

const BUNDLED_PLUGINS: [&str; 2] = ["commit-analyzer", "release-notes"];

impl Steps {
    /// `steps.plugins`: the enabled plugins in run order.
    #[must_use]
    pub fn plugins(&self) -> &[PluginName] {
        &self.plugins
    }

    /// `steps.<step>.order`, if set.
    #[must_use]
    pub fn order(&self, step: Step) -> Option<&[PluginName]> {
        self.orders.get(&step).map(Vec::as_slice)
    }

    /// `steps.success.errors`
    #[must_use]
    pub fn success_errors(&self) -> SuccessErrors {
        self.success_errors
    }

    fn is_enabled(&self, name: &PluginName) -> bool {
        self.plugins.contains(name)
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let plugins = match fields.take("plugins") {
            Some((path, value)) => parse_plugin_list(&path, value)?,
            None => BUNDLED_PLUGINS.map(PluginName::bundled).to_vec(),
        };
        let mut steps = Self {
            plugins,
            orders: BTreeMap::new(),
            success_errors: SuccessErrors::Warn,
        };
        for (path, name, value) in fields.drain() {
            let Some(step) = Step::from_name(&name) else {
                return Err(ConfigError::unknown(&path, &step_keys()));
            };
            let mut step_fields = Fields::from_value(&path, value)?;
            if let Some((order_path, order)) = step_fields.take("order") {
                let order = steps.parse_order(&order_path, order)?;
                steps.orders.insert(step, order);
            }
            if step == Step::Success
                && let Some((errors_path, errors)) = step_fields.take("errors")
            {
                steps.success_errors = choice(&errors_path, &errors, &SUCCESS_ERRORS)?;
            }
            step_fields.finish(if step == Step::Success {
                &["order", "errors"]
            } else {
                &["order"]
            })?;
        }
        Ok(steps)
    }

    fn parse_order(&self, path: &str, value: Value) -> Result<Vec<PluginName>, ConfigError> {
        array(path, value)?
            .into_iter()
            .enumerate()
            .map(|(position, item)| {
                let item_path = index(path, position);
                let name = PluginName::parse(&item_path, &item)?;
                if !self.is_enabled(&name) {
                    return Err(ConfigError::not_enabled(&item_path, &name));
                }
                Ok(name)
            })
            .collect()
    }

    fn to_table(&self) -> Table {
        let names = |names: &[PluginName]| {
            Value::Array(
                names
                    .iter()
                    .map(|name| Value::String(name.to_string()))
                    .collect(),
            )
        };
        let mut step_tables: BTreeMap<Step, Table> = BTreeMap::new();
        for (step, order) in &self.orders {
            step_tables
                .entry(*step)
                .or_default()
                .insert(String::from("order"), names(order));
        }
        step_tables.entry(Step::Success).or_default().insert(
            String::from("errors"),
            Value::String(choice_name(self.success_errors, &SUCCESS_ERRORS).to_owned()),
        );
        let mut table = Table::from_iter([(String::from("plugins"), names(&self.plugins))]);
        for (step, step_table) in step_tables {
            table.insert(step.as_str().to_owned(), Value::Table(step_table));
        }
        table
    }
}

fn parse_plugin_list(path: &str, value: Value) -> Result<Vec<PluginName>, ConfigError> {
    let mut names: Vec<PluginName> = Vec::new();
    for (position, item) in array(path, value)?.into_iter().enumerate() {
        let item_path = index(path, position);
        let name = PluginName::parse(&item_path, &item)?;
        if names.contains(&name) {
            return Err(ConfigError::invalid(
                &item_path,
                &item,
                "the plugin is already listed",
            ));
        }
        names.push(name);
    }
    Ok(names)
}

/// What a failing `success` step does to a published release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuccessErrors {
    /// Warnings, exit 0.
    Warn,
    /// The partial-failure exit code, still without rollback.
    Fail,
}

/// `[plugins.<name>]`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PluginConfig {
    version: Option<Version>,
    timeouts: BTreeMap<Step, Duration>,
    show_output: bool,
    options: serde_json::Map<String, serde_json::Value>,
}

impl PluginConfig {
    /// `version`: the pinned plugin version.
    #[must_use]
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    /// `timeouts.<step>`
    #[must_use]
    pub fn timeout(&self, step: Step) -> Option<Duration> {
        self.timeouts.get(&step).copied()
    }

    /// `show_output`
    #[must_use]
    pub fn show_output(&self) -> bool {
        self.show_output
    }

    /// The plugin's own options, validated by the plugin.
    #[must_use]
    pub fn options(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.options
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let version = fields
            .take("version")
            .map(|(path, value)| parse_version(&path, &value))
            .transpose()?;
        let mut timeouts = BTreeMap::new();
        if let Some((path, value)) = fields.take("timeouts") {
            for (step_path, name, duration) in Fields::from_value(&path, value)?.drain() {
                let Some(step) = Step::from_name(&name) else {
                    return Err(ConfigError::unknown(&step_path, &Step::NAMES));
                };
                timeouts.insert(step, values::parse_duration(&step_path, &duration)?);
            }
        }
        let show_output = match fields.take("show_output") {
            Some((_, Value::Boolean(flag))) => flag,
            Some((path, other)) => {
                return Err(ConfigError::invalid(
                    &path,
                    &other,
                    "expected `true` or `false`",
                ));
            }
            None => false,
        };
        let options = fields
            .drain()
            .into_iter()
            .map(|(path, name, value)| Ok((name, values::to_json(&path, &value)?)))
            .collect::<Result<_, ConfigError>>()?;
        Ok(Self {
            version,
            timeouts,
            show_output,
            options,
        })
    }

    fn to_table(&self) -> Table {
        let mut table: Table = self
            .options
            .iter()
            .map(|(name, value)| (name.clone(), values::from_json(value)))
            .collect();
        if let Some(version) = &self.version {
            table.insert(String::from("version"), Value::String(version.to_string()));
        }
        if !self.timeouts.is_empty() {
            table.insert(
                String::from("timeouts"),
                Value::Table(
                    self.timeouts
                        .iter()
                        .map(|(step, duration)| {
                            (
                                step.as_str().to_owned(),
                                Value::String(values::format_duration(*duration)),
                            )
                        })
                        .collect(),
                ),
            );
        }
        table.insert(
            String::from("show_output"),
            Value::Boolean(self.show_output),
        );
        table
    }
}

/// Every enabled plugin with its settings: its table, or defaults without one.
fn parse_plugins(
    mut fields: Fields,
    steps: &Steps,
) -> Result<BTreeMap<PluginName, PluginConfig>, ConfigError> {
    let mut plugins: BTreeMap<PluginName, PluginConfig> = BTreeMap::new();
    for (path, name, value) in fields.drain() {
        let name = PluginName::parse(&path, &Value::String(name))?;
        if !steps.is_enabled(&name) {
            return Err(ConfigError::not_enabled(&path, &name));
        }
        plugins.insert(
            name,
            PluginConfig::parse(Fields::from_value(&path, value)?)?,
        );
    }
    for name in steps.plugins() {
        plugins.entry(name.clone()).or_default();
    }
    Ok(plugins)
}

/// `[secrets]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secrets {
    mask_env: Vec<EnvName>,
}

impl Secrets {
    /// `secrets.mask_env`
    #[must_use]
    pub fn mask_env(&self) -> &[EnvName] {
        &self.mask_env
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let mask_env = match fields.take("mask_env") {
            Some((path, value)) => array(&path, value)?
                .iter()
                .enumerate()
                .map(|(position, item)| EnvName::parse(&index(&path, position), item))
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        };
        fields.finish(&["mask_env"])?;
        Ok(Self { mask_env })
    }

    fn to_table(&self) -> Table {
        Table::from_iter([(
            String::from("mask_env"),
            Value::Array(
                self.mask_env
                    .iter()
                    .map(|name| Value::String(name.as_str().to_owned()))
                    .collect(),
            ),
        )])
    }
}

/// The keys of one table, taken one by one; [`Fields::finish`] rejects any left over.
struct Fields {
    path: String,
    table: Table,
}

impl Fields {
    fn from_table(path: &str, table: Table) -> Self {
        Self {
            path: path.to_owned(),
            table,
        }
    }

    fn from_value(path: &str, value: Value) -> Result<Self, ConfigError> {
        match value {
            Value::Table(table) => Ok(Self::from_table(path, table)),
            other => Err(ConfigError::invalid(path, &other, "expected a table")),
        }
    }

    /// The key's full path and value, if present.
    fn take(&mut self, name: &str) -> Option<(String, Value)> {
        self.table
            .remove(name)
            .map(|value| (key(&self.path, name), value))
    }

    /// A domain table, or an empty one when it isn't set.
    fn domain(&mut self, name: &str) -> Result<Self, ConfigError> {
        match self.take(name) {
            Some((path, value)) => Self::from_value(&path, value),
            None => Ok(Self::from_table(&key(&self.path, name), Table::new())),
        }
    }

    /// Every remaining key with its full path, in key order.
    fn drain(&mut self) -> Vec<(String, String, Value)> {
        std::mem::take(&mut self.table)
            .into_iter()
            .map(|(name, value)| (key(&self.path, &name), name, value))
            .collect()
    }

    /// Rejects the first key left over; `known` are the keys valid here, for the hint.
    fn finish(self, known: &[&str]) -> Result<(), ConfigError> {
        match self.table.keys().next() {
            Some(name) => Err(ConfigError::unknown(&key(&self.path, name), known)),
            None => Ok(()),
        }
    }
}

/// The domains, in the order CONFIG.md lists them.
const DOMAINS: [&str; 8] = [
    "config", "commits", "version", "branches", "tags", "steps", "plugins", "secrets",
];

/// The keys valid directly under `[steps]`.
fn step_keys() -> Vec<&'static str> {
    std::iter::once("plugins").chain(Step::NAMES).collect()
}

/// `path.name`, or `name` at the top level.
fn key(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}.{name}")
    }
}

/// `path[position]`
fn index(path: &str, position: usize) -> String {
    format!("{path}[{position}]")
}

/// An invalid configuration: its code, the key path (`branches.rules[2].prerelease`) and what
/// is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    code: ErrorCode,
    path: String,
    message: String,
    help: Option<String>,
}

impl ConfigError {
    /// The key path, e.g. `tags.format`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    fn invalid(path: &str, value: &Value, problem: &str) -> Self {
        Self {
            code: CONFIG_INVALID_VALUE,
            path: path.to_owned(),
            message: format!("`{path}` = {value}: {problem}"),
            help: None,
        }
    }

    /// An unknown key, with a hint when a valid one in `known` is close.
    fn unknown(path: &str, known: &[&str]) -> Self {
        let (parent, name) = path.rsplit_once('.').unwrap_or(("", path));
        let help = suggest::closest(name, known)
            .map(|candidate| format!("Use `{}` instead.", key(parent, candidate)));
        Self {
            code: CONFIG_UNKNOWN_KEY,
            path: path.to_owned(),
            message: format!("unknown key `{path}`"),
            help,
        }
    }

    fn conflicting(path: &str, problem: &str) -> Self {
        Self {
            code: CONFIG_CONFLICTING_KEYS,
            path: path.to_owned(),
            message: format!("`{path}`: {problem}"),
            help: None,
        }
    }

    fn unsupported(path: &str) -> Self {
        Self {
            code: CONFIG_UNSUPPORTED_SECTION,
            path: path.to_owned(),
            message: format!("`{path}` isn't supported yet"),
            help: None,
        }
    }

    fn not_enabled(path: &str, name: &PluginName) -> Self {
        Self {
            code: CONFIG_PLUGIN_NOT_ENABLED,
            path: path.to_owned(),
            message: format!("`{path}`: plugin `{name}` isn't in `steps.plugins`"),
            help: None,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

impl ErrorInfo for ConfigError {
    fn code(&self) -> ErrorCode {
        self.code.clone()
    }

    fn help(&self) -> Option<String> {
        self.help.clone()
    }
}
