# 0013 Observability
Status: accepted (2026-10-05). Decided step by step; details and remaining proposals are in [OBSERVABILITY.md](../OBSERVABILITY.md).

## Decided
- **The library never prints.** It only emits `tracing` events. It never sets up a log output and never writes to stdout or stderr. The CLI, or an embedding program, decides where logs go and how they look.
- **Masking happens at the source, plus an output pass.** The core and the plugin host replace secret values before an event is created. The CLI's output writer masks again as a second pass, and embedders can use that writer too.
