//! Shared helpers: git2 ops over a URL, run under given SshOptions. Used by local.rs and github.rs.
#![allow(dead_code)]

pub mod server;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use git2::{
    AutotagOption, Direction, FetchOptions, Oid, PushOptions, RemoteCallbacks, Repository, Signature, build::RepoBuilder,
};
use git2_russh::SshOptions;
use tempfile::TempDir;

pub fn os() -> &'static str {
    std::env::consts::OS
}

/// One line per observation, collected by the workflow into the README table.
pub fn result(test: &str, what: &str, detail: impl std::fmt::Display) {
    println!("RESULT|{}|{test}|{what}|{detail}", os());
}

pub fn bot() -> Signature<'static> {
    Signature::now("semoxide-sandbox-bot", "semoxide-sandbox-bot@users.noreply.github.com").unwrap()
}

pub fn commit_on(repo: &Repository, parent: Option<Oid>, path: &str, content: &str, msg: &str) -> Oid {
    let blob = repo.blob(content.as_bytes()).unwrap();
    let parent = parent.map(|p| repo.find_commit(p).unwrap());
    let mut tb = repo.treebuilder(parent.as_ref().map(|p| p.tree().unwrap()).as_ref()).unwrap();
    tb.insert(path, blob, 0o100644).unwrap();
    let tree = repo.find_tree(tb.write().unwrap()).unwrap();
    let s = bot();
    let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
    repo.commit(None, &s, &s, msg, &tree, &parents).unwrap()
}

pub type Statuses = Vec<(String, Option<String>)>;

pub fn push(repo: &Repository, url: &str, specs: &[&str]) -> Result<Statuses, git2::Error> {
    let st = Rc::new(RefCell::new(Vec::new()));
    let st2 = st.clone();
    let mut cb = RemoteCallbacks::new();
    cb.push_update_reference(move |r, s| {
        st2.borrow_mut().push((r.to_string(), s.map(str::to_string)));
        Ok(())
    });
    let mut po = PushOptions::new();
    po.remote_callbacks(cb);
    repo.remote_anonymous(url)?.push(specs, Some(&mut po))?;
    Ok(st.take())
}

pub fn ls_remote(url: &str) -> Result<Vec<(String, Oid)>, git2::Error> {
    let tmp = TempDir::new().unwrap();
    let repo = Repository::init_bare(tmp.path()).unwrap();
    let mut r = repo.remote_anonymous(url)?;
    let conn = r.connect_auth(Direction::Fetch, None, None)?;
    Ok(conn.list()?.iter().map(|h| (h.name().to_string(), h.oid())).collect())
}

pub fn has(refs: &[(String, Oid)], name: &str) -> Option<Oid> {
    refs.iter().find(|(n, _)| n == name).map(|(_, o)| *o)
}

pub fn shallow_clone(url: &str, dir: &Path) -> Result<Repository, git2::Error> {
    let mut fo = FetchOptions::new();
    fo.depth(1);
    RepoBuilder::new().fetch_options(fo).clone(url, dir)
}

pub fn unshallow(repo: &Repository, url: &str) -> Result<(), git2::Error> {
    let mut fo = FetchOptions::new();
    fo.depth(i32::MAX).download_tags(AutotagOption::All);
    repo.remote_anonymous(url)?.fetch(&["+refs/heads/*:refs/remotes/origin/*", "+refs/tags/*:refs/tags/*"], Some(&mut fo), None)
}

fn count_commits(repo: &Repository) -> usize {
    let mut w = repo.revwalk().unwrap();
    w.push_head().unwrap();
    w.count()
}

