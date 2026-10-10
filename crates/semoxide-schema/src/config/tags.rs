//! `[tags]`: the tag format and metadata.

use toml::{Table, Value};

use super::values::{TagFormat, Template, string};
use super::{ConfigError, Fields};

/// `[tags]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    format: TagFormat,
    metadata: Option<Template>,
}

/// The keys of `[tags]`.
pub(super) const KEYS: [&str; 2] = ["format", "metadata"];

impl Tags {
    pub(super) fn defaults() -> Self {
        Self {
            format: TagFormat::default_format(),
            metadata: None,
        }
    }

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

    pub(super) fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let format = fields
            .take("format")
            .map(|(path, value)| TagFormat::parse(&path, &value))
            .transpose()?
            .unwrap_or_else(TagFormat::default_format);
        let metadata = fields
            .take("metadata")
            .map(|(path, value)| string(&path, &value).map(Template::unchecked))
            .transpose()?;
        fields.finish(&KEYS)?;
        Ok(Self { format, metadata })
    }

    pub(super) fn to_table(&self) -> Table {
        let mut table = Table::from_iter([(
            String::from("format"),
            Value::String(self.format.as_str().to_owned()),
        )]);
        if let Some(metadata) = &self.metadata {
            table.insert(
                String::from("metadata"),
                Value::String(metadata.as_str().to_owned()),
            );
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use crate::config::test_support::Schema::{self, ParserOnly, Rejects};
    use crate::config::test_support::schema_verdict;
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_UNKNOWN_KEY};
    use crate::config::test_support::{loaded, rejection};

    #[test]
    fn tags() {
        let config = loaded(
            r#"
[tags]
format = "release-{version}"
metadata = "{{ commit.short_sha }}"
"#,
        );

        assert_eq!(config.tags().format().as_str(), "release-{version}");
        assert_eq!(
            config.tags().metadata().map(super::Template::as_str),
            Some("{{ commit.short_sha }}")
        );
    }

    #[rstest]
    // tags
    #[case::tag_format_without_version(
        r#"tags.format = "release-tag""#,
        CONFIG_INVALID_VALUE,
        "tags.format",
        Rejects
    )]
    #[case::tag_format_twice(
        r#"tags.format = "{version}-{version}""#,
        CONFIG_INVALID_VALUE,
        "tags.format",
        ParserOnly
    )]
    #[case::tags_unknown_key(
        r#"tags.prefix = "v{version}""#,
        CONFIG_UNKNOWN_KEY,
        "tags.prefix",
        Rejects
    )]
    #[case::domain_not_a_table(r#"tags = "v{version}""#, CONFIG_INVALID_VALUE, "tags", Rejects)]
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
