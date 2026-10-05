# semoxide

A Rust release tool inspired by semantic-release. Current phase: **research and planning only. No product code.**

# IMPORTANT

- While making architecture decisions and any new functionality, including just starting a documentation do not assume or go into generic path - ask user.
- Keep in mind that we need to "collapse"/"codnence" the docs after the we complete first "planning" huge step (before we write the code) so it doesn't bloat: we clear, compact, combine and remove things that were needed during planning but not needed as a documentation itself. (For example: decisions goes into hard specification in the correct space.)
- Inline comments (`//`) are short and informative. If the code speaks for itself, write no comment. Doc comments (`///`) are required on public items of published crates (`missing_docs`, [ADR 0017](docs/decisions/0017-code-quality.md)) and are kept short too.
- Split code into small, understandable parts, but not too small: follow [CODE-ARCHITECTURE](docs/CODE-ARCHITECTURE.md) (crates §1–§2, patterns §9, where things go §8).
- Log enough for debugging: every step, plugin call and remote operation emits `tracing` events ([OBSERVABILITY](docs/OBSERVABILITY.md)).

## Hard requirements

- Library first: the core is an embeddable crate, and the CLI is a thin wrapper around it.
- Opinionated. Familiar but not compatible: migration tool, no JS bridge ([ADR 0001](docs/decisions/0001-compatibility-stance.md)).
- Keep semantic-release's core ideas, including the plugin system.
- Use no git CLI if feasible: a Rust git library, with every operation checked by a PoC.
- Research, docs and PoCs come before any product code. Planning is discussed in waves.
- Docs: short, technical, never repeated (link instead). Each index doc is a one-line-per-entry list.
- **Docs: use Mermaid diagrams wherever they help**: flows, processes, sequences and timing, how code should work, and how things connect. Never use ASCII art.
- Git commit email is the default global config. Never override it.

## Rust rules

Core rules only. Known pitfalls: [RUST-PITFALLS](docs/RUST-PITFALLS.md). Everything else: [CODE-ARCHITECTURE](docs/CODE-ARCHITECTURE.md), lints and tools [ADR 0017](docs/decisions/0017-code-quality.md), logging [ADR 0013](docs/decisions/0013-observability.md).

1. No `unwrap` / `expect` / `panic!` outside tests.
2. `#[expect(lint, reason = "…")]`, never `#[allow]`.
3. Meaning lives in types: newtypes and enums, not `bool` / `Option` / `String` parameters.
4. Struct fields are private; construct with `new` or a builder.
5. Conversions via `From` / `TryFrom` / `FromStr`; names use `as_` / `to_` / `into_`; getters have no `get_`.
6. `unsafe_code = "forbid"`. The only exception is `semoxide-git`'s transport registration, with a `// SAFETY:` comment and real-repo integration tests (miri can't run its FFI).
7. Prefer modern std over extra crates or nesting: let chains, `if let` guards, `LazyLock`/`OnceLock`, `cfg_select!`, `assert_matches!`.
8. Flat control flow: early returns and `?` over nested `if` / `match`.

## Index

### **IMPORTANT**: every agent or subagent MUST read what's required for the task! In the plan mode we MUST READ as much as needed even more so.

- [Requirements](docs/REQUIREMENTS.md): secondary requirements
- [Specifications](docs/SPECIFICATIONS.md): external specs (semantic-release, SemVer, Conventional Commits)
- [Research](docs/RESEARCH.md): code, issue, library and ecosystem research
- [Porting gaps](docs/PORTING-GAPS.md): what can't be reproduced in Rust, and the replacement
- [Decisions](docs/DECISIONS.md): ADRs
- [Architecture](docs/ARCHITECTURE.md): project architecture
- [Code architecture](docs/CODE-ARCHITECTURE.md): crates, modules, what goes where
- [Testing](docs/TESTING.md): test strategy
- [Observability](docs/OBSERVABILITY.md): logging and debugging
- [Planning](docs/PLANNING.md): GitHub milestones, epics, labels, project board
- `poc/`: throwaway proof-of-concept crates (not product code)
