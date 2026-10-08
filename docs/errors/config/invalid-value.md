# `config::invalid_value`

A config value of the wrong type or format.

## When it happens

A key exists but its value is not accepted, for example:

- `version.initial = "1.0"` (not a full SemVer version)
- `tags.format` without exactly one `{version}`
- a branch rule with `prerelease = false`, an invalid prerelease identifier, `channel = true` or `""`, a maintenance pattern without `N.x`/`N.x.x`/`N.N.x` and no `range`, or an invalid `range`
- an upstream-style maintenance entry (`"1.x"`, or `range` on a `name` entry): write it as `{ maintenance = … }`
- an invalid or repeated plugin name in `steps.plugins`
- a timeout that isn't a number followed by `s`, `m` or `h`
- a TOML date-time, `nan` or `inf` inside a plugin's options (quote dates as strings)

## How to fix it

Change the value at the key path in the message. Each key's accepted values are in [CONFIG.md](../../CONFIG.md).

## semantic-release

Replaces `ETAGNOVERSION`, and the value checks of `EINVALIDBRANCH`, `EMAINTENANCEBRANCH` and `EPRERELEASEBRANCH`.

## Related

- [`config::conflicting_keys`](conflicting-keys.md)
- [`config::unknown_key`](unknown-key.md)
