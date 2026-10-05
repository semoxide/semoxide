# 0016 Agent experience
Status: accepted (2026-10-05). Decided step by step: making semoxide easy and safe for AI agents and scripts to drive.

## Decided
- **Stable, versioned JSON output (C1):**
  - Every JSON output carries `"schema_version"`.
  - Its JSON Schema is generated from the Rust types (schemars) and published with each release.
  - Fields are only ever added. Renames and removals happen only with a `schema_version` bump in a major release.
- **JSON on every command (C2):** every command (`version`, `explain`, `doctor`, `--dry-run`, `migrate`, the main run, …) supports `--output=json` with a published schema. A test fails if any command lacks either.
- **Structured dry-run plan (C3):** the `plan` RPC returns typed actions `{ plugin, step, action, target, details }` instead of text lines (an additive protocol change, [ADR 0010](0010-plugin-architecture.md)). Text output renders them as lines; JSON carries them in the result, so agents or CI policies can check a planned release before allowing it.
