//! `~/.ssh/config` lookup via `ssh2-config`: Host alias → HostName, User, Port, IdentityFile.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use ssh2_config::{ParseRule, SshConfig};

use crate::url::SshUrl;
use crate::{SshConfigSource, SshError};

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    /// Name used for known_hosts and TCP (HostName, or the URL host).
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identity_files: Vec<PathBuf>,
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

pub fn resolve(url: &SshUrl, src: &SshConfigSource) -> Result<Resolved, SshError> {
    let path = match src {
        SshConfigSource::Off => None,
        SshConfigSource::File(p) => Some(p.clone()),
        SshConfigSource::Default => home_dir().map(|h| h.join(".ssh").join("config")).filter(|p| p.is_file()),
    };
    let params = match path {
        Some(p) => {
            let f = File::open(&p).map_err(|e| SshError::Config(format!("{}: {e}", p.display())))?;
            let cfg = SshConfig::default()
                .parse(&mut BufReader::new(f), ParseRule::ALLOW_UNKNOWN_FIELDS)
                .map_err(|e| SshError::Config(format!("{}: {e}", p.display())))?;
            Some(cfg.query(&url.host))
        }
        None => None,
    };
    let local_user = || std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "git".into());
    Ok(Resolved {
        host: params.as_ref().and_then(|p| p.host_name.clone()).unwrap_or_else(|| url.host.clone()),
        port: url.port.or(params.as_ref().and_then(|p| p.port)).unwrap_or(22),
        user: url.user.clone().or(params.as_ref().and_then(|p| p.user.clone())).unwrap_or_else(local_user),
        identity_files: params.and_then(|p| p.identity_file).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config");
        std::fs::write(
            &p,
            "Host sandbox\n  HostName ssh.github.com\n  User git\n  Port 443\n  IdentityFile /keys/id_test\n\nHost *\n  ServerAliveInterval 30\n  UnknownThing yes\n",
        )
        .unwrap();
        let u = crate::url::parse("sandbox:org/repo.git").unwrap();
        let r = resolve(&u, &SshConfigSource::File(p.clone())).unwrap();
        assert_eq!(r.host, "ssh.github.com");
        assert_eq!(r.port, 443);
        assert_eq!(r.user, "git");
        assert_eq!(r.identity_files, vec![PathBuf::from("/keys/id_test")]);
        // URL user and port win
        let u = crate::url::parse("ssh://me@sandbox:2222/x").unwrap();
        let r = resolve(&u, &SshConfigSource::File(p)).unwrap();
        assert_eq!((r.user.as_str(), r.port), ("me", 2222));
    }
}
