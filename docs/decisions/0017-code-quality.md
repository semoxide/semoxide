# 0017 Code quality
Status: accepted (2026-10-05). Decided step by step. Findings are fixed in the code. Rules and thresholds are never loosened without the user's explicit approval.

## Decided
- **Git hooks via qlty** (`qlty githooks install`): pre-commit auto-formats; pre-push runs qlty's checks plus `cargo nextest run`. Hooks are local conveniences; CI is the gate ([ADR 0015](0015-testing.md)). The qlty CLI is BSL 1.1 licensed (fine for use as a dev tool).
