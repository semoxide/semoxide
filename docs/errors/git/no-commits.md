# `git::no_commits`

The repository has no commits yet.

## When it happens

- semoxide runs in a repository just created with `git init`: `HEAD` points at a branch that doesn't exist until the first commit

## How to fix it

Make a first commit, then run semoxide again.

## semantic-release

No equivalent: git's own error (`ambiguous argument 'HEAD'`) surfaces.

## Related

- [`git::no_repository`](no-repository.md)
