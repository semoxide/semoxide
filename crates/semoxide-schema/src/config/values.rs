//! Value types shared by several domains, each valid by construction, and the conversion of a
//! plugin's options to JSON.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use toml::Value;

use super::{ConfigError, index, key};

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

    /// Any text; minijinja checks it once templates exist.
    pub(super) fn unchecked(text: String) -> Self {
        Self(text)
    }

    /// Text whose only placeholder is `{name}`.
    pub(super) fn with_name_placeholder(path: &str, value: &Value) -> Result<Self, ConfigError> {
        let text = string(path, value)?;
        if !has_only_name_placeholders(&text) {
            return Err(ConfigError::invalid(
                path,
                value,
                "the only placeholder allowed is `{name}`",
            ));
        }
        Ok(Self(text))
    }
}

fn has_only_name_placeholders(text: &str) -> bool {
    let rest = text.replace("{name}", "");
    !rest.contains(['{', '}'])
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

    pub(super) fn default_format() -> Self {
        Self(String::from("v{version}"))
    }

    pub(super) fn parse(path: &str, value: &Value) -> Result<Self, ConfigError> {
        let text = string(path, value)?;
        if text.matches("{version}").count() != 1 {
            return Err(ConfigError::invalid(
                path,
                value,
                "must contain `{version}` exactly once",
            ));
        }
        Ok(Self(text))
    }
}

/// A plugin's short name, e.g. `github`: `[a-z][a-z0-9]*(-[a-z0-9]+)*`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PluginName(String);

impl PluginName {
    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn bundled(name: &str) -> Self {
        Self(name.to_owned())
    }

    pub(super) fn parse(path: &str, value: &Value) -> Result<Self, ConfigError> {
        let name = string(path, value)?;
        name.parse().map_err(|_| {
            ConfigError::invalid(
                path,
                value,
                "expected lowercase letters and digits with single dashes, starting with a letter",
            )
        })
    }
}

