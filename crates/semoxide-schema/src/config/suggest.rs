//! "Did you mean" hints for unknown keys: the closest valid key by edit distance, as rustc and
//! cargo suggest names.

/// The edit distance between `a` and `b`, ignoring case, where swapping two neighbouring
/// characters counts as one edit; `None` when it exceeds `limit`.
pub(super) fn edit_distance(a: &str, b: &str, limit: usize) -> Option<usize> {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let (mut a, mut b) = (a.as_slice(), b.as_slice());
    // A shared prefix and suffix never change the distance.
    while let ([first_a, rest_a @ ..], [first_b, rest_b @ ..]) = (a, b)
        && first_a == first_b
    {
        (a, b) = (rest_a, rest_b);
    }
    while let ([rest_a @ .., last_a], [rest_b @ .., last_b]) = (a, b)
        && last_a == last_b
    {
        (a, b) = (rest_a, rest_b);
    }
    if a.len().abs_diff(b.len()) > limit {
        return None;
    }
    let distance = optimal_string_alignment(a, b);
    (distance <= limit).then_some(distance)
}

/// The edit distance with insertions, deletions, substitutions and swaps of two neighbouring
/// characters, each counting one (optimal string alignment).
fn optimal_string_alignment(a: &[char], b: &[char]) -> usize {
    // Row `i` holds the distances from `a[..i]` to every prefix of `b`; two rows back are kept for
    // swaps.
    let mut before_previous: Vec<usize> = vec![0; b.len() + 1];
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, &char_a) in a.iter().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, &char_b) in b.iter().enumerate() {
            let substitution = previous
                .get(j)
                .copied()
                .unwrap_or(usize::MAX)
                .saturating_add(usize::from(char_a != char_b));
            let deletion = previous
                .get(j + 1)
                .copied()
                .unwrap_or(usize::MAX)
                .saturating_add(1);
            let insertion = current
                .get(j)
                .copied()
                .unwrap_or(usize::MAX)
                .saturating_add(1);
            let mut best = substitution.min(deletion).min(insertion);
            let swapped =
                i > 0 && j > 0 && a.get(i - 1) == Some(&char_b) && b.get(j - 1) == Some(&char_a);
            if swapped && let Some(&two_back) = before_previous.get(j - 1) {
                best = best.min(two_back + 1);
            }
            if let Some(cell) = current.get_mut(j + 1) {
                *cell = best;
            }
        }
        before_previous = std::mem::replace(&mut previous, current);
    }
    previous.last().copied().unwrap_or(0)
}

/// The candidate closest to `typo`, within `max(len, 3) / 3` edits (rustc's limit); on a tie, the
/// first one.
pub(super) fn closest<'a>(typo: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let limit = typo.chars().count().max(3) / 3;
    candidates
        .iter()
        .filter_map(|candidate| Some((edit_distance(typo, candidate, limit)?, *candidate)))
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

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
    fn closest_cases(
        #[case] typo: &str,
        #[case] candidates: &[&str],
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(closest(typo, candidates), expected, "{typo}");
    }

    #[test]
    fn deletions_inside_the_word_count() {
        assert_eq!(edit_distance("sitting", "kitten", 10), Some(3));
    }
}
