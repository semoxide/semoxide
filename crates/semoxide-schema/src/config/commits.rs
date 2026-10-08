//! `[commits]`: how commits are read.

use toml::{Table, Value};

use super::values::{choice, choice_name};
use super::{ConfigError, Fields};

/// `[commits]`: how commits are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commits {
    preset: Preset,
}

const PRESETS: [(&str, Preset); 2] = [
    ("conventionalcommits", Preset::ConventionalCommits),
    ("angular", Preset::Angular),
];

impl Commits {
    pub(super) const DEFAULT: Self = Self {
        preset: Preset::ConventionalCommits,
    };

    /// `commits.preset`
    #[must_use]
    pub fn preset(&self) -> Preset {
        self.preset
    }

    pub(super) fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let preset = fields
            .take("preset")
            .map(|(path, value)| choice(&path, &value, &PRESETS))
            .transpose()?
            .unwrap_or(Preset::ConventionalCommits);
        fields.finish(&["preset"])?;
        Ok(Self { preset })
    }

    pub(super) fn to_table(&self) -> Table {
        Table::from_iter([(
            String::from("preset"),
            Value::String(choice_name(self.preset, &PRESETS).to_owned()),
        )])
    }
}

/// The commit convention shared by the analyzer and notes plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// `conventionalcommits`: `!` marks a breaking change.
    ConventionalCommits,
    /// `angular`
    Angular,
}
