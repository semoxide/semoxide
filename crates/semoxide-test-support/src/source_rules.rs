//! Source rules for the crates' `src/` files, as plain-text scans like the Rust compiler's
//! `tidy` checks: `#![forbid(unsafe_code)]` in crate roots (CLAUDE.md Rust rule 6), no inline
//! test modules (CODE-ARCHITECTURE §8), private tuple fields (Rust rule 4).
//!
//! Lines starting with `//` are skipped. Known limits: text inside `/* … */` blocks or strings
//! that starts a line can still match, and a tuple struct spread over several lines isn't seen.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

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
