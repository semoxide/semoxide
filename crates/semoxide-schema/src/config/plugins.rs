//! `[plugins.<name>]`: per-plugin settings and the plugin's own options.

use std::collections::BTreeMap;
use std::time::Duration;

use semver::Version;
use toml::{Table, Value};

use super::steps::Steps;
use super::values::{self, PluginName, Step};
use super::version::parse_version;
use super::{ConfigError, Fields};

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

    pub(super) fn to_table(&self) -> Table {
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
pub(super) fn parse_plugins(
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
