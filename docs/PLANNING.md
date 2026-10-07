# Planning

GitHub conventions for every `semoxide/*` repo.

## Milestones

- Milestones are slices (M0, M1, …), never epics. Existing: **M0 Planning closure** (condense docs, decisions → issues, repo/CI skeletons, open items) and **M1 Walking skeleton** (dry run end to end on a real temp repo, see approach A4 in [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md)).
- The next milestones are discussed and created **only after M1 is done**. Intended slices: tag + push, plugin system, publish (self-release), beta pre-release.

## Issue structure

- **Issue types** (org-level, shared by all repos): `Epic`, `Feature`, `Task`, `Bug`, `Spike` (PoC / research).
- **Epics** are parent issues; their work items are **sub-issues**. "Blocked by" uses GitHub **issue dependencies**.
- **Labels** carry only *area* and status extras, not the kind of issue.

## Labels

Created identically in every org repo, including the PoC repo `semoxide/semoxide-poc`.

| Label | Use |
| --- | --- |
| `area:version-engine` | version engine, branch model, domain types |
| `area:schema` | config types, JSON Schema |
| `area:error` | `ErrorInfo`, error codes, docs pages |
| `area:git` | git2, SSH transports, push guards, credentials |
| `area:runtime` | orchestrator, config loading, CI detection, plugin host integration |
| `area:observability` | tracing, masking, logging flags |
| `area:cli` | commands, output formats, exit codes |
| `area:api` | façade, public API, semver |
| `area:plugins` | protocol, SDK, host, conformance, plugin repos |
| `area:ci` | our CI, tooling, release pipeline |
| `area:docs` | docs |
| `needs-decision` | waiting on the user's decision |
| `tests-unlocked` | the maintainer approves changing a LOCKED test in this PR (only the maintainer adds it) |
| `blocked-external` | waiting on something outside the project |
| `good-first-issue` | newcomer-friendly |

## Projects board

One org-level project across all `semoxide/*` repos.

- **Status:** `Todo` → `In progress` → `In review` → `Done`, plus `Waiting for user` (human-review stops from approach A2 in [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md), and `needs-decision`).
- **Fields:** built-ins (milestone, issue type, labels, assignees, parent/sub-issue progress, linked PRs), plus `Priority` (`P0` blocks the milestone, `P1`, `P2`).
- **Views:** a board by status; a table sliced by milestone and grouped by parent issue (epic), since a view groups by one field only; a "Waiting for user" filter; a roadmap by milestone.
- **Automation:** new issue → `Todo`, PR opened → `In review`, closed → `Done`. No iterations: work is slice-based.
