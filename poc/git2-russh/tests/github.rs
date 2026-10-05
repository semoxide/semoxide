//! Real-remote tests against the sandbox repo on GitHub. Skipped unless the env is set:
//! - `SANDBOX_REPO` owner/name, `SANDBOX_PREFIX` unique ref prefix for this run
//! - `SSH_DEPLOY_KEY` / `SSH_DEPLOY_KEY_RSA` / `SSH_DEPLOY_KEY_ECDSA`: write deploy keys (OpenSSH format; never printed)
//! - `SSH_DEPLOY_KEY_RSA_PEM_FILE`: PEM (PKCS#1) copy of the RSA key (made by the workflow)
//! - `SANDBOX_SSH_AGENT=1`: the ed25519 key is loaded in the platform agent
//! - `SANDBOX_KNOWN_HOSTS_FILE`: known_hosts built by the workflow from `api.github.com/meta`
//! - `SANDBOX_STRESS` (default 50), `SANDBOX_EXEC=1` to run the system-ssh (b1) tests,
//!   `SANDBOX_NO_SSH=1` in the minimal container (asserts the b1 "not found" error).

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{denied, ls_remote, push, result, scenario, stress, watchdog};
use git2::Repository;
use git2_russh::{AuthSource, Backend, KnownHosts, SshConfigSource, SshOptions, with_options};
use tempfile::TempDir;

struct Ctx {
    repo: String,
    prefix: String,
    dir: TempDir,
}

impl Ctx {
    fn url(&self) -> String {
        format!("git@github.com:{}.git", self.repo)
    }
    fn key(&self, var: &str) -> Option<String> {
        std::env::var(var).ok().filter(|k| !k.trim().is_empty()).map(|k| k.trim().replace("\r\n", "\n") + "\n")
    }
    /// Key text written to a private file in this run's temp dir (deleted with it).
    fn key_file(&self, var: &str) -> Option<PathBuf> {
        let text = self.key(var)?;
        let p = self.dir.path().join(var.to_lowercase());
        std::fs::write(&p, text).unwrap();
        restrict(&p);
        Some(p)
    }
    fn native(&self, auth: Vec<AuthSource>) -> SshOptions {
        SshOptions {
            backend: Backend::Native,
            auth,
            known_hosts: vec![KnownHosts::GitHub],
            ssh_config: SshConfigSource::Off,
            connect_timeout: Duration::from_secs(30),
            io_timeout: Duration::from_secs(60),
            ..Default::default()
        }
    }
    fn mem(&self, var: &str) -> Option<AuthSource> {
        Some(AuthSource::KeyMemory { label: var.into(), text: self.key(var)?, passphrase: None })
    }
    fn known_hosts_file(&self) -> PathBuf {
        let p = self.dir.path().join("known_hosts");
        std::fs::write(&p, git2_russh::known_hosts::GITHUB_KNOWN_HOSTS).unwrap();
        p
    }
}

fn ctx(test: &str) -> Option<Ctx> {
    git2_russh::register().unwrap();
    let repo = std::env::var("SANDBOX_REPO").ok();
    let prefix = std::env::var("SANDBOX_PREFIX").ok();
    match (repo, prefix) {
        (Some(repo), Some(prefix)) if std::env::var("SSH_DEPLOY_KEY").is_ok() => {
            Some(Ctx { repo, prefix: format!("{prefix}{test}-"), dir: TempDir::new().unwrap() })
        }
        _ => {
            println!("SKIP {test}: needs SANDBOX_REPO, SANDBOX_PREFIX, SSH_DEPLOY_KEY");
            None
        }
    }
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

fn msg<T>(r: &Result<T, git2::Error>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(e) => format!("ERR {} (class={:?} code={:?})", e.message(), e.class(), e.code()),
    }
}

/// Ok, or the org-policy "Repository not found" after a completed handshake+auth (see `common::denied`).
fn reached<T, E: std::fmt::Debug>(r: &Result<T, E>) -> bool {
    match r {
        Ok(_) => true,
        Err(e) => denied(&format!("{e:?}")),
    }
}

