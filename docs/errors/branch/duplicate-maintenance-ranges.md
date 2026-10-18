# `branch::duplicate_maintenance_ranges`

Two maintenance branches maintain the same range.

## When it happens

- the branches `1.x` and `1.x.x`, or `1.x` and `1.X`, both exist: each maintains `1.x`
- a named maintenance branch with `range = "1.x"` and a `1.x` branch both exist

## How to fix it

Keep one maintenance branch per range.

## semantic-release

Replaces `EMAINTENANCEBRANCHES`.

## Related

- [`branch::not_yet_supported`](not-yet-supported.md)
