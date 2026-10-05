//! semoxide stand-in: host services (Log with masking, stub Git with the
//! release-tag rule), env policy, per-step deadlines, and one handle type for
//! out-of-process and in-process plugins.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use semoxide_plugin_sdk::launcher::{self, CallError, LaunchError, LaunchOptions, OutputStream, PluginProcess};
use semoxide_plugin_sdk::pb::{LogLevel, Step};
use semoxide_plugin_sdk::plugin::{declared_secrets, system_env};
use semoxide_plugin_sdk::{Host, Plugin, PluginEnv, Status, StepContext, async_trait};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub source: String,
    pub level: String,
    pub message: String,
}

#[derive(Default)]
struct State {
    logs: Vec<LogEntry>,
    secrets: Vec<String>,
    staged: Vec<String>,
    commits: Vec<String>,
    pushed: Vec<String>,
}

/// Host services shared by all plugins of a run.
#[derive(Default)]
pub struct HostServices {
    state: Mutex<State>,
    /// The only ref a plugin may push (set by the pipeline).
    pub release_tag: Mutex<Option<String>>,
}

impl HostServices {
    pub fn new(release_tag: &str) -> Arc<Self> {
        let s = Self::default();
        *s.release_tag.lock().unwrap() = Some(release_tag.into());
        Arc::new(s)
    }

    pub fn register_secret(&self, value: &str) {
        if !value.is_empty() {
            self.state.lock().unwrap().secrets.push(value.into());
        }
    }

    pub fn mask(&self, s: &str) -> String {
        let st = self.state.lock().unwrap();
        st.secrets.iter().fold(s.to_owned(), |acc, sec| acc.replace(sec.as_str(), "[secure]"))
    }

    pub fn push_log(&self, source: &str, level: &str, message: &str) {
        let message = self.mask(message);
        self.state.lock().unwrap().logs.push(LogEntry { source: source.into(), level: level.into(), message });
    }

    pub fn logs(&self) -> Vec<LogEntry> {
        self.state.lock().unwrap().logs.clone()
    }

    pub fn pushed(&self) -> Vec<String> {
        self.state.lock().unwrap().pushed.clone()
    }

    pub fn commits(&self) -> Vec<String> {
        self.state.lock().unwrap().commits.clone()
    }
}

#[async_trait]
impl Host for HostServices {
    async fn log(&self, level: LogLevel, message: String) -> Result<(), Status> {
        let lvl = level.as_str_name().trim_start_matches("LOG_LEVEL_").to_lowercase();
        self.push_log("plugin", &lvl, &message);
        Ok(())
    }
    async fn git_add(&self, paths: Vec<String>) -> Result<(), Status> {
        self.state.lock().unwrap().staged.extend(paths);
        Ok(())
    }
    async fn git_commit(&self, message: String) -> Result<String, Status> {
        let mut st = self.state.lock().unwrap();
        if st.staged.is_empty() {
            return Err(Status::failed_precondition("nothing staged"));
        }
        st.staged.clear();
        st.commits.push(message);
        Ok(format!("{:040x}", st.commits.len()))
    }
    async fn git_push(&self, refspec: String) -> Result<(), Status> {
        let tag = self.release_tag.lock().unwrap().clone().unwrap_or_default();
        let allowed = format!("refs/tags/{tag}");
        let mut st = self.state.lock().unwrap();
        if refspec != allowed {
            return Err(Status::permission_denied(format!("only {allowed} may be pushed, got {refspec}")));
        }
        if st.pushed.contains(&refspec) {
            return Err(Status::already_exists("existing tags are never moved"));
        }
        st.pushed.push(refspec);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error(transparent)]
    Launch(#[from] LaunchError),
    #[error(transparent)]
    Call(#[from] CallError),
    #[error("step timed out after {0:?}")]
    InProcessTimeout(Duration),
    #[error("plugin error: {0}")]
    Status(Status),
    #[error("step {0:?} not implemented by plugin")]
    NotImplemented(Step),
    #[error("bad output: {0}")]
    Output(String),
}

enum Kind {
    Process(PluginProcess),
    InProcess(Arc<dyn Plugin>),
}

/// One loaded plugin, either kind. The pipeline only sees this.
pub struct LoadedPlugin {
    pub name: String,
    pub steps: Vec<Step>,
    pub secret_env: Vec<String>,
    services: Arc<HostServices>,
    kind: Kind,
}

pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub config: Value,
}

impl LoadedPlugin {
    /// Spawn + connect + handshake + configure (secrets over the channel).
    pub async fn spawn(
        spec: SpawnSpec,
        host_env: &BTreeMap<String, String>,
        services: Arc<HostServices>,
    ) -> Result<Self, HostError> {
        let sink_services = services.clone();
        let output: launcher::OutputSink = Arc::new(move |stream, line| {
            let level = match stream {
                OutputStream::Stdout => "stdout",
                OutputStream::Stderr => "stderr",
            };
            sink_services.push_log("plugin-output", level, &line);
        });
        let mut p = launcher::launch(LaunchOptions {
            program: spec.program,
            args: spec.args,
            env: system_env(host_env),
            connect_timeout: Duration::from_secs(10),
            output,
            host: services.clone(),
        })
        .await?;
        let secret_env = p.handshake.secret_env.clone();
        let secrets = declared_secrets(host_env, &secret_env);
        for v in secrets.values() {
            services.register_secret(v);
        }
        p.configure(spec.config.to_string(), secrets).await?;
        Ok(Self {
            name: p.handshake.plugin_name.clone(),
            steps: p.steps.clone(),
            secret_env,
            services,
            kind: Kind::Process(p),
        })
    }

