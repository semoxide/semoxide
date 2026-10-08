//! How a test reads a golden history from `tests/histories/` and builds it, and which files the
//! loader rejects.

use std::path::{Path, PathBuf};

use rstest::rstest;
use semoxide_test_support::git_fixture::{GitFixture, Merge};
use semoxide_test_support::golden_history::{Expected, GoldenHistory, HistoryError, ReleaseType};

fn histories_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/histories")
}

fn sample(name: &str) -> Result<GoldenHistory, HistoryError> {
    GoldenHistory::load(&histories_dir().join(name))
}

fn parse(text: &str) -> Result<GoldenHistory, HistoryError> {
    GoldenHistory::parse(Path::new("case.toml"), text)
}

#[test]
fn every_sample_loads_and_builds() {
    let histories = GoldenHistory::all(&histories_dir()).unwrap();

    let names: Vec<_> = histories
        .iter()
        .map(|history| {
            history
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let toml_files = std::fs::read_dir(histories_dir())
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "toml")
        })
        .count();
    assert_eq!(names.len(), toml_files, "{names:?}");
    assert!(names.len() >= 3, "{names:?}");
    assert!(names.is_sorted(), "{names:?}");
    for history in &histories {
        history.build().unwrap();
    }
}

#[test]
fn minor_after_fix_sample() {
    let history = sample("minor-after-fix.toml").unwrap();

    let Expected::Release(release) = history.expected() else {
        panic!("{:?}", history.expected());
    };
    assert_eq!(release.version(), "1.1.0");
    assert_eq!(release.release_type(), ReleaseType::Minor);
    assert_eq!(release.channel(), None);
    assert_eq!(history.config(), None);

    let fixture = history.build().unwrap();
    assert_eq!(
        fixture.git(&["log", "--format=%s"]).unwrap(),
        "feat: search\nfix: a bug\nfeat: first feature"
    );
    assert_eq!(
        fixture.remote_git(&["rev-parse", "main"]).unwrap(),
        fixture.rev_parse("main").unwrap()
    );
}

#[test]
fn breaking_on_beta_sample() {
    let history = sample("breaking-on-beta.toml").unwrap();

    let Expected::Release(release) = history.expected() else {
        panic!("{:?}", history.expected());
    };
    assert_eq!(release.version(), "2.0.0-beta.1");
    assert_eq!(release.release_type(), ReleaseType::Major);
    assert_eq!(release.channel(), Some("beta"));

    let fixture = history.build().unwrap();
    assert_eq!(fixture.git(&["branch", "--show-current"]).unwrap(), "beta");
    assert_eq!(
        fixture.remote_git(&["rev-parse", "beta"]).unwrap(),
        fixture.rev_parse("beta").unwrap()
    );
}

#[test]
fn docs_only_sample() {
    let history = sample("docs-only.toml").unwrap();

    assert_eq!(
        history.expected(),
        &Expected::NoRelease(String::from("no_relevant_commits"))
    );
}

#[test]
fn steps_build_the_same_history_as_the_builder() {
    let history = parse(
        r#"
        description = "every step"
        steps = [
          { commit = "feat: first feature" },
          { tag = "v1.0.0" },
          { commit = "fix: a bug" },
          { annotated_tag = "v1.0.1", message = "release 1.0.1" },
          { branch = "next" },
          { commit = "feat: fast" },
          { checkout = "main" },
          { merge = "next", mode = "ff" },
          { branch = "other" },
          { commit = "feat!: breaking" },
          { checkout = "main" },
          { commit = "docs: readme" },
          { merge = "other", mode = "no-ff" },
          { branch = "rebased" },
          { commit = "chore: tidy" },
          { checkout = "main" },
          { commit = "fix: another bug" },
          { checkout = "rebased" },
          { rebase = "main" },
          { note = "v1.0.0", ref = "semoxide-v1.0.0", text = '{"channels":[null]}' },
          "push",
        ]
        [expected]
        no_release = "no_relevant_commits"
        "#,
    )
    .unwrap();
    let built = history.build().unwrap();
    let expected = GitFixture::builder()
        .commit("feat: first feature")
        .tag("v1.0.0")
        .commit("fix: a bug")
        .annotated_tag("v1.0.1", "release 1.0.1")
        .branch("next")
        .commit("feat: fast")
        .checkout("main")
        .merge("next", Merge::FastForward)
        .branch("other")
        .commit("feat!: breaking")
        .checkout("main")
        .commit("docs: readme")
        .merge("other", Merge::NoFastForward)
        .branch("rebased")
        .commit("chore: tidy")
        .checkout("main")
        .commit("fix: another bug")
        .checkout("rebased")
        .rebase("main")
        .note("v1.0.0", "semoxide-v1.0.0", r#"{"channels":[null]}"#)
        .push()
        .build()
        .unwrap();

    for rev in [
        "main",
        "next",
        "other",
        "rebased",
        "v1.0.0",
        "v1.0.1",
        "refs/notes/semoxide-v1.0.0",
    ] {
        assert_eq!(
            built.rev_parse(rev).unwrap(),
            expected.rev_parse(rev).unwrap(),
            "{rev}"
        );
    }
    assert_eq!(
        built.remote_git(&["for-each-ref"]).unwrap(),
        expected.remote_git(&["for-each-ref"]).unwrap()
    );
}

#[test]
fn multi_line_commit_message_keeps_its_footer() {
    let history = parse(
        r#"
        description = "a footer"
        steps = [
          { commit = """
feat: graduate

Release-As: 1.0.0""" },
        ]
        [expected]
        version = "1.0.0"
        release_type = "minor"
        "#,
    )
    .unwrap();
    let fixture = history.build().unwrap();

    assert_eq!(
        fixture.git(&["log", "-1", "--format=%B"]).unwrap(),
        "feat: graduate\n\nRelease-As: 1.0.0"
    );
}

#[test]
fn config_is_written_as_an_untracked_semoxide_toml() {
    let history = parse(
        r#"
        description = "a config"
        steps = [{ commit = "feat: a" }]
        [config]
        initial_version = "0.1.0"
        [expected]
        version = "0.1.0"
        release_type = "minor"
        "#,
    )
    .unwrap();
    let fixture = history.build().unwrap();

    let written = std::fs::read_to_string(fixture.path().join("semoxide.toml")).unwrap();
    assert_eq!(
        written.parse::<toml::Table>().unwrap(),
        *history.config().unwrap()
    );
    assert_eq!(
        fixture.git(&["status", "--porcelain"]).unwrap(),
        "?? semoxide.toml"
    );
}

#[test]
fn without_config_no_semoxide_toml_is_written() {
    let fixture = sample("minor-after-fix.toml").unwrap().build().unwrap();

    assert!(!fixture.path().join("semoxide.toml").exists());
}

#[rstest]
#[case::unknown_top_level_key(
    r#"
description = "x"
steps = []
colour = "red"
[expected]
no_release = "no_relevant_commits"
"#,
    "unknown key `colour`"
)]
#[case::missing_description(
    r#"
