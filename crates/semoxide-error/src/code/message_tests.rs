use super::{ErrorCode, InvalidErrorCode};

fn message(input: &str) -> String {
    let error: InvalidErrorCode = input.parse::<ErrorCode>().unwrap_err();
    error.to_string()
}

#[test]
fn message_names_the_bad_input_and_the_expected_shape() {
    assert_eq!(
        message("Core::bad"),
        "invalid error code `Core::bad`: expected `<namespace>::<name>`"
    );
}

#[test]
fn message_follows_the_style_guide() {
    let text = message("core:x");

    assert!(text.starts_with(char::is_lowercase), "{text}");
    assert!(!text.ends_with(['.', '!', '?']), "{text}");
    assert!(!text.contains('\n'), "{text}");
}
