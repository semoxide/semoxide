use std::time::Duration;

use rstest::rstest;
use semoxide_error::{ErrorCode, ErrorInfo};
use semver::Version;

use super::{
    BranchRule, Channel, Config, ConfigError, EnvName, Level, MergeMode, PluginName, Prerelease,
    Preset, Step, SuccessErrors,
};
use crate::codes::{
    CONFIG_CONFLICTING_KEYS, CONFIG_INVALID_VALUE, CONFIG_PLUGIN_NOT_ENABLED, CONFIG_UNKNOWN_KEY,
    CONFIG_UNSUPPORTED_SECTION,
};

fn parse(text: &str) -> Result<Config, ConfigError> {
    Config::from_table(text.parse::<toml::Table>().unwrap())
}

/// The config, failing the test with an assertion if it doesn't load.
fn loaded(text: &str) -> Config {
    let result = parse(text);
    assert!(result.is_ok(), "the config should load: {result:?}");
    result.unwrap()
}

/// The code and key path of the error, or `Ok` if the config loaded.
fn rejection(text: &str) -> Result<(), (ErrorCode, String)> {
    parse(text)
        .map(|_| ())
        .map_err(|error| (error.code(), error.path().to_owned()))
}

fn plugin(name: &str) -> PluginName {
    name.parse().unwrap()
}

fn plugins(names: &[&str]) -> Vec<PluginName> {
    names.iter().map(|name| plugin(name)).collect()
}

/// Every field of a branch rule in one line, so a test can compare whole rules.
fn describe(rule: &BranchRule) -> String {
    let channel = |channel: Option<&Channel>| match channel {
        None => String::from("-"),
        Some(Channel::Default) => String::from("default"),
        Some(Channel::Named(name)) => format!("'{}'", name.as_str()),
    };
    match rule {
        BranchRule::Release(rule) => {
            format!(
                "release {} channel={}",
                rule.name(),
                channel(rule.channel())
            )
        }
        BranchRule::Prerelease(rule) => {
            let id = match rule.prerelease() {
                Prerelease::BranchName => String::from("<branch>"),
                Prerelease::Id(id) => format!("'{}'", id.as_str()),
            };
            format!(
                "prerelease {} id={id} channel={}",
                rule.name(),
                channel(rule.channel())
            )
        }
        BranchRule::Maintenance(rule) => format!(
            "maintenance {} range={} channel={}",
            rule.pattern(),
            rule.range().unwrap_or("-"),
            channel(rule.channel())
        ),
    }
}

fn describe_rules(config: &Config) -> Vec<String> {
    config.branches().rules().iter().map(describe).collect()
}

// --- Defaults (CONFIG.md) ---

#[test]
fn empty_table_gives_the_defaults() {
    let config = loaded("");

    assert_eq!(config.config().merge(), MergeMode::Deep);
    assert_eq!(config.commits().preset(), Preset::ConventionalCommits);
    assert_eq!(config.version().initial(), &Version::new(1, 0, 0));
    let zero = config.version().zero();
    assert_eq!(
        (zero.breaking(), zero.feature(), zero.fix()),
        (Level::Minor, Level::Patch, Level::Patch)
    );
    assert_eq!(config.tags().format().as_str(), "v{version}");
    assert_eq!(config.tags().metadata(), None);
    assert_eq!(
        config.steps().plugins(),
        plugins(&["commit-analyzer", "release-notes"])
    );
    for step in [Step::VerifyConditions, Step::Publish, Step::Success] {
        assert_eq!(config.steps().order(step), None, "{step:?}");
    }
    assert_eq!(config.steps().success_errors(), SuccessErrors::Warn);
    assert_eq!(config.secrets().mask_env(), &[] as &[EnvName]);
}

#[test]
fn default_branch_rules_are_upstreams_set() {
    let config = loaded("");

    assert_eq!(
        describe_rules(&config),
        [
            "maintenance N.x range=- channel=-",
            "release master channel=-",
            "release main channel=-",
            "release next channel=-",
            "release next-major channel=-",
            "prerelease beta id=<branch> channel=-",
            "prerelease alpha id=<branch> channel=-",
        ]
    );
}

