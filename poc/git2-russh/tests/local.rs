//! Local end-to-end tests: git2 (no libssh2) + our transport against a local russh server that runs
//! the real `git upload-pack` / `receive-pack`. Needs `git` and `ssh-keygen` on PATH (else skipped).

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{Keys, gen_keys, ls_remote, result, scenario, server, stress, watchdog};
use git2::Repository;
use git2_russh::{AuthSource, Backend, KnownHosts, SshConfigSource, SshOptions, with_options};
use tempfile::TempDir;

struct Fx {
    keys: Keys,
    srv: server::TestServer,
    _root: TempDir,
}

impl Fx {
    fn url(&self, repo: &str) -> String {
        format!("ssh://git@127.0.0.1:{}/{repo}", self.srv.port)
    }
    fn opts(&self, auth: Vec<AuthSource>) -> SshOptions {
        SshOptions {
            backend: Backend::Native,
            auth,
            known_hosts: vec![KnownHosts::Text { label: "test-kh".into(), text: self.srv.known_hosts.clone() }],
            ssh_config: SshConfigSource::Off,
            connect_timeout: Duration::from_secs(10),
            io_timeout: Duration::from_secs(10),
            ..Default::default()
        }
    }
    fn file(&self, name: &str) -> AuthSource {
        AuthSource::KeyFile { path: self.keys.path(name), passphrase: None }
    }
    fn mem(&self, name: &str) -> AuthSource {
        AuthSource::KeyMemory { label: name.into(), text: self.keys.text(name), passphrase: None }
    }
}

fn fixture() -> Option<Fx> {
    git2_russh::register().unwrap();
    if std::process::Command::new("git").arg("--version").output().is_err() {
        println!("SKIP: no git for the local server");
        return None;
    }
    let keys = gen_keys()?;
    let root = TempDir::new().unwrap();
    let bare = Repository::init_bare(root.path().join("repo.git")).unwrap();
    let mut prev = None;
    for i in 0..3 {
        prev = Some(common::commit_on(&bare, prev, &format!("f{i}.txt"), &format!("{i}\n"), &format!("c{i}")));
    }
    bare.reference("refs/heads/main", prev.unwrap(), true, "init").unwrap();
    bare.set_head("refs/heads/main").unwrap();
    let host = russh::keys::load_secret_key(keys.path("host"), None).unwrap();
    let authorized = ["ed25519", "ecdsa", "rsa", "rsa_pem", "ed25519_pass"].iter().map(|n| keys.public(n)).collect();
    let srv = server::start(root.path().to_path_buf(), host, authorized);
    Some(Fx { keys, srv, _root: root })
}

