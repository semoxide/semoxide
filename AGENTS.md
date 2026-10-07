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
- Tests follow the `rust-testing` skill: tests first, locked after review, stop and report instead of editing a locked test.

## Never

- Loosen a lint, threshold, exclusion or allowlist to make a finding go away: fix the code, or ask the maintainer.
- Approve or accept snapshots (`.snap.new`) yourself.
- Edit a locked test.
