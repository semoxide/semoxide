//! Option f: russh session → `exec git-upload-pack '<path>'` → channel bytes piped to libgit2.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use russh::client::{self, AuthResult, Handle};
use russh::keys::agent::AgentIdentity;
use russh::keys::agent::client::AgentClient;
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::{ChannelMsg, ChannelWriteHalf, Disconnect, Preferred};
use tokio::sync::mpsc;

use crate::known_hosts::{self, Entry, Verdict};
use crate::sshconfig::{self, Resolved};
use crate::{AuthSource, KnownHosts, SshError, SshOptions, bridge, url};

type Phase = Arc<Mutex<&'static str>>;

struct Client {
    host: String,
    port: u16,
    entries: Arc<Vec<Entry>>,
    accept_unknown: bool,
    verdict: Arc<Mutex<Option<SshError>>>,
    phase: Phase,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(&mut self, k: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        *self.phase.lock().unwrap() = "verifying the host key";
        let addr = format!("{}:{}", self.host, self.port);
        let key = match k {
            PublicKeyOrCertificate::PublicKey { key, .. } => key,
            PublicKeyOrCertificate::Certificate(_) => {
                *self.verdict.lock().unwrap() =
                    Some(SshError::Protocol { addr, detail: "host certificates are not supported".into() });
                return Ok(false);
            }
        };
        let desc = known_hosts::describe(key);
        let err = match known_hosts::verify(&self.entries, &self.host, self.port, key) {
            Verdict::Trusted => None,
            Verdict::Unknown if self.accept_unknown => None,
            Verdict::Unknown => Some(SshError::UnknownHost { addr, key: desc }),
            Verdict::Changed { origin } => Some(SshError::HostKeyChanged { addr, key: desc, origin }),
            Verdict::Revoked { origin } => Some(SshError::HostKeyRevoked { addr, key: desc, origin }),
        };
        let ok = err.is_none();
        *self.verdict.lock().unwrap() = err;
        *self.phase.lock().unwrap() = "exchanging keys";
        Ok(ok)
    }
}

fn load_known_hosts(src: &[KnownHosts]) -> Result<Vec<Entry>, SshError> {
    let mut out = Vec::new();
    let defaults;
    let src = if src.is_empty() {
        let mut d = Vec::new();
        if let Some(h) = sshconfig::home_dir() {
            d.push(KnownHosts::File(h.join(".ssh").join("known_hosts")));
        }
        #[cfg(unix)]
        d.push(KnownHosts::File("/etc/ssh/ssh_known_hosts".into()));
        defaults = d;
        &defaults[..]
    } else {
        src
    };
    for s in src {
        match s {
            KnownHosts::File(p) => out.extend(
                known_hosts::load_file(p).map_err(|e| SshError::Config(format!("known_hosts {}: {e}", p.display())))?,
            ),
            KnownHosts::Text { label, text } => out.extend(known_hosts::parse(text, label)),
            KnownHosts::GitHub => out.extend(known_hosts::parse(known_hosts::GITHUB_KNOWN_HOSTS, "github-meta")),
        }
    }
    Ok(out)
}

fn default_auth(r: &Resolved) -> Vec<AuthSource> {
    let mut v = vec![AuthSource::Agent];
    let mut files: Vec<PathBuf> = r.identity_files.clone();
    if let Some(h) = sshconfig::home_dir() {
        for n in ["id_ed25519", "id_ecdsa", "id_rsa"] {
            files.push(h.join(".ssh").join(n));
        }
    }
    for f in files {
        if f.is_file() {
            v.push(AuthSource::KeyFile { path: f, passphrase: None });
        }
    }
    v
}

pub(crate) struct NativeStream {
    rx: mpsc::Receiver<Bytes>,
    buf: Bytes,
    got_data: bool,
    remote: Arc<Mutex<RemoteState>>,
    write: Option<ChannelWriteHalf<client::Msg>>,
    handle: Option<Handle<Client>>,
    io_timeout: Duration,
    addr: String,
}

