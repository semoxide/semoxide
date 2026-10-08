use rstest::rstest;

use super::{closest, edit_distance};

#[rstest]
#[case::identical("format", "format", Some(0))]
#[case::swap("formta", "format", Some(1))]
#[case::swap_reversed("format", "formta", Some(1))]
#[case::insertion("ormat", "format", Some(1))]
#[case::deletion("formaat", "format", Some(1))]
#[case::substitution("formxt", "format", Some(1))]
#[case::case_only("Format", "format", Some(0))]
#[case::empty_typo("", "abc", Some(3))]
#[case::empty_candidate("abc", "", Some(3))]
#[case::classic("kitten", "sitting", Some(3))]
#[case::over_the_limit("kitten", "sitting", None)]
fn edit_distance_cases(#[case] a: &str, #[case] b: &str, #[case] expected: Option<usize>) {
    let limit = if expected.is_none() { 2 } else { 10 };

    assert_eq!(edit_distance(a, b, limit), expected, "{a} -> {b}");
}

#[rstest]
#[case::swap("formta", &["format", "metadata"], Some("format"))]
#[case::short_key_one_edit("fxx", &["fox"], Some("fox"))]
#[case::short_key_two_edits("abx", &["fox"], None)]
#[case::five_chars_one_edit("prefx", &["prefix"], Some("prefix"))]
#[case::six_chars_two_edits("forest", &["format"], Some("format"))]
#[case::six_chars_three_edits("forxyz", &["format"], None)]
#[case::short_never_suggests_long("fix", &["feature"], None)]
#[case::nothing_close("colour", &["config", "commits", "tags"], None)]
#[case::closest_wins("prest", &["present", "preset"], Some("preset"))]
#[case::tie_takes_the_first("bat", &["cat", "hat"], Some("cat"))]
#[case::no_candidates("tags", &[], None)]
fn closest_cases(#[case] typo: &str, #[case] candidates: &[&str], #[case] expected: Option<&str>) {
    assert_eq!(closest(typo, candidates), expected, "{typo}");
}