fn scenario_or_blocked(t: &str, r: &Result<Result<(), String>, String>) -> bool {
    match r {
        Ok(Ok(())) => true,
        Ok(Err(e)) if denied(e) => {
            result(t, "BLOCKED", "handshake+auth OK, repo access refused (org disables deploy keys); ops not run");
            true
        }
        _ => false,
    }
}

// ---------- f: russh ----------

#[test]
fn f1_ops_scenario() {
    let t = "f1_ops";
    let Some(c) = ctx(t) else { return };
    let o = c.native(vec![c.mem("SSH_DEPLOY_KEY").unwrap()]);
    let (url, prefix) = (c.url(), c.prefix.clone());
    let r = watchdog(300, o, move || scenario(t, &url, &prefix));
    result(t, "scenario (ed25519, memory)", format!("{r:?}"));
    assert!(scenario_or_blocked(t, &r));
}

#[test]
fn f2_every_key_type() {
    let t = "f2_keys";
    let Some(c) = ctx(t) else { return };
    let mut cases: Vec<(String, AuthSource)> = Vec::new();
    for var in ["SSH_DEPLOY_KEY", "SSH_DEPLOY_KEY_ECDSA", "SSH_DEPLOY_KEY_RSA"] {
        if let Some(p) = c.key_file(var) {
            cases.push((format!("{var} openssh file"), AuthSource::KeyFile { path: p, passphrase: None }));
            cases.push((format!("{var} openssh memory"), c.mem(var).unwrap()));
        }
    }
    if let Ok(p) = std::env::var("SSH_DEPLOY_KEY_RSA_PEM_FILE") {
        let text = std::fs::read_to_string(&p).unwrap();
        result(t, "pem header", text.lines().next().unwrap_or(""));
        cases.push(("RSA PEM file".into(), AuthSource::KeyFile { path: p.into(), passphrase: None }));
        cases.push(("RSA PEM memory".into(), AuthSource::KeyMemory { label: "rsa-pem".into(), text, passphrase: None }));
    }
    let mut failed = 0;
    for (label, src) in cases {
        let o = c.native(vec![src]);
        let url = c.url();
        let t0 = Instant::now();
        let r = watchdog(90, o, move || ls_remote(&url).map_err(|e| e.message().to_string()));
        let ok = matches!(&r, Ok(inner) if reached(inner));
        failed += usize::from(!ok);
        result(t, &label, format!("{} in {} ms", if ok { "ok".to_string() } else { format!("{r:?}") }, t0.elapsed().as_millis()));
    }
    // One push per key type to prove write access, not just read.
    for var in ["SSH_DEPLOY_KEY_ECDSA", "SSH_DEPLOY_KEY_RSA"] {
        let Some(src) = c.mem(var) else { continue };
        let r = with_options(c.native(vec![src]), || push_and_delete_tag(&c.url(), &c.prefix, &var.to_lowercase()));
        result(t, &format!("{var} push+delete tag"), format!("{r:?}"));
        failed += usize::from(!reached(&r));
    }
    assert_eq!(failed, 0);
}

fn push_and_delete_tag(url: &str, prefix: &str, name: &str) -> Result<(), String> {
    let tmp = TempDir::new().unwrap();
    let r = Repository::init(tmp.path()).unwrap();
    let refs = ls_remote(url).map_err(|e| e.to_string())?;
    let main = common::has(&refs, "refs/heads/main").ok_or("no main")?;
    let mut fo = git2::FetchOptions::new();
    fo.depth(1);
    r.remote_anonymous(url)
        .and_then(|mut rm| rm.fetch(&["+refs/heads/main:refs/heads/main"], Some(&mut fo), None))
        .map_err(|e| e.to_string())?;
    let tag = format!("refs/tags/{prefix}{name}");
    r.reference(&tag, main, true, "t").unwrap();
    let st = push(&r, url, &[&format!("{tag}:{tag}")]).map_err(|e| e.to_string())?;
    let del = push(&r, url, &[&format!(":{tag}")]).map_err(|e| e.to_string())?;
    if st == vec![(tag.clone(), None)] && del == vec![(tag, None)] { Ok(()) } else { Err(format!("{st:?} {del:?}")) }
}

