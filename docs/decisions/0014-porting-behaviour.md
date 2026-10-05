# 0014 Porting behaviour
Status: accepted (2026-10-05). Decided step by step; resolves the open items in [PORTING-GAPS](../PORTING-GAPS.md).

## Decided
- **`extends` sources (O1):**
  - v1: built-in presets (`preset:<name>`), local paths, and git refs pinned to a commit SHA (`git+https://…@<sha>#<file>`), fetched via git2 with the usual auth ([ADR 0011](0011-git-backend.md)) and cached. Fetched configs are recorded in the lock file ([ADR 0010](0010-plugin-architecture.md)).
  - Later: HTTPS URLs with `sha256`. No npm resolution.
- **User JS functions (O2):** replaced by declarative TOML plus template logic. TOML handles the type → section map, hidden types, sort keys and the bump-rule list (glob values). Minijinja templates ([ADR 0004](0004-template-engine.md)) may filter, map and sort inside notes. Anything beyond that is a replacement `analyze_commits` / `generate_notes` plugin. No embedded scripting.
- **Presets (O3):**
  - `conventionalcommits` and `angular` ship as built-in data in the commit-analyzer and release-notes crates. **Default: `conventionalcommits`.** It recognises `!` as breaking, matches the spec, and is semantic-release's planned next default (#3406, #1652; no timeline upstream).
  - `angular` stays available (`preset = "angular"`), and `migrate` sets it for configs that relied on upstream's default. User presets are written in TOML.
  - Bump rules stay as in [ADR 0007](0007-bump-defaults.md).
- **Globs (O4):** `globset` with context-specific semantics.
  - File paths (assets, monorepo paths): standard file globs.
  - Bump-rule values: `*` also matches `/` (fixes commit-analyzer #175).
  - Branches: a built-in matcher for the maintenance pattern (`N.x`, `N.N.x`), plus normal globs for user patterns.
  - No extended globs. `migrate` rewrites the ones it recognises and reports the rest.
- **Versions and ranges (O5):** the `semver` crate parses versions, and comparison always uses `cmp_precedence` (build metadata ignored, [SemVer spec](../specs/SEMVER-SPEC.md)). semoxide has its own `Range { min, max_exclusive }` and bump functions (incl. prerelease increments), tested with cases ported from npm semver. No npm-range parser.
- **User regexes (O6):** `fancy-regex` (0.19), so upstream patterns with lookaround or backreferences port unchanged. Patterns without such features are delegated to the `regex` crate and run in linear time. Backtracking patterns run with a `backtrack_limit`; exceeding it is an error naming the pattern and the commit (no hang).
- **Bot identity for commit-back (O8):** resolved in this order: `[plugins.git] author` in config → `GIT_AUTHOR_*` / `GIT_COMMITTER_*` from the env snapshot → the CI platform's bot identity (GitHub Actions: `github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>`) → a semoxide default. GitLab (and other CIs) have no platform bot identity, so they use the semoxide default: `semoxide-bot` with the GitHub noreply address of a dedicated `semoxide-bot` GitHub account (the account still has to be created). At full release the default moves to an address on a semoxide-owned domain.
