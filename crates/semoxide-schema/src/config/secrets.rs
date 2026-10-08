//! `[secrets]`: extra environment variables whose values are masked.

use toml::{Table, Value};

use super::values::{EnvName, array};
use super::{ConfigError, Fields, index};

/// `[secrets]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secrets {
    mask_env: Vec<EnvName>,
}

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
        fields.finish(&["mask_env"])?;
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
