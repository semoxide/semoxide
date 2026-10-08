# `config::invalid_toml`

The config file isn't valid TOML.

## When it happens

`semoxide.toml` (or `.config/semoxide.toml`) has a TOML syntax error: an unclosed string or bracket, a missing `=`, a duplicate key. The message names the file, line and column.

## How to fix it

Fix the syntax at the reported position. Any TOML-aware editor (with the JSON Schema from `semoxide schema`) highlights it.

## semantic-release

No equivalent (cosmiconfig reports parse errors of `.releaserc` as plain exceptions).

## Related

- [`config::unreadable`](unreadable.md)
- [`config::invalid_value`](invalid-value.md)
