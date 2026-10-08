use rstest::rstest;
use semoxide_error::ErrorInfo;

use super::Config;
use crate::codes::CONFIG_UNKNOWN_KEY;

/// The code and help of the error the config fails with.
fn unknown_key_help(text: &str) -> Option<(String, Option<String>)> {
    let table = text.parse::<toml::Table>().unwrap();
    Config::from_table(table)
        .err()
        .map(|error| (error.code().to_string(), error.help()))
}

#[rstest]
#[case::domain("[tgas]\nformat = \"v{version}\"", Some("Use `tags` instead."))]
#[case::config_key(r#"config.marge = "deep""#, Some("Use `config.merge` instead."))]
#[case::commits_key(r#"commits.prest = "angular""#, Some("Use `commits.preset` instead."))]
#[case::version_key(r#"version.inittal = "1.0.0""#, Some("Use `version.initial` instead."))]
#[case::zero_key(
    r#"version.zero.brekaing = "major""#,
    Some("Use `version.zero.breaking` instead.")
)]
#[case::tags_key(r#"tags.formta = "v{version}""#, Some("Use `tags.format` instead."))]
#[case::release_rule_key(
    r#"branches.rules = [{ name = "main", chnanel = "stable" }]"#,
    Some("Use `branches.rules[0].channel` instead.")
)]
#[case::maintenance_rule_key(
    r#"branches.rules = [{ maintenance = "legacy", range = "1.x", chnanel = "old" }]"#,
    Some("Use `branches.rules[0].channel` instead.")
)]
#[case::step_name(
    r#"steps.publihs.order = ["release-notes"]"#,
    Some("Use `steps.publish` instead.")
)]
#[case::step_key(
    r#"steps.publish.orderr = ["release-notes"]"#,
    Some("Use `steps.publish.order` instead.")
)]
#[case::timeout_step(
    "[plugins.release-notes]\ntimeouts.publihs = \"1h\"",
    Some("Use `plugins.release-notes.timeouts.publish` instead.")
)]
#[case::secrets_key("secrets.mask_envs = []", Some("Use `secrets.mask_env` instead."))]
#[case::nothing_close_key(r#"tags.prefix = "v{version}""#, None)]
#[case::nothing_close_domain("[colour]\nname = \"red\"", None)]
fn unknown_key_suggests_the_closest_valid_key(#[case] text: &str, #[case] help: Option<&str>) {
    assert_eq!(
        unknown_key_help(text),
        Some((CONFIG_UNKNOWN_KEY.to_string(), help.map(str::to_owned)))
    );
}

#[test]
fn the_message_still_names_the_unknown_key() {
    let table = r#"tags.formta = "v{version}""#.parse::<toml::Table>().unwrap();

    let error = Config::from_table(table);

    assert!(error.is_err());
    assert_eq!(error.unwrap_err().to_string(), "unknown key `tags.formta`");
}
