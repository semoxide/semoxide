# Differences from semantic-release

semoxide is **familiar, not compatible**: it uses semantic-release's concepts, lifecycle steps, step order and multi-plugin rules, but its own config format and plugin protocol, and there is no JS bridge. `semoxide migrate` converts a `.releaserc` to `semoxide.toml` where it can and reports every difference below that it hits ([CLI.md](CLI.md)).

These match upstream and are **not** differences: the default bump table, first release `1.0.0`, tag pushed before publish, forced dry-run outside CI ([CONFIG.md](CONFIG.md), [CLI.md](CLI.md)).

Every row is intentional. The linked doc owns the full rule.

## Configuration

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Config file | cosmiconfig: `.releaserc*`, `release.config.{js,ts,mjs,cjs}`, `package.json#release` | TOML only: `semoxide.toml`, fallback `.config/semoxide.toml`; data only, layered with `--set` flags; config is never read from env vars | [CONFIG.md](CONFIG.md) |
| Computed config | JS config files may compute options or pass functions | No code in config; everything is declarative | [CONFIG.md](CONFIG.md) |
| Shareable configs | `extends` resolves npm packages; plugins resolved relative to the config package | `extends` takes built-in presets, local paths and SHA-pinned git refs (HTTPS + `sha256` later); no npm resolution; plugins come from their own pinned versions | [CONFIG.md](CONFIG.md) |
| Layer merge | shallow: a redefined top-level key (`plugins`, `branches`) replaces the inherited value | deep merge of tables, arrays replace; `config.merge = "shallow"` restores upstream behaviour | [CONFIG.md](CONFIG.md) |
| User functions | writer `transform`, `finalizeContext`, sort comparators, `releaseRules` as JS module or functions | Declarative TOML (type map, hidden types, sort keys, rule list) plus minijinja logic; anything more is a replacement plugin; no embedded scripting | [CONFIG.md](CONFIG.md) |
| Config validation | By convention plugins validate options in `verifyConditions` (commit-analyzer and release-notes don't); core checks only plugin/step shape | Every plugin publishes a JSON Schema; all config is validated before any step runs | [ARCHITECTURE.md](ARCHITECTURE.md) |

## Templates and release notes

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Template engine | lodash `${…}` / `<% %>` evaluating JS | minijinja; no JS expressions | [CONFIG.md](CONFIG.md) |
| `tagFormat` | lodash template, e.g. `v${version}` | `tags.format` with its own `{version}` syntax | [CONFIG.md](CONFIG.md) |
| Notes templates | Handlebars `.hbs` partials from `conventional-changelog-<preset>` modules | minijinja templates; `.hbs` unsupported; presets built into the bundled analyzer and notes plugins | [CONFIG.md](CONFIG.md) |
| Default preset | `angular` | `conventionalcommits` (`!` is breaking); `angular` available, set by `migrate` when the old config relied on the default; user presets in TOML; set once as `commits.preset` (upstream repeats it per plugin) | [CONFIG.md](CONFIG.md) |
| Sort order | JS `localeCompare` | ICU collation, default locale `en` (same order), configurable `locale` of the release-notes plugin | [CONFIG.md](CONFIG.md) |
| Untrusted text | Commit text published as written | ANSI escapes, control characters and invisible Unicode stripped from commit text, notes and plugin output everywhere | [CLI.md](CLI.md) |

## Commit analysis

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Skip marker | `[skip release]` / `[release skip]` excludes from the bump only; commit still appears in notes | Excluded from bump **and** notes; all-skipped yields no-release reason `AllCommitsSkipped` | [CONFIG.md](CONFIG.md) |
| Reverts | pair cancelled only on exact full hash (short hash never cancels); git `Revert "…"` form only | unique-prefix hash match; also `revert:` + `Refs:` footer | [CONFIG.md](CONFIG.md) |
| Rule globs | micromatch; `*` stops at `/` in `releaseRules` values | `globset`; `*` also matches `/` in bump-rule values | [CONFIG.md](CONFIG.md) |
| Extended globs | micromatch extglob (`+(…)`, `?(…)`) | Not supported; `migrate` rewrites recognised patterns and reports the rest | [CONFIG.md](CONFIG.md) |
| User regexes | JS regexes via `parserOpts` | `fancy-regex` (lookaround, backrefs); backtracking patterns run under a backtrack limit and fail with an error naming pattern and commit | [CONFIG.md](CONFIG.md) |
| Parser | `conventional-commits-parser` | `git-conventional`, used as-is (deviations documented) | [CONFIG.md](CONFIG.md) |
| Unparsable commits | silently ignored | ignored by default but always listed by `explain` with the parse error; `strict` fails the release on them (merges/fixups exempt unless `strict_merges`) | [CONFIG.md](CONFIG.md) |

## Versions and branches

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Version ranges | node-semver ranges (`satisfies`, `x` ranges, `-0` quirk) | Own `Range { min, max_exclusive }` and bump code on the `semver` crate; precedence ignores build metadata; no npm-range parser | [CODE-ARCHITECTURE.md](CODE-ARCHITECTURE.md) |
| Build metadata | tags with `+meta` break last-release lookup ([#2355](https://github.com/semantic-release/semantic-release/issues/2355)) | tags matched by version, metadata ignored; optional `tags.metadata` on the git tag only | [CONFIG.md](CONFIG.md) |
| 0.x and manual versions | no 0.x support; no manual version | 0.x policy (breaking → minor); `Release-As:` footer for 1.0.0 and any forced version | [CONFIG.md](CONFIG.md) |
| Maintenance branches | Default pattern is an extglob `+([0-9])?(.{+([0-9]),x}).x`; a name shaped like a range (`1.x`) or a `range` key makes any entry a maintenance branch | Built-in matcher, written `{ maintenance = "N.x" }` (same `1.x`/`1.x.x`/`1.2.x` shapes); range-shaped names and `range` on a `name` entry are rejected with a hint, never silently turned into release branches; `migrate` converts them | [CONFIG.md](CONFIG.md#3-branches) |
| `prerelease: false` | Same as no `prerelease` (JS configs may compute it) | Rejected: semoxide config is data, so a release branch has no `prerelease` key; `migrate` drops it | [CONFIG.md](CONFIG.md#3-branches) |
| `+` in a prerelease identifier | Accepted (`1.0.0-<id>.1` is valid SemVer), so `beta+build` puts `+build.1` into build metadata | Rejected: an identifier is letters, digits, `-` and `.` | [CONFIG.md](CONFIG.md#3-branches) |
| `channel: ""` | Kept, treated like the default channel | Rejected; `channel = false` is the default channel | [CONFIG.md](CONFIG.md#3-branches) |

## Plugins

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Plugin mechanism | npm modules loaded with `import()`, inline function plugins | Separate processes speaking gRPC over a local socket (Unix socket / named pipe, never stdio); Rust plugins can also run in-process | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Getting plugins | `npm install` | Version pinned in config, binary downloaded, sha256 recorded in a lock file (`semoxide sync`); `path` / PATH override | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Bundled plugins | commit-analyzer, release-notes-generator, npm, github | commit-analyzer and release-notes compiled in; first official set: github, gitlab, cargo, exec, changelog, git | [ARCHITECTURE.md](ARCHITECTURE.md) |
| npm | Bundled `@semantic-release/npm` | Not in the first plugin set | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Plugin environment | Plugins share the full process env | Plugin gets system vars, its manifest-declared secrets and a per-run token only | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Plugin git access | Plugins run git themselves | Plugins push only via the host `Git` service, which pushes only the release tag and never moves existing tags | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Lifecycle steps | 9 steps, camelCase | Same 9 steps in snake_case plus `rollback`, plus an optional `plan` call for dry runs | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Step order | Config order; undocumented per-step keys (`verifyConditions`, `publish`, …) replace a step's plugin list | `steps.plugins` order, documented per-step override (`steps.<step>.order`) | [CONFIG.md](CONFIG.md) |
| Timeouts | None | Per-step deadlines; on timeout or crash the plugin and its children are killed | [ARCHITECTURE.md](ARCHITECTURE.md) |

## Git and credentials

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Git backend | git CLI | git2 (libgit2) only; no git CLI needed | [ARCHITECTURE.md](ARCHITECTURE.md) |
| SSH | System `ssh` via the git CLI | Built-in pure-Rust SSH (no libssh2, no external binary); system `ssh` transport opt-in | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Credential choice | Credentials already on the remote URL can win over the token | A configured token (`GITHUB_TOKEN`, `GITLAB_TOKEN`, …) always wins; `git@host:` pushed over HTTPS when a token exists | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Credential logging | Logged only at debug level, only when several token vars exist | The credential used is logged by name; a warning when the token won't trigger downstream CI | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Channel notes | `refs/notes/semantic-release-<tag>` (+ legacy shared ref), read with one glob that can concatenate notes into invalid JSON (#4073) | writes `refs/notes/semoxide-<tag>`; reads upstream's refs too, each separately | [ARCHITECTURE.md](ARCHITECTURE.md#6-git-and-credentials) |
| Commit-back | `@semantic-release/git` commits assets | Tags only by default; configuring the `git` plugin opts in to commit-back | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Bot identity | `semantic-release-bot <semantic-release-bot@martynus.net>`, set on CI only when `GIT_AUTHOR_*`/`GIT_COMMITTER_*` are unset | Config → `GIT_AUTHOR_*`/`GIT_COMMITTER_*` → CI platform bot → `semoxide-bot` | [ARCHITECTURE.md](ARCHITECTURE.md) |

## Release flow and failure

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Dry-run | Still checks push permission | Needs no push rights and does no network writes; `--verify-push` opts in to the check | [CLI.md](CLI.md) |
| Dry-run output | Write steps skipped with a "Skip step … in dry-run mode" warning | Write steps replaced by a typed "would do" plan | [CLI.md](CLI.md) |
| Partial failure | No handling; the pushed tag stays | `rollback` step: plugins undo their work, the core deletes the tag it pushed; exit code 5 | [ARCHITECTURE.md](ARCHITECTURE.md) |
| `fail` step | Runs only if a `SemanticReleaseError` is present | Always runs on failure; each error marked `known` or `unexpected` | [ARCHITECTURE.md](ARCHITECTURE.md) |
| No release | Free-text log reason; the API returns `false` | Always a typed no-release reason plus hint | [OBSERVABILITY.md](OBSERVABILITY.md) |
| Monorepo | Not supported (community add-ons, one run per package) | Independent units under `[packages.NAME]` in one run; units that depend on each other are an error in v1 | [ARCHITECTURE.md](ARCHITECTURE.md) |

## Output, logging and errors

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Streams | `log`/`success` and dry-run notes on stdout, `warn`/`error` on stderr | Logs on stderr, data only on stdout | [OBSERVABILITY.md](OBSERVABILITY.md) |
| Machine output | None | `--output=json` on every command with a versioned schema | [CLI.md](CLI.md) |
| Next version only | no command ([#1647](https://github.com/semantic-release/semantic-release/issues/1647)); `--dry-run` + exec workaround writes nothing when there is no release | `semoxide version`: the version, or empty stdout + exit 0 when there is no release | [CLI.md](CLI.md) |
| CLI config flags | `-b -r -t -p -e` and long forms for a few options | generic `--set key=value` for any key; only `-v`/`-q` short flags; `migrate` rewrites old flags | [CLI.md](CLI.md) |
| Release command | bare `semantic-release` runs the release | `semoxide release`; bare `semoxide` prints help | [CLI.md](CLI.md) |
| CI outputs | none in core; wrapper actions (e.g. cycjimmy) write step outputs | the core writes `$GITHUB_OUTPUT` with the cycjimmy action's names, plus a step summary | [OBSERVABILITY.md](OBSERVABILITY.md) |
| `success` step fails | `fail` runs (opens a "release failed" issue), exit 1, though published ([github#738](https://github.com/semantic-release/github/issues/738)) | release stands, warnings, exit 0 by default; `steps.success.errors = "fail"` → exit 5; never rollback | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Exit codes | 0 or 1 | Distinct codes per failure class (usage, config, verify, partial failure, …) | [CLI.md](CLI.md) |
| Error codes | Mnemonics like `ENOGITREPO` | Namespaced names like `core::no_git_repo`, `github::release_exists`; each with a docs page | [OBSERVABILITY.md](OBSERVABILITY.md) |
| Masking | Patches global stdout/stderr; raw and URL-encoded forms | Masking at source plus an output pass; also base64 forms, runtime-registered secrets and a `secrets.mask_env` list | [OBSERVABILITY.md](OBSERVABILITY.md) |
| Debug logging | `--debug` flag (CLI only) or `DEBUG=semantic-release:*` via the `debug` module | `SEMOXIDE_LOG` (fallback `RUST_LOG`), `-v`/`--debug`, `--log-file` | [OBSERVABILITY.md](OBSERVABILITY.md) |

## CI and environment

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| Runtime | Node.js ≥ 22 required to run (npm package) | a single static binary; no Node, npm or other runtime | [ARCHITECTURE.md](ARCHITECTURE.md#9-distribution) |
| Environment | Core mutates `process.env` (`GIT_ASKPASS`, `GIT_AUTHOR_*`, …) | Never mutates the env; reads an explicit snapshot and builds child envs from it | [OBSERVABILITY.md](OBSERVABILITY.md) |
| CI detection | env-ci, ~31 vendors | GitHub Actions, GitLab CI, Jenkins, CircleCI, Azure Pipelines, Bitbucket Pipelines, plus `SEMOXIDE_CI_BRANCH` / `SEMOXIDE_CI_IS_PR` overrides on any CI | [ARCHITECTURE.md](ARCHITECTURE.md) |

## Library

| Area | semantic-release | semoxide | Details |
| --- | --- | --- | --- |
| API shape | `semanticRelease(options, {cwd, env, stdout, stderr})` → `Result \| false` | `Semoxide::builder()` → `.run()` → `RunReport`, plus read-only `analyze()`, `notes()`, `explain()`; individual write steps not public; library never prints | [CODE-ARCHITECTURE.md](CODE-ARCHITECTURE.md) |
| Custom plugins | Inline function plugins | Plugin crates registered in-process in the builder | [ARCHITECTURE.md](ARCHITECTURE.md) |
