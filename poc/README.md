# PoC

Throwaway proof-of-concept crates. Not product code.

- [git2-ops](git2-ops/README.md): every semantic-release git op (G1–G22) on git2. All pass locally; real-remote checks need the sandbox
- [plugin-protocol](plugin-protocol/README.md): **superseded** by ADR 0010. Old stdio/JSON hybrid PoC; kept for its lessons
- [plugin-grpc](plugin-grpc/README.md): ADR 0010 design (gRPC over named pipe / Unix socket, host services, process-tree kill, conformance). Works on Windows and Linux
