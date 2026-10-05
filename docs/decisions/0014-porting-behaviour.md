# 0014 Porting behaviour
Status: accepted (2026-10-05). Decided step by step; resolves the open items in [PORTING-GAPS](../PORTING-GAPS.md).

## Decided
- **`extends` sources (O1):**
  - v1: built-in presets (`preset:<name>`), local paths, and git refs pinned to a commit SHA (`git+https://…@<sha>#<file>`), fetched via git2 with the usual auth ([ADR 0011](0011-git-backend.md)) and cached. Fetched configs are recorded in the lock file ([ADR 0010](0010-plugin-architecture.md)).
  - Later: HTTPS URLs with `sha256`. No npm resolution.
