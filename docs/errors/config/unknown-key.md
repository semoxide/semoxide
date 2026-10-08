# `config::unknown_key`

A key no config domain defines.

## When it happens

`semoxide.toml` has a key or table semoxide doesn't know, for example `tags.prefix` (a typo or a key from another tool), a step name like `steps.deploy`, or an unknown key in a branch rule. Keys inside `[plugins.<name>]` other than `version`, `timeouts` and `show_output` are the plugin's own options and are checked by the plugin instead.

## How to fix it

Correct or remove the key at the path in the message. The domains and their keys are listed in [CONFIG.md](../../CONFIG.md).

## semantic-release

No equivalent.

## Related

- [`config::invalid_value`](invalid-value.md)
