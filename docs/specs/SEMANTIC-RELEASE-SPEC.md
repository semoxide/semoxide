# semantic-release behavioral spec (condensed)

Sources: docs repo `semantic-release/docs` @ `b8513ff` (2026-10-04, site https://semantic-release.org; the old `semantic-release/semantic-release/docs` folder **no longer exists**). Where the docs are silent or wrong, the core source `semantic-release/semantic-release` @ `04c1923` was checked; those items are tagged **[src]**. Docs vs source conflicts are tagged **[conflict]**.

## 1. Model
Links: [How it works](https://semantic-release.org/foundation/how-it-works/) · [Considerations](https://semantic-release.org/foundation/considerations/) · [Intro](https://semantic-release.org/intro/)

- Runs in CI after all tests pass, on a release branch. Answers three questions: release or not, which version, where to publish.
- Inputs: commits since the last tag reachable from the branch, branch config, plugin pipeline, CI credentials.
- Default convention is Angular: `fix`/`perf` → patch, `feat` → minor, `BREAKING CHANGE:` in the footer → major. No relevant commit → no release.
- The version source is git tags only. Nothing is committed back by default; `package.json` keeps something like `0.0.0-development` ([FAQ](https://semantic-release.org/support/faq/)).
- Commits whose message matches `/\[skip\s+release]|\[release\s+skip]/i` are dropped before `analyzeCommits` ([FAQ](https://semantic-release.org/support/faq/#can-i-exclude-commits-from-the-analysis), regex **[src]**).

## 2. Lifecycle
Links: [Release steps](https://semantic-release.org/foundation/release-steps/) · [Plugins](https://semantic-release.org/foundation/plugins/)

### 2.1 Hooks

| Hook | Req. | Runs in dry-run | Multi-plugin semantics | Return value |
|---|---|---|---|---|
| `verifyConditions` | no | yes | all plugins run, errors aggregated (settleAll) **[src]** | ignored |
| `analyzeCommits` | **yes**; defaults to `@semantic-release/commit-analyzer` if none is configured | yes | highest type wins | `"patch"\|"minor"\|"major"\|null/undefined`; anything else → `EANALYZECOMMITSOUTPUT` **[src]** |
| `verifyRelease` | no | yes | settleAll **[src]** | ignored |
| `generateNotes` | no | yes | outputs concatenated with `\n\n`; each plugin sees prior notes in `nextRelease.notes` **[src]** | string or empty; else `EGENERATENOTESOUTPUT` |
| `prepare` | no | **no** | sequential; if `HEAD` moved after a plugin, `nextRelease.gitHead` is updated and notes regenerated before the next plugin **[src]** | ignored |
| `publish` | no | **no** | sequential, stops on first error | plain object `{name?, url?, channel?, …}` or `false`/empty; merged into a release entry `{...nextRelease, ...result, pluginName}`; else `EPUBLISHOUTPUT` **[src]** |
| `addChannel` | no | **no** | like `publish` | like `publish`; else `EADDCHANNELOUTPUT` |
| `success` | no | **no** | settleAll | ignored |
| `fail` | no | **no** | settleAll | ignored |

Plugins run in `plugins` array order within each step. Steps run in fixed order.

### 2.2 Actual run order [src] ([conflict] with the docs table)
The docs put "Create Git Tag" before Prepare and "Add Channel" between tag and Prepare. The source does this:

1. Load config (§3), detect CI (`env-ci`).
2. Not in CI and no `--no-ci` → force `dryRun=true` with a warning. In CI → default `GIT_AUTHOR_*`/`GIT_COMMITTER_*` to `semantic-release-bot <semantic-release-bot@martynus.net>` unless already set, and set `GIT_ASKPASS=echo`, `GIT_TERMINAL_PROMPT=0`.
3. CI build triggered by a PR (and CI checks on) → log and return `false`.
4. Core verify: inside a git repo (`ENOGITREPO`), `repositoryUrl` set (`ENOREPOURL`) and not starting with `-` (`EINVALIDREPOURL`), tagFormat valid (§5), branch entries valid (`EINVALIDBRANCH`).
5. Resolve the auth URL (§6), then expand and fetch branches, tags and notes (§4).
6. Current branch not configured → log and return `false`.
7. `git push --dry-run --no-verify <url> HEAD:<branch>`. On failure: if local is behind remote → return `false`; else `EGITNOPERMISSION`. **This also runs in dry-run.**
8. `verifyConditions`.
9. **Merged-release promotion**: if a release from a higher branch exists on this branch's history but not on its channel, then (maintenance: version must satisfy `mergeRange`, else `EINVALIDMAINTENANCEMERGE`) `generateNotes` → add the channel to the git note and push it → `addChannel` → `success`. At most one version per run (the highest).
10. Get last release (§4.4) → collect commits `lastRelease.gitHead..HEAD`.
11. `analyzeCommits` → no type → return `{releases}` if step 9 produced any, else `false`.
12. Compute the next version (§4.5) and tag. Non-prerelease version outside `branch.range` → `EINVALIDNEXTVERSION`.
13. `verifyRelease` → `generateNotes` → `prepare`.
14. Not dry-run: `git tag` → add note `{"channels":[channel]}` → `git push --tags` → push the note ref.
15. `publish` → `success`.
16. Dry-run: print the notes (markdown rendered to the terminal).
17. On error: call `fail` with `errors` **only if** at least one error is a `SemanticReleaseError` (`error.semanticRelease`); then log and rethrow.

## 3. Configuration
Link: [Configuration](https://semantic-release.org/usage/configuration/)

**Sources** (cosmiconfig name `release`, first match wins): `.releaserc` (YAML/JSON, optional ext `.yaml|.yml|.json|.js|.ts|.cjs|.mjs`), `release.config.(js|ts|cjs|mjs)`, or the `package.json` `release` key.

**Precedence (high→low):** CLI/API options > config file > `extends` configs (later entry overrides earlier) > defaults. Merges are **shallow**: `plugins` and `branches` replace, they never merge. `null`/`undefined` values fall back to defaults **[src]**.

| Option | Type | Default | CLI |
|---|---|---|---|
| `extends` | string \| string[] | — | `-e, --extends` |
| `branches` | string \| object \| array (micromatch globs) | `['+([0-9])?(.{+([0-9]),x}).x','master','main','next','next-major',{name:'beta',prerelease:true},{name:'alpha',prerelease:true}]` | `--branches` (also `-b` **[src]**) |
| `repositoryUrl` | string | `package.json` `repository`, else `git config remote.origin.url` | `-r, --repository-url` |
| `tagFormat` | string (lodash template) | `v${version}` | `-t, --tag-format` |
| `plugins` | array of `name` \| `[name, opts]` | `commit-analyzer, release-notes-generator, npm, github` | `-p, --plugins` (names only) |
| `dryRun` | bool | `false` in CI, `true` otherwise | `-d, --dry-run` |
| `ci` | bool | `true` | `--ci` / `--no-ci` |
| `debug` | bool | `false` | `--debug` only (or `DEBUG=semantic-release:*`) |

- CLI list args take comma- or space-separated values; a single `false` means an empty list **[src]**.
- Undocumented **[src]**: per-step overrides `--verify-conditions --analyze-commits --verify-release --generate-notes --prepare --publish --success --fail`, also usable as config keys (e.g. `publish: [...]`). An object without `path` under a step key merges options into that step's plugins.
- **Global plugin options:** every non-core root key is passed to every plugin. Each plugin's `pluginConfig = {...rootOptions, ...pluginTupleOptions}` (deep-cloned) **[src]**.
- Plugin entries can be a module name, a path, a `[name, opts]` tuple, `{path, ...opts}`, or (JS API) an inline object of hook functions **[src]**.
- Tag metadata env: `GIT_AUTHOR_NAME/EMAIL`, `GIT_COMMITTER_NAME/EMAIL` (default `semantic-release-bot`).
- Adopting on an existing project: the last released commit must be in the release branch history and tagged per `tagFormat` ([Existing version tags](https://semantic-release.org/usage/configuration/#existing-version-tags)).

## 4. Branches, channels, ranges
Links: [Workflow configuration](https://semantic-release.org/foundation/workflow-configuration/) · [Supported branching](https://semantic-release.org/foundation/supported-branching/) · recipes: [channels](https://semantic-release.org/recipes/release-workflow/distribution-channels/), [maintenance](https://semantic-release.org/recipes/release-workflow/maintenance-releases/), [pre-releases](https://semantic-release.org/recipes/release-workflow/pre-releases/)

### 4.1 Branch properties

| Prop | Applies to | Default | Notes |
|---|---|---|---|
| `name` | all | string value / each glob match | Required. A glob expands per matching **remote** branch (`git ls-remote --heads`). No match → definition silently ignored. |
| `channel` | all | first release branch: `undefined` (default channel); others: `name` | `false` forces the default channel. String is a lodash template with `${name}`. |
| `range` | maintenance | `name` | Must match `N.x`, `N.x.x` or `N.N.x`. Required unless `name` has that shape. |
| `prerelease` | prerelease | — | Required. `true` → use `name`. String template with `${name}`. `1.0.0-<id>.1` must be valid semver. |

All string props are lodash templates evaluated with `{name}` **[src]**.

### 4.2 Type detection [src]
- **maintenance**: `range` set (not `false`), or `name` matches `^\d+(\.(\d+|x))?\.x$`.
- **prerelease**: `prerelease` set and not `false`.
- **release**: everything else.

### 4.3 Validation errors

| Code | Rule |
|---|---|
| `ERELEASEBRANCHES` | 1 to 3 release branches (after glob expansion and dropping missing branches) |
| `EMAINTENANCEBRANCH(ES)` | range shape invalid / ranges not unique |
| `EPRERELEASEBRANCH(ES)` | invalid id / ids not unique |
| `EDUPLICATEBRANCHES`, `EINVALIDBRANCHNAME` | duplicate names / `git check-ref-format` fails |

Branch order in the result: maintenance (sorted by range) → release (config order) → prerelease.

### 4.4 Tags, notes, last release
- Branch tags come from `git tag --merged <branch>`. A tag counts as a release if it matches the regex built from `tagFormat` (with `version` → `(.+)`) and the captured version is valid semver.
- Channels per tag live in git notes, JSON `{"channels":[null,"next",…]}` (`null` = default channel). Current ref is `refs/notes/semantic-release-<tag>` (per tag); the legacy shared `refs/notes/semantic-release` is still read **[src]**. A tag with no note means `[null]`. **[conflict]** The [troubleshooting page](https://semantic-release.org/support/troubleshooting/) still documents only the legacy ref.
- Last release = highest version among branch tags that are non-prerelease, or (on a prerelease branch) prereleases with this branch's id that are on this branch's channel.

### 4.5 Next version [src]
- No last release: `1.0.0`, or `1.0.0-<pre>.1` on a prerelease branch. Starting at `0.x` is unsupported ([FAQ](https://semantic-release.org/support/faq/#can-i-set-the-initial-release-version-of-my-package-to-001)).
- Release or maintenance branch: `inc(last, type)`.
- Prerelease branch:
  - If `last` is a prerelease on the same channel: `max(inc(last,'prerelease'), inc(highestTagInclPre, type)+'-<pre>.1')`.
  - Otherwise: `inc(major.minor.patch, type)+'-<pre>.1'`.

### 4.6 Range calculation [src]
**Release branches** `R[0..n]`:
- `lastVersion` = running max of each branch's latest release, starting from `R[0]` latest or `1.0.0`.
- `bound` = the first version on `R[i+1]` that is higher than every version on `R[0..i]`. The last branch has no bound.
- `range = ">=lastVersion <bound"`.
- `accept` = release types whose diff stays below the bound. Without a bound, all types are accepted.
- `main` = (`i == 0`).

**Maintenance branches**:
- `min = max(branch latest || 1.0.0, maintenanceMin)`. `maintenanceMin` = lower bound of the range, except for a major range (`N.x`) that follows a non-major range: then it is the upper bound of the previous range.
- `max = min(base, upper(range))`. `base` = the first version of `R[0]` not present on any maintenance branch.
- `range = ">=min <max"`; `accept` derived as above.
- `mergeRange = ">=maintenanceMin <upper(range)"` gates merged-release promotion.

**Prerelease branches**: no range check. A merged existing release is never added to the prerelease channel; the commits are treated as new.

Documented examples to use as test fixtures: [push to release](https://semantic-release.org/foundation/workflow-configuration/#pushing-to-a-release-branch), [push to maintenance](https://semantic-release.org/foundation/workflow-configuration/#pushing-to-a-maintenance-branch), [merge to maintenance](https://semantic-release.org/foundation/workflow-configuration/#merging-into-a-maintenance-branch), [pre-release](https://semantic-release.org/foundation/workflow-configuration/#pushing-to-a-pre-release-branch), plus all three recipe walkthroughs.

### 4.7 Supported workflows
- Supported: trunk-based development, GitHub Flow.
- Unsupported: git-flow, branch-for-release (except late-created maintenance branches), release-for-testing-then-promote, monorepos ([details](../research/distribution-config.md#3-monorepo-scope)).

## 5. tagFormat
- A lodash template with only `${version}` interpolation (`evaluate:false, escape:false`) **[src]**.
- `version` must appear **exactly once** (`ETAGNOVERSION`).
- The output must pass `git check-ref-format refs/tags/<t>` (`EINVALIDTAGFORMAT`). The same template is used to parse existing tags back.

## 6. CI, git and auth
Links: [CI configuration](https://semantic-release.org/usage/ci-configuration/) · [CI recipes](https://semantic-release.org/recipes/ci-configurations/) · [SSH keys](https://semantic-release.org/recipes/git-hosted-services/git-auth-ssh-keys/) · [Git version](https://semantic-release.org/support/git-version/) · [Node version](https://semantic-release.org/support/node-version/)

- **CI detection** uses the [`env-ci`](https://github.com/semantic-release/env-ci) package and provides `isCi`, `commit`, `branch`, `isPr`, `prBranch`. A PR build means no release.
- **Run after all test jobs succeed** (multi-job builds need a gate stage). This is the user's job, not checked by the tool.
- **History**: the tool runs `git fetch --unshallow --tags` (falls back to a plain fetch) for each configured branch and for `+refs/notes/*` **[src]**. Recipes still require `fetch-depth: 0` (GitHub Actions).
- **Git ≥ 2.7.1** (`git tag --merged`). **Node ≥ 22.14** per docs; engines field says `^22.14.0 || >=24.10.0` **[src]**.
- **Push auth resolution** [src]:
  1. Normalise the URL (shortcut → https, `git+http(s)` → http(s)).
  2. Try `push --dry-run` as-is (SSH keys).
  3. On failure, rebuild as https with basic auth from env:

| Env | Credential prefix |
|---|---|
| `GIT_CREDENTIALS` | none (`user:pass`, each part URL-encoded) |
| `GH_TOKEN` | none |
| `GITHUB_TOKEN` | `x-access-token:` only when `GITHUB_ACTION` is set |
| `GL_TOKEN`, `GITLAB_TOKEN` | `gitlab-ci-token:` |
| `BB_TOKEN`, `BITBUCKET_TOKEN` | `x-token-auth:` |
| `BB_TOKEN_BASIC_AUTH`, `BITBUCKET_TOKEN_BASIC_AUTH` | none (`user:token`) |

  If several of these are set, each is tried and the first that passes `push --dry-run` wins.
- **Plugin tokens** (`NPM_TOKEN`, `GH_TOKEN`, …) are the plugins' concern. OIDC trusted publishing is recommended.
- **GitHub Actions job permissions:** `contents: write`, `issues: write`, `pull-requests: write`, `id-token: write`.
- **GitHub Actions pitfalls:** don't set `setup-node` `registry-url`. The default `GITHUB_TOKEN` can't push to protected branches; use a GitHub App token.
- **Secret masking** [src]: every output stream and the `success`/`fail` payloads are filtered. Values of env vars whose name matches `/token|password|credential|secret|private|key|auth|webhook/i` (length ≥ 5, `GOPRIVATE` excluded) are replaced with `[secure]`, in raw and URL-encoded forms.

## 7. Dry-run and local runs
- Skips `prepare`, `publish`, `addChannel`, `success`, `fail` and tag/note creation and push.
- Still runs config load, branch fetch, **push-permission check**, `verifyConditions`, `analyzeCommits`, `verifyRelease`, `generateNotes`. Prints the next version and notes.
- Local release: `--no-ci` plus credentials in env. Discouraged ([FAQ](https://semantic-release.org/support/faq/#can-i-run-semantic-release-on-my-local-machine-rather-than-on-a-ci-server)).
- Recommended invocation is `npx semantic-release@<major>` with pinned plugins ([Running](https://semantic-release.org/usage/running/)).

## 8. JS API
Link: [JS API](https://semantic-release.org/developer-guide/js-api/)

`semanticRelease(options?, {cwd=process.cwd(), env=process.env, stdout, stderr}?) → Promise<Result|false>`

- `options` has the highest precedence (any core or plugin option).
- Rejects on error; the CLI exits 1.
- `debug` isn't an API option (`require('debug').enable(...)`).

| Result field | Shape |
|---|---|
| `lastRelease` | `{version, gitHead, gitTag, channel}`; `{}` if none. **[conflict]** The plugin context doc lists `channels[]` and `name`; the source returns `channels` |
| `commits[]` | `{commit:{long,short}, tree:{long,short}, author:{name,email,date}, committer:{…}, subject, body, message, hash, committerDate, gitTags}`. Docs typo `author.short` = `date` |
| `nextRelease` | `{type, version, gitHead, gitTag, name, notes, channel}` (`channel` `null` = default) |
| `releases[]` | `{name?, url?, type, version, gitHead, gitTag, notes, pluginName, channel}` |

The result is `false` when nothing is released. If only a merged-channel promotion happened, the result is `{releases}` alone **[src]**.

## 9. Plugin contract
Links: [Plugin development](https://semantic-release.org/developer-guide/plugin/) · [Plugins list](https://semantic-release.org/extending/plugins-list/)

- **Module shape:** exports named hook functions, or a default function, which is treated as a single-step plugin **[src]**.
- **Hook signature:** `async hook(pluginConfig, context)`.
- **Errors:** throw `SemanticReleaseError(message, code, details)` or an `AggregateError` of them. Any other error is "unexpected": it is logged and **does not trigger `fail`**.
- **Logger:** `log`/`warn`/`success`/`error`, auto-scoped `[semantic-release] [<plugin>]`. `context` is deep-cloned per call. `stdout`/`stderr`/`logger` are passed through as-is.

Context keys by stage (cumulative):

| Stage | Adds |
|---|---|
| all | `cwd, env, envCi{isCi,commit,branch,…}, options, logger, stdout, stderr, branch{name,type,channel,range,accept,tags,main,prerelease?,mergeRange?}, branches[]` |
| `analyzeCommits` | `commits[], releases[], lastRelease{version,gitTag,channels,gitHead,name}` |
| `verifyRelease` | `nextRelease{type,channel,gitHead,version,gitTag,name}` |
| `generateNotes`/`prepare` | `nextRelease.notes` |
| `addChannel` | as `verifyRelease`, plus `currentRelease` **[src]** |
| `publish`/`success` | `releases[]` |
| `fail` | `errors[]` |

**[conflict]** The plugin-dev page says later `analyzeCommits` plugins *override* earlier results and that `premajor/preminor/prepatch/prerelease` are valid. The plugins page and the source say the **highest of patch/minor/major wins**, and other values throw `EANALYZECOMMITSOUTPUT`.

Official plugins:

| Plugin | Hooks |
|---|---|
| commit-analyzer | analyzeCommits |
| release-notes-generator | generateNotes |
| npm | verifyConditions, prepare, publish |
| github | verifyConditions, publish, success, fail |
| gitlab | verifyConditions, publish |
| git | verifyConditions, prepare |
| changelog | verifyConditions, prepare |
| exec | all hooks |
| apm | verifyConditions, prepare, publish |

The first four are bundled with core. The plugins list page omits `addChannel` for npm and github, but channel promotion needs it. Check the plugin READMEs. The maintenance recipe text says the dist-tag is `@release-1.x` while its diagrams say `@1.x`.

## 10. Shareable configs
Links: [Shareable configurations](https://semantic-release.org/foundation/shareable-configurations/) · [Development](https://semantic-release.org/developer-guide/shareable-configuration/) · [List](https://semantic-release.org/extending/shareable-configurations-list/)

- A package or file that exports a plain config object.
- Resolved first relative to semantic-release's own install, then to `cwd` **[src]**.
- Plugins named in a shareable config resolve **relative to that config package** **[src]**. That is why config packages list their plugins as `dependencies` and semantic-release as a `peerDependency`.
- Shallow override: redefining `plugins` locally replaces the inherited array.

## 11. Documented limitations / non-goals
- No `0.x` versions; no manual "release version X".
- No monorepo support ([details](../research/distribution-config.md#3-monorepo-scope)).
- Max 3 release branches.
- Release versions are unique across channels.
- Commits during release (`@semantic-release/git`) are discouraged.
- Squash merges must keep a conventional message.
- `git push --force` loses tags and notes; recovery is manual ([troubleshooting](https://semantic-release.org/support/troubleshooting/)).
- Never delete a released tag, or the version gets republished.
- `reference already exists` means a same-name tag exists outside the branch history.
- Angular revert format differs from `git revert`.

## 12. JS-specific parts
See [research §6](../research/semantic-release.md#6-rust-port-notes).

## Ticket candidates
- **Core lifecycle engine**: run steps in the source-verified order (§2.2), including merged-release promotion, PR skip, and auto dry-run outside CI.
- **Config loader**: see [ADR 0002](../decisions/0002-config-format.md).
- **Branch model**: release/maintenance/prerelease detection, glob expansion against remote heads, all validation error codes.
- **Range/accept calculation**: port the §4.6 algorithms, with recipe walkthroughs as golden tests.
- **Next-version computation**: first release 1.0.0, prerelease numbering and channel-aware rules (§4.5).
- **Tag format**: `{version}` template ([ADR 0004](../decisions/0004-template-engine.md)), exactly-once check, ref-format validation, strict parse-back, non-matching tags ignored.
- **Git notes channel store**: read legacy and per-tag refs, write `refs/notes/semantic-release-<tag>`, fetch/push notes.
- **Auth URL resolution**: SSH-first, then token env vars with host-specific prefixes; multi-token probing.
- **Plugin protocol/ABI**: hook set, `pluginConfig` + `context` schema, return-value validators, error model (`SemanticReleaseError` vs unexpected).
- **Plugin pipeline semantics**: settleAll hooks, notes concatenation, prepare HEAD-change note regeneration, publish/addChannel release merging.
- **`semoxide migrate`**: convert `.releaserc` to semoxide.toml ([ADR 0001](../decisions/0001-compatibility-stance.md)). No JS plugin bridge.
- **Dry-run mode**: exact skip set; still verifies push permission; prints version and notes.
- **CLI**: flags `-b -r -t -p -e -d --ci/--no-ci --debug`, comma lists, `false` = empty; decide on per-step overrides.
- **Library API**: `run(options, {cwd, env, stdout, stderr}) -> Result | NoRelease`, result types mirroring §8.
- **Docs: decision record on documented-vs-source conflicts**: step order, analyzeCommits override, notes ref, lastRelease.channel(s).
- Elsewhere: `extends` → [distribution-config](../research/distribution-config.md#config); git backend, error catalog, secret masking → [research](../research/semantic-release.md#ticket-candidates); CI detection → [dependencies](../research/dependencies.md#5-env-ci); built-in analyzer → [commit-analyzer](../research/commit-analyzer.md#ticket-candidates), notes → [release-notes-generator](../research/release-notes-generator.md#ticket-candidates), GitHub → [github](../research/github.md#ticket-candidates).
