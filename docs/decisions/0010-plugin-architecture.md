# 0010 Plugin architecture
Status: accepted in part (2026-10-05). Decided step by step; open points are listed below.

## Decided
- **Separate repos:** each official plugin lives in its own repo with its own version and release cycle.
- **Not stdio:** the protocol runs over a local socket (transport details still open), not over the plugin's stdin/stdout.
- **Own protocol repo:** the protocol has its own repo and its own version, independent of semoxide's version. This avoids Nushell's lockstep pain ([nushell research](../research/nushell-plugins.md)).

## Open
Discussed one at a time; each answer is added to the Decided list above.
