# AGENTS.md

Instructions for AI coding agents (and people) working on semoxide.

## Start here

- Read [CLAUDE.md](CLAUDE.md): project rules, Rust rules, tools and the doc index. It applies to every agent, not only Claude.
- Read the skills in `.claude/skills/` before the matching work: `rust-testing` (any test or pure-core logic), `rust-async` (any concurrency).
- Setup per clone: [README § Development](README.md#development).

## Workflow

- Work on a branch, never on `main`; a pull request closes its issue.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
- The git hooks must pass; never commit or push with `--no-verify`.
- Tests follow the `rust-testing` skill: tests first, failing on assertions, approved by the maintainer before implementing.
- The PR description lists every change to an existing test or golden history.

## Never

- Loosen a lint, threshold, exclusion or allowlist to make a finding go away: fix the code, or ask the maintainer.
- Approve or accept snapshots (`.snap.new`) yourself.
- Weaken, skip, delete or rewrite an approved test or golden history to make code pass: stop and report why it should change; it changes only with the maintainer's agreement, as its own `test:` commit.
