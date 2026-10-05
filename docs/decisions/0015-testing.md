# 0015 Testing
Status: accepted (2026-10-05). Decided step by step; details are in [TESTING.md](../TESTING.md).

## Decided
- **Fixtures use the real git CLI.** A fixture DSL runs `git` (fixed dates, so SHAs are stable) to build test repos, and tests also use `git daemon` and git cross-checks. Only the tool itself is git-CLI-free ([ADR 0011](0011-git-backend.md)); the test machines and CI need git installed.
- **Sandbox credentials:** the real-remote tests run as a workflow **inside** the private `semoxide/semoxide-sandbox` repo, using its automatic `GITHUB_TOKEN` (scoped to that repo, lasting one run; permissions set in the workflow). SSH tests use a write deploy key on that repo only. A fine-grained PAT (created in the web UI, ideally owned by `semoxide-bot`) is added only when a test must run from outside GitHub Actions, e.g. cross-CI.
- **npm tests: tentative, deferred.** Verdaccio only (a throwaway local npm registry in Docker); no real npmjs publishes. To be confirmed with the npm plugin ([ADR 0014](0014-porting-behaviour.md) O10).
- **Comparison with upstream: development phase only.** A scheduled job runs a **pinned** semantic-release version in dry-run on the golden histories, with intentional differences configured on both sides, and compares next version and release type (not notes). Before the first release its outputs are frozen into our own golden fixtures and the job is removed; after release semoxide's ADRs and tests are the spec. Idea noted, not decided: `semoxide migrate --verify` on a user's repo.
- **Test placement:**
  - No inline `#[cfg(test)] mod tests { … }` blocks with a body in source files.
  - Unit tests go in sibling files (`foo.rs` → `foo/tests.rs`, declared with `#[cfg(test)] mod tests;`).
  - Integration tests go in `tests/`, compiled as **one binary**.
  - Shared fixtures and builders live in a `publish = false` `semoxide-test-support` crate.
  - Doc tests in `///` examples are allowed.
  - Basis: ripgrep, uv, jj, just and cargo use one integration-test binary; uv, jj, ruff and cargo have test-support crates.
- **Kinds of tests:**
  - **Unit** (sibling files) and **integration** (library API against real temporary repos).
  - **CLI end-to-end** with `assert_cmd`.
  - **Snapshot** with `insta` as the backbone (notes, JSON, errors, `explain`, plan); CI runs `--unreferenced reject`.
  - **Data-driven** ported upstream tables.
  - **Property-based** (`proptest`) for parsers and the version engine.
  - **Fuzzing** (`cargo-fuzz`, scheduled) for commit, config and tag parsers.
  - **Doc tests.**
  - **Fakes**, not mocks: a fake plugin binary, and `wiremock` as a fake forge.
  - **Benchmarks** (`criterion`) from the start, for large-history log walks and analysis.
  - **miri** on every crate it can run. AI-assisted coding raises the risk of subtle undefined behaviour. miri cannot execute FFI (libgit2), real processes or sockets, so it covers the pure-Rust crates.
  - Plus the conformance kit, failure injection, the sandbox E2E and the upstream comparison (above).
  - Not used: mocking frameworks, and `trycmd`.
- **How tests run:**
  - Runner: `cargo-nextest`, with a `ci` profile (retries for known-flaky network tests, JUnit output).
  - Every PR: Linux. Windows tests only on `main` and on PRs labelled `windows` while the repos are private (Windows minutes cost 2×); every PR once public. aarch64 is build-checked on PRs and tested on a schedule.
  - Toolchains: stable on PRs, an MSRV check (`cargo hack --rust-version`), beta scheduled.
  - Checks: `cargo hack --each-feature` on published crates; `cargo insta test --unreferenced reject`; doc tests as a separate step; miri on PRs for the pure-Rust crates; coverage via `cargo llvm-cov` (reported, not gating).
  - Scheduled: fuzzing, benchmarks (plus on demand for log-walk PRs), sandbox E2E, the upstream comparison.
  - Speed: deps at `opt-level=3` in dev/test, `rust-cache` in CI.
  - One aggregate `required-checks` job.
