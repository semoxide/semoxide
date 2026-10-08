# semoxide

A Rust release tool inspired by semantic-release. Current phase: **planning done; M0 repo foundation, then M1 walking skeleton** ([PLANNING](docs/PLANNING.md)).

## IMPORTANT

- While making architecture decisions and any new functionality, including just starting a documentation do not assume or go into generic path - ask user.
- Keep in mind that we need to "collapse"/"codnence" the docs after the we complete first "planning" huge step (before we write the code) so it doesn't bloat: we clear, compact, combine and remove things that were needed during planning but not needed as a documentation itself. (For example: decisions goes into hard specification in the correct space.)
- Inline comments (`//`) are short and informative. If the code speaks for itself, write no comment. Doc comments (`///`) are required on public items of published crates (`missing_docs`, [quality tooling](docs/CODE-ARCHITECTURE.md#quality-tooling)) and are kept short too.
- Comments and docs explain the code or the rule, never their history: no "approved in the conversation", "as discussed", "per review". Provenance belongs in commit messages and PRs.
- Split code into small, understandable parts, but not too small: one domain per file; split by domain (not by stage) when a file mixes concepts, never by size alone ([CODE-ARCHITECTURE](docs/CODE-ARCHITECTURE.md): crates §1–§2, [modules and files](docs/CODE-ARCHITECTURE.md#modules-and-files), patterns §9, where things go §8).
- Log enough for debugging: every step, plugin call and remote operation emits `tracing` events ([OBSERVABILITY](docs/OBSERVABILITY.md)).

## Hard requirements

- Library first: the core is an embeddable crate, and the CLI is a thin wrapper around it.
- Opinionated. Familiar but not compatible: migration tool, no JS bridge ([REQUIREMENTS](docs/REQUIREMENTS.md), [DIFFERENCES](docs/DIFFERENCES.md)).
- Keep semantic-release's core ideas, including the plugin system.
- No git CLI: git2 only; every git operation is proven in a PoC ([ARCHITECTURE](docs/ARCHITECTURE.md#6-git-and-credentials)).
- Research, docs and PoCs come before any product code. Planning is discussed in waves.
- Docs: short, technical, never repeated (link instead). Each index doc is a one-line-per-entry list.
- **Docs: use Mermaid diagrams wherever they help**: flows, processes, sequences and timing, how code should work, and how things connect. Never use ASCII art.
- Git commit email is the default global config. Never override it.
- Tests first: the maintainer approves them (failing on assertions) before implementation. Never weaken, skip, delete or rewrite an approved test or golden history to make code pass: stop and report. List every changed existing test in the PR ([A2](docs/CODE-ARCHITECTURE.md#10-approaches)).

## Rust rules

Core rules only. Known pitfalls: [RUST-PITFALLS](docs/RUST-PITFALLS.md). Patterns per situation: project skills in `.claude/skills/` (`rust-testing`, `rust-async`). Everything else: [CODE-ARCHITECTURE](docs/CODE-ARCHITECTURE.md) (lints and tools: [quality tooling](docs/CODE-ARCHITECTURE.md#quality-tooling)), logging [OBSERVABILITY](docs/OBSERVABILITY.md).

1. No `unwrap` / `expect` / `panic!` outside tests.
2. `#[expect(lint, reason = "…")]`, never `#[allow]`.
3. Meaning lives in types: newtypes and enums, not `bool` / `Option` / `String` parameters.
4. Struct fields are private; construct with `new` or a builder.
5. Conversions via `From` / `TryFrom` / `FromStr`; names use `as_` / `to_` / `into_`; getters have no `get_`.
6. No `unsafe`: the workspace denies `unsafe_code` and every crate root has `#![forbid(unsafe_code)]`. The only exception is `semoxide-git`'s transport registration: one `#[expect(unsafe_code, reason = "…")]` item with a `// SAFETY:` comment and real-repo integration tests (miri can't run its FFI).
7. Prefer modern std over extra crates or nesting: let chains, `if let` guards, `LazyLock`/`OnceLock`, `cfg_select!`, `assert_matches!`.
8. Flat control flow: early returns and `?` over nested `if` / `match`.

## Tools

Rules: [TESTING](docs/TESTING.md), [CODE-ARCHITECTURE §7](docs/CODE-ARCHITECTURE.md#7-workspace-config). All pinned.

- [rustup](https://rust-lang.github.io/rustup/) toolchain · [cargo](https://doc.rust-lang.org/cargo/) build · [rustfmt](https://rust-lang.github.io/rustfmt/) format · [clippy](https://doc.rust-lang.org/clippy/) lints · [rustdoc](https://doc.rust-lang.org/rustdoc/) docs · [rust-analyzer](https://rust-analyzer.github.io/) editor
- Tests: [cargo-nextest](https://nexte.st/) runner · [insta](https://insta.rs/) snapshots · [assert_cmd](https://docs.rs/assert_cmd) CLI · [proptest](https://proptest-rs.github.io/proptest/) properties · [cargo-fuzz](https://rust-fuzz.github.io/book/) fuzzing · [criterion](https://docs.rs/criterion) benchmarks · [miri](https://github.com/rust-lang/miri) UB · [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) coverage · [cargo-mutants](https://mutants.rs/) mutation
- Quality: [cargo-hack](https://github.com/taiki-e/cargo-hack) features/MSRV · [cargo-deny](https://embarkstudios.github.io/cargo-deny/) deps policy · [cargo-shear](https://github.com/Boshen/cargo-shear) unused deps · [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) API breakage · [typos](https://github.com/crate-ci/typos) spelling · [zizmor](https://docs.zizmor.sh/) Actions security · [qlty](https://docs.qlty.sh/) checks · [lefthook](https://lefthook.dev/) git hooks
- Release: [dist](https://axodotdev.github.io/cargo-dist/) binaries · [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) installs

## Index

### **IMPORTANT**: every agent or subagent MUST read what's required for the task! In the plan mode we MUST READ as much as needed even more so

- [Requirements](docs/REQUIREMENTS.md): secondary requirements
- [Architecture](docs/ARCHITECTURE.md): system, repos, plugins, git, failure and rollback, monorepo
- [Code architecture](docs/CODE-ARCHITECTURE.md): crates, modules, patterns, quality tooling, what goes where
- [Config](docs/CONFIG.md): `semoxide.toml`
- [CLI](docs/CLI.md): commands, flags, env vars, JSON contract, exit codes
- [Observability](docs/OBSERVABILITY.md): logging, masking, errors, diagnostics
- [Testing](docs/TESTING.md): test strategy and CI
- [Differences](docs/DIFFERENCES.md): intentional differences from semantic-release
- [Rust pitfalls](docs/RUST-PITFALLS.md): known Rust problems and mitigations
- [Specifications](docs/SPECIFICATIONS.md): external specs and upstream implementation references
- [Decisions](docs/DECISIONS.md): why each non-obvious decision was made
- [Planning](docs/PLANNING.md): GitHub milestones, issues, labels, project board
- PoCs: [semoxide-poc](https://github.com/semoxide/semoxide-poc) (never in this repo)
