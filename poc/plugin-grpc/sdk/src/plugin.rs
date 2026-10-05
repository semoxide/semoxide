//! The `Plugin` trait (same for out-of-process and in-process plugins) and the
//! `Host` trait (host services a plugin can call during a step).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tonic::Status;

use crate::pb::{LogLevel, Step};

/// What a plugin declares in the handshake.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub steps: Vec<Step>,
    /// Secret env vars the plugin needs. Only these reach it.
    pub secret_env: Vec<String>,
}

/// Host services offered to a plugin during a step.
#[async_trait]
pub trait Host: Send + Sync {
    async fn log(&self, level: LogLevel, message: String) -> Result<(), Status>;
    async fn git_add(&self, paths: Vec<String>) -> Result<(), Status>;
    async fn git_commit(&self, message: String) -> Result<String, Status>;
    async fn git_push(&self, refspec: String) -> Result<(), Status>;
}

/// Per-step context handed to a plugin.
pub struct StepContext {
    pub step: Step,
    pub context: Value,
    pub host: Arc<dyn Host>,
    /// Remaining time, from the gRPC deadline (out-of-process) or the runner.
    pub deadline: Option<Duration>,
}

/// One plugin implementation, used both by `serve()` (binary) and in-process
/// by an embedder (no process, no socket).
#[async_trait]
pub trait Plugin: Send + Sync + 'static {
    fn manifest(&self) -> Manifest;
    /// JSON Schema of the plugin's config (`describe`).
    fn config_schema(&self) -> Value;
    async fn configure(&self, config: Value, env: PluginEnv) -> Result<(), Status>;
    async fn run_step(&self, ctx: StepContext) -> Result<Value, Status>;
}

/// The env a plugin may use for itself and its children: the process env
/// (system vars only, filtered by the host) plus the declared secrets
/// delivered over the channel in `Configure`.
#[derive(Debug, Clone, Default)]
pub struct PluginEnv {
    pub vars: BTreeMap<String, String>,
}

impl PluginEnv {
    /// Current process env (already system-only when spawned by the host) + secrets.
    pub fn from_process(secrets: BTreeMap<String, String>) -> Self {
        let mut vars: BTreeMap<String, String> = std::env::vars().collect();
        vars.extend(secrets);
        Self { vars }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        // Windows env names are case-insensitive.
        if cfg!(windows) {
            self.vars.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
        } else {
            self.vars.get(key).map(String::as_str)
        }
    }

    /// A child command with exactly this env. On Windows the program is
    /// resolved through PATH + PATHEXT, so `npm` finds `npm.cmd` (std only
    /// appends `.exe`).
    pub fn command(&self, program: &str) -> tokio::process::Command {
        let resolved = self.resolve(program).unwrap_or_else(|| program.into());
        let mut cmd = tokio::process::Command::new(resolved);
        cmd.env_clear().envs(&self.vars);
        cmd
    }

    pub fn resolve(&self, program: &str) -> Option<std::path::PathBuf> {
        let path = self.get("PATH")?;
        let exts: Vec<String> = if cfg!(windows) {
            self.get("PATHEXT")
                .unwrap_or(".COM;.EXE;.BAT;.CMD")
                .split(';')
                .filter(|e| !e.is_empty())
                .map(str::to_owned)
                .collect()
        } else {
            vec![String::new()]
        };
        for dir in std::env::split_paths(path) {
            for ext in &exts {
                let p = dir.join(format!("{program}{ext}"));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }
}

/// System vars a plugin always gets; everything else is dropped unless it is a
/// declared secret.
pub const SYSTEM_VARS: &[&str] = if cfg!(windows) {
    &[
        "PATH", "PATHEXT", "SystemRoot", "SystemDrive", "windir", "ComSpec", "TEMP", "TMP",
        "USERPROFILE", "APPDATA", "LOCALAPPDATA", "ProgramData", "ProgramFiles",
        "ProgramFiles(x86)", "ProgramW6432", "CommonProgramFiles", "HOMEDRIVE", "HOMEPATH",
        "USERNAME", "COMPUTERNAME", "NUMBER_OF_PROCESSORS", "PROCESSOR_ARCHITECTURE", "OS",
    ]
} else {
    &["PATH", "HOME", "USER", "LOGNAME", "LANG", "LC_ALL", "TMPDIR", "SHELL", "TERM", "TZ"]
};

/// Filter a host env down to [`SYSTEM_VARS`] (case-insensitive on Windows).
pub fn system_env(host_env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    host_env
        .iter()
        .filter(|(k, _)| {
            SYSTEM_VARS.iter().any(|s| if cfg!(windows) { s.eq_ignore_ascii_case(k) } else { s == k })
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Pick the declared secrets out of a host env.
pub fn declared_secrets(host_env: &BTreeMap<String, String>, declared: &[String]) -> BTreeMap<String, String> {
    declared
        .iter()
        .filter_map(|d| host_env.get(d).map(|v| (d.clone(), v.clone())))
        .collect()
}