#[derive(Default)]
struct RemoteState {
    stderr: Vec<u8>,
    exit: Option<u32>,
    exec_refused: bool,
}

pub(crate) fn open(raw_url: &str, service: &str, opts: &SshOptions) -> Result<NativeStream, SshError> {
    let u = url::parse(raw_url)?;
    let r = sshconfig::resolve(&u, &opts.ssh_config)?;
    let cmd = url::remote_command(service, &u.path);
    let entries = Arc::new(load_known_hosts(&opts.known_hosts)?);
    let auth = if opts.auth.is_empty() {
        let mut a = default_auth(&r);
        // ssh config IdentityFile also applies when the caller passed nothing else
        a.dedup_by(|a, b| matches!((a, b), (AuthSource::Agent, AuthSource::Agent)));
        a
    } else {
        let mut a = opts.auth.clone();
        a.extend(r.identity_files.iter().filter(|p| p.is_file()).map(|p| AuthSource::KeyFile {
            path: p.clone(),
            passphrase: None,
        }));
        a
    };
    let addr = format!("{}:{}", r.host, r.port);
    let phase: Phase = Arc::new(Mutex::new("connecting (TCP)"));
    let fut = connect(r, cmd, entries, auth, opts.accept_unknown_hosts, phase.clone());
    match bridge::block_on_timeout(opts.connect_timeout, fut) {
        Ok(Ok((handle, write, rx, remote))) => Ok(NativeStream {
            rx,
            buf: Bytes::new(),
            got_data: false,
            remote,
            write: Some(write),
            handle: Some(handle),
            io_timeout: opts.io_timeout,
            addr,
        }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(SshError::Timeout {
            phase: phase.lock().unwrap().to_string(),
            addr,
            secs: opts.connect_timeout.as_secs(),
        }),
    }
}

type Connected = (Handle<Client>, ChannelWriteHalf<client::Msg>, mpsc::Receiver<Bytes>, Arc<Mutex<RemoteState>>);

async fn connect(
    r: Resolved,
    cmd: String,
    entries: Arc<Vec<Entry>>,
    auth: Vec<AuthSource>,
    accept_unknown: bool,
    phase: Phase,
) -> Result<Connected, SshError> {
    let addr = format!("{}:{}", r.host, r.port);
    let verdict = Arc::new(Mutex::new(None));
    let mut cfg = client::Config { nodelay: true, ..Default::default() };
    let algs = known_hosts::algorithms_for(&entries, &r.host, r.port);
    if !algs.is_empty() && !accept_unknown {
        cfg.preferred = Preferred { key: algs.into(), ..Preferred::default() };
    }
    let handler = Client {
        host: r.host.clone(),
        port: r.port,
        entries,
        accept_unknown,
        verdict: verdict.clone(),
        phase: phase.clone(),
    };
    let tcp = tokio::net::TcpStream::connect((r.host.as_str(), r.port))
        .await
        .map_err(|e| SshError::Connect { addr: addr.clone(), detail: e.to_string() })?;
    let _ = tcp.set_nodelay(true);
    *phase.lock().unwrap() = "exchanging keys";
    let mut h = match client::connect_stream(Arc::new(cfg), tcp, handler).await {
        Ok(h) => h,
        Err(e) => {
            return Err(verdict
                .lock()
                .unwrap()
                .take()
                .unwrap_or(SshError::Protocol { addr, detail: format!("handshake failed: {e}") }));
        }
    };
    *phase.lock().unwrap() = "authenticating";
    authenticate(&mut h, &r, auth).await?;

    *phase.lock().unwrap() = "opening the session channel";
    let proto = |e: russh::Error| SshError::Protocol { addr: addr.clone(), detail: e.to_string() };
    let ch = h.channel_open_session().await.map_err(proto)?;
    ch.exec(true, cmd.as_bytes()).await.map_err(proto)?;
    let (mut read, write) = ch.split();
    let (tx, rx) = mpsc::channel::<Bytes>(32);
    let remote = Arc::new(Mutex::new(RemoteState::default()));
    let st = remote.clone();
    // Pump: channel messages -> bounded queue (backpressure), stderr and exit status -> shared state.
    tokio::spawn(async move {
        while let Some(msg) = read.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    if tx.send(data).await.is_err() {
                        break;
                    }
                }
                ChannelMsg::ExtendedData { data, ext: 1 } => st.lock().unwrap().stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => st.lock().unwrap().exit = Some(exit_status),
                ChannelMsg::Failure => {
                    st.lock().unwrap().exec_refused = true;
                    break;
                }
                ChannelMsg::Close => break,
                _ => {}
            }
        }
    });
    Ok((h, write, rx, remote))
}

