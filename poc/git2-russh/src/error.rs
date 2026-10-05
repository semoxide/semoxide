use std::fmt;

#[derive(Debug, Clone)]
pub enum SshError {
    BadUrl(String),
    Config(String),
    Connect { addr: String, detail: String },
    Timeout { phase: String, addr: String, secs: u64 },
    UnknownHost { addr: String, key: String },
    HostKeyChanged { addr: String, key: String, origin: String },
    HostKeyRevoked { addr: String, key: String, origin: String },
    Auth { target: String, attempts: Vec<String> },
    Protocol { addr: String, detail: String },
    ExecNotFound { program: String },
    Exec(String),
}

impl fmt::Display for SshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SshError::BadUrl(s) => write!(f, "invalid ssh URL {s}"),
            SshError::Config(s) => write!(f, "ssh config: {s}"),
            SshError::Connect { addr, detail } => write!(f, "could not connect to {addr}: {detail}"),
            SshError::Timeout { phase, addr, secs } => {
                write!(f, "timed out after {secs} s while {phase} ({addr})")
            }
            SshError::UnknownHost { addr, key } => write!(
                f,
                "host key verification failed: {addr} is not in known_hosts (server key {key}); \
                 add it to known_hosts or allow unknown hosts explicitly"
            ),
            SshError::HostKeyChanged { addr, key, origin } => write!(
                f,
                "HOST KEY MISMATCH for {addr}: server presented {key}, but {origin} lists a different key \
                 of that type; refusing to connect (possible man-in-the-middle)"
            ),
            SshError::HostKeyRevoked { addr, key, origin } => {
                write!(f, "host key {key} for {addr} is revoked ({origin}); refusing to connect")
            }
            SshError::Auth { target, attempts } => {
                write!(f, "ssh authentication failed for {target}; tried: ")?;
                if attempts.is_empty() {
                    write!(f, "nothing (no agent, no key configured)")
                } else {
                    write!(f, "{}", attempts.join("; "))
                }
            }
            SshError::Protocol { addr, detail } => write!(f, "ssh error with {addr}: {detail}"),
            SshError::ExecNotFound { program } => write!(
                f,
                "ssh executable `{program}` not found (exec backend selected via SEMOXIDE_SSH_BACKEND=exec or \
                 GIT_SSH/GIT_SSH_COMMAND); install an OpenSSH client or use the built-in backend"
            ),
            SshError::Exec(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for SshError {}

impl From<SshError> for git2::Error {
    fn from(e: SshError) -> Self {
        use git2::ErrorCode as C;
        let code = match &e {
            SshError::Auth { .. } => C::Auth,
            SshError::UnknownHost { .. } | SshError::HostKeyChanged { .. } | SshError::HostKeyRevoked { .. } => {
                C::Certificate
            }
            SshError::Timeout { .. } => C::Timeout,
            _ => C::GenericError,
        };
        git2::Error::new(code, git2::ErrorClass::Ssh, e.to_string())
    }
}
