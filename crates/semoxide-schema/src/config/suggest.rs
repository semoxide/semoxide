//! "Did you mean" hints for unknown keys: the closest valid key by edit distance, as rustc and
//! cargo suggest names.

#[cfg(test)]
mod tests;

/// The edit distance between `a` and `b`, ignoring case, where swapping two neighbouring
/// characters counts as one edit; `None` when it exceeds `limit`.
pub(super) fn edit_distance(a: &str, b: &str, limit: usize) -> Option<usize> {
    let _ = (a, b, limit);
    Some(usize::MAX)
}

/// The candidate closest to `typo`, within `max(len, 3) / 3` edits (rustc's limit); on a tie, the
/// first one.
pub(super) fn closest<'a>(typo: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let _ = (typo, candidates);
    Some("")
}
