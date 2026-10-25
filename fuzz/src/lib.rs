//! The checks behind the fuzz targets in `fuzz_targets/`: each takes one input and panics when
//! the code under test breaks a law.

#![forbid(unsafe_code)]

use semoxide_schema::config::Config;
use semoxide_version_engine::{CommitId, CommitMessage, Skipped, TagPattern, Version};

/// A version that `Version::from_str` accepts displays as the input.
///
/// # Panics
///
/// When an accepted version displays differently.
pub fn version(input: &str) {
    let Ok(version) = input.parse::<Version>() else {
        return;
    };
    assert_eq!(version.to_string(), input);
}

/// `input` is a `tags.format` and a tag name, split at the first NUL. A tag the format parses
/// is the version's rendered tag, or that with build metadata before the suffix, and the
/// rendered tag parses back.
///
/// # Panics
///
/// When a parsed tag isn't shaped like its version's tag, or a rendered tag doesn't parse back.
pub fn tag(input: &str) {
    let Some((format, name)) = input.split_once('\0') else {
        return;
    };
    let Some(pattern) = pattern(format) else {
        return;
    };
    let Some(version) = pattern.parse(name) else {
        return;
    };
    let rendered = pattern.render(&version);
    let (prefix, suffix) = format.split_once("{version}").unwrap_or((format, ""));
    let with_build = name
        .strip_prefix(format!("{prefix}{version}+").as_str())
        .and_then(|rest| rest.strip_suffix(suffix))
        .is_some_and(|build| semver::BuildMetadata::new(build).is_ok());

    assert!(
        name == rendered || with_build,
        "`{name}` parsed as {version}"
    );
    assert_eq!(pattern.parse(&rendered), Some(version));
}

/// Whether a commit is skipped doesn't depend on the ASCII case of its message.
///
/// # Panics
///
/// When upper-casing the message changes whether the commit is skipped.
pub fn skip_marker(message: &str) {
    assert_eq!(
        is_skipped(message),
        is_skipped(&message.to_ascii_uppercase()),
        "{message:?}"
    );
}

/// The pattern of `format`, or `None` when the config rejects it.
fn pattern(format: &str) -> Option<TagPattern> {
    let mut table = Config::defaults_table();
    *table.get_mut("tags")?.get_mut("format")? = format.into();
    let config = Config::from_table(table).ok()?;
    Some(TagPattern::new(config.tags().format()))
}

/// A commit with a fuzzed message.
struct Fuzzed {
    id: CommitId,
    message: String,
}

impl CommitMessage for Fuzzed {
    fn id(&self) -> &CommitId {
        &self.id
    }

    fn message(&self) -> &str {
        &self.message
    }
}

fn is_skipped(message: &str) -> bool {
    let Ok(id) = CommitId::try_from("0".repeat(40).as_str()) else {
        return false;
    };
    let commit = Fuzzed {
        id,
        message: message.to_owned(),
    };
    !Skipped::filter(vec![commit]).skipped().is_empty()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::release("1.2.3")]
    #[case::prerelease("1.0.0-rc.1")]
    #[case::leading_zero("01.0.0")]
    #[case::build_metadata("1.0.0+b")]
    #[case::leading_v("v1.0.0")]
    #[case::past_u64("18446744073709551616.0.0")]
    #[case::empty("")]
    fn version_holds(#[case] input: &str) {
        version(input);
    }

    #[rstest]
    #[case::default("v{version}\0v1.0.0")]
    #[case::build_metadata("v{version}\0v1.0.0+b.1")]
    #[case::suffix("release/{version}-x\0release/1.0.0-rc.1-x")]
    #[case::suffix_with_build("{version}-x\u{0}1.0.0+b-x")]
    #[case::no_match("v{version}\0w1.0.0")]
    #[case::invalid_format("{version}{version}\0x")]
    #[case::no_separator("v{version}")]
    fn tag_holds(#[case] input: &str) {
        tag(input);
    }

    #[test]
    fn a_valid_format_loads() {
        assert_eq!(
            (
                pattern("v{version}").is_some(),
                pattern("{version}{version}").is_some()
            ),
            (true, false)
        );
    }

    #[rstest]
    #[case::marker("fix: x [Skip Release]")]
    #[case::reversed("fix: x [release\nskip]")]
    #[case::other_text("fix: x [skip ci]")]
    #[case::look_alike_letter("fix: x [\u{17f}kip release]")]
    #[case::empty("")]
    fn skip_marker_holds(#[case] message: &str) {
        skip_marker(message);
    }

    #[test]
    fn a_skipped_message_is_detected() {
        assert_eq!(
            (is_skipped("[skip release]"), is_skipped("[skip ci]")),
            (true, false)
        );
    }
}
