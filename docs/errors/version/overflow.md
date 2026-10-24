# `version::overflow`

The next version's number would exceed 18446744073709551615 (`u64::MAX`).

## When it happens

- the last release is `18446744073709551615.0.0` and a breaking change asks for a major release

## How to fix it

Release a lower level: no higher version exists at this one.

## semantic-release

No equivalent: its numbers stop at 2^53−1.

## Related

- [`version::out_of_range`](out-of-range.md)
