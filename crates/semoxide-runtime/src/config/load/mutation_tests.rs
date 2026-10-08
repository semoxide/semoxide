use semoxide_error::ErrorInfo;
use tempfile::TempDir;

use super::load;

#[test]
fn the_did_you_mean_hint_survives_loading() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("semoxide.toml"),
        "tags.formta = \"v{version}\"",
    )
    .unwrap();

    let help = load(dir.path(), &[]).err().and_then(|error| error.help());

    assert_eq!(help.as_deref(), Some("Use `tags.format` instead."));
}
