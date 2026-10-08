# `config::unsupported_section`

A section semoxide knows but doesn't support yet.

## When it happens

`semoxide.toml` sets a section that is planned but not implemented in this version:

- `config.extends`
- `[packages.<name>]` (monorepo release units)

## How to fix it

Remove the section. The key path in the message names it.

## semantic-release

No equivalent.

## Related

- [`config::unknown_key`](unknown-key.md)
