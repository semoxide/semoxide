//! `branches.rules`: release, prerelease and maintenance rules (CONFIG §3).

use toml::{Table, Value};

use super::values::{Template, string};
use super::{ConfigError, Fields, index, key};

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

    /// Upstream's default set.
    pub(super) fn default_rules() -> Self {
        let release = |name: &str| {
            BranchRule::Release(ReleaseRule {
                name: name.to_owned(),
                channel: None,
            })
        };
        let prerelease = |name: &str| {
            BranchRule::Prerelease(PrereleaseRule {
                name: name.to_owned(),
                prerelease: Prerelease::BranchName,
                channel: None,
            })
        };
        Self {
            rules: vec![
                BranchRule::Maintenance(MaintenanceRule {
                    pattern: String::from("N.x"),
                    range: None,
                    channel: None,
                }),
                release("master"),
                release("main"),
                release("next"),
                release("next-major"),
                prerelease("beta"),
                prerelease("alpha"),
            ],
        }
    }

    pub(super) fn parse(fields: &mut Fields) -> Result<Self, ConfigError> {
        let Some((path, value)) = fields.take("rules") else {
            return Ok(Self::default_rules());
        };
        let entries = super::values::array(&path, value)?;
        if entries.is_empty() {
            return Err(ConfigError::invalid(
                &path,
                &Value::Array(Vec::new()),
                "at least one branch rule is needed",
            ));
        }
        let rules = entries
            .into_iter()
            .enumerate()
            .map(|(position, entry)| BranchRule::parse(&index(&path, position), entry))
            .collect::<Result<_, _>>()?;
        Ok(Self { rules })
    }

    pub(super) fn to_value(&self) -> Value {
        Value::Array(self.rules.iter().map(BranchRule::to_value).collect())
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

const MAINTENANCE_HINT: &str = "maintenance branches are written `{ maintenance = \"N.x\" }` or `{ maintenance = \"<name>\", range = \"1.x\" }`";

impl BranchRule {
    fn parse(path: &str, value: Value) -> Result<Self, ConfigError> {
        match value {
            Value::String(name) => {
                if is_range_shaped(&name) {
                    return Err(ConfigError::invalid(
                        path,
                        &Value::String(name),
                        MAINTENANCE_HINT,
                    ));
                }
                Ok(Self::Release(ReleaseRule {
                    name,
                    channel: None,
                }))
            }
            Value::Table(table) => Self::parse_table(path, table),
            other => Err(ConfigError::invalid(
                path,
                &other,
                "expected a branch name or a table",
            )),
        }
    }

    fn parse_table(path: &str, table: Table) -> Result<Self, ConfigError> {
        if table.contains_key("maintenance") {
            if table.contains_key("name") {
                return Err(ConfigError::conflicting(
                    &key(path, "maintenance"),
                    "`maintenance` can't be combined with `name`",
                ));
            }
            if table.contains_key("prerelease") {
                return Err(ConfigError::conflicting(
                    &key(path, "prerelease"),
                    "a maintenance branch can't be a prerelease branch",
                ));
            }
            return MaintenanceRule::parse(path, table).map(Self::Maintenance);
        }
        if !table.contains_key("name") {
            return Err(ConfigError::invalid(
                path,
                &Value::Table(table),
                "a branch rule needs `name` or `maintenance`",
            ));
        }
        let mut fields = Fields::from_table(path, table);
        let Some((name_path, name_value)) = fields.take("name") else {
            return Err(ConfigError::invalid(
                path,
                &Value::Table(Table::new()),
                "missing `name`",
            ));
        };
        let name = string(&name_path, &name_value)?;
        if is_range_shaped(&name) {
            return Err(ConfigError::invalid(
                &name_path,
                &name_value,
                MAINTENANCE_HINT,
            ));
        }
        if let Some((range_path, range_value)) = fields.take("range") {
            return Err(ConfigError::invalid(
                &range_path,
                &range_value,
                MAINTENANCE_HINT,
            ));
        }
        let channel = take_channel(&mut fields)?;
        let rule = match fields.take("prerelease") {
            None => Self::Release(ReleaseRule { name, channel }),
            Some((prerelease_path, prerelease_value)) => {
                let prerelease = Prerelease::parse(&prerelease_path, &prerelease_value, &name)?;
                Self::Prerelease(PrereleaseRule {
                    name,
                    prerelease,
                    channel,
                })
            }
        };
        fields.finish()?;
        Ok(rule)
    }

    fn to_value(&self) -> Value {
        let mut table = Table::new();
        let channel = match self {
            Self::Release(rule) => {
                if rule.channel.is_none() {
                    return Value::String(rule.name.clone());
                }
                table.insert(String::from("name"), Value::String(rule.name.clone()));
                rule.channel.as_ref()
            }
            Self::Prerelease(rule) => {
                table.insert(String::from("name"), Value::String(rule.name.clone()));
                let prerelease = match &rule.prerelease {
                    Prerelease::BranchName => Value::Boolean(true),
                    Prerelease::Id(id) => Value::String(id.as_str().to_owned()),
                };
                table.insert(String::from("prerelease"), prerelease);
                rule.channel.as_ref()
            }
            Self::Maintenance(rule) => {
                table.insert(
                    String::from("maintenance"),
                    Value::String(rule.pattern.clone()),
                );
                if let Some(range) = &rule.range {
                    table.insert(String::from("range"), Value::String(range.clone()));
                }
                rule.channel.as_ref()
            }
        };
        if let Some(channel) = channel {
            table.insert(String::from("channel"), channel.to_value());
        }
        Value::Table(table)
    }
}

fn take_channel(fields: &mut Fields) -> Result<Option<Channel>, ConfigError> {
    fields
        .take("channel")
        .map(|(path, value)| Channel::parse(&path, &value))
        .transpose()
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

/// Pattern endings that stand for a range-shaped branch name, longest first.
const RANGE_ENDINGS: [&str; 3] = ["N.N.x", "N.x.x", "N.x"];

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
        match pattern_prefix(&self.pattern) {
            Some(prefix) => branch.strip_prefix(prefix).is_some_and(is_range_shaped),
            None => branch == self.pattern,
        }
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

    fn parse(path: &str, table: Table) -> Result<Self, ConfigError> {
        let mut fields = Fields::from_table(path, table);
        let Some((pattern_path, pattern_value)) = fields.take("maintenance") else {
            return Err(ConfigError::invalid(
                path,
                &Value::Table(Table::new()),
                "missing `maintenance`",
            ));
        };
        let pattern = string(&pattern_path, &pattern_value)?;
        let range = fields.take("range");
        let range = match (pattern_prefix(&pattern), range) {
            (Some(_), Some((range_path, _))) => {
                return Err(ConfigError::conflicting(
                    &range_path,
                    "the pattern already ends in `N.x`, so its range comes from the branch name",
                ));
            }
            (Some(_), None) => None,
            (None, None) => {
                return Err(ConfigError::invalid(
                    &pattern_path,
                    &pattern_value,
                    "must end in `N.x`, `N.x.x` or `N.N.x`, or come with a `range`",
                ));
            }
            (None, Some((range_path, range_value))) => {
                let range = string(&range_path, &range_value)?;
                if !is_range_shaped(&range) {
                    return Err(ConfigError::invalid(
                        &range_path,
                        &range_value,
                        "expected `1.x`, `1.x.x` or `1.2.x`",
                    ));
                }
                Some(range)
            }
        };
        let channel = take_channel(&mut fields)?;
        fields.finish()?;
        Ok(Self {
            pattern,
            range,
            channel,
        })
    }
}

/// The prefix of a pattern ending in `N.x`, `N.x.x` or `N.N.x`.
fn pattern_prefix(pattern: &str) -> Option<&str> {
    RANGE_ENDINGS
        .iter()
        .find_map(|ending| pattern.strip_suffix(ending))
}

/// `1.x`, `1.x.x` or `1.2.x`, `x` in any case: upstream's `^\d+(\.(\d+|x))?\.x$` (case-insensitive).
fn is_range_shaped(name: &str) -> bool {
    let is_number = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
    let is_x = |part: &str| part.eq_ignore_ascii_case("x");
    let parts: Vec<&str> = name.split('.').collect();
    match parts.as_slice() {
        [major, last] => is_number(major) && is_x(last),
        [major, middle, last] => {
            is_number(major) && (is_number(middle) || is_x(middle)) && is_x(last)
        }
        _ => false,
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

impl Channel {
    fn parse(path: &str, value: &Value) -> Result<Self, ConfigError> {
        match value {
            Value::Boolean(false) => Ok(Self::Default),
            Value::String(text) if text.is_empty() => Err(ConfigError::invalid(
                path,
                value,
                "use `channel = false` for the default channel",
            )),
            Value::String(_) => Template::with_name_placeholder(path, value).map(Self::Named),
            _ => Err(ConfigError::invalid(
                path,
                value,
                "expected a channel name, or `false` for the default channel",
            )),
        }
    }

    fn to_value(&self) -> Value {
        match self {
            Self::Default => Value::Boolean(false),
            Self::Named(name) => Value::String(name.as_str().to_owned()),
        }
    }
}

/// `prerelease` of a branch rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prerelease {
    /// `true`: the branch name is the identifier.
    BranchName,
    /// An identifier, optionally with `{name}`.
    Id(Template),
}

impl Prerelease {
    fn parse(path: &str, value: &Value, branch: &str) -> Result<Self, ConfigError> {
        match value {
            Value::Boolean(true) => {
                if !is_glob(branch) && !is_prerelease_id(branch) {
                    return Err(ConfigError::invalid(
                        path,
                        value,
                        "the branch name isn't a valid prerelease identifier; set one, e.g. `prerelease = \"beta\"`",
                    ));
                }
                Ok(Self::BranchName)
            }
            Value::Boolean(false) => Err(ConfigError::invalid(
                path,
                value,
                "a release branch has no `prerelease` key",
            )),
            Value::String(text) => {
                let id = Template::with_name_placeholder(path, value)?;
                if !is_prerelease_id(&text.replace("{name}", "x")) {
                    return Err(ConfigError::invalid(
                        path,
                        value,
                        "not a valid prerelease identifier (letters, digits, `-` and `.`, no leading zeros)",
                    ));
                }
                Ok(Self::Id(id))
            }
            _ => Err(ConfigError::invalid(
                path,
                value,
                "expected `true` or an identifier",
            )),
        }
    }
}

/// Whether `1.0.0-<id>.1` is a valid version, as upstream checks.
fn is_prerelease_id(id: &str) -> bool {
    semver::Version::parse(&format!("1.0.0-{id}.1")).is_ok()
}

fn is_glob(name: &str) -> bool {
    name.contains(['*', '?', '[', '{'])
}
