//! A small known_hosts reader: plain, comma lists, `[host]:port`, `*`/`?` wildcards, `!` negation,
//! hashed `|1|salt|hash` entries and the `@revoked` marker. `@cert-authority` lines are skipped
//! (host certificates are not supported in this PoC).

use std::path::Path;

use data_encoding::BASE64;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::{Algorithm, HashAlg, PublicKey, parse_public_key_base64};
use sha1::Sha1;

/// GitHub's published host keys (`api.github.com/meta` → `ssh_keys`), for github.com:22 and ssh.github.com:443.
pub const GITHUB_KNOWN_HOSTS: &str = "\
github.com,[ssh.github.com]:443 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl
github.com,[ssh.github.com]:443 ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEmKSENjQEezOmxkZMy7opKgwFB9nkt5YRrYMjNuG5N87uRgg6CLrbo5wAdT/y6v0mKV0U2w0WZ2YB/++Tpockg=
github.com,[ssh.github.com]:443 ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQCj7ndNxQowgcQnjshcLrqPEiiphnt+VTTvDP6mHBL9j1aNUkY4Ue1gvwnGLVlOhGeYrnZaMgRK6+PKCUXaDbC7qtbW8gIkhL7aGCsOr/C56SJMy/BCZfxd1nWzAOxSDPgVsmerOBYfNqltV9/hWCqBywINIR+5dIg6JTJ72pcEpEjcYgXkE2YEFXV1JHnsKgbLWNlhScqb2UmyRkQyytRLtL+38TGxkxCflmO+5Z8CSSNY7GidjMIZ7Q4zMjA2n1nGrlTDkzwDCsw+wqFPGQA179cnfGWOWRVruj16z6XyvxvjJwbz0wQZ75XK5tKSb7FNyeIEs4TT4jk+S4dhPeAUC5y+bDYirYgM4GC7uEnztnZyaVWQ7B381AK4Qdrwt51ZqExKbQpTUNn+EjqoTwvqNj4kqx5QUCI0ThS/YkOxJCXmPUWZbhjpCg56i+2aB6CmK2JGhn57K5mj0MNdBXA4/WnwH6XoPWJzK5Nyu2zB3nAZp+S5hpQs+p1vN1/wsjk=
";

#[derive(Debug, Clone)]
pub struct Entry {
    revoked: bool,
    patterns: String,
    pub key: PublicKey,
    /// `file:line`, for error messages.
    pub origin: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Trusted,
    Unknown,
    /// The host is listed with a key of the same type but different bytes.
    Changed { origin: String },
    Revoked { origin: String },
}

pub fn parse(text: &str, label: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f = line.split_whitespace();
        let mut first = f.next().unwrap_or_default();
        let mut revoked = false;
        if first.starts_with('@') {
            if first != "@revoked" {
                continue; // @cert-authority: unsupported
            }
            revoked = true;
            first = f.next().unwrap_or_default();
        }
        let (_ty, Some(b64)) = (f.next(), f.next()) else { continue };
        let Ok(key) = parse_public_key_base64(b64) else { continue };
        out.push(Entry { revoked, patterns: first.to_string(), key, origin: format!("{label}:{}", i + 1) });
    }
    out
}

pub fn load_file(path: &Path) -> std::io::Result<Vec<Entry>> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(parse(&t, &path.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e),
    }
}

fn host_key(host: &str, port: u16) -> String {
    if port == 22 { host.to_string() } else { format!("[{host}]:{port}") }
}

fn glob(pat: &str, s: &str) -> bool {
    fn go(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], s) || (!s.is_empty() && go(p, &s[1..])),
            (Some(b'?'), Some(_)) => go(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => go(&p[1..], &s[1..]),
            _ => false,
        }
    }
    go(pat.as_bytes(), s.as_bytes())
}

fn hashed_match(entry: &str, name: &str) -> bool {
    let mut parts = entry.split('|').skip(2);
    let (Some(Ok(salt)), Some(Ok(hash))) =
        (parts.next().map(|p| BASE64.decode(p.as_bytes())), parts.next().map(|p| BASE64.decode(p.as_bytes())))
    else {
        return false;
    };
    let Ok(mac) = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt) else { return false };
    mac.chain_update(name.as_bytes()).verify_slice(&hash).is_ok()
}

pub fn matches(patterns: &str, host: &str, port: u16) -> bool {
    let name = host_key(host, port);
    let mut hit = false;
    for p in patterns.split(',') {
        let (neg, p) = match p.strip_prefix('!') {
            Some(p) => (true, p),
            None => (false, p),
        };
        let m = if p.starts_with("|1|") { hashed_match(p, &name) } else { glob(p, &name) };
        if m && neg {
            return false;
        }
        hit |= m;
    }
    hit
}

