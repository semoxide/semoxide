//! Simplified git2 smart subtransport over russh (from poc/git2-russh/src: bridge, native, sshconfig, url).
//! Enough to compile in the real code path: ssh config, known_hosts, agent + key-file auth, exec, pump.

use std::io::{self, Read, Write};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use git2::transport::{Service, SmartSubtransport, SmartSubtransportStream, Transport};
use russh::client::{self, AuthResult, Handle};
use russh::keys::agent::AgentIdentity;
use russh::keys::agent::client::AgentClient;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::{ChannelMsg, ChannelWriteHalf, Disconnect};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc;

const TIMEOUT: Duration = Duration::from_secs(30);

static RT: LazyLock<Runtime> =
    LazyLock::new(|| Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("tokio runtime"));

fn block_on<F: Future>(f: F) -> Result<F::Output, io::Error> {
    let _ctx = RT.enter();
    futures::executor::block_on(tokio::time::timeout(TIMEOUT, f))
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "ssh timeout"))
}

fn err(e: impl std::fmt::Display) -> git2::Error {
    git2::Error::from_str(&e.to_string())
}

// ---- URL + ssh config ----

struct Target {
    host: String,
    port: u16,
    user: String,
    path: String,
    identity_files: Vec<std::path::PathBuf>,
}

fn resolve(url: &str) -> Result<Target, git2::Error> {
    let (user, host, port, path) = if let Some(rest) = url.split_once("://").map(|x| x.1) {
        let (auth, path) = rest.split_once('/').ok_or_else(|| err("ssh url without path"))?;
        let (user, hp) = auth.rsplit_once('@').map(|(u, h)| (Some(u.to_string()), h)).unwrap_or((None, auth));
        let (host, port) = hp.rsplit_once(':').map(|(h, p)| (h, p.parse().ok())).unwrap_or((hp, None));
        (user, host.to_string(), port, format!("/{path}"))
    } else {
        let (auth, path) = url.split_once(':').ok_or_else(|| err("bad scp-style url"))?;
        let (user, host) = auth.rsplit_once('@').map(|(u, h)| (Some(u.to_string()), h)).unwrap_or((None, auth));
        (user, host.to_string(), None, path.to_string())
    };
    let params = std::env::home_dir()
        .map(|h| h.join(".ssh").join("config"))
        .filter(|p| p.is_file())
        .and_then(|p| std::fs::File::open(p).ok())
        .and_then(|f| {
            ssh2_config::SshConfig::default()
                .parse(&mut io::BufReader::new(f), ssh2_config::ParseRule::ALLOW_UNKNOWN_FIELDS)
                .ok()
        })
        .map(|c| c.query(&host));
    Ok(Target {
        host: params.as_ref().and_then(|p| p.host_name.clone()).unwrap_or(host),
        port: port.or(params.as_ref().and_then(|p| p.port)).unwrap_or(22),
        user: user.or(params.as_ref().and_then(|p| p.user.clone())).unwrap_or_else(|| "git".into()),
        identity_files: params.and_then(|p| p.identity_file).unwrap_or_default(),
        path,
    })
}

// ---- russh client ----

struct Client {
    host: String,
    port: u16,
}

impl client::Handler for Client {
    type Error = russh::Error;
    async fn check_server_key(&mut self, k: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        match k {
            PublicKeyOrCertificate::PublicKey { key, .. } => {
                Ok(russh::keys::check_known_hosts(&self.host, self.port, key).unwrap_or(false))
            }
            PublicKeyOrCertificate::Certificate(_) => Ok(false),
        }
    }
}

type Agent = AgentClient<Box<dyn russh::keys::agent::client::AgentStream + Send + Unpin>>;

async fn agent() -> Option<Agent> {
    #[cfg(windows)]
    {
        let p = std::env::var("SSH_AUTH_SOCK").unwrap_or_else(|_| r"\\.\pipe\openssh-ssh-agent".into());
        AgentClient::connect_named_pipe(&p).await.ok().map(|a| a.dynamic())
    }
    #[cfg(unix)]
    {
        AgentClient::connect_uds(std::env::var("SSH_AUTH_SOCK").ok()?).await.ok().map(|a| a.dynamic())
    }
}

async fn rsa_hash(h: &Handle<Client>, key: &PublicKey) -> Option<HashAlg> {
    if !key.algorithm().is_rsa() {
        return None;
    }
    h.best_supported_rsa_hash().await.ok().flatten().unwrap_or(Some(HashAlg::Sha256))
}

