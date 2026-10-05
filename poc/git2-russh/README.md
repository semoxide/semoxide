# PoC: git2 without libssh2, with SSH through a custom transport (options f and b1)

Throwaway. git2 0.21 is built with `vendored-libgit2` + `https` and **no `ssh` feature**, so libssh2 is not built.
`git2::transport::register("ssh" | "ssh+git" | "git+ssh")` installs our `SmartSubtransport` (`rpc=false`). libgit2 also
routes scp-style `git@host:path` URLs to the `ssh` transport. The subtransport runs `git-upload-pack '<path>'` or
`git-receive-pack '<path>'` and pipes the channel to libgit2.

- **f (default):** `russh` 0.64.1 (`ring` backend, no aws-lc) plus `ssh2-config` 0.8. See `src/native.rs`.
- **b1 (opt-in):** spawns the system `ssh` with `-o BatchMode=yes -o ConnectTimeout=N`. Selected with `SEMOXIDE_SSH_BACKEND=exec`
  or `SshOptions::backend`. The command is `GIT_SSH_COMMAND`, then `GIT_SSH`, then `exec_command`, then `ssh`. See `src/exec.rs`.

`cargo test` runs the unit tests and `tests/local.rs`: a local **russh server** that runs the real `git upload-pack`/`receive-pack`.
It needs `git` and `ssh-keygen`. Each test generates throwaway keys in a TempDir, which is deleted afterwards.
`tests/github.rs` runs against `semoxide/semoxide-sandbox` through `.github/workflows/russh.yml` (code copied to `russh-poc/`).
Runs [37339331502](https://github.com/semoxide/semoxide-sandbox/actions/runs/37339331502) and **[37340728955](https://github.com/semoxide/semoxide-sandbox/actions/runs/37340728955)** (all jobs green).
Only `main` and `fixture-root` are left afterwards.

> **Blocker on GitHub:** the `semoxide` org has `deploy_keys_enabled_for_repositories: false` (the org was updated at 15:06Z,
> after the last green `remote-git` run). GitHub completes the handshake, accepts the deploy key, runs the exec, and then
> answers `ERROR: Repository not found.` So on GitHub everything up to the git service is proven, but **push, fetch and
> shallow clone could not run there**. Those are proven against the local git server on both OSes (same client code path).
> To finish the GitHub part, re-enable deploy keys for the org and re-dispatch `russh.yml`; the tests then run the full scenario automatically.
> We did not change the setting.

## Results (f = russh)

"Auth OK" means GitHub accepted the key and ran the exec, then gave the org-policy answer above. An unregistered key gets an `Auth` error instead.

| # | item | Linux | Windows | minimal container¹ | evidence / exact text |
|---|---|---|---|---|---|
| 1 | push lw tag, annotated tag, `refs/notes/semantic-release-*`, branch (one push each); ls-remote (tag `^{}` peeled); fetch; shallow `depth(1)` clone then unshallow; delete | ✅ local / ⛔ GitHub | ✅ local / ⛔ GitHub | ⛔ GitHub | Locally, per-ref status is `None` for every push, `1 -> 3 commits, is_shallow=false`, and ls-remote shows 7 refs. On GitHub: `github.com:22: remote git exited with 1: ERROR: Repository not found.` (policy) |
| 2 | keys: ed25519, ecdsa-p256, rsa (OpenSSH format), rsa PEM (PKCS#1), each from a **file** and from **memory**; also an encrypted ed25519 with a passphrase | ✅ | ✅ | ✅ (no PEM: no ssh-keygen) | Local server: all key forms authenticate. GitHub: all 8 deploy-key cases reach "Auth OK" (180–890 ms per connect). The libssh2/WinCNG failures (R7b, R7c) are gone. |
| 3 | agent | ✅ `SSH_AUTH_SOCK` (ssh-agent) | ✅ `\\.\pipe\openssh-ssh-agent` (the default when `SSH_AUTH_SOCK` is unset, or set explicitly) | n/a | russh has `connect_named_pipe` and `connect_pageant`. Pageant not running gives `tried: agent pageant: early eof`, a clear error with no hang. |
| 4 | host keys | ✅ | ✅ | ✅ | Matches known_hosts (plain, `[host]:port`, hashed `\|1\|`, wildcard/negation, `@revoked`), the embedded GitHub keys, and a known_hosts built live from `api.github.com/meta`. Each algorithm pinned alone works (the client offers only the known types). Unknown: `host key verification failed: github.com:22 is not in known_hosts (server key ssh-ed25519 SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU); add it to known_hosts or allow unknown hosts explicitly` (`class=Ssh code=Certificate`). `accept_unknown_hosts` lets it pass. Wrong key: `HOST KEY MISMATCH for github.com:22: server presented ssh-ed25519 SHA256:+DiY…, but wrong:1 lists a different key of that type; refusing to connect`, even with accept_unknown. |
| 5 | `~/.ssh/config` (ssh2-config) | ✅ | ✅ | ✅ | `Host sandbox-gh` → `HostName ssh.github.com`, `Port 443`, `User git`, `IdentityFile <ecdsa>` with the URL `sandbox-gh:org/repo.git` (GitHub: Auth OK; local server: full ls-remote). URL user/port take precedence over the config. Unknown fields are ignored. |
| 6a | bad key / no access | ✅ | ✅ | ✅ | Bad key: `ssh authentication failed for git@github.com:22; tried: key random-unregistered (ssh-ed25519 SHA256:…): rejected by server` (`code=Auth`). The list names every source tried, e.g. `agent: unavailable (…)`. No repo or no access: `github.com:22: remote git exited with 1: ERROR: Repository not found.` (the server's stderr plus the exit code). Local missing repo: `remote git exited with 128: fatal: '…/nope.git' does not appear to be a git repository`. Refused: `could not connect to 127.0.0.1:35395: Connection refused (os error 111)` |
| 6b | no hang (timeouts) | ✅ | ✅ | ✅ | Blackhole IP: `timed out after 3 s while connecting (TCP) (10.255.255.1:22)` after 3.0 s. Server with no banner: `timed out after 2 s while exchanging keys` after 2.0 s. Server that stalls after exec: `127.0.0.1:…: no data for 2 s` (`io_timeout`). Every phase is bounded by `connect_timeout` and every read/write by `io_timeout`. |
| 6c | handshake stability, 50 × connect + ls-remote | ✅ 50/50, avg 624 ms | ✅ 50/50, avg 175 ms | ✅ 50/50, avg 192 ms | Local server 50/50 on both OSes. libssh2/WinCNG was 59/60 with `Unable to exchange encryption keys` (R7e); no kex failure seen here. |
| 7 | async bridge | ✅ | ✅ | ✅ | Works from a plain thread, inside a multi-thread tokio runtime, from `spawn_blocking`, and inside a current-thread runtime (`bridge_from_inside_tokio`). |
| 8 | no external binary | | | ✅ | `debian:stable-slim` (trixie) with no `ssh`, `git`, `ssh-agent`, `ssh-keygen`, libssl or libssh. `ldd` shows only libc, libm and libgcc_s (Linux build uses `vendored-openssl` + `LIBZ_SYS_STATIC=1`). |

¹ The same Linux test binary, built on ubuntu-latest and run in the container.

## Build metrics

Stripped release binary of a minimal ls-remote program (`metrics/`, `metrics/measure.sh`), each built clean from a cold `target/`.
Crate count is unique packages in `cargo tree -e normal` for the host.

| | Windows (local, 1.99 MSVC) build / size / crates | Linux (ubuntu-latest) build / size / crates | OpenSSL on Linux |
|---|---|---|---|
| git2 https only (baseline) | 29 s / 1.58 MB / 39 | 54 s / 1.46 MB / 54 | yes (git2 `https`) |
| git2 + libssh2 (today) | 31 s / 1.74 MB / 40 | 59 s / 1.70 MB / 56 | yes |
| git2 + russh (f) | 56 s / 6.34 MB / 183 | 136 s / 5.72 MB / 242 | yes, **only** from git2 `https`. russh+ring adds no OpenSSL. Windows has none. |

- **Delta f vs libssh2:** about +4.0 to 4.6 MB binary, +143 to 186 crates, and about 2× the clean build time.
  The weight comes from tokio (multi-thread), and from russh's full algorithm set (p256/p384/p521, ml-kem, rsa, curve25519/ed25519-dalek, aes/chacha) plus RustCrypto bigint.
  Trimming algorithms isn't possible through russh features today.
- CI: the full test build (release) took 274 s on Linux (with vendored OpenSSL) and 363 s on Windows. Uncached; rust-cache was not saved on run 1.

## Async bridge (`src/bridge.rs`)

- One lazily started process-wide tokio multi-thread runtime with 2 workers (`git2-russh`). It owns the TCP sockets, the russh
  session task, and a per-channel **pump** task: `Data` goes into a bounded mpsc(32) queue for backpressure, and stderr and `exit-status` go into shared state.
- git2 calls `read`/`write` synchronously on the caller's thread. Each call enters the runtime context (`RT.enter()`) and polls
  `tokio::time::timeout(io_timeout, …)` with `futures::executor::block_on`. Unlike `Runtime::block_on`, this does **not** panic when
  the caller is already inside a tokio runtime (tested). `Drop` closes the channel and session in the background with a 5 s cap, and never blocks libgit2's free.
- **Cost:** 2 idle threads after the first SSH use, plus one mpsc hop and one `Bytes` copy per chunk. Throughput, cloning one 64 MiB incompressible blob from the local russh server
  on Windows (release): russh client **5.2 MiB/s**, system OpenSSH client **5.0 MiB/s** (`file://` gives 26 MiB/s). The bridge isn't the bottleneck there; the test server is.
- **Risks:**
  - Calling from a tokio worker blocks that worker for the length of the git op, which matches any sync git2 call. Use `spawn_blocking` in an async product.
  - `transport::register` is `unsafe` and process-global; it's guarded by a `OnceLock`.
  - Options reach the transport through a thread-local (`with_options`), because git2's `RemoteCallbacks` (credentials, `certificate_check`) are **not** called for a custom transport.
    Auth and host-key policy therefore live in our own `SshOptions`, not in callbacks.
  - russh is pre-1.0 (0.x) and its API churns. Host certificates (`@cert-authority`), ProxyJump/ProxyCommand, and FIDO/sk keys
    (agent-backed sk keys are untested) are not implemented.

## b1 (system `ssh`)

| check | Linux | Windows | text |
|---|---|---|---|
| ops scenario (push, fetch, ls-remote, shallow) | ✅ local / ⛔ GitHub (policy) | ✅ local (System32 `ssh.exe`) / ⛔ GitHub | Locally, the same 4 pushes + ls-remote + fetch + delete pass. GitHub: `` `ssh` failed (exit status: 1): ERROR: Repository not found. `` |
| key file via `exec_command`, agent, `GIT_SSH_COMMAND='ssh -F cfg'` + Host alias (ssh.github.com:443) | ✅ Auth OK | ✅ Auth OK | |
| unknown host, BatchMode | ✅ | ✅ | `` `ssh` failed (exit status: 255): Host key verification failed. `` No prompt and no hang. |
| bad key | ✅ | ✅ | Windows: `` Load key "…": invalid format \| git@github.com: Permission denied (publickey). `` Linux: OpenSSH's `UNPROTECTED PRIVATE KEY FILE` banner. ssh's stderr is passed through verbatim, so the text is noisy. |
| 10 × connect | ✅ 10/10, avg 804 ms | ✅ 10/10, avg 312 ms | Slower than russh because it spawns a process per connection. |
| no `ssh` (minimal container) | ✅ | n/a | `` ssh executable `ssh` not found (exec backend selected via SEMOXIDE_SSH_BACKEND=exec or GIT_SSH/GIT_SSH_COMMAND); install an OpenSSH client or use the built-in backend `` |

## Verdict

- **f holds.** Every SSH problem in R7 is gone on Windows: all key formats from file and memory, no memory-key hang, and no kex flake in 50+50+50 GitHub
  connects plus 50 local connects per OS. On top of that it gives known_hosts verification, `~/.ssh/config`, both agents, bounded timeouts with phase-named
  errors, and runs with **no external binary**.
  - The price is about +4.5 MB binary, about +150–190 crates, about 2× clean build time, a tokio runtime, and owning about 1,100 LOC of transport and policy code (src/, including unit tests) that libssh2 used to cover.
  - The **open item**: real-GitHub push, fetch and shallow over SSH are blocked by the org's deploy-key policy. They are proven locally against real `git` server binaries.
- **f + b1 is cheap and worth keeping as the escape hatch,** the same pattern as cargo's `git-fetch-with-cli`.
  - It's about 200 LOC, gives full OpenSSH semantics (config, agent, FIDO, ProxyJump, `GIT_SSH_COMMAND`), and BatchMode makes it non-interactive.
  - Failures are OpenSSH's stderr verbatim, and a missing `ssh` gives a clear error.
  - Recommend f as the default and b1 opt-in via `SEMOXIDE_SSH_BACKEND=exec` (plus `GIT_SSH_COMMAND`/`GIT_SSH`).
  - Drop the git2 `ssh` feature, and with it libssh2.
