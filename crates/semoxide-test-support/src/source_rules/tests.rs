use rstest::rstest;

use super::{forbids_unsafe_code, inline_test_modules, pub_tuple_fields};

#[rstest]
#[case::alone("//! Docs.\n\n#![forbid(unsafe_code)]\n")]
#[case::combined("#![forbid(missing_docs, unsafe_code)]\n")]
#[case::spaced("#! [ forbid ( unsafe_code ) ]\n")]
fn crate_root_forbids_unsafe_code(#[case] source: &str) {
    assert!(forbids_unsafe_code(source));
}

#[rstest]
#[case::missing("//! Docs.\n")]
#[case::in_line_comment("// #![forbid(unsafe_code)]\n")]
#[case::in_block_comment("/*\n#![forbid(unsafe_code)]\n*/\n")]
#[case::in_string("const S: &str = \"#![forbid(unsafe_code)]\";\n")]
#[case::other_lint("#![forbid(missing_docs)]\n")]
#[case::deny_not_forbid("#![deny(unsafe_code)]\n")]
#[case::outer_attribute("#[forbid(unsafe_code)]\nfn f() {}\n")]
fn crate_root_does_not_forbid_unsafe_code(#[case] source: &str) {
    assert!(!forbids_unsafe_code(source));
}

#[rstest]
#[case::inline("#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n", &["tests"])]
#[case::with_other_attribute("#[cfg(test)]\n#[allow(dead_code)]\nmod checks { }\n", &["checks"])]
#[case::public("#[cfg(test)]\npub(crate) mod helpers { }\n", &["helpers"])]
#[case::nested("mod outer {\n    #[cfg(test)]\n    mod inner { }\n}\n", &["inner"])]
#[case::sibling_file("#[cfg(test)]\nmod tests;\n", &[])]
#[case::not_test_cfg("#[cfg(unix)]\nmod unix { }\n", &[])]
#[case::in_comment("// #[cfg(test)] mod tests { }\n", &[])]
#[case::in_string("const S: &str = \"#[cfg(test)] mod tests { }\";\n", &[])]
fn inline_test_modules_are_found(#[case] source: &str, #[case] expected: &[&str]) {
    assert_eq!(inline_test_modules(source), expected);
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
