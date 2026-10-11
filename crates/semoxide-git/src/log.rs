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
    /// pushed `origin/main`, and last a commit by another author.
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
        fixture
            .git(&[
                "commit",
                "--allow-empty",
                "--author=Ann Other <ann@example.invalid>",
                "-m",
                "fix: by another author",
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

    /// Writes a commit object exactly as given: `git commit` re-encodes a message that isn't
    /// UTF-8 (as Latin-1) and can't set the dates or offsets.
    fn raw_commit(fixture: &GitFixture, object: &[u8]) -> String {
        let path = fixture.path().join(".git").join("raw-commit");
        std::fs::write(&path, object).unwrap();
        fixture
            .git(&["hash-object", "-t", "commit", "-w", &path.to_string_lossy()])
            .unwrap()
    }

    /// A commit object on `parents` with the fixture's current tree.
    fn object(
        fixture: &GitFixture,
        parents: &[&str],
        author: &[u8],
        committer: &[u8],
        message: &[u8],
    ) -> Vec<u8> {
        let tree = fixture.rev_parse("HEAD^{tree}").unwrap();
        let mut object = format!("tree {tree}\n").into_bytes();
        for parent in parents {
            object.extend(format!("parent {parent}\n").as_bytes());
        }
        object.extend(b"author ");
        object.extend(author);
        object.extend(b"\ncommitter ");
        object.extend(committer);
        object.extend(b"\n\n");
        object.extend(message);
        object
    }

    // --- Order ---

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

    /// Committer dates that go back in time, and ties, as rebases and scripts make them.
    #[test]
    fn the_order_matches_git_log_with_clock_skew_and_equal_dates() {
        let fixture = GitFixture::builder().commit("feat: first").build().unwrap();
        let first = fixture.rev_parse("HEAD").unwrap();
        let at = |seconds: u64| format!("T <t@example.invalid> {seconds} +0000").into_bytes();
        let make = |parents: &[&str], seconds: u64, message: &str| {
            let made = object(
                &fixture,
                parents,
                &at(seconds),
                &at(seconds),
                message.as_bytes(),
            );
            raw_commit(&fixture, &made)
        };
        // `older` is dated before its parent `newer`, so git shows `newer` (through `side`)
        // first.
        let newer = make(&[&first], 1_900_000_100, "newer");
        let older = make(&[&newer], 1_900_000_010, "older");
        let side = make(&[&newer], 1_900_000_050, "side");
        let merge = make(&[&older, &side], 1_900_000_200, "merge");
        // Two branches whose commits all share one date, merged.
        let left = make(&[&merge], 1_900_000_300, "left");
        let right = make(&[&merge], 1_900_000_300, "right");
        let left_tip = make(&[&left], 1_900_000_300, "left tip");
        let right_tip = make(&[&right], 1_900_000_300, "right tip");
        let tie = make(&[&left_tip, &right_tip], 1_900_000_300, "tie");
        fixture
            .git(&["update-ref", "refs/heads/main", &tie])
            .unwrap();
        let repo = repo(&fixture);

        assert_eq!(
            (
                repo.log(None, &commit(&fixture, "HEAD"))
                    .map(|commits| ids(&commits)),
                repo.log(Some(&commit(&fixture, &older)), &commit(&fixture, "HEAD"))
                    .map(|commits| ids(&commits))
            ),
            (
                Ok(ids_by_git(&fixture, "HEAD")),
                Ok(ids_by_git(&fixture, &format!("{older}..HEAD")))
            )
        );
    }

    // --- Fields ---

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

    /// `%d` as git 2.38 and later show it by default: git 2.37 also lists other refs by their
    /// full name (`refs/notes/…`, `refs/pull/…`), which are dropped here.
    fn default_decorations(decorations: &str) -> String {
        let Some(list) = decorations
            .strip_prefix(" (")
            .and_then(|rest| rest.strip_suffix(')'))
        else {
            return decorations.to_owned();
        };
        let kept: Vec<&str> = list
            .split(", ")
            .filter(|name| !name.starts_with("refs/") || *name == "refs/stash")
            .collect();
        if kept.is_empty() {
            String::new()
        } else {
            format!(" ({})", kept.join(", "))
        }
    }

    /// The same fields from `git log -1 --format`, raw dates as `<seconds> <offset>`.
    fn described_by_git(fixture: &GitFixture, id: &str) -> String {
        let fields = fixture
            .git(&[
                "log",
                "-1",
                "--date=raw",
                "--format=%H%x00%h%x00%T%x00%t%x00%P%x00%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd\
                 %x00%s%x00%b%x00%d%x00%ci",
                id,
            ])
            .unwrap();
        let mut fields: Vec<String> = fields.split('\0').map(str::to_owned).collect();
        // `%d` is the second to last field.
        let decorations = fields.len() - 2;
        fields[decorations] = default_decorations(&fields[decorations]);
        fields.join("\0")
    }

    /// Every commit's fields against git's, for the commits `rev` reaches.
    fn fields_match_git(fixture: &GitFixture, rev: &str) {
        let commits = repo(fixture).log(None, &commit(fixture, rev));
        let ours: Result<Vec<String>, String> = commits
            .map(|commits| commits.iter().map(describe).collect())
            .map_err(|error| error.to_string());
        let git: Vec<String> = ids_by_git(fixture, rev)
            .iter()
            .map(|id| described_by_git(fixture, id))
            .collect();

        assert_eq!(ours, Ok(git));
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

        fields_match_git(&fixture, "HEAD");
    }

    /// Messages shaped every way `%s` and `%b` split differently, and signatures with offsets
    /// of both signs, in minutes and with a local date past midnight; on `main`.
    fn messages_fixture() -> GitFixture {
        let fixture = GitFixture::builder().commit("feat: first").build().unwrap();
        let messages: [&[u8]; 11] = [
            b"\n\n  leading blank lines\nwrapped line  \n \t \nbody after a blank-ish line\n",
            b"crlf subject\r\nwraps\r\n\r\nbody\r\nline\r\n",
            b"no newline at the end",
            b"subject only\n\n\n\n",
            b"subject\n\nbody with blank lines after\n\n\n",
            b"form feed\x0c\n\x0c\nis not blank to git\n",
            b"",
            b"   \n\t\n",
            b"\xc2\xa0nbsp subject \xc2\xa0\n\nbody\n",
            b"tab\tsubject\n\n\n\nbody after blank lines\n\n",
            b"fix: price \xa3 5\n\nbody \xb5 \xe4\xef end\n",
        ];
        let authors: [&[u8]; 4] = [
            b"Ann Other <ann@example.invalid> 1835479800 +0130",
            b"Ann \xe9 <ann\xe9@example.invalid> 1798760700 -0700",
            b"A <a@example.invalid> 1772324100 -0030",
            b"B <b@example.invalid> 1772324100 +0545",
        ];
        // Committer times a day apart around midnight UTC, with offsets moving the local date.
        let committers = [
            (
                "Cee Committer <cee@example.invalid>",
                1_798_760_700,
                "+0130",
            ),
            ("Dee <dee@example.invalid>", 1_835_479_800, "-0030"),
            ("Eve <eve@example.invalid>", 1_772_324_100, "+0545"),
            ("Fay <fay@example.invalid>", 1_798_760_700, "-0700"),
        ];
        let mut parent = fixture.rev_parse("HEAD").unwrap();
        for (index, message) in messages.iter().enumerate() {
            let author = authors[index % authors.len()];
            let (who, seconds, offset) = committers[index % committers.len()];
            let committer = format!("{who} {} {offset}", seconds + 86_400 * index);
            let made = object(&fixture, &[&parent], author, committer.as_bytes(), message);
            parent = raw_commit(&fixture, &made);
        }
        fixture
            .git(&["update-ref", "refs/heads/main", &parent])
            .unwrap();
        fixture
    }

    #[test]
    fn every_field_matches_git_for_awkward_messages_and_signatures() {
        fields_match_git(&messages_fixture(), "HEAD");
    }

    /// `message()` is `%B` trimmed and `raw_message()` is `%B` whole.
    fn messages_match_git(fixture: &GitFixture) {
        let commits = repo(fixture)
            .log(None, &commit(fixture, "HEAD"))
            .unwrap_or_default();
        let ours: Vec<(String, String)> = commits
            .iter()
            .map(|commit| (commit.message().to_owned(), commit.raw_message().to_owned()))
            .collect();
        let expected: Vec<(String, String)> = ids_by_git(fixture, "HEAD")
            .iter()
            .map(|id| {
                // The `%x00` at the end keeps the fixture's newline trim off `%B`.
                let raw = fixture.git(&["log", "-1", "--format=%B%x00", id]).unwrap();
                let raw = raw.strip_suffix('\0').unwrap().to_owned();
                (raw.trim().to_owned(), raw)
            })
            .collect();

        assert_eq!(ours, expected);
    }

    #[test]
    fn the_message_is_trimmed_and_the_raw_message_is_as_stored() {
        messages_match_git(&fixture());
    }

    #[test]
    fn messages_match_git_for_awkward_messages() {
        messages_match_git(&messages_fixture());
    }

    #[test]
    fn non_utf_8_text_is_decoded_with_replacement_characters() {
        let fixture = messages_fixture();
        let repo = repo(&fixture);
        let only = |from: &str, to: &str| {
            repo.log(Some(&commit(&fixture, from)), &commit(&fixture, to))
                .unwrap_or_default()
        };
        let last = only("HEAD~1", "HEAD");
        let second = only("HEAD~10", "HEAD~9");

        assert_eq!(
            (
                last.first().map(|commit| (commit.subject(), commit.body())),
                second.first().map(|commit| (
                    commit.author().name().to_owned(),
                    commit.author().email().to_owned()
                ))
            ),
            (
                Some((
                    String::from("fix: price \u{fffd} 5"),
                    String::from("body \u{fffd} \u{fffd}\u{fffd} end\n")
                )),
                Some((
                    String::from("Ann \u{fffd}"),
                    String::from("ann\u{fffd}@example.invalid")
                ))
            )
        );
    }

    #[test]
    fn short_ids_follow_core_abbrev() {
        let fixture = fixture();
        fixture.git(&["config", "core.abbrev", "12"]).unwrap();
        let ours = repo(&fixture)
            .log(None, &commit(&fixture, "HEAD"))
            .map(|commits| {
                commits
                    .iter()
                    .map(|commit| format!("{} {}", commit.short_id(), commit.short_tree_id()))
                    .collect::<Vec<_>>()
            });
        let git: Vec<String> = fixture
            .git(&["log", "--format=%h %t", "HEAD"])
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();

        assert_eq!(ours, Ok(git));
    }

    // --- Decorations ---

    /// Tags created out of name order, a tag of a tag, a loose tag next to packed ones,
    /// branches and remote branches on one commit, a symbolic `origin/HEAD`, and a notes and a
    /// pull request ref that current git doesn't show.
    fn decorations_fixture() -> GitFixture {
        let fixture = GitFixture::builder()
            .commit("feat: first")
            .tag("v1.0.0")
            .commit("fix: second")
            .tag("v2.0.0")
            .annotated_tag("v10.0.0", "ten")
            .tag("V3.0.0")
            .tag("a-tag")
            .branch("zeta")
            .checkout("main")
            .branch("alpha")
            .checkout("main")
            .note("HEAD", "semoxide-v2.0.0", "{}")
            .push()
            .build()
            .unwrap();
        for args in [
            &["tag", "-a", "nested", "-m", "nested", "v10.0.0"][..],
            &["update-ref", "refs/remotes/upstream/main", "HEAD~1"],
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
            &["update-ref", "refs/pull/1/head", "HEAD"],
            &["pack-refs", "--all"],
            &["tag", "loose", "HEAD"],
        ] {
            fixture.git(args).unwrap();
        }
        fixture
    }

    /// `%d` of each commit on `main`, as current git shows it.
    fn decorations_by_git(fixture: &GitFixture) -> Vec<String> {
        fixture
            .git(&["log", "--format=%d", "main"])
            .unwrap()
            .lines()
            .map(default_decorations)
            .collect()
    }

    #[test]
    fn decorations_match_git_in_order_for_every_kind_of_ref() {
        let fixture = decorations_fixture();
        let ours = |fixture: &GitFixture| {
            repo(fixture)
                .log(None, &commit(fixture, "main"))
                .map(|commits| {
                    commits
                        .iter()
                        .map(|commit| commit.git_tags().to_owned())
                        .collect::<Vec<_>>()
                })
        };
        let attached = (ours(&fixture), decorations_by_git(&fixture));
        fixture.git(&["checkout", "--detach", "HEAD~1"]).unwrap();
        let detached_below = (ours(&fixture), decorations_by_git(&fixture));
        fixture.git(&["checkout", "--detach", "main"]).unwrap();
        let detached = (ours(&fixture), decorations_by_git(&fixture));

        assert_eq!(
            (attached.0, detached_below.0, detached.0),
            (Ok(attached.1), Ok(detached_below.1), Ok(detached.1))
        );
    }

    // --- Errors ---

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
