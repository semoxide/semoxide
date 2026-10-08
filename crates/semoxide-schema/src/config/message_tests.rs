use super::{Config, PluginName};

#[test]
fn prerelease_false_says_to_drop_the_key() {
    let table = r#"branches.rules = [{ name = "beta", prerelease = false }]"#
        .parse::<toml::Table>()
        .unwrap();

    let error = Config::from_table(table);

    assert!(error.is_err());
    let message = error.unwrap_err().to_string();
    assert!(
        message.contains("a release branch has no `prerelease` key"),
        "{message}"
    );
}

#[test]
fn plugin_name_as_written() {
    let name: PluginName = "commit-analyzer".parse().unwrap();

    assert_eq!(name.as_str(), "commit-analyzer");
    assert_eq!(name.to_string(), "commit-analyzer");
}
