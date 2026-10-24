//! A tag's channel notes (G9 read): semoxide's own ref and upstream's two, each read on its
//! own so notes from several refs never run together (upstream #4073). Parsing them and
//! choosing between them is the runtime's job.

use tracing::debug;

use crate::id::to_oid;
use crate::{GitError, ObjectId, Repo, Tag};

const OURS_PREFIX: &str = "refs/notes/semoxide-";
const UPSTREAM_PREFIX: &str = "refs/notes/semantic-release-";
const UPSTREAM_LEGACY: &str = "refs/notes/semantic-release";

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
    pub fn tag_notes(&self, tag: &Tag) -> Result<TagNotes, GitError> {
        let read = |error: git2::Error| GitError::read_failed(&error);
        let name = tag.name();
        let commit = to_oid(tag.commit()).map_err(read)?;
        let annotation = tag
            .annotation()
            .map(ObjectId::to_oid)
            .transpose()
            .map_err(read)?;
        // Upstream's notes: the tag object first, where upstream writes them, then the commit.
        let upstream_on: Vec<git2::Oid> = annotation.into_iter().chain([commit]).collect();
        let notes = TagNotes {
            semoxide: self.note(&format!("{OURS_PREFIX}{name}"), &[commit])?,
            upstream: self.note(&format!("{UPSTREAM_PREFIX}{name}"), &upstream_on)?,
            upstream_legacy: self.note(UPSTREAM_LEGACY, &upstream_on)?,
        };
        debug!(
            target: "semoxide::git",
            op = "G9",
            tag = name,
            semoxide = notes.semoxide.is_some(),
            upstream = notes.upstream.is_some(),
            upstream_legacy = notes.upstream_legacy.is_some(),
            "tag notes read"
        );
        Ok(notes)
    }

    /// The first note in `notes_ref` on one of `objects`.
    fn note(&self, notes_ref: &str, objects: &[git2::Oid]) -> Result<Option<String>, GitError> {
        for &object in objects {
            match self.git().find_note(Some(notes_ref), object) {
                Ok(note) => {
                    return Ok(Some(
                        String::from_utf8_lossy(note.message_bytes()).into_owned(),
                    ));
                }
                // No notes ref, no note, or a tag name that makes no valid ref name.
                Err(error)
                    if matches!(
                        error.code(),
                        git2::ErrorCode::NotFound | git2::ErrorCode::InvalidSpec
                    ) => {}
                Err(error) => return Err(GitError::read_failed(&error)),
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
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
    /// - `v1.2.0`, annotated: upstream's note on the commit only, as a fixed upstream would;
    ///   ours on the tag object only, where semoxide never looks.
    /// - `v1.3.0`, annotated: upstream's and the legacy note on both the tag object and the
    ///   commit.
    /// - `v1.4.0`: no notes.
    /// - `v1.5.0`, annotated: only a legacy note, on the commit.
    /// - `v1.6.0`: only ours.
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
            .note("v1.2.0", "semoxide-v1.2.0", OURS)
            .commit("fix: fourth")
            .annotated_tag("v1.3.0", "release 1.3.0")
            .note("v1.3.0", "semantic-release-v1.3.0", "on the tag object")
            .note(
                "v1.3.0^{commit}",
                "semantic-release-v1.3.0",
                "on the commit",
            )
            .note("v1.3.0", "semantic-release", "legacy on the tag object")
            .note(
                "v1.3.0^{commit}",
                "semantic-release",
                "legacy on the commit",
            )
            .commit("fix: fifth")
            .tag("v1.4.0")
            .commit("fix: sixth")
            .annotated_tag("v1.5.0", "release 1.5.0")
            .note("v1.5.0^{commit}", "semantic-release", LEGACY)
            .commit("fix: seventh")
            .tag("v1.6.0")
            .note("v1.6.0", "semoxide-v1.6.0", OURS)
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

    /// Each ref's note stays its own and is looked up only where it belongs.
    #[test]
    fn each_ref_keeps_to_itself_and_its_place() {
        let fixture = fixture();
        let note = |text: &str| Some(format!("{text}\n"));

        assert_eq!(
            [
                ours(&fixture, "v1.2.0"),
                ours(&fixture, "v1.3.0"),
                ours(&fixture, "v1.5.0"),
                ours(&fixture, "v1.6.0"),
            ],
            [
                Ok((None, note(UPSTREAM), None)),
                Ok((
                    None,
                    note("on the tag object"),
                    note("legacy on the tag object")
                )),
                Ok((None, None, note(LEGACY))),
                Ok((note(OURS), None, None)),
            ]
        );
    }

    #[test]
    fn a_note_that_isnt_utf_8_is_decoded_with_replacement_characters() {
        let fixture = GitFixture::builder()
            .commit("feat: first")
            .tag("v1.0.0")
            .build()
            .unwrap();
        std::fs::write(fixture.path().join("note.bin"), b"a\xffb\n").unwrap();
        let blob = fixture.git(&["hash-object", "-w", "note.bin"]).unwrap();
        fixture
            .git(&[
                "notes",
                "--ref",
                "semoxide-v1.0.0",
                "add",
                "-C",
                &blob,
                "v1.0.0",
            ])
            .unwrap();

        assert_eq!(
            ours(&fixture, "v1.0.0"),
            Ok((Some(String::from("a\u{fffd}b\n")), None, None))
        );
    }

    /// `refs/tags/v1..0` can be listed from `packed-refs`, but `refs/notes/semoxide-v1..0`
    /// isn't a valid ref name, so it has no notes rather than an error.
    #[test]
    fn a_tag_whose_notes_ref_would_be_invalid_has_no_notes() {
        let fixture = GitFixture::builder()
            .commit("feat: first")
            .tag("v1.0.0")
            .note("v1.0.0", "semoxide-v1.0.0", OURS)
            .build()
            .unwrap();
        let head = fixture.rev_parse("HEAD").unwrap();
        std::fs::write(
            fixture.path().join(".git").join("packed-refs"),
            format!("{head} refs/tags/v1..0\n"),
        )
        .unwrap();
        let repo = Repo::discover(&fixture.path()).unwrap();
        let notes = repo.head().and_then(|head| {
            repo.merged_tags(head.commit())?
                .iter()
                .map(|tag| {
                    repo.tag_notes(tag)
                        .map(|notes| (tag.name().to_owned(), notes.semoxide().map(str::to_owned)))
                })
                .collect::<Result<Vec<_>, _>>()
        });

        assert_eq!(
            notes.map_err(|error| error.to_string()),
            Ok(vec![
                (String::from("v1..0"), None),
                (String::from("v1.0.0"), Some(format!("{OURS}\n")))
            ])
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

    #[rstest]
    #[case::ours("semoxide-v1.0.0")]
    #[case::upstream("semantic-release-v1.0.0")]
    #[case::upstream_legacy("semantic-release")]
    fn a_broken_notes_ref_is_a_read_failure(#[case] notes_ref: &str) {
        let fixture = fixture();
        std::fs::write(
            fixture
                .path()
                .join(".git")
                .join("refs")
                .join("notes")
                .join(notes_ref),
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