fn same_family(a: Algorithm, b: Algorithm) -> bool {
    a == b || a.is_rsa() && b.is_rsa()
}

pub fn verify(entries: &[Entry], host: &str, port: u16, key: &PublicKey) -> Verdict {
    let mine: Vec<&Entry> = entries.iter().filter(|e| matches(&e.patterns, host, port)).collect();
    if let Some(e) = mine.iter().find(|e| e.revoked && e.key.key_data() == key.key_data()) {
        return Verdict::Revoked { origin: e.origin.clone() };
    }
    if mine.iter().any(|e| !e.revoked && e.key.key_data() == key.key_data()) {
        return Verdict::Trusted;
    }
    match mine.iter().find(|e| !e.revoked && same_family(e.key.algorithm(), key.algorithm())) {
        Some(e) => Verdict::Changed { origin: e.origin.clone() },
        None => Verdict::Unknown,
    }
}

/// Host-key algorithms to offer, restricted to the types already known for this host (as OpenSSH
/// orders them), so a host known only by its RSA key isn't reported "unknown" when it would also
/// offer ed25519. Empty = no entries, use the library default.
pub fn algorithms_for(entries: &[Entry], host: &str, port: u16) -> Vec<Algorithm> {
    let mut out: Vec<Algorithm> = Vec::new();
    for e in entries.iter().filter(|e| !e.revoked && matches(&e.patterns, host, port)) {
        let algs = if e.key.algorithm().is_rsa() {
            vec![Algorithm::Rsa { hash: Some(HashAlg::Sha512) }, Algorithm::Rsa { hash: Some(HashAlg::Sha256) }]
        } else {
            vec![e.key.algorithm()]
        };
        for a in algs {
            if !out.contains(&a) {
                out.push(a);
            }
        }
    }
    out
}

pub fn describe(key: &PublicKey) -> String {
    format!("{} {}", key.algorithm(), key.fingerprint(HashAlg::Sha256))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_keys_and_matching() {
        let e = parse(GITHUB_KNOWN_HOSTS, "gh");
        assert_eq!(e.len(), 3);
        let fps: Vec<String> = e.iter().map(|e| e.key.fingerprint(HashAlg::Sha256).to_string()).collect();
        assert!(fps.contains(&"SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU".to_string()));
        assert!(fps.contains(&"SHA256:p2QAMXNIC1TJYWeIOttrVc98/R1BUFWu3/LiyKgUfQM".to_string()));
        assert!(fps.contains(&"SHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s".to_string()));
        let k = &e[0].key;
        assert_eq!(verify(&e, "github.com", 22, k), Verdict::Trusted);
        assert_eq!(verify(&e, "ssh.github.com", 443, k), Verdict::Trusted);
        assert_eq!(verify(&e, "ssh.github.com", 22, k), Verdict::Unknown);
        assert_eq!(verify(&e, "gitlab.com", 22, k), Verdict::Unknown);
        assert_eq!(algorithms_for(&e, "github.com", 22).len(), 4);
        assert!(matches("*.example.com,!bad.example.com", "a.example.com", 22));
        assert!(!matches("*.example.com,!bad.example.com", "bad.example.com", 22));
    }

    #[test]
    fn changed_and_revoked() {
        let gh = parse(GITHUB_KNOWN_HOSTS, "gh");
        let ed = gh[0].key.clone();
        // github's ed25519 key recorded for another host = "changed" for that host
        let other = parse(&format!("example.com ssh-ed25519 {}\n", ed_b64()), "kh");
        let fake = parse("example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ\n", "kh");
        assert_eq!(verify(&other, "example.com", 22, &ed), Verdict::Trusted);
        assert_eq!(verify(&fake, "example.com", 22, &ed), Verdict::Changed { origin: "kh:1".into() });
        let rev = parse(&format!("@revoked * ssh-ed25519 {}\n", ed_b64()), "rv");
        assert_eq!(verify(&rev, "github.com", 22, &ed), Verdict::Revoked { origin: "rv:1".into() });
    }

    #[test]
    fn hashed() {
        // |1|salt|HMAC-SHA1(salt, "github.com"), generated with `ssh-keygen -H`
        let salt = [7u8; 20];
        let mac = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt).unwrap().chain_update(b"github.com").finalize().into_bytes();
        let line = format!("|1|{}|{} ssh-ed25519 {}\n", BASE64.encode(&salt), BASE64.encode(&mac), ed_b64());
        let e = parse(&line, "h");
        assert_eq!(verify(&e, "github.com", 22, &e[0].key), Verdict::Trusted);
        assert_eq!(verify(&e, "gitlab.com", 22, &e[0].key), Verdict::Unknown);
    }

    fn ed_b64() -> &'static str {
        "AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl"
    }
}
