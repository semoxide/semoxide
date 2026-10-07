//! End-to-end tests of the `semoxide` binary.

use assert_cmd::Command;

#[test]
fn version_flag_prints_name_and_version() {
    let output = Command::cargo_bin("semoxide")
        .expect("binary is built")
        .arg("--version")
        .output()
        .expect("binary runs");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("semoxide {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn bare_invocation_prints_help_and_does_nothing_else() {
    let output = Command::cargo_bin("semoxide")
        .expect("binary is built")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2), "CLI usage exit code (CLI.md)");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: semoxide\n"));
}
