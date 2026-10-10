//! `[config]`: how the configuration itself is assembled.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use toml::{Table, Value};

use super::schema::closed_table;
use super::values::{choice, choice_name, choice_schema};
use super::{ConfigError, Fields};

/// `[config]`: how the configuration itself is assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDomain {
    merge: MergeMode,
}

/// The keys of `[config]`.
pub(super) const KEYS: [&str; 1] = ["merge"];

const MERGE_MODES: [(&str, MergeMode); 2] =
    [("deep", MergeMode::Deep), ("shallow", MergeMode::Shallow)];

impl ConfigDomain {
    pub(super) const DEFAULT: Self = Self {
        merge: MergeMode::Deep,
    };

    /// `config.merge`
    #[must_use]
    pub fn merge(&self) -> MergeMode {
        self.merge
    }

    pub(super) fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        if let Some((path, _)) = fields.take("extends") {
            return Err(ConfigError::unsupported(&path));
        }
        let merge = fields
            .take("merge")
            .map(|(path, value)| choice(&path, &value, &MERGE_MODES))
            .transpose()?
            .unwrap_or(MergeMode::Deep);
        fields.finish(&KEYS)?;
        Ok(Self { merge })
    }

    pub(super) fn to_table(&self) -> Table {
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

impl JsonSchema for ConfigDomain {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ConfigDomain")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        closed_table(
            [("merge", generator.subschema_for::<MergeMode>())],
            &Self::DEFAULT.to_table(),
        )
    }
}

impl JsonSchema for MergeMode {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("MergeMode")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        choice_schema(&MERGE_MODES)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::test_support::Schema::{self, Rejects};
    use crate::config::test_support::schema_verdict;
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use super::*;
    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_UNKNOWN_KEY, CONFIG_UNSUPPORTED_SECTION};
    use crate::config::test_support::{loaded, rejection};

    #[rstest]
    #[case::deep("deep", MergeMode::Deep)]
    #[case::shallow("shallow", MergeMode::Shallow)]
    fn merge_mode(#[case] value: &str, #[case] expected: MergeMode) {
        let config = loaded(&format!("config.merge = \"{value}\""));

        assert_eq!(config.config().merge(), expected);
    }

    #[rstest]
    // config
    #[case::merge(
        r#"config.merge = "partial""#,
        CONFIG_INVALID_VALUE,
        "config.merge",
        Rejects
    )]
    #[case::extends(
        r#"config.extends = "preset:rust""#,
        CONFIG_UNSUPPORTED_SECTION,
        "config.extends",
        Rejects
    )]
    #[case::extends_list(
        r#"config.extends = ["./a.toml"]"#,
        CONFIG_UNSUPPORTED_SECTION,
        "config.extends",
        Rejects
    )]
    #[case::config_unknown_key(
        "config.strict = true",
        CONFIG_UNKNOWN_KEY,
        "config.strict",
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
}
