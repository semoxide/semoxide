//! `semoxide.toml` as typed configuration, one type per domain (CONFIG.md).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use semoxide_error::{ErrorCode, ErrorInfo};
use semver::Version;

use crate::codes::CONFIG_INVALID_VALUE;

#[cfg(test)]
mod tests;

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
    pub fn from_table(table: toml::Table) -> Result<Self, ConfigError> {
        drop(table);
        Err(ConfigError {
            code: CONFIG_INVALID_VALUE,
            path: String::new(),
            message: String::from("not implemented"),
        })
    }

    /// The configuration as a TOML table that [`Config::from_table`] reads back unchanged.
    #[must_use]
    pub fn to_table(&self) -> toml::Table {
        toml::Table::new()
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

    /// `[plugins.<name>]` of one plugin.
    #[must_use]
    pub fn plugin(&self, name: &PluginName) -> Option<&PluginConfig> {
        self.plugins.get(name)
    }

    /// `[secrets]`
    #[must_use]
    pub fn secrets(&self) -> &Secrets {
        &self.secrets
    }

    fn stub() -> Self {
        Self {
            layering: ConfigDomain {
                merge: MergeMode::Shallow,
            },
            commits: Commits {
                preset: Preset::Angular,
            },
            version: VersionDomain {
                initial: Version::new(0, 0, 0),
                zero: ZeroLevels {
                    breaking: Level::Major,
                    feature: Level::Major,
                    fix: Level::Major,
                },
            },
            branches: Branches { rules: Vec::new() },
            tags: Tags {
                format: TagFormat(String::new()),
                metadata: None,
            },
            steps: Steps {
                plugins: Vec::new(),
                orders: BTreeMap::new(),
                success_errors: SuccessErrors::Fail,
            },
            plugins: BTreeMap::new(),
            secrets: Secrets {
                mask_env: Vec::new(),
            },
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::stub()
    }
}

/// `[config]`: how the configuration itself is assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDomain {
    merge: MergeMode,
}

impl ConfigDomain {
    /// `config.merge`
    #[must_use]
    pub fn merge(&self) -> MergeMode {
        self.merge
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

impl Commits {
    /// `commits.preset`
    #[must_use]
    pub fn preset(&self) -> Preset {
        self.preset
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
}

/// The release level each kind of change gets on 0.x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroLevels {
    breaking: Level,
    feature: Level,
    fix: Level,
}

impl ZeroLevels {
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

/// `[branches]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branches {
    rules: Vec<BranchRule>,
}

impl Branches {
    /// `branches.rules`, in order.
    #[must_use]
    pub fn rules(&self) -> &[BranchRule] {
        &self.rules
    }
}

/// One entry of `branches.rules`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchRule {
    /// `"main"` or `{ name = "main", channel = … }`
    Release(ReleaseRule),
    /// `{ name = "beta", prerelease = true }`
    Prerelease(PrereleaseRule),
    /// `{ maintenance = "N.x" }`
    Maintenance(MaintenanceRule),
}

/// A release branch rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRule {
    name: String,
    channel: Option<Channel>,
}

impl ReleaseRule {
    /// The branch name or glob.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The channel, or `None` for the default rule (CONFIG §3).
    #[must_use]
    pub fn channel(&self) -> Option<&Channel> {
        self.channel.as_ref()
    }
}

/// A prerelease branch rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrereleaseRule {
    name: String,
    prerelease: Prerelease,
    channel: Option<Channel>,
}

impl PrereleaseRule {
    /// The branch name or glob.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The prerelease identifier.
    #[must_use]
    pub fn prerelease(&self) -> &Prerelease {
        &self.prerelease
    }

    /// The channel, or `None` for the default rule (CONFIG §3).
    #[must_use]
    pub fn channel(&self) -> Option<&Channel> {
        self.channel.as_ref()
    }
}

/// A maintenance branch rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceRule {
    pattern: String,
    range: Option<String>,
    channel: Option<Channel>,
}

impl MaintenanceRule {
    /// The pattern, e.g. `N.x` or `release/N.x`, or a plain name with a [`range`](Self::range).
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Whether a branch name falls under this rule: for a pattern, its prefix followed by a
    /// range-shaped name (`1.x`, `1.x.x`, `1.2.x`, `x` in any case); for a name with
    /// [`range`](Self::range), that exact name.
    #[must_use]
    pub fn matches(&self, branch: &str) -> bool {
        let _ = branch;
        false
    }

    /// The explicit range, e.g. `1.x`, for a name without `N.x`.
    #[must_use]
    pub fn range(&self) -> Option<&str> {
        self.range.as_deref()
    }

    /// The channel, or `None` for the default rule (CONFIG §3).
    #[must_use]
    pub fn channel(&self) -> Option<&Channel> {
        self.channel.as_ref()
    }
}

/// `channel` of a branch rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    /// `false`: the default channel.
    Default,
    /// A channel name, optionally with `{name}`.
    Named(Template),
}

/// `prerelease` of a branch rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prerelease {
    /// `true`: the branch name is the identifier.
    BranchName,
    /// An identifier, optionally with `{name}`.
    Id(Template),
}

/// A string with at most the `{name}` placeholder (branch rules) or a template checked later
/// (`tags.metadata`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template(String);

impl Template {
    /// The text as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
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
}

/// A tag format with exactly one `{version}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagFormat(String);

impl TagFormat {
    /// The format as written, e.g. `v{version}`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `[steps]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steps {
    plugins: Vec<PluginName>,
    orders: BTreeMap<Step, Vec<PluginName>>,
    success_errors: SuccessErrors,
}

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
}

/// A lifecycle step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    /// `verify_conditions`
    VerifyConditions,
    /// `analyze_commits`
    AnalyzeCommits,
    /// `verify_release`
    VerifyRelease,
    /// `generate_notes`
    GenerateNotes,
    /// `prepare`
    Prepare,
    /// `publish`
    Publish,
    /// `add_channel`
    AddChannel,
    /// `success`
    Success,
    /// `fail`
    Fail,
}

/// What a failing `success` step does to a published release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuccessErrors {
    /// Warnings, exit 0.
    Warn,
    /// The partial-failure exit code, still without rollback.
    Fail,
}

/// A plugin's short name, e.g. `github`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PluginName(String);

impl PluginName {
    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for PluginName {
    type Err = ConfigError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(Self(name.to_owned()))
    }
}

/// `[plugins.<name>]`
#[derive(Debug, Clone, PartialEq)]
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
}

/// An environment variable name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvName(String);

impl EnvName {
    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An invalid configuration: its code, the key path (`branches.rules[2].prerelease`) and what
/// is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    code: ErrorCode,
    path: String,
    message: String,
}

impl ConfigError {
    /// The key path, e.g. `tags.format`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
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
}
