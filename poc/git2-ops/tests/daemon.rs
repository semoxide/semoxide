//! Ops that need a real network transport (libgit2's local file:// transport can't do them):
//! shallow clone/fetch/unshallow (G8, G9) and server-side push rejection.
//! The *server* is `git daemon` (test infra only, like GitHub would be); every client op is git2.
//! Skipped (with a message) if git isn't on PATH.
mod common;
use common::*;

use git2::{AutotagOption, FetchOptions, Repository, build::RepoBuilder};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

struct Daemon {
    child: Child,
    port: u16,
}
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn daemon(base: &std::path::Path) -> Daemon {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    // Spawn git-daemon directly: on Windows `git daemon` is a wrapper whose child survives kill().
    let exec = git_cli(base, &["--exec-path"]).trim().to_string();
    let child = Command::new(std::path::Path::new(&exec).join("git-daemon"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .args(["--reuseaddr", "--export-all", "--enable=receive-pack", "--listen=127.0.0.1"])
        .arg(format!("--port={port}"))
        .arg(format!("--base-path={}", base.display()))
        .arg(base)
        .spawn()
        .unwrap();
    let t = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(t.elapsed() < Duration::from_secs(10), "git daemon did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    Daemon { child, port }
}

/// Remote with 5 commits on main, tags v1..v5 (v3 annotated), and a notes ref.
fn rich_fixture() -> (Fixture, usize) {
    let f = fixture();
    let r = &f.work;
    for i in 1..=4 {
        commit(r, &format!("f{i}"), "x", &format!("feat: {i}"), 1_000 + i * 100);
    }
    let mut walk = r.revwalk().unwrap();
    walk.push_head().unwrap();
    let all: Vec<_> = walk.map(Result::unwrap).collect();
    for (i, c) in all.iter().rev().enumerate() {
        let obj = r.find_object(*c, None).unwrap();
        let name = format!("v{}.0.0", i + 1);
        if i == 2 { r.tag(&name, &obj, &sig(5_000), "annotated\n", false).unwrap(); } else { r.tag_lightweight(&name, &obj, false).unwrap(); }
    }
    r.note(&bot(), &bot(), Some("refs/notes/semantic-release-v5.0.0"), all[0], r#"{"channels":[null]}"#, true).unwrap();
    let specs: Vec<String> = ["refs/heads/main:refs/heads/main".to_string(), "refs/notes/semantic-release-v5.0.0".into()]
        .into_iter()
        .chain((1..=5).map(|i| format!("refs/tags/v{i}.0.0")))
        .collect();
    let specs: Vec<&str> = specs.iter().map(String::as_str).collect();
    push(r, "origin", &specs).unwrap();
    let n = all.len();
    (f, n)
}

fn count_history(r: &Repository) -> usize {
    let mut w = r.revwalk().unwrap();
    w.push_head().unwrap();
    w.count()
}

// G8a/G9a: depth-1 CI clone -> fetch tags -> unshallow -> fetch notes, all via git2 over git://
#[test]
fn g08_g09_shallow_clone_fetch_unshallow() {
    if !git_cli_available() {
        eprintln!("SKIP: git (daemon) not available");
        return;
    }
    let (f, total) = rich_fixture();
    let d = daemon(f._tmp.path());
    let url = format!("git://127.0.0.1:{}/remote.git", d.port);

    // Shallow clone (what actions/checkout does by default).
    let mut fo = FetchOptions::new();
    fo.depth(1);
    let dst = f._tmp.path().join("ci");
    let ci = RepoBuilder::new().fetch_options(fo).clone(&url, &dst).unwrap();
    assert!(ci.is_shallow());
    assert_eq!(count_history(&ci), 1);
    let shallow = std::fs::read_to_string(ci.path().join("shallow")).unwrap();
    assert_eq!(shallow.lines().count(), 1);

    // G8a: `git fetch --unshallow --tags URL` == depth(i32::MAX) + tags refspec + AutotagOption::All
    fetch(&ci, &url, &["+refs/tags/*:refs/tags/*"], AutotagOption::All, Some(i32::MAX)).unwrap();
    assert!(!ci.is_shallow(), "unshallowed");
    assert!(!ci.path().join("shallow").exists());
    assert_eq!(count_history(&ci), total);
    assert_eq!(tag_list(&ci), ["v1.0.0", "v2.0.0", "v3.0.0", "v4.0.0", "v5.0.0"]);
    // annotated tag object arrived and peels
    assert_eq!(ci.revparse_single("v3.0.0").unwrap().kind(), Some(git2::ObjectType::Tag));

    // G8c: unshallow on an already-complete repo. git errors ("--unshallow on a complete repository
    // does not make sense"), hence sr's fallback. libgit2: record what it does.
    let again = fetch(&ci, &url, &["+refs/tags/*:refs/tags/*"], AutotagOption::All, Some(i32::MAX));
    eprintln!("unshallow on complete repo: {again:?}");
    assert!(again.is_ok(), "libgit2 tolerates unshallow on a complete repo (no fallback needed)");

    // G9: notes
    fetch(&ci, &url, &["+refs/notes/*:refs/notes/*"], AutotagOption::None, None).unwrap();
    let head = ci.head().unwrap().target().unwrap();
    assert!(ci.find_note(Some("refs/notes/semantic-release-v5.0.0"), head).is_ok());

    // Cross-check with real git: repo is complete and consistent.
    let fsck = Command::new("git").current_dir(&dst).args(["fsck", "--no-progress"]).output().unwrap();
    assert!(fsck.status.success(), "{}", String::from_utf8_lossy(&fsck.stderr));
    assert_eq!(git_cli(&dst, &["rev-parse", "--is-shallow-repository"]).trim(), "false");
}

// G8b on a shallow repo with the branch checked out: deepen + force-update local branch.
#[test]
fn g08b_shallow_fetch_into_checked_out_branch() {
    if !git_cli_available() {
        eprintln!("SKIP: git (daemon) not available");
        return;
    }
    let (f, total) = rich_fixture();
    let d = daemon(f._tmp.path());
    let url = format!("git://127.0.0.1:{}/remote.git", d.port);
    let mut fo = FetchOptions::new();
    fo.depth(1);
    let ci = RepoBuilder::new().fetch_options(fo).clone(&url, &f._tmp.path().join("ci")).unwrap();
    // Remote moves ahead.
    let c = commit(&f.work, "new", "n", "feat: new", 9_000);
    push(&f.work, "origin", &["refs/heads/main:refs/heads/main"]).unwrap();

    fetch(&ci, &url, &["+refs/heads/main:refs/heads/main"], AutotagOption::All, Some(i32::MAX)).unwrap();
    assert_eq!(ref_oid(&ci, "refs/heads/main"), Some(c));
    assert!(!ci.is_shallow());
    assert_eq!(count_history(&ci), total + 1);
}

// libgit2 shallow over the local file:// transport: documents libgit2#6634.
#[test]
fn shallow_over_file_url_is_unsupported() {
    let (f, _) = rich_fixture();
    let mut fo = FetchOptions::new();
    fo.depth(1);
    let res = RepoBuilder::new().fetch_options(fo).clone(&f.url, &f._tmp.path().join("ci"));
    let e = res.err().expect("if this clones, libgit2 fixed local shallow (#6634): update README");
    assert!(e.message().contains("shallow fetch is not supported by the local transport"), "{e}");
}

// Server-side rejection (pre-receive hook ~ branch protection) arrives via push_update_reference.
#[test]
fn push_server_side_rejection_reported_per_ref() {
    if !git_cli_available() {
        eprintln!("SKIP: git (daemon) not available");
        return;
    }
    let f = fixture();
    let hook = f.remote_path.join("hooks").join("pre-receive");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\necho 'protected branch' >&2\nexit 1\n").unwrap();
    let d = daemon(f._tmp.path());
    let url = format!("git://127.0.0.1:{}/remote.git", d.port);
    f.work.remote("daemon", &url).unwrap();
    commit(&f.work, "a", "a", "feat: a", 2_000);
    let res = push(&f.work, "daemon", &["refs/heads/main:refs/heads/main"]);
    eprintln!("server-rejected push: {res:?}");
    let st = res.unwrap();
    assert_eq!(st.len(), 1);
    assert_eq!(st[0].0, "refs/heads/main");
    assert!(st[0].1.is_some(), "rejection message expected, got {st:?}");
}
