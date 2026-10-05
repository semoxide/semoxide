//! `ProcessPlugin`: the `Plugin` trait over JSON-RPC 2.0 on a child's stdio.
//! One long-lived child per plugin per run.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::oneshot;

use crate::plugin::*;
use crate::protocol::{self, Incoming, InitializeParams, InitializeResult, LogParams, codes};

#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Config name; used as log scope until the handshake returns the plugin's own name.
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub config: Value,
    /// Per-request timeout (handshake included).
    pub timeout: Duration,
}

impl PluginCommand {
    pub fn new(name: &str, program: impl Into<PathBuf>) -> Self {
        PluginCommand {
            name: name.into(),
            program: program.into(),
            args: vec![],
            config: json!({}),
            timeout: Duration::from_secs(30),
        }
    }
    pub fn args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, a: I) -> Self {
        self.args = a.into_iter().map(Into::into).collect();
        self
    }
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
    pub fn config(mut self, c: Value) -> Self {
        self.config = c;
        self
    }
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, PluginError>>>>>;

pub struct ProcessPlugin {
    name: String,
    steps: StepSet,
    timeout: Duration,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    child: Arc<Mutex<Option<Child>>>,
    pending: Pending,
    /// Set once the child is unusable (exit, timeout kill); reason for the error.
    dead: Arc<Mutex<Option<String>>>,
    next_id: AtomicU64,
    logger: Logger,
}

impl ProcessPlugin {
    /// Spawn the child with exactly `env` as its environment, then handshake.
    pub async fn spawn(
        cmd: &PluginCommand,
        env: &BTreeMap<String, String>,
        cwd: &Path,
        logger: &Logger,
    ) -> Result<Self, PluginError> {
        let mut child = Command::new(&cmd.program)
            .args(&cmd.args)
            .env_clear()
            .envs(env)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| PluginError::Protocol(format!("spawn {}: {e}", cmd.program.display())))?;

        let logger = logger.scoped(&cmd.name);
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take().unwrap();
        let pending: Pending = Arc::default();
        let dead: Arc<Mutex<Option<String>>> = Arc::default();
        let child = Arc::new(Mutex::new(Some(child)));

        // stderr: never part of the protocol; every line becomes a debug log.
        let elog = logger.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                elog.debug(&format!("stderr: {}", line.trim_end()));
            }
        });

        // stdout: responses, `log` notifications; anything else is logged and dropped.
        let (olog, opend, odead, ochild) = (logger.clone(), pending.clone(), dead.clone(), child.clone());
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                route_line(line.trim_end(), &olog, &opend);
            }
            let reason = exit_reason(&ochild).await;
            mark_dead(&odead, &opend, &reason);
        });

        let mut p = ProcessPlugin {
            name: cmd.name.clone(),
            steps: StepSet::new(),
            timeout: cmd.timeout,
            stdin: tokio::sync::Mutex::new(Some(stdin)),
            child,
            pending,
            dead,
            next_id: AtomicU64::new(1),
            logger,
        };

        let params = InitializeParams {
            protocol: protocol::PROTOCOL_VERSION,
            host_version: env!("CARGO_PKG_VERSION"),
            plugin_config: &cmd.config,
        };
        let init: InitializeResult = match p.request("initialize", &params).await {
            Ok(v) => serde_json::from_value(v)
                .map_err(|e| PluginError::Protocol(format!("bad initialize result: {e}")))?,
            Err(e) => {
                p.kill();
                return Err(e);
            }
        };
        if !protocol::SUPPORTED_PROTOCOLS.contains(&init.protocol) {
            p.kill();
            return Err(PluginError::Protocol(format!(
                "plugin '{}' speaks protocol {}, host supports {:?}",
                init.name,
                init.protocol,
                protocol::SUPPORTED_PROTOCOLS
            )));
        }
        for s in &init.steps {
            match Step::from_method(s) {
                Some(st) => {
                    p.steps.insert(st);
                }
                None => p.logger.debug(&format!("ignoring unknown step '{s}'")),
            }
        }
        p.logger.debug(&format!("initialized '{}' steps={:?}", init.name, p.steps));
        Ok(p)
    }

    /// Send one request and await its response, bounded by the timeout.
    pub async fn request(&self, method: &str, params: &impl serde::Serialize) -> Result<Value, PluginError> {
        if let Some(r) = self.dead.lock().unwrap().clone() {
            return Err(PluginError::Exited(r));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let mut line = protocol::request(id, method, params);
        line.push('\n');
        {
            let mut guard = self.stdin.lock().await;
            let Some(stdin) = guard.as_mut() else {
                return Err(PluginError::Exited("stdin closed".into()));
            };
            // A dead child surfaces here as a broken pipe.
            if let Err(e) = async {
                stdin.write_all(line.as_bytes()).await?;
                stdin.flush().await
            }
            .await
            {
                self.pending.lock().unwrap().remove(&id);
                let reason = exit_reason(&self.child).await;
                return Err(PluginError::Exited(format!("write failed ({e}); {reason}")));
            }
        }
        match tokio::time::timeout(self.timeout, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(PluginError::Exited("response channel dropped".into())),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                self.kill();
                *self.dead.lock().unwrap() = Some(format!("killed after {method} timed out"));
                Err(PluginError::Timeout(self.timeout))
            }
        }
    }

    fn kill(&self) {
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.start_kill();
        }
    }

    async fn call<T: DeserializeOwned>(&self, step: Step, ctx: &Context, code: &'static str) -> Result<T, PluginError> {
        let v = self.request(step.method(), &json!({ "context": ctx })).await?;
        serde_json::from_value(v.clone()).map_err(|e| PluginError::InvalidOutput {
            code,
            detail: format!("{v} ({e})"),
        })
    }
}