#[test]
fn enabled_plugins_without_a_table_have_default_settings() {
    let config = loaded("");

    for name in ["commit-analyzer", "release-notes"] {
        let settings = config.plugin(&plugin(name));
        assert!(settings.is_some(), "{name}");
        let settings = settings.unwrap();
        assert_eq!(settings.version(), None);
        assert_eq!(settings.timeout(Step::Publish), None);
        assert!(!settings.show_output());
        assert_eq!(settings.options(), &serde_json::Map::new());
    }
    assert_eq!(config.plugin(&plugin("github")), None);
}

// --- Valid values ---

#[rstest]
#[case::deep("deep", MergeMode::Deep)]
#[case::shallow("shallow", MergeMode::Shallow)]
fn merge_mode(#[case] value: &str, #[case] expected: MergeMode) {
    let config = loaded(&format!("config.merge = \"{value}\""));

    assert_eq!(config.config().merge(), expected);
}

#[rstest]
#[case::conventional("conventionalcommits", Preset::ConventionalCommits)]
#[case::angular("angular", Preset::Angular)]
fn commits_preset(#[case] value: &str, #[case] expected: Preset) {
    let config = loaded(&format!("commits.preset = \"{value}\""));

    assert_eq!(config.commits().preset(), expected);
}

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

#[test]
fn branch_rules_of_every_kind() {
    let config = loaded(
        r#"
[branches]
rules = [
  { maintenance = "N.x" },
  { maintenance = "release/N.N.x", channel = "{name}" },
  { maintenance = "legacy", range = "1.x", channel = false },
  "main",
  { name = "next", channel = "beta" },
  { name = "next-major", channel = false },
  { name = "beta", prerelease = true },
  { name = "release/*", prerelease = "rc", channel = "rc-{name}" },
  { name = "preview/*", prerelease = "pre-{name}" },
]
"#,
    );

    assert_eq!(
        describe_rules(&config),
        [
            "maintenance N.x range=- channel=-",
            "maintenance release/N.N.x range=- channel='{name}'",
            "maintenance legacy range=1.x channel=default",
            "release main channel=-",
            "release next channel='beta'",
            "release next-major channel=default",
            "prerelease beta id=<branch> channel=-",
            "prerelease release/* id='rc' channel='rc-{name}'",
            "prerelease preview/* id='pre-{name}' channel=-",
        ]
    );
}

#[rstest]
#[case::dot("rc.1")]
#[case::numeric("1")]
#[case::hyphen("pre-release")]
fn valid_prerelease_identifier(#[case] id: &str) {
    let config = loaded(&format!(
        "branches.rules = [{{ name = \"beta\", prerelease = \"{id}\" }}]"
    ));

    assert_eq!(
        describe_rules(&config),
        [format!("prerelease beta id='{id}' channel=-")]
    );
}

