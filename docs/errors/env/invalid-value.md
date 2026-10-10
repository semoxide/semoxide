# `env::invalid_value`

An environment variable semoxide reads has a value it doesn't accept.

## When it happens

- `SEMOXIDE_CI_IS_PR` is set to something other than `true` or `false` (any letter case)

## How to fix it

Set the variable to one of the values the message lists, or unset it. Accepted values: [CLI § Environment variables](../../CLI.md#environment-variables).

## semantic-release

No equivalent: semantic-release has no such override.

## Related

- [`env::not_unicode`](not-unicode.md)
