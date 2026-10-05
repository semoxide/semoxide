# ssh-cost: marginal cost of SSH on top of the real baseline

`poc/git2-russh` measured russh against a bare git2 binary, so tokio was charged to russh. This crate measures each SSH option
on top of what semoxide already ships: tokio (rt-multi-thread, net, io-util, time, sync, process, macros), a tonic 0.14 + prost
gRPC server and client (protox/tonic-prost-build codegen, unary and server-streaming), tracing + tracing-subscriber, a
`tokio::process` call, and a git2 HTTPS ls-remote. All of it runs, so nothing is optimized away.

| variant | features | code |
|---|---|---|
| **A** `libssh2` | git2 `vendored-libgit2, https, ssh` | baseline + an agent credential callback |
| **B** `russh` | git2 `vendored-libgit2, https` + russh 0.64.1 (`flate2, ring, rsa`) + ssh2-config | baseline + `src/russh_transport.rs` (263 LoC: a simplified copy of git2-russh with ssh config, known_hosts, agent and key-file auth, an exec pump, and a sync bridge) |
| **C** `https-only` | git2 `vendored-libgit2, https` | baseline (the floor) |

Run `./measure.sh [cargo args]`. It runs `cargo fetch`, then builds each variant clean in its own `target-<v>/` dir, 2 times (`RUNS`).
Sizes are of the stripped release binary. MB = 10^6 bytes. "crates" counts the unique packages in `cargo tree -e normal --prefix none`.

## Windows: local, rustc 1.99 MSVC, median of 2

| | build | size | crates | Δ vs C |
|---|---|---|---|---|
| C https-only | 44.5 s | 4.33 MB | 105 | |
| A libssh2 | 45.5 s | 4.46 MB | 106 | +1 s, **+0.14 MB**, +1 crate |
| B russh | 65.5 s | 8.44 MB | 229 | +21 s (+47%), **+4.12 MB**, +124 crates |

## Linux: moscow_temp, `rust:1` (1.99) in Docker, 1 vCPU / 1 GB, median of 2

Absolute times are slow because the host has 1 vCPU. Compare the ratios, not the seconds.

| | build | size | crates | Δ vs C |
|---|---|---|---|---|
| C https-only | 578 s | 4.08 MB | 107 | |
| A libssh2 | 579 s | 4.29 MB | 108 | ±0 s, **+0.22 MB**, +1 crate |
| B russh | 882 s | 7.74 MB | 221 | +303 s (+52%), **+3.67 MB**, +114 crates |

**Static musl + `vendored-openssl`** (helsinki_temp, 1 vCPU, 1 run each): C 1164 s / 9.79 MB, A 1200 s / 9.99 MB (+0.20 MB),
B 1557 s / 13.45 MB (+3.66 MB, +34%). All three binaries are `static-pie` and run the HTTPS ls-remote.

- **ldd (glibc):** all three binaries show the same list: `libssl.so.3 libcrypto.so.3 libz.so.1 libzstd.so.1 libgcc_s libm libc`.
  - A has no `libssh2.so`: libssh2-sys builds the bundled libssh2 statically, even though the image has `libssh2-1-dev`.
  - `libzstd` comes in through Debian's libcrypto.
- **OpenSSL:** it's pulled in on Linux in **every** variant by git2 `https` (`git2 → libgit2-sys → openssl-sys`). A adds a second
  consumer (`libssh2-sys → openssl-sys`). B adds **no** OpenSSL: russh uses ring plus RustCrypto. Windows has no OpenSSL in any
  variant (WinHTTP/SChannel). So dropping libssh2 does not remove OpenSSL. Only replacing git2's HTTPS backend would.
- Smoke runs: all variants list 1112 refs over HTTPS on both OSes. B over `git@github.com:` reaches GitHub and gets through the handshake and
  known_hosts check, then fails at auth as expected (Windows had no key loaded; Linux reports `Unknown server key` with an empty known_hosts). A over SSH on Windows hung
  until the 60 s kill (the libssh2 agent path, the same R7 behaviour).

## What B adds over A besides size

- **Code we own:** about 1,040 non-test LoC (git2-russh `src/` without `exec.rs`, or 1,253 with the b1 exec fallback). It covers known_hosts, ssh config, the
  agent, Pageant and named-pipe agents, auth ordering, the sync↔async bridge, timeouts and error text. With A, libssh2 covers all of this.
- **Pre-1.0 and RC dependencies:** russh 0.64 (the API churns between minors), plus `ssh-key 0.7.0-rc.11`, `rsa 0.10.0-rc.18` and `pkcs1 0.8.0-rc.4`, and the
  0.x RustCrypto stack (crypto-bigint, elliptic-curve, ml-kem, p256/p384/p521). That's about 115–125 more crates for cargo-deny/vet.
- **A second crypto stack** (ring + RustCrypto) next to OpenSSL/SChannel, a global `unsafe` transport registration, and 2 extra
  runtime threads (or reuse the product's runtime).
- **What you get for it:** none of libssh2's Windows failures (memory-key hang, kex flakes), modern algorithms (ed25519, ECDSA,
  rsa-sha2, mlkem768x25519 PQ kex) with keys in every format, Windows OpenSSH-agent and Pageant support, real known_hosts
  verification and `~/.ssh/config`, and phase-named timeouts.
  - Still missing: `@cert-authority`, ProxyJump/ProxyCommand, and FIDO keys outside an agent.

## Conclusion

1. With tokio and tonic already in the binary, libssh2 costs ~nothing (+0.14–0.22 MB, +1 crate, no build time). russh still costs **+3.7–4.1 MB, +114–124 crates and ~+50% clean build time**.
2. tokio was not the main cost. The weight is russh's algorithm suite (RustCrypto bigint/ECC/PQ, ring, ssh-key) plus our transport, and russh features can't trim it today.
3. OpenSSL stays on Linux either way (git2 `https`). The case for B is correctness and features on Windows, not footprint, and it has to justify ~4 MB, ~1k LoC of our own code and an RC-heavy dependency tree.
