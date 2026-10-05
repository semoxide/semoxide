# Requirements (secondary)

Hard requirements: [CLAUDE.md](../CLAUDE.md).

- The final plan takes the form of GitHub milestones, epics (sub-issues), issues, "blocked by" dependencies, labels and a Projects v2 board. The board spans repos so it can hold future plugin repos.
- The first milestone set is the research/doc/PoC phase itself. The product plan is formed through discussion, never in one pass.
- Project architecture and code architecture are separate docs.
- Logging/debugging is planned up front: a debug flag, tooling, and secret masking ([Observability](OBSERVABILITY.md)).
- Testing is planned up front: local repos, a CI sandbox repo, and dry runs ([Testing](TESTING.md)).
- Repo: private `sm-steel/semoxide`.
- A user documentation website is planned, and its tooling and hosting are decided in a later wave. It must document every intentional difference from semantic-release and the drawbacks of each opt-in behavior (e.g. [ADR 0009](decisions/0009-commit-back.md)).