#[test]
fn prerelease_true_on_a_glob_is_checked_after_expansion() {
    let config = loaded(r#"branches.rules = [{ name = "preview/*", prerelease = true }]"#);

    assert_eq!(
        describe_rules(&config),
        ["prerelease preview/* id=<branch> channel=-"]
    );
}

#[rstest]
#[case::major("1.x", true)]
#[case::major_x_x("1.x.x", true)]
#[case::minor("1.2.x", true)]
#[case::upper_x("1.X", true)]
#[case::multi_digit("12.34.x", true)]
#[case::release_branch("main", false)]
#[case::full_version("1.2.3", false)]
#[case::no_number("x.x", false)]
#[case::prefixed("release/1.x", false)]
fn default_maintenance_rule_matches_range_shaped_names(
    #[case] branch: &str,
    #[case] matches: bool,
) {
    let config = loaded("");

    let [BranchRule::Maintenance(rule), ..] = config.branches().rules() else {
        panic!("{:?}", describe_rules(&config));
    };
    assert_eq!(rule.matches(branch), matches, "{branch}");
}

#[rstest]
#[case::prefixed("release/1.2.x", true)]
#[case::without_prefix("1.2.x", false)]
#[case::other_prefix("hotfix/1.2.x", false)]
fn prefixed_maintenance_pattern(#[case] branch: &str, #[case] matches: bool) {
    let config = loaded(r#"branches.rules = [{ maintenance = "release/N.x" }]"#);

    let [BranchRule::Maintenance(rule)] = config.branches().rules() else {
        panic!("{:?}", describe_rules(&config));
    };
    assert_eq!(rule.matches(branch), matches, "{branch}");
}

#[rstest]
#[case::exact("legacy", true)]
#[case::other("legacy-2", false)]
#[case::range_shaped("1.x", false)]
fn named_maintenance_rule_matches_its_name_only(#[case] branch: &str, #[case] matches: bool) {
    let config = loaded(r#"branches.rules = [{ maintenance = "legacy", range = "1.x" }]"#);

    let [BranchRule::Maintenance(rule)] = config.branches().rules() else {
        panic!("{:?}", describe_rules(&config));
    };
    assert_eq!(rule.matches(branch), matches, "{branch}");
}

#[test]
fn tags() {
    let config = loaded(
        r#"
[tags]
format = "release-{version}"
metadata = "{{ commit.short_sha }}"
"#,
    );

    assert_eq!(config.tags().format().as_str(), "release-{version}");
    assert_eq!(
        config.tags().metadata().map(super::Template::as_str),
        Some("{{ commit.short_sha }}")
    );
}

#[test]
fn steps() {
    let config = loaded(
        r#"
[steps]
plugins = ["commit-analyzer", "release-notes", "git", "github", "npm"]
publish.order = ["npm", "github"]
success.order = ["github"]
success.errors = "fail"
"#,
    );

    assert_eq!(
        config.steps().plugins(),
        plugins(&["commit-analyzer", "release-notes", "git", "github", "npm"])
    );
    assert_eq!(
        config.steps().order(Step::Publish),
        Some(plugins(&["npm", "github"]).as_slice())
    );
    assert_eq!(
        config.steps().order(Step::Success),
        Some(plugins(&["github"]).as_slice())
    );
    assert_eq!(config.steps().order(Step::Prepare), None);
    assert_eq!(config.steps().success_errors(), SuccessErrors::Fail);
}

#[rstest]
#[case::warn("warn", SuccessErrors::Warn)]
#[case::fail("fail", SuccessErrors::Fail)]
fn success_errors(#[case] value: &str, #[case] expected: SuccessErrors) {
    let config = loaded(&format!("steps.success.errors = \"{value}\""));

    assert_eq!(config.steps().success_errors(), expected);
}

#[rstest]
#[case::verify_conditions("verify_conditions", Step::VerifyConditions)]
#[case::analyze_commits("analyze_commits", Step::AnalyzeCommits)]
#[case::verify_release("verify_release", Step::VerifyRelease)]
#[case::generate_notes("generate_notes", Step::GenerateNotes)]
#[case::prepare("prepare", Step::Prepare)]
#[case::publish("publish", Step::Publish)]
#[case::add_channel("add_channel", Step::AddChannel)]
#[case::success("success", Step::Success)]
#[case::fail("fail", Step::Fail)]
fn every_step_takes_an_order(#[case] name: &str, #[case] step: Step) {
    let config = loaded(&format!("steps.{name}.order = [\"release-notes\"]"));

    assert_eq!(
        config.steps().order(step),
        Some(plugins(&["release-notes"]).as_slice())
    );
}

#[rstest]
#[case::simple("git")]
#[case::dashed("commit-analyzer")]
#[case::digits("s3-upload2")]
fn valid_plugin_name(#[case] name: &str) {
    let config = loaded(&format!("steps.plugins = [\"{name}\"]"));

    assert_eq!(config.steps().plugins(), plugins(&[name]));
}

#[test]
fn plugin_config() {
    let config = loaded(
        r#"
steps.plugins = ["commit-analyzer", "github"]

[plugins.github]
version = "1.4.2"
timeouts.publish = "30m"
timeouts.success = "90s"
timeouts.prepare = "1h"
show_output = true
assets = ["dist/*.tar.gz"]
draft = { enabled = true, limit = 3, ratio = 0.5 }
"#,
    );

    let github = config.plugin(&plugin("github"));
    assert!(github.is_some());
    let github = github.unwrap();
    assert_eq!(github.version(), Some(&Version::new(1, 4, 2)));
    assert_eq!(github.timeout(Step::Publish), Some(Duration::from_mins(30)));
    assert_eq!(github.timeout(Step::Success), Some(Duration::from_secs(90)));
    assert_eq!(github.timeout(Step::Prepare), Some(Duration::from_hours(1)));
    assert_eq!(github.timeout(Step::Fail), None);
    assert!(github.show_output());
    assert_eq!(
        serde_json::Value::Object(github.options().clone()),
        serde_json::json!({
            "assets": ["dist/*.tar.gz"],
            "draft": { "enabled": true, "limit": 3, "ratio": 0.5 },
        })
    );
}

#[test]
fn unknown_plugin_option_keys_are_left_to_the_plugin() {
    let config = loaded("[plugins.release-notes]\nversion_from = \"tags\"");

    let notes = config.plugin(&plugin("release-notes"));
    assert!(notes.is_some());
    assert_eq!(
        notes.unwrap().options().get("version_from"),
        Some(&serde_json::json!("tags"))
    );
}

#[test]
fn mask_env() {
    let config = loaded(r#"secrets.mask_env = ["DEPLOY_TOKEN", "_private2"]"#);

    let names: Vec<&str> = config
        .secrets()
        .mask_env()
        .iter()
        .map(EnvName::as_str)
        .collect();
    assert_eq!(names, ["DEPLOY_TOKEN", "_private2"]);
}

// --- Rejections: code and key path ---

#[rstest]
// config
#[case::merge(r#"config.merge = "partial""#, CONFIG_INVALID_VALUE, "config.merge")]
#[case::extends(
    r#"config.extends = "preset:rust""#,
    CONFIG_UNSUPPORTED_SECTION,
    "config.extends"
)]
#[case::extends_list(
    r#"config.extends = ["./a.toml"]"#,
    CONFIG_UNSUPPORTED_SECTION,
    "config.extends"
)]
#[case::config_unknown_key("config.strict = true", CONFIG_UNKNOWN_KEY, "config.strict")]
#[case::packages(
    "[packages.web]\npath = \"web\"",
    CONFIG_UNSUPPORTED_SECTION,
    "packages"
)]
// commits
#[case::preset(r#"commits.preset = "eslint""#, CONFIG_INVALID_VALUE, "commits.preset")]
#[case::commits_unknown_key("commits.types = []", CONFIG_UNKNOWN_KEY, "commits.types")]
// version
#[case::initial_two_parts(r#"version.initial = "1.0""#, CONFIG_INVALID_VALUE, "version.initial")]
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
// branches: shape
#[case::branches_not_a_list(r#"branches.rules = "main""#, CONFIG_INVALID_VALUE, "branches.rules")]
#[case::no_branch_rules("branches.rules = []", CONFIG_INVALID_VALUE, "branches.rules")]
#[case::branches_unknown_key(
    r#"branches.remote = "origin""#,
    CONFIG_UNKNOWN_KEY,
    "branches.remote"
)]
#[case::rule_unknown_key(
    r#"branches.rules = [{ name = "main", colour = "red" }]"#,
    CONFIG_UNKNOWN_KEY,
    "branches.rules[0].colour"
)]
#[case::rule_without_name(
    r#"branches.rules = [{ channel = "next" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0]"
)]
#[case::rule_not_a_string_or_table(
    "branches.rules = [1]",
    CONFIG_INVALID_VALUE,
    "branches.rules[0]"
)]
// branches: conflicting keys (the path names the extra key)
#[case::name_and_maintenance(
    r#"branches.rules = [{ name = "beta", maintenance = "N.x" }]"#,
    CONFIG_CONFLICTING_KEYS,
    "branches.rules[0].maintenance"
)]
#[case::prerelease_on_maintenance(
    r#"branches.rules = [{ maintenance = "N.x", prerelease = true }]"#,
    CONFIG_CONFLICTING_KEYS,
    "branches.rules[0].prerelease"
)]
#[case::range_with_range_pattern(
    r#"branches.rules = [{ maintenance = "N.x", range = "1.x" }]"#,
    CONFIG_CONFLICTING_KEYS,
    "branches.rules[0].range"
)]
// branches: maintenance
#[case::maintenance_without_range(
    r#"branches.rules = [{ maintenance = "legacy" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].maintenance"
)]
#[case::maintenance_pattern_not_at_end(
    r#"branches.rules = [{ maintenance = "N.x/legacy" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].maintenance"
)]
#[case::range_full_version(
    r#"branches.rules = [{ maintenance = "legacy", range = "1.0.0" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].range"
)]
#[case::range_semver_range(
    r#"branches.rules = [{ maintenance = "legacy", range = ">=1 <2" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].range"
)]
// branches: upstream-style maintenance entries
#[case::range_shaped_string(
    r#"branches.rules = ["1.x"]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0]"
)]
#[case::range_shaped_name(
    r#"branches.rules = [{ name = "1.2.x" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].name"
)]
#[case::range_shaped_name_upper(
    r#"branches.rules = [{ name = "2.X" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].name"
)]
#[case::range_on_name(
    r#"branches.rules = [{ name = "legacy", range = "1.x" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].range"
)]
// branches: prerelease
#[case::prerelease_false(
    r#"branches.rules = [{ name = "beta", prerelease = false }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].prerelease"
)]
#[case::prerelease_invalid_char(
    r#"branches.rules = [{ name = "beta", prerelease = "rc!" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].prerelease"
)]
#[case::prerelease_empty(
    r#"branches.rules = [{ name = "beta", prerelease = "" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].prerelease"
)]
#[case::prerelease_leading_zero(
    r#"branches.rules = [{ name = "beta", prerelease = "01" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].prerelease"
)]
#[case::prerelease_true_on_invalid_name(
    r#"branches.rules = [{ name = "feature/x", prerelease = true }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].prerelease"
)]
#[case::prerelease_unknown_placeholder(
    r#"branches.rules = [{ name = "beta", prerelease = "{branch}" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].prerelease"
)]
// branches: channel
#[case::channel_true(
    r#"branches.rules = ["main", { name = "next", channel = true }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[1].channel"
)]
#[case::channel_empty(
    r#"branches.rules = [{ name = "next", channel = "" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].channel"
)]
#[case::channel_unknown_placeholder(
    r#"branches.rules = [{ name = "next", channel = "{branch}" }]"#,
    CONFIG_INVALID_VALUE,
    "branches.rules[0].channel"
)]
// tags
#[case::tag_format_without_version(
    r#"tags.format = "release-tag""#,
    CONFIG_INVALID_VALUE,
    "tags.format"
)]
#[case::tag_format_twice(
    r#"tags.format = "{version}-{version}""#,
    CONFIG_INVALID_VALUE,
    "tags.format"
)]
#[case::tags_unknown_key(r#"tags.prefix = "v{version}""#, CONFIG_UNKNOWN_KEY, "tags.prefix")]
#[case::domain_not_a_table(r#"tags = "v{version}""#, CONFIG_INVALID_VALUE, "tags")]
// steps
#[case::plugins_not_a_list(r#"steps.plugins = "git""#, CONFIG_INVALID_VALUE, "steps.plugins")]
#[case::plugin_name_uppercase(
    r#"steps.plugins = ["GitHub"]"#,
    CONFIG_INVALID_VALUE,
    "steps.plugins[0]"
)]
#[case::plugin_name_leading_digit(
    r#"steps.plugins = ["3s"]"#,
    CONFIG_INVALID_VALUE,
    "steps.plugins[0]"
)]
#[case::plugin_name_leading_dash(
    r#"steps.plugins = ["-git"]"#,
    CONFIG_INVALID_VALUE,
    "steps.plugins[0]"
)]
#[case::plugin_name_trailing_dash(
    r#"steps.plugins = ["git-"]"#,
    CONFIG_INVALID_VALUE,
    "steps.plugins[0]"
)]
#[case::plugin_name_double_dash(
    r#"steps.plugins = ["git--hub"]"#,
    CONFIG_INVALID_VALUE,
    "steps.plugins[0]"
)]
#[case::plugin_name_underscore(
    r#"steps.plugins = ["git_hub"]"#,
    CONFIG_INVALID_VALUE,
    "steps.plugins[0]"
)]
#[case::plugin_name_empty(r#"steps.plugins = [""]"#, CONFIG_INVALID_VALUE, "steps.plugins[0]")]
#[case::duplicate_plugin(
    r#"steps.plugins = ["git", "git"]"#,
    CONFIG_INVALID_VALUE,
    "steps.plugins[1]"
)]
#[case::unknown_step(r#"steps.deploy.order = ["git"]"#, CONFIG_UNKNOWN_KEY, "steps.deploy")]
#[case::step_unknown_key(
    "steps.publish.parallel = true",
    CONFIG_UNKNOWN_KEY,
    "steps.publish.parallel"
)]
#[case::order_of_disabled_plugin(
    r#"steps.publish.order = ["npm"]"#,
    CONFIG_PLUGIN_NOT_ENABLED,
    "steps.publish.order[0]"
)]
#[case::order_invalid_name(
    r#"steps.publish.order = ["Git"]"#,
    CONFIG_INVALID_VALUE,
    "steps.publish.order[0]"
)]
#[case::errors_on_another_step(
    r#"steps.publish.errors = "fail""#,
    CONFIG_UNKNOWN_KEY,
    "steps.publish.errors"
)]
#[case::success_errors(
    r#"steps.success.errors = "ignore""#,
    CONFIG_INVALID_VALUE,
    "steps.success.errors"
)]
// plugins
#[case::options_of_disabled_plugin(
    "[plugins.npm]\ntag = \"next\"",
    CONFIG_PLUGIN_NOT_ENABLED,
    "plugins.npm"
)]
#[case::plugin_version(
    "[plugins.release-notes]\nversion = \"1.4\"",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.version"
)]
#[case::timeout_words(
    "[plugins.release-notes]\ntimeouts.publish = \"1 hour\"",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.timeouts.publish"
)]
#[case::timeout_zero(
    "[plugins.release-notes]\ntimeouts.publish = \"0s\"",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.timeouts.publish"
)]
#[case::timeout_no_unit(
    "[plugins.release-notes]\ntimeouts.publish = \"30\"",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.timeouts.publish"
)]
#[case::timeout_integer(
    "[plugins.release-notes]\ntimeouts.publish = 30",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.timeouts.publish"
)]
#[case::timeout_days(
    "[plugins.release-notes]\ntimeouts.publish = \"30d\"",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.timeouts.publish"
)]
#[case::timeout_fraction(
    "[plugins.release-notes]\ntimeouts.publish = \"1.5h\"",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.timeouts.publish"
)]
#[case::timeout_unknown_step(
    "[plugins.release-notes]\ntimeouts.deploy = \"1h\"",
    CONFIG_UNKNOWN_KEY,
    "plugins.release-notes.timeouts.deploy"
)]
#[case::show_output_not_bool(
    "[plugins.release-notes]\nshow_output = \"yes\"",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.show_output"
)]
#[case::option_datetime(
    "[plugins.release-notes]\nsince = 2026-01-01",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.since"
)]
#[case::option_datetime_in_array(
    "[plugins.release-notes]\nwindows = [{ start = 2026-01-01T00:00:00Z }]",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.windows[0].start"
)]
#[case::option_datetime_in_table(
    "[plugins.release-notes]\nrange = { from = 08:00:00 }",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.range.from"
)]
#[case::option_nan(
    "[plugins.release-notes]\nratio = nan",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.ratio"
)]
#[case::option_inf_in_array(
    "[plugins.release-notes]\nlimits = [1.0, inf]",
    CONFIG_INVALID_VALUE,
    "plugins.release-notes.limits[1]"
)]
// secrets
#[case::env_name_dash(
    r#"secrets.mask_env = ["MY-TOKEN"]"#,
    CONFIG_INVALID_VALUE,
    "secrets.mask_env[0]"
)]
#[case::env_name_leading_digit(
    r#"secrets.mask_env = ["1TOKEN"]"#,
    CONFIG_INVALID_VALUE,
    "secrets.mask_env[0]"
)]
#[case::env_name_empty(
    r#"secrets.mask_env = [""]"#,
    CONFIG_INVALID_VALUE,
    "secrets.mask_env[0]"
)]
#[case::mask_env_not_a_list(
    r#"secrets.mask_env = "TOKEN""#,
    CONFIG_INVALID_VALUE,
    "secrets.mask_env"
)]
#[case::secrets_unknown_key("secrets.files = []", CONFIG_UNKNOWN_KEY, "secrets.files")]
// top level
#[case::unknown_domain("[colour]\nname = \"red\"", CONFIG_UNKNOWN_KEY, "colour")]
fn invalid_config_is_rejected(#[case] text: &str, #[case] code: ErrorCode, #[case] path: &str) {
    assert_eq!(rejection(text), Err((code, path.to_owned())));
}

