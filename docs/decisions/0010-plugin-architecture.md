# 0010 Plugin architecture
Status: accepted in part (2026-10-05). Decided step by step; open points are listed below.

## Decided
- **Separate repos:** each official plugin lives in its own repo with its own version and release cycle.
- **Not stdio:** the protocol runs over a native local socket, never over the plugin's stdin/stdout. That is a Unix domain socket on Linux/macOS and a named pipe on Windows (e.g. via the `interprocess` crate); the SDK hides the difference.
- **Own protocol repo:** the protocol has its own repo and its own version, independent of semoxide's version. This avoids Nushell's lockstep pain ([nushell research](../research/nushell-plugins.md)).
- **The protocol repo contains:** the spec, a Rust SDK crate (message types, socket and handshake code, the `Plugin` trait, `serve()`), and a conformance kit. The kit is a test tool any plugin repo runs in CI to prove it follows the protocol, whatever language the plugin is written in.
- **semoxide starts the plugin:** it creates the socket, starts the plugin with the socket address (e.g. `--socket <addr>`), and stops it when the run ends, together with any processes the plugin started.
- **One process per run:** a plugin is started once, receives every step it implements, and can keep state between steps (e.g. its API client and rate-limit info).
- **gRPC over the local socket (Protobuf).** The `.proto` file in the protocol repo is the spec: it defines both the messages and the plugin service. Client and server code is generated for any language (Rust: `tonic`). Protobuf's field-evolution rules govern compatibility, and gRPC provides calls, errors, deadlines, cancellation and streaming. This is the same model as HashiCorp go-plugin. Windows named pipes need a custom connector in tonic.
- **Getting plugins:**
  - By default, config pins a version (`[plugins.github] version = "1.4.2"`). semoxide downloads that binary from the plugin's GitHub release, checks its checksum (recorded in a lock file) and caches it.
  - Override: an explicit `path = "..."` or PATH lookup of `semoxide-plugin-<name>`, for plugin development and offline/air-gapped CI.
- **Embedders:** each Rust plugin repo publishes a library crate as well as its binary, both implementing the SDK `Plugin` trait. Embedders can link a plugin in-process and register it in the builder (no process, no socket), or use binaries like the CLI does. The conformance kit tests both paths.
- **Version compatibility:** the protocol uses SemVer, and host and plugin must share the same major version. Within a major version only additive Protobuf changes are allowed (new fields, new optional RPCs), and both sides ignore what they don't know. A major bump breaks every plugin at once, so it must be rare.
- **Environment and secrets:** a plugin gets the system variables (`PATH`, `SystemRoot`, `PATHEXT`, …) plus only the secret variables it declares in its manifest or handshake. Every declared secret is registered for log masking.
- **Host services:** during a step, a plugin can call a small gRPC service offered by semoxide: `Log`, plus a narrow `Git` service (add, commit, push) backed by semoxide's git2 and credentials. semoxide enforces its rules there: only the release tag is pushed, existing tags are never moved, and commit-back follows [ADR 0009](0009-commit-back.md). Plugins get no generic access to the host.
- **Several plugins on one step:** semantic-release's rules apply. All plugins run in config order. Errors are collected. For analyzeCommits the highest release type wins; for generateNotes the outputs are joined ([spec](../specs/SEMANTIC-RELEASE-SPEC.md)). Config can override the order for a single step, e.g. `[steps.publish] order = ["npm", "github"]`.
- **Lifecycle steps:** semantic-release's 9 steps (`verify_conditions`, `analyze_commits`, `verify_release`, `generate_notes`, `prepare`, `publish`, `add_channel`, `success`, `fail`) plus **`rollback`**. `rollback` runs when a step fails after the tag is pushed; each plugin undoes its own work, and plugins whose work can't be undone log a warning ([ADR 0012](0012-partial-failure.md)).
- **Plugin config validation:**
  - Each plugin provides a JSON Schema for its config, via a gRPC `describe` call and published with its release (generated with schemars for Rust plugins).
  - semoxide validates all plugin config before any step runs, and the editor schema for `semoxide.toml` includes the plugin options.
  - Plugins still do runtime checks (e.g. token permissions) in `verify_conditions`.
- **Trusting downloaded plugins:**
  - Now: a checksum lock only. The sha256 recorded on first download is required on every later run.
  - Once the plugin repos are public: also verify GitHub artifact attestations (Sigstore, e.g. the `sigstore` crate) before first use. GitHub's Free, Pro and Team plans only offer attestations for public repos.

## Open
Discussed one at a time; each answer is added to the Decided list above.
