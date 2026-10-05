# Dependencies and extra plugins: research

Sources (shallow clones, 2026-10-05): `conventional-changelog` monorepo @ `f90c80e`, `env-ci` @ `0a8c00b`, `git` @ `08bfb3d`, `changelog` @ `d780a55`, `exec` @ `3988e52`, `error` @ `bfc9b06`, `semantic-release` master (for lodash template use).
Issue counts = total reactions, open issues, sorted by 👍.

## 0. Version skew (read first)

| Package | Latest | Used by commit-analyzer / release-notes-generator |
|---|---|---|
| conventional-commits-parser | 7.1.2 | `^6.0.0` |
| conventional-changelog-writer | 9.2.1 (**no Handlebars**) | `^8.0.0` (Handlebars) |
| conventional-changelog-angular | 9.4.0 (render functions) | `^8.0.0` |
| conventional-changelog-conventionalcommits | 10.4.0 | devDep only, user-installed |
| conventional-commits-filter | 6.0.1 | `^5.0.0` |

- Writer 9 (2026-06) replaced Handlebars templates/partials with TS render functions (`@conventional-changelog/template`). semantic-release still renders through Handlebars (writer 8).
- Presets ≥ 9/10 add `createLegacyWriterGuard()`: a `mainTemplate` string that makes writer ≤ 8 throw. So `conventionalcommits@10` + release-notes-generator (writer 8) fails loudly.
- Consequence: no single upstream "truth" to port. semoxide should freeze its own behavior (below = parser 7.1.2 + presets 9.4/10.4).

## 1. conventional-commits-parser (7.1.2)

Purpose: commit message → `{type, scope, subject, merge, header, body, footer, notes[], references[], mentions[], revert}`.

### Default options (`src/options.ts`)

| Option | Default |
|---|---|
| `headerPattern` | `^(\w*)(?:\(([\w$@.\-*/ ]*)\))?: (.*)$` |
| `headerCorrespondence` | `[type, scope, subject]` (named groups win if present) |
| `breakingHeaderPattern` | unset (presets set it) |
| `mergePattern` / `mergeCorrespondence` | unset |
| `revertPattern` | `^Revert\s"([\s\S]*)"\s*This reverts commit (\w*)\.?` |
| `revertCorrespondence` | `[header, hash]` |
| `fieldPattern` | `^-(?=.*\w)(.*?)-$` (git-log `-hash-` meta fields) |
| `noteKeywords` | `BREAKING CHANGE`, `BREAKING-CHANGE` (string or RegExp) |
| `notesPattern` | `(keywords) => RegExp` override |
| `issuePrefixes` | `#` ; `issuePrefixesCaseSensitive` false |
| `referenceActions` | close(s/d), fix(es/ed), resolve(s/d) |
| `commentChar` | unset; if set, drop lines starting with it and truncate at `<c> ------------------------ >8 ------------------------` |

### Derived regexes (`src/regex.ts`), `K` = escaped keywords joined by `|`

| Name | Regex | Flags |
|---|---|---|
| notes | `^(?:\*\s+)?(K):\s*(.*)` | i |
| referenceParts | `(?:.*?)??\s*([\w-\.\/]*?)??(PREFIXES)([\w-]+)(?=\s\|$\|[,;.)\]])` | gi (g if case-sensitive) |
| references | `(ACTIONS)(?:\s+(.*?))(?=(?:ACTIONS)\|$)` ; fallback `()(.+)` | gi |
| footerToken | `^(?:BREAKING CHANGE\|[\w-]+)(?::\s+\|\s+(?:PREFIXES)).+` | i |
| mentions | `@([\w-]+)` | g |
| url | `\b(?:https?):\/\/(?:www\.)?([-a-zA-Z0-9@:%_+.~#?&//=])+\b` | — |
| no-match | `(?!.*)` (when option is unset) | — |

