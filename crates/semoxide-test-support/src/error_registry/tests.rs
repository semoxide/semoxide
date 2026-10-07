use semoxide_error::ErrorCode;

use super::{Problem, codes_in_source, constant_name, registry_problems};

const CORE_NO_GIT_REPO: ErrorCode = ErrorCode::from_static("core::no_git_repo");
const GIT_PUSH_REJECTED: ErrorCode = ErrorCode::from_static("git::push_rejected");

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn consistent_registry_has_no_problems() {
    let problems = registry_problems(
        &[CORE_NO_GIT_REPO, GIT_PUSH_REJECTED],
        &strings(&["core/no-git-repo", "git/push-rejected"]),
        &strings(&["core::no_git_repo", "git::push_rejected"]),
    );

    assert_eq!(problems, []);
}

#[test]
fn removed_page_is_a_missing_page() {
    let problems = registry_problems(
        &[CORE_NO_GIT_REPO, GIT_PUSH_REJECTED],
        &strings(&["core/no-git-repo"]),
        &[],
    );

    assert_eq!(problems, [Problem::MissingPage(GIT_PUSH_REJECTED)]);
}

#[test]
fn page_without_a_code_is_an_orphan() {
    let problems = registry_problems(
        &[CORE_NO_GIT_REPO],
        &strings(&["core/no-git-repo", "core/removed-error"]),
        &[],
    );

    assert_eq!(
        problems,
        [Problem::OrphanPage(String::from("core/removed-error"))]
    );
}

#[test]
fn code_listed_twice_is_a_duplicate() {
    let problems = registry_problems(
        &[CORE_NO_GIT_REPO, CORE_NO_GIT_REPO],
        &strings(&["core/no-git-repo"]),
        &[],
    );

    assert_eq!(problems, [Problem::Duplicate(CORE_NO_GIT_REPO)]);
}

#[test]
fn codes_with_the_same_constant_name_collide() {
    let dashed = ErrorCode::from_static("a-b::c");
    let underscored = ErrorCode::from_static("a::b_c");

    let problems = registry_problems(
        &[dashed.clone(), underscored.clone()],
        &strings(&["a-b/c", "a/b-c"]),
        &[],
    );

    assert_eq!(problems, [Problem::NameCollision(dashed, underscored)]);
}

#[test]
fn code_in_source_but_not_in_all_is_not_registered() {
    let problems = registry_problems(
        &[CORE_NO_GIT_REPO],
        &strings(&["core/no-git-repo"]),
        &strings(&["core::no_git_repo", "git::auth_failed"]),
    );

    assert_eq!(
        problems,
        [Problem::NotRegistered(String::from("git::auth_failed"))]
    );
}

#[test]
fn constant_name_follows_the_code() {
    assert_eq!(constant_name(&CORE_NO_GIT_REPO), "CORE_NO_GIT_REPO");
    assert_eq!(
        constant_name(&ErrorCode::from_static("commit-analyzer::invalid_rule")),
        "COMMIT_ANALYZER_INVALID_RULE"
    );
}

#[test]
fn source_scan_finds_codes_and_skips_comments() {
    let source = r#"
pub const GIT_PUSH_REJECTED: ErrorCode = ErrorCode::from_static("git::push_rejected");
// const OLD: ErrorCode = ErrorCode::from_static("git::old_code");
/// const DOC: ErrorCode = ErrorCode::from_static("git::doc_example");
let pair = (ErrorCode::from_static("a::b"), ErrorCode::from_static("c::d"));
"#;

    assert_eq!(
        codes_in_source(source),
        ["git::push_rejected", "a::b", "c::d"]
    );
}

#[test]
fn several_problems_are_all_reported_in_a_fixed_order() {
    let problems = registry_problems(
        &[CORE_NO_GIT_REPO, GIT_PUSH_REJECTED],
        &strings(&["core/no-git-repo", "core/removed-error"]),
        &strings(&[
            "core::no_git_repo",
            "git::push_rejected",
            "git::auth_failed",
        ]),
    );

    assert_eq!(
        problems,
        [
            Problem::MissingPage(GIT_PUSH_REJECTED),
            Problem::OrphanPage(String::from("core/removed-error")),
            Problem::NotRegistered(String::from("git::auth_failed")),
        ]
    );
}

#[test]
fn page_named_with_underscores_is_an_orphan_and_the_code_misses_its_page() {
    let problems = registry_problems(&[CORE_NO_GIT_REPO], &strings(&["core/no_git_repo"]), &[]);

    assert_eq!(
        problems,
        [
            Problem::MissingPage(CORE_NO_GIT_REPO),
            Problem::OrphanPage(String::from("core/no_git_repo")),
        ]
    );
}

#[test]
fn source_scan_skips_trailing_and_block_comments() {
    let source = r#"
pub const GIT_X: ErrorCode = ErrorCode::from_static("git::x"); // was from_static("git::old")
/* old codes:
const GIT_GONE: ErrorCode = ErrorCode::from_static("git::gone");
*/ const GIT_Y: ErrorCode = ErrorCode::from_static("git::y");
const URL: &str = "https://example.com"; const GIT_Z: ErrorCode = ErrorCode::from_static("git::z");
"#;

    assert_eq!(codes_in_source(source), ["git::x", "git::y", "git::z"]);
}
