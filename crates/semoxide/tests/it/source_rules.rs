//! Rust rules checked on the tokens of every crate source (CLAUDE.md Rust rules 4 and 6,
//! TESTING: layout).

use std::path::Path;

use semoxide_test_support::source_rules::{
    crate_sources, extra_test_binaries, forbids_unsafe_code, module_files,
    out_of_line_test_modules, pub_tuple_fields, unwired_files,
};

#[test]
fn crate_sources_follow_the_rules() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let sources = crate_sources(&crates).expect("crates/ is readable");
    let mut problems = Vec::new();

    for (path, text) in &sources {
        let shown = path.strip_prefix(&crates).unwrap_or(path).display();
        let is_root = path.ends_with("src/lib.rs") || path.ends_with("src/main.rs");
        let is_git_crate = path
            .components()
            .any(|part| part.as_os_str() == "semoxide-git");
        if is_root && !is_git_crate && !forbids_unsafe_code(text) {
            problems.push(format!("{shown}: missing #![forbid(unsafe_code)]"));
        }
        for name in out_of_line_test_modules(text) {
            problems.push(format!(
                "{shown}: test module `{name}` is out of line; put unit tests inline in `mod tests {{ … }}`"
            ));
        }
        for name in pub_tuple_fields(text) {
            problems.push(format!("{shown}: `{name}` has a pub tuple field"));
        }
    }

    assert!(!sources.is_empty(), "no crate sources found");
    assert!(
        problems.is_empty(),
        "source rules:\n{}",
        problems.join("\n")
    );
}

#[test]
fn every_module_file_is_wired_and_each_crate_has_one_test_binary() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let files = module_files(&crates).expect("crates/ is readable");
    let shown = |path: &Path| {
        path.strip_prefix(&crates)
            .unwrap_or(path)
            .display()
            .to_string()
    };

    let mut problems: Vec<String> = unwired_files(&files)
        .iter()
        .map(|path| {
            format!(
                "{}: no `mod` declares it, so it is never compiled",
                shown(path)
            )
        })
        .collect();
    problems.extend(
        extra_test_binaries(&crates)
            .expect("crates/*/tests is readable")
            .iter()
            .map(|path| {
                format!(
                    "{}: integration tests go in tests/it/ (one binary)",
                    shown(path)
                )
            }),
    );

    assert!(!files.is_empty(), "no module files found");
    assert!(problems.is_empty(), "test layout:\n{}", problems.join("\n"));
}
