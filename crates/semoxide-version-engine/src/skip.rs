//! [`Skipped`]: the commits of a range without those carrying a skip marker, and why a range
//! releases nothing.

use semoxide_schema::config::Level;

use crate::CommitId;

/// A commit as the skip filter reads it.
pub trait CommitMessage {
    /// The commit's ID.
    fn id(&self) -> &CommitId;
    /// The full message, subject and body.
    fn message(&self) -> &str;
}

/// The commits of a range split by the skip marker `[skip release]` or `[release skip]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped<C> {
    kept: Vec<C>,
    skipped: Vec<CommitId>,
}

/// Why a range releases nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NoRelease {
    /// The range has no commits.
    NoCommitsSince,
    /// Every commit carries the skip marker.
    #[non_exhaustive]
    AllCommitsSkipped {
        /// The skipped commits.
        skipped: Vec<CommitId>,
    },
    /// No kept commit bumps the version.
    #[non_exhaustive]
    NoRelevantCommits {
        /// The skipped commits.
        skipped: Vec<CommitId>,
    },
}

impl<C: CommitMessage> Skipped<C> {
    /// Splits `commits`, keeping their order. Each commit is judged by its own message.
    #[must_use]
    pub fn filter(commits: Vec<C>) -> Self {
        Self {
            kept: commits,
            skipped: Vec::new(),
        }
    }
}

impl<C> Skipped<C> {
    /// The commits without a skip marker.
    #[must_use]
    pub fn kept(&self) -> &[C] {
        &self.kept
    }

    /// The IDs of the commits with a skip marker.
    #[must_use]
    pub fn skipped(&self) -> &[CommitId] {
        &self.skipped
    }

    /// The commits for the analyzer.
    ///
    /// # Errors
    ///
    /// Returns [`NoRelease`] when the range has no commits, or every commit is skipped.
    pub fn to_analyze(&self) -> Result<&[C], NoRelease> {
        Ok(&self.kept)
    }

    /// The level to release at, from the analyzer's verdict on [`Self::to_analyze`].
    ///
    /// # Errors
    ///
    /// Returns [`NoRelease::NoRelevantCommits`] when the analyzer found no bump.
    pub fn level(&self, analyzed: Option<Level>) -> Result<Level, NoRelease> {
        Ok(analyzed.unwrap_or(Level::Patch))
    }
}

impl NoRelease {
    /// The skipped commits; empty when the range has none.
    #[must_use]
    pub fn skipped(&self) -> &[CommitId] {
        &[]
    }
}

