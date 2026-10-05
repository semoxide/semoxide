//! Plugin side: `serve()` connects to the host and serves the `Plugin` service.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

use crate::pb::host_git_client::HostGitClient;
use crate::pb::host_log_client::HostLogClient;
use crate::pb::plugin_server::{self, PluginServer};
use crate::pb::{self, LogLevel, Step};
use crate::plugin::{Host, Plugin, PluginEnv, StepContext};
use crate::transport::{self, Io};
use crate::{PROTOCOL, version};

/// Test hooks for misbehaving plugins (conformance and host tests only).
#[derive(Debug, Clone, Default)]
pub struct ServeOptions {
    /// Advertise this protocol version instead of the real one.
    pub advertise_protocol: Option<(u32, u32, u32)>,
    /// Extra raw step numbers to advertise (simulate a newer plugin).
    pub extra_raw_steps: Vec<i32>,
}

/// Parse `--socket <addr>` from argv and serve until `Shutdown` or until the
/// host closes the connection.
pub async fn serve<P: Plugin>(plugin: P) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    serve_with(plugin, ServeOptions::default()).await
}

pub fn socket_arg() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--socket" {
            return args.next();
        }
    }
    None
}

pub async fn serve_with<P: Plugin>(
    plugin: P,
    opts: ServeOptions,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = socket_arg().ok_or("missing --socket <addr>")?;
    let plugin_conn = transport::dial(&addr, transport::TAG_PLUGIN).await?;
    let host_conn = transport::dial(&addr, transport::TAG_HOST).await?;
    let host_channel = transport::channel_over(host_conn).await?;
    let host = Arc::new(GrpcHost::new(host_channel));

    // Shutdown on the Shutdown RPC or when the host's connection drops.
    let (stop_tx, mut stop_rx) = mpsc::channel::<()>(4);
    let svc = Adapter { plugin: Arc::new(plugin), host, stop: stop_tx.clone(), opts };
    let incoming = tokio_stream::once(Ok::<_, std::io::Error>(Io::notify_on_drop(plugin_conn, stop_tx)))
        .chain(tokio_stream::pending());
    let signal = async move {
        stop_rx.recv().await;
    };
    // Graceful: in-flight responses (incl. Shutdown's own) are still sent.
    Server::builder()
        .add_service(PluginServer::new(svc))
        .serve_with_incoming_shutdown(incoming, signal)
        .await?;
    Ok(())
}

/// `Host` implemented as gRPC calls to the host services connection.
pub struct GrpcHost {
    log: HostLogClient<Channel>,
    git: HostGitClient<Channel>,
}

impl GrpcHost {
    pub fn new(ch: Channel) -> Self {
        Self { log: HostLogClient::new(ch.clone()), git: HostGitClient::new(ch) }
    }
}

#[async_trait]
impl Host for GrpcHost {
    async fn log(&self, level: LogLevel, message: String) -> Result<(), Status> {
        self.log.clone().log(pb::LogRequest { level: level as i32, message }).await.map(|_| ())
    }
    async fn git_add(&self, paths: Vec<String>) -> Result<(), Status> {
        self.git.clone().add(pb::GitAddRequest { paths }).await.map(|_| ())
    }
    async fn git_commit(&self, message: String) -> Result<String, Status> {
        Ok(self.git.clone().commit(pb::GitCommitRequest { message }).await?.into_inner().sha)
    }
    async fn git_push(&self, refspec: String) -> Result<(), Status> {
        self.git.clone().push(pb::GitPushRequest { refspec }).await.map(|_| ())
    }
}

struct Adapter<P> {
    plugin: Arc<P>,
    host: Arc<GrpcHost>,
    stop: mpsc::Sender<()>,
    opts: ServeOptions,
}

/// Parse the `grpc-timeout` header (e.g. `1500m`) into a Duration.
fn grpc_timeout<T>(req: &Request<T>) -> Option<Duration> {
    let v = req.metadata().get("grpc-timeout")?.to_str().ok()?;
    let (num, unit) = v.split_at(v.len().checked_sub(1)?);
    let n: u64 = num.parse().ok()?;
    Some(match unit {
        "H" => Duration::from_secs(n * 3600),
        "M" => Duration::from_secs(n * 60),
        "S" => Duration::from_secs(n),
        "m" => Duration::from_millis(n),
        "u" => Duration::from_micros(n),
        "n" => Duration::from_nanos(n),
        _ => return None,
    })
}

#[tonic::async_trait]
impl<P: Plugin> plugin_server::Plugin for Adapter<P> {
    async fn handshake(&self, req: Request<pb::HandshakeRequest>) -> Result<Response<pb::HandshakeResponse>, Status> {
        let req = req.into_inner();
        let host_major = req.protocol.map(|p| p.major).unwrap_or(0);
        if host_major != PROTOCOL.0 {
            return Err(Status::failed_precondition(format!(
                "protocol major mismatch: host {host_major}, plugin {}",
                PROTOCOL.0
            )));
        }
        let m = self.plugin.manifest();
        let mut steps: Vec<i32> = m.steps.iter().map(|s| *s as i32).collect();
        steps.extend(&self.opts.extra_raw_steps);
        Ok(Response::new(pb::HandshakeResponse {
            protocol: Some(version(self.opts.advertise_protocol.unwrap_or(PROTOCOL))),
            plugin_name: m.name,
            plugin_version: m.version,
            steps,
            secret_env: m.secret_env,
        }))
    }

    async fn describe(&self, _: Request<pb::DescribeRequest>) -> Result<Response<pb::DescribeResponse>, Status> {
        Ok(Response::new(pb::DescribeResponse { config_schema_json: self.plugin.config_schema().to_string() }))
    }

    async fn configure(&self, req: Request<pb::ConfigureRequest>) -> Result<Response<pb::ConfigureResponse>, Status> {
        let req = req.into_inner();
        let config = if req.config_json.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&req.config_json).map_err(|e| Status::invalid_argument(format!("config: {e}")))?
        };
        let declared = self.plugin.manifest().secret_env;
        // Accept only what was declared, even if the host sends more.
        let secrets: BTreeMap<String, String> =
            req.secrets.into_iter().filter(|(k, _)| declared.contains(k)).collect();
        self.plugin.configure(config, PluginEnv::from_process(secrets)).await?;
        Ok(Response::new(pb::ConfigureResponse {}))
    }

    async fn run_step(&self, req: Request<pb::StepRequest>) -> Result<Response<pb::StepResponse>, Status> {
        let deadline = grpc_timeout(&req);
        let req = req.into_inner();
        let step = Step::try_from(req.step)
            .ok()
            .filter(|s| *s != Step::Unspecified)
            .ok_or_else(|| Status::invalid_argument(format!("unknown step {}", req.step)))?;
        if !self.plugin.manifest().steps.contains(&step) {
            return Err(Status::unimplemented(format!("step {} not implemented", step.as_str_name())));
        }
        let context = if req.context_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&req.context_json).map_err(|e| Status::invalid_argument(format!("context: {e}")))?
        };
        let ctx = StepContext { step, context, host: self.host.clone(), deadline };
        let out = self.plugin.run_step(ctx).await?;
        Ok(Response::new(pb::StepResponse { output_json: out.to_string() }))
    }

    async fn shutdown(&self, _: Request<pb::ShutdownRequest>) -> Result<Response<pb::ShutdownResponse>, Status> {
        let _ = self.stop.try_send(());
        Ok(Response::new(pb::ShutdownResponse {}))
    }
}
