# 0010 Plugin architecture
Status: accepted (2026-10-05). Decided step by step.

## Decided
- **Separate repos:** each official plugin lives in its own repo with its own version and release cycle.
- **Not stdio:** the protocol runs over a native local socket, never over the plugin's stdin/stdout. That is a Unix domain socket on Linux/macOS and a named pipe on Windows (tokio's own support is enough; the PoC needed no `interprocess`). The SDK hides the difference.
- **Own protocol repo:** the protocol has its own repo and its own version, independent of semoxide's version. This avoids Nushell's lockstep pain ([nushell research](../research/nushell-plugins.md)).
- **The protocol repo contains:**
  - the spec (the `.proto` files)
  - `semoxide-plugin-sdk`: the plugin side (message types, socket and handshake code, the `Plugin` trait, `serve()`)
  - `semoxide-plugin-host`: the host side (start, connect, kill and handshake), used by semoxide and the conformance kit
  - `semoxide-plugin-conformance`: a test tool any plugin repo runs in CI to prove it follows the protocol, whatever language the plugin is written in
- **semoxide starts the plugin:** it creates the socket, starts the plugin with the socket address (e.g. `--socket <addr>`), and stops it when the run ends, together with any processes the plugin started.
- **One process per run:** a plugin is started once, receives every step it implements, and can keep state between steps (e.g. its API client and rate-limit info).
- **gRPC over the local socket (Protobuf).** The `.proto` file in the protocol repo is the spec: it defines both the messages and the plugin service. Client and server code is generated for any language (Rust: `tonic`). Protobuf's field-evolution rules govern compatibility, and gRPC provides calls, errors, deadlines, cancellation and streaming. This is the same model as HashiCorp go-plugin. Windows named pipes need a custom connector in tonic.
- **Getting plugins:**
  - By default, config pins a version (`[plugins.github] version = "1.4.2"`). semoxide downloads that binary from the plugin's GitHub release, checks its checksum (recorded in a lock file) and caches it.
  - Override: an explicit `path = "..."` or PATH lookup of `semoxide-plugin-<name>`, for plugin development and offline/air-gapped CI.
- **Embedders:** each Rust plugin repo publishes a library crate as well as its binary, both implementing the SDK `Plugin` trait. Embedders can link a pure-computation plugin in-process (see In-process limits) and register it in the builder (no process, no socket), or use binaries like the CLI does. The conformance kit tests both paths.
- **Version compatibility:** the protocol uses SemVer, and host and plugin must share the same major version. Within a major version only additive Protobuf changes are allowed (new fields, new optional RPCs), and both sides ignore what they don't know. A major bump breaks every plugin at once, so it must be rare.
- **Environment and secrets:**
  - A plugin gets the system variables (`PATH`, `SystemRoot`, `PATHEXT`, …) plus only the secret variables listed in its **manifest**, a file shipped with each plugin release.
  - semoxide reads the manifest before starting the plugin, so the secrets are in the plugin's environment from the start (children such as `npm` inherit them). The handshake declaration is only a consistency check against the manifest.
  - Every declared secret is registered for log masking.
- **Host services:** during a step, a plugin can call a small gRPC service offered by semoxide: `Log`, `RegisterSecret` ([ADR 0013](0013-observability.md)), plus a narrow `Git` service (add, commit, push) backed by semoxide's git2 and credentials. semoxide enforces its rules there: only the release tag is pushed, existing tags are never moved, and commit-back follows [ADR 0009](0009-commit-back.md). Plugins get no generic access to the host.
- **Several plugins on one step:** semantic-release's rules apply. All plugins run in config order. Errors are collected. For analyzeCommits the highest release type wins; for generateNotes the outputs are joined ([spec](../specs/SEMANTIC-RELEASE-SPEC.md)). Config can override the order for a single step, e.g. `[steps.publish] order = ["npm", "github"]`.
- **Lifecycle steps:** semantic-release's 9 steps (`verify_conditions`, `analyze_commits`, `verify_release`, `generate_notes`, `prepare`, `publish`, `add_channel`, `success`, `fail`) plus **`rollback`**. `rollback` runs when a step fails after the tag is pushed; each plugin undoes its own work, and plugins whose work can't be undone log a warning ([ADR 0012](0012-partial-failure.md)).
- **Plugin config validation:**
  - Each plugin provides a JSON Schema for its config, via a gRPC `describe` call and published with its release (generated with schemars for Rust plugins).
  - semoxide validates all plugin config before any step runs, and the editor schema for `semoxide.toml` includes the plugin options.
  - Plugins still do runtime checks (e.g. token permissions) in `verify_conditions`.
  - Schema language: JSON Schema for v1, because that's what editors support today (Taplo/SchemaStore) and schemars generates it. [TOML Schema](https://tomlschema.org/) (`.tosd`, spec 1.0.0-rc.2, 2026) is a candidate to re-evaluate once it reaches 1.0, has a crates.io release and has mainstream editor support.
