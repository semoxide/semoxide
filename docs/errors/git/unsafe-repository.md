# `git::unsafe_repository`

The repository is owned by another user and git's `safe.directory` doesn't list it.

## When it happens

- in a container or CI job whose user differs from the owner of the checkout; git refuses the same repository with "detected dubious ownership"

## How to fix it

If you trust the repository, run `git config --global --add safe.directory <path>` with the repository's path, as for git itself.

## semantic-release

No equivalent: git's "dubious ownership" error surfaces as a failed git command.

## Related

- [git `safe.directory`](https://git-scm.com/docs/git-config#Documentation/git-config.txt-safedirectory)
