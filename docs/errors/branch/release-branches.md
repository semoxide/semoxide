# `branch::release_branches`

The existing branches give no release branch, or more than 3.

## When it happens

- no release rule in `branches.rules` matches an existing branch, e.g. only `beta` exists under the default rules
- more than 3 existing branches are release branches, e.g. `master`, `main`, `next` and `next-major` under the default rules

## How to fix it

Keep 1 to 3 release branches: create the branch a release rule names, add a rule for an existing branch, or remove release rules or turn them into prerelease rules ([CONFIG §3](../../CONFIG.md#3-branches)).

## semantic-release

Replaces `ERELEASEBRANCHES`.

## Related

- [`branch::not_yet_supported`](not-yet-supported.md)
