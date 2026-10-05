use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

/// Which SSH implementation the transport uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Pure-Rust `russh` client (option f). The default.
    Native,
    /// Spawn the system `ssh` (option b1): OpenSSH's own config, agent, known_hosts, `GIT_SSH_COMMAND`.
    Exec,
}

/// Env switch for the backend: `SEMOXIDE_SSH_BACKEND=exec` (or `native`).
pub const BACKEND_ENV: &str = "SEMOXIDE_SSH_BACKEND";

impl Backend {
    pub fn from_env() -> Self {
        match std::env::var(BACKEND_ENV).as_deref() {
            Ok("exec") | Ok("ssh") => Backend::Exec,
            _ => Backend::Native,
        }
    }
}

/// One way to authenticate. Sources are tried in order; every key of an agent is tried.
#[derive(Clone)]
pub enum AuthSource {
    /// `SSH_AUTH_SOCK`; on Windows, if unset, the OpenSSH agent pipe `\\.\pipe\openssh-ssh-agent`.
    Agent,
    /// A specific agent: a Unix socket path, or a Windows named pipe (`\\.\pipe\...`).
    AgentAt(String),
    /// PuTTY's Pageant (Windows only).
    Pageant,
    KeyFile { path: PathBuf, passphrase: Option<String> },
    /// Key text (OpenSSH, PEM PKCS#1/PKCS#8, or PuTTY .ppk). `label` is used in messages; the key never is.
    KeyMemory { label: String, text: String, passphrase: Option<String> },
}

impl fmt::Debug for AuthSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthSource::Agent => write!(f, "Agent"),
            AuthSource::AgentAt(p) => write!(f, "AgentAt({p})"),
            AuthSource::Pageant => write!(f, "Pageant"),
            AuthSource::KeyFile { path, .. } => write!(f, "KeyFile({})", path.display()),
            AuthSource::KeyMemory { label, .. } => write!(f, "KeyMemory({label}, <redacted>)"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum KnownHosts {
    File(PathBuf),
    Text { label: String, text: String },
    /// GitHub's published keys for github.com and ssh.github.com:443.
    GitHub,
}

#[derive(Debug, Clone)]
pub enum SshConfigSource {
    /// `~/.ssh/config` if it exists.
    Default,
    File(PathBuf),
    Off,
}

#[derive(Debug, Clone)]
pub struct SshOptions {
    pub backend: Backend,
    /// Empty = default chain: agent, then the IdentityFile(s) from ssh config, then
    /// `~/.ssh/id_ed25519`, `id_ecdsa`, `id_rsa`.
    pub auth: Vec<AuthSource>,
    /// Empty = `~/.ssh/known_hosts` (+ `/etc/ssh/ssh_known_hosts` on Unix).
    pub known_hosts: Vec<KnownHosts>,
    /// Trust a host that has no known_hosts entry. A *changed* or revoked key is always refused.
    pub accept_unknown_hosts: bool,
    pub ssh_config: SshConfigSource,
    /// TCP connect + key exchange + auth + channel open.
    pub connect_timeout: Duration,
    /// Max wait for any single read/write once connected.
    pub io_timeout: Duration,
    /// Exec backend: command line to run instead of `ssh`, split into words like `GIT_SSH_COMMAND`
    /// (precedence: `GIT_SSH_COMMAND` > `GIT_SSH` > this > `ssh`).
    pub exec_command: Option<String>,
}

impl Default for SshOptions {
    fn default() -> Self {
        SshOptions {
            backend: Backend::from_env(),
            auth: vec![],
            known_hosts: vec![],
            accept_unknown_hosts: false,
            ssh_config: SshConfigSource::Default,
            connect_timeout: Duration::from_secs(30),
            io_timeout: Duration::from_secs(120),
            exec_command: None,
        }
    }
}
