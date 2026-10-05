# 0015 Testing
Status: accepted (2026-10-05). Decided step by step; details are in [TESTING.md](../TESTING.md).

## Decided
- **Fixtures use the real git CLI.** A fixture DSL runs `git` (fixed dates, so SHAs are stable) to build test repos, and tests also use `git daemon` and git cross-checks. Only the tool itself is git-CLI-free ([ADR 0011](0011-git-backend.md)); the test machines and CI need git installed.
- **Sandbox credentials:** the real-remote tests run as a workflow **inside** the private `sm-steel/semoxide-sandbox` repo, using its automatic `GITHUB_TOKEN` (scoped to that repo, lasting one run; permissions set in the workflow). SSH tests use a write deploy key on that repo only. A fine-grained PAT (created in the web UI, ideally owned by `semoxide-bot`) is added only when a test must run from outside GitHub Actions, e.g. cross-CI.
- **npm tests: tentative, deferred.** Verdaccio only (a throwaway local npm registry in Docker); no real npmjs publishes. To be confirmed with the npm plugin ([ADR 0014](0014-porting-behaviour.md) O10).
- **Comparison with upstream: development phase only.** A scheduled job runs a **pinned** semantic-release version in dry-run on the golden histories, with intentional differences configured on both sides, and compares next version and release type (not notes). Before the first release its outputs are frozen into our own golden fixtures and the job is removed; after release semoxide's ADRs and tests are the spec. Idea noted, not decided: `semoxide migrate --verify` on a user's repo.
