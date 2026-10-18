# `branch::not_yet_supported`

The release runs on a prerelease or maintenance branch, which this version of semoxide can't release from yet.

## When it happens

- semoxide runs on a branch that a prerelease rule (`{ name = "beta", prerelease = true }`) or a maintenance rule (`{ maintenance = "N.x" }`) claims

## How to fix it

Release from a release branch, such as the main line. A release from a release branch still checks every existing branch, so prerelease and maintenance rules can stay in `branches.rules`.

## semantic-release

No equivalent: semantic-release releases from prerelease and maintenance branches.

## Related

- [`branch::release_branches`](release-branches.md)
