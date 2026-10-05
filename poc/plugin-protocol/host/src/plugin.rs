//! The `Plugin` trait, the step context, logging and secret masking.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::protocol::Step;

pub type StepSet = BTreeSet<Step>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseType {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub hash: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRelease {
    pub version: String,
    pub git_tag: String,
    pub git_head: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextRelease {
    #[serde(rename = "type")]
    pub kind: ReleaseType,
    pub version: String,
    pub git_tag: String,
    pub notes: String,
}

/// What `publish` may return (semantic-release: plain object or false/empty).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PublishResult {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
}

/// Per-step context. Serialized as-is into every process-plugin request,
/// except `env` and `logger`, which never cross the wire.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub cwd: PathBuf,
    /// The run's env (given by the embedder, not necessarily the process env).
    /// Built-ins read secrets here; process plugins get it as their process env.
    #[serde(skip)]
    pub env: BTreeMap<String, String>,
    pub branch: Branch,
    pub commits: Vec<Commit>,
    pub last_release: Option<LastRelease>,
    pub next_release: Option<NextRelease>,
    pub options: Value,
    #[serde(skip)]
    pub logger: Logger,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Equivalent of semantic-release's `SemanticReleaseError(message, code)`.
    #[error("{code}: {message}")]
    Release { code: String, message: String },
    #[error("step not implemented")]
    NotImplemented,
    #[error("invalid output ({code}): {detail}")]
    InvalidOutput { code: &'static str, detail: String },
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("plugin process gone: {0}")]
    Exited(String),
    #[error("step timed out after {0:?}")]
    Timeout(Duration),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// One method per step; a default body means "not implemented".
/// `steps()` must list exactly the overridden ones; the pipeline only calls those.
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn steps(&self) -> StepSet;

    async fn verify_conditions(&self, _ctx: &Context) -> Result<(), PluginError> {
        Err(PluginError::NotImplemented)
    }
    async fn analyze_commits(&self, _ctx: &Context) -> Result<Option<ReleaseType>, PluginError> {
        Err(PluginError::NotImplemented)
    }
    async fn generate_notes(&self, _ctx: &Context) -> Result<Option<String>, PluginError> {
        Err(PluginError::NotImplemented)
    }
    async fn publish(&self, _ctx: &Context) -> Result<Option<PublishResult>, PluginError> {
        Err(PluginError::NotImplemented)
    }
    /// End of run. Process plugins stop their child here.
    async fn shutdown(&self) {}
}

// ---------------------------------------------------------------- logging

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn parse(s: &str) -> Level {
        match s {
            "debug" => Level::Debug,
            "warn" => Level::Warn,
            "error" => Level::Error,
            _ => Level::Info,
        }
    }
}

pub trait LogSink: Send + Sync {
    fn write(&self, level: Level, scope: &str, msg: &str);
}

pub struct StderrSink {
    pub debug: bool,
}

impl LogSink for StderrSink {
    fn write(&self, level: Level, scope: &str, msg: &str) {
        if level == Level::Debug && !self.debug {
            return;
        }
        eprintln!("[semoxide] [{scope}] {level:?}: {msg}");
    }
}

/// Collects lines; used by tests.
#[derive(Default)]
pub struct MemorySink(pub Mutex<Vec<String>>);

impl LogSink for MemorySink {
    fn write(&self, level: Level, scope: &str, msg: &str) {
        self.0.lock().unwrap().push(format!("[{scope}] {level:?}: {msg}"));
    }
}

impl MemorySink {
    pub fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

/// Masks every message before it reaches the sink, whatever its origin
/// (built-in call, `log` notification, plugin stderr, garbage stdout).
#[derive(Clone)]
pub struct Logger {
    sink: Arc<dyn LogSink>,
    masker: Arc<Masker>,
    scope: String,
}

impl Logger {
    pub fn new(sink: Arc<dyn LogSink>, masker: Arc<Masker>) -> Self {
        Logger { sink, masker, scope: "core".into() }
    }
    pub fn scoped(&self, scope: &str) -> Logger {
        Logger { scope: scope.into(), ..self.clone() }
    }
    pub fn log(&self, level: Level, msg: &str) {
        self.sink.write(level, &self.scope, &self.masker.mask(msg));
    }
    pub fn info(&self, msg: &str) {
        self.log(Level::Info, msg)
    }
    pub fn warn(&self, msg: &str) {
        self.log(Level::Warn, msg)
    }
    pub fn debug(&self, msg: &str) {
        self.log(Level::Debug, msg)
    }
}

/// semantic-release rule: values of env vars whose name matches
/// /token|password|credential|secret|private|key|auth|webhook/i, length >= 5,
/// GOPRIVATE excluded; raw and URL-encoded forms become `[secure]`.
pub struct Masker {
    secrets: Vec<String>,
}

impl Masker {
    const NEEDLES: [&str; 8] = ["token", "password", "credential", "secret", "private", "key", "auth", "webhook"];

    pub fn from_env(env: &BTreeMap<String, String>) -> Self {
        let mut secrets = Vec::new();
        for (k, v) in env {
            let lk = k.to_ascii_lowercase();
            if k == "GOPRIVATE" || v.len() < 5 || !Self::NEEDLES.iter().any(|n| lk.contains(n)) {
                continue;
            }
            secrets.push(v.clone());
            let enc = url_encode(v);
            if enc != *v {
                secrets.push(enc);
            }
        }
        // Longest first so a secret containing another is masked whole.
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        secrets.dedup();
        Masker { secrets }
    }

    pub fn mask(&self, msg: &str) -> String {
        let mut out = msg.to_string();
        for s in &self.secrets {
            if out.contains(s.as_str()) {
                out = out.replace(s.as_str(), "[secure]");
            }
        }
        out
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_raw_and_url_encoded_secrets_only_for_matching_names() {
        let env: BTreeMap<String, String> = [
            ("GH_TOKEN", "abc/def+ghi"),
            ("NPM_AUTH", "abc"), // too short
            ("GOPRIVATE", "github.com/private"),
            ("HOME", "C:/Users/someone"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let m = Masker::from_env(&env);
        assert_eq!(m.mask("t=abc/def+ghi"), "t=[secure]");
        assert_eq!(m.mask("u=https://x:abc%2Fdef%2Bghi@h"), "u=https://x:[secure]@h");
        assert_eq!(m.mask("abc github.com/private C:/Users/someone"), "abc github.com/private C:/Users/someone");
    }
}
