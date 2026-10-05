# License research

> Not legal advice (not a lawyer). Practical reading of the license texts, verified 2026-10-05 via GitHub API / npm registry / live sites.

## Upstream licenses (verified)

| Source | License | Copyright line | Where verified |
|---|---|---|---|
| semantic-release/semantic-release | MIT | (c) 2017 Contributors | repo `LICENSE`, npm `license` |
| @semantic-release/commit-analyzer | MIT | (c) 2017 Pierre-Denis Vanduynslager | repo `LICENSE`, npm |
| @semantic-release/release-notes-generator | MIT | (c) 2017 Pierre-Denis Vanduynslager | repo `LICENSE`, npm |
| @semantic-release/npm | MIT | (c) 2017 Contributors | repo `LICENSE`, npm |
| @semantic-release/github | MIT | (c) 2017 Contributors | repo `LICENSE`, npm |
| conventional-commits-parser | MIT | (c) conventional-changelog team | `packages/*/LICENSE.md`, npm |
| conventional-changelog-writer (incl. default `.hbs` templates) | MIT | (c) conventional-changelog team | `packages/*/LICENSE.md`, npm |
| conventional-changelog-angular (preset + templates) | ISC | (c) conventional-changelog team | `packages/*/LICENSE.md`, npm |
| conventional-changelog-conventionalcommits (preset + templates) | ISC | (c) conventional-changelog team | `packages/*/LICENSE.md`, npm |
| conventional-changelog monorepo root | ISC | (c) conventional-changelog team | root `LICENSE.md` |
| SemVer 2.0.0 spec text (semver/semver `semver.md`, semver.org) | CC BY 3.0 | Tom Preston-Werner | `semver.md` §License, site footer (no LICENSE file; repo tooling `package.json` = ISC) |
| Conventional Commits 1.0.0 spec text | CC BY 3.0 | — | site footer (`config.yaml`) |
| conventionalcommits.org repo (site code) | MIT | (c) 2018 Conventional Changelog | repo `LICENSE` |

MIT and ISC are functionally equivalent: permissive, sole condition = keep copyright + permission notice in copies/substantial portions. CC BY 3.0: free to share/adapt incl. commercially; must attribute, link the license, indicate changes.

## Answers

1. **From-scratch Rust reimplementation: OK.** Ideas, behavior, algorithms, config option names, step names (`verifyConditions`, `analyzeCommits`, …) are not copyrightable expression. No obligation if no code/text is copied.
2. **Obligations:**
   - (a) **Tests ported 1:1** (same inputs/expected outputs, translated to Rust): likely a derivative of MIT/ISC test files → keep upstream copyright + license notice (file header + `THIRD_PARTY_LICENSES`). Re-deriving cases from behavior/spec in own words: no obligation, attribution courteous.
   - (b) **Fixtures / Handlebars templates copied or adapted:** must include MIT (writer) or ISC (angular/conventionalcommits presets) notice alongside them. Trivial data (single commit messages) is arguably not copyrightable, but treat fixture sets as covered.
   - (c) **Spec text quoted/condensed in `docs/specs/*.md`:** CC BY 3.0 adaptation → per file: title, author/source link, license link (CC BY 3.0), "condensed/modified" statement. Do not imply endorsement. Fine to keep our own docs under our license; the adapted portions stay CC BY 3.0.
3. **Naming/trademark:** no registered "semantic-release" mark found (web search; USPTO TESS not checked directly), no trademark policy in the org. Name is still a de-facto brand → don't name the tool/crate `semantic-release*` or use its logo; don't claim compatibility/affiliation. "Inspired by semantic-release" (nominative use) is fine; add "not affiliated with or endorsed by".
4. **Recommended license: `MIT OR Apache-2.0` (dual).** Rust ecosystem norm (rustc, most crates) → frictionless reuse; Apache-2.0 adds explicit patent grant; MIT side is GPLv2-compatible; both compatible with incoming MIT/ISC/CC BY material. Decide before going public and before taking outside contributions.
5. **Attribution checklist:**
   - [ ] `LICENSE-MIT` + `LICENSE-APACHE`; `Cargo.toml` `license = "MIT OR Apache-2.0"`.
   - [ ] `THIRD_PARTY_LICENSES.md`: full MIT/ISC texts + copyright lines for each upstream actually copied from (table above).
   - [ ] Header comment in every ported test/fixture/template file: `Ported from <repo>@<commit>/<path>, <license>, (c) <holder>`.
   - [ ] Every `docs/specs/*.md` condensing SemVer / Conventional Commits: source link, author, "CC BY 3.0 (link)", "condensed and modified".
   - [ ] README: "Inspired by semantic-release. Not affiliated with or endorsed by the semantic-release project."
   - [ ] Pin upstream commit SHA for each ported batch (provenance).
   - [ ] If templates are embedded in the binary: ship notices with release artifacts (e.g. `--licenses` output or bundled file).

## Name availability (`semoxide`, 2026-10-05)

| Registry | Result |
|---|---|
| crates.io (`/api/v1/crates/semoxide`) | 404 — free |
| npm (`registry.npmjs.org/semoxide`) | 404 — free |
| GitHub user/org `semoxide` | 404 — free; only repo named so: `sm-steel/semoxide` (ours) |

## Ticket candidates

- Choose license: add `LICENSE-MIT`, `LICENSE-APACHE`, `Cargo.toml` `license` field — adopt `MIT OR Apache-2.0`.
- Add `THIRD_PARTY_LICENSES.md` — MIT/ISC texts + copyright holders for ported upstream material.
- Attribute condensed specs — add CC BY 3.0 notice block to each `docs/specs/*.md`.
- Porting provenance convention — header format + pinned SHA for ported tests/fixtures/templates.
- README disclaimer — "inspired by, not affiliated with semantic-release".
- Reserve name — publish placeholder `semoxide` crate (and optionally GitHub org) before going public.
- Ship notices in binary — bundle third-party notices for embedded templates.
