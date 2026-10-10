//! `[version]`: the first release version and the 0.x release levels.

use semver::Version;
use toml::{Table, Value};

use super::values::{choice, choice_name, string};
use super::{ConfigError, Fields};

/// `[version]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDomain {
    initial: Version,
    zero: ZeroLevels,
}

/// The keys of `[version]`.
pub(super) const KEYS: [&str; 2] = ["initial", "zero"];

/// The keys of `version.zero`.
pub(super) const ZERO_KEYS: [&str; 3] = ["breaking", "feature", "fix"];

impl VersionDomain {
    pub(super) fn defaults() -> Self {
        Self {
            initial: default_initial_version(),
            zero: ZeroLevels::DEFAULT,
        }
    }

    /// `version.initial`: the first release version.
    #[must_use]
    pub fn initial(&self) -> &Version {
        &self.initial
    }

    /// `version.zero`: how the analyzer's levels map while the version is 0.x.
    #[must_use]
    pub fn zero(&self) -> ZeroLevels {
        self.zero
    }

    pub(super) fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let initial = fields
            .take("initial")
            .map(|(path, value)| parse_version(&path, &value))
            .transpose()?
            .unwrap_or_else(default_initial_version);
        if !initial.build.is_empty() {
            return Err(ConfigError::invalid(
                "version.initial",
                &Value::String(initial.to_string()),
                "build metadata isn't allowed",
            ));
        }
        let zero = match fields.take("zero") {
            Some((path, value)) => ZeroLevels::parse(Fields::from_value(&path, value)?)?,
            None => ZeroLevels::DEFAULT,
        };
        fields.finish(&KEYS)?;
        Ok(Self { initial, zero })
    }

    pub(super) fn to_table(&self) -> Table {
        Table::from_iter([
            (
                String::from("initial"),
                Value::String(self.initial.to_string()),
            ),
            (String::from("zero"), Value::Table(self.zero.to_table())),
        ])
    }
}

fn default_initial_version() -> Version {
    Version::new(1, 0, 0)
}

pub(super) fn parse_version(path: &str, value: &Value) -> Result<Version, ConfigError> {
    let text = string(path, value)?;
    Version::parse(&text).map_err(|_| {
        ConfigError::invalid(
            path,
            value,
            "expected a full SemVer version such as `1.0.0`",
        )
    })
}

/// The release level each kind of change gets on 0.x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroLevels {
    breaking: Level,
    feature: Level,
    fix: Level,
}

const LEVELS: [(&str, Level); 3] = [
    ("major", Level::Major),
    ("minor", Level::Minor),
    ("patch", Level::Patch),
];

impl ZeroLevels {
    const DEFAULT: Self = Self {
        breaking: Level::Minor,
        feature: Level::Patch,
        fix: Level::Patch,
    };

    /// `version.zero.breaking`
    #[must_use]
    pub fn breaking(&self) -> Level {
        self.breaking
    }

    /// `version.zero.feature`
    #[must_use]
    pub fn feature(&self) -> Level {
        self.feature
    }

    /// `version.zero.fix`
    #[must_use]
    pub fn fix(&self) -> Level {
        self.fix
    }

    fn parse(mut fields: Fields) -> Result<Self, ConfigError> {
        let mut level = |name: &str, default: Level| {
            fields
                .take(name)
                .map(|(path, value)| choice(&path, &value, &LEVELS))
                .transpose()
                .map(|level| level.unwrap_or(default))
        };
        let levels = Self {
            breaking: level("breaking", Self::DEFAULT.breaking)?,
            feature: level("feature", Self::DEFAULT.feature)?,
            fix: level("fix", Self::DEFAULT.fix)?,
        };
        fields.finish(&ZERO_KEYS)?;
        Ok(levels)
    }

    fn to_table(self) -> Table {
        let name = |level: Level| Value::String(choice_name(level, &LEVELS).to_owned());
        Table::from_iter([
            (String::from("breaking"), name(self.breaking)),
            (String::from("feature"), name(self.feature)),
            (String::from("fix"), name(self.fix)),
        ])
    }
}

/// A release level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// `major`
    Major,
    /// `minor`
    Minor,
    /// `patch`
    Patch,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use super::*;
    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_UNKNOWN_KEY};
    use crate::config::test_support::{loaded, rejection};

    #[rstest]
    #[case::zero_x("0.1.0")]
    #[case::prerelease("2.0.0-rc.1")]
    fn initial_version(#[case] value: &str) {
        let config = loaded(&format!("version.initial = \"{value}\""));

        assert_eq!(config.version().initial(), &Version::parse(value).unwrap());
    }

    #[rstest]
    #[case::major("major", Level::Major)]
    #[case::minor("minor", Level::Minor)]
    #[case::patch("patch", Level::Patch)]
    fn zero_level(#[case] value: &str, #[case] expected: Level) {
        let config = loaded(&format!(
            "[version.zero]\nbreaking = \"{value}\"\nfeature = \"{value}\"\nfix = \"{value}\""
        ));

        let zero = config.version().zero();
        assert_eq!(
            (zero.breaking(), zero.feature(), zero.fix()),
            (expected, expected, expected)
        );
    }

    #[test]
    fn zero_levels_not_set_keep_their_defaults() {
        let config = loaded("version.zero.breaking = \"major\"");

        let zero = config.version().zero();
        assert_eq!(
            (zero.breaking(), zero.feature(), zero.fix()),
            (Level::Major, Level::Patch, Level::Patch)
        );
    }

    #[rstest]
    // version
    #[case::initial_two_parts(
        r#"version.initial = "1.0""#,
        CONFIG_INVALID_VALUE,
        "version.initial"
    )]
    #[case::initial_v_prefix(
        r#"version.initial = "v1.0.0""#,
        CONFIG_INVALID_VALUE,
        "version.initial"
    )]
    #[case::initial_build_metadata(
        r#"version.initial = "1.0.0+build""#,
        CONFIG_INVALID_VALUE,
        "version.initial"
    )]
    #[case::initial_not_a_string("version.initial = 1", CONFIG_INVALID_VALUE, "version.initial")]
    #[case::zero_breaking(
        r#"version.zero.breaking = "huge""#,
        CONFIG_INVALID_VALUE,
        "version.zero.breaking"
    )]
    #[case::zero_feature(
        r#"version.zero.feature = "none""#,
        CONFIG_INVALID_VALUE,
        "version.zero.feature"
    )]
    #[case::zero_fix(
        r#"version.zero.fix = "Patch""#,
        CONFIG_INVALID_VALUE,
        "version.zero.fix"
    )]
    #[case::zero_unknown_key(
        r#"version.zero.docs = "patch""#,
        CONFIG_UNKNOWN_KEY,
        "version.zero.docs"
    )]
    #[case::version_unknown_key(r#"version.first = "1.0.0""#, CONFIG_UNKNOWN_KEY, "version.first")]
    fn invalid_config_is_rejected(#[case] text: &str, #[case] code: ErrorCode, #[case] path: &str) {
        assert_eq!(rejection(text), Err((code, path.to_owned())));
    }
}