steps = []
[expected]
no_release = "no_relevant_commits"
"#,
    "missing `description`"
)]
#[case::missing_steps(
    r#"
description = "x"
[expected]
no_release = "no_relevant_commits"
"#,
    "missing `steps`"
)]
#[case::missing_expected(
    r#"
description = "x"
steps = []
"#,
    "missing `expected`"
)]
#[case::unknown_expected_key(
    r#"
description = "x"
steps = []
[expected]
no_release = "no_relevant_commits"
notes = "x"
"#,
    "unknown key `expected.notes`"
)]
#[case::version_and_no_release(
    r#"
description = "x"
steps = []
[expected]
version = "1.0.0"
release_type = "major"
no_release = "no_relevant_commits"
"#,
    "`expected` has both `version` and `no_release`"
)]
#[case::neither_version_nor_no_release(
    r#"
description = "x"
steps = []
[expected]
release_type = "major"
"#,
    "`expected` needs `version` or `no_release`"
)]
#[case::version_without_release_type(
    r#"
description = "x"
steps = []
[expected]
version = "1.0.0"
"#,
    "missing `expected.release_type`"
)]
#[case::unknown_release_type(
    r#"
description = "x"
steps = []
[expected]
version = "1.0.0-beta.1"
release_type = "prerelease"
"#,
    "unknown release type `prerelease`: expected `major`, `minor` or `patch`"
)]
#[case::unknown_step(
    r#"
description = "x"
steps = [{ squash = "main" }]
[expected]
no_release = "no_relevant_commits"
"#,
    "step 1: unknown step `squash`"
)]
#[case::unknown_bare_step(
    r#"
description = "x"
steps = ["fetch"]
[expected]
no_release = "no_relevant_commits"
"#,
    "step 1: unknown step `fetch`"
)]
#[case::extra_step_key(
    r#"
description = "x"
steps = [{ commit = "feat: a" }, { commit = "feat: b", message = "x" }]
[expected]
no_release = "no_relevant_commits"
"#,
    "step 2: unknown key `message` for `commit`"
)]
#[case::missing_step_argument(
    r#"
description = "x"
steps = [{ annotated_tag = "v1.0.0" }]
[expected]
no_release = "no_relevant_commits"
"#,
    "step 1: `annotated_tag` needs `message`"
)]
#[case::unknown_merge_mode(
    r#"
description = "x"
steps = [{ merge = "next", mode = "octopus" }]
[expected]
no_release = "no_relevant_commits"
"#,
    "step 1: unknown merge mode `octopus`: expected `ff` or `no-ff`"
)]
#[case::not_toml("description = ", "invalid TOML")]
fn invalid_history_is_rejected_naming_the_file(#[case] text: &str, #[case] message: &str) {
    let error = GoldenHistory::parse(Path::new("bad.toml"), text).unwrap_err();

    let error = error.to_string();
    assert!(error.starts_with("`bad.toml`: "), "{error}");
    assert!(error.contains(message), "{error}");
}

#[test]
fn valid_minimal_history_parses() {
    let history = parse(
        r#"
description = "x"
steps = []
[expected]
no_release = "no_relevant_commits"
"#,
    )
    .unwrap();

    assert_eq!(history.description(), "x");
}

#[test]
fn failing_step_names_the_file_and_the_git_command() {
    let history = parse(
        r#"
description = "x"
steps = [{ checkout = "missing" }]
[expected]
no_release = "no_relevant_commits"
"#,
    )
    .unwrap();

    let error = history.build().unwrap_err().to_string();
    assert!(error.starts_with("`case.toml`: "), "{error}");
    assert!(error.contains("`git checkout missing` failed"), "{error}");
}