    /// Embedder path: same trait, no process, no socket.
    pub async fn in_process(
        plugin: impl Plugin,
        config: Value,
        host_env: &BTreeMap<String, String>,
        services: Arc<HostServices>,
    ) -> Result<Self, HostError> {
        let m = plugin.manifest();
        let secrets = declared_secrets(host_env, &m.secret_env);
        for v in secrets.values() {
            services.register_secret(v);
        }
        let mut vars = system_env(host_env);
        vars.extend(secrets);
        plugin.configure(config, PluginEnv { vars }).await.map_err(HostError::Status)?;
        Ok(Self { name: m.name, steps: m.steps, secret_env: m.secret_env, services, kind: Kind::InProcess(Arc::new(plugin)) })
    }

    pub fn pid(&self) -> Option<u32> {
        match &self.kind {
            Kind::Process(p) => p.pid(),
            Kind::InProcess(_) => None,
        }
    }

    pub async fn describe(&mut self) -> Result<Value, HostError> {
        let s = match &mut self.kind {
            Kind::Process(p) => p.describe().await?,
            Kind::InProcess(p) => return Ok(p.config_schema()),
        };
        serde_json::from_str(&s).map_err(|e| HostError::Output(e.to_string()))
    }

    pub async fn run_step(&mut self, step: Step, context: Value, deadline: Duration) -> Result<Value, HostError> {
        if !self.steps.contains(&step) {
            return Err(HostError::NotImplemented(step));
        }
        let out = match &mut self.kind {
            Kind::Process(p) => p.run_step(step, context.to_string(), deadline).await?,
            Kind::InProcess(p) => {
                let ctx = StepContext { step, context, host: self.services.clone(), deadline: Some(deadline) };
                // In-process: we can only drop the future, not kill anything.
                return match tokio::time::timeout(deadline, p.run_step(ctx)).await {
                    Err(_) => Err(HostError::InProcessTimeout(deadline)),
                    Ok(r) => r.map_err(HostError::Status),
                };
            }
        };
        serde_json::from_str(&out).map_err(|e| HostError::Output(e.to_string()))
    }

    pub async fn shutdown(self) -> Option<std::process::ExitStatus> {
        match self.kind {
            Kind::Process(p) => p.shutdown(Duration::from_secs(2)).await,
            Kind::InProcess(_) => None,
        }
    }
}

/// Default per-step deadlines (ADR 0010: e.g. verify 2 min, publish 30 min),
/// overridable per plugin and step.
pub fn default_deadline(step: Step) -> Duration {
    match step {
        Step::Publish | Step::AddChannel => Duration::from_secs(30 * 60),
        _ => Duration::from_secs(2 * 60),
    }
}

/// The host env as a map (production would use `std::env::vars()`).
pub fn current_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}
