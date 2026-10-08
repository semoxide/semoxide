# `config::conflicting_keys`

Config keys that can't appear together.

## When it happens

A branch rule mixes two kinds:

- `name` together with `maintenance`
- `prerelease` on a maintenance rule
- `range` with a maintenance pattern that already ends in `N.x`, `N.x.x` or `N.N.x`

The key path points at the extra key: `maintenance` next to `name`, or `prerelease` or `range` on a maintenance rule.

## How to fix it

Keep one kind per rule: `"main"` or `{ name = … }` for release branches, `{ name = …, prerelease = … }` for prerelease branches, `{ maintenance = … }` for maintenance branches ([CONFIG.md](../../CONFIG.md#3-branches)).

## semantic-release

No equivalent.

## Related

- [`config::invalid_value`](invalid-value.md)
