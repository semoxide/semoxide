# Plugin mechanisms

> **Verification:** only WASI 0.3 and dprint are backed by the sources below; other precedents and the size/latency estimates are from model memory and unverified (marked).

Goal: keep semantic-release's step plugins (`verifyConditions` … `fail`, see [spec](../specs/SEMANTIC-RELEASE-SPEC.md)) in a library-first Rust crate. Plugins need network, fs, env/secrets and sometimes `exec` (e.g. `npm publish`).

## Options

| | (a) Compiled-in | (b) Subprocess JSON-RPC | (c) WASM component | (d) Dylib | (e) Hybrid a+b |
|---|---|---|---|---|---|
| Mechanism | `trait Plugin` + `Box<dyn Plugin>`, cargo features for built-ins | child process, stdio, JSON-RPC 2.0 | wasmtime + WIT (or extism) | `libloading` / `abi_stable` / `stabby` | built-ins via (a); third party via (b) behind the same trait |
| Embeddable | best: embedder calls `builder.plugin(MyPlugin)` | good: adapter implements the trait | ok: pulls in wasmtime (~10–20 MB (unverified), long build) | poor: unsafe, per-target | best |
| Windows | yes | yes (pipes; avoid Unix sockets) | yes | DLL locking, no safe unload | yes |
| 3rd-party authoring | Rust only, needs a fork or custom build | any language, `stdin`/`stdout` | Rust/Go/JS/C/… via wit-bindgen or extism PDKs; toolchain friction | Rust/C only, exact toolchain or stable-ABI crate | any language |
| Install | rebuild binary | config entry + PATH / download + sha256 | config URL + sha256, one `.wasm` for all OSes | per-OS/arch binary | as (b) |
| Sandbox | none | none (OS process) | real: preopened dirs, explicit env, allowed hosts | none, crashes host | none (same as npm plugins) |
| Versioning | semver of the crate | protocol version + capability list in the handshake | WIT package versions | ABI breaks on rustc/dependency changes | crate semver + protocol version |
| Perf | zero cost | ~1–10 ms spawn per run (unverified); JSON negligible | ~ms instantiate with cached precompile; first compile is slow | zero cost | fine (a release is network-bound) |
| Async | native `async fn` | host async I/O; plugin can be anything | WASI 0.2 sync + pollables; WASI 0.3 (ratified 2026-06, Wasmtime 46) native async | can't share a tokio runtime across the boundary | native |
| Exec (`npm publish`) | yes | yes | **no**: WASI can't spawn, needs a custom host `exec` import (defeats the sandbox) | yes | yes |
| Secrets | `&Context.env` | inherited process env; never serialized into messages | host passes an explicit env allow-list | struct by pointer | as (a)/(b) |
| Logger | `&dyn Logger` in the context | `log` notifications on stdout; stderr captured at debug | host import `log(level,msg)` | FFI callback | as (a)/(b) |

**Rejected: (d).** Rust has no stable ABI. `abi_stable` is slow-moving with heavy type checks, `stabby` is younger (unverified). Both need matching allocators, have panic-across-FFI hazards and DLL lifetime problems on Windows, and still need N binaries per plugin. A subprocess gives the same reach with none of these costs.

## Precedents

