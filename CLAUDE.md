# semoxide

A Rust release tool inspired by semantic-release. Current phase: **research and planning only. No product code.**

# IMPORTANT

- While making architecture decisions and any new functionality, including just starting a documentation do not assume or go into generic path - ask user.

## Hard requirements

- Library first: the core is an embeddable crate, and the CLI is a thin wrapper around it.
- Opinionated. Familiar but not compatible: migration tool, no JS bridge ([ADR 0001](docs/decisions/0001-compatibility-stance.md)).
- Keep semantic-release's core ideas, including the plugin system.
- Use no git CLI if feasible: a Rust git library, with every operation checked by a PoC.
- Research, docs and PoCs come before any product code. Planning is discussed in waves.
- Docs: short, technical, never repeated (link instead). Each index doc is a one-line-per-entry list.
- **Docs: use Mermaid diagrams wherever they help**: flows, processes, sequences and timing, how code should work, and how things connect. Never use ASCII art.
- Git commit email is the default global config. Never override it.

## Index

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