impl FromStr for PluginName {
    type Err = ConfigError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let mut parts = name.split('-');
        let starts_with_letter = name.starts_with(|c: char| c.is_ascii_lowercase());
        let parts_valid = parts.all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
        if !(starts_with_letter && parts_valid) {
            return Err(ConfigError::invalid(
                "",
                &Value::String(name.to_owned()),
                "not a plugin name",
            ));
        }
        Ok(Self(name.to_owned()))
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An environment variable name: `[A-Za-z_][A-Za-z0-9_]*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvName(String);

impl EnvName {
    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn parse(path: &str, value: &Value) -> Result<Self, ConfigError> {
        let name = string(path, value)?;
        let valid = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            return Err(ConfigError::invalid(
                path,
                value,
                "expected letters, digits and `_`, not starting with a digit",
            ));
        }
        Ok(Self(name))
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

impl Step {
    const ALL: [(&'static str, Self); 9] = [
        ("verify_conditions", Self::VerifyConditions),
        ("analyze_commits", Self::AnalyzeCommits),
        ("verify_release", Self::VerifyRelease),
        ("generate_notes", Self::GenerateNotes),
        ("prepare", Self::Prepare),
        ("publish", Self::Publish),
        ("add_channel", Self::AddChannel),
        ("success", Self::Success),
        ("fail", Self::Fail),
    ];

    /// The config names of all steps, in run order.
    pub(super) const NAMES: [&'static str; 9] = [
        "verify_conditions",
        "analyze_commits",
        "verify_release",
        "generate_notes",
        "prepare",
        "publish",
        "add_channel",
        "success",
        "fail",
    ];

    /// The step's config name, e.g. `verify_conditions`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, step)| *step == self)
            .map_or("", |(name, _)| name)
    }

    pub(super) fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, step)| *step)
    }
}

/// A timeout such as `90s`, `30m` or `1h`: a positive whole number and one unit.
pub(super) fn parse_duration(path: &str, value: &Value) -> Result<Duration, ConfigError> {
    let invalid = || {
        ConfigError::invalid(
            path,
            value,
            "expected a positive whole number followed by `s`, `m` or `h`, e.g. `30m`",
        )
    };
    let text = value.as_str().ok_or_else(invalid)?;
    let (unit_start, unit) = text.char_indices().last().ok_or_else(invalid)?;
    let number = text.get(..unit_start).ok_or_else(invalid)?;
    let seconds_per_unit = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3600,
        _ => return Err(invalid()),
    };
    // `parse` alone would accept a leading `+`; it rejects an empty number itself.
    if !number.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalid());
    }
    let count: u64 = number.parse().map_err(|_| invalid())?;
    let seconds = count.checked_mul(seconds_per_unit).ok_or_else(invalid)?;
    if seconds == 0 {
        return Err(invalid());
    }
    Ok(Duration::from_secs(seconds))
}

/// The shortest exact form of a timeout: `1h`, `30m` or `90s`.
pub(super) fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds.is_multiple_of(3600) {
        format!("{}h", seconds / 3600)
    } else if seconds.is_multiple_of(60) {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

/// A plugin option as JSON: TOML date-times and non-finite floats have no JSON form.
pub(super) fn to_json(path: &str, value: &Value) -> Result<serde_json::Value, ConfigError> {
    Ok(match value {
        Value::String(text) => serde_json::Value::String(text.clone()),
        Value::Integer(number) => serde_json::Value::from(*number),
        Value::Boolean(flag) => serde_json::Value::Bool(*flag),
        Value::Float(number) => serde_json::Number::from_f64(*number)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                ConfigError::invalid(path, value, "`nan` and `inf` have no JSON form")
            })?,
        Value::Datetime(_) => {
            return Err(ConfigError::invalid(
                path,
                value,
                "a TOML date-time has no JSON form; quote it as a string",
            ));
        }
        Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(position, item)| to_json(&index(path, position), item))
                .collect::<Result<_, _>>()?,
        ),
        Value::Table(table) => serde_json::Value::Object(
            table
                .iter()
                .map(|(name, item)| Ok((name.clone(), to_json(&key(path, name), item)?)))
                .collect::<Result<_, ConfigError>>()?,
        ),
    })
}

/// A JSON option back as TOML. Options always come from TOML, so `null` never appears; it is
/// written as an empty string to keep the conversion total.
pub(super) fn from_json(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::String(String::new()),
        serde_json::Value::Bool(flag) => Value::Boolean(*flag),
        serde_json::Value::Number(number) => number
            .as_i64()
            .map(Value::Integer)
            .or_else(|| number.as_f64().map(Value::Float))
            .unwrap_or(Value::Integer(0)),
        serde_json::Value::String(text) => Value::String(text.clone()),
        serde_json::Value::Array(items) => Value::Array(items.iter().map(from_json).collect()),
        serde_json::Value::Object(map) => Value::Table(
            map.iter()
                .map(|(name, item)| (name.clone(), from_json(item)))
                .collect(),
        ),
    }
}

/// The value as a string.
pub(super) fn string(path: &str, value: &Value) -> Result<String, ConfigError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ConfigError::invalid(path, value, "expected a string"))
}

/// The value as an array.
pub(super) fn array(path: &str, value: Value) -> Result<Vec<Value>, ConfigError> {
    match value {
        Value::Array(items) => Ok(items),
        other => Err(ConfigError::invalid(path, &other, "expected an array")),
    }
}

/// One of the named variants of a string-valued enum.
pub(super) fn choice<T: Copy>(
    path: &str,
    value: &Value,
    variants: &[(&str, T)],
) -> Result<T, ConfigError> {
    let names: Vec<String> = variants
        .iter()
        .map(|(name, _)| format!("`{name}`"))
        .collect();
    let problem = format!("expected one of {}", names.join(", "));
    let text = value
        .as_str()
        .ok_or_else(|| ConfigError::invalid(path, value, &problem))?;
    variants
        .iter()
        .find(|(name, _)| *name == text)
        .map(|(_, variant)| *variant)
        .ok_or_else(|| ConfigError::invalid(path, value, &problem))
}

/// The name of an enum variant in `variants`.
pub(super) fn choice_name<T: Copy + PartialEq>(
    variant: T,
    variants: &[(&'static str, T)],
) -> &'static str {
    variants
        .iter()
        .find(|(_, known)| *known == variant)
        .map_or("", |(name, _)| name)
}
