# PoC

Throwaway proof-of-concept crates. Not product code.

- [git2-ops](git2-ops/README.md): every semantic-release git op (G1–G22) on git2. All pass locally; real-remote checks need the sandbox
- [plugin-protocol](plugin-protocol/README.md): **superseded** by ADR 0010. Old stdio/JSON hybrid PoC; kept for its lessons
- [plugin-grpc](plugin-grpc/README.md): ADR 0010 design (gRPC over named pipe / Unix socket, host services, process-tree kill, conformance). Works on Windows and Linux
- [sort-order](sort-order/README.md): byte vs case-insensitive vs `icu_collator` against JS `localeCompare` (ICU matches exactly, +1.1 MiB)
- [git2-russh](git2-russh/README.md): SSH via pure-Rust russh (default) or the system `ssh` (opt-in) as git2 transports; no libssh2. All key types on Windows, no external binary needed