#[test]
fn f3_agent() {
    let t = "f3_agent";
    let Some(c) = ctx(t) else { return };
    if std::env::var("SANDBOX_SSH_AGENT").as_deref() != Ok("1") {
        println!("SKIP {t}: no agent");
        return;
    }
    result(t, "SSH_AUTH_SOCK", std::env::var("SSH_AUTH_SOCK").map(|_| "set").unwrap_or("unset (Windows: named pipe)"));
    let r = with_options(c.native(vec![AuthSource::Agent]), || push_and_delete_tag(&c.url(), &c.prefix, "agent"));
    result(t, "agent: ls-remote + push + delete tag", format!("{r:?}"));
    assert!(reached(&r));
    #[cfg(windows)]
    {
        let r = with_options(c.native(vec![AuthSource::AgentAt(r"\\.\pipe\openssh-ssh-agent".into())]), || ls_remote(&c.url()));
        result(t, r"explicit \\.\pipe\openssh-ssh-agent", msg(&r));
        let r = with_options(c.native(vec![AuthSource::Pageant]), || ls_remote(&c.url()));
        result(t, "pageant (not running)", msg(&r));
    }
}

#[test]
fn f4_host_keys() {
    let t = "f4_hostkey";
    let Some(c) = ctx(t) else { return };
    let key = || vec![c.mem("SSH_DEPLOY_KEY").unwrap()];
    // Live keys from api.github.com/meta (written by the workflow) as a known_hosts file.
    if let Ok(f) = std::env::var("SANDBOX_KNOWN_HOSTS_FILE") {
        let mut o = c.native(key());
        o.known_hosts = vec![KnownHosts::File(f.into())];
        let r = with_options(o, || ls_remote(&c.url()));
        result(t, "known_hosts from api.github.com/meta", msg(&r));
        assert!(reached(&r));
    }
    // Each algorithm on its own: the client offers only the known type, and it matches.
    for alg in ["ssh-ed25519", "ecdsa-sha2-nistp256", "ssh-rsa"] {
        let line = git2_russh::known_hosts::GITHUB_KNOWN_HOSTS.lines().find(|l| l.contains(alg)).unwrap();
        let mut o = c.native(key());
        o.known_hosts = vec![KnownHosts::Text { label: alg.into(), text: line.into() }];
        let r = with_options(o, || ls_remote(&c.url()));
        result(t, &format!("pinned {alg} only"), msg(&r));
        assert!(reached(&r));
    }
    let mut o = c.native(key());
    o.known_hosts = vec![KnownHosts::Text { label: "empty".into(), text: String::new() }];
    let r = with_options(o.clone(), || ls_remote(&c.url()));
    result(t, "unknown host (empty known_hosts)", msg(&r));
    assert_eq!(r.as_ref().err().map(|e| e.code()), Some(git2::ErrorCode::Certificate));
    o.accept_unknown_hosts = true;
    let r = with_options(o.clone(), || ls_remote(&c.url()));
    result(t, "unknown host + accept_unknown_hosts", msg(&r));
    assert!(reached(&r));
    // github.com listed with a wrong ed25519 key.
    o.known_hosts = vec![KnownHosts::Text {
        label: "wrong".into(),
        text: "github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ\n".into(),
    }];
    let r = with_options(o, || ls_remote(&c.url()));
    result(t, "wrong pinned key (+accept_unknown)", msg(&r));
    assert!(r.as_ref().err().is_some_and(|e| e.message().contains("MISMATCH")));
}

