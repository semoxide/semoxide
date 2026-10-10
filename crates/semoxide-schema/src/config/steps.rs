//! `[steps]`: the enabled plugins, their order per step and how `success` errors end a run.

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use toml::{Table, Value};

use super::schema::closed_table;
use super::values::{
    PluginName, Step, array, choice, choice_name, choice_schema, plugin_name_schema,
};
use super::{ConfigError, Fields, index};

/// `[steps]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steps {
    plugins: Vec<PluginName>,
    orders: BTreeMap<Step, Vec<PluginName>>,
    success_errors: SuccessErrors,
}

const SUCCESS_ERRORS: [(&str, SuccessErrors); 2] =
    [("warn", SuccessErrors::Warn), ("fail", SuccessErrors::Fail)];

/// The keys of `steps.<step>`.
pub(super) const STEP_KEYS: [&str; 1] = ["order"];

/// The keys of `steps.success`.
pub(super) const SUCCESS_KEYS: [&str; 2] = ["order", "errors"];

const BUNDLED_PLUGINS: [&str; 2] = ["commit-analyzer", "release-notes"];

impl Steps {
    pub(super) fn defaults() -> Self {
        Self {
            plugins: BUNDLED_PLUGINS.map(PluginName::bundled).to_vec(),
            orders: BTreeMap::new(),
            success_errors: SuccessErrors::Warn,
        }
    }

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

    pub(super) fn is_enabled(&self, name: &PluginName) -> bool {
        self.plugins.contains(name)
    }

    pub(super) fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
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
                &SUCCESS_KEYS
            } else {
                &STEP_KEYS
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

    pub(super) fn to_table(&self) -> Table {
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

/// The keys valid directly under `[steps]`.
pub(super) fn step_keys() -> Vec<&'static str> {
    std::iter::once("plugins").chain(Step::NAMES).collect()
}

/// What a failing `success` step does to a published release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuccessErrors {
    /// Warnings, exit 0.
    Warn,
    /// The partial-failure exit code, still without rollback.
    Fail,
}

impl JsonSchema for Steps {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Steps")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let defaults = Self::defaults().to_table();
        let name = plugin_name_schema();
        let mut properties = vec![(
            "plugins",
            json_schema!({ "type": "array", "items": name, "uniqueItems": true }),
        )];
        for step in Step::NAMES {
            let mut keys = vec![("order", json_schema!({ "type": "array", "items": name }))];
            if step == Step::Success.as_str() {
                keys.push(("errors", choice_schema(&SUCCESS_ERRORS)));
            }
            let step_defaults = defaults
                .get(step)
                .and_then(Value::as_table)
                .cloned()
                .unwrap_or_default();
            properties.push((step, closed_table(keys, &step_defaults)));
        }
        closed_table(properties, &defaults)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::test_support::Schema::{self, ParserOnly, Rejects};
    use crate::config::test_support::schema_verdict;
    use rstest::rstest;
    use semoxide_error::{ErrorCode, ErrorInfo};

