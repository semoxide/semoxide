//! How a test builds a git history with `GitFixture`, and what each step leaves in the repository.

use semoxide_test_support::git_fixture::{GitFixture, GitFixtureBuilder, Merge};

/// Every kind of step once.
fn every_step() -> GitFixtureBuilder {
    GitFixture::builder()
        .commit("feat: first feature")
        .tag("v1.0.0")
        .commit("fix: a bug")
        .annotated_tag("v1.0.1", "release 1.0.1")
        .branch("next")
        .commit("feat!: breaking")
        .checkout("main")
        .commit("docs: readme")
        .merge("next", Merge::NoFastForward)
        .branch("rebased")
        .commit("chore: tidy")
        .checkout("main")
        .commit("fix: another bug")
        .checkout("rebased")
        .rebase("main")
        .note("v1.0.0", "semoxide-v1.0.0", r#"{"channels":[null]}"#)
        .push()
}

#[test]
fn same_fixture_built_twice_has_identical_shas() {
    let first = every_step().build().unwrap();
    let second = every_step().build().unwrap();

    for rev in [
        "main",
        "next",
        "rebased",
        "v1.0.0",
        "v1.0.1",
        "refs/notes/semoxide-v1.0.0",
    ] {
        let sha = first.rev_parse(rev).unwrap();
        assert_eq!(sha.len(), 40, "{rev}: {sha:?}");
        assert_eq!(sha, second.rev_parse(rev).unwrap(), "{rev}");
    }
}

#[test]
fn commit_does_not_depend_on_the_machine() {
    let fixture = GitFixture::builder()
        .commit("feat: first feature")
        .build()
        .unwrap();

    // The raw commit object, which the SHA hashes: empty tree, fixed identity, 2026-01-01T00:00:00Z,
    // and nothing from the machine's git config (no signature, SHA-1).
    assert_eq!(
        fixture.git(&["cat-file", "-p", "HEAD"]).unwrap(),
        "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
         author semoxide-test <test@example.invalid> 1767225600 +0000\n\
         committer semoxide-test <test@example.invalid> 1767225600 +0000\n\
         \n\
         feat: first feature"
    );
}

#[test]
fn history_starts_on_main() {
    let fixture = GitFixture::builder().commit("feat: a").build().unwrap();

    assert_eq!(fixture.git(&["branch", "--show-current"]).unwrap(), "main");
}

#[test]
fn each_step_is_one_minute_after_the_previous() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .annotated_tag("v1.0.0", "release 1.0.0")
        .commit("feat: b")
        .build()
        .unwrap();

    assert_eq!(
        fixture.git(&["log", "--format=%s %ct"]).unwrap(),
        "feat: b 1767225720\nfeat: a 1767225600"
    );
    assert_eq!(
        fixture
            .git(&[
                "for-each-ref",
                "--format=%(taggerdate:raw)",
                "refs/tags/v1.0.0"
            ])
            .unwrap(),
        "1767225660 +0000"
    );
}

#[test]
fn lightweight_and_annotated_tags() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .tag("v1.0.0")
        .annotated_tag("v1.0.1", "release 1.0.1")
        .build()
        .unwrap();

    assert_eq!(
        fixture.git(&["cat-file", "-t", "v1.0.0"]).unwrap(),
        "commit"
    );
    assert_eq!(fixture.git(&["cat-file", "-t", "v1.0.1"]).unwrap(), "tag");
    assert_eq!(
        fixture.rev_parse("v1.0.1^{commit}").unwrap(),
        fixture.rev_parse("v1.0.0").unwrap()
    );
}

#[test]
fn branch_creates_and_checks_out() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .branch("next")
        .commit("feat: b")
        .build()
        .unwrap();

    assert_eq!(fixture.git(&["branch", "--show-current"]).unwrap(), "next");
    assert_eq!(
        fixture.rev_parse("next^").unwrap(),
        fixture.rev_parse("main").unwrap()
    );
}

#[test]
fn checkout_of_a_tag_detaches_head() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .tag("v1.0.0")
        .commit("feat: b")
        .checkout("v1.0.0")
        .build()
        .unwrap();

    assert_eq!(fixture.git(&["branch", "--show-current"]).unwrap(), "");
    assert_eq!(
        fixture.rev_parse("HEAD").unwrap(),
        fixture.rev_parse("v1.0.0").unwrap()
    );
}

#[test]
fn fast_forward_merge_moves_the_branch() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .branch("next")
        .commit("feat: b")
        .checkout("main")
        .merge("next", Merge::FastForward)
        .build()
        .unwrap();

    assert_eq!(
        fixture.rev_parse("main").unwrap(),
        fixture.rev_parse("next").unwrap()
    );
}

#[test]
fn no_fast_forward_merge_makes_a_merge_commit() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .branch("next")
        .commit("feat: b")
        .checkout("main")
        .merge("next", Merge::NoFastForward)
        .build()
        .unwrap();

    assert_eq!(
        fixture.rev_parse("main^2").unwrap(),
        fixture.rev_parse("next").unwrap()
    );
}

#[test]
fn rebase_replays_the_branch_onto_another() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .branch("next")
        .commit("feat: b")
        .checkout("main")
        .commit("feat: c")
        .checkout("next")
        .rebase("main")
        .build()
        .unwrap();

    assert_eq!(
        fixture.rev_parse("next^").unwrap(),
        fixture.rev_parse("main").unwrap()
    );
    assert_eq!(
        fixture.git(&["log", "-1", "--format=%s", "next"]).unwrap(),
        "feat: b"
    );
}

#[test]
fn note_is_stored_under_its_ref() {
    let fixture = GitFixture::builder()
        .commit("feat: a")
        .tag("v1.0.0")
        .note("v1.0.0", "semoxide-v1.0.0", r#"{"channels":[null]}"#)
        .build()
        .unwrap();

    assert_eq!(
        fixture
            .git(&["notes", "--ref", "semoxide-v1.0.0", "show", "v1.0.0"])
            .unwrap(),
        r#"{"channels":[null]}"#
    );
}

#[test]
fn push_copies_branches_tags_and_notes_to_the_remote() {
    let fixture = every_step().build().unwrap();

    for rev in [
        "refs/heads/main",
        "refs/heads/next",
        "refs/heads/rebased",
        "refs/tags/v1.0.0",
        "refs/tags/v1.0.1",
        "refs/notes/semoxide-v1.0.0",
    ] {
        assert_eq!(
            fixture.remote_git(&["rev-parse", rev]).unwrap(),
            fixture.rev_parse(rev).unwrap(),
            "{rev}"
        );
    }
}

#[test]
fn without_push_the_remote_is_empty() {
    let fixture = GitFixture::builder().commit("feat: a").build().unwrap();

    assert_eq!(
        fixture.remote_git(&["for-each-ref"]).unwrap(),
        "",
        "the bare remote exists but has no refs"
    );
}

#[test]
fn origin_is_the_remote_file_url() {
    let fixture = GitFixture::builder().commit("feat: a").build().unwrap();

    assert!(
        fixture.remote_url().starts_with("file://"),
        "{}",
        fixture.remote_url()
    );
    assert_eq!(
        fixture.git(&["remote", "get-url", "origin"]).unwrap(),
        fixture.remote_url()
    );
}

#[test]
fn failing_step_reports_the_git_command() {
    let error = GitFixture::builder()
        .commit("feat: a")
        .checkout("missing")
        .build()
        .unwrap_err();

    assert!(
        error
            .to_string()
            .starts_with("`git checkout missing` failed:"),
        "{error}"
    );
}
