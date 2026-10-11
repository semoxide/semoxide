//! The commits of a range, `[from..]to`, in `git log` order (G16), with every field upstream
//! gives plugins.

use crate::{CommitId, GitError, ObjectId, Repo};

/// A commit as `git log --format` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Commit {
    id: CommitId,
    short_id: String,
    tree_id: ObjectId,
    short_tree_id: String,
    parents: Vec<CommitId>,
    author: Signature,
    committer: Signature,
    raw_message: String,
    git_tags: String,
}

/// Who wrote or committed a commit, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Signature {
    name: String,
    email: String,
    time: Time,
}

/// A point in time with the UTC offset it was recorded in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Time {
    seconds: i64,
    offset_minutes: i32,
}

impl Commit {
    /// `%H`
    #[must_use]
    pub fn id(&self) -> &CommitId {
        &self.id
    }

    /// `%h`: the shortest unique prefix, at least 7 characters.
    #[must_use]
    pub fn short_id(&self) -> &str {
        &self.short_id
    }

    /// `%T`
    #[must_use]
    pub fn tree_id(&self) -> &ObjectId {
        &self.tree_id
    }

    /// `%t`
    #[must_use]
    pub fn short_tree_id(&self) -> &str {
        &self.short_tree_id
    }

    /// `%P`, in order.
    #[must_use]
    pub fn parents(&self) -> &[CommitId] {
        &self.parents
    }

    /// `%an`, `%ae`, `%ad`
    #[must_use]
    pub fn author(&self) -> &Signature {
        &self.author
    }

    /// `%cn`, `%ce`, `%cd`
    #[must_use]
    pub fn committer(&self) -> &Signature {
        &self.committer
    }

    /// `%s`: the first paragraph, its lines joined with spaces.
    #[must_use]
    pub fn subject(&self) -> String {
        String::new()
    }

    /// `%b`: everything after the first paragraph.
    #[must_use]
    pub fn body(&self) -> String {
        String::new()
    }

    /// `%B` trimmed at both ends, as upstream gives plugins.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.raw_message
    }

    /// The message exactly as stored.
    #[must_use]
    pub fn raw_message(&self) -> &str {
        &self.raw_message
    }

    /// `%d`: the refs pointing here, e.g. ` (HEAD -> main, tag: v1.0.0)`; empty without any.
    #[must_use]
    pub fn git_tags(&self) -> &str {
        &self.git_tags
    }

    /// `%ci`, e.g. `2026-01-01 00:00:00 +0000`.
    #[must_use]
    pub fn committer_date(&self) -> String {
        String::new()
    }
}

impl Signature {
    /// The name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The email.
    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    /// When.
    #[must_use]
    pub fn time(&self) -> Time {
        self.time
    }
}

impl Time {
    /// Seconds since the Unix epoch.
    #[must_use]
    pub fn seconds(self) -> i64 {
        self.seconds
    }

    /// The UTC offset in minutes.
    #[must_use]
    pub fn offset_minutes(self) -> i32 {
        self.offset_minutes
    }
}

impl Repo {
    /// The commits reachable from `to` but not from `from`, in `git log` order; `None` is the
    /// whole history (`git log [from..]to`). Text that isn't UTF-8 is decoded with
    /// replacement characters.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if a commit doesn't exist or libgit2 fails to read the history.
    pub fn log(&self, _from: Option<&CommitId>, _to: &CommitId) -> Result<Vec<Commit>, GitError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use semoxide_error::ErrorInfo;
    use semoxide_test_support::git_fixture::{GitFixture, Merge};

    use super::*;
    use crate::codes::GIT_READ_FAILED;

    /// Tags, a commit whose first paragraph wraps, a body with a footer, a merged branch, a
    /// pushed `origin/main`, and last a commit by another author with a message that isn't
    /// UTF-8.
    fn fixture() -> GitFixture {
        let fixture = GitFixture::builder()
            .commit("feat: first")
            .tag("v1.0.0")
            .commit("fix: second\n\nbody line\n\nBREAKING CHANGE: changed")
            .commit("feat: a subject\nthat wraps\n\nand a body")
            .annotated_tag("v1.1.0", "release 1.1.0")
            .branch("feature")
            .commit("feat: on feature")
            .checkout("main")
            .commit("fix: on main")
            .merge("feature", Merge::NoFastForward)
            .push()
            .build()
            .unwrap();
        let message = fixture.path().join("message.txt");
        std::fs::write(&message, b"fix: price \xa3 5\n\nbody \xb5 \xe4\xef end\n").unwrap();
        fixture
            .git(&[
                "commit",
                "--allow-empty",
                "--author=Ann Other <ann@example.invalid>",
                "-F",
                &message.to_string_lossy(),
            ])
            .unwrap();
        fixture
    }

    fn commit(fixture: &GitFixture, rev: &str) -> CommitId {
        CommitId::from(git2::Oid::from_str(&fixture.rev_parse(rev).unwrap()).unwrap())
    }

    fn repo(fixture: &GitFixture) -> Repo {
        Repo::discover(&fixture.path()).unwrap()
    }

    fn ids(commits: &[Commit]) -> Vec<String> {
        commits
            .iter()
            .map(|commit| commit.id().to_string())
            .collect()
    }

