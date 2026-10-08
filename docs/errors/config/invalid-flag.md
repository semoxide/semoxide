# `config::invalid_flag`

A `--set` flag that can't be read as `<key>=<value>`.

## When it happens

- no `=` (`--set tags.format`)
- an empty or malformed key (`--set =x`, `--set tags..format=x`)
- an array index in the key (`--set 'branches.rules[0].channel=next'`); arrays are set whole
- a value that starts like a TOML string, array or table but isn't valid TOML (`--set 'steps.plugins=["git"'`)

A well-formed flag with a wrong key or value fails like the same key in `semoxide.toml` (`config::unknown_key`, `config::invalid_value`).

## How to fix it

Write the flag as `--set <dotted.key>=<value>` ([CLI.md](../../CLI.md)). Set a whole array at once: `--set 'branches.rules=["main", "next"]'`.

## semantic-release

No equivalent.

## Related

- [`config::invalid_value`](invalid-value.md)
- [`config::unknown_key`](unknown-key.md)
