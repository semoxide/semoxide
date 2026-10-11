# `git::no_remote`

The remote named by `git.remote` doesn't exist.

## When it happens

- the repository has no `origin` (the default) and `git.remote` isn't set
- `git.remote` names a remote that isn't configured, or isn't a valid remote name

## How to fix it

Add the remote with `git remote add <name> <url>`, or set `git.remote` to an existing remote or `git.url` to the URL to publish to ([CONFIG](../../CONFIG.md)).

## semantic-release

Replaces `ENOREPOURL`.

## Related

- [`git::no_repository`](no-repository.md)
