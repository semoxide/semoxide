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

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use super::*;
    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_PLUGIN_NOT_ENABLED, CONFIG_UNKNOWN_KEY};
    use crate::config::test_support::{loaded, plugin, rejection};

    #[test]
    fn enabled_plugins_without_a_table_have_default_settings() {
        let config = loaded("");

        for name in ["commit-analyzer", "release-notes"] {
            let settings = config.plugin(&plugin(name));
            assert!(settings.is_some(), "{name}");
            let settings = settings.unwrap();
            assert_eq!(settings.version(), None);
            assert_eq!(settings.timeout(Step::Publish), None);
            assert!(!settings.show_output());
            assert_eq!(settings.options(), &serde_json::Map::new());
        }
        assert_eq!(config.plugin(&plugin("github")), None);
    }

    #[test]
    fn plugin_config() {
        let config = loaded(
            r#"
steps.plugins = ["commit-analyzer", "github"]

[plugins.github]
version = "1.4.2"
timeouts.publish = "30m"
timeouts.success = "90s"
timeouts.prepare = "1h"
show_output = true
assets = ["dist/*.tar.gz"]
draft = { enabled = true, limit = 3, ratio = 0.5 }
"#,
        );

        let github = config.plugin(&plugin("github"));
        assert!(github.is_some());
        let github = github.unwrap();
        assert_eq!(github.version(), Some(&Version::new(1, 4, 2)));
        assert_eq!(github.timeout(Step::Publish), Some(Duration::from_mins(30)));
        assert_eq!(github.timeout(Step::Success), Some(Duration::from_secs(90)));
        assert_eq!(github.timeout(Step::Prepare), Some(Duration::from_hours(1)));
        assert_eq!(github.timeout(Step::Fail), None);
        assert!(github.show_output());
        assert_eq!(
            serde_json::Value::Object(github.options().clone()),
            serde_json::json!({
                "assets": ["dist/*.tar.gz"],
                "draft": { "enabled": true, "limit": 3, "ratio": 0.5 },
            })
        );
    }

    #[test]
    fn unknown_plugin_option_keys_are_left_to_the_plugin() {
        let config = loaded("[plugins.release-notes]\nversion_from = \"tags\"");

        let notes = config.plugin(&plugin("release-notes"));
        assert!(notes.is_some());
        assert_eq!(
            notes.unwrap().options().get("version_from"),
            Some(&serde_json::json!("tags"))
        );
    }

    #[rstest]
    // plugins
    #[case::options_of_disabled_plugin(
        "[plugins.npm]\ntag = \"next\"",
        CONFIG_PLUGIN_NOT_ENABLED,
        "plugins.npm"
    )]
    #[case::plugin_version(
        "[plugins.release-notes]\nversion = \"1.4\"",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.version"
    )]
    #[case::timeout_words(
        "[plugins.release-notes]\ntimeouts.publish = \"1 hour\"",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.timeouts.publish"
    )]
    #[case::timeout_zero(
        "[plugins.release-notes]\ntimeouts.publish = \"0s\"",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.timeouts.publish"
    )]
    #[case::timeout_no_unit(
        "[plugins.release-notes]\ntimeouts.publish = \"30\"",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.timeouts.publish"
    )]
    #[case::timeout_integer(
        "[plugins.release-notes]\ntimeouts.publish = 30",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.timeouts.publish"
    )]
    #[case::timeout_days(
        "[plugins.release-notes]\ntimeouts.publish = \"30d\"",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.timeouts.publish"
    )]
    #[case::timeout_fraction(
        "[plugins.release-notes]\ntimeouts.publish = \"1.5h\"",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.timeouts.publish"
    )]
    #[case::timeout_unknown_step(
        "[plugins.release-notes]\ntimeouts.deploy = \"1h\"",
        CONFIG_UNKNOWN_KEY,
        "plugins.release-notes.timeouts.deploy"
    )]
    #[case::show_output_not_bool(
        "[plugins.release-notes]\nshow_output = \"yes\"",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.show_output"
    )]
    #[case::option_datetime(
        "[plugins.release-notes]\nsince = 2026-01-01",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.since"
    )]
    #[case::option_datetime_in_array(
        "[plugins.release-notes]\nwindows = [{ start = 2026-01-01T00:00:00Z }]",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.windows[0].start"
    )]
    #[case::option_datetime_in_table(
        "[plugins.release-notes]\nrange = { from = 08:00:00 }",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.range.from"
    )]
    #[case::option_nan(
        "[plugins.release-notes]\nratio = nan",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.ratio"
    )]
    #[case::option_inf_in_array(
        "[plugins.release-notes]\nlimits = [1.0, inf]",
        CONFIG_INVALID_VALUE,
        "plugins.release-notes.limits[1]"
    )]
    fn invalid_config_is_rejected(#[case] text: &str, #[case] code: ErrorCode, #[case] path: &str) {
        assert_eq!(rejection(text), Err((code, path.to_owned())));
    }
}
