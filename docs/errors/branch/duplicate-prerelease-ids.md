# `branch::duplicate_prerelease_ids`

Two prerelease branches have the same prerelease identifier.

## When it happens

- two rules set the same `prerelease` string, e.g. `"rc"`, and both branches exist
- a glob rule with a fixed `prerelease` string matches more than one branch

## How to fix it

Give each prerelease branch its own identifier with `prerelease`, e.g. `prerelease = "{name}"` on a glob rule ([CONFIG §3](../../CONFIG.md#3-branches)).

## semantic-release

Replaces `EPRERELEASEBRANCHES`.

## Related

- [`branch::invalid_prerelease_id`](invalid-prerelease-id.md)
