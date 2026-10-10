# `env::not_unicode`

An environment variable semoxide reads as text has a value that isn't valid UTF-8.

## When it happens

- a variable such as `SEMOXIDE_CI_BRANCH` or a CI variable (`GITHUB_REF`) was set from bytes that aren't UTF-8

## How to fix it

Set the variable to UTF-8 text. The message names the variable.

## semantic-release

No equivalent: Node.js decodes the environment lossily.

## Related

- [CLI § Environment variables](../../CLI.md#environment-variables)
