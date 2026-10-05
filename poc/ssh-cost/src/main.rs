//! Marginal SSH cost on top of the product baseline (tokio + tonic/prost + tracing + git2 https).
//! Usage: ssh-cost [git-url]  (default: an HTTPS ls-remote; with feature `russh`/`libssh2`, ssh URLs work too)

use std::pin::Pin;

use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::info;

pub mod pb {
    tonic::include_proto!("stub.v1");
}
use pb::plugin_client::PluginClient;
use pb::plugin_server::{Plugin, PluginServer};
use pb::{Req, Resp};

#[cfg(feature = "russh")]
mod russh_transport;

#[derive(Default)]
struct Svc;

#[tonic::async_trait]
impl Plugin for Svc {
    async fn call(&self, r: Request<Req>) -> Result<Response<Resp>, Status> {
        let r = r.into_inner();
        // tokio::process: the product shells out to tools.
        let out = tokio::process::Command::new(&r.name).args(&r.args).output().await.map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Resp { out: String::from_utf8_lossy(&out.stdout).into(), code: out.status.code().unwrap_or(-1) as u32 }))
    }
    type WatchStream = Pin<Box<dyn Stream<Item = Result<Resp, Status>> + Send>>;
    async fn watch(&self, r: Request<Req>) -> Result<Response<Self::WatchStream>, Status> {
        let n = r.into_inner().args.len() as u32;
        let s = tokio_stream::iter((0..n).map(|i| Ok(Resp { out: format!("tick {i}"), code: i })));
        Ok(Response::new(Box::pin(s)))
    }
}

fn ls_remote(url: &str) -> Result<Vec<String>, git2::Error> {
    let repo = git2::Repository::init_bare(std::env::temp_dir().join("ssh-cost-bare"))?;
    let mut r = repo.remote_anonymous(url)?;
    let mut cb = git2::RemoteCallbacks::new();
    #[cfg(feature = "libssh2")]
    cb.credentials(|_, u, _| git2::Cred::ssh_key_from_agent(u.unwrap_or("git")));
    cb.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificatePassthrough));
    let c = r.connect_auth(git2::Direction::Fetch, Some(cb), None)?;
    Ok(c.list()?.iter().map(|h| format!("{} {}", h.oid(), h.name())).collect())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();
    #[cfg(feature = "russh")]
    russh_transport::register()?;

    // gRPC server + client over a local socket (loopback).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    tokio::spawn(tonic::transport::Server::builder().add_service(PluginServer::new(Svc)).serve_with_incoming(incoming));
    let mut client = PluginClient::connect(format!("http://{addr}")).await?;
    let r = client.call(Req { name: "git".into(), args: vec!["--version".into()] }).await?.into_inner();
    info!(out = %r.out.trim(), code = r.code, "plugin call");
    let mut s = client.watch(Req { name: "w".into(), args: vec!["a".into(), "b".into()] }).await?.into_inner();
    while let Some(m) = s.message().await? {
        info!(out = %m.out, "watch");
    }

    // git2 ls-remote on a blocking thread, as the product would.
    let url = std::env::args().nth(1).unwrap_or_else(|| "https://github.com/rust-lang/git2-rs".into());
    let refs = tokio::task::spawn_blocking(move || ls_remote(&url)).await??;
    for r in refs.iter().take(5) {
        println!("{r}");
    }
    println!("{} refs", refs.len());
    Ok(())
}
