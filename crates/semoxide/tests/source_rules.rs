//! Rust rules checked on the tokens of every crate source (CLAUDE.md Rust rules 4 and 6,
//! CODE-ARCHITECTURE §8).

use std::path::Path;

use semoxide_test_support::source_rules::{
    crate_sources, forbids_unsafe_code, inline_test_modules, pub_tuple_fields,
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
        for name in inline_test_modules(text) {
            problems.push(format!(
                "{shown}: inline test module `{name}`; use `mod {name};` and a sibling file"
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
