// LOCKED: approved in 8b6e909. Do not edit; if a test looks wrong, stop and report.

use std::fs;
use std::path::Path;

use super::{page_slugs, source_codes};

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn sorted(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items
}

#[test]
fn missing_docs_directory_has_no_pages() {
    let root = tempfile::tempdir().unwrap();

    assert_eq!(
        page_slugs(&root.path().join("docs/errors")).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn pages_are_slugs_of_nested_markdown_files_except_the_top_level_readme() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "README.md", "style guide");
    write(root.path(), "core/no-git-repo.md", "");
    write(root.path(), "commit-analyzer/invalid-rule.md", "");
    write(root.path(), "core/README.md", "");
    write(root.path(), "core/notes.txt", "");

    assert_eq!(
        sorted(page_slugs(root.path()).unwrap()),
        [
            "commit-analyzer/invalid-rule",
            "core/README",
            "core/no-git-repo"
        ]
    );
}

#[test]
fn source_codes_come_from_crate_sources_only() {
    let root = tempfile::tempdir().unwrap();
    let code = |name: &str| format!("const C: ErrorCode = ErrorCode::from_static(\"{name}\");");
    write(root.path(), "semoxide-git/src/codes.rs", &code("git::used"));
    write(
        root.path(),
        "semoxide-git/src/push/mod.rs",
        &code("git::nested"),
    );
    write(
        root.path(),
        "semoxide-git/src/push/tests.rs",
        &code("git::unit_test"),
    );
    write(
        root.path(),
        "semoxide-git/tests/push.rs",
        &code("git::integration_test"),
    );
    write(
        root.path(),
        "semoxide-error/src/lib.rs",
        &code("core::example"),
    );
    write(
        root.path(),
        "semoxide-git/target/debug/build.rs",
        &code("git::build_output"),
    );
    write(root.path(), "semoxide-git/README.md", &code("git::readme"));

    assert_eq!(
        sorted(source_codes(root.path()).unwrap()),
        ["git::nested", "git::used"]
    );
}
