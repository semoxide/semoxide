use super::{LargeHistory, TagNotes};

fn small() -> LargeHistory {
    LargeHistory::new(50)
        .tag_every(10)
        .untagged_tail(15)
        .notes(TagNotes::OnEveryTag)
}

#[test]
fn commits_tags_and_untagged_tail() {
    let fixture = small().build().unwrap();

    assert_eq!(fixture.git(&["rev-list", "--count", "main"]).unwrap(), "50");
    assert_eq!(
        fixture.git(&["tag", "--sort=v:refname"]).unwrap(),
        "v1.0.1\nv1.0.2\nv1.0.3"
    );
    assert_eq!(
        fixture
            .git(&["rev-list", "--count", "v1.0.3..main"])
            .unwrap(),
        "20"
    );
}

#[test]
fn every_tag_has_a_note() {
    let fixture = small().build().unwrap();

    assert_eq!(
        fixture
            .git(&["notes", "--ref", "semoxide", "list"])
            .unwrap()
            .lines()
            .count(),
        3
    );
    assert_eq!(
        fixture
            .git(&["notes", "--ref", "semoxide", "show", "v1.0.2"])
            .unwrap(),
        r#"{"channels":[null]}"#
    );
}

#[test]
fn commits_cycle_conventional_types_with_fixed_dates() {
    let fixture = LargeHistory::new(5).build().unwrap();

    assert_eq!(
        fixture
            .git(&["log", "--reverse", "--format=%s %at %an <%ae>"])
            .unwrap(),
        "fix: change 1 1767225660 semoxide-test <test@example.invalid>\n\
         chore: change 2 1767225720 semoxide-test <test@example.invalid>\n\
         docs: change 3 1767225780 semoxide-test <test@example.invalid>\n\
         feat: change 4 1767225840 semoxide-test <test@example.invalid>\n\
         fix: change 5 1767225900 semoxide-test <test@example.invalid>"
    );
}

#[test]
fn same_history_twice_has_identical_shas() {
    let first = small().build().unwrap();
    let second = small().build().unwrap();

    for rev in ["main", "v1.0.3", "refs/notes/semoxide"] {
        assert_eq!(
            first.rev_parse(rev).unwrap(),
            second.rev_parse(rev).unwrap(),
            "{rev}"
        );
    }
}

#[test]
fn without_notes_no_notes_ref_exists() {
    let fixture = LargeHistory::new(20).tag_every(10).build().unwrap();

    assert!(fixture.rev_parse("refs/notes/semoxide").is_err());
}