/// The full option-f scenario against `url` (an existing repo with >= 2 commits on its default branch):
/// shallow clone, unshallow, push lw tag / annotated tag / notes ref / branch (one push each),
/// ls-remote, fetch into a fresh repo, delete everything again. Returns Err(text) on the first failure.
pub fn scenario(test: &str, url: &str, prefix: &str) -> Result<(), String> {
    let e = |what: &str| {
        let what = what.to_string();
        move |err: git2::Error| format!("{what}: {err} (class={:?} code={:?})", err.class(), err.code())
    };
    let tmp = TempDir::new().unwrap();
    let t0 = Instant::now();
    let r = shallow_clone(url, &tmp.path().join("c")).map_err(e("shallow clone"))?;
    let shallow_n = count_commits(&r);
    if !r.is_shallow() || shallow_n != 1 {
        return Err(format!("expected a depth-1 shallow clone, got is_shallow={} commits={shallow_n}", r.is_shallow()));
    }
    unshallow(&r, url).map_err(e("unshallow"))?;
    let full_n = count_commits(&r);
    if r.is_shallow() || full_n < 2 {
        return Err(format!("unshallow failed: is_shallow={} commits={full_n}", r.is_shallow()));
    }
    result(test, "shallow+unshallow", format!("ok: 1 -> {full_n} commits, is_shallow=false"));

    let head = r.head().unwrap().target().unwrap();
    let c1 = commit_on(&r, Some(head), &format!("{prefix}.txt"), "x\n", "test: russh sandbox commit");
    let obj = r.find_object(c1, None).unwrap();
    let (lw, ann, br) = (format!("{prefix}lw"), format!("{prefix}ann"), format!("{prefix}br"));
    r.tag_lightweight(&lw, &obj, false).unwrap();
    r.tag(&ann, &obj, &bot(), "release test\n", false).unwrap();
    r.branch(&br, &r.find_commit(c1).unwrap(), false).unwrap();
    let nref = format!("refs/notes/semantic-release-{ann}");
    r.note(&bot(), &bot(), Some(&nref), c1, r#"{"channels":[null]}"#, true).unwrap();
    let refs = [format!("refs/tags/{lw}"), format!("refs/tags/{ann}"), nref.clone(), format!("refs/heads/{br}")];
    for rf in &refs {
        let spec = format!("{rf}:{rf}");
        let st = push(&r, url, &[&spec]).map_err(e(&format!("push {rf}")))?;
        if st != vec![(rf.clone(), None)] {
            return Err(format!("push {rf}: per-ref status {st:?}"));
        }
    }
    result(test, "push lw+ann tag, notes, branch", "ok: 4 pushes, per-ref status None");

    let listed = ls_remote(url).map_err(e("ls-remote"))?;
    for rf in &refs {
        if has(&listed, rf).is_none() {
            return Err(format!("ls-remote: {rf} missing"));
        }
    }
    if has(&listed, &format!("refs/tags/{ann}^{{}}")) != Some(c1) {
        return Err("ls-remote: annotated tag not peeled to the commit".into());
    }
    result(test, "ls-remote", format!("ok: {} refs incl. the 4 new + peeled tag", listed.len()));

    let tmp2 = TempDir::new().unwrap();
    let fresh = Repository::init_bare(tmp2.path()).unwrap();
    let spec = format!("+refs/heads/{br}:refs/heads/{br}");
    fresh
        .remote_anonymous(url)
        .and_then(|mut rm| rm.fetch(&[spec.as_str(), "+refs/notes/*:refs/notes/*"], None, None))
        .map_err(e("fetch"))?;
    if fresh.refname_to_id(&format!("refs/heads/{br}")).ok() != Some(c1) || fresh.refname_to_id(&nref).is_err() {
        return Err("fetch: branch / notes ref not fetched".into());
    }
    result(test, "fetch branch+notes", "ok");

    let dels: Vec<String> = refs.iter().map(|rf| format!(":{rf}")).collect();
    let dels: Vec<&str> = dels.iter().map(String::as_str).collect();
    let st = push(&r, url, &dels).map_err(e("delete refs"))?;
    if st.iter().any(|(_, s)| s.is_some()) {
        return Err(format!("delete: {st:?}"));
    }
    let after = ls_remote(url).map_err(e("ls-remote after delete"))?;
    if refs.iter().any(|rf| has(&after, rf).is_some()) {
        return Err("refs still present after delete".into());
    }
    result(test, "delete refs", format!("ok; whole scenario {:.1} s", t0.elapsed().as_secs_f64()));
    Ok(())
}

/// GitHub answers "Repository not found" *after* a successful SSH handshake + auth + exec when the
/// org disables deploy keys (`deploy_keys_enabled_for_repositories: false`). That still proves the
/// transport end to end up to the git service; only repo access is refused.
pub fn denied(msg: &str) -> bool {
    msg.contains("Repository not found")
}

/// N x (connect + ls-remote). Returns (ok, failures, first error text, avg ms). A `denied` answer
/// counts as ok (handshake, auth and exec all completed).
pub fn stress(url: &str, n: usize) -> (usize, usize, Option<String>, u128) {
    let (mut ok, mut bad, mut first) = (0, 0, None);
    let t0 = Instant::now();
    for _ in 0..n {
        match ls_remote(url) {
            Ok(_) => ok += 1,
            Err(e) if denied(e.message()) => ok += 1,
            Err(e) => {
                bad += 1;
                first.get_or_insert(e.message().to_string());
            }
        }
    }
    (ok, bad, first, t0.elapsed().as_millis() / n.max(1) as u128)
}

/// Run `f` on a thread with a watchdog; a hang becomes Err("HANG").
pub fn watchdog<T: Send + 'static>(secs: u64, opts: SshOptions, f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(git2_russh::with_options(opts, f));
    });
    rx.recv_timeout(Duration::from_secs(secs)).map_err(|_| format!("HANG (> {secs} s)"))
}

// ---------- keys (generated with ssh-keygen into a TempDir that is deleted afterwards) ----------

pub struct Keys {
    pub dir: TempDir,
}

impl Keys {
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    pub fn text(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }
    pub fn public(&self, name: &str) -> russh::keys::PublicKey {
        russh::keys::load_public_key(self.path(&format!("{name}.pub"))).unwrap()
    }
}

pub fn ssh_keygen() -> Option<String> {
    let candidates = if cfg!(windows) {
        vec![r"C:\Windows\System32\OpenSSH\ssh-keygen.exe", r"C:\Program Files\Git\usr\bin\ssh-keygen.exe", "ssh-keygen"]
    } else {
        vec!["ssh-keygen"]
    };
    candidates.into_iter().map(String::from).find(|c| Command::new(c).arg("-?").output().is_ok())
}

/// ed25519, ecdsa (p256), rsa (OpenSSH format), rsa_pem (PKCS#1), ed25519_pass (passphrase "pw"), host (ed25519).
pub fn gen_keys() -> Option<Keys> {
    let kg = ssh_keygen()?;
    let dir = TempDir::new().unwrap();
    let specs: [(&str, &[&str], &str); 6] = [
        ("ed25519", &["-t", "ed25519"], ""),
        ("ecdsa", &["-t", "ecdsa", "-b", "256"], ""),
        ("rsa", &["-t", "rsa", "-b", "3072"], ""),
        ("rsa_pem", &["-t", "rsa", "-b", "3072", "-m", "PEM"], ""),
        ("ed25519_pass", &["-t", "ed25519"], "pw"),
        ("host", &["-t", "ed25519"], ""),
    ];
    for (name, args, pass) in specs {
        let st = Command::new(&kg)
            .args(args)
            .args(["-q", "-C", "git2-russh-test", "-N", pass, "-f"])
            .arg(dir.path().join(name))
            .status()
            .unwrap();
        assert!(st.success(), "ssh-keygen {name}");
    }
    Some(Keys { dir })
}
