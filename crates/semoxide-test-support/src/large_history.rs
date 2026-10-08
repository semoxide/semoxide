//! Large generated histories for benchmarks (TESTING: benchmarks), written in one
//! `git fast-import` stream: 100k commits take seconds, where per-step git calls take minutes.

use std::fmt::Write as _;

use crate::git_fixture::{FixtureError, GitFixture};

#[cfg(test)]
mod tests;

const IDENTITY: &str = "semoxide-test <test@example.invalid>";
/// 2026-01-01T00:00:00Z; commit `n` is `n` minutes later.
const FIRST_TIME: u64 = 1_767_225_600;
const NOTE: &str = r#"{"channels":[null]}"#;

/// Whether each tag gets a git note (as upstream stores a release's channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagNotes {
    /// A note under `refs/notes/semoxide` on every tag.
    OnEveryTag,
    /// No notes.
    None,
}

/// A linear history on `main`: commits cycling `feat`, `fix`, `chore` and `docs`, each changing
/// one file; a lightweight tag `v1.<n / 100>.<n % 100>` every `tag_every` commits, except in the
/// untagged tail.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct LargeHistory {
    commits: u64,
    tag_every: u64,
    untagged_tail: u64,
    notes: TagNotes,
}

impl LargeHistory {
    /// `commits` commits without tags.
    pub fn new(commits: u64) -> Self {
        Self {
            commits,
            tag_every: 0,
            untagged_tail: 0,
            notes: TagNotes::None,
        }
    }

    /// The benchmark scenario: 100k commits, a tag with a note every 20 commits (3,500 tags),
    /// and the last 30k commits untagged. Sized after real reports: upstream slows down from
    /// about 1k tags with notes, and API-backed tools from tens of thousands of commits since the
    /// last release.
    pub fn large() -> Self {
        Self::new(100_000)
            .tag_every(20)
            .untagged_tail(30_000)
            .notes(TagNotes::OnEveryTag)
    }

    /// A tag every `commits` commits; 0 means none.
    pub fn tag_every(mut self, commits: u64) -> Self {
        self.tag_every = commits;
        self
    }

    /// The last `commits` commits get no tag.
    pub fn untagged_tail(mut self, commits: u64) -> Self {
        self.untagged_tail = commits;
        self
    }

    /// Whether tags get notes.
    pub fn notes(mut self, notes: TagNotes) -> Self {
        self.notes = notes;
        self
    }

    /// Generates the history in a new [`GitFixture`].
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if git fails.
    pub fn build(&self) -> Result<GitFixture, FixtureError> {
        let fixture = GitFixture::builder().build()?;
        fixture.fast_import(self.stream().as_bytes())?;
        Ok(fixture)
    }

    fn stream(&self) -> String {
        let mut stream = String::new();
        let mut tagged = Vec::new();
        let mut releases: u64 = 0;
        for commit in 1..=self.commits {
            let time = FIRST_TIME + commit * 60;
            let kind = match commit % 4 {
                0 => "feat",
                1 => "fix",
                2 => "chore",
                _ => "docs",
            };
            let message = format!("{kind}: change {commit}");
            let content = format!("line {commit}");
            // `write!` into a String can't fail.
            let _ = write!(
                stream,
                "commit refs/heads/main\nmark :{commit}\nauthor {IDENTITY} {time} +0000\n\
                 committer {IDENTITY} {time} +0000\ndata {}\n{message}\n",
                message.len()
            );
            if commit > 1 {
                let _ = writeln!(stream, "from :{}", commit - 1);
            }
            let _ = write!(
                stream,
                "M 644 inline file.txt\ndata {}\n{content}\n\n",
                content.len()
            );
            if self.is_tagged(commit) {
                releases += 1;
                let _ = write!(
                    stream,
                    "reset refs/tags/v1.{}.{}\nfrom :{commit}\n\n",
                    releases / 100,
                    releases % 100
                );
                tagged.push(commit);
            }
        }
        if self.notes == TagNotes::OnEveryTag && !tagged.is_empty() {
            let time = FIRST_TIME + self.commits * 60;
            let _ = write!(
                stream,
                "commit refs/notes/semoxide\ncommitter {IDENTITY} {time} +0000\ndata 5\nnotes\n"
            );
            for commit in tagged {
                let _ = write!(stream, "N inline :{commit}\ndata {}\n{NOTE}\n", NOTE.len());
            }
            stream.push('\n');
        }
        stream
    }

    fn is_tagged(&self, commit: u64) -> bool {
        self.tag_every > 0
            && commit.is_multiple_of(self.tag_every)
            && commit <= self.commits.saturating_sub(self.untagged_tail)
    }
}
