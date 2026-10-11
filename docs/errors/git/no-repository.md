# `git::no_repository`

There is no git repository at or above the directory semoxide runs in.

## When it happens

- semoxide runs outside a clone, or `--cwd` points outside one
- the repository is selected with `GIT_DIR`: semoxide never reads the `GIT_*` variables ([DIFFERENCES](../../DIFFERENCES.md))

## How to fix it

Run semoxide in a clone, or point `--cwd` at one ([CLI § Global flags](../../CLI.md#global-flags)).

## semantic-release

Replaces `ENOGITREPO`.

## Related

- [`git::bare_repository`](bare-repository.md)
