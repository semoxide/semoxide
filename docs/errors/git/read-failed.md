# `git::read_failed`

libgit2 failed to read the repository; the message carries libgit2's own text.

## When it happens

- the repository is corrupt, incomplete or unreadable (permissions, a broken file system)

## How to fix it

Check the repository with `git fsck`, or clone it again.

## semantic-release

No equivalent: failing git commands surface as their own output.

## Related

- [`git::no_repository`](no-repository.md)
