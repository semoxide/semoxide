# `branch::invalid_prerelease_id`

A prerelease branch gives an invalid prerelease identifier.

## When it happens

- a glob rule with `prerelease = true` matches a branch whose name isn't a valid identifier, e.g. `{ name = "pre/*", prerelease = true }` and the branch `pre/a`
- a `{name}` template gives an invalid identifier once the branch name is filled in

## How to fix it

Set `prerelease` on the rule to a valid identifier: letters, digits, `-` and `.`, no leading zeros ([CONFIG §3](../../CONFIG.md#3-branches)).

## semantic-release

Replaces `EPRERELEASEBRANCH`.

## Related

- [`branch::duplicate_prerelease_ids`](duplicate-prerelease-ids.md)
- [`config::invalid_value`](../config/invalid-value.md), for a literal name, checked when the config loads
