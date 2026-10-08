use super::edit_distance;

#[test]
fn deletions_inside_the_word_count() {
    assert_eq!(edit_distance("sitting", "kitten", 10), Some(3));
}
