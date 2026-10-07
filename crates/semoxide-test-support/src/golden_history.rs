//! Golden histories: a git history, its config and the expected release, one TOML file each
//! under `tests/histories/` (TESTING: fixtures).

use std::fmt;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

use crate::git_fixture::{GitFixture, GitFixtureBuilder, Merge};

const CONFIG_FILE: &str = "semoxide.toml";

/// One `tests/histories/*.toml` file.
#[derive(Debug)]
pub struct GoldenHistory {
    path: PathBuf,
    description: String,
    steps: Vec<Step>,
    config: Option<Table>,
    expected: Expected,
}

/// What semoxide should decide for a history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    /// A release.
    Release(ExpectedRelease),
    /// No release, with the `NoReleaseReason` in snake case (`no_relevant_commits`).
    NoRelease(String),
}

/// The expected release of a history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedRelease {
    version: String,
    release_type: ReleaseType,
    channel: Option<String>,
}

impl ExpectedRelease {
    /// The next version, e.g. `2.0.0-beta.1`.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The level the commits asked for.
    #[must_use]
    pub fn release_type(&self) -> ReleaseType {
        self.release_type
    }

    /// The channel, or `None` for the branch's default channel.
    #[must_use]
    pub fn channel(&self) -> Option<&str> {
        self.channel.as_deref()
    }
}

/// The level of a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseType {
    /// `major`
    Major,
    /// `minor`
    Minor,
    /// `patch`
    Patch,
}

/// A step as written in the file; each maps to one [`GitFixtureBuilder`] method.
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

impl Step {
    fn apply(&self, builder: GitFixtureBuilder) -> GitFixtureBuilder {
        match self {
            Self::Commit(message) => builder.commit(message),
            Self::Tag(name) => builder.tag(name),
            Self::AnnotatedTag(name, message) => builder.annotated_tag(name, message),
            Self::Branch(name) => builder.branch(name),
            Self::Checkout(rev) => builder.checkout(rev),
            Self::Merge(branch, how) => builder.merge(branch, *how),
            Self::Rebase(onto) => builder.rebase(onto),
            Self::Note(rev, notes_ref, text) => builder.note(rev, notes_ref, text),
            Self::Push => builder.push(),
        }
    }
}

impl GoldenHistory {
    /// Reads and parses one history file.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] if the file can't be read or isn't a valid history.
    pub fn load(path: &Path) -> Result<Self, HistoryError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| HistoryError::new(path, format!("can't be read: {error}")))?;
        Self::parse(path, &text)
    }

    /// Every `*.toml` history in `dir`, sorted by file name.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] for the first file that can't be loaded.
    pub fn all(dir: &Path) -> Result<Vec<Self>, HistoryError> {
        let unreadable =
            |error: std::io::Error| HistoryError::new(dir, format!("can't be read: {error}"));
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(unreadable)? {
            let path = entry.map_err(unreadable)?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                paths.push(path);
            }
        }
        paths.sort();
        paths.iter().map(|path| Self::load(path)).collect()
    }

    /// Parses `text` as the history file at `path` (used in error messages).
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] if `text` isn't a valid history.
    pub fn parse(path: &Path, text: &str) -> Result<Self, HistoryError> {
        parse_history(path, text).map_err(|message| HistoryError::new(path, message))
    }

    /// The file the history was loaded from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One line on what the history shows.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The `[config]` table, written as `semoxide.toml` by [`Self::build`]; `None` = defaults.
    #[must_use]
    pub fn config(&self) -> Option<&Table> {
        self.config.as_ref()
    }

    /// What semoxide should decide.
    #[must_use]
    pub fn expected(&self) -> &Expected {
        &self.expected
    }

    /// Runs the steps with [`GitFixture`] and writes `[config]` as an untracked `semoxide.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError`] if a step fails or the config can't be written.
    pub fn build(&self) -> Result<GitFixture, HistoryError> {
        let fixture = self
            .steps
            .iter()
            .fold(GitFixture::builder(), |builder, step| step.apply(builder))
            .build()
            .map_err(|error| HistoryError::new(&self.path, error.to_string()))?;
        if let Some(config) = &self.config {
            let text = toml::to_string(config).map_err(|error| {
                HistoryError::new(&self.path, format!("`config` can't be written: {error}"))
            })?;
            std::fs::write(fixture.path().join(CONFIG_FILE), text).map_err(|error| {
                HistoryError::new(
                    &self.path,
                    format!("`{CONFIG_FILE}` can't be written: {error}"),
                )
            })?;
        }
        Ok(fixture)
    }
}

fn parse_history(path: &Path, text: &str) -> Result<GoldenHistory, String> {
    let mut table: Table = text
        .parse()
        .map_err(|error: toml::de::Error| format!("invalid TOML: {}", error.message()))?;
    let description = take_string(&mut table, "description", "")?
        .ok_or_else(|| String::from("missing `description`"))?;
    let steps = match table.remove("steps") {
        Some(Value::Array(steps)) => parse_steps(steps)?,
        Some(_) => return Err(String::from("`steps` must be an array")),
        None => return Err(String::from("missing `steps`")),
    };
    let config = match table.remove("config") {
        Some(Value::Table(config)) => Some(config),
        Some(_) => return Err(String::from("`config` must be a table")),
        None => None,
    };
    let expected = match table.remove("expected") {
        Some(Value::Table(expected)) => parse_expected(expected)?,
        Some(_) => return Err(String::from("`expected` must be a table")),
        None => return Err(String::from("missing `expected`")),
    };
    reject_unknown(&table, "")?;
    Ok(GoldenHistory {
        path: path.to_path_buf(),
        description,
        steps,
        config,
        expected,
    })
}

