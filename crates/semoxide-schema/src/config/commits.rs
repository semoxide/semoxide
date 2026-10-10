//! `[commits]`: how commits are read.

use toml::{Table, Value};

use super::values::{choice, choice_name};
use super::{ConfigError, Fields};

/// `[commits]`: how commits are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commits {
    preset: Preset,
}

/// The keys of `[commits]`.
pub(super) const KEYS: [&str; 1] = ["preset"];

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
        fields.finish(&KEYS)?;
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

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use super::*;
    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_UNKNOWN_KEY};
    use crate::config::test_support::{loaded, rejection};

    #[rstest]
    #[case::conventional("conventionalcommits", Preset::ConventionalCommits)]
    #[case::angular("angular", Preset::Angular)]
    fn commits_preset(#[case] value: &str, #[case] expected: Preset) {
        let config = loaded(&format!("commits.preset = \"{value}\""));

        assert_eq!(config.commits().preset(), expected);
    }

    #[rstest]
    // commits
    #[case::preset(r#"commits.preset = "eslint""#, CONFIG_INVALID_VALUE, "commits.preset")]
    #[case::commits_unknown_key("commits.types = []", CONFIG_UNKNOWN_KEY, "commits.types")]
    fn invalid_config_is_rejected(#[case] text: &str, #[case] code: ErrorCode, #[case] path: &str) {
        assert_eq!(rejection(text), Err((code, path.to_owned())));
    }
}
