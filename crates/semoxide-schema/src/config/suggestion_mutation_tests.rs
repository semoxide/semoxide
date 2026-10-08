use rstest::rstest;
use semoxide_error::ErrorInfo;

use super::Config;

#[rstest]
#[case::success_has_errors(
    r#"steps.success.errorss = "fail""#,
    Some("Use `steps.success.errors` instead.")
)]
#[case::other_steps_have_no_errors(r#"steps.publish.errorss = "fail""#, None)]
fn errors_is_suggested_only_on_the_success_step(#[case] text: &str, #[case] help: Option<&str>) {
    let table = text.parse::<toml::Table>().unwrap();

    let error = Config::from_table(table);

    assert!(error.is_err());
    assert_eq!(error.unwrap_err().help().as_deref(), help);
}
