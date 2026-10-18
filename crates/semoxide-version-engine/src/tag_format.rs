//! [`TagPattern`]: `tags.format` rendered for a version and parsed back out of tag names.

use semoxide_schema::config::TagFormat;

use crate::Version;

/// `tags.format` split at its `{version}`. A tag matches only as a whole name:
/// `<prefix><version>[+<metadata>]<suffix>`, with a strict version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPattern {
    prefix: String,
    suffix: String,
}

impl TagPattern {
    /// The pattern of `format`, which holds `{version}` exactly once.
    #[must_use]
    pub fn new(_format: &TagFormat) -> Self {
        Self {
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    /// The tag name of `version`, without build metadata.
    #[must_use]
    pub fn render(&self, _version: &Version) -> String {
        String::new()
    }

    /// The version a tag name holds; build metadata is dropped, so `v1.2.3+a` is 1.2.3.
    /// `None` when the name doesn't match the format.
    #[must_use]
    pub fn parse(&self, _name: &str) -> Option<Version> {
        Some(Version::from_parts(0, 0, 0, semver::Prerelease::EMPTY))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semoxide_schema::config::Config;

    use super::*;

    fn pattern(format: &str) -> TagPattern {
        let mut table = Config::defaults_table();
        table["tags"]["format"] = format.into();
        TagPattern::new(Config::from_table(table).unwrap().tags().format())
    }

    fn version(input: &str) -> Version {
        input.parse().unwrap()
    }

    #[rstest]
    #[case::default("v{version}", "1.2.3", "v1.2.3")]
    #[case::bare("{version}", "1.2.3", "1.2.3")]
    #[case::prefix_and_suffix("release/{version}-x", "1.2.3", "release/1.2.3-x")]
    #[case::prerelease("v{version}", "2.0.0-beta.1", "v2.0.0-beta.1")]
    #[case::literal_braces("{v}{version}", "1.2.3", "{v}1.2.3")]
    fn render_puts_the_version_into_the_format(
        #[case] format: &str,
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(pattern(format).render(&version(input)), expected);
    }

    #[rstest]
    #[case::release("v1.2.3", "1.2.3")]
    #[case::prerelease("v1.0.0-beta.1", "1.0.0-beta.1")]
    #[case::metadata("v1.2.3+abc", "1.2.3")]
    #[case::dotted_metadata("v1.2.3+abc.def", "1.2.3")]
    fn a_matching_tag_gives_its_version(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(pattern("v{version}").parse(name), Some(version(expected)));
    }

    #[rstest]
    #[case::no_prefix("1.2.3")]
    #[case::double_prefix("vv1.2.3")]
    #[case::equals("v=1.2.3")]
    #[case::uppercase_prefix("V1.2.3")]
    #[case::two_parts("v1.2")]
    #[case::leading_zero("v01.2.3")]
    #[case::empty_metadata("v1.2.3+")]
    #[case::two_plus("v1.2.3+a+b")]
    #[case::leading_space(" v1.2.3")]
    #[case::trailing_space("v1.2.3 ")]
    #[case::prefix_only("v")]
    #[case::empty("")]
    fn a_tag_that_doesnt_match_is_ignored(#[case] name: &str) {
        assert_eq!(pattern("v{version}").parse(name), None);
    }

    #[test]
    fn a_suffix_must_end_the_name() {
        let pattern = pattern("release-{version}-x");

        assert_eq!(
            [
                "release-1.2.3-x",
                "release-1.2.3-rc.1-x",
                "release-1.2.3-x-x",
                "release-1.2.3-x-extra",
                "release-1.2.3",
            ]
            .map(|name| pattern.parse(name)),
            [
                Some(version("1.2.3")),
                Some(version("1.2.3-rc.1")),
                Some(version("1.2.3-x")),
                None,
                None
            ]
        );
    }

    #[test]
    fn a_bare_format_takes_only_the_version() {
        let pattern = pattern("{version}");

        assert_eq!(
            [pattern.parse("1.2.3"), pattern.parse("v1.2.3")],
            [Some(version("1.2.3")), None]
        );
    }

    /// Tags are matched by version: metadata never tells two releases apart.
    #[test]
    fn tags_differing_only_in_metadata_hold_the_same_version() {
        let pattern = pattern("v{version}");

        assert_eq!(
            [pattern.parse("v1.2.3+a"), pattern.parse("v1.2.3+b")],
            [Some(version("1.2.3")), Some(version("1.2.3"))]
        );
    }

    #[rstest]
    #[case::default("v{version}")]
    #[case::bare("{version}")]
    #[case::prefix_and_suffix("release/{version}-x")]
    fn a_rendered_tag_parses_back(#[case] format: &str) {
        let pattern = pattern(format);
        let versions = ["0.1.0", "1.2.3", "2.0.0-beta.1", "1.0.0-rc.1.x-y"].map(version);

        assert_eq!(
            versions
                .clone()
                .map(|version| pattern.parse(&pattern.render(&version))),
            versions.map(Some)
        );
    }
}
