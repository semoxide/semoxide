//! The error-code registry check: every code has a docs page and is listed in its crate's
//! `codes::ALL` (OBSERVABILITY §8).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::Path;

use semoxide_error::ErrorCode;

const STATIC_CODE: &str = "from_static(\"";

/// One way the registry, the docs pages and the source disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// A registered code has no `docs/errors/<slug>.md` page.
    MissingPage(ErrorCode),
    /// A page whose slug is not the slug of any registered code.
    OrphanPage(String),
    /// The same code is registered more than once.
    Duplicate(ErrorCode),
    /// Two codes map to the same constant name (`a-b::c` and `a::b_c` both become `A_B_C`).
    NameCollision(ErrorCode, ErrorCode),
    /// A `from_static("…")` code in the source that no crate lists in `codes::ALL`.
    NotRegistered(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPage(code) => {
                write!(f, "`{code}` has no page docs/errors/{}.md", code.slug())
            }
            Self::OrphanPage(slug) => {
                write!(f, "docs/errors/{slug}.md documents no registered code")
            }
            Self::Duplicate(code) => write!(f, "`{code}` is registered more than once"),
            Self::NameCollision(a, b) => {
                write!(f, "`{a}` and `{b}` map to the same constant name")
            }
            Self::NotRegistered(code) => {
                write!(f, "`{code}` is used in the source but in no `codes::ALL`")
            }
        }
    }
}

/// Compares the registered codes with the docs page slugs and the codes found in the source.
#[must_use]
pub fn registry_problems(
    codes: &[ErrorCode],
    page_slugs: &[String],
    source_codes: &[String],
) -> Vec<Problem> {
    let mut problems = Vec::new();

    let mut seen = BTreeSet::new();
    let mut constant_names: BTreeMap<String, &ErrorCode> = BTreeMap::new();
    for code in codes {
        if !seen.insert(code.as_str()) {
            problems.push(Problem::Duplicate(code.clone()));
            continue;
        }
        if let Some(other) = constant_names.insert(constant_name(code), code) {
            problems.push(Problem::NameCollision(other.clone(), code.clone()));
        }
    }

    let pages: BTreeSet<&str> = page_slugs.iter().map(String::as_str).collect();
    let slugs: BTreeSet<String> = codes.iter().map(ErrorCode::slug).collect();
    problems.extend(
        codes
            .iter()
            .filter(|code| !pages.contains(code.slug().as_str()))
            .map(|code| Problem::MissingPage(code.clone())),
    );
    problems.extend(
        pages
            .iter()
            .filter(|slug| !slugs.contains(**slug))
            .map(|slug| Problem::OrphanPage((*slug).to_owned())),
    );

    let unregistered: BTreeSet<&str> = source_codes
        .iter()
        .map(String::as_str)
        .filter(|code| !seen.contains(code))
        .collect();
    problems.extend(
        unregistered
            .into_iter()
            .map(|code| Problem::NotRegistered(code.to_owned())),
    );

    problems
}

/// The constant name for a code: `core::no_git_repo` → `CORE_NO_GIT_REPO`.
#[must_use]
pub fn constant_name(code: &ErrorCode) -> String {
    code.as_str()
        .replace("::", "_")
        .replace('-', "_")
        .to_ascii_uppercase()
}

/// The code strings in `ErrorCode::from_static("…")` calls of one source file.
///
/// A plain-text scan, like the Rust compiler's `tidy` checks: lines starting with `//` are
/// skipped, and the scan stops at the first `#[cfg(test)]` line (the inline unit tests, whose
/// example codes are not real). Known limits: a code inside a trailing `// …` comment, a
/// `/* … */` block or a string is counted too (a false "not in `codes::ALL`" report; delete the
/// commented-out code), a code written as a raw string, `from_static(r"…")`, is not found, and
/// a code after a `#[cfg(test)]` item that isn't the test module is not found either.
#[must_use]
pub fn codes_in_source(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|line| line.trim() != "#[cfg(test)]")
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(|line| line.split(STATIC_CODE).skip(1))
        .filter_map(|rest| rest.split_once('"').map(|(code, _)| code.to_owned()))
        .collect()
}

/// The slugs of all pages under `docs_dir` (`core/no-git-repo` for `core/no-git-repo.md`),
/// except a top-level `README.md`. A missing directory has no pages.
///
/// # Errors
///
/// Returns an I/O error if the directory can't be read.
pub fn page_slugs(docs_dir: &Path) -> io::Result<Vec<String>> {
    let mut slugs = Vec::new();
    if docs_dir.is_dir() {
        collect_pages(docs_dir, docs_dir, &mut slugs)?;
    }
    Ok(slugs)
}

fn collect_pages(root: &Path, dir: &Path, slugs: &mut Vec<String>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_pages(root, &path, slugs)?;
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if relative == "README.md" {
            continue;
        }
        if let Some(slug) = relative.strip_suffix(".md") {
            slugs.push(slug.to_owned());
        }
    }
    Ok(())
}

/// The codes used in the crates under `crates_dir`. Skips `semoxide-error` (its docs and tests
/// use example codes), `tests/` directories, `tests.rs` files and `target/`.
///
/// # Errors
///
/// Returns an I/O error if a directory or file can't be read.
pub fn source_codes(crates_dir: &Path) -> io::Result<Vec<String>> {
    let mut codes = Vec::new();
    collect_source_codes(crates_dir, &mut codes)?;
    Ok(codes)
}

fn collect_source_codes(dir: &Path, codes: &mut Vec<String>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if path.is_dir() {
            if !matches!(name.as_str(), "semoxide-error" | "tests" | "target") {
                collect_source_codes(&path, codes)?;
            }
            continue;
        }
        let is_rust = path.extension().is_some_and(|extension| extension == "rs");
        if is_rust && name != "tests.rs" {
            codes.extend(codes_in_source(&std::fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use semoxide_error::ErrorCode;

    use super::*;

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
    fn source_scan_stops_at_the_test_module() {
        let source = r#"
pub const GIT_PUSH_REJECTED: ErrorCode = ErrorCode::from_static("git::push_rejected");

#[cfg(test)]
mod tests {
    const EXAMPLE: ErrorCode = ErrorCode::from_static("test::example");
}
"#;

        assert_eq!(codes_in_source(source), ["git::push_rejected"]);
    }

    #[test]
    fn source_scan_reads_a_cfg_test_line_that_is_not_alone() {
        let source = r#"
#[cfg(test)] use std::fmt;
pub const GIT_PUSH_REJECTED: ErrorCode = ErrorCode::from_static("git::push_rejected");
"#;

        assert_eq!(codes_in_source(source), ["git::push_rejected"]);
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
}