fn err_of<T>(r: Result<T, git2::Error>) -> git2::Error {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

#[test]
fn ops_scenario_over_russh() {
    let Some(fx) = fixture() else { return };
    let url = fx.url("repo.git");
    with_options(fx.opts(vec![fx.file("ed25519")]), || scenario("local_ops", &url, "t-local-")).unwrap();
}

#[test]
fn every_key_type_file_and_memory() {
    let Some(fx) = fixture() else { return };
    let url = fx.url("repo.git");
    for name in ["ed25519", "ecdsa", "rsa", "rsa_pem"] {
        for src in [fx.file(name), fx.mem(name)] {
            let label = format!("{src:?}");
            let r = with_options(fx.opts(vec![src]), || ls_remote(&url));
            result("local_keys", &label, r.as_ref().map(|_| "ok".to_string()).unwrap_or_else(|e| e.to_string()));
            assert!(r.is_ok(), "{label}: {:?}", r.err());
        }
    }
    let pass = AuthSource::KeyFile { path: fx.keys.path("ed25519_pass"), passphrase: Some("pw".into()) };
    assert!(with_options(fx.opts(vec![pass]), || ls_remote(&url)).is_ok());
    // Wrong key first, then a good one: the chain moves on.
    let wrong = AuthSource::KeyFile { path: fx.keys.path("host"), passphrase: None };
    assert!(with_options(fx.opts(vec![wrong, fx.mem("ecdsa")]), || ls_remote(&url)).is_ok());
}

#[test]
fn scp_style_via_ssh_config_alias() {
    let Some(fx) = fixture() else { return };
    let cfg = fx.keys.path("config");
    std::fs::write(
        &cfg,
        format!(
            "Host sandbox-alias\n  HostName 127.0.0.1\n  User git\n  Port {}\n  IdentityFile {}\n",
            fx.srv.port,
            fx.keys.path("ecdsa").display()
        ),
    )
    .unwrap();
    let mut o = fx.opts(vec![]);
    o.auth = vec![AuthSource::AgentAt(if cfg!(windows) { r"\\.\pipe\no-such-agent".into() } else { "/nonexistent".into() })];
    o.ssh_config = SshConfigSource::File(cfg);
    let r = with_options(o, || ls_remote("sandbox-alias:repo.git"));
    assert!(r.is_ok(), "{:?}", r.err());
}

#[test]
fn host_key_policy() {
    let Some(fx) = fixture() else { return };
    let url = fx.url("repo.git");
    let mut o = fx.opts(vec![fx.file("ed25519")]);
    o.known_hosts = vec![KnownHosts::Text { label: "empty".into(), text: String::new() }];
    let e = err_of(with_options(o.clone(), || ls_remote(&url)));
    result("local_hostkey", "unknown", e.message());
    assert_eq!(e.code(), git2::ErrorCode::Certificate);
    assert!(e.message().contains("not in known_hosts"));

    o.accept_unknown_hosts = true;
    assert!(with_options(o.clone(), || ls_remote(&url)).is_ok());

    // Same host listed with another ed25519 key: refused even with accept_unknown_hosts.
    let other = fx.keys.public("ed25519").to_openssh().unwrap();
    o.known_hosts = vec![KnownHosts::Text { label: "kh".into(), text: format!("[127.0.0.1]:{} {other}\n", fx.srv.port) }];
    let e = err_of(with_options(o, || ls_remote(&url)));
    result("local_hostkey", "changed", e.message());
    assert!(e.message().contains("HOST KEY MISMATCH"));
}

#[test]
fn clear_errors_and_no_hangs() {
    let Some(fx) = fixture() else { return };
    let url = fx.url("repo.git");
    // A key the server doesn't know.
    let e = err_of(with_options(fx.opts(vec![fx.file("host")]), || ls_remote(&url)));
    result("local_errors", "bad key", e.message());
    assert_eq!(e.code(), git2::ErrorCode::Auth);
    // No source at all.
    let e = err_of(with_options(fx.opts(vec![AuthSource::AgentAt("/nonexistent-agent".into())]), || ls_remote(&url)));
    result("local_errors", "no agent", e.message());
    // Repo that doesn't exist: the server's stderr is in the error.
    let e = err_of(with_options(fx.opts(vec![fx.file("ed25519")]), || ls_remote(&fx.url("nope.git"))));
    result("local_errors", "no repo", e.message());
    assert!(e.message().contains("nope.git"), "{}", e.message());
    // Connection refused.
    let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let e = err_of(with_options(fx.opts(vec![]), || ls_remote(&format!("ssh://git@127.0.0.1:{dead}/x"))));
    result("local_errors", "refused", e.message());

    // Server that never sends a banner: connect timeout.
    let silent = server::start_silent();
    let mut o = fx.opts(vec![fx.file("ed25519")]);
    o.connect_timeout = Duration::from_secs(2);
    let t0 = Instant::now();
    let r = watchdog(20, o.clone(), move || ls_remote(&format!("ssh://git@{silent}/x")).map_err(|e| e.message().to_string()));
    let e = r.expect("must not hang").unwrap_err();
    result("local_errors", "silent server", format!("{e} after {:.1} s", t0.elapsed().as_secs_f64()));
    assert!(e.contains("timed out after 2 s"));

    // Server accepts exec and then never answers: I/O timeout.
    o.io_timeout = Duration::from_secs(2);
    let u = fx.url("hang.git");
    let t0 = Instant::now();
    let r = watchdog(20, o, move || ls_remote(&u).map_err(|e| e.message().to_string()));
    let e = r.expect("must not hang").unwrap_err();
    result("local_errors", "stalled server", format!("{e} after {:.1} s", t0.elapsed().as_secs_f64()));
    assert!(e.contains("no data for 2 s"));
}

#[test]
fn handshake_stress_50() {
    let Some(fx) = fixture() else { return };
    let url = fx.url("repo.git");
    let (ok, bad, first, avg) = with_options(fx.opts(vec![fx.file("ed25519")]), || stress(&url, 50));
    result("local_stress", "50 x connect+ls", format!("{ok} ok, {bad} failed, avg {avg} ms, first error {first:?}"));
    assert_eq!(bad, 0);
}

/// The bridge works when the caller is itself inside a tokio runtime (no nested-runtime panic).
#[test]
fn bridge_from_inside_tokio() {
    let Some(fx) = fixture() else { return };
    let url = fx.url("repo.git");
    let o = fx.opts(vec![fx.file("ed25519")]);
    let mt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let (o2, u2) = (o.clone(), url.clone());
    assert!(mt.block_on(async move { with_options(o2, || ls_remote(&u2)) }).is_ok());
    let (o3, u3) = (o.clone(), url.clone());
    assert!(mt.block_on(async move { tokio::task::spawn_blocking(move || with_options(o3, || ls_remote(&u3))).await.unwrap() }).is_ok());
    let ct = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    assert!(ct.block_on(async move { with_options(o, || ls_remote(&url)) }).is_ok());
}

/// b1: the system `ssh` against the same local server.
#[test]
fn exec_backend_local() {
    let Some(fx) = fixture() else { return };
    let ssh = if cfg!(windows) { r"C:\Windows\System32\OpenSSH\ssh.exe" } else { "ssh" };
    if std::process::Command::new(ssh).arg("-V").output().is_err() {
        println!("SKIP exec_backend_local: no ssh");
        return;
    }
    let kh = fx.keys.path("known_hosts");
    std::fs::write(&kh, &fx.srv.known_hosts).unwrap();
    let q = |p: &Path| format!("\"{}\"", p.display());
    let mut o = fx.opts(vec![]);
    o.backend = Backend::Exec;
    o.exec_command = Some(format!(
        "{ssh} -F none -i {} -o IdentitiesOnly=yes -o UserKnownHostsFile={} -o IdentityAgent=none",
        q(&fx.keys.path("ed25519")),
        q(&kh)
    ));
    // OpenSSH refuses keys readable by others.
    restrict(&fx.keys.path("ed25519"));
    let url = fx.url("repo.git");
    let u = url.clone();
    let r = watchdog(120, o.clone(), move || scenario("local_exec", &u, "t-exec-"));
    assert!(matches!(r, Ok(Ok(()))), "{r:?}");

    // Unknown host with BatchMode: refused, no prompt, no hang.
    std::fs::write(&kh, "").unwrap();
    let u = url.clone();
    let r = watchdog(30, o.clone(), move || ls_remote(&u).map_err(|e| e.message().to_string()));
    let e = r.expect("must not hang").unwrap_err();
    result("local_exec", "unknown host", &e);
    assert!(e.contains("Host key verification failed"), "{e}");

    // No ssh binary.
    o.exec_command = Some("definitely-not-ssh-xyz".into());
    let r = with_options(o, || ls_remote(&url));
    let e = err_of(r);
    result("local_exec", "no ssh", e.message());
    assert!(e.message().contains("not found"));
}

fn restrict(p: &Path) {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap();
        let _ = std::process::Command::new("icacls")
            .arg(p)
            .args(["/inheritance:r", "/grant:r", &format!("{user}:F")])
            .output();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// Bridge cost: clone a repo holding one 64 MiB incompressible blob over russh (native),
/// over the system ssh (exec) and over file:// (no transport). `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn bench_throughput() {
    use rand::RngExt;
    let Some(fx) = fixture() else { return };
    let big = fx._root.path().join("big.git");
    let r = Repository::init_bare(&big).unwrap();
    let mut data = vec![0u8; 64 << 20];
    rand::rng().fill(&mut data[..]);
    let blob = r.blob(&data).unwrap();
    let mut tb = r.treebuilder(None).unwrap();
    tb.insert("big.bin", blob, 0o100644).unwrap();
    let tree = r.find_tree(tb.write().unwrap()).unwrap();
    let s = common::bot();
    let c = r.commit(None, &s, &s, "big", &tree, &[]).unwrap();
    r.reference("refs/heads/main", c, true, "").unwrap();
    r.set_head("refs/heads/main").unwrap();
    let kh = fx.keys.path("known_hosts");
    std::fs::write(&kh, &fx.srv.known_hosts).unwrap();
    restrict(&fx.keys.path("ed25519"));
    let ssh = if cfg!(windows) { r"C:\Windows\System32\OpenSSH\ssh.exe" } else { "ssh" };
    let mut exec = fx.opts(vec![]);
    exec.backend = Backend::Exec;
    exec.exec_command = Some(format!(
        "{ssh} -F none -i \"{}\" -o IdentitiesOnly=yes -o UserKnownHostsFile=\"{}\" -o IdentityAgent=none",
        fx.keys.path("ed25519").display(),
        kh.display()
    ));
    let file_url = format!("file:///{}", big.display().to_string().replace('\\', "/").trim_start_matches('/'));
    for (name, o, url) in [
        ("file://", fx.opts(vec![]), file_url),
        ("russh", fx.opts(vec![fx.file("ed25519")]), fx.url("big.git")),
        ("system ssh", exec, fx.url("big.git")),
    ] {
        for round in 0..2 {
            let tmp = TempDir::new().unwrap();
            let t0 = Instant::now();
            let ok = with_options(o.clone(), || {
                let d = Repository::init_bare(tmp.path()).unwrap();
                d.remote_anonymous(&url).and_then(|mut rm| rm.fetch(&["+refs/heads/main:refs/heads/main"], None, None))
            });
            let s = t0.elapsed().as_secs_f64();
            assert!(ok.is_ok(), "{name}: {ok:?}");
            result("bench", &format!("{name} round {round}"), format!("64 MiB in {s:.2} s = {:.0} MiB/s", 64.0 / s));
        }
    }
}
