# 0015 Testing
Status: accepted (2026-10-05). Decided step by step; details are in [TESTING.md](../TESTING.md).

## Decided
- **Fixtures use the real git CLI.** A fixture DSL runs `git` (fixed dates, so SHAs are stable) to build test repos, and tests also use `git daemon` and git cross-checks. Only the tool itself is git-CLI-free ([ADR 0011](0011-git-backend.md)); the test machines and CI need git installed.
