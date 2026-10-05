# 0006 Conventional Commits parser
Status: accepted (2026-10-05)

- Wrap the `git-conventional` crate first.
- Write our own parser only if a PoC shows we need strict mode or error positions.

Why: see [CC spec §5](../specs/CONVENTIONAL-COMMITS-SPEC.md) and [dependencies §1](../research/dependencies.md).
