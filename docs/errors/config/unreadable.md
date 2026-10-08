# `config::unreadable`

The config file exists but can't be read.

## When it happens

- the process lacks permission to read `semoxide.toml`
- the file isn't UTF-8 text
- `semoxide.toml` is a directory

## How to fix it

Make the file readable UTF-8 text, or remove it. The message names the path and the operating system's error.

## semantic-release

No equivalent.

## Related

- [`config::invalid_toml`](invalid-toml.md)
