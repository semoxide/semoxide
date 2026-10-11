//! A tag's channel notes (G9 read): semoxide's own ref and upstream's two, each read on its
//! own so notes from several refs never run together (upstream #4073). Parsing them and
//! choosing between them is the runtime's job.

use crate::{GitError, Repo, Tag};

/// The text of each notes ref for one tag, as stored; `None` where there is none.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TagNotes {
    semoxide: Option<String>,
    upstream: Option<String>,
    upstream_legacy: Option<String>,
}

impl TagNotes {
    /// `refs/notes/semoxide-<tag>`, on the tag's commit.
    #[must_use]
    pub fn semoxide(&self) -> Option<&str> {
        self.semoxide.as_deref()
    }

    /// `refs/notes/semantic-release-<tag>`: on an annotated tag's own object, where upstream
    /// writes it (upstream #1708), else on the commit.
    #[must_use]
    pub fn upstream(&self) -> Option<&str> {
        self.upstream.as_deref()
    }

    /// `refs/notes/semantic-release`, upstream's older shared ref, looked up like
    /// [`Self::upstream`].
    #[must_use]
    pub fn upstream_legacy(&self) -> Option<&str> {
        self.upstream_legacy.as_deref()
    }
}

impl Repo {
    /// The notes of `tag` in each ref. Text that isn't UTF-8 is decoded with replacement
    /// characters.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if libgit2 fails to read a notes ref.
    pub fn tag_notes(&self, _tag: &Tag) -> Result<TagNotes, GitError> {
        Ok(TagNotes::default())
    }
}

#[cfg(test)]
mod tests {
    use semoxide_error::ErrorInfo;
    use semoxide_test_support::git_fixture::GitFixture;

    use super::*;
    use crate::codes::GIT_READ_FAILED;

    const OURS: &str = r#"{"channels":[null]}"#;
    const UPSTREAM: &str = r#"{"channels":[null,"next"]}"#;
    const LEGACY: &str = r#"{"channels":["legacy"]}"#;

    /// Notes where upstream puts them: `git notes add <tag>` lands on the tag object of an
    /// annotated tag.
    ///
    /// - `v1.0.0`, lightweight: a note in each ref, all on one commit (upstream #4073).
    /// - `v1.1.0`, annotated: ours on the commit, upstream's two on the tag object.
    /// - `v1.2.0`, annotated: upstream's note on the commit only, as a fixed upstream would.
    /// - `v1.3.0`, annotated: upstream's note on both the tag object and the commit.
    /// - `v1.4.0`: no notes.
    fn fixture() -> GitFixture {
        GitFixture::builder()
            .commit("feat: first")
            .tag("v1.0.0")
            .note("v1.0.0", "semoxide-v1.0.0", OURS)
            .note("v1.0.0", "semantic-release-v1.0.0", UPSTREAM)
            .note("v1.0.0", "semantic-release", LEGACY)
            .commit("fix: second")
            .annotated_tag("v1.1.0", "release 1.1.0")
            .note("v1.1.0^{commit}", "semoxide-v1.1.0", OURS)
            .note("v1.1.0", "semantic-release-v1.1.0", UPSTREAM)
            .note("v1.1.0", "semantic-release", LEGACY)
            .commit("fix: third")
            .annotated_tag("v1.2.0", "release 1.2.0")
            .note("v1.2.0^{commit}", "semantic-release-v1.2.0", UPSTREAM)
            .commit("fix: fourth")
            .annotated_tag("v1.3.0", "release 1.3.0")
            .note("v1.3.0", "semantic-release-v1.3.0", "on the tag object")
            .note(
                "v1.3.0^{commit}",
                "semantic-release-v1.3.0",
                "on the commit",
            )
            .commit("fix: fifth")
            .tag("v1.4.0")
            .build()
            .unwrap()
    }

    /// The notes of `name` as `(semoxide, upstream, upstream_legacy)`.
    type Texts = (Option<String>, Option<String>, Option<String>);

    fn ours(fixture: &GitFixture, name: &str) -> Result<Texts, String> {
        let repo = Repo::discover(&fixture.path()).map_err(|error| error.to_string())?;
        let tag = repo
            .tag_commit(name)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("no tag {name}"))?;
        repo.tag_notes(&tag)
            .map(|notes| {
                (
                    notes.semoxide().map(str::to_owned),
                    notes.upstream().map(str::to_owned),
                    notes.upstream_legacy().map(str::to_owned),
                )
            })
            .map_err(|error| error.to_string())
    }

    /// `git notes --ref <notes_ref> show <object>`, as stored: a note added with `-m` ends with
    /// the one newline the fixture's output drops.
    fn note_by_git(fixture: &GitFixture, notes_ref: &str, object: &str) -> Option<String> {
        fixture
            .git(&["notes", "--ref", notes_ref, "show", object])
            .ok()
            .map(|text| format!("{text}\n"))
    }

    #[test]
    fn each_ref_is_read_where_upstream_writes_it() {
        let fixture = fixture();
        let by_git = |name: &str, upstream_on: &str| -> Texts {
            (
                note_by_git(
                    &fixture,
                    &format!("semoxide-{name}"),
                    &format!("{name}^{{commit}}"),
                ),
                note_by_git(&fixture, &format!("semantic-release-{name}"), upstream_on),
                note_by_git(&fixture, "semantic-release", upstream_on),
            )
        };

        assert_eq!(
            (ours(&fixture, "v1.0.0"), ours(&fixture, "v1.1.0")),
            (
                Ok(by_git("v1.0.0", "v1.0.0")),
                Ok(by_git("v1.1.0", "v1.1.0"))
            )
        );
    }

    /// Three refs annotating one commit stay three whole notes.
    #[test]
    fn notes_from_several_refs_on_one_commit_stay_apart() {
        let fixture = fixture();
        let note = |text: &str| Some(format!("{text}\n"));

        assert_eq!(
            ours(&fixture, "v1.0.0"),
            Ok((note(OURS), note(UPSTREAM), note(LEGACY)))
        );
    }

    #[test]
    fn upstreams_note_on_the_commit_is_found_and_the_tag_objects_wins() {
        let fixture = fixture();
        let upstream = |name: &str| ours(&fixture, name).map(|texts| texts.1);

        assert_eq!(
            (upstream("v1.2.0"), upstream("v1.3.0")),
            (
                Ok(Some(format!("{UPSTREAM}\n"))),
                Ok(Some(String::from("on the tag object\n")))
            )
        );
    }

    #[test]
    fn a_tag_without_notes_has_none_in_each_ref() {
        let fixture = fixture();
        let without = GitFixture::builder()
            .commit("feat: first")
            .tag("v1.0.0")
            .build()
            .unwrap();

        assert_eq!(
            (
                ours(&fixture, "v1.4.0"),
                ours(&without, "v1.0.0"),
                ours(&fixture, "v1.0.0").map(|texts| texts.0.is_some())
            ),
            (Ok((None, None, None)), Ok((None, None, None)), Ok(true))
        );
    }

    #[test]
    fn a_broken_notes_ref_is_a_read_failure() {
        let fixture = fixture();
        std::fs::write(
            fixture
                .path()
                .join(".git")
                .join("refs")
                .join("notes")
                .join("semoxide-v1.0.0"),
            "garbage\n",
        )
        .unwrap();
        let repo = Repo::discover(&fixture.path()).unwrap();
        let tag = repo.tag_commit("v1.0.0").unwrap().unwrap();

        assert_eq!(
            repo.tag_notes(&tag).map_err(|error| error.code()),
            Err(GIT_READ_FAILED)
        );
    }
}
