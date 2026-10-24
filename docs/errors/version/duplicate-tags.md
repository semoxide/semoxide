# `version::duplicate_tags`

Tags of the last release's version point at different commits, so it's unclear where the last release is.

## When it happens

- `v1.2.0` and `v1.2.0+b` are on different commits and 1.2.0 is the branch's highest release (build metadata doesn't make a different version)

Duplicates of older versions, and of prereleases, don't matter.

## How to fix it

Delete the tags on the wrong commits, so one commit holds the version.

## semantic-release

No equivalent: the first tag by name wins, so the commits after it can be released again.

## Related

- [`version::out_of_range`](out-of-range.md)
