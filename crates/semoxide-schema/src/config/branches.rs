//! `branches.rules`: release, prerelease and maintenance rules (CONFIG §3).

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use toml::{Table, Value};

use super::schema::{Property, closed_table};
use super::values::{Template, string, string_schema};
use super::{ConfigError, Fields, index, key};

/// `[branches]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branches {
    rules: Vec<BranchRule>,
}

/// The keys of `[branches]`.
pub(super) const KEYS: [&str; 1] = ["rules"];

/// The keys of a release or prerelease rule.
pub(super) const RULE_KEYS: [&str; 3] = ["name", "prerelease", "channel"];

/// The keys of a maintenance rule.
pub(super) const MAINTENANCE_KEYS: [&str; 3] = ["maintenance", "range", "channel"];

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
        fields.finish(&RULE_KEYS)?;
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
        fields.finish(&MAINTENANCE_KEYS)?;
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

/// Whether `1.0.0-<id>.1` is a valid version, as upstream checks, with no `+` turning part of
/// `id` into build metadata.
fn is_prerelease_id(id: &str) -> bool {
    semver::Version::parse(&format!("1.0.0-{id}.1")).is_ok_and(|version| version.build.is_empty())
}

fn is_glob(name: &str) -> bool {
    name.contains(['*', '?', '[', '{'])
}

/// What [`is_range_shaped`] accepts.
const RANGE_PATTERN: &str = r"^[0-9]+(\.([0-9]+|[xX]))?\.[xX]$";

/// What [`pattern_prefix`] finds.
const RANGE_ENDING_PATTERN: &str = r"(N\.x|N\.x\.x|N\.N\.x)$";

/// A prerelease identifier as [`is_prerelease_id`] checks it, `{name}` counting as a letter.
const PRERELEASE_PATTERN: &str = r"^(0|[1-9][0-9]*|([0-9A-Za-z-]|\{name\})*([A-Za-z-]|\{name\})([0-9A-Za-z-]|\{name\})*)(\.(0|[1-9][0-9]*|([0-9A-Za-z-]|\{name\})*([A-Za-z-]|\{name\})([0-9A-Za-z-]|\{name\})*))*$";

/// Text whose only placeholder is `{name}`.
const NAME_PLACEHOLDER_PATTERN: &str = r"^([^{}]|\{name\})*$";

impl JsonSchema for Branches {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Branches")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let rule = generator.subschema_for::<BranchRule>();
        closed_table(
            "Which branches release, and how.",
            [Property::new(
                "rules",
                "The branch rules in order; the first release branch is the main line.",
                json_schema!({ "type": "array", "minItems": 1, "items": rule }),
            )],
            &Table::from_iter([(String::from("rules"), Self::default_rules().to_value())]),
        )
    }
}

/// `channel` of any rule.
const CHANNEL_DESCRIPTION: &str = "The release channel: `false` for the default channel, or a \
                                   name that may contain `{name}`. Unset: the default channel \
                                   for the first release branch, the branch name for the others.";

