# `git::bare_repository`

The repository semoxide found has no working tree.

## When it happens

- semoxide runs in a bare repository (a mirror or a server-side repository): a release needs a working tree for plugins that change files

## How to fix it

Run semoxide in a clone with a working tree.

## semantic-release

No equivalent.

## Related

- [`git::no_repository`](no-repository.md)
