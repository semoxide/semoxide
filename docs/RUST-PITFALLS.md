# Rust pitfalls

Known Rust problems that apply to semoxide, and how we avoid them. Sources: 2025–26 articles and the issues of 10 large Rust CLIs (research phase).

| # | Pitfall | Mitigation | Where |
|---|---|---|---|
| 1 | Slow incremental rebuilds in workspaces | split hot crates, trim deps/features, LLD, dev deps at `opt-level=3` | [CODE-ARCH §1, §7](CODE-ARCHITECTURE.md) |
| 2 | Crate splits help and hurt | split only on real boundaries; no tiny crates | [CODE-ARCH §1](CODE-ARCHITECTURE.md#1-workspace-layout) |
| 3 | Proc-macro / serde build cost | serde types live at the edges (`semoxide-schema`) | [CODE-ARCH §1](CODE-ARCHITECTURE.md#1-workspace-layout) |
| 4 | Async cancellation stops work mid-operation | async only in the plugin host; cooperative, cancel-safe steps | [CODE-ARCH §3, §5](CODE-ARCHITECTURE.md) |
| 5 | `Send` bounds; no `async fn` in `dyn` traits yet | boxed futures in the plugin trait; sync core | [CODE-ARCH §3](CODE-ARCHITECTURE.md#3-sync-vs-async) |
| 6 | Non-deterministic async tests | `#[tokio::test(start_paused = true)]`, current-thread runtime | – |
| 7 | Errors shaped for display, not for code | typed enums + `ErrorInfo`; rendering only in the CLI | [CODE-ARCH §4](CODE-ARCHITECTURE.md#4-error-and-result-types) |
| 8 | Over-generic, trait-heavy code | concrete types until a 2nd implementation exists | [A3, P2](CODE-ARCHITECTURE.md) |
| 9 | Orphan rule: no foreign trait on a foreign type | newtype + extension trait | – |
| 10 | Dependency bloat | audit each dependency; cargo-shear; a `Cargo.lock` package budget checked in CI (number set once code exists) | [0017](decisions/0017-code-quality.md) |
| 11 | Supply-chain attacks (malicious crates in 2025) | cargo-deny, `--locked`, care with new or typo-like crates | [0017](decisions/0017-code-quality.md) |
| 12 | Panics in production | no `unwrap` (R1); panic hook; `indexing_slicing` lint in libraries | [0013](decisions/0013-observability.md) |
| 13 | Broken pipe panics (`semoxide … \| head`) | the CLI handles `EPIPE` explicitly (exit quietly) | – |
| 14 | Feature leaks | `cargo hack --each-feature` | [0015](decisions/0015-testing.md) |
| 15 | Lint debt from a loose start | strict lints from day one | [0017](decisions/0017-code-quality.md) |
| 16 | "Library in name only": the library lags the CLI | the CLI uses only the façade | [CODE-ARCH §2](CODE-ARCHITECTURE.md#2-crate-boundaries-and-enforcement) |
| 17 | git2 slow on huge repos | benchmarks for large-history log walks | [0015](decisions/0015-testing.md) |
