use rstest::rstest;
use semoxide_error::ErrorInfo;
use toml::Table;

use super::parse_flag;
use crate::codes::CONFIG_INVALID_FLAG;
use crate::config::Source;

/// The flag's layer table, failing the test with an assertion if it doesn't parse.
fn parsed(position: usize, flag: &str) -> (Source, Table) {
    let result = parse_flag(position, flag);
    assert!(result.is_ok(), "`{flag}` should parse: {result:?}");
    let layer = result.unwrap();
    (layer.source().clone(), layer.table().clone())
}

fn expected(text: &str) -> Table {
    text.parse().unwrap()
}

#[rstest]
#[case::plain_string("tags.format=v{version}", r#"tags.format = "v{version}""#)]
#[case::version_string("version.initial=0.1.0", r#"version.initial = "0.1.0""#)]
#[case::word("steps.success.errors=fail", r#"steps.success.errors = "fail""#)]
#[case::empty_value("tags.format=", r#"tags.format = """#)]
#[case::boolean("plugins.github.draft=true", "plugins.github.draft = true")]
#[case::integer("plugins.npm.tag=1", "plugins.npm.tag = 1")]
#[case::quoted_forces_a_string(r#"plugins.npm.tag="1""#, r#"plugins.npm.tag = "1""#)]
#[case::array(
    r#"steps.plugins=["git", "github"]"#,
    r#"steps.plugins = ["git", "github"]"#
)]
#[case::inline_table(
    "plugins.github.draft={ enabled = true }",
    "plugins.github.draft = { enabled = true }"
)]
#[case::dashed_key(
    "plugins.release-notes.locale=sv",
    r#"plugins.release-notes.locale = "sv""#
)]
#[case::quoted_key_part(r#"plugins."my.plugin".x=1"#, r#"plugins."my.plugin".x = 1"#)]
#[case::equals_in_the_value("tags.format=a={version}", r#"tags.format = "a={version}""#)]
fn valid_flag(#[case] flag: &str, #[case] table: &str) {
    assert_eq!(parsed(1, flag), (Source::Flag(1), expected(table)));
}

#[test]
fn the_flags_position_is_its_source() {
    assert_eq!(parsed(3, "tags.format=v{version}").0, Source::Flag(3));
}

#[rstest]
#[case::no_equals("tags.format")]
#[case::empty_key("=v{version}")]
#[case::empty_key_part("tags..format=x")]
#[case::array_index("branches.rules[0].channel=next")]
#[case::broken_array(r#"steps.plugins=["git""#)]
#[case::unterminated_string(r#"tags.format="v{version}"#)]
#[case::broken_inline_table("plugins.github.draft={ enabled = true")]
#[case::single_quote_start("tags.format='v{version}")]
fn invalid_flag_is_rejected_with_its_position(#[case] flag: &str) {
    let result = parse_flag(2, flag);

    assert_eq!(
        result
            .map(|_| ())
            .map_err(|error| (error.code(), error.position())),
        Err((CONFIG_INVALID_FLAG, 2)),
        "{flag}"
    );
}

#[test]
fn error_message_quotes_the_flag() {
    let result = parse_flag(1, "tags.format");

    assert!(result.is_err());
    let message = result.unwrap_err().to_string();
    assert!(message.contains("`--set tags.format`"), "{message}");
    assert!(message.contains("expected `<key>=<value>`"), "{message}");
}
