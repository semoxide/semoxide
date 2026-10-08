# Decisions

Why each non-obvious choice was made. The rules themselves live in the linked docs.

## Scope and compatibility

- **Familiar, not compatible** — the gap among release tools is a single-binary, plugin-based, library-first tool that semantic-release's user base can migrate to; runtime compatibility would bring back every JS-only problem. (→ [REQUIREMENTS](REQUIREMENTS.md), [DIFFERENCES](DIFFERENCES.md))
- **`migrate` instead of reading `.releaserc`** — migration, not feature count, is the differentiator; without it semoxide is "another knope". (→ [CLI](CLI.md))
- **No JS plugin bridge** — it would need a Node runtime and reintroduce npm resolution, user JS functions and stdout-fragile plugins. (→ [REQUIREMENTS](REQUIREMENTS.md))
- **Monorepo in the core, hybrid with plugins** — monorepo is semantic-release's most-wanted issue (#193, 335 reactions; ~1.1M monthly downloads of community add-ons and forks); the plugin-only approach fragmented and runs N times, and orchestration can't live inside a step plugin. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **v1 refuses interdependent units** — releasing them correctly needs dependency order plus manifest updates between publishes, which is what upstream declined over; independent units are the easy, safe part. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Same default bump table and `1.0.0` start as upstream** — users coming from semantic-release get the same versions. (→ [CONFIG](CONFIG.md))
- **`conventionalcommits` as default preset** — it recognises `!`, matches the spec, and is upstream's planned next default. (→ [CONFIG](CONFIG.md))

## Config and templates

