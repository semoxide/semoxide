//! The error-code registry check: every code has a docs page and is listed in its crate's
//! `codes::ALL` (OBSERVABILITY §8).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::Path;

use semoxide_error::ErrorCode;

#[cfg(test)]
mod reader_tests;
#[cfg(test)]
mod scan_tests;
#[cfg(test)]
mod tests;

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

/// The code strings in `ErrorCode::from_static("…")` calls of one source file, skipping comments.
#[must_use]
pub fn codes_in_source(text: &str) -> Vec<String> {
    without_comments(text)
        .split(STATIC_CODE)
        .skip(1)
        .filter_map(|rest| rest.split_once('"').map(|(code, _)| code.to_owned()))
        .collect()
}

/// The source with `//` and (nested) `/* */` comments removed; `//` inside a string literal stays.
/// Raw strings and char literals are not special-cased: a stray quote in them can only make the
/// scan miss or invent a code, which the registry test then reports.
fn without_comments(text: &str) -> String {
    let mut code = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut block_depth = 0_usize;
    while let Some(c) = chars.next() {
        if block_depth > 0 {
            match (c, chars.peek()) {
                ('*', Some('/')) => {
                    chars.next();
                    block_depth -= 1;
                }
                ('/', Some('*')) => {
                    chars.next();
                    block_depth += 1;
                }
                ('\n', _) => code.push('\n'),
                _ => {}
            }
            continue;
        }
        if in_string {
            code.push(c);
            match c {
                '\\' => code.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('/', Some('/')) => {
                // Drop the rest of the line, keep the newline.
                code.extend(chars.by_ref().find(|&n| n == '\n'));
            }
            ('/', Some('*')) => {
                chars.next();
                block_depth = 1;
            }
            _ => {
                in_string = c == '"';
                code.push(c);
            }
        }
    }
    code
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
