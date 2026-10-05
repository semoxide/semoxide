# Competitors

Stars, last release and issue reactions `[n]`: GitHub API, 2026-10-05. Issue numbers are per repo.

> **Verification:** the feature columns (scope, lib, git, templates, config, monorepo, extensibility, CI) and the lessons are partly from model memory and unverified; specific doubtful claims are marked (unverified).

## Rust tools

| Tool | Scope | Lib? | Git | Templates | Config | Monorepo | Extensibility | CI | Stars / last rel. | Lic. |
|---|---|---|---|---|---|---|---|---|---|---|
| [git-cliff](https://github.com/orhun/git-cliff) | Changelog only (+ `--bump`) | yes, `git-cliff-core` | git2 | Tera | `cliff.toml` / Cargo / pyproject | `--include-path` | regex commit parsers, remote (GH/GL/Gitea/BB) metadata | Action | 12.3k / 2026-09 | Apache/MIT |
| [release-plz](https://github.com/release-plz/release-plz) | Cargo only: release PR, crates.io publish, GH/GL/Gitea release | `release_plz_core` (internal) | git2 + git CLI | Tera (via git-cliff-core) | `release-plz.toml` | Cargo workspace | none (cargo-semver-checks built in) | Action | 1.5k / 2026-09 | Apache/MIT |
| [cargo-release](https://github.com/crate-ci/cargo-release) | Cargo only: bump/tag/publish, **manual** level | no | git2 + git CLI | regex `pre-release-replacements` | `release.toml` / Cargo metadata | workspace | shell hooks | none official | 1.6k / 2026-09 | Apache/MIT |
| [cocogitto](https://github.com/cocogitto/cocogitto) | CC lint, bump, changelog, tag | crate, CLI-focused | git2 | Tera | `cog.toml` | `packages` | pre/post-bump shell hooks, bump profiles | Action | 1.2k / 2026-03 | MIT |
| [knope](https://github.com/knope-dev/knope) | Multi-lang bump + changelog + GH/Gitea release; changesets **and** CC | `knope-versioning` | git2 | variable substitution | `knope.toml` workflows | yes | built-in steps + `Command` step, no plugins | Action | 196 / 2026-05 | MIT |
| [convco](https://github.com/convco/convco) | CC lint, version, changelog | no | git2 or gix (feature) | Handlebars | `.versionrc` | path filter | none | Docker | 322 / 2026-09 | MIT |
| [cargo-smart-release](https://github.com/GitoxideLabs/cargo-smart-release) | Cargo workspace release in dep order, changelog merge | no | gix | own MD merge | CLI flags | workspace | none | none | 129 / 2026-09 | Apache/MIT |
| [semantic-rs](https://github.com/semantic-rs/semantic-rs), [kettleby/semantic-release-rust](https://github.com/kettleby/semantic-release-rust) | semantic-release for crates | no | git2 | - | - | no | - | - | 165, 17 / dead 2021 | MIT |
| [semantic-release-cargo](https://github.com/semantic-release-cargo/semantic-release-cargo) | JS semantic-release **plugin** for Cargo, in Rust | - | - | - | - | - | is a plugin | - | 38 / active | Apache |
| 2025-26 newcomers: [super-release](https://github.com/BowlingX/super-release) (git2 + git-cliff-core + octocrab), [FerrFlow](https://github.com/FerrLabs/FerrFlow) (gix, 16 version-file formats), [releasaurus](https://github.com/robgonnella/releasaurus) (release-PR, multi-forge), [moonlit](https://github.com/wolfware-labs/moonlit) (**wasmtime WASI-component plugins**, YAML pipelines), [sr](https://github.com/urmzd/sr), [vnext](https://github.com/unbounded-tech/vnext), [cranko](https://github.com/pkgw/cranko) | semantic-release-likes | mostly no | mixed | | | most yes | only moonlit has plugins | | 0-27 each | MIT/Apache |

## Non-Rust references

| Tool | Model | Git | Templates | Config | Monorepo | Extensibility | Stars / last rel. | Lic. |
|---|---|---|---|---|---|---|---|---|
| [semantic-release](https://github.com/semantic-release/semantic-release) | Commit-driven, CI-only, lifecycle plugins | git CLI | lodash (conventional-changelog) | `.releaserc` | **no** (#193 [335]) | npm plugins per lifecycle step | 24k / 2026-08 | MIT |
| [go-semantic-release](https://github.com/go-semantic-release/semantic-release) | Single-binary port | go-git / provider API | Go code | `.semrelrc` (JSON) | no | **subprocess plugins (hashicorp go-plugin, gRPC)**, auto-downloaded from own registry: provider, analyzer, condition, changelog, files-updater, hooks | 485 / **2024-10, stagnant** (#204 "maintained?") | MIT |
| [python-semantic-release](https://github.com/python-semantic-release/python-semantic-release) | Commit-driven port, Python-first | GitPython | Jinja2 | `pyproject.toml` | limited (#168) | custom commit parser classes, `build_command` | 1.1k / 2026-09 | MIT |
| [release-please](https://github.com/googleapis/release-please) | Release **PR**; GitHub API only, no clone | GitHub API | conventional-changelog | `release-please-config.json` + manifest | yes (manifest) | compiled-in strategies + workspace plugins | 7.6k / 2026-08 | Apache |
| [changesets](https://github.com/changesets/changesets) | Intent files written by devs, Version PR | git CLI | pluggable changelog modules | `.changeset/config.json` | yes, JS-only | changelog-generator npm modules | 12.5k / 2026-09 | MIT |
| [goreleaser](https://github.com/goreleaser/goreleaser) | Build/package/publish artifacts for an **existing** tag; no version calc | git CLI | Go templates | `.goreleaser.yaml` | **Pro (paid)** only | hooks, many built-in publishers | 16k / 2026-09 | MIT + paid Pro |

## Top user complaints / requests (by reactions)

| Theme | Evidence |
|---|---|
| Monorepo / multi-package | semantic-release #193 [335], #1688 [79]; changesets #1160 [44], #264, #1137; PSR #168; release-please #1921, #2167; knope #1558 |
| Print next version / dry run | semantic-release #753 [140], #1647 [87]; changesets #614 [49]; go-semrel #183 |
| Pre-releases / multi-branch / LTS | release-please #510 [57]; git-cliff #1380; release-plz #2159; PSR #386, #267; cocogitto #241 |
| Conventional commits in intent-based tools | changesets #862 [99], #577 [44] |
| Non-GitHub forges | release-please #1021 [46]; changesets #879; PSR #666; go-semrel #141 |
| Forge API fragility / rate limits | release-please #2577 [109] (502s); semantic-release #2204 [67] |
| Git-only mode (no registry / no PR) | release-plz #1144 [35] |
| More version files (lockfiles, custom fields) | release-please #2561 [79] (uv.lock); release-plz #1024; knope #482, #162 |
| Dependency breakage | PSR #1476 [31] (GitPython update broke all configs) |
| Reverted/squash commits | release-please #296; git-cliff #521 |
| Contributors in notes | release-please #292; release-plz #989; git-cliff #119 |
| Don't fail on "no release" | go-semrel #8; cocogitto #457 |

## Lessons for semoxide

| Copy | Avoid |
|---|---|
| semantic-release lifecycle steps (verifyConditions → analyze → notes → prepare → publish → success/fail) as plugin contract; go-semantic-release proved it ports | go-semantic-release's self-hosted plugin download registry: infra burden; the project stalled (causal link unverified) |
| Use `git-cliff-core` or Tera/MiniJinja for notes; Jinja-like syntax is what users know (PSR, git-cliff, cocogitto) | Handlebars/lodash templates (convco, conventional-changelog): weak for logic |
| First-class `--dry-run` + `next-version` JSON output | Making "no release" an error exit code |
| Monorepo from day one (top request everywhere) | Paywalling monorepo (goreleaser Pro) |
| Forge abstraction (GitHub, GitLab, Gitea/Forgejo) + retry/backoff on API (release-plz uses reqwest-retry (unverified)) | GitHub-API-only design (release-please 502s on big repos) |
| Git-only mode: tag + notes, no forge, no registry | Hard-wiring one ecosystem (release-plz, cargo-release) |
| Version-file updaters as plugins incl. lockfiles (knope, FerrFlow) | Fragile third-party runtime deps (PSR/GitPython); single static binary instead |
| GitHub Action + prebuilt binaries + `cargo binstall` from v1 | Opt-out-only defaults that publish to npm (semantic-release #1260) |
| Ignore reverted commits, handle squash merges | |

## The gap

**Thin.** Coverage today:
- Changelog: git-cliff owns it (12k stars, library crate).
- Rust crates release: release-plz + cargo-release own it.
- Language-agnostic, single binary, commit-driven: knope (no plugins, small), go-semantic-release (stagnant), plus ~7 tiny 2025-26 Rust clones; moonlit already does WASM plugins.

What nobody does well together:
1. **Real plugin system + single binary** with a maintained ecosystem (go-semrel stalled; moonlit 3 stars).
2. **semantic-release compatibility**: read `.releaserc`, same lifecycle and plugin names, so the 24k-star user base can migrate without rewriting config. No Rust tool targets this.
3. **Library-first API** that other tools embed (only git-cliff-core is used this way, and only for changelogs).
4. Monorepo + pre-release channels + multi-forge in one tool.

Risk: the differentiator is execution and migration, not a missing feature. Without (2), semoxide is "another knope".

## Ticket candidates

- Decide semantic-release config/lifecycle compatibility level — `.releaserc` parity scope, plugin name mapping.
- Evaluate `git-cliff-core` as changelog engine vs own Tera/MiniJinja — embed vs dependency risk.
- Git-only mode — tag + notes without forge or registry.
- Version-file updater plugins — Cargo.toml/lock, package.json/lock, pyproject/uv.lock, go, custom regex.
- Plugin protocol PoC vs go-semantic-release gRPC and moonlit WASI components; plugin distribution via GitHub releases / binstall / OCI: see [plugin-mechanisms](plugin-mechanisms.md#ticket-candidates).
- Covered elsewhere: `next-version`/JSON output → [semantic-release](semantic-release.md#ticket-candidates); monorepo (study knope, release-please manifest) → [distribution-config](distribution-config.md#monorepo); branches → [spec](../specs/SEMANTIC-RELEASE-SPEC.md#ticket-candidates); forge abstraction + retry → [github](github.md#ticket-candidates); commit edge cases → [CC spec](../specs/CONVENTIONAL-COMMITS-SPEC.md#ticket-candidates); distribution → [distribution-config](distribution-config.md#distribution).
