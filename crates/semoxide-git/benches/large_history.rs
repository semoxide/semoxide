//! Benchmarks on the generated `large` history (TESTING: benchmarks).
//!
//! Placeholders: they time the git CLI on the fixture until semoxide-git's own log walk and
//! notes reading exist (E-M1.6), which then replace the measured closures.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
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
    let since_last_tag = format!("{last_tag}..main");

    let mut group = c.benchmark_group("large");
    // Each run takes a sizeable fraction of a second; 10 samples keep `cargo bench` short.
    group.sample_size(10);
    group.bench_function("log_walk", |b| {
        b.iter(|| {
            black_box(
                fixture
                    .git(&["rev-list", "--count", &since_last_tag])
                    .expect("rev-list runs"),
            )
        });
    });
    group.bench_function("tag_notes", |b| {
        b.iter(|| {
            black_box(
                fixture
                    .git(&[
                        "log",
                        "--tags",
                        "--no-walk",
                        "--notes=semoxide",
                        "--format=%H %N",
                    ])
                    .expect("log runs"),
            )
        });
    });
    group.finish();
}

criterion_group!(benches, large);
criterion_main!(benches);
