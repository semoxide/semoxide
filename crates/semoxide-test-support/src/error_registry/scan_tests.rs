use super::{codes_in_source, try_codes_in_source};

#[test]
fn nested_block_comments_are_skipped() {
    let source = r#"
/* outer /* inner */ still a comment: ErrorCode::from_static("git::hidden") */
const GIT_X: ErrorCode = ErrorCode::from_static("git::x");
"#;

    assert_eq!(codes_in_source(source), ["git::x"]);
}

#[test]
fn escaped_quote_in_a_string_does_not_end_it() {
    let source = r#"
const NOTE: &str = "say \"hi\" // not a comment";
const GIT_X: ErrorCode = ErrorCode::from_static("git::x");
"#;

    assert_eq!(codes_in_source(source), ["git::x"]);
}

#[test]
fn raw_string_with_quotes_and_slashes_is_not_code() {
    let source = r##"
const TEXT: &str = r#"a "quoted" // part: ErrorCode::from_static("git::in_text")"#;
const GIT_X: ErrorCode = ErrorCode::from_static("git::x");
"##;

    assert_eq!(codes_in_source(source), ["git::x"]);
}

#[test]
fn char_literal_quote_does_not_start_a_string() {
    let source = r#"
const QUOTE: char = '"';
const GIT_X: ErrorCode = ErrorCode::from_static("git::x");
"#;

    assert_eq!(codes_in_source(source), ["git::x"]);
}

#[test]
fn code_inside_a_block_comment_spanning_lines_is_skipped() {
    let source = r#"
const GIT_A: ErrorCode = ErrorCode::from_static("git::a"); /* starts here
ErrorCode::from_static("git::hidden")
ends here */ const GIT_B: ErrorCode = ErrorCode::from_static("git::b");
"#;

    assert_eq!(codes_in_source(source), ["git::a", "git::b"]);
}

#[test]
fn unreadable_source_is_an_error() {
    let source = "const GIT_X: ErrorCode = ErrorCode::from_static(\"git::x\"));";

    assert!(try_codes_in_source(source).is_err());
}

#[test]
fn readable_source_gives_the_same_codes_as_the_lenient_scan() {
    let source = "const GIT_X: ErrorCode = ErrorCode::from_static(\"git::x\");";

    assert_eq!(try_codes_in_source(source).unwrap(), ["git::x"]);
}

#[test]
fn code_written_as_a_raw_string_is_not_found() {
    let source = r#"const GIT_X: ErrorCode = ErrorCode::from_static(r"git::x");"#;

    assert_eq!(codes_in_source(source), Vec::<String>::new());
}