async fn connect_agent(src: &AuthSource) -> Result<(String, AgentClient<Box<dyn russh::keys::agent::client::AgentStream + Send + Unpin>>), String> {
    let at = |p: String| async move {
        #[cfg(windows)]
        {
            if p.starts_with(r"\\.\pipe\") || p.starts_with("//./pipe/") {
                return AgentClient::connect_named_pipe(&p).await.map(|a| (p, a.dynamic())).map_err(|e| e.to_string());
            }
            Err(format!("{p}: not a named pipe (Unix-socket agents are not reachable from Windows)"))
        }
        #[cfg(unix)]
        {
            AgentClient::connect_uds(&p).await.map(|a| (p, a.dynamic())).map_err(|e| e.to_string())
        }
    };
    match src {
        AuthSource::AgentAt(p) => at(p.clone()).await,
        AuthSource::Agent => match std::env::var("SSH_AUTH_SOCK") {
            Ok(p) if !p.is_empty() => {
                let first = at(p.clone()).await;
                #[cfg(windows)]
                if first.is_err() {
                    return at(r"\\.\pipe\openssh-ssh-agent".into()).await;
                }
                first
            }
            _ => {
                #[cfg(windows)]
                {
                    at(r"\\.\pipe\openssh-ssh-agent".into()).await
                }
                #[cfg(unix)]
                {
                    Err("SSH_AUTH_SOCK is not set".into())
                }
            }
        },
        AuthSource::Pageant => {
            #[cfg(windows)]
            {
                AgentClient::connect_pageant().await.map(|a| ("pageant".into(), a.dynamic())).map_err(|e| e.to_string())
            }
            #[cfg(unix)]
            {
                Err("Pageant exists only on Windows".into())
            }
        }
        _ => unreachable!(),
    }
}

async fn rsa_hash(h: &Handle<Client>, key: &PublicKey) -> Option<HashAlg> {
    if !key.algorithm().is_rsa() {
        return None;
    }
    match h.best_supported_rsa_hash().await {
        Ok(Some(alg)) => alg,
        _ => Some(HashAlg::Sha256), // no server-sig-algs: rsa-sha2-256 is the safe modern choice
    }
}

async fn authenticate(h: &mut Handle<Client>, r: &Resolved, sources: Vec<AuthSource>) -> Result<(), SshError> {
    let user = r.user.clone();
    let target = format!("{user}@{}:{}", r.host, r.port);
    let mut tried = Vec::new();
    for src in sources {
        match &src {
            AuthSource::Agent | AuthSource::AgentAt(_) | AuthSource::Pageant => {
                let (name, mut agent) = match connect_agent(&src).await {
                    Ok(a) => a,
                    Err(e) => {
                        tried.push(format!("agent: unavailable ({e})"));
                        continue;
                    }
                };
                let ids = match agent.request_identities().await {
                    Ok(ids) => ids,
                    Err(e) => {
                        tried.push(format!("agent {name}: {e}"));
                        continue;
                    }
                };
                if ids.is_empty() {
                    tried.push(format!("agent {name}: no keys"));
                }
                for id in ids {
                    let AgentIdentity::PublicKey { key, .. } = id else { continue };
                    let hash = rsa_hash(h, &key).await;
                    let desc = known_hosts::describe(&key);
                    match h.authenticate_publickey_with(user.clone(), key, hash, &mut agent).await {
                        Ok(AuthResult::Success) => return Ok(()),
                        Ok(AuthResult::Failure { .. }) => tried.push(format!("agent key {desc}: rejected by server")),
                        Err(e) => tried.push(format!("agent key {desc}: {e}")),
                    }
                }
            }
            AuthSource::KeyFile { path, passphrase } => {
                let label = format!("key file {}", path.display());
                match std::fs::read_to_string(path) {
                    Ok(text) => {
                        if try_key(h, &user, &label, &text, passphrase.as_deref(), &mut tried).await {
                            return Ok(());
                        }
                    }
                    Err(e) => tried.push(format!("{label}: cannot read ({e})")),
                }
            }
            AuthSource::KeyMemory { label, text, passphrase } => {
                let label = format!("key {label}");
                if try_key(h, &user, &label, text, passphrase.as_deref(), &mut tried).await {
                    return Ok(());
                }
            }
        }
    }
    Err(SshError::Auth { target, attempts: tried })
}

