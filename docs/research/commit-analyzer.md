# @semantic-release/commit-analyzer: research

Source: `semantic-release/commit-analyzer` master @ `96bba2f` (2026-10-04), diffed against `beta` (v14). ~200 LOC. Step: `analyzeCommits(pluginConfig, context) -> "major"|"minor"|"patch"|... |null`.

## 1. Features

| Feature | File |
|---|---|
| Single export `analyzeCommits`, which loops over commits and returns the highest type | `index.js` |
| Skips commits whose message is empty or only whitespace (debug log only) | `index.js:36` |
| Parses each commit with `CommitParser(config).parse(message)` and merges the result into the raw commit object | `index.js:44` |
| Drops revert pairs (a revert plus the commit it reverts) inside the analysed range | `index.js:35` (`filterRevertedCommitsSync`) |
| Custom `releaseRules` first, falling back to the default rules only if the custom result is `undefined` | `index.js:58-68` |
| Stops early once a commit or rule reaches `major` | `index.js:82`, `lib/analyze-commit.js:35` |
| Rule matching: lodash `isMatchWith` (deep partial) with micromatch for string pairs | `lib/analyze-commit.js:18-27` |
| Special rule keys `breaking`, `revert` and `release` | `lib/analyze-commit.js:19-23` |
| Release-type ordering and comparison | `lib/compare-release-types.js`, `lib/default-release-types.js` |
| Built-in rules for the angular, atom, ember, eslint, express and jshint conventions | `lib/default-release-rules.js` |
| Loads `releaseRules` from an inline array or from a module path or package name, then validates it | `lib/load-release-rules.js` |
| Loads parser options from a preset, a config package or the angular default, then shallow-merges `parserOpts` | `lib/load-parser-config.js` |

## 2. Config options

| Option | Type | Default | Semantics |
|---|---|---|---|
| `preset` | string | `angular` (implicit) | Lowercased, then imports `conventional-changelog-${preset}` from the plugin dir and falls back to `cwd`. The imported function is called as `fn(presetConfig)`. Only `.parser` is used. **Wins over `config`.** |
| `config` | string | – | Package name or path, imported the same way and called **without args**. Only `.parser` is used. |
| `parserOpts` | object | – | Shallow `{...preset.parser, ...parserOpts}`. The README says that "without preset/config only parserOpts are used", which is **wrong**: angular is always loaded as the base. |
| `releaseRules` | array \| string | `undefined` (then only `DEFAULT_RELEASE_RULES` apply) | A string is imported as a module (plugin dir first, then `cwd`). Validation: must be an array, each item an object with a `release` key whose value is a valid type, `false` or `null`. |
| `presetConfig` | object | `undefined` | Passed to the preset function only. It is not passed for `config`. Required by `conventionalcommits`. |

Valid `release` values: `major, premajor, minor, preminor, patch, prepatch, prerelease, false, null` (`lib/default-release-types.js`). Index order is the priority, with lower index meaning higher priority.

Beta/v14 (not on master yet): drops the atom/ember/eslint/express/jshint presets. `angular` and `conventionalcommits` become bundled prod deps. Removes the unused `conventional-changelog-writer` dep. Node `^22.22.2 || >=24.15`. The default rules are **unchanged**, and the default preset is still angular, after #858 switched it to conventionalcommits and `ce52d65` reverted that.

## 3. Algorithm

```
rules     = loadReleaseRules()          // undefined | Rule[]
parser    = CommitParser(loadParserConfig())
commits'  = filterRevertedCommitsSync(commits.filter(msg.trim()).map(c => {rawMsg, message, ...c, ...parser.parse(message)}))
result    = null
for c in commits':
    log("Analyzing commit: %s", rawMsg)
    t = rules ? analyzeCommit(rules, c) : undefined
    if t === undefined: t = analyzeCommit(DEFAULTS, c)    // false/null from custom rules does NOT fall back
    log(t ? "The release type for the commit is %s" : "The commit should not trigger a release")
    if t && higher(result, t): result = t
    if result == "major": break
log("Analysis of %s commits complete: %s release", commits.length /*incl. empty*/, result || "no")
return result                                             // string | null, never false
```