/// Whether `message` holds `[skip release]` or `[release skip]`: anywhere, in any ASCII case,
/// with any whitespace between the words.
fn has_marker(_message: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// A commit written as one hex digit, with its message.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Stub {
        id: CommitId,
        message: &'static str,
    }

    impl CommitMessage for Stub {
        fn id(&self) -> &CommitId {
            &self.id
        }

        fn message(&self) -> &str {
            self.message
        }
    }

    fn sha(commit: char) -> CommitId {
        CommitId::try_from(commit.to_string().repeat(40).as_str()).unwrap()
    }

    fn skipped(commits: &[(char, &'static str)]) -> Skipped<Stub> {
        Skipped::filter(
            commits
                .iter()
                .map(|&(commit, message)| Stub {
                    id: sha(commit),
                    message,
                })
                .collect(),
        )
    }

    fn ids(stubs: &[Stub]) -> Vec<CommitId> {
        stubs.iter().map(|stub| stub.id.clone()).collect()
    }

    #[rstest]
    #[case::subject("fix: x [skip release]")]
    #[case::reversed("fix: x [release skip]")]
    #[case::uppercase("fix: x [SKIP RELEASE]")]
    #[case::mixed_case("fix: x [Release Skip]")]
    #[case::in_the_body("fix: x\n\nbody\n[skip release]\n\nRefs: #1")]
    #[case::glued_to_text("fix: x[skip release]y")]
    #[case::spaces("fix: x [skip   release]")]
    #[case::tab("fix: x [skip\trelease]")]
    #[case::line_break("fix: x [skip\nrelease]")]
    #[case::crlf("fix: x [release\r\nskip]")]
    #[case::no_break_space("fix: x [skip\u{a0}release]")]
    #[case::alone("[skip release]")]
    #[case::after_another_bracket("fix: x [ci] [skip release]")]
    #[case::before_another_bracket("fix: x [skip release] [ci]")]
    fn a_marker_is_found(#[case] message: &str) {
        assert!(has_marker(message), "{message:?}");
    }

    #[rstest]
    #[case::other_marker("fix: x [skip ci]")]
    #[case::no_whitespace("fix: x [skiprelease]")]
    #[case::hyphen("fix: x [skip-release]")]
    #[case::no_brackets("fix: x skip release")]
    #[case::unclosed("fix: x [skip release")]
    #[case::space_after_opening_bracket("fix: x [ skip release]")]
    #[case::space_before_closing_bracket("fix: x [skip release ]")]
    #[case::longer_word("fix: x [release skips]")]
    #[case::non_ascii_letter("fix: x [\u{17f}kip release]")]
    #[case::missing_opening_bracket("skip release] x")]
    #[case::empty("")]
    fn anything_else_is_not_a_marker(#[case] message: &str) {
        assert!(!has_marker(message), "{message:?}");
    }

    #[test]
    fn commits_with_a_marker_are_split_off_in_order() {
        let split = skipped(&[
            ('a', "feat: a"),
            ('b', "fix: b [skip release]"),
            ('c', "fix: c"),
            ('d', "chore: d\n\n[release skip]"),
        ]);

        assert_eq!(
            (ids(split.kept()), split.skipped().to_vec()),
            (vec![sha('a'), sha('c')], vec![sha('b'), sha('d')])
        );
    }

    #[test]
    fn a_range_without_commits_has_nothing_to_analyze() {
        assert_eq!(
            skipped(&[]).to_analyze().map(ids),
            Err(NoRelease::NoCommitsSince)
        );
    }

    #[test]
    fn a_range_of_skipped_commits_has_nothing_to_analyze() {
        assert_eq!(
            skipped(&[
                ('a', "fix: a [skip release]"),
                ('b', "feat: b [release skip]")
            ])
            .to_analyze()
            .map(ids),
            Err(NoRelease::AllCommitsSkipped {
                skipped: vec![sha('a'), sha('b')]
            })
        );
    }

    #[test]
    fn the_kept_commits_are_analyzed() {
        assert_eq!(
            skipped(&[
                ('a', "feat: a"),
                ('b', "fix: b [skip release]"),
                ('c', "docs: c")
            ])
            .to_analyze()
            .map(ids),
            Ok(vec![sha('a'), sha('c')])
        );
    }

    #[test]
    fn nothing_skipped_is_all_analyzed() {
        assert_eq!(
            skipped(&[('a', "feat: a")]).to_analyze().map(ids),
            Ok(vec![sha('a')])
        );
    }

    #[test]
    fn the_analyzed_level_is_released() {
        assert_eq!(
            skipped(&[('a', "feat: a")]).level(Some(Level::Minor)),
            Ok(Level::Minor)
        );
    }

    #[rstest]
    #[case::none_skipped(&[('a', "docs: a")], vec![])]
    #[case::some_skipped(&[('a', "docs: a"), ('b', "fix: b [skip release]")], vec![sha('b')])]
    fn no_level_releases_nothing(
        #[case] commits: &[(char, &'static str)],
        #[case] expected: Vec<CommitId>,
    ) {
        assert_eq!(
            skipped(commits).level(None),
            Err(NoRelease::NoRelevantCommits { skipped: expected })
        );
    }

    #[rstest]
    #[case::no_commits(NoRelease::NoCommitsSince, vec![])]
    #[case::all_skipped(NoRelease::AllCommitsSkipped { skipped: vec![sha('a'), sha('b')] }, vec![sha('a'), sha('b')])]
    #[case::not_relevant(NoRelease::NoRelevantCommits { skipped: vec![sha('c')] }, vec![sha('c')])]
    fn a_reason_gives_its_skipped_commits(
        #[case] reason: NoRelease,
        #[case] expected: Vec<CommitId>,
    ) {
        assert_eq!(reason.skipped(), expected.as_slice());
    }
}
