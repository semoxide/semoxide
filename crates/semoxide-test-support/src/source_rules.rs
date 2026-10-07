//! Source rules checked on Rust tokens (`proc-macro2`), so comments and string contents never
//! count: `#![forbid(unsafe_code)]` in crate roots (CLAUDE.md Rust rule 6), no inline test
//! modules (CODE-ARCHITECTURE §8), private tuple fields (Rust rule 4).

use std::io;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Whether the file has a `#![forbid(…)]` inner attribute that lists `unsafe_code`.
#[must_use]
pub fn forbids_unsafe_code(_text: &str) -> bool {
    // Stub (test commit): wrong but valid value.
    true
}

/// Names of modules declared `#[cfg(test)] mod name { … }` with a body instead of `mod name;`.
#[must_use]
pub fn inline_test_modules(_text: &str) -> Vec<String> {
    // Stub (test commit): wrong but valid value.
    Vec::new()
}

/// Names of tuple structs with a `pub` field, e.g. `struct Tag(pub String)`.
#[must_use]
pub fn pub_tuple_fields(_text: &str) -> Vec<String> {
    // Stub (test commit): wrong but valid value.
    Vec::new()
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