| Tool | Mechanism | Takeaway |
|---|---|---|
| cargo (unverified) | external subcommands: `cargo-<x>` found on PATH | name convention + PATH lookup = zero-config install (`cargo install`, `cargo binstall`) |
| mdbook preprocessors (unverified) | `mdbook-<x>`: `supports <renderer>` probe, then JSON `[ctx, book]` on stdin, book JSON on stdout | simplest one-shot JSON stdio; config table passed through as-is |
| protoc plugins (unverified) | `protoc-gen-<x>`, protobuf request on stdin, response on stdout | a one-shot request/response is enough for pure functions |
| LSP (unverified) | JSON-RPC 2.0, `Content-Length` framing, `initialize` capability negotiation | capability negotiation, not version pinning, for evolution |
| Terraform go-plugin (unverified) | gRPC over a local socket, handshake line on stdout, magic-cookie env, protocol version | handshake line + negotiated protocol version; gRPC is heavy for us |
| nushell (unverified) | `nu_plugin_<x>` binaries, `plugin add` registry, JSON or MessagePack over stdio, persistent process, `Hello` with version | **avoid strict version coupling**: plugins break on every nu release |
| dprint | Wasm plugins (sandboxed; moved from wasmer to wasmtime) + process plugins (per-platform zip, **sha256 required**); config lists URLs | best distribution model: URL@checksum in config, cached download |
| zellij (unverified) | wasm plugins (wasmtime), permission prompts per capability | capability permissions work for UI apps; we have no interactive user in CI |
| Zed (unverified) | wasmtime + WIT components (`zed_extension_api`), registry repo | WIT is good for typed APIs, but releases are tied to an API version |
| Helix (unverified) | chose Steel (Scheme), not wasm | n/a: an editor scripting need |
| proto / moon (moonrepo) (unverified) | extism WASM plugins + host functions for exec/HTTP | shows WASM needs an "exec" escape hatch in tooling |
| sccache (unverified) | no plugin system (compiler wrapper, dist server) | n/a |
| release tools | release-plz/knope/cocogitto: no plugins, templates or shell hooks; go-semantic-release (gRPC), moonlit (WASI): [competitors](competitors.md#rust-tools) | config + templates + a shell step (≈ `@semantic-release/exec`) cover most needs |

## Recommendation: (e) hybrid

1. **Core trait** in the library: `#[async_trait]`/AFIT `Plugin` with one method per step (default impl = not implemented) plus `fn steps() -> StepSet`. `Context` holds a borrowed logger, a filtered env (secrets masked) and cwd/options/branch/commits/releases. Built-ins (commit-analyzer, notes, github, git, changelog, exec, npm?) sit behind cargo features. Embedders call `Semoxide::builder().plugin(impl Plugin)`.
2. **External protocol**: `ProcessPlugin` implements the same trait. One long-lived process per plugin per run, JSON-RPC 2.0, newline-delimited JSON on stdio.
   - Handshake: `initialize {protocol: 1, host_version, plugin_config}` → `{protocol, name, steps[]}`. Steps are requests; `log` is a notification.
   - Ignore unknown fields, so new fields are additive. Bump `protocol` only on a breaking change, and the host supports N and N-1.
   - Env is inherited (like semantic-release) and never put in messages. Secret masking applies to plugin logs and stderr.
3. **Resolution / install**: the built-in name is used first. Otherwise look up `semoxide-plugin-<name>` on PATH (cargo convention). Otherwise use `{ url = "...", sha256 = "..." }` with a per-OS archive, downloaded and cached (dprint model). An explicit `command = ["npx", "pkg"]` also works.
4. **SDK crates**: `semoxide-plugin-protocol` (serde types + JSON Schema via `schemars`) and `semoxide-plugin-sdk` (a stdio server loop that wraps any `impl Plugin`). The same trait then works compiled in or out of process.
5. **WASM: defer.** Its only advantage is sandboxing, and the plugins we need (npm, cargo publish) must exec, which breaks the sandbox. Revisit when a real untrusted-plugin use case appears. The protocol types can be reused through WIT.

## Wave B PoCs

| PoC | Decides |
|---|---|
| `poc/plugin-trait` | async trait shape, `Context` borrows, logger/secret masking, embedder registration, object safety (AFIT vs `async_trait`) |
| `poc/plugin-stdio` | JSON-RPC over stdio host + Rust SDK. Measures spawn/latency. Checks Windows: pipe deadlocks, `.cmd`/`npx` shims, kill on drop, stderr capture |
| `poc/plugin-stdio-node` | a third-party plugin in TS/JS against the same schema (authoring ease) |
| `poc/plugin-resolve` | PATH lookup + URL@sha256 download/cache, per-OS archive selection |
| `poc/plugin-wasm` (timeboxed) | wasmtime + WIT + wasi-http plugin calling the GitHub API. Measures binary size, build time and cold/warm start, to confirm deferral |

Sources: [WASI 0.3 launch](https://bytecodealliance.org/articles/WASI-0.3), [dprint plugins](https://dprint.dev/plugins/), [dprint plugin dev](https://dprint.dev/plugin-dev/), [dprint 0.55.0 release](https://github.com/dprint/dprint/releases/tag/0.55.0)

## Ticket candidates
- Plugin trait and Context: define `Plugin`, `StepSet`, `Context`, logger and secret-masked env in the core crate
- Built-in plugin features: a cargo feature per built-in plugin, plus embedder registration API on the builder
- Plugin protocol v1 spec: JSON-RPC methods, handshake, versioning rules, JSON Schema (`semoxide-plugin-protocol`)
- ProcessPlugin host: spawn, handshake, step dispatch, log forwarding, timeouts, Windows process handling
- Plugin SDK crate: `semoxide-plugin-sdk` stdio server wrapping `impl Plugin`
- Plugin resolution and install: built-in → PATH `semoxide-plugin-<x>` → URL@sha256 download cache
- PoC: plugin trait shape: async trait, object safety, context borrows
- PoC: stdio JSON-RPC host + Rust/Node plugins on Windows/Linux/macOS
- PoC: wasmtime WIT plugin cost (timeboxed): size/build/start numbers for the WASM deferral decision
- ADR: plugin mechanism: record the hybrid decision and why dylib/WASM were rejected
