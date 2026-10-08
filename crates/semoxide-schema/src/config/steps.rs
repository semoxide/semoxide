//! `[steps]`: the enabled plugins, their order per step and how `success` errors end a run.

use std::collections::BTreeMap;

use toml::{Table, Value};

use super::values::{PluginName, Step, array, choice, choice_name};
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
fn step_keys() -> Vec<&'static str> {
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