impl JsonSchema for BranchRule {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("BranchRule")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let name = json_schema!({ "type": "string", "not": { "pattern": RANGE_PATTERN } });
        let channel = json_schema!({
            "anyOf": [
                { "const": false },
                { "type": "string", "minLength": 1, "pattern": NAME_PLACEHOLDER_PATTERN },
            ],
        });
        let prerelease = json_schema!({
            "anyOf": [
                { "const": true },
                { "type": "string", "pattern": PRERELEASE_PATTERN },
            ],
        });
        let mut rule = closed_table(
            "A release branch, or a prerelease branch with `prerelease`.",
            [
                Property::new(
                    "name",
                    "The branch name or a glob; a name shaped like a range (`1.x`) is a \
                     maintenance rule instead.",
                    name.clone(),
                ),
                Property::new(
                    "prerelease",
                    "`true` for the branch name as the identifier, or an identifier that may \
                     contain `{name}`.",
                    prerelease,
                ),
                Property::new("channel", CHANNEL_DESCRIPTION, channel.clone()),
            ],
            &Table::new(),
        );
        rule.insert(String::from("required"), serde_json::json!(["name"]));
        let mut maintenance = closed_table(
            "A maintenance branch.",
            [
                Property::new(
                    "maintenance",
                    "A pattern ending in `N.x`, `N.x.x` or `N.N.x`, or a branch name with `range`.",
                    string_schema(),
                ),
                Property::new(
                    "range",
                    "The versions of a named maintenance branch: `1.x`, `1.x.x` or `1.2.x`.",
                    json_schema!({ "type": "string", "pattern": RANGE_PATTERN }),
                ),
                Property::new("channel", CHANNEL_DESCRIPTION, channel),
            ],
            &Table::new(),
        );
        maintenance.insert(String::from("required"), serde_json::json!(["maintenance"]));
        // A pattern ending in `N.x` takes its range from the branch name; any other needs one.
        maintenance.insert(
            String::from("if"),
            serde_json::json!({ "properties": { "maintenance": { "pattern": RANGE_ENDING_PATTERN } } }),
        );
        maintenance.insert(
            String::from("then"),
            serde_json::json!({ "not": { "required": ["range"] } }),
        );
        maintenance.insert(
            String::from("else"),
            serde_json::json!({ "required": ["range"] }),
        );
        // `if`/`then`/`else` picks one form, so an error points at the key inside it.
        json_schema!({
            "description": "A branch name (a release branch), a release or prerelease table, or a maintenance table.",
            "if": { "type": "string" },
            "then": name,
            "else": {
                "if": { "type": "object", "required": ["maintenance"] },
                "then": maintenance,
                "else": rule,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::test_support::Schema::{self, ParserOnly, Rejects};
    use crate::config::test_support::schema_verdict;
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use super::*;
    use crate::codes::{CONFIG_CONFLICTING_KEYS, CONFIG_INVALID_VALUE, CONFIG_UNKNOWN_KEY};
    use crate::config::Config;
    use crate::config::test_support::{describe_rules, loaded, rejection, schema_error_kinds};

    #[test]
    fn default_branch_rules_are_upstreams_set() {
        let config = loaded("");

        assert_eq!(
            describe_rules(&config),
            [
                "maintenance N.x range=- channel=-",
                "release master channel=-",
                "release main channel=-",
                "release next channel=-",
                "release next-major channel=-",
                "prerelease beta id=<branch> channel=-",
                "prerelease alpha id=<branch> channel=-",
            ]
        );
    }

    #[test]
    fn branch_rules_of_every_kind() {
        let config = loaded(
            r#"
[branches]
rules = [
  { maintenance = "N.x" },
  { maintenance = "release/N.N.x", channel = "{name}" },
  { maintenance = "legacy", range = "1.x", channel = false },
  "main",
  { name = "next", channel = "beta" },
  { name = "next-major", channel = false },
  { name = "beta", prerelease = true },
  { name = "release/*", prerelease = "rc", channel = "rc-{name}" },
  { name = "preview/*", prerelease = "pre-{name}" },
]
"#,
        );

        assert_eq!(
            describe_rules(&config),
            [
                "maintenance N.x range=- channel=-",
                "maintenance release/N.N.x range=- channel='{name}'",
                "maintenance legacy range=1.x channel=default",
                "release main channel=-",
                "release next channel='beta'",
                "release next-major channel=default",
                "prerelease beta id=<branch> channel=-",
                "prerelease release/* id='rc' channel='rc-{name}'",
                "prerelease preview/* id='pre-{name}' channel=-",
            ]
        );
    }

    #[rstest]
    #[case::dot("rc.1")]
    #[case::numeric("1")]
    #[case::hyphen("pre-release")]
    fn valid_prerelease_identifier(#[case] id: &str) {
        let config = loaded(&format!(
            "branches.rules = [{{ name = \"beta\", prerelease = \"{id}\" }}]"
        ));

        assert_eq!(
            describe_rules(&config),
            [format!("prerelease beta id='{id}' channel=-")]
        );
    }

    #[test]
    fn prerelease_true_on_a_glob_is_checked_after_expansion() {
        let config = loaded(r#"branches.rules = [{ name = "preview/*", prerelease = true }]"#);

        assert_eq!(
            describe_rules(&config),
            ["prerelease preview/* id=<branch> channel=-"]
        );
    }

    #[rstest]
    #[case::major("1.x", true)]
    #[case::major_x_x("1.x.x", true)]
    #[case::minor("1.2.x", true)]
    #[case::upper_x("1.X", true)]
    #[case::multi_digit("12.34.x", true)]
    #[case::release_branch("main", false)]
    #[case::full_version("1.2.3", false)]
    #[case::no_number("x.x", false)]
    #[case::prefixed("release/1.x", false)]
    fn default_maintenance_rule_matches_range_shaped_names(
        #[case] branch: &str,
        #[case] matches: bool,
    ) {
        let config = loaded("");

        let [BranchRule::Maintenance(rule), ..] = config.branches().rules() else {
            panic!("{:?}", describe_rules(&config));
        };
        assert_eq!(rule.matches(branch), matches, "{branch}");
    }

    #[rstest]
    #[case::prefixed("release/1.2.x", true)]
    #[case::without_prefix("1.2.x", false)]
    #[case::other_prefix("hotfix/1.2.x", false)]
    fn prefixed_maintenance_pattern(#[case] branch: &str, #[case] matches: bool) {
        let config = loaded(r#"branches.rules = [{ maintenance = "release/N.x" }]"#);

        let [BranchRule::Maintenance(rule)] = config.branches().rules() else {
            panic!("{:?}", describe_rules(&config));
        };
        assert_eq!(rule.matches(branch), matches, "{branch}");
    }

    #[rstest]
    #[case::exact("legacy", true)]
    #[case::other("legacy-2", false)]
    #[case::range_shaped("1.x", false)]
    fn named_maintenance_rule_matches_its_name_only(#[case] branch: &str, #[case] matches: bool) {
        let config = loaded(r#"branches.rules = [{ maintenance = "legacy", range = "1.x" }]"#);

        let [BranchRule::Maintenance(rule)] = config.branches().rules() else {
            panic!("{:?}", describe_rules(&config));
        };
        assert_eq!(rule.matches(branch), matches, "{branch}");
    }

    #[rstest]
    // branches: shape
    #[case::branches_not_a_list(
        r#"branches.rules = "main""#,
        CONFIG_INVALID_VALUE,
        "branches.rules",
        Rejects
    )]
    #[case::no_branch_rules("branches.rules = []", CONFIG_INVALID_VALUE, "branches.rules", Rejects)]
    #[case::branches_unknown_key(
        r#"branches.remote = "origin""#,
        CONFIG_UNKNOWN_KEY,
        "branches.remote",
        Rejects
    )]
    #[case::rule_unknown_key(
        r#"branches.rules = [{ name = "main", colour = "red" }]"#,
        CONFIG_UNKNOWN_KEY,
        "branches.rules[0].colour",
        Rejects
    )]
    #[case::rule_without_name(
        r#"branches.rules = [{ channel = "next" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0]",
        Rejects
    )]
    #[case::rule_not_a_string_or_table(
        "branches.rules = [1]",
        CONFIG_INVALID_VALUE,
        "branches.rules[0]",
        Rejects
    )]
    // branches: conflicting keys (the path names the extra key)
    #[case::name_and_maintenance(
        r#"branches.rules = [{ name = "beta", maintenance = "N.x" }]"#,
        CONFIG_CONFLICTING_KEYS,
        "branches.rules[0].maintenance",
        Rejects
    )]
    #[case::prerelease_on_maintenance(
        r#"branches.rules = [{ maintenance = "N.x", prerelease = true }]"#,
        CONFIG_CONFLICTING_KEYS,
        "branches.rules[0].prerelease",
        Rejects
    )]
    #[case::range_with_range_pattern(
        r#"branches.rules = [{ maintenance = "N.x", range = "1.x" }]"#,
        CONFIG_CONFLICTING_KEYS,
        "branches.rules[0].range",
        Rejects
    )]
    // branches: maintenance
    #[case::maintenance_without_range(
        r#"branches.rules = [{ maintenance = "legacy" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].maintenance",
        Rejects
    )]
    #[case::maintenance_pattern_not_at_end(
        r#"branches.rules = [{ maintenance = "N.x/legacy" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].maintenance",
        Rejects
    )]
    #[case::range_full_version(
        r#"branches.rules = [{ maintenance = "legacy", range = "1.0.0" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].range",
        Rejects
    )]
    #[case::range_semver_range(
        r#"branches.rules = [{ maintenance = "legacy", range = ">=1 <2" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].range",
        Rejects
    )]
    // branches: upstream-style maintenance entries
    #[case::range_shaped_string(
        r#"branches.rules = ["1.x"]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0]",
        Rejects
    )]
    #[case::range_shaped_name(
        r#"branches.rules = [{ name = "1.2.x" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].name",
        Rejects
    )]
    #[case::range_shaped_name_upper(
        r#"branches.rules = [{ name = "2.X" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].name",
        Rejects
    )]
    #[case::range_on_name(
        r#"branches.rules = [{ name = "legacy", range = "1.x" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].range",
        Rejects
    )]
    // branches: prerelease
    #[case::prerelease_false(
        r#"branches.rules = [{ name = "beta", prerelease = false }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        Rejects
    )]
    #[case::prerelease_invalid_char(
        r#"branches.rules = [{ name = "beta", prerelease = "rc!" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        Rejects
    )]
    #[case::prerelease_empty(
        r#"branches.rules = [{ name = "beta", prerelease = "" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        Rejects
    )]
    #[case::prerelease_leading_zero(
        r#"branches.rules = [{ name = "beta", prerelease = "01" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        Rejects
    )]
    #[case::prerelease_with_plus(
        r#"branches.rules = [{ name = "beta", prerelease = "beta+build" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        Rejects
    )]
    #[case::prerelease_true_on_invalid_name(
        r#"branches.rules = [{ name = "feature/x", prerelease = true }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        ParserOnly
    )]
    #[case::prerelease_true_on_name_with_plus(
        r#"branches.rules = [{ name = "beta+x", prerelease = true }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        ParserOnly
    )]
    #[case::prerelease_unknown_placeholder(
        r#"branches.rules = [{ name = "beta", prerelease = "{branch}" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].prerelease",
        Rejects
    )]
    // branches: channel
    #[case::channel_true(
        r#"branches.rules = ["main", { name = "next", channel = true }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[1].channel",
        Rejects
    )]
    #[case::channel_empty(
        r#"branches.rules = [{ name = "next", channel = "" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].channel",
        Rejects
    )]
    #[case::channel_unknown_placeholder(
        r#"branches.rules = [{ name = "next", channel = "{branch}" }]"#,
        CONFIG_INVALID_VALUE,
        "branches.rules[0].channel",
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
    #[case::bad_prerelease(
        r#"branches.rules = [{ name = "beta", prerelease = "rc!" }]"#,
        &[("/branches/rules/0/prerelease", "anyOf")]
    )]
    #[case::empty_channel(
        r#"branches.rules = [{ name = "next", channel = "" }]"#,
        &[("/branches/rules/0/channel", "anyOf")]
    )]
    #[case::maintenance_without_range(
        r#"branches.rules = [{ maintenance = "legacy" }]"#,
        &[("/branches/rules/0", "required")]
    )]
    #[case::range_with_range_pattern(
        r#"branches.rules = [{ maintenance = "N.x", range = "1.x" }]"#,
        &[("/branches/rules/0", "not")]
    )]
    #[case::range_shaped_string(r#"branches.rules = ["1.x"]"#, &[("/branches/rules/0", "not")])]
    #[case::rule_unknown_key(
        r#"branches.rules = [{ name = "main", colour = "red" }]"#,
        &[("/branches/rules/0", "additionalProperties")]
    )]
    fn a_mistake_in_a_branch_rule_is_reported_at_its_key(
        #[case] text: &str,
        #[case] errors: &[(&str, &str)],
    ) {
        let expected: Vec<(String, String)> = errors
            .iter()
            .map(|(path, keyword)| ((*path).to_owned(), (*keyword).to_owned()))
            .collect();

        assert_eq!(schema_error_kinds(&text.parse().unwrap()), expected);
    }

    #[test]
    fn prerelease_false_says_to_drop_the_key() {
        let table = r#"branches.rules = [{ name = "beta", prerelease = false }]"#
            .parse::<toml::Table>()
            .unwrap();

        let error = Config::from_table(table);

        assert!(error.is_err());
        let message = error.unwrap_err().to_string();
        assert!(
            message.contains("a release branch has no `prerelease` key"),
            "{message}"
        );
    }
}
