//! Shared fixtures: a bare "remote" + working clone over file:// URLs.
#![allow(dead_code)]

use git2::{
    Cred, CredentialType, Direction, FetchOptions, Oid, PushOptions, RemoteCallbacks, Repository,
    Signature, Time,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct Fixture {
    pub _tmp: TempDir,
    pub remote_path: PathBuf,
    pub url: String,
    pub work: Repository,
}

/// `file:///C:/...` on Windows, `file:///tmp/...` elsewhere.
pub fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// Deterministic signature; `t` seconds since epoch so log order is controlled.
pub fn sig(t: i64) -> Signature<'static> {
    Signature::new("Test User", "test@example.com", &Time::new(t, 0)).unwrap()
}

pub fn bot() -> Signature<'static> {
    Signature::now("semantic-release-bot", "semantic-release-bot@martynus.net").unwrap()
}

/// Write `file`, stage it, commit on HEAD with commit time `t`.
pub fn commit(repo: &Repository, file: &str, content: &str, msg: &str, t: i64) -> Oid {
    let wd = repo.workdir().unwrap();
    let p = wd.join(file);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(&p, content).unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new(file)).unwrap();
    idx.write().unwrap();
    let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
    let parents: Vec<_> = repo.head().ok().and_then(|h| h.peel_to_commit().ok()).into_iter().collect();
    let parents: Vec<_> = parents.iter().collect();
    let s = sig(t);
    repo.commit(Some("HEAD"), &s, &s, msg, &tree, &parents).unwrap()
}

/// Bare remote with one commit on `main`, plus a working clone of it.
pub fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let remote = Repository::init_opts(
        &remote_path,
        git2::RepositoryInitOptions::new().bare(true).initial_head("main"),
    )
    .unwrap();
    // Seed the bare repo through a throwaway non-bare repo + push.
    let seed = init_work(&tmp.path().join("seed"));
    commit(&seed, "README.md", "seed\n", "chore: init", 1_000);
    seed.remote("origin", &file_url(&remote_path)).unwrap();
    push(&seed, "origin", &["refs/heads/main:refs/heads/main"]).unwrap();
    drop(remote);
    let url = file_url(&remote_path);
    let work = Repository::clone(&url, tmp.path().join("work")).unwrap();
    Fixture { _tmp: tmp, remote_path, url, work }
}

pub fn init_work(p: &Path) -> Repository {
    Repository::init_opts(p, git2::RepositoryInitOptions::new().initial_head("main")).unwrap()
}

pub fn remote_repo(f: &Fixture) -> Repository {
    Repository::open_bare(&f.remote_path).unwrap()
}

/// Push refspecs; returns per-ref (refname, rejection message) from the server report.
pub fn push(repo: &Repository, remote: &str, specs: &[&str]) -> Result<Vec<(String, Option<String>)>, git2::Error> {
    let statuses = RefCell::new(Vec::new());
    {
        let mut cb = RemoteCallbacks::new();
        cb.push_update_reference(|r, s| {
            statuses.borrow_mut().push((r.to_string(), s.map(str::to_string)));
            Ok(())
        });
        let mut opts = PushOptions::new();
        opts.remote_callbacks(cb);
        repo.find_remote(remote)?.push(specs, Some(&mut opts))?;
    }
    Ok(statuses.into_inner())
}

pub fn fetch(repo: &Repository, url: &str, specs: &[&str], tags: git2::AutotagOption, depth: Option<i32>) -> Result<(), git2::Error> {
    let mut remote = repo.remote_anonymous(url)?;
    let mut fo = FetchOptions::new();
    fo.download_tags(tags);
    if let Some(d) = depth {
        fo.depth(d);
    }
    remote.fetch(specs, Some(&mut fo), None)
}

/// ls-remote: (refname, oid) of every advertised ref.
pub fn ls_remote(repo: &Repository, url: &str) -> Vec<(String, Oid)> {
    let mut r = repo.remote_anonymous(url).unwrap();
    r.connect(Direction::Fetch).unwrap();
    let out = r.list().unwrap().iter().map(|h| (h.name().to_string(), h.oid())).collect();
    r.disconnect().unwrap();
    out
}

pub fn ref_oid(repo: &Repository, name: &str) -> Option<Oid> {
    repo.refname_to_id(name).ok()
}

/// The product's credential callback. HTTPS: GitHub-App style `x-access-token:<token>`.
/// SSH: agent, user from the URL (default `git`). Gives up after `max_tries` calls so a
/// bad token does not loop forever on 401.
pub fn make_cred_cb(token: Option<String>, max_tries: usize) -> impl FnMut(&str, Option<&str>, CredentialType) -> Result<Cred, git2::Error> {
    let mut tries = 0;
    move |_url, user_from_url, allowed| {
        tries += 1;
        if tries > max_tries {
            return Err(git2::Error::from_str("authentication failed: giving up"));
        }
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT)
            && let Some(t) = &token
        {
            return Cred::userpass_plaintext("x-access-token", t);
        }
        if allowed.contains(CredentialType::SSH_KEY) {
            return Cred::ssh_key_from_agent(user_from_url.unwrap_or("git"));
        }
        if allowed.contains(CredentialType::USERNAME) {
            return Cred::username(user_from_url.unwrap_or("git"));
        }
        Err(git2::Error::from_str("no supported credential type"))
    }
}

pub fn git_cli_available() -> bool {
    std::process::Command::new("git").arg("--version").output().is_ok()
}

/// Test-only cross-check against real git (never used by the ops themselves).
pub fn git_cli(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// `git tag -l` (sorted by name, like libgit2 returns them).
pub fn tag_list(repo: &Repository) -> Vec<String> {
    repo.tag_names(None).unwrap().iter().map(|t| t.unwrap().unwrap().to_string()).collect()
}
