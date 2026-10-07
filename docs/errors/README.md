# Error messages and pages

How semoxide errors are worded, and the template for each code's page. Codes, the registry and the docs-page check: [OBSERVABILITY §8](../OBSERVABILITY.md#8-errors).

## Message (`Display`)

What went wrong, following the Rust API guidelines ([C-GOOD-ERR](https://rust-lang.github.io/api-guidelines/interoperability.html#error-types-are-meaningful-and-well-behaved-c-good-err)):

| Rule | Example |
| --- | --- |
| lowercase, no trailing punctuation, concise, one line | `not a git repository` |
| names the bad input in backticks | ``invalid error code `Core::bad`: expected `<namespace>::<name>` `` |
| says what is wrong, not how to fix it (that is the help line) | not ``not a git repository, run `git init` `` |
| no `error:` or `failed to` prefix: the renderer shows the code | ``push of `v1.2.0` rejected by `origin` `` |

## Help line (`help()`)

How to fix it:

| Rule | Example |
| --- | --- |
| one sentence, imperative, starts with a verb, ends with a period | ``Set `GITHUB_TOKEN`, or run `semoxide doctor --online`.`` |
| commands and values in backticks | |
| omitted when there is nothing actionable to say | |

## Page template

One page per code at `docs/errors/<slug>.md`, e.g. `core/no-git-repo.md` for `core::no_git_repo`:

```markdown
# `core::no_git_repo`

Not a git repository.

## When it happens

## How to fix it

## semantic-release

Replaces `ENOGITREPO`.

## Related
```

The summary line restates the message as a sentence. The `semantic-release` section names the upstream error code this one replaces, or says "No equivalent.", so people migrating can search for the code they know.
