---
name: rust-async
description: Use when adding concurrency, parallelism, timeouts, cancellation, tokio, async/await, spawned tasks or threads anywhere in semoxide, or when something "is slow because calls run one after another"
---

# Async and concurrency in semoxide

## Overview
semoxide's core is **sync**. Async exists in exactly two places: the plugin host module in `semoxide-runtime` (gRPC/tonic) and the SSH transport bridge inside `semoxide-git` (russh). The façade offers an async `run()` that wraps the sync core on its own thread. Nothing else is async.

## Rules
| Where | Allowed |
|---|---|
| `semoxide-error`, `-schema`, `-engine` | no tokio, no async, no threads |
| `semoxide-git` | sync public API only. tokio stays private inside the SSH bridge |
| `semoxide-runtime` | sync, except the plugin host module (private runtime on its own thread, blocking calls outward) |
| `semoxide` façade | blocking `run()` + async `run()` only |

A public `async fn`, or a public type exposing tokio (`Arc` needed for `spawn_blocking`, `JoinSet`…), outside the façade is wrong. Redesign instead.

## Making slow remote work faster
Release steps write to remotes, and **order and partial-failure state matter more than speed**.
1. **Batch, don't parallelise:** several tags or refs go in **one push with several refspecs** (git2 `Remote::push(&[refspecs])`); per-ref status comes back through the callback. One connection, one auth, known state.
2. Read-only work that is genuinely slow (e.g. large log walks) may use `std::thread::scope`, which joins every thread before returning.
3. Remote writes are never run concurrently unless the step's spec says so.

## Timeouts and cancellation
- **No work may outlive the call.** A timeout or error must not leave pushes, publishes or threads running in the background: join, cancel or kill everything before returning. "Can't be cancelled, it finishes in the background" is a bug for a release tool.
- Timeouts use the operation's own deadline (git2 / gRPC deadlines, [ARCHITECTURE §5](../../../docs/ARCHITECTURE.md)), or cooperative checks between steps (CODE-ARCHITECTURE §5).
- `remote_writes_happened` must be true if **any** write may have reached the remote before the failure.

| Excuse | Reality |
|---|---|
| "tokio is an approved workspace dependency" | Approved for the plugin host and SSH bridge, not for every crate. |
| "spawn_blocking keeps git2 sync" | It still makes the public API async and leaves work running after a timeout. Batch the push instead. |
| "concurrency is just faster" | For remote writes, it makes partial failure unknowable. |
