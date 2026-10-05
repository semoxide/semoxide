//! One test per semantic-release git op (G#), all over file:// fixtures.
mod common;
use common::*;

use git2::{
    AutotagOption, BranchType, Direction, IndexAddOption, ObjectType, Oid, PushOptions, Reference,
    RemoteCallbacks, Repository, RepositoryOpenFlags, Sort, StatusOptions,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;

// G2 `git config --get remote.origin.url`
#[test]
fn g02_remote_origin_url() {
    let f = fixture();
    assert_eq!(f.work.find_remote("origin").unwrap().url().unwrap(), f.url);
    let cfg = f.work.config().unwrap().snapshot().unwrap();
    assert_eq!(cfg.get_str("remote.origin.url").unwrap(), f.url);
}

// G3 `git rev-parse --git-dir` (walks up)
#[test]
fn g03_is_git_repo() {
    let f = fixture();
    let sub = f.work.workdir().unwrap().join("a/b");
    std::fs::create_dir_all(&sub).unwrap();
    let found = Repository::discover(&sub).unwrap();
    assert_eq!(found.path(), f.work.path());
    // Non-repo: ceiling at its own parent so a repo above %TEMP% cannot leak in.
    let tmp = tempfile::TempDir::new().unwrap();
    let r = Repository::open_ext(tmp.path(), RepositoryOpenFlags::empty(), [tmp.path().parent().unwrap()]);
    assert!(r.is_err());
}

// G4 / G12 `git check-ref-format refs/tags/<t>` / `refs/heads/<b>`
#[test]
fn g04_g12_check_ref_format() {
    assert!(Reference::is_valid_name("refs/tags/v0.0.0"));
    assert!(Reference::is_valid_name("refs/heads/release/1.x"));
    for bad in ["refs/tags/v1..0", "refs/tags/v1 0", "refs/heads/a~b", "refs/heads/x.lock", "refs/tags/@{x}"] {
        assert!(!Reference::is_valid_name(bad), "{bad}");
    }
}

// G5 `git push --dry-run HEAD:<branch>`: push with a push_negotiation callback that aborts
// after the server's ref advertisement and the update commands are computed, before any pack is sent.
#[test]
fn g05_push_dry_run_via_negotiation_abort() {
    let f = fixture();
    let new = commit(&f.work, "a.txt", "a", "feat: a", 2_000);
    let before = ref_oid(&remote_repo(&f), "refs/heads/main");
    let seen = RefCell::new(Vec::new());
    let res = {
        let mut cb = RemoteCallbacks::new();
        cb.push_negotiation(|ups| {
            for u in ups {
                seen.borrow_mut().push((u.dst_refname().unwrap().to_string(), u.src(), u.dst()));
            }
            Err(git2::Error::from_str("semoxide-dry-run"))
        });
        let mut po = PushOptions::new();
        po.remote_callbacks(cb);
        f.work.remote_anonymous(&f.url).unwrap().push(&["HEAD:refs/heads/main"], Some(&mut po))
    };
    let err = res.unwrap_err();
    assert!(err.message().contains("semoxide-dry-run"), "{err}");
    assert_eq!(seen.into_inner(), vec![("refs/heads/main".to_string(), before.unwrap(), new)]);
    assert_eq!(ref_oid(&remote_repo(&f), "refs/heads/main"), before, "dry run must not update");
}

// G6 `git ls-remote --heads URL`
#[test]
fn g06_ls_remote_heads() {
    let f = fixture();
    commit(&f.work, "a", "a", "x", 2_000);
    push(&f.work, "origin", &["HEAD:refs/heads/next"]).unwrap();
    let heads: Vec<_> = ls_remote(&f.work, &f.url).into_iter().filter(|(n, _)| n.starts_with("refs/heads/")).map(|(n, _)| n).collect();
    assert_eq!(heads, vec!["refs/heads/main", "refs/heads/next"]);
}

// G7 `git rev-parse --abbrev-ref HEAD` (== "HEAD" when detached)
#[test]
fn g07_abbrev_ref_head() {
    let f = fixture();
    assert_eq!(f.work.head().unwrap().shorthand().unwrap(), "main");
    assert!(!f.work.head_detached().unwrap());
    let c = f.work.head().unwrap().target().unwrap();
    f.work.set_head_detached(c).unwrap();
    assert!(f.work.head_detached().unwrap());
    assert_eq!(f.work.head().unwrap().name().unwrap(), "HEAD");
}

// G8a `git fetch --tags URL` (non-shallow repo; shallow variant lives in tests/shallow.rs)
#[test]
fn g08a_fetch_tags() {
    let f = fixture();
    let other = Repository::clone(&f.url, f._tmp.path().join("other")).unwrap();
    let c = commit(&other, "b", "b", "feat: b", 2_000);
    other.tag_lightweight("v1.0.0", &other.find_object(c, None).unwrap(), false).unwrap();
    push(&other, "origin", &["refs/heads/main:refs/heads/main", "refs/tags/v1.0.0:refs/tags/v1.0.0"]).unwrap();

    fetch(&f.work, &f.url, &["+refs/tags/*:refs/tags/*"], AutotagOption::All, None).unwrap();
    assert_eq!(ref_oid(&f.work, "refs/tags/v1.0.0"), Some(c));
    // Fetching tags must not move HEAD / the checked-out branch.
    assert_ne!(f.work.head().unwrap().target(), Some(c));
}

// G8b `git fetch --update-head-ok URL +refs/heads/B:refs/heads/B` while B is checked out.
#[test]
fn g08b_fetch_into_checked_out_branch() {
    let f = fixture();
    let other = Repository::clone(&f.url, f._tmp.path().join("other")).unwrap();
    let c = commit(&other, "b", "b", "feat: b", 2_000);
    push(&other, "origin", &["refs/heads/main:refs/heads/main"]).unwrap();
    assert_eq!(f.work.head().unwrap().shorthand().unwrap(), "main");

    fetch(&f.work, &f.url, &["+refs/heads/main:refs/heads/main"], AutotagOption::None, None).unwrap();
    // libgit2 has no "refusing to fetch into current branch" guard: ref moves, worktree does not
    // (exactly what --update-head-ok does).
    assert_eq!(ref_oid(&f.work, "refs/heads/main"), Some(c));
    assert!(!f.work.workdir().unwrap().join("b").exists());
}

// G9a/b `git fetch URL +refs/notes/*:refs/notes/*`
#[test]
fn g09_fetch_notes() {
    let f = fixture();
    let other = Repository::clone(&f.url, f._tmp.path().join("other")).unwrap();
    let head = other.head().unwrap().target().unwrap();
    other.note(&bot(), &bot(), Some("refs/notes/semantic-release-v1.0.0"), head, r#"{"channels":[null]}"#, true).unwrap();
    push(&other, "origin", &["refs/notes/semantic-release-v1.0.0"]).unwrap();

    fetch(&f.work, &f.url, &["+refs/notes/*:refs/notes/*"], AutotagOption::None, None).unwrap();
    let n = f.work.find_note(Some("refs/notes/semantic-release-v1.0.0"), head).unwrap();
    assert_eq!(n.message().unwrap(), r#"{"channels":[null]}"#);
}

// G10 tag -> note map: read `refs/notes/semantic-release-<tag>` per tag (+ legacy shared ref).
#[test]
fn g10_read_tag_notes() {
    let f = fixture();
    let r = &f.work;
    let c1 = r.head().unwrap().target().unwrap();
    let c2 = commit(r, "x", "x", "feat: x", 2_000);
    r.tag_lightweight("v1.0.0", &r.find_object(c1, None).unwrap(), false).unwrap();
    r.tag_lightweight("v1.1.0", &r.find_object(c2, None).unwrap(), false).unwrap();
    r.note(&bot(), &bot(), Some("refs/notes/semantic-release"), c1, r#"{"channels":["legacy"]}"#, true).unwrap();
    r.note(&bot(), &bot(), Some("refs/notes/semantic-release-v1.1.0"), c2, r#"{"channels":["next"]}"#, true).unwrap();

    let mut map = Vec::new();
    for t in tag_list(r) {
        let commit = r.revparse_single(&t).unwrap().peel_to_commit().unwrap().id();
        let note = [format!("refs/notes/semantic-release-{t}"), "refs/notes/semantic-release".into()]
            .iter()
            .find_map(|nr| r.find_note(Some(nr), commit).ok().map(|n| n.message().unwrap().to_string()));
        map.push((t, note));
    }
    assert_eq!(map, vec![
        ("v1.0.0".into(), Some(r#"{"channels":["legacy"]}"#.into())),
        ("v1.1.0".into(), Some(r#"{"channels":["next"]}"#.into())),
    ]);
}

// G11 `git tag --merged <branch>`
#[test]
fn g11_tags_merged_into_head() {
    let f = fixture();
    let r = &f.work;
    let base = r.head().unwrap().target().unwrap();
    let on_main = commit(r, "m", "m", "feat: m", 2_000);
    // side branch commit, not reachable from main
    let side_tree = r.find_commit(base).unwrap().tree().unwrap();
    let side = r.commit(None, &sig(3_000), &sig(3_000), "side", &side_tree, &[&r.find_commit(base).unwrap()]).unwrap();
    r.tag_lightweight("v1.0.0", &r.find_object(base, None).unwrap(), false).unwrap();
    r.tag(
        "v1.1.0",
        &r.find_object(on_main, None).unwrap(),
        &sig(2_500),
        "annotated",
        false,
    )
    .unwrap();
    r.tag_lightweight("v9.9.9-side", &r.find_object(side, None).unwrap(), false).unwrap();

    // One ancestry walk into a set, then peel each tag (O(history + tags)).
    let mut walk = r.revwalk().unwrap();
    walk.push_ref("refs/heads/main").unwrap();
    let reachable: HashSet<Oid> = walk.map(Result::unwrap).collect();
    let merged: Vec<String> = tag_list(r)
        .into_iter()
        .filter(|t| reachable.contains(&r.revparse_single(t).unwrap().peel_to_commit().unwrap().id()))
        .collect();
    assert_eq!(merged, vec!["v1.0.0", "v1.1.0"]);
    if git_cli_available() {
        let cli: Vec<String> = git_cli(r.workdir().unwrap(), &["tag", "--merged", "main"]).lines().map(str::to_string).collect();
        assert_eq!(merged, cli);
    }
}

// G13 `git ls-remote --heads URL <branch>` + G14: isBranchUpToDate
#[test]
fn g13_is_branch_up_to_date() {
    let f = fixture();
    let remote_tip = |f: &Fixture| {
        ls_remote(&f.work, &f.url).into_iter().find(|(n, _)| n == "refs/heads/main").map(|(_, o)| o).unwrap()
    };
    let head = f.work.head().unwrap().target().unwrap();
    assert_eq!(remote_tip(&f), head, "up to date");

    // Someone else pushes: remote moves ahead, we are behind.
    let other = Repository::clone(&f.url, f._tmp.path().join("other")).unwrap();
    commit(&other, "b", "b", "feat: b", 2_000);
    push(&other, "origin", &["refs/heads/main:refs/heads/main"]).unwrap();
    let tip = remote_tip(&f);
    assert_ne!(tip, head);
    // sr semantics: up to date iff remote tip is an ancestor of (or equal to) HEAD.
    // Remote tip is unknown locally until fetched.
    assert!(f.work.find_commit(tip).is_err());
    fetch(&f.work, &f.url, &["+refs/heads/main:refs/remotes/origin/main"], AutotagOption::None, None).unwrap();
    assert!(!f.work.graph_descendant_of(head, tip).unwrap());
    // Local branch list (G-plugin needs) + remote-tracking list.
    let locals: Vec<_> = f.work.branches(Some(BranchType::Local)).unwrap().map(|b| b.unwrap().0.name().unwrap().unwrap().to_string()).collect();
    let remotes: Vec<_> = f.work.branches(Some(BranchType::Remote)).unwrap().map(|b| b.unwrap().0.name().unwrap().unwrap().to_string()).collect();
    assert_eq!(locals, vec!["main"]);
    assert!(remotes.contains(&"origin/main".to_string()));
}

// G14 `git rev-parse HEAD`
#[test]
fn g14_head_sha() {
    let f = fixture();
    let c = commit(&f.work, "a", "a", "feat: a", 2_000);
    assert_eq!(f.work.head().unwrap().peel_to_commit().unwrap().id(), c);
    assert_eq!(f.work.revparse_single("HEAD").unwrap().id(), c);
}

// G15 `git rev-list -1 <tag>`: lightweight and annotated tag -> commit
#[test]
fn g15_tag_to_commit() {
    let f = fixture();
    let r = &f.work;
    let c = r.head().unwrap().target().unwrap();
    r.tag_lightweight("v1.0.0", &r.find_object(c, None).unwrap(), false).unwrap();
    let tag_obj = r.tag("v2.0.0", &r.find_object(c, None).unwrap(), &sig(5), "rel", false).unwrap();
    assert_eq!(r.revparse_single("v1.0.0").unwrap().peel_to_commit().unwrap().id(), c);
    let at = r.revparse_single("v2.0.0").unwrap();
    assert_eq!(at.kind(), Some(ObjectType::Tag));
    assert_eq!(at.id(), tag_obj, "G22 show-ref would return this (tag object sha)");
    assert_eq!(at.peel_to_commit().unwrap().id(), c);
    // via the ref directly
    assert_eq!(r.find_reference("refs/tags/v2.0.0").unwrap().peel_to_commit().unwrap().id(), c);
}

// G16 `git log [from..]to` with full messages, git's default order
#[test]
fn g16_log_range() {
    let f = fixture();
    let r = &f.work;
    let from = r.head().unwrap().target().unwrap();
    r.tag_lightweight("v1.0.0", &r.find_object(from, None).unwrap(), false).unwrap();
    commit(r, "a", "a", "feat: a\n\nbody line 1\n\nBREAKING CHANGE: x\n", 2_000);
    // side branch + merge
    let a = r.head().unwrap().target().unwrap();
    let br = r.find_commit(a).unwrap();
    let tb = r.find_tree({
        let mut tb = r.treebuilder(Some(&br.tree().unwrap())).unwrap();
        tb.insert("side", r.blob(b"s").unwrap(), 0o100644).unwrap();
        tb.write().unwrap()
    }).unwrap();
    let side = r.commit(None, &sig(2_500), &sig(2_500), "fix: side", &tb, &[&br]).unwrap();
    commit(r, "b", "b", "fix: b", 3_000);
    let main_tip = r.find_commit(r.head().unwrap().target().unwrap()).unwrap();
    let merged_tree = {
        let mut tb = r.treebuilder(Some(&main_tip.tree().unwrap())).unwrap();
        tb.insert("side", r.blob(b"s").unwrap(), 0o100644).unwrap();
        tb.write().unwrap()
    };
    let to = r
        .commit(Some("HEAD"), &sig(4_000), &sig(4_000), "Merge side", &r.find_tree(merged_tree).unwrap(), &[&main_tip, &r.find_commit(side).unwrap()])
        .unwrap();
    r.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();

    let mut walk = r.revwalk().unwrap();
    walk.set_sorting(Sort::TIME).unwrap();
    walk.push(to).unwrap();
    walk.hide(r.revparse_single("v1.0.0").unwrap().peel_to_commit().unwrap().id()).unwrap();
    let commits: Vec<_> = walk.map(|o| r.find_commit(o.unwrap()).unwrap()).collect();
    let subjects: Vec<_> = commits.iter().map(|c| c.summary().unwrap().unwrap().to_string()).collect();
    assert_eq!(subjects, ["Merge side", "fix: b", "fix: side", "feat: a"]);
    let feat = commits.last().unwrap();
    assert_eq!(feat.message().unwrap(), "feat: a\n\nbody line 1\n\nBREAKING CHANGE: x\n");
    // libgit2 trims trailing newlines from body(); git's %b keeps them (sr trims anyway).
    assert_eq!(feat.body().unwrap().unwrap(), "body line 1\n\nBREAKING CHANGE: x");
    assert_eq!(feat.author().name().unwrap(), "Test User");
    assert_eq!(feat.committer().when().seconds(), 2_000);
    let _tree = feat.tree_id();
    if git_cli_available() {
        let ids: Vec<String> = commits.iter().map(|c| c.id().to_string()).collect();
        let cli: Vec<String> = git_cli(r.workdir().unwrap(), &["log", "--format=%H", "v1.0.0..HEAD"]).lines().map(str::to_string).collect();
        assert_eq!(ids, cli, "same order as `git log`");
    }
}

// G17 `git tag <tag> <sha>` (lightweight) + annotated variant
#[test]
fn g17_create_tags() {
    let f = fixture();
    let r = &f.work;
    let c = r.head().unwrap().target().unwrap();
    let obj = r.find_object(c, None).unwrap();
    assert_eq!(r.tag_lightweight("v1.0.0", &obj, false).unwrap(), c);
    assert!(r.tag_lightweight("v1.0.0", &obj, false).is_err(), "no clobber without force");
    // libgit2 stores the message verbatim; `git tag -m` appends a newline. Prettify for parity.
    let msg = git2::message_prettify("Release v1.0.1", None).unwrap();
    let t = r.tag("v1.0.1", &obj, &bot(), &msg, false).unwrap();
    assert_eq!(r.find_tag(t).unwrap().target_id(), c);
    assert_eq!(r.find_tag(t).unwrap().message().unwrap(), Some("Release v1.0.1\n"));
}

// G18 `git notes --ref semantic-release-<tag> add -f -m <json> <tag>`
#[test]
fn g18_write_note() {
    let f = fixture();
    let r = &f.work;
    let c = r.head().unwrap().target().unwrap();
    r.tag_lightweight("v1.0.0", &r.find_object(c, None).unwrap(), false).unwrap();
    let nref = "refs/notes/semantic-release-v1.0.0";
    let target = r.revparse_single("v1.0.0").unwrap().peel_to_commit().unwrap().id();
    r.note(&bot(), &bot(), Some(nref), target, r#"{"channels":[null]}"#, true).unwrap();
    // -f overwrite (addChannel)
    r.note(&bot(), &bot(), Some(nref), target, r#"{"channels":[null,"next"]}"#, true).unwrap();
    assert!(r.note(&bot(), &bot(), Some(nref), target, "x", false).is_err(), "no overwrite without force");
    assert_eq!(r.find_note(Some(nref), target).unwrap().message().unwrap(), r#"{"channels":[null,"next"]}"#);
    if git_cli_available() {
        let out = git_cli(r.workdir().unwrap(), &["notes", "--ref", "semantic-release-v1.0.0", "show", "v1.0.0"]);
        assert_eq!(out.trim_end(), r#"{"channels":[null,"next"]}"#);
    }
}

// G19 push ONE tag (not --tags), lightweight + annotated
#[test]
fn g19_push_single_tag() {
    let f = fixture();
    let r = &f.work;
    let c = r.head().unwrap().target().unwrap();
    let obj = r.find_object(c, None).unwrap();
    r.tag_lightweight("v1.0.0", &obj, false).unwrap();
    let at = r.tag("v1.0.1", &obj, &bot(), "annotated", false).unwrap();
    r.tag_lightweight("stray-local-tag", &obj, false).unwrap();

    let st = push(r, "origin", &["refs/tags/v1.0.0:refs/tags/v1.0.0", "refs/tags/v1.0.1:refs/tags/v1.0.1"]).unwrap();
    assert_eq!(st, vec![("refs/tags/v1.0.0".into(), None), ("refs/tags/v1.0.1".into(), None)]);
    let rem = remote_repo(&f);
    assert_eq!(ref_oid(&rem, "refs/tags/v1.0.0"), Some(c));
    assert_eq!(ref_oid(&rem, "refs/tags/v1.0.1"), Some(at), "annotated tag object pushed");
    assert_eq!(ref_oid(&rem, "refs/tags/stray-local-tag"), None, "other local tags not pushed");

    // GAP vs git CLI: git refuses to move an existing remote tag ("already exists") unless
    // forced; libgit2 only checks fast-forward, so a ff-move of a tag is PUSHED. Guard it in
    // push_negotiation: refuse any update of refs/tags/* whose remote side already exists.
    let c2 = commit(r, "z", "z", "x", 9_000);
    r.tag_lightweight("v1.0.0", &r.find_object(c2, None).unwrap(), true).unwrap();
    let guarded = {
        let mut cb = RemoteCallbacks::new();
        cb.push_negotiation(|ups| {
            for u in ups {
                if u.dst_refname().unwrap().starts_with("refs/tags/") && !u.src().is_zero() {
                    return Err(git2::Error::from_str(&format!("tag {} already exists on remote", u.dst_refname().unwrap())));
                }
            }
            Ok(())
        });
        let mut po = PushOptions::new();
        po.remote_callbacks(cb);
        r.find_remote("origin").unwrap().push(&["refs/tags/v1.0.0:refs/tags/v1.0.0"], Some(&mut po))
    };
    assert!(guarded.unwrap_err().message().contains("already exists"));
    assert_eq!(ref_oid(&rem, "refs/tags/v1.0.0"), Some(c), "guard kept remote tag");
    let unguarded = push(r, "origin", &["refs/tags/v1.0.0:refs/tags/v1.0.0"]).unwrap();
    assert_eq!(unguarded[0].1, None);
    assert_eq!(ref_oid(&rem, "refs/tags/v1.0.0"), Some(c2), "libgit2 moved the tag (git would refuse)");
}

// G20 `git push URL refs/notes/semantic-release-<tag>`
#[test]
fn g20_push_notes_ref() {
    let f = fixture();
    let r = &f.work;
    let c = r.head().unwrap().target().unwrap();
    let nref = "refs/notes/semantic-release-v1.0.0";
    r.note(&bot(), &bot(), Some(nref), c, r#"{"channels":[null]}"#, true).unwrap();
    let st = push(r, "origin", &[nref]).unwrap();
    assert_eq!(st, vec![(nref.to_string(), None)]);
    let rem = remote_repo(&f);
    assert_eq!(rem.find_note(Some(nref), c).unwrap().message().unwrap(), r#"{"channels":[null]}"#);
}

// Rejected non-fast-forward push surfaces as an error / per-ref status.
#[test]
fn push_rejection_non_ff_is_reported() {
    let f = fixture();
    let other = Repository::clone(&f.url, f._tmp.path().join("other")).unwrap();
    commit(&other, "b", "b", "feat: b", 2_000);
    push(&other, "origin", &["refs/heads/main:refs/heads/main"]).unwrap();
    commit(&f.work, "a", "a", "feat: a", 2_100);
    // Remote tip unknown locally: libgit2 refuses client-side.
    let e = push(&f.work, "origin", &["refs/heads/main:refs/heads/main"]).unwrap_err();
    assert!(e.message().contains("not present locally"), "{e}");
    // After fetching, it is a true non-ff: GIT_ENONFASTFORWARD, also client-side.
    fetch(&f.work, &f.url, &["+refs/heads/main:refs/remotes/origin/main"], AutotagOption::None, None).unwrap();
    let e = push(&f.work, "origin", &["refs/heads/main:refs/heads/main"]).unwrap_err();
    assert_eq!(e.code(), git2::ErrorCode::NotFastForward, "{e}");
    // Server-side rejection (hooks / protection) is in tests/daemon.rs.
}

// @semantic-release/git: ls-files -m -o, add (respecting .gitignore vs --force), commit as bot, push branch
#[test]
fn plugin_git_add_commit_push() {
    let f = fixture();
    let r = &f.work;
    let wd = r.workdir().unwrap().to_path_buf();
    std::fs::write(wd.join(".gitignore"), "dist/\n").unwrap();
    commit(r, ".gitignore", "dist/\n", "chore: ignore", 2_000);
    std::fs::write(wd.join("CHANGELOG.md"), "# 1.0.0\n").unwrap();
    std::fs::write(wd.join("README.md"), "changed\n").unwrap();
    std::fs::create_dir_all(wd.join("dist")).unwrap();
    std::fs::write(wd.join("dist/bundle.js"), "x").unwrap();

    // ls-files -m -o (modified + untracked, excluding ignored)
    let mut so = StatusOptions::new();
    so.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false);
    let mut changed: Vec<String> = r.statuses(Some(&mut so)).unwrap().iter().map(|e| e.path().unwrap().to_string()).collect();
    changed.sort();
    assert_eq!(changed, ["CHANGELOG.md", "README.md"]);
    assert!(r.is_path_ignored("dist/bundle.js").unwrap());

    let mut idx = r.index().unwrap();
    // Default add_all respects .gitignore ...
    idx.add_all(["CHANGELOG.md", "README.md", "dist/bundle.js"], IndexAddOption::DEFAULT, None).unwrap();
    assert!(idx.get_path(Path::new("dist/bundle.js"), 0).is_none(), "ignored file skipped");
    // ... FORCE is the `git add --force` semantic-release uses.
    idx.add_all(["dist/bundle.js"], IndexAddOption::FORCE, None).unwrap();
    assert!(idx.get_path(Path::new("dist/bundle.js"), 0).is_some());
    idx.write().unwrap();

    let tree = r.find_tree(idx.write_tree().unwrap()).unwrap();
    let parent = r.head().unwrap().peel_to_commit().unwrap();
    let author = bot();
    let committer = bot();
    let msg = "chore(release): 1.0.0 [skip ci]\n\nnotes\n";
    let c = r.commit(Some("HEAD"), &author, &committer, msg, &tree, &[&parent]).unwrap();
    let cm = r.find_commit(c).unwrap();
    assert_eq!(cm.author().name().unwrap(), "semantic-release-bot");
    assert_eq!(cm.committer().email().unwrap(), "semantic-release-bot@martynus.net");
    // worktree clean (`git status` empty)
    assert!(r.statuses(Some(&mut so)).unwrap().is_empty());

    // push HEAD:<branch> only (no --tags)
    r.tag_lightweight("v1.0.0", &r.find_object(c, None).unwrap(), false).unwrap();
    let st = push(r, "origin", &["HEAD:refs/heads/main"]).unwrap();
    assert_eq!(st, vec![("refs/heads/main".into(), None)]);
    let rem = remote_repo(&f);
    assert_eq!(ref_oid(&rem, "refs/heads/main"), Some(c));
    assert_eq!(ref_oid(&rem, "refs/tags/v1.0.0"), None);
}

// Connect for push over file:// works (no auth); see http_auth.rs for the auth-sensitive variant.
#[test]
fn g05_connect_push_lists_receive_pack_refs() {
    let f = fixture();
    let mut rem = f.work.remote_anonymous(&f.url).unwrap();
    rem.connect(Direction::Push).unwrap();
    let names: Vec<_> = rem.list().unwrap().iter().map(|h| h.name().to_string()).collect();
    assert!(names.contains(&"refs/heads/main".to_string()), "{names:?}");
}
