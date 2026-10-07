//! Git histories for tests, written with the real git CLI (TESTING: fixtures).
//!
//! Every commit, tag and note gets a fixed identity and a date derived from its step, and git
//! runs without the machine's config, so the same fixture gives the same SHAs everywhere.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        std::fs::write(dir.path().join(GLOBAL_CONFIG), GLOBAL_CONFIG_TEXT).map_err(|error| {
            FixtureError {
                command: format!("write {GLOBAL_CONFIG}"),
                output: error.to_string(),
            }
        })?;
        let mut fixture = GitFixture { dir, step: 0 };
        let root = fixture.dir.path().to_path_buf();
        fixture.run(&root, &["init", "-b", "main", REPO])?;
        fixture.run(&root, &["init", "--bare", "-b", "main", REMOTE])?;
        fixture.git(&["remote", "add", "origin", &fixture.remote_url()])?;
        for step in &self.steps {
            fixture.run_step(step)?;
            fixture.step += 1;
        }
        Ok(fixture)
    }
}

const REPO: &str = "repo";
const REMOTE: &str = "remote.git";
const GLOBAL_CONFIG: &str = "gitconfig";
/// Replaces the user's global config; the system config is off (`GIT_CONFIG_NOSYSTEM`).
const GLOBAL_CONFIG_TEXT: &str =
    "[commit]\n\tgpgsign = false\n[tag]\n\tgpgsign = false\n[core]\n\tautocrlf = false\n";
const NAME: &str = "semoxide-test";
const EMAIL: &str = "test@example.invalid";
/// 2026-01-01T00:00:00Z; step `n` is `n` minutes later.
const FIRST_TIME: u64 = 1_767_225_600;
const STEP_SECONDS: u64 = 60;
const PUSHED_REFS: [&str; 3] = [
    "refs/heads/*:refs/heads/*",
    "refs/tags/*:refs/tags/*",
    "refs/notes/*:refs/notes/*",
];

/// A built history: a working repository on `main` plus a bare `origin` remote, deleted on drop.
#[derive(Debug)]
pub struct GitFixture {
    dir: TempDir,
    /// The current step; sets the date of everything git writes.
    step: u64,
}

impl GitFixture {
    /// Starts an empty history on `main`.
    pub fn builder() -> GitFixtureBuilder {
        GitFixtureBuilder::default()
    }

    /// The working repository.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.dir.path().join(REPO)
    }

    /// The bare remote, configured as `origin`.
    #[must_use]
    pub fn remote_path(&self) -> PathBuf {
        self.dir.path().join(REMOTE)
    }

    /// The remote's `file://` URL.
    #[must_use]
    pub fn remote_url(&self) -> String {
        let path = self.remote_path().to_string_lossy().replace('\\', "/");
        // A Windows path (`C:/…`) lacks the leading slash a Unix path has.
        if path.starts_with('/') {
            format!("file://{path}")
        } else {
            format!("file:///{path}")
        }
    }

    /// Runs git in the working repository with the fixture's isolated environment; returns
    /// stdout without the trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if git fails.
    pub fn git(&self, args: &[&str]) -> Result<String, FixtureError> {
        self.run(&self.path(), args)
    }

    /// Like [`Self::git`], in the bare remote.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if git fails.
    pub fn remote_git(&self, args: &[&str]) -> Result<String, FixtureError> {
        self.run(&self.remote_path(), args)
    }

    /// The SHA `rev` resolves to in the working repository.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError`] if `rev` doesn't resolve.
    pub fn rev_parse(&self, rev: &str) -> Result<String, FixtureError> {
        self.git(&["rev-parse", rev])
    }

    fn run_step(&self, step: &Step) -> Result<String, FixtureError> {
        match step {
            Step::Commit(message) => self.git(&["commit", "--allow-empty", "-m", message]),
            Step::Tag(name) => self.git(&["tag", name]),
            Step::AnnotatedTag(name, message) => self.git(&["tag", "-a", name, "-m", message]),
            Step::Branch(name) => self.git(&["checkout", "-b", name]),
            Step::Checkout(rev) => self.git(&["checkout", rev]),
            Step::Merge(branch, Merge::FastForward) => self.git(&["merge", "--ff-only", branch]),
            Step::Merge(branch, Merge::NoFastForward) => self.git(&["merge", "--no-ff", branch]),
            Step::Rebase(onto) => self.git(&["rebase", onto]),
            Step::Note(rev, notes_ref, text) => {
                self.git(&["notes", "--ref", notes_ref, "add", "-m", text, rev])
            }
            Step::Push => {
                let mut args = vec!["push", "origin"];
                args.extend(PUSHED_REFS);
                self.git(&args)
            }
        }
    }

    fn run(&self, dir: &Path, args: &[&str]) -> Result<String, FixtureError> {
        let command = format!("git {}", args.join(" "));
        let output = isolated_git(self.dir.path(), self.step)
            .current_dir(dir)
            .args(args)
            .output()
            .map_err(|error| FixtureError {
                command: command.clone(),
                output: error.to_string(),
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FixtureError {
                command,
                output: format!("{}{}", stderr.trim_end(), stdout.trim_end()),
            });
        }
        Ok(stdout.trim_end_matches(['\r', '\n']).to_owned())
    }
}

/// `git` with only `PATH` kept from the process env, plus the fixture's config, identity and
/// the date of `step`.
#[expect(
    clippy::disallowed_methods,
    reason = "fixtures write history with the real git CLI, isolated from the process env (TESTING)"
)]
fn isolated_git(root: &Path, step: u64) -> Command {
    let date = format!("{} +0000", FIRST_TIME + step * STEP_SECONDS);
    let mut git = Command::new("git");
    git.env_clear()
        .env("HOME", root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", root.join(GLOBAL_CONFIG))
        .env("GIT_AUTHOR_NAME", NAME)
        .env("GIT_AUTHOR_EMAIL", EMAIL)
        .env("GIT_COMMITTER_NAME", NAME)
        .env("GIT_COMMITTER_EMAIL", EMAIL)
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .env("GIT_MERGE_AUTOEDIT", "no")
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(path) = std::env::var_os("PATH") {
        git.env("PATH", path);
    }
    git
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
