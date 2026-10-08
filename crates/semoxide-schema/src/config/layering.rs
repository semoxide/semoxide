//! `[config]`: how the configuration itself is assembled.

use toml::{Table, Value};

use super::values::{choice, choice_name};
use super::{ConfigError, Fields};

/// `[config]`: how the configuration itself is assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDomain {
    merge: MergeMode,
}

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
        fields.finish(&["merge"])?;
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
