# `config::plugin_not_enabled`

Settings for a plugin that isn't in `steps.plugins`.

## When it happens

`[plugins.<name>]` or a `steps.<step>.order` names a plugin missing from `steps.plugins`, so it would never run.

## How to fix it

Add the plugin to `steps.plugins`, or remove its settings ([CONFIG.md](../../CONFIG.md#9-plugins-and-steps)).

## semantic-release

No equivalent.

## Related

- [`config::unknown_key`](unknown-key.md)
