# 0016 Agent experience
Status: accepted (2026-10-05). Decided step by step: making semoxide easy and safe for AI agents and scripts to drive.

## Decided
- **Stable, versioned JSON output (C1):**
  - Every JSON output carries `"schema_version"`.
  - Its JSON Schema is generated from the Rust types (schemars) and published with each release.
  - Fields are only ever added. Renames and removals happen only with a `schema_version` bump in a major release.
