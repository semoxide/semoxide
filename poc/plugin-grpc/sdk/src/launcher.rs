//! Host side: spawn a plugin, own its lifetime and process tree, talk to it.
//! Shared by semoxide and the conformance kit.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tonic::transport::{Channel, Server};
use tonic::{Code, Request, Response, Status};

use crate::pb::host_git_server::{HostGit, HostGitServer};
use crate::pb::host_log_server::{HostLog, HostLogServer};
use crate::pb::plugin_client::PluginClient;
use crate::pb::{self, LogLevel, Step};
use crate::plugin::Host;
use crate::proctree::ProcessTree;
use crate::transport::{self, Io, Listener};
use crate::{PROTOCOL, version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

pub type OutputSink = Arc<dyn Fn(OutputStream, String) + Send + Sync>;

pub struct LaunchOptions {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Exact env of the plugin process (the host passes system vars only).
    pub env: BTreeMap<String, String>,
    pub connect_timeout: Duration,
    /// Receives every stdout/stderr line of the plugin (and its children).
    pub output: OutputSink,
    pub host: Arc<dyn Host>,
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("plugin exited before connecting: {0}")]
    ExitedEarly(std::process::ExitStatus),
    #[error("plugin did not connect within {0:?}")]
    ConnectTimeout(Duration),
    #[error("transport: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("handshake failed: {0}")]
    Handshake(Status),
    #[error("incompatible protocol: host {host}, plugin {plugin}")]
    Protocol { host: String, plugin: String },
}

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("step timed out after {0:?}; plugin process tree killed")]
    Timeout(Duration),
    #[error("plugin crashed ({exit}); process tree killed: {status}")]
    Crashed { exit: String, status: Status },
    #[error("plugin error: {0}")]
    Status(Status),
    #[error("plugin is dead")]
    Dead,
}

/// A running plugin process. Dropping it kills the whole process tree.
pub struct PluginProcess {
    pub handshake: pb::HandshakeResponse,
    /// Steps from the handshake the host knows; unknown values are ignored.
    pub steps: Vec<Step>,
    client: Option<PluginClient<Channel>>,
    child: Child,
    tree: ProcessTree,
    host_server: JoinHandle<()>,
    readers: Vec<JoinHandle<()>>,
    dead: bool,
}

pub async fn launch(opts: LaunchOptions) -> Result<PluginProcess, LaunchError> {
    let mut listener = Listener::bind_unique()?;
    let mut cmd = Command::new(&opts.program);
    cmd.arg("--socket")
        .arg(listener.addr())
        .args(&opts.args)
        .env_clear()
        .envs(&opts.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    ProcessTree::prepare(&mut cmd);
    let mut child = cmd.spawn()?;
    let tree = ProcessTree::attach(&child)?;

    let mut readers = Vec::new();
    for (stream, reader) in [
        (OutputStream::Stdout, child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>)),
        (OutputStream::Stderr, child.stderr.take().map(|s| Box::new(s) as _)),
    ] {
        let Some(reader) = reader else { continue };
        let sink = opts.output.clone();
        readers.push(tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                sink(stream, line);
            }
        }));
    }

    // Accept the two tagged connections (or notice an early exit).
    let mut plugin_conn = None;
    let mut host_conn = None;
    let accept_all = async {
        while plugin_conn.is_none() || host_conn.is_none() {
            let (tag, s) = listener.accept().await?;
            match tag {
                transport::TAG_PLUGIN => plugin_conn = Some(s),
                transport::TAG_HOST => host_conn = Some(s),
                _ => {} // unknown tag: drop the connection
            }
        }
        Ok::<_, std::io::Error>(())
    };
    tokio::select! {
        r = tokio::time::timeout(opts.connect_timeout, accept_all) => match r {
            Ok(r) => r?,
            Err(_) => { tree.kill(); return Err(LaunchError::ConnectTimeout(opts.connect_timeout)); }
        },
        status = child.wait() => { tree.kill(); return Err(LaunchError::ExitedEarly(status?)); }
    }
    drop(listener); // nobody else may connect from now on
    let (plugin_conn, host_conn) = (plugin_conn.unwrap(), host_conn.unwrap());

    let host_svc = HostServer(opts.host.clone());
    let host_server = tokio::spawn(async move {
        let incoming = tokio_stream::StreamExt::chain(
            tokio_stream::once(Ok::<_, std::io::Error>(Io::new(host_conn))),
            tokio_stream::pending(),
        );
        let _ = Server::builder()
            .add_service(HostLogServer::new(host_svc.clone()))
            .add_service(HostGitServer::new(host_svc))
            .serve_with_incoming(incoming)
            .await;
    });

    let mut client = PluginClient::new(transport::channel_over(plugin_conn).await?);
    let hs = client
        .handshake(pb::HandshakeRequest {
            protocol: Some(version(PROTOCOL)),
            host_name: "semoxide-poc".into(),
            host_version: env!("CARGO_PKG_VERSION").into(),
        })
        .await;
    let hs = match hs {
        Ok(r) => r.into_inner(),
        Err(s) => {
            tree.kill();
            return Err(if s.code() == Code::FailedPrecondition {
                LaunchError::Protocol { host: fmt_version(PROTOCOL), plugin: s.message().into() }
            } else {
                LaunchError::Handshake(s)
            });
        }
    };
    let pv = hs.protocol.unwrap_or_default();
    if pv.major != PROTOCOL.0 {
        tree.kill();
        return Err(LaunchError::Protocol {
            host: fmt_version(PROTOCOL),
            plugin: fmt_version((pv.major, pv.minor, pv.patch)),
        });
    }
    let steps = hs
        .steps
        .iter()
        .filter_map(|s| Step::try_from(*s).ok())
        .filter(|s| *s != Step::Unspecified)
        .collect();
    Ok(PluginProcess { handshake: hs, steps, client: Some(client), child, tree, host_server, readers, dead: false })
}

