//! Parse the SSH remote URL forms git accepts: `ssh://[user@]host[:port]/path`
//! (also `ssh+git://`, `git+ssh://`) and scp-style `[user@]host:path`.

use crate::SshError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshUrl {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    /// Path as sent to `git-upload-pack '<path>'`.
    pub path: String,
}

pub fn parse(url: &str) -> Result<SshUrl, SshError> {
    let bad = |why: &str| SshError::BadUrl(format!("{url}: {why}"));
    let lower = url.to_ascii_lowercase();
    let rest = ["ssh://", "ssh+git://", "git+ssh://"]
        .iter()
        .find(|p| lower.starts_with(*p))
        .map(|p| &url[p.len()..]);

    let parsed = if let Some(rest) = rest {
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => return Err(bad("missing path")),
        };
        let (user, hostport) = split_user(authority);
        let (host, port) = if let Some(h) = hostport.strip_prefix('[') {
            let end = h.find(']').ok_or_else(|| bad("unterminated [ipv6]"))?;
            let port = match &h[end + 1..] {
                "" => None,
                p => Some(parse_port(p.strip_prefix(':').ok_or_else(|| bad("junk after ]"))?, url)?),
            };
            (h[..end].to_string(), port)
        } else {
            match hostport.rsplit_once(':') {
                Some((h, "")) => (h.to_string(), None),
                Some((h, p)) => (h.to_string(), Some(parse_port(p, url)?)),
                None => (hostport.to_string(), None),
            }
        };
        // ssh://host/~user/repo -> "~user/repo" (as git does)
        let path = path.strip_prefix('/').filter(|p| p.starts_with('~')).unwrap_or(path);
        SshUrl { user, host, port, path: path.to_string() }
    } else {
        // scp-like: the first ':' before any '/' separates host and path.
        let colon = url.find(':').ok_or_else(|| bad("not an ssh URL"))?;
        if url[..colon].contains('/') {
            return Err(bad("not an ssh URL (looks like a local path)"));
        }
        let (user, host) = split_user(&url[..colon]);
        let host = host.trim_start_matches('[').trim_end_matches(']').to_string();
        SshUrl { user, host, port: None, path: url[colon + 1..].to_string() }
    };
    if parsed.host.is_empty() {
        return Err(bad("empty host"));
    }
    if parsed.host.starts_with('-') || parsed.user.as_deref().is_some_and(|u| u.starts_with('-')) {
        return Err(bad("host or user starts with '-'"));
    }
    if parsed.path.is_empty() {
        return Err(bad("empty path"));
    }
    Ok(parsed)
}

fn split_user(s: &str) -> (Option<String>, &str) {
    match s.rsplit_once('@') {
        Some((u, h)) => (Some(u.to_string()), h),
        None => (None, s),
    }
}

fn parse_port(p: &str, url: &str) -> Result<u16, SshError> {
    p.parse().map_err(|_| SshError::BadUrl(format!("{url}: bad port {p:?}")))
}

/// `git-upload-pack '<path>'`, quoted the way git quotes it for a POSIX shell.
pub fn remote_command(service: &str, path: &str) -> String {
    format!("{service} '{}'", path.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> SshUrl {
        parse(s).unwrap()
    }

    #[test]
    fn forms() {
        assert_eq!(
            p("git@github.com:org/repo.git"),
            SshUrl { user: Some("git".into()), host: "github.com".into(), port: None, path: "org/repo.git".into() }
        );
        assert_eq!(
            p("ssh://git@ssh.github.com:443/org/repo.git"),
            SshUrl { user: Some("git".into()), host: "ssh.github.com".into(), port: Some(443), path: "/org/repo.git".into() }
        );
        assert_eq!(p("ssh://host/~me/r").path, "~me/r");
        assert_eq!(p("git+ssh://h/x").host, "h");
        assert_eq!(p("ssh://[::1]:2222/x").host, "::1");
        assert_eq!(p("ssh://[::1]:2222/x").port, Some(2222));
        assert_eq!(p("alias:repo").user, None);
        assert!(parse("ssh://-oProxyCommand=x/y").is_err());
        assert!(parse("./a:b").is_err());
        assert!(parse("ssh://h:99999/x").is_err());
    }

    #[test]
    fn quoting() {
        assert_eq!(remote_command("git-upload-pack", "a'b"), r"git-upload-pack 'a'\''b'");
    }
}