async fn try_key(
    h: &mut Handle<Client>,
    user: &str,
    label: &str,
    text: &str,
    pass: Option<&str>,
    tried: &mut Vec<String>,
) -> bool {
    let key: PrivateKey = match russh::keys::decode_secret_key(text, pass) {
        Ok(k) => k,
        Err(e) => {
            tried.push(format!("{label}: cannot parse ({e})"));
            return false;
        }
    };
    let desc = known_hosts::describe(key.public_key());
    let hash = rsa_hash(h, key.public_key()).await;
    match h.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await {
        Ok(AuthResult::Success) => return true,
        Ok(AuthResult::Failure { .. }) => tried.push(format!("{label} ({desc}): rejected by server")),
        Err(e) => tried.push(format!("{label} ({desc}): {e}")),
    }
    false
}

impl NativeStream {
    fn eof(&self) -> io::Result<usize> {
        let st = self.remote.lock().unwrap();
        let stderr = String::from_utf8_lossy(&st.stderr).trim().replace('\n', " | ");
        if st.exec_refused {
            return Err(io::Error::other(format!("{}: server refused to run the git command", self.addr)));
        }
        match st.exit {
            Some(c) if c != 0 => Err(io::Error::other(format!("{}: remote git exited with {c}: {stderr}", self.addr))),
            _ if !self.got_data && !stderr.is_empty() => {
                Err(io::Error::other(format!("{}: remote: {stderr}", self.addr)))
            }
            _ => Ok(0),
        }
    }
}

impl Read for NativeStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.buf.is_empty() {
            match bridge::block_on_timeout(self.io_timeout, self.rx.recv()) {
                Ok(Some(b)) => {
                    self.got_data = true;
                    self.buf = b;
                }
                Ok(None) => return self.eof(),
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("{}: no data for {} s", self.addr, self.io_timeout.as_secs()),
                    ));
                }
            }
        }
        let n = out.len().min(self.buf.len());
        out[..n].copy_from_slice(&self.buf.split_to(n));
        Ok(n)
    }
}

impl Write for NativeStream {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let w = self.write.as_ref().ok_or_else(|| io::Error::other("stream closed"))?;
        match bridge::block_on_timeout(self.io_timeout, w.data_bytes(Bytes::copy_from_slice(data))) {
            Ok(Ok(())) => Ok(data.len()),
            Ok(Err(e)) => Err(io::Error::new(io::ErrorKind::BrokenPipe, format!("{}: {e}", self.addr))),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{}: write blocked for {} s", self.addr, self.io_timeout.as_secs()),
            )),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for NativeStream {
    fn drop(&mut self) {
        let (w, h) = (self.write.take(), self.handle.take());
        // Close in the background; never block git2's free path.
        bridge::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_secs(5), async move {
                if let Some(w) = w {
                    let _ = w.eof().await;
                    let _ = w.close().await;
                }
                if let Some(h) = h {
                    let _ = h.disconnect(Disconnect::ByApplication, "", "en").await;
                }
            })
            .await;
        });
    }
}
