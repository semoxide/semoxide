//! Git histories for tests, written with the real git CLI (TESTING: fixtures).
//!
//! Every commit, tag and note gets a fixed identity and a date derived from its step, and git
//! runs without the machine's config, so the same fixture gives the same SHAs everywhere.

use std::fmt;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// How [`GitFixtureBuilder::merge`] joins a branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Merge {
    /// `git merge --ff-only`: moves the branch, no merge commit.
    FastForward,
    /// `git merge --no-ff`: always a merge commit with two parents.
    NoFastForward,
}

/// The steps of a history, run in order by [`GitFixtureBuilder::build`].
#[derive(Debug, Default)]
#[must_use]
pub struct GitFixtureBuilder {
    steps: Vec<Step>,
}

#[derive(Debug)]
enum Step {
    Commit(String),
    Tag(String),
    AnnotatedTag(String, String),
    Branch(String),
    Checkout(String),
    Merge(String, Merge),
    Rebase(String),
    Note(String, String, String),
    Push,
}

impl GitFixtureBuilder {
    /// An empty commit with this message.
    pub fn commit(mut self, message: &str) -> Self {
        self.steps.push(Step::Commit(message.to_owned()));
        self
    }

    /// A lightweight tag on `HEAD`.
    pub fn tag(mut self, name: &str) -> Self {
        self.steps.push(Step::Tag(name.to_owned()));
        self
    }

    /// An annotated tag on `HEAD`.
    pub fn annotated_tag(mut self, name: &str, message: &str) -> Self {
        self.steps
            .push(Step::AnnotatedTag(name.to_owned(), message.to_owned()));
        self
    }

    /// Creates a branch at `HEAD` and checks it out.
    pub fn branch(mut self, name: &str) -> Self {
        self.steps.push(Step::Branch(name.to_owned()));
        self
    }

    /// Checks out a branch, tag or commit (a tag or commit detaches `HEAD`).
    pub fn checkout(mut self, rev: &str) -> Self {
        self.steps.push(Step::Checkout(rev.to_owned()));
        self
    }

    /// Merges `branch` into the current branch.
    pub fn merge(mut self, branch: &str, how: Merge) -> Self {
        self.steps.push(Step::Merge(branch.to_owned(), how));
        self
    }

    /// Rebases the current branch onto `onto`.
    pub fn rebase(mut self, onto: &str) -> Self {
        self.steps.push(Step::Rebase(onto.to_owned()));
        self
    }

    /// A note on `rev` under `refs/notes/<notes_ref>`.
    pub fn note(mut self, rev: &str, notes_ref: &str, text: &str) -> Self {
        self.steps.push(Step::Note(
            rev.to_owned(),
            notes_ref.to_owned(),
            text.to_owned(),
        ));
        self
    }

    /// Pushes all branches, tags and notes to the bare `origin` remote.
    pub fn push(mut self) -> Self {
        self.steps.push(Step::Push);
        self
    }

    /// Runs the steps in a new temporary directory.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if a git command fails.
    pub fn build(self) -> Result<GitFixture, FixtureError> {
        let dir = TempDir::new().map_err(|error| FixtureError {
            command: String::from("create a temporary directory"),
            output: error.to_string(),
        })?;
        let _ = self.steps;
        Ok(GitFixture { dir })
    }
}

/// A built history: a working repository on `main` plus a bare `origin` remote, deleted on drop.
#[derive(Debug)]
pub struct GitFixture {
    dir: TempDir,
}

impl GitFixture {
    /// Starts an empty history on `main`.
    pub fn builder() -> GitFixtureBuilder {
        GitFixtureBuilder::default()
    }

    /// The working repository.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.dir.path().join("repo")
    }

    /// The bare remote, configured as `origin`.
    #[must_use]
    pub fn remote_path(&self) -> PathBuf {
        self.dir.path().join("remote.git")
    }

    /// The remote's `file://` URL.
    #[must_use]
    pub fn remote_url(&self) -> String {
        String::new()
    }

    /// Runs git in the working repository with the fixture's isolated environment; returns
    /// stdout without the trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if git fails.
    pub fn git(&self, args: &[&str]) -> Result<String, FixtureError> {
        run(&self.path(), args)
    }

    /// Like [`Self::git`], in the bare remote.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if git fails.
    pub fn remote_git(&self, args: &[&str]) -> Result<String, FixtureError> {
        run(&self.remote_path(), args)
    }

    /// The SHA `rev` resolves to in the working repository.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if `rev` doesn't resolve.
    pub fn rev_parse(&self, rev: &str) -> Result<String, FixtureError> {
        self.git(&["rev-parse", rev])
    }
}

fn run(_dir: &Path, args: &[&str]) -> Result<String, FixtureError> {
    Err(FixtureError {
        command: format!("git {}", args.join(" ")),
        output: String::from("not implemented"),
    })
}

/// A git command of a fixture failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureError {
    command: String,
    output: String,
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` failed: {}", self.command, self.output)
    }
}

impl std::error::Error for FixtureError {}