fn parse_expected(mut table: Table) -> Result<Expected, String> {
    const PREFIX: &str = "expected.";
    let version = take_string(&mut table, "version", PREFIX)?;
    let no_release = take_string(&mut table, "no_release", PREFIX)?;
    let release_type = take_string(&mut table, "release_type", PREFIX)?;
    let channel = take_string(&mut table, "channel", PREFIX)?;
    reject_unknown(&table, PREFIX)?;
    match (version, no_release) {
        (Some(_), Some(_)) => Err(String::from(
            "`expected` has both `version` and `no_release`",
        )),
        (None, None) => Err(String::from("`expected` needs `version` or `no_release`")),
        (None, Some(_)) if release_type.is_some() || channel.is_some() => Err(String::from(
            "`expected.release_type` and `expected.channel` need `version`",
        )),
        (None, Some(reason)) => Ok(Expected::NoRelease(reason)),
        (Some(version), None) => {
            let release_type =
                release_type.ok_or_else(|| String::from("missing `expected.release_type`"))?;
            Ok(Expected::Release(ExpectedRelease {
                version,
                release_type: parse_release_type(&release_type)?,
                channel,
            }))
        }
    }
}

fn parse_release_type(text: &str) -> Result<ReleaseType, String> {
    match text {
        "major" => Ok(ReleaseType::Major),
        "minor" => Ok(ReleaseType::Minor),
        "patch" => Ok(ReleaseType::Patch),
        _ => Err(format!(
            "unknown release type `{text}`: expected `major`, `minor` or `patch`"
        )),
    }
}

fn parse_steps(steps: Vec<Value>) -> Result<Vec<Step>, String> {
    steps
        .into_iter()
        .enumerate()
        .map(|(index, step)| {
            parse_step(step).map_err(|message| format!("step {}: {message}", index + 1))
        })
        .collect()
}

fn parse_step(step: Value) -> Result<Step, String> {
    let mut table = match step {
        Value::String(name) if name == "push" => return Ok(Step::Push),
        Value::String(name) => return Err(format!("unknown step `{name}`")),
        Value::Table(table) => table,
        _ => return Err(String::from("expected a table or `\"push\"`")),
    };
    let Some(kind) = STEP_KINDS.iter().find(|kind| table.contains_key(**kind)) else {
        let name = table.keys().next().map_or("", String::as_str);
        return Err(format!("unknown step `{name}`"));
    };
    let mut arg = |key: &str| -> Result<String, String> {
        match table.remove(key) {
            Some(Value::String(value)) => Ok(value),
            Some(_) => Err(format!("`{key}` must be a string")),
            None => Err(format!("`{kind}` needs `{key}`")),
        }
    };
    let parsed = match *kind {
        "commit" => Step::Commit(arg("commit")?),
        "tag" => Step::Tag(arg("tag")?),
        "annotated_tag" => Step::AnnotatedTag(arg("annotated_tag")?, arg("message")?),
        "branch" => Step::Branch(arg("branch")?),
        "checkout" => Step::Checkout(arg("checkout")?),
        "merge" => Step::Merge(arg("merge")?, parse_merge_mode(&arg("mode")?)?),
        "rebase" => Step::Rebase(arg("rebase")?),
        _ => Step::Note(arg("note")?, arg("ref")?, arg("text")?),
    };
    if let Some(key) = table.keys().next() {
        return Err(format!("unknown key `{key}` for `{kind}`"));
    }
    Ok(parsed)
}

const STEP_KINDS: [&str; 8] = [
    "commit",
    "tag",
    "annotated_tag",
    "branch",
    "checkout",
    "merge",
    "rebase",
    "note",
];

fn parse_merge_mode(text: &str) -> Result<Merge, String> {
    match text {
        "ff" => Ok(Merge::FastForward),
        "no-ff" => Ok(Merge::NoFastForward),
        _ => Err(format!(
            "unknown merge mode `{text}`: expected `ff` or `no-ff`"
        )),
    }
}

fn take_string(table: &mut Table, key: &str, prefix: &str) -> Result<Option<String>, String> {
    match table.remove(key) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(format!("`{prefix}{key}` must be a string")),
        None => Ok(None),
    }
}

fn reject_unknown(table: &Table, prefix: &str) -> Result<(), String> {
    match table.keys().next() {
        Some(key) => Err(format!("unknown key `{prefix}{key}`")),
        None => Ok(()),
    }
}

/// A history file that can't be loaded or built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryError {
    path: PathBuf,
    message: String,
}

impl HistoryError {
    fn new(path: &Path, message: impl Into<String>) -> Self {
        Self {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for HistoryError {}