    /// `git log --format=%H <range>`
    fn ids_by_git(fixture: &GitFixture, range: &str) -> Vec<String> {
        fixture
            .git(&["log", "--format=%H", range])
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn the_order_matches_git_log_for_the_history_and_ranges() {
        let fixture = fixture();
        let repo = repo(&fixture);
        let log = |from: Option<&str>, to: &str| {
            repo.log(
                from.map(|rev| commit(&fixture, rev)).as_ref(),
                &commit(&fixture, to),
            )
            .map(|commits| ids(&commits))
        };

        assert_eq!(
            (
                log(None, "HEAD"),
                log(Some("v1.0.0"), "HEAD"),
                log(Some("feature"), "main"),
                log(Some("HEAD"), "HEAD")
            ),
            (
                Ok(ids_by_git(&fixture, "HEAD")),
                Ok(ids_by_git(&fixture, "v1.0.0..HEAD")),
                Ok(ids_by_git(&fixture, "feature..main")),
                Ok(Vec::new())
            )
        );
    }

    /// Every field as one line, separated like the `git log` format below.
    fn describe(commit: &Commit) -> String {
        let signature = |who: &Signature| {
            let offset = who.time().offset_minutes();
            format!(
                "{}\0{}\0{} {}{:02}{:02}",
                who.name(),
                who.email(),
                who.time().seconds(),
                if offset < 0 { '-' } else { '+' },
                offset.abs() / 60,
                offset.abs() % 60
            )
        };
        let parents: Vec<String> = commit.parents().iter().map(ToString::to_string).collect();
        [
            commit.id().to_string(),
            commit.short_id().to_owned(),
            commit.tree_id().to_string(),
            commit.short_tree_id().to_owned(),
            parents.join(" "),
            signature(commit.author()),
            signature(commit.committer()),
            commit.subject(),
            commit.body(),
            commit.git_tags().to_owned(),
            commit.committer_date(),
        ]
        .join("\0")
    }

    /// The same fields from `git log -1 --format`, raw dates as `<seconds> <offset>`.
    fn described_by_git(fixture: &GitFixture, id: &str) -> String {
        fixture
            .git(&[
                "log",
                "-1",
                "--date=raw",
                "--format=%H%x00%h%x00%T%x00%t%x00%P%x00%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd\
                 %x00%s%x00%b%x00%d%x00%ci",
                id,
            ])
            .unwrap()
    }

    #[test]
    fn every_field_matches_git_log_format() {
        let fixture = fixture();
        // The fixture decorates every kind of ref, so the comparison covers each.
        let decorations = fixture.git(&["log", "--format=%d", "HEAD"]).unwrap();
        for expected in [
            "HEAD -> main",
            "tag: v1.0.0",
            "tag: v1.1.0",
            "origin/main",
            "feature",
        ] {
            assert!(
                decorations.contains(expected),
                "{expected} in {decorations}"
            );
        }
        let commits = repo(&fixture).log(None, &commit(&fixture, "HEAD"));
        let ours: Result<Vec<String>, String> = commits
            .map(|commits| commits.iter().map(describe).collect())
            .map_err(|error| error.to_string());
        let git: Vec<String> = ids_by_git(&fixture, "HEAD")
            .iter()
            .map(|id| described_by_git(&fixture, id))
            .collect();

        assert_eq!(ours, Ok(git));
    }

    #[test]
    fn the_message_is_trimmed_and_the_raw_message_is_as_stored() {
        let fixture = fixture();
        let commits = repo(&fixture)
            .log(None, &commit(&fixture, "HEAD"))
            .unwrap_or_default();
        let messages: Vec<(&str, &str)> = commits
            .iter()
            .map(|commit| (commit.message(), commit.raw_message()))
            .collect();
        let expected: Vec<(String, String)> = ids_by_git(&fixture, "HEAD")
            .iter()
            .map(|id| {
                let message = fixture.git(&["log", "-1", "--format=%B", id]).unwrap();
                let stored = fixture.git(&["cat-file", "commit", id]).unwrap();
                // The fixture's output drops the final newline every stored message ends with.
                let raw = format!("{}\n", stored.split_once("\n\n").unwrap().1);
                (message.trim().to_owned(), raw)
            })
            .collect();

        assert_eq!(
            messages,
            expected
                .iter()
                .map(|(message, raw)| (message.as_str(), raw.as_str()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn text_that_isnt_utf_8_is_decoded_with_replacement_characters() {
        let fixture = fixture();
        let head = repo(&fixture)
            .log(Some(&commit(&fixture, "HEAD~1")), &commit(&fixture, "HEAD"))
            .unwrap_or_default();

        assert_eq!(
            head.first()
                .map(|commit| (commit.subject(), commit.author().name().to_owned())),
            Some((
                String::from("fix: price \u{fffd} 5"),
                String::from("Ann Other")
            ))
        );
    }

    #[test]
    fn an_unknown_commit_is_a_read_failure() {
        let fixture = fixture();
        let repo = repo(&fixture);
        let unknown = CommitId::from(
            git2::Oid::from_str("1111111111111111111111111111111111111111").unwrap(),
        );
        let head = commit(&fixture, "HEAD");
        let code = |result: Result<Vec<Commit>, GitError>| {
            result.map(|_| ()).map_err(|error| error.code())
        };

        assert_eq!(
            (
                code(repo.log(None, &unknown)),
                code(repo.log(Some(&unknown), &head))
            ),
            (Err(GIT_READ_FAILED), Err(GIT_READ_FAILED))
        );
    }
}
