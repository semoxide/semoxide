use toml::Table;

use super::parse_flag;

#[test]
fn equals_inside_a_quoted_key_part_is_part_of_the_key() {
    let layer = parse_flag(1, r#"plugins."a=b".x=1"#);

    assert!(layer.is_ok(), "{layer:?}");
    let expected: Table = r#"plugins."a=b".x = 1"#.parse().unwrap();
    assert_eq!(layer.unwrap().table(), &expected);
}
