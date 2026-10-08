use std::fs;
use std::path::PathBuf;

use rstest::rstest;

use super::{declared_modules, extra_test_binaries, module_files, unwired_files};

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