async fn authenticate(h: &mut Handle<Client>, t: &Target) -> Result<(), String> {
    if let Some(mut a) = agent().await {
        for id in a.request_identities().await.unwrap_or_default() {
            let AgentIdentity::PublicKey { key, .. } = id else { continue };
            let hash = rsa_hash(h, &key).await;
            if let Ok(AuthResult::Success) = h.authenticate_publickey_with(t.user.clone(), key, hash, &mut a).await {
                return Ok(());
            }
        }
    }
    let mut files = t.identity_files.clone();
    if let Some(home) = std::env::home_dir() {
        files.extend(["id_ed25519", "id_ecdsa", "id_rsa"].map(|n| home.join(".ssh").join(n)));
    }
    for f in files.iter().filter(|f| f.is_file()) {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let Ok(key) = russh::keys::decode_secret_key(&text, None) else { continue };
        let hash = rsa_hash(h, key.public_key()).await;
        if let Ok(AuthResult::Success) =
            h.authenticate_publickey(&t.user, PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await
        {
            return Ok(());
        }
    }
    Err(format!("{}@{}:{}: authentication failed", t.user, t.host, t.port))
}

type ExitState = Arc<Mutex<(Option<u32>, Vec<u8>)>>;

struct Stream {
    rx: mpsc::Receiver<Bytes>,
    buf: Bytes,
    exit: ExitState,
    write: Option<ChannelWriteHalf<client::Msg>>,
    handle: Option<Handle<Client>>,
}

async fn open(t: Target, cmd: String) -> Result<Stream, String> {
    let tcp = tokio::net::TcpStream::connect((t.host.as_str(), t.port)).await.map_err(|e| e.to_string())?;
    let cfg = Arc::new(client::Config { nodelay: true, ..Default::default() });
    let mut h = client::connect_stream(cfg, tcp, Client { host: t.host.clone(), port: t.port })
        .await
        .map_err(|e| format!("handshake: {e}"))?;
    authenticate(&mut h, &t).await?;
    let ch = h.channel_open_session().await.map_err(|e| e.to_string())?;
    ch.exec(true, cmd.as_bytes()).await.map_err(|e| e.to_string())?;
    let (mut read, write) = ch.split();
    let (tx, rx) = mpsc::channel::<Bytes>(32);
    let exit: ExitState = Arc::new(Mutex::new((None, Vec::new())));
    let st = exit.clone();
    tokio::spawn(async move {
        while let Some(msg) = read.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    if tx.send(data).await.is_err() {
                        break;
                    }
                }
                ChannelMsg::ExtendedData { data, ext: 1 } => st.lock().unwrap().1.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => st.lock().unwrap().0 = Some(exit_status),
                ChannelMsg::Close | ChannelMsg::Failure => break,
                _ => {}
            }
        }
    });
    Ok(Stream { rx, buf: Bytes::new(), exit, write: Some(write), handle: Some(h) })
}

impl Read for Stream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.buf.is_empty() {
            match block_on(self.rx.recv())? {
                Some(b) => self.buf = b,
                None => {
                    let st = self.exit.lock().unwrap();
                    return match st.0 {
                        Some(c) if c != 0 => Err(io::Error::other(format!(
                            "remote git exited {c}: {}",
                            String::from_utf8_lossy(&st.1)
                        ))),
                        _ => Ok(0),
                    };
                }
            }
        }
        let n = out.len().min(self.buf.len());
        out[..n].copy_from_slice(&self.buf.split_to(n));
        Ok(n)
    }
}

impl Write for Stream {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let w = self.write.as_ref().ok_or_else(|| io::Error::other("closed"))?;
        block_on(w.data_bytes(Bytes::copy_from_slice(data)))?.map_err(io::Error::other)?;
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        let (w, h) = (self.write.take(), self.handle.take());
        RT.spawn(async move {
            if let Some(w) = w {
                let _ = w.eof().await;
            }
            if let Some(h) = h {
                let _ = h.disconnect(Disconnect::ByApplication, "", "en").await;
            }
        });
    }
}

// ---- git2 registration ----

struct Sub;

impl SmartSubtransport for Sub {
    fn action(&self, url: &str, action: Service) -> Result<Box<dyn SmartSubtransportStream>, git2::Error> {
        let service = match action {
            Service::UploadPackLs | Service::UploadPack => "git-upload-pack",
            Service::ReceivePackLs | Service::ReceivePack => "git-receive-pack",
        };
        let t = resolve(url)?;
        let cmd = format!("{service} '{}'", t.path.replace('\'', r"'\''"));
        let s = block_on(open(t, cmd)).map_err(err)?.map_err(err)?;
        Ok(Box::new(s))
    }
    fn close(&self) -> Result<(), git2::Error> {
        Ok(())
    }
}

static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();

pub fn register() -> Result<(), git2::Error> {
    REGISTERED
        .get_or_init(|| {
            for prefix in ["ssh", "ssh+git", "git+ssh"] {
                // SAFETY: once per prefix, before any transport lookup; the factory is 'static + Send + Sync.
                unsafe { git2::transport::register(prefix, |remote| Transport::smart(remote, false, Sub)) }
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
        .clone()
        .map_err(|e| git2::Error::from_str(&e))
}
