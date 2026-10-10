//! `[secrets]`: extra environment variables whose values are masked.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use toml::{Table, Value};

use super::schema::{Property, closed_table};
use super::values::{EnvName, array, env_name_schema};
use super::{ConfigError, Fields, index};

/// `[secrets]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secrets {
    mask_env: Vec<EnvName>,
}

/// The keys of `[secrets]`.
pub(super) const KEYS: [&str; 1] = ["mask_env"];

impl Secrets {
    pub(super) const DEFAULT: Self = Self {
        mask_env: Vec::new(),
    };

    /// `secrets.mask_env`
    #[must_use]
    pub fn mask_env(&self) -> &[EnvName] {
        &self.mask_env
    }

    pub(super) fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let mask_env = match fields.take("mask_env") {
            Some((path, value)) => array(&path, value)?
                .iter()
                .enumerate()
                .map(|(position, item)| EnvName::parse(&index(&path, position), item))
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        };
        fields.finish(&KEYS)?;
        Ok(Self { mask_env })
    }

    pub(super) fn to_table(&self) -> Table {
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

impl JsonSchema for Secrets {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Secrets")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_table(
            "Extra values to mask in all output.",
            [Property::new(
                "mask_env",
                "Names of environment variables whose values are masked.",
                json_schema!({ "type": "array", "items": env_name_schema() }),
            )],
            &Self::DEFAULT.to_table(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::config::test_support::Schema::{self, Rejects};
    use crate::config::test_support::schema_verdict;
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use super::*;
    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_UNKNOWN_KEY};
    use crate::config::test_support::{loaded, rejection};

    #[test]
    fn mask_env() {
        let config = loaded(r#"secrets.mask_env = ["DEPLOY_TOKEN", "_private2"]"#);

        let names: Vec<&str> = config
            .secrets()
            .mask_env()
            .iter()
            .map(EnvName::as_str)
            .collect();
        assert_eq!(names, ["DEPLOY_TOKEN", "_private2"]);
    }

    #[rstest]
    // secrets
    #[case::env_name_dash(
        r#"secrets.mask_env = ["MY-TOKEN"]"#,
        CONFIG_INVALID_VALUE,
        "secrets.mask_env[0]",
        Rejects
    )]
    #[case::env_name_leading_digit(
        r#"secrets.mask_env = ["1TOKEN"]"#,
        CONFIG_INVALID_VALUE,
        "secrets.mask_env[0]",
        Rejects
    )]
    #[case::env_name_empty(
        r#"secrets.mask_env = [""]"#,
        CONFIG_INVALID_VALUE,
        "secrets.mask_env[0]",
        Rejects
    )]
    #[case::mask_env_not_a_list(
        r#"secrets.mask_env = "TOKEN""#,
        CONFIG_INVALID_VALUE,
        "secrets.mask_env",
        Rejects
    )]
    #[case::secrets_unknown_key("secrets.files = []", CONFIG_UNKNOWN_KEY, "secrets.files", Rejects)]
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
