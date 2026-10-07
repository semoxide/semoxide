//! Every error code semoxide can report has a docs page, and every page documents a code.

use std::path::Path;

use semoxide_test_support::error_registry::{page_slugs, registry_problems, source_codes};

#[test]
fn every_error_code_has_a_docs_page() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pages = page_slugs(&repo.join("docs/errors")).expect("docs/errors is readable");
    let sources = source_codes(&repo.join("crates")).expect("crates/ is readable");

    let problems = registry_problems(&semoxide::error_codes(), &pages, &sources);

    let report: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        report.is_empty(),
        "error-code registry:\n{}",
        report.join("\n")
    );
}
