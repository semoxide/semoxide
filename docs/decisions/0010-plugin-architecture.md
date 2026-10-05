# 0010 Plugin architecture
Status: accepted in part (2026-10-05). Decided step by step; open points are listed below.

## Decided
- **Separate repos:** each official plugin lives in its own repo with its own version and release cycle.
- **Not stdio:** the protocol runs over a native local socket, never over the plugin's stdin/stdout. That is a Unix domain socket on Linux/macOS and a named pipe on Windows (e.g. via the `interprocess` crate); the SDK hides the difference.
- **Own protocol repo:** the protocol has its own repo and its own version, independent of semoxide's version. This avoids Nushell's lockstep pain ([nushell research](../research/nushell-plugins.md)).
- **The protocol repo contains:** the spec, a Rust SDK crate (message types, socket and handshake code, the `Plugin` trait, `serve()`), and a conformance kit. The kit is a test tool any plugin repo runs in CI to prove it follows the protocol, whatever language the plugin is written in.
- **semoxide starts the plugin:** it creates the socket, starts the plugin with the socket address (e.g. `--socket <addr>`), and stops it when the run ends, together with any processes the plugin started.
- **One process per run:** a plugin is started once, receives every step it implements, and can keep state between steps (e.g. its API client and rate-limit info).

## Open
Discussed one at a time; each answer is added to the Decided list above.