`analyzeCommit(rules, commit)`:
1. Filters the rules. A rule matches if all of these hold:
   - `breaking` is truthy and `commit.notes.length > 0`. **Any note counts**, not only `BREAKING CHANGE` (#335).
   - `revert` is truthy and `commit.revert` is truthy. The parser's `revertPattern` sets `commit.revert`.
   - The remaining keys match through `_.isMatchWith(commit, rest, cust)`. This is a deep partial match: nested objects are a subset match, and arrays are an unordered subset. The customizer is used only when **both** values are strings, and calls `micromatch.isMatch(value, pattern)`. Everything else uses lodash `isEqual`, so `{scope: 1}` works. A rule with no criteria, `{release:"patch"}`, matches everything (#339).
2. Folds the matches in rule order with `higher(cur, new) = !cur || idx(new) < idx(cur)`, where `idx(false|null) = -1`. Consequences:
   - A truthy type after a falsy one always replaces it. A **falsy type after a truthy one also always replaces it**, except when the current value is `major`, because the fold stops at `major`. So a trailing `{release:false}` acts as a veto that depends on rule order (README `no-release` example, #377, #122).
   - Otherwise the higher type wins.
3. Returns `undefined` when no rule matched. Only `undefined` triggers the fallback to the default rules.

Matching notes:
- A rule value is a **glob, not a regex**. JSON has no RegExp, and a JS RegExp in a rule falls through to `isEqual` and never matches a string. micromatch is path-oriented: `*` does not cross `/` (#175), leading `.` is treated as a dotfile, and picomatch on win32 converts `\` to `/`. Braces and extglobs (`{a,b}`, `+(x|y)`) work. Matching is case-sensitive (#496, #641).
- Rule keys can be any field of the merged commit object. Parser fields (`type, scope, subject, header, body, footer, notes, references, mentions, merge, revert` plus custom `headerCorrespondence` names such as eslint `tag`/`message`) **override** the raw semantic-release commit fields of the same name (`subject`, `body`, `message`). Raw fields such as `hash`, `author.email` and `committerDate` stay matchable.
- If a custom rule matches, it **suppresses all default rules for that commit**. With `{type:"refactor", release:"patch"}`, a `refactor!:` commit gives only a patch unless the user also adds `{breaking:true, release:"major"}` (#413, #805).
- Revert handling: a revert whose target is in the range removes both commits. A revert whose target is outside the range stays and becomes a `patch` through the default `{revert:true}` rule.
- The preset's `whatBump`/recommended-bump is **ignored** (#171). Release rules are separate from the preset.

Default rules (`lib/default-release-rules.js`):

| Rule | Release |
|---|---|
| `breaking:true` | major |
| `revert:true` | patch |
| `type`: feat / fix / perf | minor / patch / patch |
| `emoji`: `:racehorse:` `:bug:` `:penguin:` `:apple:` `:checkered_flag:` | patch |
| `tag`: BUGFIX / FEATURE / SECURITY | patch / minor / patch |
| `tag`: Breaking / Fix / Update / New | major / patch / minor / minor |
| `component`: perf / deps | patch |
| `type`: FEAT / FIX | minor / patch |

## 4. Side effects

- `context.logger.log` writes 1 line per commit for analysis, 1 line per commit for the result, and 1 summary line. The summary count includes skipped empty commits.
- `debug("semantic-release:commit-analyzer")` logs skipped empty commits, the custom or default rule path, and each match.
- Dynamic `import()` of presets and rules from the plugin dir and `cwd` can execute arbitrary JS. There is no other I/O and no context mutation.
- Errors: the `TypeError`/`Error` messages from the rule validation, `MODULE_NOT_FOUND` for an unknown preset or config, and parser regex errors, which are re-thrown.

## 5. Rust port notes

| JS dep | Use | Rust equivalent |
|---|---|---|
| `conventional-commits-parser` `CommitParser(opts).parse(msg)` | Header/body/notes/revert parsing | [dependencies §1](dependencies.md#1-conventional-commits-parser-712). Interface needed: `parse(&str) -> Commit` with an open field map for custom correspondences. |
| `conventional-commits-filter` `filterRevertedCommitsSync` | Revert-pair removal | [dependencies §2](dependencies.md#2-conventional-commits-filter-601) |
| `conventional-changelog-<preset>` (only `.parser` used) | Parser options | Built-in static data ([dependencies §4](dependencies.md#4-presets)), user presets as TOML/YAML parser options. |
| `micromatch` | String globs | `globset` with `literal_separator(false)` (`*` crosses `/`, fixing #175), or `glob-match`. Optional `regex` crate for `/…/` values. |
| `lodash isMatchWith` | Deep partial match | Small recursive fn over `serde_json::Value`, or typed `Rule { type_, scope, … }` plus `extra: Map`. |
| `debug` | Trace logs | `tracing` |
| `import-from-esm` | Load modules from the plugin dir or `cwd` | N/A. Rules are inline config only, or a file path to TOML/JSON. |

JS-specific, so drop or redesign:
- Presets resolved as npm packages: `conventional-changelog-${name}`, the plugin dir before `cwd`, a dual async-function or object shape, and `presetConfig` passed only to `preset`. This is the source of most support issues (#517, #589, #642, #921).
- `releaseRules` given as a JS module path, and the request for JS function rules (#627).
- `false` vs `null` vs `undefined` tri-state. In Rust, use `enum RuleRelease { Bump(Bump), NoRelease }` with `Option<…>` for "no match".
- Prerelease types (`premajor`…`prerelease`) are accepted in rules, but the core only reasons about major/minor/patch. Consider restricting to `Major|Minor|Patch|None`.

## 6. Tests (ava; `test/`)

| File | # | Covers | Port 1:1 as table? |
|---|---|---|---|
| `compare-release-types.test.js` | 1 test with 13 asserts | Ordering, falsy candidates | Yes: `(cur, new, expected)` |
| `analyze-commit.test.js` | 12 | breaking, revert, combined criteria, glob, highest wins, `false`/`null` result, no match | Yes: `(rules, commit_json, expected)` |
| `load-release-rules.test.js` | 9 | Inline array, module path, undefined, preserving `false`/`null`, invalid type, missing `release`, non-array, `undefined` item | Validation cases yes. Module loading no. |
| `load-parser-config.test.js` | 7 + 14 macro | Angular default, `parserOpts` override, preset/config × 7 presets, missing module, `importFrom.silent` fallback (sinon) | Override/merge only. Preset loading is JS-only. |
| `integration.test.js` (separate npm script) | 25 | End to end: message list → type plus logged lines. Covers presets, `parserOpts`, revert pairs, custom vs default fallback, glob on `message`, rule order, `false` precedence, empty commits, errors | Yes for angular/conventionalcommits cases: `(opts, messages[], expected_type)`. eslint-preset cases need an eslint parser config or must be dropped. Log assertions → snapshot (`insta`). |

Fixtures: `release-rules.cjs` (4 rules), `release-rules-invalid.cjs` (`42`).

## 7. Issue history (120 non-bot issues; ranked by reactions + comments)

**Recurring problems**

| Problem | Issues |
|---|---|
| `!` and `BREAKING CHANGE` not giving major. Cause: the angular default ignores `!`, custom rules shadow the default `breaking`, or preset version skew | [#231](https://github.com/semantic-release/commit-analyzer/issues/231) [#759](https://github.com/semantic-release/commit-analyzer/issues/759) [#413](https://github.com/semantic-release/commit-analyzer/issues/413) [#805](https://github.com/semantic-release/commit-analyzer/issues/805) [#153](https://github.com/semantic-release/commit-analyzer/issues/153) [#53](https://github.com/semantic-release/commit-analyzer/issues/53) [#108](https://github.com/semantic-release/commit-analyzer/issues/108) [#139](https://github.com/semantic-release/commit-analyzer/issues/139) [#439](https://github.com/semantic-release/commit-analyzer/issues/439) [#748](https://github.com/semantic-release/commit-analyzer/issues/748) |
| Preset major bumps break silently: wrong preset version or a missing module leads to "should not trigger a release" or exit 0 | [#517](https://github.com/semantic-release/commit-analyzer/issues/517) [#642](https://github.com/semantic-release/commit-analyzer/issues/642) [#589](https://github.com/semantic-release/commit-analyzer/issues/589) [#523](https://github.com/semantic-release/commit-analyzer/issues/523) [#921](https://github.com/semantic-release/commit-analyzer/issues/921) |
| Commits that are not parsed at all: squash or merge subjects, a PR body, a `/` in the message | [#65](https://github.com/semantic-release/commit-analyzer/issues/65) [#177](https://github.com/semantic-release/commit-analyzer/issues/177) [#175](https://github.com/semantic-release/commit-analyzer/issues/175) [#535](https://github.com/semantic-release/commit-analyzer/issues/535) |
| Case sensitivity (`Feat:`, `feat(ABC)`) | [#496](https://github.com/semantic-release/commit-analyzer/issues/496) [#641](https://github.com/semantic-release/commit-analyzer/issues/641) |
| `release:false` precedence is confusing, and there is no way to disable the default rules | [#377](https://github.com/semantic-release/commit-analyzer/issues/377) [#122](https://github.com/semantic-release/commit-analyzer/issues/122) [#612](https://github.com/semantic-release/commit-analyzer/issues/612) |
| `presetConfig` and `config` are poorly documented | [#263](https://github.com/semantic-release/commit-analyzer/issues/263) [#262](https://github.com/semantic-release/commit-analyzer/issues/262) |

**Rejected or not planned (reason)**

| Request | Reason |
|---|---|
| [#174](https://github.com/semantic-release/commit-analyzer/issues/174) Filter by scope (21 reactions, the top request) | Monorepos out of scope ([details](distribution-config.md#3-monorepo-scope)) |
| [#252](https://github.com/semantic-release/commit-analyzer/issues/252) Filter by path (open, 10 reactions) | Same monorepo stance. Never implemented. |
| [#65](https://github.com/semantic-release/commit-analyzer/issues/65) Parse squashed commit bodies | Conventions define one header per commit, and squashing is used to rewrite messages. Community plugin `semantic-release-unsquash`. |
| [#508](https://github.com/semantic-release/commit-analyzer/issues/508) Multiple headers in one commit | The convention defines one header |
| [#116](https://github.com/semantic-release/commit-analyzer/issues/116), [#811](https://github.com/semantic-release/commit-analyzer/issues/811) Strict mode: fail on unmatched commits | Too late after merge. Use commitlint. |
| [#496](https://github.com/semantic-release/commit-analyzer/issues/496) Case-insensitive types | Angular is case-sensitive. Pick another convention. |
| [#500](https://github.com/semantic-release/commit-analyzer/issues/500) Types with spaces | A type describes the change, not the bump |
| [#847](https://github.com/semantic-release/commit-analyzer/issues/847) Filter by author | Fix the bot's commit types instead |
| [#171](https://github.com/semantic-release/commit-analyzer/issues/171) Use the preset's `recommendedBump`/`whatBump` | Never adopted (open) |
| [#680](https://github.com/semantic-release/commit-analyzer/issues/680) conventionalcommits as the default | Moved to semantic-release#3406. Implemented in #858, then reverted on beta. |

**Open known problems**: [#335](https://github.com/semantic-release/commit-analyzer/issues/335) (any note counts as breaking), [#776](https://github.com/semantic-release/commit-analyzer/issues/776) (a body line starting with "Breaking change" gives major), [#175](https://github.com/semantic-release/commit-analyzer/issues/175) (`/` breaks globs), [#377](https://github.com/semantic-release/commit-analyzer/issues/377) (false precedence), [#413](https://github.com/semantic-release/commit-analyzer/issues/413)/[#805](https://github.com/semantic-release/commit-analyzer/issues/805) (custom rules shadow `breaking`), [#627](https://github.com/semantic-release/commit-analyzer/issues/627) (function rules), [#591](https://github.com/semantic-release/commit-analyzer/issues/591) (use the standard preset loader), [#340](https://github.com/semantic-release/commit-analyzer/issues/340) (no way to silence logs), [#180](https://github.com/semantic-release/commit-analyzer/issues/180) (cap the release type).

## Ticket candidates

- **Commit analyzer core**: `analyze(commits, &Config) -> Option<Bump>` with an early exit on major. Pure, no I/O.
- **Rule model and matcher**: typed `Rule` with arbitrary field criteria, `breaking`, `revert`, and `release: Bump|NoRelease`. Glob that crosses `/`, plus opt-in `/regex/`, plus an opt-in case-insensitive flag.
- **Rule precedence semantics**: define and document a deterministic order. Proposal: highest bump wins, `NoRelease` is an explicit veto only when marked `veto`, and the default `breaking → major` always applies unless disabled (#377, #413, #805).
- **Default rule sets per convention**: angular and conventionalcommits only. Drop the atom/ember/eslint/jshint/express rules, as upstream beta does.
- **`use_default_rules: bool`**: opt out of the fallback rules (#122, #612).
- **Built-in presets as data**: parser options for angular and conventionalcommits embedded, user presets as TOML. No package resolution.
- **Breaking detection**: count only the configured note keywords and `!`, not any note (#335, #776).
- **Lone revert → patch** rule. Pair filtering: [dependencies](dependencies.md#ticket-candidates).
- **Empty-message skip and log parity**: per-commit structured events (`tracing`) instead of format strings. Silenceable (#340).
- **Port the test tables**: compare-types, analyze-commit and the integration cases as table-driven tests, with `insta` snapshots for the per-commit decisions.
- **Optional `max_bump` cap** (#180). Unmatched-commit report (#116): [CC spec](../specs/CONVENTIONAL-COMMITS-SPEC.md#ticket-candidates).