impl Drop for ProcessPlugin {
    // The reader task holds a clone of the child, so tokio's kill_on_drop would never fire.
    fn drop(&mut self) {
        self.kill();
    }
}

fn route_line(line: &str, log: &Logger, pending: &Pending) {
    if line.is_empty() {
        return;
    }
    let msg: Incoming = match serde_json::from_str(line) {
        Ok(m) => m,
        Err(_) => {
            log.warn(&format!("non-protocol stdout ignored: {line}"));
            return;
        }
    };
    if msg.jsonrpc != "2.0" {
        log.warn(&format!("non-protocol stdout ignored: {line}"));
        return;
    }
    match (msg.id, msg.method.as_deref()) {
        (None, Some("log")) => match msg.params.map(serde_json::from_value::<LogParams>) {
            Some(Ok(p)) => log.log(Level::parse(&p.level), &p.message),
            _ => log.warn("malformed log notification"),
        },
        (None, Some(m)) => log.debug(&format!("ignoring notification '{m}'")),
        (Some(id), None) => {
            let Some(tx) = pending.lock().unwrap().remove(&id) else {
                log.warn(&format!("response for unknown id {id} ignored"));
                return;
            };
            let r = match msg.error {
                None => Ok(msg.result.unwrap_or(Value::Null)),
                Some(e) if e.code == codes::METHOD_NOT_FOUND => Err(PluginError::NotImplemented),
                Some(e) => {
                    let code = e.data.as_ref().and_then(|d| d.get("code")).and_then(|c| c.as_str());
                    match code {
                        Some(c) => Err(PluginError::Release { code: c.into(), message: e.message }),
                        None => Err(PluginError::Protocol(format!("rpc error {}: {}", e.code, e.message))),
                    }
                }
            };
            let _ = tx.send(r);
        }
        (Some(_), Some(m)) => log.warn(&format!("plugin->host requests unsupported ('{m}')")),
        (None, None) => log.warn(&format!("non-protocol stdout ignored: {line}")),
    }
}

async fn exit_reason(child: &Arc<Mutex<Option<Child>>>) -> String {
    // stdout EOF usually precedes the exit status by a few ms.
    for _ in 0..50 {
        if let Some(c) = child.lock().unwrap().as_mut()
            && let Ok(Some(st)) = c.try_wait() {
                return format!("plugin exited ({st})");
            }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    "plugin closed stdout".into()
}

fn mark_dead(dead: &Mutex<Option<String>>, pending: &Pending, reason: &str) {
    dead.lock().unwrap().get_or_insert_with(|| reason.to_string());
    for (_, tx) in pending.lock().unwrap().drain() {
        let _ = tx.send(Err(PluginError::Exited(reason.to_string())));
    }
}

#[async_trait]
impl Plugin for ProcessPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn steps(&self) -> StepSet {
        self.steps.clone()
    }
    async fn verify_conditions(&self, ctx: &Context) -> Result<(), PluginError> {
        self.call::<Value>(Step::VerifyConditions, ctx, "EVERIFYCONDITIONSOUTPUT").await.map(|_| ())
    }
    async fn analyze_commits(&self, ctx: &Context) -> Result<Option<ReleaseType>, PluginError> {
        self.call(Step::AnalyzeCommits, ctx, "EANALYZECOMMITSOUTPUT").await
    }
    async fn generate_notes(&self, ctx: &Context) -> Result<Option<String>, PluginError> {
        self.call(Step::GenerateNotes, ctx, "EGENERATENOTESOUTPUT").await
    }
    async fn publish(&self, ctx: &Context) -> Result<Option<PublishResult>, PluginError> {
        // `false` is a valid "did not publish" answer.
        let v: Value = self.call(Step::Publish, ctx, "EPUBLISHOUTPUT").await?;
        match v {
            Value::Null | Value::Bool(false) => Ok(None),
            Value::Object(_) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| PluginError::InvalidOutput { code: "EPUBLISHOUTPUT", detail: format!("{v} ({e})") }),
            other => Err(PluginError::InvalidOutput { code: "EPUBLISHOUTPUT", detail: other.to_string() }),
        }
    }
    async fn shutdown(&self) {
        // Polite: `shutdown` notification + close stdin; then kill after a grace period.
        if let Some(mut stdin) = self.stdin.lock().await.take() {
            let line = format!("{}\n", protocol::notification("shutdown", &json!({})));
            let _ = stdin.write_all(line.as_bytes()).await;
            drop(stdin);
        }
        let child = self.child.lock().unwrap().take();
        if let Some(mut c) = child
            && tokio::time::timeout(Duration::from_secs(2), c.wait()).await.is_err() {
                self.logger.warn("plugin ignored shutdown; killing");
                let _ = c.kill().await;
            }
    }
}
