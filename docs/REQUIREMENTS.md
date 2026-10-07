# Requirements (secondary)

Hard requirements: [CLAUDE.md](../CLAUDE.md). Reasons: [DECISIONS](DECISIONS.md).

## Compatibility
- **Familiar, not compatible:** semoxide uses semantic-release's concepts and step names, but its own config and plugin protocol. Intentional differences: [DIFFERENCES](DIFFERENCES.md).
- `semoxide migrate` converts `.releaserc` to `semoxide.toml` where it can and reports anything it can't convert ([CLI](CLI.md)).
- No JS plugin bridge.

## Project
- Project architecture ([ARCHITECTURE](ARCHITECTURE.md)) and code architecture ([CODE-ARCHITECTURE](CODE-ARCHITECTURE.md)) are separate docs.
- Logging/debugging is planned up front: a debug flag, tooling, and secret masking ([OBSERVABILITY](OBSERVABILITY.md)).
- Testing is planned up front: local repos, a CI sandbox repo, and dry runs ([TESTING](TESTING.md)).
- The plan lives on GitHub (milestones, epics, issues, dependencies, labels, an org-level board, [PLANNING](PLANNING.md)). It is formed through discussion, never in one pass.

## GitHub
- All repos live in the `semoxide` org (Free plan; owner `sm-steel`).
- `semoxide`, the protocol repo, the plugin repos and `semoxide-poc` are **public**; `semoxide-sandbox` stays **private**.
- Every proof of concept lives in `semoxide/semoxide-poc`, one directory per PoC, never in the product repos.

## User docs site
- Built once semoxide reaches a beta pre-release; its tooling and hosting are chosen then.
- It documents every intentional difference from semantic-release ([DIFFERENCES](DIFFERENCES.md)) and the drawbacks of each opt-in behaviour (e.g. commit-back, [ARCHITECTURE](ARCHITECTURE.md)).

## License
- `MIT OR Apache-2.0` for every semoxide repo (core, protocol, plugins, PoCs): `LICENSE-MIT`, `LICENSE-APACHE`, and `license = "MIT OR Apache-2.0"` in every `Cargo.toml`.
- Upstream material copied into a repo (MIT/ISC; e.g. notes templates; tests are not copied):
  - a header in each copied file: `Copied from <repo>@<sha>/<path>, <license>, (c) <holder>`
  - the full upstream license texts and copyright lines in `THIRD_PARTY_LICENSES.md`, also shipped with release artifacts when templates are embedded in the binary.
- Condensed spec text (SemVer, Conventional Commits; CC BY 3.0): each `docs/specs/*.md` names the source, the author, the license (with link) and states "condensed and modified".
- The README says semoxide is inspired by, and not affiliated with or endorsed by, semantic-release. No `semantic-release*` names or logo.