    use super::*;
    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_PLUGIN_NOT_ENABLED, CONFIG_UNKNOWN_KEY};
    use crate::config::Config;
    use crate::config::test_support::{loaded, plugins, rejection};

    #[test]
    fn steps() {
        let config = loaded(
            r#"
[steps]
plugins = ["commit-analyzer", "release-notes", "git", "github", "npm"]
publish.order = ["npm", "github"]
success.order = ["github"]
success.errors = "fail"
"#,
        );

        assert_eq!(
            config.steps().plugins(),
            plugins(&["commit-analyzer", "release-notes", "git", "github", "npm"])
        );
        assert_eq!(
            config.steps().order(Step::Publish),
            Some(plugins(&["npm", "github"]).as_slice())
        );
        assert_eq!(
            config.steps().order(Step::Success),
            Some(plugins(&["github"]).as_slice())
        );
        assert_eq!(config.steps().order(Step::Prepare), None);
        assert_eq!(config.steps().success_errors(), SuccessErrors::Fail);
    }

    #[rstest]
    #[case::warn("warn", SuccessErrors::Warn)]
    #[case::fail("fail", SuccessErrors::Fail)]
    fn success_errors(#[case] value: &str, #[case] expected: SuccessErrors) {
        let config = loaded(&format!("steps.success.errors = \"{value}\""));

        assert_eq!(config.steps().success_errors(), expected);
    }

    #[rstest]
    #[case::verify_conditions("verify_conditions", Step::VerifyConditions)]
    #[case::analyze_commits("analyze_commits", Step::AnalyzeCommits)]
    #[case::verify_release("verify_release", Step::VerifyRelease)]
    #[case::generate_notes("generate_notes", Step::GenerateNotes)]
    #[case::prepare("prepare", Step::Prepare)]
    #[case::publish("publish", Step::Publish)]
    #[case::add_channel("add_channel", Step::AddChannel)]
    #[case::success("success", Step::Success)]
    #[case::fail("fail", Step::Fail)]
    fn every_step_takes_an_order(#[case] name: &str, #[case] step: Step) {
        let config = loaded(&format!("steps.{name}.order = [\"release-notes\"]"));

        assert_eq!(
            config.steps().order(step),
            Some(plugins(&["release-notes"]).as_slice())
        );
    }

    #[rstest]
    // steps
    #[case::plugins_not_a_list(
        r#"steps.plugins = "git""#,
        CONFIG_INVALID_VALUE,
        "steps.plugins",
        Rejects
    )]
    #[case::plugin_name_uppercase(
        r#"steps.plugins = ["GitHub"]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[0]",
        Rejects
    )]
    #[case::plugin_name_leading_digit(
        r#"steps.plugins = ["3s"]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[0]",
        Rejects
    )]
    #[case::plugin_name_leading_dash(
        r#"steps.plugins = ["-git"]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[0]",
        Rejects
    )]
    #[case::plugin_name_trailing_dash(
        r#"steps.plugins = ["git-"]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[0]",
        Rejects
    )]
    #[case::plugin_name_double_dash(
        r#"steps.plugins = ["git--hub"]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[0]",
        Rejects
    )]
    #[case::plugin_name_underscore(
        r#"steps.plugins = ["git_hub"]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[0]",
        Rejects
    )]
    #[case::plugin_name_empty(
        r#"steps.plugins = [""]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[0]",
        Rejects
    )]
    #[case::duplicate_plugin(
        r#"steps.plugins = ["git", "git"]"#,
        CONFIG_INVALID_VALUE,
        "steps.plugins[1]",
        Rejects
    )]
    #[case::unknown_step(
        r#"steps.deploy.order = ["git"]"#,
        CONFIG_UNKNOWN_KEY,
        "steps.deploy",
        Rejects
    )]
    #[case::step_unknown_key(
        "steps.publish.parallel = true",
        CONFIG_UNKNOWN_KEY,
        "steps.publish.parallel",
        Rejects
    )]
    #[case::order_of_disabled_plugin(
        r#"steps.publish.order = ["npm"]"#,
        CONFIG_PLUGIN_NOT_ENABLED,
        "steps.publish.order[0]",
        ParserOnly
    )]
    #[case::order_invalid_name(
        r#"steps.publish.order = ["Git"]"#,
        CONFIG_INVALID_VALUE,
        "steps.publish.order[0]",
        Rejects
    )]
    #[case::errors_on_another_step(
        r#"steps.publish.errors = "fail""#,
        CONFIG_UNKNOWN_KEY,
        "steps.publish.errors",
        Rejects
    )]
    #[case::success_errors(
        r#"steps.success.errors = "ignore""#,
        CONFIG_INVALID_VALUE,
        "steps.success.errors",
        Rejects
    )]
    fn invalid_config_is_rejected(
        #[case] text: &str,
        #[case] code: ErrorCode,
        #[case] path: &str,
        #[case] schema: Schema,
    ) {
        assert_eq!(rejection(text), Err((code, path.to_owned())));
        assert_eq!(schema_verdict(text), schema, "{text}");
    }

    #[rstest]
    #[case::success_has_errors(
        r#"steps.success.errorss = "fail""#,
        Some("Use `steps.success.errors` instead.")
    )]
    #[case::other_steps_have_no_errors(r#"steps.publish.errorss = "fail""#, None)]
    fn errors_is_suggested_only_on_the_success_step(
        #[case] text: &str,
        #[case] help: Option<&str>,
    ) {
        let table = text.parse::<toml::Table>().unwrap();

        let error = Config::from_table(table);

        assert!(error.is_err());
        assert_eq!(error.unwrap_err().help().as_deref(), help);
    }
}
