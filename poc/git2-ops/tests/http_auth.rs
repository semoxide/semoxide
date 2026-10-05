//! Credentials callback + G5 push-permission probe against a mock smart-HTTP server on 127.0.0.1.
//! The mock speaks just enough of the protocol: 401 Basic challenge, then a receive-pack ref
//! advertisement for the right token, 403 for a "read-only" token, 401 again for a bad one.
//! This proves the client side (callback invoked, credential on the wire, probe outcome),
//! NOT GitHub's actual server behavior (that needs a real remote).
mod common;
use common::*;

use git2::{CredentialType, Direction, PushOptions, RemoteCallbacks};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct Req {
    line: String,
    auth: Option<String>,
}

fn b64(s: &str) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let b = s.as_bytes();
    let mut o = String::new();
    for ch in b.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            o.push(if i <= ch.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    o
}

fn pkt(s: &str) -> String {
    format!("{:04x}{s}", s.len() + 4)
}

/// Serve forever on a background thread; returns (base url, request log).
fn mock_server(main_oid: git2::Oid) -> (String, Arc<Mutex<Vec<Req>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}/org/repo.git", l.local_addr().unwrap().port());
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut rd = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            rd.read_line(&mut line).unwrap();
            let (mut auth, mut len) = (None, 0usize);
            loop {
                let mut h = String::new();
                if rd.read_line(&mut h).unwrap() == 0 || h == "\r\n" {
                    break;
                }
                let lower = h.to_ascii_lowercase();
                if lower.starts_with("authorization:") {
                    auth = Some(h[14..].trim().to_string());
                }
                if let Some(v) = lower.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; len];
            let _ = rd.read_exact(&mut body);
            log2.lock().unwrap().push(Req { line: line.trim().to_string(), auth: auth.clone() });
            let good = format!("Basic {}", b64("x-access-token:good"));
            let ro = format!("Basic {}", b64("x-access-token:ro"));
            let resp = if line.starts_with("POST") {
                "HTTP/1.1 500 Unexpected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else if auth.as_deref() == Some(good.as_str()) {
                let svc = if line.contains("git-receive-pack") { "git-receive-pack" } else { "git-upload-pack" };
                let body = format!(
                    "{}0000{}0000",
                    pkt(&format!("# service={svc}\n")),
                    pkt(&format!("{main_oid} refs/heads/main\0report-status delete-refs ofs-delta agent=mock\n"))
                );
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/x-{svc}-advertisement\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
            } else if auth.as_deref() == Some(ro.as_str()) {
                "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else {
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"mock\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            };
            let _ = s.write_all(resp.as_bytes());
        }
    });
    (url, log)
}

/// G5 probe: `connect(Push)` with credentials. Returns Ok(advertised refs) or the error.
fn probe(f: &Fixture, url: &str, token: &str, calls: &Arc<Mutex<Vec<CredentialType>>>) -> Result<Vec<String>, git2::Error> {
    let mut inner = make_cred_cb(Some(token.to_string()), 1);
    let calls = calls.clone();
    let mut cb = RemoteCallbacks::new();
    cb.credentials(move |u, user, allowed| {
        calls.lock().unwrap().push(allowed);
        inner(u, user, allowed)
    });
    let mut rem = f.work.remote_anonymous(url)?;
    let conn = rem.connect_auth(Direction::Push, Some(cb), None)?;
    Ok(conn.list()?.iter().map(|h| h.name().to_string()).collect())
}

#[test]
fn g05_https_token_probe_write_ro_bad() {
    let f = fixture();
    let head = f.work.head().unwrap().target().unwrap();
    let (url, log) = mock_server(head);

    // write token
    let calls = Arc::new(Mutex::new(Vec::new()));
    let refs = probe(&f, &url, "good", &calls).unwrap();
    assert_eq!(refs, ["refs/heads/main"]);
    let c = calls.lock().unwrap().clone();
    assert_eq!(c.len(), 1, "callback invoked once after the 401 challenge");
    assert!(c[0].contains(CredentialType::USER_PASS_PLAINTEXT), "{c:?}");
    let l = log.lock().unwrap().clone();
    assert!(l.iter().all(|r| r.line.contains("info/refs?service=git-receive-pack")), "{l:?}");
    assert_eq!(l.last().unwrap().auth.as_deref(), Some(format!("Basic {}", b64("x-access-token:good")).as_str()));

    // read-only token: 403 on the receive-pack advertisement
    let e = probe(&f, &url, "ro", &Arc::new(Mutex::new(Vec::new()))).unwrap_err();
    eprintln!("ro token: {e} ({:?}/{:?})", e.class(), e.code());
    // bad token: 401 again -> callback re-invoked -> our cb gives up
    let calls = Arc::new(Mutex::new(Vec::new()));
    let e2 = probe(&f, &url, "bad", &calls).unwrap_err();
    eprintln!("bad token: {e2} ({:?}/{:?}), callback calls: {}", e2.class(), e2.code(), calls.lock().unwrap().len());
    assert_eq!(calls.lock().unwrap().len(), 2, "retry loop is bounded by the callback");
    assert_ne!(e.message(), e2.message(), "ro (403) and bad (401) are distinguishable");
}

// Full G5 dry-run over HTTP: negotiation-abort sends only GET info/refs, never the POST.
#[test]
fn g05_https_push_dry_run_sends_no_pack() {
    let f = fixture();
    let parent = f.work.head().unwrap().target().unwrap();
    let (url, log) = mock_server(parent);
    let child = commit(&f.work, "a", "a", "feat: a", 2_000);
    let mut cb = RemoteCallbacks::new();
    cb.credentials(make_cred_cb(Some("good".into()), 1));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    cb.push_negotiation(move |ups| {
        seen2.lock().unwrap().extend(ups.iter().map(|u| (u.src(), u.dst())));
        Err(git2::Error::from_str("semoxide-dry-run"))
    });
    let mut po = PushOptions::new();
    po.remote_callbacks(cb);
    let e = f.work.remote_anonymous(&url).unwrap().push(&["HEAD:refs/heads/main"], Some(&mut po)).unwrap_err();
    assert!(e.message().contains("semoxide-dry-run"), "{e}");
    assert_eq!(*seen.lock().unwrap(), [(parent, child)]);
    let l = log.lock().unwrap().clone();
    assert!(l.iter().all(|r| r.line.starts_with("GET")), "no POST (pack upload): {l:?}");
}

// SSH: no local sshd, so only the callback branch is exercised (needs sandbox for the transport).
#[test]
fn ssh_agent_callback_shape() {
    let mut cb = make_cred_cb(None, 3);
    let c = cb("ssh://git@github.com/org/repo.git", Some("git"), CredentialType::SSH_KEY | CredentialType::SSH_MEMORY).unwrap();
    assert_eq!(c.credtype() as u32, CredentialType::SSH_KEY.bits());
    assert!(c.has_username());
    // libgit2 first asks for USERNAME when the URL carries none.
    let c = cb("ssh://github.com/org/repo.git", None, CredentialType::USERNAME).unwrap();
    assert_eq!(c.credtype() as u32, CredentialType::USERNAME.bits());
    // https token branch
    let mut cb = make_cred_cb(Some("t0k".into()), 1);
    let c = cb("https://github.com/o/r.git", None, CredentialType::USER_PASS_PLAINTEXT).unwrap();
    assert_eq!(c.credtype() as u32, CredentialType::USER_PASS_PLAINTEXT.bits());
}
