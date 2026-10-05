# Plugin mechanisms

The decision is in [ADR 0010](../decisions/0010-plugin-architecture.md). This file holds the evidence behind it.

Need: semantic-release's step plugins (see [spec](../specs/SEMANTIC-RELEASE-SPEC.md)) for a library-first Rust crate. Plugins need network, fs, env/secrets and often run commands (`npm publish`, `cargo publish`).

## Mechanisms

"Measured" = the [stdio PoC](../../poc/plugin-protocol/README.md) or the [gRPC PoC](../../poc/plugin-grpc/README.md) (Windows 11 desktop; Linux = 1-vCPU VPS in Docker). Everything else is from docs or memory; "(unverified)" marks memory.

| Mechanism | Authoring / distribution | Windows | Runs commands | Isolation | Evolution | Cost |
|---|---|---|---|---|---|---|
| Compiled-in (`trait Plugin`, crate) | Rust only; rebuild the host | yes | yes, but in-process children and stdout/stderr escape kill and masking (measured: the demo's raw token print) | none; a timeout only drops the future | crate semver | call 0.001–0.003 ms (measured) |
| Process, stdio (JSON-RPC / NDJSON) | any language; per-OS binary, PATH or URL+sha256 | works, but killing a `.cmd`-shimmed plugin orphans the grandchild without a Job Object (measured) | yes, but a child that inherits stdout writes into the protocol stream (measured; nushell has the same flaw) | OS process | protocol version in handshake | spawn+handshake 5.1 ms Rust, 34 ms Python, 51 ms via `.cmd`; call 0.036 ms (measured, Windows) |
| Process, local socket (UDS / named pipe) | as stdio | named pipes via tokio, no `interprocess` crate; needs a custom tonic connector, and `WaitNamedPipeW` against `ERROR_PIPE_BUSY` (measured) | yes; stdout/stderr are free to capture as logs (measured, 250 noise lines) | OS process + Job Object / process group kill of the whole tree (measured on both OSes) | as below | see gRPC row |
| Encoding: plain messages (JSON-RPC, msgpack) | hand-written types per language; nushell derives them from internal types and so can't version independently | — | — | — | capability/version handshake (LSP style) | baseline |
| Encoding: gRPC / Protobuf | codegen for any language; deadlines, cancellation, streaming, errors built in | tonic reports a client deadline as `CANCELLED` (measured) | — | — | Protobuf field-evolution rules | +~4 ms per plugin, +~0.05 ms per call vs stdio JSON; total spawn+connect+handshake 9.0 ms Windows, 5.6 ms Linux (measured) |
| WASM component (wasmtime + WIT, or extism) | Rust/Go/JS/C via wit-bindgen or extism PDKs; Rust `wasm32-wasip2` worked first try (measured); one `.wasm` for all OSes | yes | **only through a host-provided capability** (custom `exec` import / host function, as proto/moon do); plain WASI gives `Unsupported` (measured). The capability runs outside the sandbox | real: preopened dirs, explicit env, allowed hosts (measured env/fs) | WIT package versions; WASI 0.2 sync, WASI 0.3 (ratified 2026-06, Wasmtime 46) native async | host +21.5 MB (1.3 → 22.8 MB), +3 min release build, plugin 140 KB, cold compile 29 ms, precompiled load 0.7 ms, call 0.001 ms (measured) |
| Dylib (`libloading`, `abi_stable`, `stabby`) | Rust/C, exact toolchain or stable-ABI crate; per-OS/arch binary | DLL locking, no safe unload | yes | none; a crash takes the host down | ABI breaks on rustc/dependency changes; `abi_stable` slow-moving, `stabby` younger (unverified); allocator and panic-across-FFI hazards | zero |
| Exec hooks (shell step, ≈ `@semantic-release/exec`) | any; config + templates | yes | it is the command | none | none; results parsed from output | one process per hook |

## Precedents

| Tool | Mechanism | Takeaway |
|---|---|---|
| cargo (unverified) | `cargo-<x>` on PATH | name convention + PATH lookup = zero-config install |
| mdbook preprocessors (unverified) | `mdbook-<x>`, `supports` probe, JSON on stdin/stdout | simplest one-shot JSON stdio; config passed through as-is |
| protoc plugins (unverified) | `protoc-gen-<x>`, protobuf request on stdin, response on stdout | one-shot request/response suffices for pure functions |
| LSP (unverified) | JSON-RPC 2.0, `Content-Length` framing, `initialize` capability negotiation | negotiate capabilities rather than pin versions |
| HashiCorp go-plugin (unverified) | gRPC over a local socket, handshake line on stdout, magic-cookie env, protocol version | the model ADR 0010 follows; the PoC measured its overhead as negligible |
| nushell | see [nushell-plugins](nushell-plugins.md) | lockstep protocol versioning and stdout discipline break plugins |
| dprint | Wasm plugins (wasmer → wasmtime) + process plugins (per-platform zip, **sha256 required**); config lists URLs | URL@checksum in config, cached download |
| zellij (unverified) | wasm plugins (wasmtime), per-capability permission prompts | prompts need an interactive user; CI has none |
| Zed (unverified) | wasmtime + WIT components (`zed_extension_api`), registry repo | typed WIT API, releases tied to an API version |
| Helix (unverified) | Steel (Scheme), not wasm | n/a: editor scripting |
| proto / moon (unverified) | extism WASM plugins + host functions for exec/HTTP | WASM tooling plugins need a host exec capability |
| sccache (unverified) | no plugin system | n/a |
| release-plz, knope, cocogitto; go-semantic-release (gRPC); moonlit (WASI) | see [competitors](competitors.md#rust-tools) | config + templates + a shell step cover most needs |

## PoC evidence

- [poc/plugin-protocol](../../poc/plugin-protocol/README.md) (stdio JSON-RPC + WASM, Windows): Rust plugin spawn 5.1 ms, Python 34–51 ms, calls under 0.2 ms even with a 7 KB context. WASM loads in 0.7 ms but adds 21.5 MB to the host and cannot exec without a host import. Failures: stdout shared with grandchildren, orphaned grandchildren on kill, `env_clear` dropping `SystemRoot`/`PATHEXT`.
- [poc/plugin-grpc](../../poc/plugin-grpc/README.md) (gRPC over named pipe / UDS, Windows + Linux): 9.0 / 5.6 ms to a configured plugin, ~0.1 / 0.3 ms per call, whole-tree kill on timeout and crash, conformance kit 9/9 on both OSes. Its open issues fed ADR 0010 and its implementation notes.

Sources: [WASI 0.3 launch](https://bytecodealliance.org/articles/WASI-0.3), [dprint plugins](https://dprint.dev/plugins/), [dprint plugin dev](https://dprint.dev/plugin-dev/), [dprint 0.55.0 release](https://github.com/dprint/dprint/releases/tag/0.55.0)

## Ticket candidates
- `semoxide-plugin-protocol` repo: v1 `.proto` (typed context, step results incl. `rollback`, `describe`, handshake, host `Log`/`Git` services, config as `Struct`)
- `semoxide-plugin-sdk`: `Plugin` trait, `serve()`, UDS/named-pipe transport, token check, `PATHEXT` program resolution, in-process restrictions
- `semoxide-plugin-host`: create socket, spawn with `--socket` + token, Job Object / process group, per-step deadlines (`CANCELLED` and `DEADLINE_EXCEEDED`), output capture
- `semoxide-plugin-conformance`: checks for process and in-process paths, runnable from any plugin repo's CI
- Plugin manifest: format, shipped per release, read before spawn for secret env
- Plugin resolution: version pin → GitHub release download, checksum lock file, cache; `path` and PATH overrides
- Host services in semoxide: masked `Log`; `Git` add/commit/push with the release-tag and [ADR 0009](../decisions/0009-commit-back.md) rules
- Step orchestration: config order, per-step `order`, result merging, `rollback` ([ADR 0012](../decisions/0012-partial-failure.md))
- Config validation via `describe` schemas, before any step; plugin options in the `semoxide.toml` editor schema
- Plugin output handling: masked tagged logs, `show_output`, `--log-file`, per-plugin working-tree diff and the `assets` check
- Repos `semoxide-plugin-commit-analyzer` and `semoxide-plugin-release-notes`, compiled in as bundled defaults
- First official plugin repos: github, cargo, exec, gitlab, changelog, git
- Attestation (Sigstore) verification once plugin repos are public
- Investigate: Job Object assigned at process creation (`CreateProcessW` / `raw_attribute`); Unix cleanup when semoxide itself dies
