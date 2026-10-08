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
        fields.finish(&["format", "metadata"])?;
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
