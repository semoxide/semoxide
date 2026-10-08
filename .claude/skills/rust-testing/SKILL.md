---
name: rust-testing
description: Use when writing, changing or fixing tests in semoxide, implementing pure-core logic (versions, bumps, branches, channels, commit parsing, config merge), or when a snapshot (insta) changes or a test fails
---

# Rust testing in semoxide

## Overview

Tests are the human-approved spec, and **only a human approves them**. How much ceremony that takes depends on the area:

| Area | Approach |
| --- | --- |
| Pure core: versions, bumps, branches, channels, commit parser, config merge | the strict sequence below: tests first, then **stop** for review |
| Notes, templates, rendering | write the code and tests together; outputs are insta snapshots that stay pending (`.snap.new`) for the human |
| CLI commands, JSON output | an `assert_cmd` case first, then the code; output snapshots stay pending |
| git, plugin host, forge, I/O | characterization tests after a PoC; every bug gets a failing reproduction test first |

Outside the pure core, don't stop for test review: finish the work and report the pending snapshots.

## Pure-core work: the sequence

For versions, bumps, branches, channels, the commit parser and config merge:

1. **Tests only, in their own file.** Write tests from the approved source (a spec or table, e.g. `docs/test-tables/*.toml`) in a new sibling file for this feature, e.g. `src/next_version_tests.rs` declared as `#[cfg(test)] mod next_version_tests;`. Never append to an existing test file: the review in step 4 must cover exactly the new tests.
2. **Real red.** Make the stub return a wrong but valid value (e.g. `Version::new(0, 0, 0)`), so the new tests fail **on their assertions**. A `todo!()` panic or a compile error doesn't count as red. If a test already passes against the stub (e.g. a rejection case against a stub that always errors), keep it and list it in your report as "passes on stub".
3. **One `test: …` commit** containing the new test file, the `mod` line and the wrong-value stub. The stub belongs here: it isn't implementation, and it makes this commit show the red on its own.
4. **STOP and report** the failing test names (and any "passes on stub"), then wait for human review. Don't implement in the same turn.
5. After approval, implement in a separate `feat:`/`fix:` commit. Never weaken, skip, delete or rewrite an approved test to make code pass; if one looks wrong, stop and report. It changes only with the maintainer's agreement, as its own `test:` commit.

New dependencies, including dev-dependencies: ask before adding them.

## Snapshots (insta)

**Never approve a snapshot yourself.** No `cargo insta accept`, no `cargo insta test --accept`, no `INSTA_UPDATE=always`, no hand-editing `.snap` files.

When output changes: run `cargo insta test`, leave the `.snap.new` files, and report "N snapshots pending review" with a one-line summary of each diff. CI not being green yet is the correct state until a human accepts them.

| Excuse | Reality |
| --- | --- |
| "I reviewed the diff, it's exactly the intended change" | You wrote the change, so your review doesn't count. The human approves. |
| "The task says CI must be green" | Green CI from a self-approved snapshot is fake green. Report the pending snapshots. |
| "It's a trivial reorder" | Trivial diffs are cheap for the human to approve. Leave them. |

## Placement

Unit tests go in a sibling `tests.rs` (`#[cfg(test)] mod tests;`); integration tests go in `tests/`; no inline test blocks with a body. Fixtures live in `semoxide-test-support`.