### Algorithm (`CommitParser.parse`)
1. Reject blank input. Trim leading/trailing newlines, split `\r?\n`, drop `gpg:` lines (+ comment/scissors filtering).
2. Line 0 vs `mergePattern` → `merge`, consume, skip empty lines.
3. Header = next line. Try `breakingHeaderPattern`, then `headerPattern`; assign correspondence. References parsed from header.
4. Loop over remaining lines: `parseMeta` (fieldPattern blocks) → `parseNotes` → `parseBodyAndFooter`.
   - Note line (notes regex): push `{title, text}`, append to footer; following lines append to note text until a meta/note line or a **footer token** line (token line goes to footer, ends note).
   - Otherwise: body until first footer-token line, then everything is footer. Every line is scanned for references.
5. If no notes and `breakingHeaderPattern` matches header → note `BREAKING CHANGE: <group 3>`.
6. Mentions and revert scanned over the **whole raw input**.
7. Cleanup: trim newlines of body/footer/notes; dedupe references by `lower(action + raw)`.
- Reference parse: per action-sentence, skip if it contains a URL; `repository` split on first `/` into `owner/repository`.

Rust: `regex` crate has **no lookaround** (`referenceParts`, `references`, `fieldPattern`, no-match all use it) → `fancy-regex` or a hand-written tokenizer. Prior art: `git-conventional` (used by git-cliff), cocogitto; crate assessment in [CC spec §5](../specs/CONVENTIONAL-COMMITS-SPEC.md#5-implementation-notes-for-semoxide). Wrap `git-conventional` first ([ADR 0006](../decisions/0006-cc-parser.md)); port compat tests from upstream `*.spec.ts`.

Top issues:
- [#415](https://github.com/conventional-changelog/conventional-changelog/issues/415) (10) `#` in body → false `closes` reference
- [#925](https://github.com/conventional-changelog/conventional-changelog/issues/925) (9) reference matching confused without reference part
- [#828](https://github.com/conventional-changelog/conventional-changelog/issues/828) (1) types not case-sensitive
- [#827](https://github.com/conventional-changelog/conventional-changelog/issues/827) header pattern

## 2. conventional-commits-filter (6.0.1)

`RevertedCommitsFilter` (input newest-first): holds revert commits; a later commit whose fields equal **every** key of a held `revert` object (`header` and `hash`, trimmed, exact) cancels both; other commits are buffered while reverts are pending. Short hashes in revert messages never equal a full `hash` → pair not filtered. Used by analyzer and writer (`ignoreReverted: true`). Rust: ~50 LOC; match hash by prefix.

## 3. conventional-changelog-writer (9.2.1)

Purpose: commits stream → changelog text blocks.

| Option | Default | Note |
|---|---|---|
| `transform(commit, ctx, opts)` | hash→7 chars, header→100 chars, `committerDate` → `YYYY-MM-DD` | commit is a read-only Proxy; returns patch, falsy = drop; result keeps `raw` |
| `groupBy` | `type` | groups keyed by `commit[groupBy] \|\| ''`, insertion order |
| `commitGroupsSort` / `commitsSort` | – / `header` | string, string[] (concatenated keys), or comparator; `localeCompare` |
| `noteGroupsSort` / `notesSort` | `title` / `text` | |
| `generateOn` | `semver.valid(commit.version)` | string = field exists; splits stream into release blocks |
| `reverse`, `doFlush`, `skip`, `ignoreReverted` | false, true, –, true | |
| `finalizeContext` | identity | last hook before render |
| `template`, `headerPartial`, `preamblePartial`, `commitPartial`, `footerPartial` | render functions | v8: Handlebars strings |

Context defaults: `commit: 'commits'`, `issue: 'issues'`, `date: today`, `linkReferences = true` when `repository\|repoUrl` set. `isPatch` = semver patch ≠ 0. Note groups keep first-seen order unless sorted.

Rust: render functions map naturally to Rust code; user-template engine: [distribution-config](distribution-config.md#templating). `localeCompare` ≠ byte order: pick and document.

Top issues: [#1337](https://github.com/conventional-changelog/conventional-changelog/issues/1337) (3) ignores most recent tag; [#1298](https://github.com/conventional-changelog/conventional-changelog/issues/1298) commas between multiple issue ids.

## 4. Presets

| | angular 9.4.0 | conventionalcommits 10.4.0 |
|---|---|---|
| headerPattern | `^(\w*)(?:\((.*)\))?: (.*)$` (**no `!`**) | `^(\w*)(?:\((.*)\))?!?: (.*)$` |
| breakingHeaderPattern | – | `^(\w*)(?:\((.*)\))?!: (.*)$` |
| noteKeywords | `BREAKING CHANGE` | `BREAKING CHANGE`, `BREAKING-CHANGE` |
| revertPattern | `^(?:Revert\|revert:)\s"?([\s\S]+?)"?\s*This reverts commit (\w{7,40})\b` i | same but `(\w*)\.` |
| issuePrefixes | `#` | config, default `#` |
| commits | `merges: false`, `ignore: ignoreCommits` | same |
| config | `ignoreCommits` | `types`, `scope`, `scopeOnly`, `preMajor`, `issuePrefixes`, `format{Issue,Commit,Compare,User}Url`, `formatNote{Title,Icon}` |

Sections. angular: hard-coded `feat`→Features, `fix`→Bug Fixes, `perf`→Performance Improvements, `revert`→Reverts always shown; docs/style/refactor/test/build/ci shown **only if commit has notes**; else dropped. Groups sorted by title.
conventionalcommits `DEFAULT_COMMIT_TYPES` (`effect`: `bump` | `changelog` | `hidden`, default `bump`; replaced `hidden: true` in 10.0):

| type | section | effect |
|---|---|---|
| feat, feature | Features | bump |
| fix | Bug Fixes | bump |
| perf | Performance Improvements | bump |
| revert | Reverts | bump |
| docs / style / chore / refactor / test / build / ci | Documentation / Styles / Miscellaneous Chores / Code Refactoring / Tests / Build System / Continuous Integration | hidden |

- Groups sorted in `types` order. `Release-As:` footer/body forces inclusion. `scope` filter: `scopeOnly` drops unscoped commits.
- `!` breaking: `getNotes()` synthesizes a `BREAKING CHANGE` note from subject unless one exists (parser only adds it when there are no notes at all).
- Notes: `noteTitle()` folds `BREAKING-CHANGE` into `BREAKING CHANGES`, other keywords upper-cased into own groups. References in subject/notes linkified; duplicates removed from trailing `closes`.
- Templates: `## [version](compare) "title" (date)` (`###`/small for patch in angular), `### Section` + `* **scope:** subject ([hash](url)), closes #1`, then note groups.

whatBump (`conventional-recommended-bump`, **not** used by semantic-release, whose commit-analyzer has its own `releaseRules`, see [commit-analyzer](commit-analyzer.md)):
- angular: breaking note → major(0); `feat` → minor(1); else patch(2) **always** (never null).
- conventionalcommits: only `bump`-effect types count; none → `null`; `preMajor` shifts level +1.

Rust: presets become built-in config structs (types table + patterns). No dynamic preset loading.

## 5. env-ci

Purpose: detect CI + normalize env. Result: `{isCi, name, service, commit, tag, build, buildUrl, job, jobUrl, branch, pr, isPr, prBranch, slug, root}`. First `detect()` match wins (object key order); fallback `{isCi: Boolean(CI), commit: git rev-parse HEAD, branch}`.

| Service | Detect var | Service | Detect var |
|---|---|---|---|
| appveyor | `APPVEYOR` | jenkins | `JENKINS_URL` |
| azurePipelines | `BUILD_BUILDURI` | netlify | `NETLIFY` |
| bamboo | `bamboo_agentId` | puppet | `DISTELLI_APPNAME` |
| bitbucket | `BITBUCKET_BUILD_NUMBER` | sail | `SAILCI` |
| bitrise | `BITRISE_IO` | screwdriver | `SCREWDRIVER` |
| buddy | `BUDDY_WORKSPACE_ID` | scrutinizer | `SCRUTINIZER` |
| buildkite | `BUILDKITE` | semaphore | `SEMAPHORE` |
| circleci | `CIRCLECI` | shippable | `SHIPPABLE` |
| cirrus | `CIRRUS_CI` | teamcity | `TEAMCITY_VERSION` (+ reads Java `.properties` files) |
| cloudflarePages | `CF_PAGES` | travis | `TRAVIS` |
| codebuild | `CODEBUILD_BUILD_ID` | vela | `VELA` |
| codefresh | `CF_BUILD_ID` | vercel | `VERCEL` / `NOW_GITHUB_DEPLOYMENT` |
| codeship | `CI_NAME=codeship` | wercker | `WERCKER_MAIN_PIPELINE_STARTED` |
| drone | `DRONE` | woodpecker | `CI=woodpecker` |
| github | `GITHUB_ACTIONS` | jetbrainsSpace | `JB_SPACE_EXECUTION_NUMBER` |
| gitlab | `GITLAB_CI` | | |

Key mappings:
- GitHub: `commit=GITHUB_SHA`, `branch=parseBranch(GITHUB_REF)` (strips `refs/heads/`), `isPr` for `pull_request(_target)`; on PR reads `GITHUB_EVENT_PATH` JSON → `pr=number`, `branch=base.ref`. `root=GITHUB_WORKSPACE`.
- GitLab: `branch = MR ? CI_MERGE_REQUEST_TARGET_BRANCH_NAME : CI_COMMIT_REF_NAME`, `pr=CI_MERGE_REQUEST_ID`, `tag=CI_COMMIT_TAG`.
- Jenkins: chains `ghprb*` / `gitlab*` / `CHANGE_ID` / `GIT_LOCAL_BRANCH|GIT_BRANCH|BRANCH_NAME`.
- Git fallback: `rev-parse --abbrev-ref HEAD`; if detached, first `origin/*` in `git show -s --pretty=%d`.
- semantic-release uses: `isCi` (dry-run/ci guard), `branch`, `isPr` (skip PR builds).

Rust: `ci_info` crate exists (vendor + PR + branch) but semantics differ; a ~300 LOC table-driven port (data in a static table) is safer and testable. Fallback via git library, not CLI.

Top issues: [#96](https://github.com/semantic-release/env-ci/issues/96) (2) GitHub Actions wrong behavior; [#363](https://github.com/semantic-release/env-ci/issues/363) branch detection unreliable; [#249](https://github.com/semantic-release/env-ci/issues/249) no slug on Jenkins.

## 6. @semantic-release/git

Purpose: `prepare` step commits release assets and pushes. README itself recommends against committing during release.

| Option | Default |
|---|---|
| `assets` | `["CHANGELOG.md","package.json","package-lock.json","npm-shrinkwrap.json"]`; `false` disables; items: glob, glob[] (one group), or `{path}` |
| `message` | `chore(release): ${nextRelease.version} [skip ci]\n\n${nextRelease.notes}` (lodash template, ctx `{branch: branch.name, lastRelease, nextRelease}`) |

Behavior:
- `verifyConditions` borrows `assets`/`message` from the `prepare` plugin entry, validates (`EINVALIDASSETS`, `EINVALIDMESSAGE`). Module-level `verified` flag.
- Only **modified or untracked** files (`ls-files -m -o`) matching assets are committed (micromatch, `dot: true`, dir-glob expansion; a lone `!glob` group is ignored). No match → no commit, no push.
- Author/committer: core sets `GIT_AUTHOR_*`/`GIT_COMMITTER_*` = semantic-release-bot unless user env overrides.
- Push target is `options.repositoryUrl` (core-injected token URL). Core re-reads HEAD after prepare, so the tag lands on the release commit.

| Step | Command | Side effect |
|---|---|---|
| list candidates | `git ls-files -m -o` | none (maxBuffer issue on large untracked trees) |
| stage | `git add --force --ignore-errors <files…>` | ignores `.gitignore`; errors swallowed (`reject: false`) |
| commit | `git commit -F -` (message on stdin) | runs commit hooks; signing per user config |
| push | `git push --tags <repositoryUrl> HEAD:<branch.name>` | pushes **all** local tags too |

Rust: gix status (modified+untracked) + `globset` + index add + commit; push via gix if PoC passes, else git CLI fallback. Make it opt-in, no `--tags` blanket push.

Top issues: [#196](https://github.com/semantic-release/git/issues/196) (13) not using GH_TOKEN; [#347](https://github.com/semantic-release/git/issues/347) (8) ignoreFile option; [#280](https://github.com/semantic-release/git/issues/280) (7) `assets: false` errors; [#321](https://github.com/semantic-release/git/issues/321) (4) package.json not pushed; [#406](https://github.com/semantic-release/git/issues/406) push to other branch; [#506](https://github.com/semantic-release/git/issues/506) `ls-files` maxBuffer; [#477](https://github.com/semantic-release/git/issues/477) discourage plugin.

## 7. @semantic-release/changelog

Purpose: `prepare` prepends `nextRelease.notes` to a file.

| Option | Default |
|---|---|
| `changelogFile` | `CHANGELOG.md` (non-empty string, **not** templated) |
| `changelogTitle` | unset |

Algorithm: if notes non-empty → mkdir -p, create file if missing; `cur = trim(file)`; strip leading `changelogTitle` if `cur.startsWith(title)`; write `[title\n\n] + notes.trim() + "\n" + (cur ? "\n"+cur+"\n" : "")`. No dedup: a retried release prepends twice. Errors `EINVALIDCHANGELOGFILE`, `EINVALIDCHANGELOGTITLE`.

Rust: trivial; add idempotency (skip if version header already present).

Top issues: [#89](https://github.com/semantic-release/changelog/issues/89) (12) template the file name; [#142](https://github.com/semantic-release/changelog/issues/142) (11) full commit message; [#92](https://github.com/semantic-release/changelog/issues/92) (7) title expansion; [#150](https://github.com/semantic-release/changelog/issues/150) (5) backfill existing repo.

## 8. @semantic-release/exec

Purpose: shell command per lifecycle step.

| Option | Meaning |
|---|---|
| `verifyConditionsCmd`, `analyzeCommitsCmd`, `verifyReleaseCmd`, `generateNotesCmd`, `prepareCmd`, `publishCmd`, `addChannelCmd`, `successCmd`, `failCmd` | step command (lodash template, ctx `{config, ...context}` minus `cwd/env/stdout/stderr/logger`) |
| `cmd` | fallback for every step |
| `shell` | `true` (default) or shell path |
| `execCwd` | relative to `cwd` |

| Step | stdout contract |
|---|---|
| verifyConditions / verifyRelease | non-zero → `SemanticReleaseError(EVERIFYCONDITIONS/EVERIFYRELEASE)` |
| analyzeCommits | release type or empty (= no release) |
| generateNotes | notes text |
| publish / addChannel | optional JSON release info; invalid JSON ignored (debug only); returns `false` if no cmd |
| others | logging only |

- stdout/stderr piped live to context streams; result stdout `.trim()`.
- Bug: `error.stdout.trim.length > 0` checks the function's arity (0) → stdout never used as error message; always `name: message`.
- Commands are interpolated into a shell string → injection risk from notes/branch names.

Rust: `std::process::Command`, argv form preferred; template via same engine as §10. Support command arrays.

Top issues: [#345](https://github.com/semantic-release/exec/issues/345) (8) arrays of commands; [#110](https://github.com/semantic-release/exec/issues/110) (6) several commands; [#160](https://github.com/semantic-release/exec/issues/160) (3) env vars; [#335](https://github.com/semantic-release/exec/issues/335) (2) failCmd not invoked on publishCmd failure.

## 9. @semantic-release/error

`class SemanticReleaseError extends Error { name="SemanticReleaseError", code, details, semanticRelease: true }`. Core treats `semanticRelease: true` errors as user-facing (printed with `details` markdown, fed to `fail` plugins) vs. unexpected errors. Plugins throw `AggregateError` of these.

Rust: `thiserror` enum with `code()`, `message`, `details` + `Vec` aggregation; `is_user_facing()` instead of the marker flag. Issue: [#195](https://github.com/semantic-release/error/issues/195) add `cause` (→ `std::error::Error::source`).

## 10. lodash `template` usage

| Where | Call | Variables |
|---|---|---|
| core `tagFormat` → tag | `template(fmt, {evaluate:false, escape:false})({version})` | `version` |
| core tag regex | render with `version=" "`, escape, replace `" "` → `(.+)` | |
| core verify | render `0.0.0` → `git check-ref-format refs/tags/<t>`; exactly one `" "` in render with `version=" "` | |
| core branches | `template(value, same opts)({name})` on every string branch prop (`channel`, `range`, `prerelease`) | `name` |
| git `message`, exec `*Cmd` | `template(str)` **default opts** | see §6, §8 |

- Default opts enable `<%= %>` and ES `${ }` interpolation, `<% %>` evaluate (arbitrary JS), `<%- %>` HTML-escape. Expressions are JS (`new Function`), e.g. `${nextRelease.version.split('.')[0]}` is used in the wild.
- Rust: cannot run JS; engine pick in [distribution-config](distribution-config.md#templating); document the break. Build the tag matcher from parsed template segments, not regex-from-template tricks.

## Ticket candidates

**conventional-commits-parser**
- Commit parser core — conventional-changelog fields (notes, merge, revert, meta) per §1; spec grammar tickets: [CC spec](../specs/CONVENTIONAL-COMMITS-SPEC.md#ticket-candidates).
- Reference & mention extraction — actions, prefixes, owner/repo, URL skip, dedupe; no lookaround.
- Parser compat test corpus — port upstream `CommitParser.spec.ts` / `regex.spec.ts` cases as fixtures.
- Fix #415/#925 class bugs by design — references only from footer/trailers by default.

**conventional-commits-filter**
- Revert pair filter — port `RevertedCommitsFilter`.

**writer + presets**
- Changelog model — group/sort/notes pipeline (writer §3) with stable, documented ordering.
- Built-in renderer — conventionalcommits-style markdown as Rust code.
- User templates — minijinja-based template override with typed context.
- Preset config — types table with `effect`, scope filter, `!` breaking synthesis, `Release-As`.

**env-ci**
- CI detection table — 31 services, static data + per-service mapping.
- GitHub/GitLab PR event handling — event JSON, MR target branch.
- Git fallback branch/commit — via git library, detached HEAD handling.

**git plugin**
- Release commit step — status (modified+untracked), glob filter, add, commit with bot identity.
- Push step — push branch + only the new tag; auth via token URL; PoC in gix.

**changelog plugin**
- CHANGELOG prepend — title handling, idempotent on retry, templated file path (#89).

**exec plugin**
- Exec step runner — per-step commands, arrays (#345), argv mode, cwd, live output, stdout contracts.
- Exec error reporting — use stdout/stderr in error (fix upstream arity bug), call fail on publish failure (#335).

**error**
- Error model — user-facing vs internal, codes, details, aggregation, `source()` chain.

**templating**
- `tagFormat` parse/render/validate from parsed segments, no regex-from-template tricks (§10). Engine choice: [distribution-config](distribution-config.md#config).
