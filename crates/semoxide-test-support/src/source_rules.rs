//! Source rules for the crates' files, as plain-text scans like the Rust compiler's `tidy`
//! checks: `#![forbid(unsafe_code)]` in crate roots (CLAUDE.md Rust rule 6), no inline test
//! modules (CODE-ARCHITECTURE §8), private tuple fields (Rust rule 4), every module file declared
//! and one integration-test binary per crate (TESTING: layout).
//!
//! Lines starting with `//` are skipped. Known limits: text inside `/* … */` blocks or strings
//! that starts a line can still match, and a tuple struct spread over several lines isn't seen.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod wiring_tests;

/// Whether the file has a crate-level `#![forbid(…)]` (at the start of a line, not indented)
/// that lists `unsafe_code`.
#[must_use]
pub fn forbids_unsafe_code(text: &str) -> bool {
    code_lines(text).any(|line| {
        line.strip_prefix("#![forbid(")
            .and_then(|rest| rest.split_once(")]"))
            .is_some_and(|(lints, _)| lints.split(',').any(|lint| lint.trim() == "unsafe_code"))
    })
}

/// Names of modules declared `#[cfg(test)] mod name { … }` with a body instead of `mod name;`.
/// `cfg(all(test, …))` counts as a test cfg; a file containing `#![cfg(test)]` is test-only and
/// skipped.
#[must_use]
pub fn inline_test_modules(text: &str) -> Vec<String> {
    let lines: Vec<&str> = code_lines(text).map(str::trim).collect();
    if lines.contains(&"#![cfg(test)]") {
        return Vec::new();
    }
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == "#[cfg(test)]" || line.starts_with("#[cfg(all(test"))
        .filter_map(|(index, _)| {
            let declaration = lines
                .iter()
                .skip(index + 1)
                .find(|line| !line.starts_with("#["))?;
            inline_module_name(declaration).map(str::to_owned)
        })
        .collect()
}

/// `mod name {` (any visibility) gives `name`; `mod name;` and anything else give nothing.
fn inline_module_name(line: &str) -> Option<&str> {
    let rest = without_visibility(line).strip_prefix("mod ")?;
    let end = rest.find(|c: char| c == '{' || c == ';' || c.is_whitespace())?;
    let (name, after) = rest.split_at(end);
    after.trim_start().starts_with('{').then_some(name)
}

/// Names of tuple structs with a `pub` field, e.g. `struct Tag(pub String);`.
#[must_use]
pub fn pub_tuple_fields(text: &str) -> Vec<String> {
    code_lines(text)
        .filter_map(|line| {
            let rest = without_visibility(line.trim()).strip_prefix("struct ")?;
            let (name, fields) = rest.split_at(rest.find(['(', '{', ';'])?);
            let fields = fields.strip_prefix('(')?;
            let fields = fields.rsplit_once(')').map_or(fields, |(fields, _)| fields);
            let name = name.split('<').next().unwrap_or(name).trim();
            fields.split(',').any(is_pub_field).then(|| name.to_owned())
        })
        .collect()
}

/// A tuple field, after an optional `#[…]` attribute, starts with `pub`.
fn is_pub_field(field: &str) -> bool {
    let field = field.trim();
    let field = field
        .strip_prefix("#[")
        .and_then(|attribute| attribute.split_once(']'))
        .map_or(field, |(_, after)| after.trim_start());
    field.starts_with("pub ") || field.starts_with("pub(")
}

/// The lines that aren't `//` comments.
fn code_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
}

/// The line without a leading `pub ` or `pub(…) `.
fn without_visibility(line: &str) -> &str {
    let Some(rest) = line.strip_prefix("pub") else {
        return line;
    };
    if let Some(rest) = rest.strip_prefix(' ') {
        return rest;
    }
    rest.strip_prefix('(')
        .and_then(|scope| scope.split_once(')'))
        .map_or(line, |(_, after)| after.trim_start())
}