#[test]
fn f5_ssh_config() {
    let t = "f5_config";
    let Some(c) = ctx(t) else { return };
    let Some(key) = c.key_file("SSH_DEPLOY_KEY_ECDSA") else { return };
    let cfg = c.dir.path().join("config");
    std::fs::write(
        &cfg,
        format!(
            "Host sandbox-gh\n  HostName ssh.github.com\n  User git\n  Port 443\n  IdentityFile {}\n\nHost *\n  ServerAliveInterval 30\n",
            key.display()
        ),
    )
    .unwrap();
    let mut o = c.native(vec![]);
    o.auth = vec![AuthSource::AgentAt(if cfg!(windows) { r"\\.\pipe\none".into() } else { "/none".into() })];
    o.ssh_config = SshConfigSource::File(cfg);
    let url = format!("sandbox-gh:{}.git", c.repo);
    let r = with_options(o, || push_and_delete_tag(&url, &c.prefix, "cfg"));
    result(t, "alias -> ssh.github.com:443, User, IdentityFile (ecdsa)", format!("{r:?}"));
    assert!(reached(&r));
}

#[test]
fn f6_errors_and_stress() {
    let t = "f6_errors";
    let Some(c) = ctx(t) else { return };
    // A key GitHub doesn't know (random, in memory only).
    let rnd = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let text = rnd.to_openssh(russh::keys::ssh_key::LineEnding::LF).unwrap().to_string();
    let o = c.native(vec![AuthSource::KeyMemory { label: "random-unregistered".into(), text, passphrase: None }]);
    let r = with_options(o, || ls_remote(&c.url()));
    result(t, "unregistered key", msg(&r));
    assert_eq!(r.as_ref().err().map(|e| e.code()), Some(git2::ErrorCode::Auth));
    // A valid deploy key on a repo it has no access to (does not exist).
    let o = c.native(vec![c.mem("SSH_DEPLOY_KEY").unwrap()]);
    let r = with_options(o.clone(), || ls_remote("git@github.com:semoxide/no-such-repo-russh-poc.git"));
    result(t, "no access (other repo)", msg(&r));
    assert!(r.is_err());
    // Unreachable host: bounded by connect_timeout.
    let mut o2 = o.clone();
    o2.connect_timeout = Duration::from_secs(3);
    let t0 = Instant::now();
    let r = watchdog(30, o2, || ls_remote("ssh://git@10.255.255.1/x.git").map_err(|e| e.message().to_string()));
    result(t, "blackhole 10.255.255.1", format!("{r:?} after {:.1} s", t0.elapsed().as_secs_f64()));
    assert!(r.is_ok(), "hang");

    let n: usize = std::env::var("SANDBOX_STRESS").ok().and_then(|s| s.parse().ok()).unwrap_or(50);
    let url = c.url();
    let r = watchdog(600, o, move || stress(&url, n));
    let (ok, bad, first, avg) = r.unwrap();
    result(t, &format!("stress {n} x connect+ls-remote"), format!("{ok} ok, {bad} failed, avg {avg} ms, first error {first:?}"));
    assert_eq!(bad, 0);
}

// ---------- b1: system ssh ----------

fn exec_enabled(t: &str) -> bool {
    let on = std::env::var("SANDBOX_EXEC").as_deref() == Ok("1");
    if !on {
        println!("SKIP {t}: SANDBOX_EXEC != 1");
    }
    on
}

