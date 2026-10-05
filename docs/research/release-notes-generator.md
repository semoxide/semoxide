# @semantic-release/release-notes-generator — research

Source: `semantic-release/release-notes-generator` master `9b15394` (2026-09-21), diffed against `beta`. ~190 LOC: `index.js`, `lib/hosts-config.js`, `lib/load-changelog-config.js`, `wrappers/conventional-changelog-writer.js`.

## 1. Features

| Feature | File |
|---|---|
| Single step `generateNotes(pluginConfig, context) -> Promise<string>` | `index.js:29` |
| Preset loading by short name (`conventional-changelog-<preset>`), tried from the plugin dir first, then `cwd` | `lib/load-changelog-config.js:24-28` |
| Custom preset package via `config` | `lib/load-changelog-config.js:29-30` |
| Built-in default preset: angular (on `feat/default-preset` branch it becomes conventionalcommits) | `lib/load-changelog-config.js:32` |
| `presetConfig` is passed to the preset factory | `lib/load-changelog-config.js:28` |
| Shallow override of preset parser/writer opts with `parserOpts`/`writerOpts` | `lib/load-changelog-config.js:35-39` |
| Hand-rolled repo URL parsing (scp-like, ssh, git+http(s), http(s)); no git-url-parse or hosted-git-info | `index.js:31-40` |
| Per-host link keywords and reference parsing (github, bitbucket, gitlab, default), matched on exact hostname | `lib/hosts-config.js` |
| Skips empty-message commits | `index.js:48-51` |
| Preset `commits.ignore` regex filter | `index.js:53-56` |
| Removes reverted commits and their reverts (`filterRevertedCommitsSync`) | `index.js:45` |
| Compare link between previous and current tag (falls back to gitHead) | `index.js:65-76` |
| `package.json` (read upward from cwd) exposed to templates as `packageData` | `index.js:79` |
| Rendering delegated to `conventional-changelog-writer.writeChangelogString` (wrapper exists only for test mocking) | `wrappers/conventional-changelog-writer.js` |

## 2. Config options

| Option | Default | Notes |
|---|---|---|
| `preset` | angular (no preset/config set) | lowercased; wins over `config` |
| `config` | — | npm package name; factory called with no args |
| `presetConfig` | `undefined` | conventionalcommits needs it (`types`, `issuePrefixes`, `*UrlFormat`, `ignoreCommits`) |
| `parserOpts` | — | shallow spread over `preset.parser` and over host `referenceActions`/`issuePrefixes` |
| `writerOpts` | — | shallow spread over `preset.writer` |
| `host` | derived from `repositoryUrl` | full origin string, e.g. `http://my-host:90` |
| `linkCompare` | `currentTag && previousTag` (truthy) | README says `true`. In practice it is false when there is no previous release |
| `linkReferences` | `undefined` → writer default | |
| `commit` | host table: `commit` (`commits` on bitbucket.org) | URL path keyword |
| `issue` | host table: `issues` (`issue` on bitbucket.org) | URL path keyword |