fn fmt_version(v: (u32, u32, u32)) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

impl PluginProcess {
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    fn client(&self) -> Result<PluginClient<Channel>, CallError> {
        match (&self.client, self.dead) {
            (Some(c), false) => Ok(c.clone()),
            _ => Err(CallError::Dead),
        }
    }

    pub async fn describe(&mut self) -> Result<String, CallError> {
        let r = self.client()?.describe(pb::DescribeRequest {}).await;
        self.check(r).await.map(|r| r.config_schema_json)
    }

    pub async fn configure(&mut self, config_json: String, secrets: BTreeMap<String, String>) -> Result<(), CallError> {
        let r = self.client()?.configure(pb::ConfigureRequest { config_json, secrets: secrets.into_iter().collect() }).await;
        self.check(r).await.map(|_| ())
    }

    /// Run a step with a deadline: sent as `grpc-timeout` (the plugin's
    /// server cancels the handler) AND enforced here (a hung plugin cannot
    /// ignore it). On expiry: kill the tree, fail.
    pub async fn run_step_raw(&mut self, step: i32, context_json: String, deadline: Duration) -> Result<String, CallError> {
        let mut req = Request::new(pb::StepRequest { step, context_json });
        req.set_timeout(deadline);
        let mut client = self.client()?;
        match tokio::time::timeout(deadline, client.run_step(req)).await {
            Err(_) => {
                self.kill().await;
                Err(CallError::Timeout(deadline))
            }
            // tonic's Channel enforces `set_timeout` client-side too and
            // reports it as CANCELLED "Timeout expired" (not DEADLINE_EXCEEDED).
            Ok(Err(s))
                if s.code() == Code::DeadlineExceeded
                    || (s.code() == Code::Cancelled && s.message().contains("Timeout expired")) =>
            {
                self.kill().await;
                Err(CallError::Timeout(deadline))
            }
            Ok(r) => self.check(r).await.map(|r| r.output_json),
        }
    }

    pub async fn run_step(&mut self, step: Step, context_json: String, deadline: Duration) -> Result<String, CallError> {
        self.run_step_raw(step as i32, context_json, deadline).await
    }

    /// Map transport failures to a crash: wait briefly for the exit status,
    /// then kill the remaining tree (grandchildren).
    async fn check<T>(&mut self, r: Result<Response<T>, Status>) -> Result<T, CallError> {
        match r {
            Ok(r) => Ok(r.into_inner()),
            Err(s) if is_transport_failure(&s) => {
                let exit = match tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await {
                    Ok(Ok(st)) => st.to_string(),
                    _ => "still running".into(),
                };
                self.kill().await;
                Err(CallError::Crashed { exit, status: s })
            }
            Err(s) => Err(CallError::Status(s)),
        }
    }

    /// Kill the whole process tree and reap the plugin.
    pub async fn kill(&mut self) {
        self.dead = true;
        self.client = None;
        self.tree.kill();
        let _ = self.child.wait().await;
        self.host_server.abort();
        // Readers end once every holder of the pipes (grandchildren) is gone.
        for r in self.readers.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(2), r).await;
        }
    }

    /// Polite stop: Shutdown RPC, wait for exit, then kill whatever is left.
    pub async fn shutdown(mut self, grace: Duration) -> Option<std::process::ExitStatus> {
        let mut status = None;
        if let Ok(mut c) = self.client() {
            let _ = tokio::time::timeout(grace, c.shutdown(pb::ShutdownRequest {})).await;
            self.client = None; // drop our end so the plugin's server can finish
            drop(c);
            if let Ok(Ok(st)) = tokio::time::timeout(grace, self.child.wait()).await {
                status = Some(st);
            }
        }
        self.kill().await;
        status
    }
}

fn is_transport_failure(s: &Status) -> bool {
    // Broken connection: Unavailable, or Unknown with a transport source.
    matches!(s.code(), Code::Unavailable | Code::Cancelled)
        || (s.code() == Code::Unknown && s.message().contains("transport"))
        || (s.code() == Code::Internal && s.message().contains("h2"))
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        self.host_server.abort();
        self.tree.kill();
    }
}

/// Adapter: host-side `Host` impl -> generated gRPC services.
#[derive(Clone)]
struct HostServer(Arc<dyn Host>);

#[tonic::async_trait]
impl HostLog for HostServer {
    async fn log(&self, req: Request<pb::LogRequest>) -> Result<Response<pb::LogResponse>, Status> {
        let r = req.into_inner();
        let level = LogLevel::try_from(r.level).unwrap_or(LogLevel::Info);
        self.0.log(level, r.message).await?;
        Ok(Response::new(pb::LogResponse {}))
    }
}

#[tonic::async_trait]
impl HostGit for HostServer {
    async fn add(&self, req: Request<pb::GitAddRequest>) -> Result<Response<pb::GitAddResponse>, Status> {
        self.0.git_add(req.into_inner().paths).await?;
        Ok(Response::new(pb::GitAddResponse {}))
    }
    async fn commit(&self, req: Request<pb::GitCommitRequest>) -> Result<Response<pb::GitCommitResponse>, Status> {
        let sha = self.0.git_commit(req.into_inner().message).await?;
        Ok(Response::new(pb::GitCommitResponse { sha }))
    }
    async fn push(&self, req: Request<pb::GitPushRequest>) -> Result<Response<pb::GitPushResponse>, Status> {
        self.0.git_push(req.into_inner().refspec).await?;
        Ok(Response::new(pb::GitPushResponse {}))
    }
}
