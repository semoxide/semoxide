# `version::tags_not_found`

No tag matches `tags.format`, but some tags look like releases in another format, so the first release would repeat one.

## When it happens

- `tags.format = "v{version}"` and the branch has `1.0.0` and `1.1.0`, e.g. after a migration
- `tags.format = "{version}"` and the branch has `v1.0.0`

Only bare and `v`-prefixed versions count, prereleases included. Any tag matching the format, a prerelease too, turns the check off.

## How to fix it

Set `tags.format` to the format the help names, or delete or rename the tags if they aren't releases.

## semantic-release

No equivalent: tags that don't match are ignored, and the first release is made again.

## Related

- [`version::duplicate_tags`](duplicate-tags.md)