README claims that without preset/config only `parserOpts`/`writerOpts` are used; the code always loads angular (same as [commit-analyzer](commit-analyzer.md#2-config-options)).

## 3. Algorithm and data flow

```
loadChangelogConfig -> {commitOpts, parserOpts, writerOpts}
repositoryUrl -> {protocol, hostname, port, owner, repository}
hostname -> HOSTS_CONFIG entry (exact match) | default
commits -> filter(empty, ignore) -> map(CommitParser.parse merged onto raw commit) -> filterRevertedCommitsSync
context = lodash.merge(derived, pluginOverrides)   // undefined overrides are skipped
return writeChangelogString(parsedCommits, context, writerOpts)
```

**URL parsing** (`index.js:31-40`):
1. Strip a trailing `.git` (case-insensitive).
2. If there is no `://` and the URL matches `[auth@]host:path` (scp-like), rewrite it to `ssh://[auth@]host/path`.
3. Parse with WHATWG `new URL`.
4. Drop the port when the protocol contains `ssh`.
5. Use `http` when the protocol matches `/http[^s]/` (`http:`, `git+http:`). Use `https` for everything else, including ssh, git, file and codecommit.
6. Path regex `^/(owner)?/?(repository)?$`: the first segment is the owner and the rest is the repository. For example, `grp/sub/r` gives owner `grp` and repo `sub/r`.
7. `host = url.format({protocol, hostname, port})`. Credentials are dropped.

Probed results:

| Input | host / owner / repository |
|---|---|
| `git@gitlab.com:grp/sub/r.git` | `https://gitlab.com` / `grp` / `sub/r` |
| `git+ssh://git@h.com:2222/o/r.git` | `https://h.com` / `o` / `r` (ssh port dropped, [#151](https://github.com/semantic-release/release-notes-generator/issues/151)) |
| `http://h.com:90/o/r` | `http://h.com:90` / `o` / `r` |
| `http://bb.corp/scm/PROJ/repo.git` | `http://bb.corp` / **`scm`** / **`PROJ/repo`** (bug [#862](https://github.com/semantic-release/release-notes-generator/issues/862), [#990](https://github.com/semantic-release/release-notes-generator/issues/990)) |
| `codecommit::eu-central-1://profile@repo` | `https:` / – / – (references are not linked) |
| `file:///tmp/repo` | `https:` / `tmp` / `repo` (garbage) |

**Host table** (`lib/hosts-config.js`):

| host | issue | commit | referenceActions | issuePrefixes |
|---|---|---|---|---|
| github.com | issues | commit | close(s/d), fix(es/ed), resolve(s/d) | `#`, `gh-` |
| bitbucket.org | issue | commits | + closing/fixing/resolving | `#` |
| gitlab.com | issues | commit | close*, fix* (no resolve) | `#` |
| default (self-hosted, anything else) | issues | commit | all 12 | `#`, `gh-` |

Precedence for parser options, lowest first: host table, then `preset.parser`, then `parserOpts` (`new CommitParser({referenceActions, issuePrefixes, ...parserOpts})`). Presets therefore usually override host prefixes.

**Writer context** (verified by the tests at `test/integration.test.js:39-108`):

| Field | Value |
|---|---|
| `version` | `nextRelease.version` |
| `host`, `owner`, `repository` | from URL parsing; `host` can be overridden by the plugin option |
| `previousTag` | `lastRelease.gitTag \|\| lastRelease.gitHead` |
| `currentTag` | `nextRelease.gitTag \|\| nextRelease.gitHead` |
| `linkCompare` | option, else `previousTag` (truthy) |
| `issue`, `commit` | option, else host table |
| `linkReferences` | option, else undefined |
| `packageData` | raw `package.json`, not normalized |

`repoUrl`, `date`, `title` and `isPatch` are not set here. The writer fills them from its defaults and the preset's `finalizeContext`.

**Writer interface used.** Only `writeChangelogString(commits, context, options) -> Promise<string>`, options = preset + `writerOpts` (writer 8: Handlebars `mainTemplate` + partials; option semantics in [dependencies §3](dependencies.md#3-conventional-changelog-writer-921)). `transform` must return a new object since writer v8 ([#658](https://github.com/semantic-release/release-notes-generator/issues/658), [#685](https://github.com/semantic-release/release-notes-generator/issues/685)). Templates build URLs from `@root.host/owner/repository/{commit}/{hash}` or the conventionalcommits `*UrlFormat` options. Output shape: [dependencies §4](dependencies.md#4-presets).

## 4. Side effects

- File system reads: `readPackageUp({cwd, normalize:false})`, plus dynamic `import()` of preset modules from the plugin dir and from `cwd`.
- `debug` logging (namespace `semantic-release:release-notes-generator`). `debug("host", changelogContext.hostname)` always logs `undefined` (`index.js:85`).
- No network access, no writes, no git calls. Commits arrive pre-fetched in `context.commits`.

## 5. Rust port notes

| Topic | Notes |
|---|---|
| Templates | **handlebars-rs**: only option that reuses preset `.hbs` files, partials, `@root`, `~` whitespace control; JS helpers must be reimplemented. **minijinja**: best errors, small; templates need porting. **tera**: heavier, no advantage. minijinja unless `.hbs` compat becomes a goal (it isn't); see [distribution-config](distribution-config.md#templating). |
| URL parsing | `url` crate (WHATWG, same semantics as `new URL`) plus a scp-like regex, or `git-url-parse` (crate) / `gix-url` (handles scp, ssh, file). Prefer `gix-url` and an explicit `RepoCoordinates {web_base, owner_path, name}`. |
| Host forges | Make the table data-driven and matchable by hostname or by an explicit `forge = gitlab` setting, so self-hosted instances get the right keywords. Model owner as a path (for GitLab subgroups) and support URL prefixes (GitLab `/-/`, Bitbucket Server `/scm/`, `/projects/X/repos/Y/commits`). |
| JS-specific | `writerOpts.transform`, `finalizeContext` and sort functions are user JS functions. Replace them with declarative config (type→section map, hidden, sort keys) plus a Rust trait for plugins. Dynamic preset import from `node_modules` has no Rust equivalent; use built-in presets plus plugin crates or WASM. `lodash.merge` skips undefined values; use explicit `Option` layering. |
| Revert filter | [dependencies §2](dependencies.md#2-conventional-commits-filter-601) |
| Ignore quirk | `commitOpts.ignore` applies only when `!commitOpts.merges`, which looks like a bug coupling. Do not port it. |

## 6. Tests

| Test file | Port 1:1? |
|---|---|
| `test/integration.test.js`: about 30 cases. Each feeds `{hash, message}` commits with a `repositoryUrl` and asserts regex substrings (compare link, section headers, commit/issue links) | **Yes**, as golden/snapshot tests (insta). The URL matrix (http+port, scp, scp without user, git+http, `.git`, git+https, git+ssh+port, bitbucket, gitlab, codecommit, `host`/`commit`/`issue`/`linkCompare`/`linkReferences` overrides), revert exclusion, empty messages and malformed commits all port directly. |
| Context-shape tests (`writerDouble` via testdouble) | Yes, as unit tests on the context builder. |
| `test/load-changelog-config.test.js`: preset/config loading for 7 presets, override merging, missing module → `MODULE_NOT_FOUND`, `transform.toString()` comparison | Not portable (npm resolution, JS function identity). Keep only the merge-precedence tests. |

The tests are substring regexes, not full golden files. Full-output snapshots would be stricter.

## 7. Issue history

117 issues in total; 2 are closed as not planned.

**Recurring problems**
- Ecosystem major-version skew between preset, writer, parser and semantic-release breaks generateNotes or produces empty notes. This is the most-reacted class of issues: [#633](https://github.com/semantic-release/release-notes-generator/issues/633) (53 reactions), [#992](https://github.com/semantic-release/release-notes-generator/issues/992) (43, open), [#1027](https://github.com/semantic-release/release-notes-generator/issues/1027), [#660](https://github.com/semantic-release/release-notes-generator/issues/660), [#447](https://github.com/semantic-release/release-notes-generator/issues/447), [#565](https://github.com/semantic-release/release-notes-generator/issues/565), [#787](https://github.com/semantic-release/release-notes-generator/issues/787).
- Duplicate package copies cause "Cannot modify immutable object" and `Date.prototype.toString` errors: [#685](https://github.com/semantic-release/release-notes-generator/issues/685), [#675](https://github.com/semantic-release/release-notes-generator/issues/675), [#657](https://github.com/semantic-release/release-notes-generator/issues/657), [#746](https://github.com/semantic-release/release-notes-generator/issues/746).
- Wrong links on self-hosted and non-GitHub forges: GitLab `/-/` [#449](https://github.com/semantic-release/release-notes-generator/issues/449), self-hosted GitLab gets no issue links [#584](https://github.com/semantic-release/release-notes-generator/issues/584), Bitbucket Server [#491](https://github.com/semantic-release/release-notes-generator/issues/491), [#862](https://github.com/semantic-release/release-notes-generator/issues/862), [#990](https://github.com/semantic-release/release-notes-generator/issues/990), Bitbucket compare [#131](https://github.com/semantic-release/release-notes-generator/issues/131), ssh port leaks into https [#151](https://github.com/semantic-release/release-notes-generator/issues/151), over-trimmed `.git` [#111](https://github.com/semantic-release/release-notes-generator/issues/111), CodeCommit crash [#182](https://github.com/semantic-release/release-notes-generator/issues/182).
- False "closes" references: plain issue mentions [#201](https://github.com/semantic-release/release-notes-generator/issues/201), URLs in the body [#788](https://github.com/semantic-release/release-notes-generator/issues/788), `diffhunk://#` [#1025](https://github.com/semantic-release/release-notes-generator/issues/1025), cross-project refs [#751](https://github.com/semantic-release/release-notes-generator/issues/751).
- Customization requires Handlebars/JS knowledge (body in notes, extra sections, sort order, partials that silently drop preset options): [#332](https://github.com/semantic-release/release-notes-generator/issues/332), [#153](https://github.com/semantic-release/release-notes-generator/issues/153), [#431](https://github.com/semantic-release/release-notes-generator/issues/431), [#216](https://github.com/semantic-release/release-notes-generator/issues/216), [#399](https://github.com/semantic-release/release-notes-generator/issues/399), [#403](https://github.com/semantic-release/release-notes-generator/issues/403).
- Silent abort on large initial releases: [#459](https://github.com/semantic-release/release-notes-generator/issues/459).
- Host parser options were always overwritten (fixed): [#62](https://github.com/semantic-release/release-notes-generator/issues/62).

**Rejected or declined**
- Filter commits by author: [#906](https://github.com/semantic-release/release-notes-generator/issues/906). Not planned; maintainers want users to write a wrapper plugin.
- Cap notes at the 125k GitHub limit: [#687](https://github.com/semantic-release/release-notes-generator/issues/687). Redirected to the github plugin.
- Bitbucket `/scm/` parsing: [#990](https://github.com/semantic-release/release-notes-generator/issues/990). Deferred upstream to conventional-changelog, although the parsing actually lives in this plugin.

**Open feature requests**
- PR links, authors and contributors: [#553](https://github.com/semantic-release/release-notes-generator/issues/553), [#260](https://github.com/semantic-release/release-notes-generator/issues/260).
- Custom header/footer text: [#263](https://github.com/semantic-release/release-notes-generator/issues/263).
- `[skip release]` commits still appear in notes: [#531](https://github.com/semantic-release/release-notes-generator/issues/531).
- Heading uses the version, not the `tagFormat` tag: [#179](https://github.com/semantic-release/release-notes-generator/issues/179).
- Bundle angular and conventionalcommits presets: [#1028](https://github.com/semantic-release/release-notes-generator/issues/1028). In progress on `beta`, which pins writer@9 and parser@7 and bundles conventionalcommits@10; other presets are dropped upstream.

## Ticket candidates

- **Repo coordinates parser**: parse scp/ssh/http(s)/git+ and file URLs into `{web_base, owner_path, name}`; drop credentials; include the probe table above as tests.
- **Forge profiles**: data-driven link templates for commit, issue, compare and PR (github, gitlab incl. `/-/` and subgroups, bitbucket cloud and server, gitea), selectable by hostname or by config.
- **Notes context builder**: version, tags (gitHead fallback), compare link only when a previous tag exists, package metadata.
- **Commit filtering pipeline**: empty messages, ignore regex, revert pairs, `[skip release]`, author/bot filter.
- **Reference extraction rules**: action keywords and prefixes per forge; mentions only count as "closes" when an action keyword precedes them; ignore URLs and `diffhunk://`.
- **Template engine decision**: minijinja vs handlebars-rs spike; built-in conventionalcommits template; header/footer injection.
- **Declarative section config**: type→section, hidden, sort keys, body inclusion; replaces JS `transform`.
- **Golden-output test suite**: port the integration matrix as insta snapshots.
- **Release-notes size guard**: see [github](github.md#ticket-candidates) (125k body guard).