#[test]
fn b1_exec_scenario_and_errors() {
    let t = "b1_exec";
    let Some(c) = ctx(t) else { return };
    if !exec_enabled(t) {
        return;
    }
    let key = c.key_file("SSH_DEPLOY_KEY").unwrap();
    let kh = c.known_hosts_file();
    let q = |p: &Path| format!("\"{}\"", p.display().to_string().replace('\\', "/"));
    let base = SshOptions { backend: Backend::Exec, connect_timeout: Duration::from_secs(20), io_timeout: Duration::from_secs(60), ..Default::default() };
    let mut o = base.clone();
    o.exec_command = Some(format!("ssh -F none -i {} -o IdentitiesOnly=yes -o UserKnownHostsFile={}", q(&key), q(&kh)));
    let (url, prefix) = (c.url(), c.prefix.clone());
    let r = watchdog(300, o.clone(), move || scenario(t, &url, &prefix));
    result(t, "scenario via system ssh (key file)", format!("{r:?}"));
    assert!(scenario_or_blocked(t, &r));

    // GIT_SSH_COMMAND + an ssh config file (alias to ssh.github.com:443).
    let cfg = c.dir.path().join("config");
    std::fs::write(
        &cfg,
        format!(
            "Host sandbox-gh\n  HostName ssh.github.com\n  User git\n  Port 443\n  IdentityFile {}\n  IdentitiesOnly yes\n  UserKnownHostsFile {}\n",
            key.display().to_string().replace('\\', "/"),
            kh.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();
    // SAFETY: test binary runs with --test-threads=1.
    unsafe { std::env::set_var("GIT_SSH_COMMAND", format!("ssh -F {}", q(&cfg))) };
    let r = with_options(base.clone(), || ls_remote(&format!("sandbox-gh:{}.git", c.repo)));
    unsafe { std::env::remove_var("GIT_SSH_COMMAND") };
    result(t, "GIT_SSH_COMMAND='ssh -F cfg' + Host alias", msg(&r));
    assert!(reached(&r));

    if std::env::var("SANDBOX_SSH_AGENT").as_deref() == Ok("1") {
        let mut oa = base.clone();
        oa.exec_command = Some(format!("ssh -F none -o UserKnownHostsFile={}", q(&kh)));
        let r = with_options(oa, || ls_remote(&c.url()));
        result(t, "agent via system ssh", msg(&r));
        assert!(reached(&r));
    }

    // Unknown host: BatchMode refuses instead of prompting.
    let empty = c.dir.path().join("kh_empty");
    std::fs::write(&empty, "").unwrap();
    let mut ou = base.clone();
    ou.exec_command = Some(format!("ssh -F none -i {} -o IdentitiesOnly=yes -o UserKnownHostsFile={} -o GlobalKnownHostsFile={}", q(&key), q(&empty), q(&empty)));
    let url = c.url();
    let r = watchdog(60, ou, move || ls_remote(&url).map_err(|e| e.message().to_string()));
    result(t, "unknown host", format!("{r:?}"));
    assert!(matches!(&r, Ok(Err(e)) if e.contains("Host key verification failed")));

    // No access / bad key.
    let r = with_options(o.clone(), || ls_remote("git@github.com:semoxide/no-such-repo-russh-poc.git"));
    result(t, "no access (other repo)", msg(&r));
    let mut ob = o.clone();
    ob.exec_command = Some(format!("ssh -F none -i {} -o IdentitiesOnly=yes -o IdentityAgent=none -o UserKnownHostsFile={}", q(&kh), q(&kh)));
    let url = c.url();
    let r = watchdog(60, ob, move || ls_remote(&url).map_err(|e| e.message().to_string()));
    result(t, "bad key (not a key file)", format!("{r:?}"));

    let n = 10;
    let url = c.url();
    let (ok, bad, first, avg) = watchdog(300, o, move || stress(&url, n)).unwrap();
    result(t, &format!("stress {n} x connect+ls-remote"), format!("{ok} ok, {bad} failed, avg {avg} ms, first error {first:?}"));
}

/// Minimal container: no `ssh` binary. The exec backend must fail fast with a clear message.
#[test]
fn b1_no_ssh_binary() {
    let t = "b1_no_ssh";
    git2_russh::register().unwrap();
    if std::env::var("SANDBOX_NO_SSH").as_deref() != Ok("1") {
        println!("SKIP {t}: SANDBOX_NO_SSH != 1");
        return;
    }
    let o = SshOptions { backend: Backend::Exec, ..Default::default() };
    let r = with_options(o, || ls_remote("git@github.com:semoxide/semoxide-sandbox.git"));
    result(t, "exec backend without ssh", msg(&r));
    assert!(r.err().is_some_and(|e| e.message().contains("not found")));
}
