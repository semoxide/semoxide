# 0005 Dry-run
Status: accepted (2026-10-05)

- Dry-run never needs push rights and makes no network writes.
- `--verify-push` opts in to the push-permission check.

Why: semantic-release checks push permission even in dry-run, a frequent complaint (semantic-release#2232). See [semantic-release research](../research/semantic-release.md).
