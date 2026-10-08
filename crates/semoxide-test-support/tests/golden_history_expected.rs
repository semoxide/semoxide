// LOCKED: approved in 31e1964. Do not edit; if a test looks wrong, stop and report.
//! `[expected]` cases beyond the samples: the `patch` release type, and release fields left
//! next to `no_release`.

use std::path::Path;

use rstest::rstest;
use semoxide_test_support::golden_history::{Expected, GoldenHistory, ReleaseType};

#[test]
fn patch_release_type_loads() {
    let history = GoldenHistory::parse(
        Path::new("case.toml"),
        r#"
description = "x"
steps = []
[expected]
version = "1.0.1"
release_type = "patch"
"#,
    )
    .unwrap();

    let Expected::Release(release) = history.expected() else {
        panic!("{:?}", history.expected());
    };
    assert_eq!(release.release_type(), ReleaseType::Patch);
}

#[rstest]
#[case::release_type(r#"release_type = "minor""#)]
#[case::channel(r#"channel = "beta""#)]
fn release_field_next_to_no_release_is_rejected(#[case] leftover: &str) {
    let text = format!(
        "description = \"x\"\nsteps = []\n[expected]\nno_release = \"no_relevant_commits\"\n{leftover}\n"
    );

    let error = GoldenHistory::parse(Path::new("bad.toml"), &text)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "`bad.toml`: `expected.release_type` and `expected.channel` need `version`"
    );
}
