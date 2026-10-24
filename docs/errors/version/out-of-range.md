# `version::out_of_range`

The next version is outside the range of the branch being released.

## When it happens

- `main` has 1.0.0 and `next` has 1.1.0, so `main` releases `>=1.0.0 <1.1.0`, and a feature on `main` would give 1.1.0
- the range is empty, e.g. `main` has no release yet while `next` has 1.0.0 (`>=1.0.0 <1.0.0`)

The help names another release branch whose range contains the version, if there is one.

## How to fix it

Release from the branch the help names, or merge the higher branch into this one so its range moves up ([SEMANTIC-RELEASE-SPEC §4.6](../../specs/SEMANTIC-RELEASE-SPEC.md#46-range-calculation-src)).

## semantic-release

Replaces `EINVALIDNEXTVERSION`.

## Related

- [`version::duplicate_tags`](duplicate-tags.md)
- [`version::overflow`](overflow.md)