#[test]
fn error_message_names_the_key_and_the_bad_value() {
    let error = parse(r#"tags.format = "release-tag""#);

    assert!(error.is_err());
    let message = error.unwrap_err().to_string();
    assert!(message.contains("`tags.format`"), "{message}");
    assert!(message.contains("release-tag"), "{message}");
}

// --- Examples and round trip ---

/// CONFIG.md's full example, verbatim.
const CONFIG_MD_EXAMPLE: &str = r#"
[config]
extends = "preset:rust"
merge = "deep"

[commits]
preset = "conventionalcommits"

[version]
initial = "0.1.0"
zero = { breaking = "minor", feature = "patch", fix = "patch" }

[branches]
rules = [{ maintenance = "N.x" }, "main", { name = "beta", prerelease = true }]

[tags]
format = "v{version}"

[steps]
plugins = ["commit-analyzer", "release-notes", "git", "github"]
publish.order = ["github", "git"]
success.errors = "warn"

[plugins.github]
version = "1.4.2"

[secrets]
mask_env = ["DEPLOY_TOKEN"]
"#;

#[test]
fn config_md_example_is_rejected_until_extends_is_supported() {
    assert_eq!(
        rejection(CONFIG_MD_EXAMPLE),
        Err((CONFIG_UNSUPPORTED_SECTION, String::from("config.extends")))
    );
}

#[test]
fn config_md_example_without_extends_loads() {
    let config = loaded(&CONFIG_MD_EXAMPLE.replace("extends = \"preset:rust\"\n", ""));

    assert_eq!(config.config().merge(), MergeMode::Deep);
    assert_eq!(config.commits().preset(), Preset::ConventionalCommits);
    assert_eq!(config.version().initial(), &Version::new(0, 1, 0));
    let zero = config.version().zero();
    assert_eq!(
        (zero.breaking(), zero.feature(), zero.fix()),
        (Level::Minor, Level::Patch, Level::Patch)
    );
    assert_eq!(
        describe_rules(&config),
        [
            "maintenance N.x range=- channel=-",
            "release main channel=-",
            "prerelease beta id=<branch> channel=-",
        ]
    );
    assert_eq!(config.tags().format().as_str(), "v{version}");
    assert_eq!(
        config.steps().plugins(),
        plugins(&["commit-analyzer", "release-notes", "git", "github"])
    );
    assert_eq!(
        config.steps().order(Step::Publish),
        Some(plugins(&["github", "git"]).as_slice())
    );
    assert_eq!(config.steps().success_errors(), SuccessErrors::Warn);
    assert_eq!(
        config
            .plugin(&plugin("github"))
            .and_then(|github| github.version()),
        Some(&Version::new(1, 4, 2))
    );
    let names: Vec<&str> = config
        .secrets()
        .mask_env()
        .iter()
        .map(EnvName::as_str)
        .collect();
    assert_eq!(names, ["DEPLOY_TOKEN"]);
}

/// Every key set to a value that differs from its default.
const EVERY_KEY_CHANGED: &str = r#"
[config]
merge = "shallow"

[commits]
preset = "angular"

[version]
initial = "0.1.0"
zero = { breaking = "major", feature = "minor", fix = "minor" }

[branches]
rules = [
  { maintenance = "release/N.N.x", channel = "{name}" },
  { maintenance = "legacy", range = "1.x", channel = false },
  { name = "trunk", channel = "stable" },
  { name = "rc/*", prerelease = "rc-{name}", channel = "{name}" },
]

[tags]
format = "release-{version}"
metadata = "{{ commit.short_sha }}"

[steps]
plugins = ["release-notes", "github"]
verify_conditions.order = ["github", "release-notes"]
success.order = ["github"]
success.errors = "fail"

[plugins.github]
version = "1.4.2"
timeouts.publish = "30m"
show_output = true
assets = ["a", { path = "b", label = "B" }]

[secrets]
mask_env = ["DEPLOY_TOKEN"]
"#;

#[rstest]
#[case::defaults("")]
#[case::every_key_changed(EVERY_KEY_CHANGED)]
fn config_round_trips_through_toml(#[case] text: &str) {
    let config = loaded(text);

    assert_eq!(Config::from_table(config.to_table()), Ok(config));
}

#[test]
fn every_key_changed_differs_from_the_defaults_in_every_domain() {
    let config = loaded(EVERY_KEY_CHANGED);
    let defaults = loaded("");

    assert_ne!(config.config(), defaults.config());
    assert_ne!(config.commits(), defaults.commits());
    assert_ne!(config.version(), defaults.version());
    assert_ne!(config.branches(), defaults.branches());
    assert_ne!(config.tags(), defaults.tags());
    assert_ne!(config.steps(), defaults.steps());
    assert_ne!(config.secrets(), defaults.secrets());
    assert_ne!(
        config.plugin(&plugin("github")),
        defaults.plugin(&plugin("github"))
    );
}
