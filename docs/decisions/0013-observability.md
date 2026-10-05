# 0013 Observability
Status: accepted (2026-10-05). Decided step by step; details and remaining proposals are in [OBSERVABILITY.md](../OBSERVABILITY.md).

## Decided
- **The library never prints.** It only emits `tracing` events. It never sets up a log output and never writes to stdout or stderr. The CLI, or an embedding program, decides where logs go and how they look.
- **Masking happens at the source, plus an output pass.** The core and the plugin host replace secret values before an event is created. The CLI's output writer masks again as a second pass, and embedders can use that writer too.
- **Explicit env snapshot.** The builder takes an environment map (the CLI passes a copy of its own). The library reads only that map, never reads or changes the real process environment, and builds child-process environments from it.
- **Logs on stderr, data on stdout.** All human-readable logs go to stderr. stdout carries only data (e.g. `--output=json`, the printed next version), so piping works.
- **"No release" is never silent.** Every run without a release ends with a typed reason from a fixed list (e.g. `NotCi`, `PullRequest`, `BranchNotConfigured`, `NoCommitsSince`, `NoRelevantCommits`, `TagsNotFound`) plus a hint. It is logged at the normal level and included in the machine-readable result. The list is part of the public API.
- **Exit codes:** 0 released / promoted / no release · 1 failed before any remote write · 2 CLI usage · 3 config invalid (incl. plugin schema) · 4 verify failed · 5 partial failure (tag pushed, later step failed, rollback result in summary; completes [ADR 0012](0012-partial-failure.md)) · 101 panic · 130 interrupted. Opt-in `--fail-on-no-release` exits with its own code (proposed: 6) when nothing was released.
- **Error codes are namespaced names**, e.g. `core::no_git_repo`, `git::push_rejected`, `github::release_exists`. The plugin name is the namespace. The docs map upstream mnemonics (`ENOGITREPO` → `core::no_git_repo`).
- **Log filter env var:** `SEMOXIDE_LOG`, falling back to `RUST_LOG` when `SEMOXIDE_LOG` is unset (EnvFilter syntax).
