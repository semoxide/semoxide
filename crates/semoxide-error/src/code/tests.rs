use proptest::prelude::*;
use rstest::rstest;

use super::ErrorCode;

#[rstest]
#[case::core("core::no_git_repo", "core/no-git-repo")]
#[case::git("git::push_rejected", "git/push-rejected")]
#[case::plugin_namespace_with_dash("commit-analyzer::invalid_rule", "commit-analyzer/invalid-rule")]
#[case::digits("github::release_exists_2", "github/release-exists-2")]
fn valid_code_parses_with_its_slug(#[case] input: &str, #[case] slug: &str) {
    let code: ErrorCode = input.parse().unwrap();

    assert_eq!(code.as_str(), input);
    assert_eq!(code.to_string(), input);
    assert_eq!(code.slug(), slug);
    assert_eq!(ErrorCode::from_slug(slug).unwrap(), code);
}

#[rstest]
#[case::single_colon("core:no_git_repo")]
#[case::two_separators("core::no::git")]
#[case::empty_namespace("::no_git_repo")]
#[case::empty_name("core::")]
#[case::uppercase_namespace("Core::no_git_repo")]
#[case::uppercase_name("core::NoGitRepo")]
#[case::dash_in_name("core::no-git-repo")]
#[case::namespace_starts_with_digit("2core::x")]
#[case::name_starts_with_underscore("core::_x")]
#[case::upstream_mnemonic("ENOGITREPO")]
#[case::namespace_trailing_dash("commit-analyzer-::x")]
#[case::namespace_double_dash("commit--analyzer::x")]
#[case::leading_whitespace(" core::x")]
#[case::trailing_newline("core::x\n")]
#[case::non_ascii_letter("core::n\u{f6}")]
#[case::cyrillic_lookalike("\u{441}ore::x")]
fn invalid_code_is_rejected(#[case] input: &str) {
    assert!(input.parse::<ErrorCode>().is_err());
}

#[test]
fn length_limit_is_64_characters() {
    let at_limit = format!("core::{}", "a".repeat(58));
    let over_limit = format!("core::{}", "a".repeat(59));
    assert_eq!((at_limit.len(), over_limit.len()), (64, 65));

    assert!(at_limit.parse::<ErrorCode>().is_ok());
    assert!(over_limit.parse::<ErrorCode>().is_err());
}

#[rstest]
#[case::no_separator("core")]
#[case::uppercase("Core/x")]
#[case::empty_name("core/")]
#[case::not_the_canonical_slug("core/no_git_repo")]
fn invalid_slug_is_rejected(#[case] slug: &str) {
    assert!(ErrorCode::from_slug(slug).is_err());
}

proptest! {
    #[test]
    fn slug_round_trips(
        namespace in "[a-z][a-z0-9]{0,9}(-[a-z0-9]{1,9}){0,2}",
        name in "[a-z][a-z0-9_]{0,20}",
    ) {
        let code: ErrorCode = format!("{namespace}::{name}").parse().unwrap();

        prop_assert_eq!(ErrorCode::from_slug(&code.slug()).unwrap(), code);
    }
}
