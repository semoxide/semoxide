# 0016 Agent experience
Status: accepted (2026-10-05). Decided step by step: making semoxide easy and safe for AI agents and scripts to drive.

## Decided
- **Stable, versioned JSON output (C1):**
  - Every JSON output carries `"schema_version"`.
  - Its JSON Schema is generated from the Rust types (schemars) and published with each release.
  - Fields are only ever added. Renames and removals happen only with a `schema_version` bump in a major release.
- **JSON on every command (C2):** every command (`version`, `explain`, `doctor`, `--dry-run`, `migrate`, the main run, …) supports `--output=json` with a published schema. A test fails if any command lacks either.
- **Structured dry-run plan (C3):** the `plan` RPC returns typed actions `{ plugin, step, action, target, details }` instead of text lines (an additive protocol change, [ADR 0010](0010-plugin-architecture.md)). Text output renders them as lines; JSON carries them in the result, so agents or CI policies can check a planned release before allowing it.
- **Self-description (C4):** `semoxide schema` (offline, JSON) and its alias `semoxide help --output=json` (the same `--output` flag as every command, no separate `--json`). It lists every command and flag, the exit codes, all error codes with docs links, and the JSON Schemas, generated from the CLI definition and error catalog so it cannot drift. With a config present it also includes the configured plugins' config schemas (`describe`, [ADR 0010](0010-plugin-architecture.md)). It never downloads plugins: if the local plugins and lock aren't in sync with the config, it warns and tells the user to run `semoxide sync` first.
- **`init` and `sync` are separate commands:**
  - `semoxide init` creates a new `semoxide.toml`.
  - `semoxide sync` downloads the pinned plugins, verifies their checksums and writes or updates the lock file ([ADR 0010](0010-plugin-architecture.md)).
  - Like `uv init` / `uv sync`.
  - `init` detects the forge (git remote), the ecosystem (`Cargo.toml`, `package.json`, …) and the release branch, then writes a short `semoxide.toml`: default branches, the matching forge plugin, and the analyzer/notes defaults. If a `.releaserc` exists, it offers `semoxide migrate` instead.
  - In a terminal, `init` asks to confirm or adjust the detections. Without a TTY it never prompts and uses flags and defaults.
- **Non-interactive rule (C5):**
  - Without a TTY, or with `--no-input`, semoxide never prompts, pages, opens an editor or shows spinners or progress bars. Anything that needs an answer fails fast, naming the flag to pass instead.
  - Child processes get `GIT_TERMINAL_PROMPT=0`, `GIT_PAGER=cat`, `GIT_EDITOR=true`, and BatchMode for `ssh`.
  - A test runs every command without a TTY and fails on any prompt or hang.
- **Retry hints on errors (C7):** every JSON error carries `retryable` (transient: network timeouts, rate limits, the SSH handshake flake) and `remote_writes_happened` (true once anything was pushed or published, [ADR 0012](0012-partial-failure.md)). Retrying is safe only when `retryable && !remote_writes_happened`.