/// Names in body-less `mod name;` declarations (any visibility), e.g. `tests` for
/// `#[cfg(test)]` + `mod tests;`. Known limit: an attribute on the same line (`#[cfg(test)] mod
/// tests;`) and `#[path]` are not understood.
#[must_use]
pub fn declared_modules(text: &str) -> Vec<String> {
    code_lines(text)
        .filter_map(|line| {
            let name = without_visibility(line.trim())
                .strip_prefix("mod ")?
                .strip_suffix(';')?
                .trim();
            Some(name.to_owned())
        })
        .collect()
}

/// Files no module declares: rustc never compiles them, and nothing reports it.
///
/// `files` are the `.rs` files of module trees with their content. Crate roots (`lib.rs`,
/// `main.rs`) need no declaration; any other file `dir/name.rs` or `dir/name/mod.rs` must be
/// declared by `mod name;` in `dir/lib.rs`, `dir/main.rs`, `dir/mod.rs` or `dir.rs`.
#[must_use]
pub fn unwired_files(files: &[(PathBuf, String)]) -> Vec<PathBuf> {
    files
        .iter()
        .filter(|(path, _)| !is_wired(path, files))
        .map(|(path, _)| path.clone())
        .collect()
}

fn is_wired(path: &Path, files: &[(PathBuf, String)]) -> bool {
    let file_name = path.file_name().and_then(|name| name.to_str());
    if matches!(file_name, Some("lib.rs" | "main.rs")) {
        return true;
    }
    let module_file = if file_name == Some("mod.rs") {
        path.parent()
    } else {
        Some(path)
    };
    let Some((dir, name)) =
        module_file.and_then(|file| Some((file.parent()?, file.file_stem()?.to_str()?)))
    else {
        return false;
    };
    let parents = [
        dir.join("lib.rs"),
        dir.join("main.rs"),
        dir.join("mod.rs"),
        dir.with_extension("rs"),
    ];
    files.iter().any(|(candidate, text)| {
        parents.contains(candidate) && declared_modules(text).iter().any(|module| module == name)
    })
}

/// Every `.rs` file in each crate's module trees, `crates/*/src/` and `crates/*/tests/it/`, with
/// its content.
///
/// # Errors
///
/// Returns an I/O error if a directory or file can't be read.
pub fn module_files(crates_dir: &Path) -> io::Result<Vec<(PathBuf, String)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(crates_dir)? {
        let package = entry?.path();
        for tree in [package.join("src"), package.join("tests").join("it")] {
            if tree.is_dir() {
                collect_rust_files(&tree, &mut files)?;
            }
        }
    }
    Ok(files)
}

/// Test binaries other than `tests/it/`: a `tests/*.rs` file or a `tests/*/main.rs` directory,
/// each of which cargo would build as one more binary (one binary per crate, TESTING: layout).
///
/// # Errors
///
/// Returns an I/O error if a directory can't be read.
pub fn extra_test_binaries(crates_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut extra = Vec::new();
    for entry in std::fs::read_dir(crates_dir)? {
        let tests = entry?.path().join("tests");
        if !tests.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&tests)? {
            let path = entry?.path();
            let is_file_binary = path.extension().is_some_and(|extension| extension == "rs");
            let is_dir_binary =
                path.join("main.rs").is_file() && path.file_name().is_some_and(|name| name != "it");
            if is_file_binary || is_dir_binary {
                extra.push(path);
            }
        }
    }
    Ok(extra)
}

/// Every `.rs` file under `crates/*/src/`, with its content.
///
/// # Errors
///
/// Returns an I/O error if a directory or file can't be read.
pub fn crate_sources(crates_dir: &Path) -> io::Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(crates_dir)? {
        let src = entry?.path().join("src");
        if src.is_dir() {
            collect_rust_files(&src, &mut sources)?;
        }
    }
    Ok(sources)
}

fn collect_rust_files(dir: &Path, sources: &mut Vec<(PathBuf, String)>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_files(&path, sources)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = std::fs::read_to_string(&path)?;
            sources.push((path, text));
        }
    }
    Ok(())
}
