# Testing

How semoxide is tested, from pure functions to real releases. Per-area test-first approach and the AI-agent workflow: [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md) (A2) and the [`rust-testing` skill](../.claude/skills/rust-testing/SKILL.md).

## Where tests live

| Repo | Tests |
| --- | --- |
| `semoxide` | core: config, planning, release units, git2 layer, step pipeline, host services, CLI |
| `semoxide-plugin-protocol` | SDK, host launcher, the conformance kit itself |
| `semoxide-plugin-<name>` | the plugin's own logic (cases derived from upstream behaviour), its registry/forge fakes, the conformance kit in CI |

Inside a crate:

- Unit tests: inline in one `#[cfg(test)] mod tests { … }`, the last item of the module they test ([Rust Book](https://doc.rust-lang.org/book/ch11-03-test-organization.html#unit-tests)). A workspace check fails on an out-of-line test module (`#[cfg(test)] mod tests;`).
- Integration tests: one binary per crate, `tests/it/main.rs` declaring each file as a module ([why](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html)).
- A workspace check fails on a module file no `mod` declares (rustc silently skips it) in `src/` or `tests/it/`, and on any other test binary (`tests/*.rs`, `tests/*/main.rs`).
- Shared fixtures and builders: the `semoxide-test-support` crate (`publish = false`).
- Doc tests in `///` examples are allowed.

## Kinds of tests

- **Unit** and **integration** (library API against real temporary repos).
- **CLI end-to-end:** `assert_cmd`.
- **Snapshot:** `insta` is the backbone (notes, JSON, errors, `explain`, plan).
- **Data-driven:** `rstest` tables, incl. cases derived from upstream behaviour.
- **Property-based:** `proptest` for parsers and the version engine (incl. semver round trip).
- **Fuzzing:** `cargo-fuzz` for the commit, config and tag parsers.
- **Doc tests.**
- **Fakes, not mocks:** a fake plugin binary, `wiremock` as a fake forge. No mocking frameworks, no `trycmd`.
- **Benchmarks:** `criterion`, per crate in `benches/` (log walks: `semoxide-git`). They run on `LargeHistory::large()` (`semoxide-test-support`), generated in one `git fast-import` stream in seconds: 100k commits, a tag with a note every 20 commits (3,500 tags), the last 30k untagged. Sized after real reports: upstream's per-tag notes lookups slowed down from about 1k tags ([#2827](https://github.com/semantic-release/semantic-release/issues/2827)), and API-backed tools from tens of thousands of commits since the last release. Branches and monorepo units become generator parameters when their features get benchmarks.
- **miri:** every crate it can run. miri cannot execute FFI (libgit2), real processes or sockets, so it covers the pure-Rust crates.
- Plus [plugin conformance](#plugin-conformance), [failure injection](#failure-injection), the [sandbox E2E](#sandbox-repo) and the [upstream comparison](#upstream-comparison).

## Layers

```mermaid
flowchart TB
    L7["7 Dogfood: self-release, dry-run per PR and nightly"]
    L6["6 Live E2E: sandbox repo, scheduled"]
    L5["5 Container E2E: registries, git over HTTP / git daemon"]
    L4["4 Pipeline: whole lib, stub plugins, wiremock forge"]
    L3["3 Plugin: conformance kit, wiremock APIs"]
    L2["2 Git: git2 against fixture repos"]
    L1["1 Unit: parser, version math, templates, config"]
    L7 --- L6 --- L5 --- L4 --- L3 --- L2 --- L1
```

| # | Tests | Tooling |
| --- | --- | --- |
| 1 | CC parser, bump rules, next/last release, branch normalization, notes context, templates, config merge + plugin schema validation, env-ci table, secret masking | `rstest`, `insta`, `proptest`, `cargo-fuzz` |
| 2 | Every git op from the [git2 PoC](https://github.com/semoxide/semoxide-poc/tree/main/git2-ops) table, plus the [guards](#git) | `tempfile`, [fixtures](#fixtures), `file://` bare remotes; `git daemon` for shallow |
| 3 | Protocol compliance; per-plugin API calls, retry/throttle | [conformance kit](#plugin-conformance), `wiremock`, `tokio::time::pause` |
| 4 | Full pipeline: branches, channels, prerelease, maintenance, multiple units, dry-run, `fail`/`rollback`, error aggregation, host `Git` service rules | lib API + stub plugins (process and in-process), `wiremock` |
| 4b | CLI: args, exit codes, output, `migrate` | `assert_cmd` + `insta` |
| 5 | core: git over HTTP with auth, shallow/unshallow; plugin repos: real registry publishes | `testcontainers`: a git-http container (core); `kellnr` (cargo plugin), Verdaccio (npm plugin) |
| 6 | Real GitHub push, releases, assets, comments, channels | [sandbox](#sandbox-repo) |
| 7 | Self-release; dry-run per PR and nightly on main | dogfooding ([CODE-ARCHITECTURE](CODE-ARCHITECTURE.md) A8) |

When each layer runs: [CI](#ci).

## Fixtures

- **Fixture DSL** (`semoxide-test-support`): builder for commits, tags, notes, merges (ff/no-ff/rebase), shallow clone, detached HEAD, modeled on upstream's `git-utils.js`. It writes history with the real git CLI, an independent oracle; tests also use `git daemon` and git cross-checks. Test machines and CI need git installed (only the tool itself is git-CLI-free).
- **Fake secrets:** token-shaped test values always contain `SEMOXIDE_FAKE` (e.g. `ghp_SEMOXIDE_FAKE_0001`); gitleaks allowlists only that shape.
- **Deterministic SHAs:** fixed identity, step `n` dated 2026-01-01T00:00:00Z + `n` minutes; git runs with a cleared env (only `PATH` kept), no system config and the fixture's own global config (no signing, `core.autocrlf=false`), so the machine's git setup never reaches the history.
- **Golden histories:** `tests/histories/*.toml`, one case per file: `description`, `steps` (the fixture DSL as inline tables, `"push"` bare), optional `[config]` (written untracked as `semoxide.toml`; absent = defaults) and `[expected]` (`version` + `release_type` + optional `channel`, or `no_release` = a `NoReleaseReason` in snake case). The full plan and notes are `insta` snapshots in `tests/histories/snapshots/`. Loading is strict: unknown keys or steps fail naming the file. Shared by layers 2 and 4 and dry-run snapshots.
- **Remotes:** `file://` bare repos by default. libgit2 refuses shallow over `file://`, so shallow/unshallow tests use a `git daemon` or HTTP server.

## Git

Required guard tests:

| Guard | Test |
| --- | --- |
| Tag clobber | remote already has the tag at another commit: `push_negotiation` rejects, remote unchanged (libgit2 would fast-forward it) |
| Per-ref push status | pre-receive hook / protected branch declines: `push()` returns Ok, the per-ref status error must fail the step |
| Credential retry cap | server answers 401 forever: the callback gives up after N attempts instead of looping |
| One tag only | unrelated local tags exist: remote gains exactly the release tag |
| Bare `failed` retry | first push answers a reasonless per-ref `failed`: one logged retry; a retry reporting "exists" succeeds only if the remote tag points at our commit |
| Lookup by version | tags `v1.2.3+a` and `v1.2.3+b` exist: both are 1.2.3; clobber guard, rerun and promotion find them by version, never by a built name |

Real-remote items (GitHub 401 vs 403, SSH agent, proxies, smart-HTTP shallow) are layer 6.

## Upstream tests as reference

semoxide is not a 1:1 rewrite, so upstream test files are not ported. Their suites ([UPSTREAM-*](SPECIFICATIONS.md)) are read for edge cases and known bugs; where semoxide deliberately behaves the same, we write our own cases for it (`rstest` tables, golden histories, `insta`). Intentional differences: [DIFFERENCES](DIFFERENCES.md).

## Plugin conformance

`semoxide-plugin-conformance` (protocol repo) runs against any plugin, in any language; every plugin repo runs it in CI. It uses `semoxide-plugin-host`, the launcher semoxide uses. PoC: [plugin-grpc](https://github.com/semoxide/semoxide-poc/tree/main/plugin-grpc).

| Check | Fails when |
| --- | --- |
| Transport | gRPC over the local socket (UDS / named pipe) fails, or the per-run token is not presented |
| Handshake | protocol major differs, `steps` invalid, or unknown fields/steps are rejected |
| Manifest | handshake `secret_env` disagrees with the manifest |
| `describe` | it returns no config JSON Schema, or an invalid one |
| Secrets | a sentinel secret appears unmasked in any log, stdout or stderr (plugin and children) |
| Process tree | a grandchild survives kill after timeout or crash (Job Object / process group) |
| Deadlines | a step overruns its deadline and is not reported as timeout (`CANCELLED` or `DEADLINE_EXCEEDED`) |
| Shutdown | the plugin outlives `Shutdown` or the dropped connection |
| In-process path | for Rust plugins, the library crate passes the same checks in-process; it spawns no children and prints nothing directly |

## Dry-run

Behavior: [CLI](CLI.md). Tests:

- **No writes:** `wiremock` catch-all for non-GET `.expect(0)`; remote refs identical before and after; host `Git` service push is never called.
- **Read-only token** passes dry-run (regression for semantic-release#2232); `--verify-push` fails with it.
- **Plan snapshot** per golden history (`insta`), the readable spec of release behavior.

## Failure injection

One named regression test per row. Rollback and step-order rules: [ARCHITECTURE](ARCHITECTURE.md).

| Case | Injection | Expected | Layer |
| --- | --- | --- | --- |
| Publish fails after tag push (upstream #896, #2381) | stub publisher fails | `rollback` runs for each plugin; tag deleted via Git service; irreversible plugin logs a warning | 4 |
| Rollback without delete rights | remote refuses tag deletion | partial failure, partial-failure exit code ([CLI](CLI.md)), message names the leftover tag | 4, 6 |
| Plugin timeout / crash | plugin hangs or exits mid-step | tree killed, step fails, `rollback`/`fail` run | 4 |
| Asset changed after git commit | a plugin after `git` modifies a file matching its `assets` | in `prepare`: error by default, warning when configured; in `publish`: warning | 4 |
| Host `Git` rule | plugin pushes a non-release tag or moves a tag | `PERMISSION_DENIED`, remote unchanged | 4 |
| Half-done GitHub release | asset upload 500 once | draft deleted or reused on rerun; one release | 3, 6 |
| `push --tags` pushes every tag | unrelated local tags | remote gains only the new tag | 2, 4 |
| Secondary rate limit | 403 secondary, 429 `retry-after`, `x-ratelimit-remaining: 0` | retries after delay (paused clock), no duplicate POSTs | 3 |
| Shallow clone, missing tags | depth 1, with/without tags (`git daemon`) | correct last release | 2, 5 |
| Detached HEAD | checkout a SHA, branch from CI env | branch from env detection | 2, 4 |
| Branch behind remote | remote advances after clone | abort before tagging, remote unchanged | 4 |
| Several tags on one commit (#4073) | two tags, one commit | each tag's note read separately | 2 |
| Secret leak (upstream `plugin-log-env`) | plugin logs its env | masked everywhere | 3, 4 |
| `success` failure (github#738) | 500 on comments | default: outcome `Released`, warnings + `success_errors`, exit 0, no rollback, no `fail`; `steps.success.errors = "fail"`: exit 5, no rollback | 3, 4 |

## Sandbox repo

Private `semoxide/semoxide-sandbox` hosts layer 6 and the git2 real-remote items.

- **Credentials:** the tests run as a workflow inside the sandbox repo with its automatic `GITHUB_TOKEN` (scoped to that repo, one run, permissions set in the workflow). SSH tests use a write deploy key on that repo only. A fine-grained PAT (owned by `semoxide-bot`) is added only when a test must run outside GitHub Actions.
- Workflow on `schedule` + `workflow_dispatch` from `main` only, never fork PRs; one `concurrency` group.
- Per-run `tags.format` prefix `e2e-<run_id>-v{version}`; an `if: always()` cleanup deletes releases, tags, notes refs and `e2e/*` branches with that prefix; a weekly sweep removes leftovers older than 7 days.
- Scenarios: first release, `beta`/`next` channels, promotion (`add_channel`), maintenance `1.x`, assets, success comments, fail issue, rollback; protected-branch rejection; 401 vs 403 tokens.
- cargo: `kellnr` container only, never crates.io.
- npm: Verdaccio only (throwaway local registry in Docker), never npmjs; tentative until the npm plugin is built.

## Upstream comparison

Development phase only. A scheduled job runs a pinned semantic-release version in dry-run on the golden histories, with intentional differences ([DIFFERENCES](DIFFERENCES.md)) configured on both sides, and compares next version and release type (not notes). Before the first release its outputs are frozen into golden fixtures and the job is removed.

## CI

- **Runner:** `cargo-nextest`, configured in `.config/nextest.toml`; CI sets `NEXTEST_PROFILE=ci` (no fail-fast, failures shown immediately and at the end, slow-test summary, JUnit). Retries come with the first flaky network test.
- **Snapshots:** on Linux the tests run through `cargo insta test --test-runner nextest --unreferenced reject --require-full-match`, so a stale, mismatched or metadata-only-different snapshot fails; insta never writes snapshots in CI. Redactions (temp paths, …) arrive as a shared `semoxide-test-support` helper with the first snapshot that needs them.
- Linux jobs run in Docker containers; Windows runs natively.
- **Toolchains:** stable on PRs; MSRV check with `cargo hack --rust-version` (policy: [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md)); beta scheduled.
- **Speed:** `rust-cache`; dependency opt-level per [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md) profiles.
- **Coverage:** `cargo llvm-cov` on layers 1 to 4, reported, not gating.
- One aggregate `required-checks` job gates merges. The workflow always runs; a PR that changes only Markdown skips the Rust jobs (test, MSRV, cargo-deny), while `quality` still runs.

| Job | OS | Trigger |
| --- | --- | --- |
| Quality checks ([CODE-ARCHITECTURE](CODE-ARCHITECTURE.md)) | Linux | PR |
| Layers 1 to 4 + CLI | Linux, Windows | PR |
| `cargo insta test --unreferenced reject --require-full-match` | Linux | PR |
| Doc tests (separate step) | Linux | PR |
| miri, pure-Rust crates | Linux | PR |
| `cargo hack --each-feature`, published crates | Linux | PR |
| MSRV | Linux | PR |
| Layer 5: git-http container | Linux | PR |
| musl static build; aarch64 build check | Linux | PR |
| aarch64 tests | Linux aarch64 | scheduled |
| Fuzz (time-boxed), beta toolchain | Linux | nightly |
| Benchmarks | Linux | scheduled + on demand for log-walk PRs |
| Dogfood dry-run with the PR's own build | Linux | PR |
| Live E2E (sandbox), dogfood dry-run on main | Linux | nightly + manual |
| Upstream comparison (development phase) | Linux | scheduled |
| Conformance kit | per plugin repo, Linux + Windows | PR |
| Layer 5: registry container (`kellnr`, Verdaccio) | per plugin repo, Linux | PR |

Windows-specific cases: CRLF in messages, path separators in asset globs, named pipes, Job Object kill, `PATHEXT` resolution (`npm` → `npm.cmd`).
