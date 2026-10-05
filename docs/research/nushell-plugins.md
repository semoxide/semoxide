# Nushell plugins

Sources: nushell/nushell @ `04680006` (v0.116.2, 2026-10-04) and nushell.github.io; checked against the raw files.

| Aspect | Nushell |
|---|---|
| Model | One executable per plugin (`nu_plugin_<name>`), many commands per plugin ([book](https://github.com/nushell/nushell.github.io/blob/main/book/plugins.md)) |
| Registration | `plugin add <path>`: runs a handshake and caches the plugin's commands in `plugin.msgpackz`. `plugin use` loads or reloads it in a session |
| Lifecycle | Spawned lazily on first use and kept alive. Stops after 10 s idle (`plugin_gc`); a plugin can opt out |
| Transport | stdio is mandatory. A local socket is optional: a Unix socket or a Windows named pipe, advertised as a feature in `Hello`, with relaunch via `--local-socket` ([#12448](https://github.com/nushell/nushell/pull/12448), [reference](https://github.com/nushell/nushell.github.io/blob/main/contributor-book/plugin_protocol_reference.md)) |
| Encoding | The plugin picks it with its first bytes: `\x04json` or `\x07msgpack` |
| Messages | `Hello{version, features[]}` (unknown features are ignored); multiplexed `Call`/`CallResponse`; `EngineCall` lets the plugin call back into the host; streams with `Ack` backpressure; `Goodbye` |
| Stdout safety | None. The rule is just "don't use stdout" (a stray `println!` breaks the run). The socket mode exists partly for this |
| Windows | Named pipes; scripts run through an interpreter picked by file extension; each plugin gets a new process group |
| SDK | `nu-plugin`: `Plugin` + `PluginCommand` traits, `serve_plugin()`, and `nu-plugin-test-support` for in-process tests. Non-Rust plugins are rare (one third-party Go SDK) |
| Official plugins | In the main repo, versioned and released in lockstep with Nushell, and shipped in the release archive |

## Versioning pain
- The protocol version **is** Nushell's crate version. While Nushell is 0.x the minor version must match, so every plugin must be rebuilt every release ([protocol_info.rs](https://github.com/nushell/nushell/blob/main/crates/nu-plugin-protocol/src/protocol_info.rs)).
- 67 of 77 third-party plugins are flagged as outdated ([awesome-nu](https://github.com/nushell/awesome-nu/blob/main/plugin_details.md)). A maintainer: "sick of recompiling every release" ([#18297](https://github.com/nushell/nushell/issues/18297)).
- Fix in progress: an independent protocol version ([#18079](https://github.com/nushell/nushell/pull/18079), open). It is blocked because the wire format is serde-derived from internal types, so a separate version number alone doesn't guarantee anything.

## Lessons
- Give the protocol its own version from day one, with explicit wire types that are never derived from internal types ([ADR 0010](../decisions/0010-plugin-architecture.md)).
- Don't depend on stdout discipline. Plugins that run noisy tools (e.g. `npm publish`) will break it.
- A plugin process that lives for one run is enough for a release tool; Nushell's idle-shutdown lifecycle exists because it is an interactive shell.