- **TOML only** — `serde_yaml` is deprecated and its forks stalled; TOML is native to the Rust ecosystem and has editor schema support. (→ [CONFIG](CONFIG.md))
- **Own layer merge, not figment/config** — figment is stalled since 2024, `config` is weakly typed; the merge is ~200 lines. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **No env→config layer** — env config is invisible and typo-prone, flags are as easy in CI, and upstream has none; env holds only secrets and a fixed var list. (→ [CLI](CLI.md#environment-variables))
- **Shared `commits.preset`, other analysis/notes keys in their plugin tables** — upstream users must repeat `preset` per plugin and forget to; plugin-owned keys stay validated by that plugin, so a replacement analyzer brings its own. (→ [CONFIG](CONFIG.md))
- **Deep merge by default, `config.merge = "shallow"` opt-in** — overriding one key of an inherited table shouldn't need copying the table; arrays replace so presets' lists never mix; shallow stays available for upstream-like behaviour. (→ [CONFIG](CONFIG.md))
- **0.x: breaking → minor, feature/fix → patch** — matches how cargo and npm `^0.y` resolve, so dependents get features but never breaks; same as release-plz. (→ [CONFIG](CONFIG.md))
- **Graduate to 1.0.0 only via `Release-As:`** — 1.0.0 declares a stable API, a human decision often without a code change; `!` and `BREAKING CHANGE:` are equivalent in the spec and already mean "minor" on 0.x. (→ [CONFIG](CONFIG.md))
- **Build metadata only in the git tag, identity by version** — npm and crates.io drop `+meta` and Docker rejects it, so only the tag and forge release keep it; matching by version fixes upstream's #2355 class of bugs. (→ [CONFIG](CONFIG.md))
- **Lenient parser by default, `strict` opt-in, parse errors always shown** — a typo must not block releases by default, yet never be silent; teams that want enforcement opt in. (→ [CONFIG](CONFIG.md))
- **Parse merges like upstream, exempt unparsable merges/fixups from strict** — some teams make only the merge (PR title) conventional, so skipping merges would lose bumps; default merge messages must not break strict mode; `strict_merges` opts in. (→ [CONFIG](CONFIG.md))
- **minijinja, not Handlebars or lodash** — Jinja syntax is what users of git-cliff, PSR and cocogitto know; Handlebars is weak for logic; lodash means JS. (→ [CONFIG](CONFIG.md))
- **Own `{version}` syntax for `tags.format`** — versions must be parsed back out of tags, so the format has to be reversible. (→ [CONFIG](CONFIG.md))
- **Declarative TOML + templates instead of user JS functions** — no embedded scripting runtime; anything beyond it is a replacement plugin. (→ [CONFIG](CONFIG.md))
- **Every config key in a domain table** — keys can grow without later moves (ruff moved its top-level lint keys into `[lint]` in 0.2 with deprecation warnings for every user); one domain is one schema type, one docs section and one merge unit. (→ [CONFIG](CONFIG.md))
- **`branches.rules` as an array of names or inline tables** — order matters (the first release branch is the main line), so a table keyed by name can't hold it; plain names stay one word, and upstream's array maps one to one. Maintenance is an explicit `{ maintenance = "N.x" }` entry, not a reserved name. (→ [CONFIG](CONFIG.md#3-branches))
- **Plugin order from `steps.plugins`, options in `[plugins.<name>]`** — TOML gives table order no meaning, and order must survive layer merges; an array replaces whole. (→ [CONFIG](CONFIG.md#9-plugins-and-steps))
- **`extends` from pinned git refs, no npm** — no npm runtime and no registry to run; a SHA pin gives integrity. (→ [CONFIG](CONFIG.md))
- **JSON Schema for plugin config, not TOML Schema** — editors (Taplo/SchemaStore) support it today and schemars generates it; TOML Schema is not 1.0 yet. (→ [CONFIG](CONFIG.md))
- **`fancy-regex`** — upstream patterns with lookaround or backreferences port unchanged; a backtrack limit prevents hangs. (→ [CONFIG](CONFIG.md))
- **`icu_collator` for notes sorting** — the only option matching JS `localeCompare` exactly (0/36 mismatches) for +1.1 MiB. (→ [CONFIG](CONFIG.md))
- **`git-conventional` as-is** — maintained, zero-copy, handles `!` and both breaking footers; its two known spec deviations (footer `:` without space, lowercase `breaking-change`) are cheaper to document than to patch. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **`semver` crate + own bump/range** — strict SemVer 2.0.0 parsing used by cargo; it lacks bump and npm ranges, which are small to write. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))

## Git

- **git2, not gix** — gix has no push and no ETA for it. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **No git CLI** — a hard requirement; git2 covers every operation, proven by PoC against real GitHub. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Retry a bare per-ref `failed` once** — GitHub answered ~1 in 30 tag pushes that way in the PoC and a retry succeeded; the commit check makes the retry safe. (→ [ARCHITECTURE](ARCHITECTURE.md#6-git-and-credentials))
- **A failing `success` step never rolls back; warn by default** — the release is already public; a missed comment must not delete a tag or claim failure (github#738). `steps.success.errors = "fail"` for teams that want CI red. (→ [ARCHITECTURE](ARCHITECTURE.md#7-failure-and-rollback))
- **Own channel-notes refs, upstream's read too** — migrated repos keep channel history with no conversion step, while semoxide never writes under upstream's name. (→ [ARCHITECTURE](ARCHITECTURE.md#6-git-and-credentials))
- **No libssh2; russh by default** — on Windows libssh2 accepts only PEM RSA key files, hangs on others and its handshake fails intermittently (#804); jj left git2 for the same SSH reasons. russh costs +4.0–4.6 MB, ~143–186 extra crates and ~2× clean build time (measured in the PoC) but needs no external binary. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **System `ssh` only opt-in** — needed just for OpenSSH-only features (ProxyJump, FIDO); CI almost never needs SSH since HTTPS tokens push everywhere. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Configured token beats remote credentials** — avoids semantic-release/gitlab#891. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Tags only by default** — avoids extra commits, CI loops, branch-protection holes, push races and signing issues; upstream's own git plugin advises against committing back. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Tag first, `rollback` on later failure** — keeps upstream's order; upstream has no answer for partial failure (#896, #2381). (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Only the core deletes tags** — a plugin must never be able to remove a release tag. (→ [ARCHITECTURE](ARCHITECTURE.md))

## Plugins

- **Process plugins over a local socket, not stdio** — a stray `println!` or a child such as `npm publish` corrupts a stdout protocol (measured; Nushell has the same flaw); a socket leaves stdout/stderr free to capture as logs. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **gRPC/Protobuf** — codegen for any language, deadlines, cancellation and field-evolution rules built in, at ~4 ms per plugin (measured); the HashiCorp go-plugin model. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Not WASM** — plugins must run tools (`cargo publish`, `npm publish`), which WASI can't without a host exec escape hatch that defeats the sandbox; +21 MB to the host. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Not dylib** — Rust has no stable ABI; a crash takes the host down and unloading is unsafe. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Protocol in its own repo and version** — Nushell's lockstep protocol forces every plugin to rebuild each release (67 of 77 outdated); wire types are explicit, never derived from internal types. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Plugins in separate repos** — own version and release cycle, replaceable. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **One plugin process per run** — enough for a release tool and keeps state (API client, rate limits) across steps; idle shutdown only matters for an interactive shell. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Download from GitHub releases with a checksum lock** — no registry to run; go-semantic-release's own registry was an infra burden. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Secrets from a manifest read before spawn** — children like `npm` must inherit them from the start. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Narrow host `Git` service** — plugins commit through semoxide so the tag and commit-back rules can be enforced. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Analyzer and notes as separate repos, bundled in-process** — replaceable like any plugin, yet the default run spawns nothing; they are the first users of the in-process path. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Sync `Plugin` trait** — release steps run sequentially, so async buys nothing; the cost for embedders is about one OS thread per concurrent release. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **SDK as a pinned git dependency until protocol 0.1.0** — the protocol changes too fast for crates.io releases before it is usable. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **npm not in the first plugin set** — its design (hybrid package manager + native HTTP) is still tentative. (→ [ARCHITECTURE](ARCHITECTURE.md))

## Code

- **Async only in the plugin host and SSH bridge** — async only where concurrency is real; a private runtime on its own thread also works inside an embedder's tokio runtime. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **`semoxide-error` as its own crate** — every crate, including the pure ones, implements `ErrorInfo` without pulling in any dependency. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **Crates split by purity and heavy deps** — the version engine and schema build and test without git2, tonic or tokio: fast rebuilds, and miri can run them. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **`unsafe_code` denied in the workspace, forbidden per crate root** — Cargo can't exempt one crate from an inherited workspace lint, and `forbid` can't be lifted for semoxide-git's single transport registration. (→ [CLAUDE.md](../CLAUDE.md))
- **Only the façade is stable** — published inner crates are required by crates.io, but a promise on them would freeze the internals (uv/ruff's model). (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **Individual write steps not public** — embedders can't bypass the safety rules. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))

## Observability and CLI

- **`semoxide release`, bare `semoxide` prints help** — typing the tool name must never release; an explicit verb is clearer for agents and matches the other subcommands. (→ [CLI](CLI.md))
- **Generic `--set key=value` instead of per-option flags** — covers every key incl. plugin options with one rule and no flag list to maintain (cargo `--config` model). (→ [CLI](CLI.md))
- **Library never prints; env snapshot** — the embedder decides output, and the library never reads or changes process state. (→ [OBSERVABILITY](OBSERVABILITY.md))
- **Own masking everywhere** — GitLab masks only predefined variables, so semoxide's masking is the only protection there. (→ [OBSERVABILITY](OBSERVABILITY.md))
- **"No release" is exit 0 with a typed reason** — making it an error is a common complaint (go-semrel #8, cocogitto #457), and silence makes it undebuggable. (→ [CLI](CLI.md), [OBSERVABILITY](OBSERVABILITY.md))
- **`fail` always runs** — plugins can report unexpected errors too, not only known ones. (→ [OBSERVABILITY](OBSERVABILITY.md))
- **Skip marker excludes commits from notes too** — fixes upstream's notes bug (release-notes-generator #531). (→ [CONFIG](CONFIG.md))
- **`version` prints nothing when there is no release** — the upstream workaround (#1647) writes no file in that case and users script around it; empty stdout is a documented signal, a "current version" would be ambiguous. (→ [CLI](CLI.md))
- **`$GITHUB_OUTPUT` uses the cycjimmy action's names** — migrated workflows keep their `steps.*.outputs` conditions unchanged. (→ [OBSERVABILITY](OBSERVABILITY.md))
- **Outside CI: `NotCi` plus the would-be result** — answers both "why wasn't it released" and "what would CI do". (→ [OBSERVABILITY](OBSERVABILITY.md#7-no-release-reasons))
- **Dry-run needs no push rights** — upstream's push check in dry-run is a frequent complaint (#2232). (→ [CLI](CLI.md))
- **Agent-ready CLI (JSON everywhere, `schema`, retry hints, non-interactive)** — agents and scripts drive releases; versioned contracts and safe-retry flags keep them from guessing. (→ [CLI](CLI.md))
- **No MCP server in v1** — the MCP spec and its Rust SDK haven't settled; a release must never be one tool call away. (→ [CLI](CLI.md))

## Testing and quality

- **Fixtures via the real git CLI** — independent of the code under test; only the tool itself must be git-CLI-free. (→ [TESTING](TESTING.md))
- **Upstream tests read, not ported** — semoxide is not a 1:1 rewrite, so upstream suites serve as a source of edge cases; our own cases cover deliberately matching behaviour. (→ [TESTING](TESTING.md))
- **Containers per repo** — core runs only the git-http container on PRs; registry containers live in the plugin repos that publish to them. (→ [TESTING](TESTING.md))
- **cargo-deny: unmaintained only for direct dependencies** — cargo-deny 0.20 has no warn level; failing on abandoned transitive crates would turn upstream events into red CI and ignore-entry approvals we can't act on. Vulnerabilities fail at any depth. (→ `deny.toml`)
- **cargo-deny `allow-wildcard-paths`** — `semoxide-test-support` is never published, so its users depend on it by path only (cargo strips path dev-dependencies on publish); real `*` versions and path deps of published crates still fail. (→ `deny.toml`)
- **Unit tests inline (Rust Book)** — the Book and most Rust projects (cargo, ripgrep, uv, ruff, jj, helix) keep unit tests next to the code; rustc's separate `tests.rs` exists to avoid rebuilding core/std, a cost that doesn't apply at our size. (→ [TESTING](TESTING.md#where-tests-live))
- **Modules split by domain, not by stage or size** — our config reader is hand-written, so each validation belongs next to its type; Cargo, Ruff and uv split types from parsing only because serde generates their parsing. A line limit would split code that belongs together. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#modules-and-files))
- **Clippy lints chosen one by one** — clippy advises against enabling whole `restriction`/`nursery` groups; we took uv's and ruff's hygiene set plus lints that enforce our own rules (`mod_module_files`, `fallible_impl_from`, `iter_over_hash_type`), and left off lints that flag deliberate idioms: `map_err_ignore` (coded errors replace library errors on purpose), `let_underscore_must_use` (`let _ =` is the Rust idiom; uv, ruff and cargo use it), `unused_trait_names` (style only), `unwrap_in_result` (`unwrap_used` already covers product code). (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **Fakes, not mocks** — fakes (a fake plugin binary, wiremock) exercise real protocol and HTTP paths. (→ [TESTING](TESTING.md))
- **Approved tests protected by rules and review, not tooling** — agents act with the maintainer's own GitHub account, so any lock (hook, label, check) can be bypassed with the same token, and it adds friction to every test change; notable Rust projects (rust-lang/rust, turso, Bun, uv, biome) rely on written rules plus review. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#10-approaches))
- **miri on the pure crates** — AI-assisted coding raises the risk of subtle undefined behaviour. (→ [TESTING](TESTING.md))
- **Upstream comparison in development only** — after the first release semoxide's own tests are the spec. (→ [TESTING](TESTING.md))
- **Sandbox token from inside the sandbox repo** — scoped to one repo and one run; no long-lived PAT. (→ [TESTING](TESTING.md))
- **Findings fixed, rules never loosened** — loosened rules hide real problems. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md))
- **lefthook runs the hooks** — one versioned `lefthook.yml` lists every hook step (format, lint, secrets, tests) with globs and staged-file fixing; no per-tool hook installers. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **rustfmt and clippy via cargo** — cargo on `rust-toolchain.toml` gives one version in hooks, CI and the editor; a separately bundled Rust would format and lint differently. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **mise for dev tools** — global installs collide across projects; one committed `mise.toml` gives per-directory versions locally and the same versions in CI, and Renovate understands it. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#7-workspace-config))
- **No qlty: tools run directly** — mise pins, lefthook, one CI step each. qlty's smells never gated anything (not part of `qlty check`; `block` is a Qlty Cloud label) and miscounted rstest `#[case]` attributes; the wrapper added an old bundled Rust and sandbox quirks. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **No code-duplication check** — none of rustc, cargo, rust-analyzer, uv, ruff, ripgrep, tokio, jj, helix runs one; review + clippy instead. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **gitleaks with one fake-token shape** — offline and catches revoked keys too; a fixed `SEMOXIDE_FAKE` shape avoids per-fixture allowlist approvals. trufflehog reports only live-verified keys and needs network. (→ [TESTING](TESTING.md))
- **osv-scanner dropped** — cargo-deny reads the same RustSec advisories for `Cargo.lock`; no other lockfiles; revisit if one appears. trivy is not used either: it duplicates gitleaks (secrets) and cargo-deny (lockfiles); its Dockerfile scanner is revisited once there is an image. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **semgrep with our own rules only** — registry packs need network and the semgrep-rules (Semgrep Rules License) and Trail of Bits (AGPL) sets can't be copied into an MIT/Apache repo; every public Rust rule we reviewed is covered by clippy or cargo-deny. Our rules enforce documented rules clippy can't express, each with a `semgrep --test` fixture. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **Prefer clippy over semgrep when both can check a rule** — clippy parses real Rust, resolves imports and shows in rust-analyzer: `// SAFETY:` via `undocumented_unsafe_blocks`, env/process limits via per-crate bans with `#[expect]` at each allowed site. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))
- **Repo source checks as plain-text scans (tidy style)** — the Rust compiler's `tidy` and rust-analyzer check their own source with plain text and accept edge cases; a `proc-macro2` lexer version doubled the code to handle comment and string cases that don't occur here. Checks semgrep can't express as syntax patterns live in `source_rules`. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#10-approaches) A3)
- **One root `clippy.toml`, exceptions via `#[expect]`** — clippy reads only the nearest file, so per-crate files would each repeat every ban; the bans apply everywhere and the few allowed sites stay visible in code. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#2-crate-boundaries-and-enforcement))
- **markdownlint MD013 off** — table rows can't be wrapped and hard-wrapped prose makes doc diffs noisy; doc length is governed by the "short docs" rule. (→ [CODE-ARCHITECTURE](CODE-ARCHITECTURE.md#quality-tooling))

## Project

- **`MIT OR Apache-2.0`** — the Rust norm; Apache adds a patent grant, MIT stays GPLv2-compatible, both accept the incoming MIT/ISC/CC BY material. (→ [REQUIREMENTS](REQUIREMENTS.md))
- **Repos public, sandbox private** — on the Free plan private repos lack protected branches, rulesets, environments, attestations and Pages, and public Actions minutes are free; the sandbox holds test credentials. (→ [REQUIREMENTS](REQUIREMENTS.md))
- **PoCs in `semoxide-poc`** — throwaway code stays out of the product repo while the evidence stays linkable. (→ [REQUIREMENTS](REQUIREMENTS.md))
- **macOS dropped for now** — CI never releases from macOS, and macOS users can run the Docker image. (→ [ARCHITECTURE](ARCHITECTURE.md))
- **Docs site deferred to beta** — nothing stable to document before then; its tooling is chosen then. (→ [REQUIREMENTS](REQUIREMENTS.md))
