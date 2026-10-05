# 0013 Observability
Status: accepted (2026-10-05). Decided step by step; details and remaining proposals are in [OBSERVABILITY.md](../OBSERVABILITY.md).

## Decided
- **The library never prints.** It only emits `tracing` events. It never sets up a log output and never writes to stdout or stderr. The CLI, or an embedding program, decides where logs go and how they look.
