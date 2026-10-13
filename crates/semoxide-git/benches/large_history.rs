//! Benchmarks on the generated `large` history (TESTING: benchmarks): the reads of a release
//! since the last tag, on one open repository.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use semoxide_git::Repo;
use semoxide_test_support::large_history::LargeHistory;

#[expect(
    clippy::expect_used,
    reason = "a benchmark can't run without its fixture; failing loudly is the report"
)]
fn large(c: &mut Criterion) {
    let fixture = LargeHistory::large()
        .build()
        .expect("the large history builds");
    let last_tag = fixture
        .git(&["describe", "--tags", "--abbrev=0", "main"])
        .expect("the history has tags");
    let repo = Repo::discover(&fixture.path()).expect("the fixture is a repository");
    let head = repo.head().expect("HEAD is readable");
    let head = head.commit();
    let last_tag = repo
        .tag_commit(&last_tag)
        .expect("the last tag is readable")
        .expect("the last tag exists");
    let since = Some(last_tag.commit());
    let tags = repo.merged_tags(head).expect("tags are readable");

    // A wrong fixture would make the numbers meaningless.
    assert_eq!(repo.log(since, head).expect("log runs").len(), 30_000);
    assert_eq!(tags.len(), 3_500);
    assert!(tags.iter().all(|tag| {
        repo.tag_notes(tag)
            .expect("notes are readable")
            .semoxide()
            .is_some()
    }));

    let mut group = c.benchmark_group("large");
    // Each run takes about a second or more; 10 samples keep `cargo bench` short.
    group.sample_size(10);
    group.bench_function("log_walk", |b| {
        b.iter(|| black_box(repo.log(since, head).expect("log runs")));
    });
    group.bench_function("merged_tags", |b| {
        b.iter(|| black_box(repo.merged_tags(head).expect("tags are readable")));
    });
    group.bench_function("tag_notes", |b| {
        b.iter(|| {
            for tag in &tags {
                black_box(repo.tag_notes(tag).expect("notes are readable"));
            }
        });
    });
    group.finish();
}

criterion_group!(benches, large);
criterion_main!(benches);