- **Trusting downloaded plugins:**
  - Now: a checksum lock only. The sha256 recorded on first download is required on every later run.
  - Once the plugin repos are public: also verify GitHub artifact attestations (Sigstore, e.g. the `sigstore` crate) before first use. GitHub's Free, Pro and Team plans only offer attestations for public repos.
- **Bundled defaults:** commit-analyzer and release-notes live in their own repos (`semoxide-plugin-commit-analyzer`, `semoxide-plugin-release-notes`) like every plugin. semoxide depends on their crates and compiles them in, running them in-process and enabled by default. They are also released as binaries, so they can be replaced. They are the first users of the in-process crate path.
- **First official plugins** (each in its own repo): github, cargo, exec, gitlab, changelog, git. npm is not in the first set.
- **Naming:**
  - Each plugin uses `semoxide-plugin-<name>` for its repo, binary and crate (e.g. `sm-steel/semoxide-plugin-github`). Config uses the short name (`[plugins.github]`).
  - The protocol repo is `semoxide-plugin-protocol`. It holds the `.proto` spec, the SDK crate `semoxide-plugin-sdk` and the conformance kit `semoxide-plugin-conformance`.
- **Timeouts and crashes:** each step has a default deadline (e.g. verify 2 min, publish 30 min), which config can override per plugin and step (`[plugins.github] timeouts.publish = "1h"`). On a timeout or crash, semoxide kills the plugin and everything it started (a Job Object on Windows, a process group on Unix), fails the step, and runs `rollback`/`fail`.
- **Plugin output (stdout/stderr, including child processes such as `npm publish`):**
  - semoxide captures it as masked log lines tagged with the plugin name. It is never parsed: results come only via gRPC.
  - By default it is shown in full when that plugin's step fails, live with `-v`/`--debug`, and always written to `--log-file`.
  - A per-plugin `show_output = true` shows that plugin's output live on every run.
- **Files changed by plugins:**
  - As in semantic-release: plugins change files freely and the user sets the order. The `git` plugin commits and pushes its `assets` through the host `Git` service.
  - semoxide compares the working tree before and after each plugin, so it knows which files each plugin changed.
  - If a file matching git's `assets` changes after git has committed: during `prepare` this is an error by default (configurable down to a warning), so nothing is published with a stale commit; during `publish` it can only be a warning.
- **In-process limits:**
  - In-process plugins must not start child processes; the SDK only allows that in process mode. Plugins that run external tools (cargo, exec, git hooks, …) always run as processes.
  - In-process plugins log only via the `Log` API, which is masked; the conformance kit checks that they don't print directly.
  - On a timeout the step fails and the call is abandoned (an in-process plugin can't be killed).
- **Socket access control:**
  - OS permissions: the Unix socket lives in a private 0700 temp dir; the Windows named pipe has a security descriptor allowing only the current user.
  - Per-run token: semoxide passes a random one-time token to the plugin at start, in its environment. The plugin's first message must present it; connections without it are dropped while semoxide keeps listening.
  - Limit: a same-user process able to read process environments could steal the token, but it could equally read semoxide's own secrets.
- **Wire data shapes:** the context, step results and host services are typed Protobuf messages (semoxide's contract). Each plugin's own config section travels as `google.protobuf.Struct`, validated against that plugin's schema. No JSON strings inside Protobuf.

## Implementation notes (from the [gRPC PoC](../../poc/plugin-grpc/README.md); not decisions)
- Windows: the Job Object is assigned right after spawn, so a grandchild started in that instant could escape. A full fix needs `CreateProcessW` / `raw_attribute`, which is still unstable.
- Linux: a process group isn't cleaned up if semoxide itself crashes. The SDK exits when the connection drops; `PR_SET_PDEATHSIG` is optional. Children that call `setsid()` escape the group kill.
- Windows: the SDK resolves program names via `PATHEXT` (`npm` → `npm.cmd`); batch-file argument escaping needs care.
- tonic reports a client-side deadline as `CANCELLED`, so the host treats both `CANCELLED` and `DEADLINE_EXCEEDED` as a timeout.

## Open
Discussed one at a time; each answer is added to the Decided list above.
