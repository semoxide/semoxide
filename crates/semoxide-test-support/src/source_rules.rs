//! Source rules for the crates' files, as plain-text scans like the Rust compiler's `tidy`
//! checks: `#![forbid(unsafe_code)]` in crate roots (CLAUDE.md Rust rule 6), no out-of-line test
//! modules (TESTING: layout), private tuple fields (Rust rule 4), every module file declared and
//! one integration-test binary per crate (TESTING: layout).
//!
//! Lines starting with `//` are skipped. Known limits: text inside `/* … */` blocks or strings
//! that starts a line can still match, and a tuple struct spread over several lines isn't seen.

use std::io;
use std::path::{Path, PathBuf};

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

/// Names of test modules declared out of line, `#[cfg(test)] mod name;`, instead of with a body
/// `mod name { … }`. `cfg(all(test, …))` counts as a test cfg; a file containing `#![cfg(test)]`
/// is test-only and skipped.
#[must_use]
pub fn out_of_line_test_modules(text: &str) -> Vec<String> {
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
            body_less_module_name(declaration).map(str::to_owned)
        })
        .collect()
}

/// `mod name;` (any visibility) gives `name`; `mod name { … }` and anything else give nothing.
fn body_less_module_name(line: &str) -> Option<&str> {
    let name = without_visibility(line)
        .strip_prefix("mod ")?
        .strip_suffix(';')?
        .trim();
    Some(name)
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
        .filter_map(|line| body_less_module_name(line.trim()).map(str::to_owned))
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::alone("//! Docs.\n\n#![forbid(unsafe_code)]\n")]
    #[case::combined("#![forbid(missing_docs, unsafe_code)]\n")]
    fn crate_root_forbids_unsafe_code(#[case] source: &str) {
        assert!(forbids_unsafe_code(source));
    }

    #[rstest]
    #[case::missing("//! Docs.\n")]
    #[case::in_line_comment("// #![forbid(unsafe_code)]\n")]
    #[case::in_string("const S: &str = \"#![forbid(unsafe_code)]\";\n")]
    #[case::other_lint("#![forbid(missing_docs)]\n")]
    #[case::deny_not_forbid("#![deny(unsafe_code)]\n")]
    #[case::outer_attribute("#[forbid(unsafe_code)]\nfn f() {}\n")]
    #[case::nested_module("mod x {\n    #![forbid(unsafe_code)]\n}\n")]
    fn crate_root_does_not_forbid_unsafe_code(#[case] source: &str) {
        assert!(!forbids_unsafe_code(source));
    }

    #[rstest]
    #[case::inline("#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n", &[])]
    #[case::with_other_attribute("#[cfg(test)]\n#[allow(dead_code)]\nmod checks;\n", &["checks"])]
    #[case::public("#[cfg(test)]\npub(crate) mod helpers;\n", &["helpers"])]
    #[case::nested("mod outer {\n    #[cfg(test)]\n    mod inner;\n}\n", &["inner"])]
    #[case::after_other_items("use std::fmt;\n\nfn f() {}\n\n#[cfg(test)]\nmod tests;\n", &["tests"])]
    #[case::second_attribute_after_items("fn f() {}\n#[cfg(test)]\n#[allow(dead_code)]\nmod checks;\n", &["checks"])]
    #[case::all_test_and_unix("#[cfg(all(test, unix))]\nmod tests;\n", &["tests"])]
    #[case::not_test("#[cfg(not(test))]\nmod real;\n", &[])]
    #[case::any_test_or_unix("#[cfg(any(test, unix))]\nmod either;\n", &[])]
    #[case::sibling_file("#[cfg(test)]\nmod tests;\n", &["tests"])]
    #[case::not_test_cfg("#[cfg(unix)]\nmod unix;\n", &[])]
    #[case::inner_cfg_test_file("#![cfg(test)]\n#[cfg(test)]\nmod helpers;\n", &[])]
    #[case::in_comment("// #[cfg(test)] mod tests;\n", &[])]
    #[case::in_string("const S: &str = \"#[cfg(test)] mod tests;\";\n", &[])]
    fn out_of_line_test_modules_are_found(#[case] source: &str, #[case] expected: &[&str]) {
        assert_eq!(out_of_line_test_modules(source), expected);
    }

    #[rstest]
    #[case::pub_field("pub struct Tag(pub String);\n", &["Tag"])]
    #[case::restricted_pub("struct Id(pub(crate) u64);\n", &["Id"])]
    #[case::second_field("struct Pair(u8, pub u8);\n", &["Pair"])]
    #[case::after_attribute("struct Wrapped(#[serde(skip)] pub u8);\n", &["Wrapped"])]
    #[case::generic("struct Wrapper<T: Clone>(pub T);\n", &["Wrapper"])]
    #[case::private("pub struct Tag(String);\n", &[])]
    #[case::named_fields("pub struct Named { pub name: String }\n", &[])]
    #[case::unit_struct_before_tuple("struct Unit;\nstruct Tag(pub u8);\n", &["Tag"])]
    #[case::in_comment("// struct Tag(pub String);\n", &[])]
    #[case::in_string("const S: &str = \"struct Tag(pub String);\";\n", &[])]
    fn pub_tuple_fields_are_found(#[case] source: &str, #[case] expected: &[&str]) {
        assert_eq!(pub_tuple_fields(source), expected);
    }

    #[test]
    fn crate_sources_are_the_rust_files_under_each_crates_src() {
        let root = tempfile::tempdir().unwrap();
        let write = |relative: &str| {
            let path = root.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, relative).unwrap();
        };
        write("semoxide-a/src/lib.rs");
        write("semoxide-a/src/push/mod.rs");
        write("semoxide-a/tests/push.rs");
        write("semoxide-a/src/notes.md");
        write("semoxide-b/build.rs");

        let mut found: Vec<String> = crate_sources(root.path())
            .unwrap()
            .into_iter()
            .map(|(path, text)| {
                assert!(path.ends_with(&text), "content belongs to its file");
                text
            })
            .collect();
        found.sort();

        assert_eq!(
            found,
            ["semoxide-a/src/lib.rs", "semoxide-a/src/push/mod.rs"]
        );
    }

    #[rstest]
    #[case::plain("mod tests;\n", &["tests"])]
    #[case::visibility("pub mod codes;\npub(crate) mod render;\n", &["codes", "render"])]
    #[case::attribute_above("#[cfg(test)]\nmod tests;\n", &["tests"])]
    #[case::inline_body_is_not_a_declaration("mod tests {\n}\n", &[])]
    #[case::commented_out("// mod tests;\n", &[])]
    fn declared_modules_are_found(#[case] source: &str, #[case] expected: &[&str]) {
        assert_eq!(declared_modules(source), expected);
    }

    fn files(entries: &[(&str, &str)]) -> Vec<(PathBuf, String)> {
        entries
            .iter()
            .map(|(path, text)| (PathBuf::from(path), (*text).to_owned()))
            .collect()
    }

    #[test]
    fn declared_files_are_wired() {
        let tree = files(&[
            ("c/src/lib.rs", "pub mod code;\nmod util;\n"),
            ("c/src/code.rs", "#[cfg(test)]\nmod tests;\n"),
            ("c/src/code/tests.rs", ""),
            ("c/src/util/mod.rs", "mod inner;\n"),
            ("c/src/util/inner.rs", ""),
            ("c/tests/it/main.rs", "mod cli;\n"),
            ("c/tests/it/cli.rs", ""),
        ]);

        assert_eq!(unwired_files(&tree), Vec::<PathBuf>::new());
    }

    #[test]
    fn undeclared_files_are_unwired() {
        let tree = files(&[
            ("c/src/lib.rs", "pub mod code;\n"),
            ("c/src/code.rs", "#[cfg(test)]\nmod tests;\n"),
            ("c/src/code/tests.rs", ""),
            ("c/src/code/message_tests.rs", ""),
            ("c/src/orphan.rs", ""),
            ("c/tests/it/main.rs", "mod cli;\n"),
            ("c/tests/it/forgotten.rs", ""),
        ]);

        assert_eq!(
            unwired_files(&tree),
            [
                PathBuf::from("c/src/code/message_tests.rs"),
                PathBuf::from("c/src/orphan.rs"),
                PathBuf::from("c/tests/it/forgotten.rs"),
            ]
        );
    }

    #[test]
    fn a_declaration_in_another_directory_does_not_count() {
        let tree = files(&[
            ("c/src/lib.rs", "mod tests;\n"),
            ("c/src/code/tests.rs", ""),
        ]);

        assert_eq!(unwired_files(&tree), [PathBuf::from("c/src/code/tests.rs")]);
    }

    #[test]
    fn module_files_cover_src_and_tests_it_only() {
        let crates = tempfile::tempdir().unwrap();
        let krate = crates.path().join("c");
        for dir in ["src", "tests/it", "tests/data"] {
            fs::create_dir_all(krate.join(dir)).unwrap();
        }
        for file in [
            "src/lib.rs",
            "tests/it/main.rs",
            "tests/data/sample.rs",
            "src/notes.md",
        ] {
            fs::write(krate.join(file), "").unwrap();
        }

        let mut found: Vec<_> = module_files(crates.path())
            .unwrap()
            .into_iter()
            .map(|(path, _)| path.strip_prefix(&krate).unwrap().to_path_buf())
            .collect();
        found.sort();
        assert_eq!(
            found,
            [
                PathBuf::from("src/lib.rs"),
                PathBuf::from("tests/it/main.rs")
            ]
        );
    }

    #[test]
    fn extra_test_binaries_are_found() {
        let crates = tempfile::tempdir().unwrap();
        let tests = crates.path().join("c/tests");
        for dir in ["it", "other", "data"] {
            fs::create_dir_all(tests.join(dir)).unwrap();
        }
        for file in ["it/main.rs", "other/main.rs", "stray.rs", "data/input.toml"] {
            fs::write(tests.join(file), "").unwrap();
        }

        let mut found = extra_test_binaries(crates.path()).unwrap();
        found.sort();
        assert_eq!(found, [tests.join("other"), tests.join("stray.rs")]);
    }
}
