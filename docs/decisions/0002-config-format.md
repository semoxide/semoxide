# 0002 Config format
Status: accepted (2026-10-05)

- TOML only: `semoxide.toml`, with `.config/semoxide.toml` as a fallback.
- Layers, lowest to highest precedence: defaults, `extends`, file, `SEMOXIDE_*` env vars, CLI flags.
- A JSON schema is generated with schemars.

Why: `serde_yaml` is deprecated. Details are in [distribution-config §2](../research/distribution-config.md).
