# 0003 Monorepo scope
Status: accepted (2026-10-05)

- v1 releases exactly one package. Monorepo support comes in v2.
- Designed in from day one:
  - the core works on a `Package` abstraction
  - tags are looked up per package
  - changed paths are recorded per commit
  - the `[packages.*]` config key is reserved

Why: see [distribution-config §3](../research/distribution-config.md).
