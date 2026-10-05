# sort-order PoC

Throwaway PoC for semoxide: how to sort release-note entries so the order matches upstream
semantic-release (JS `localeCompare`). Compares **b** (case-insensitive `chars().flat_map(to_lowercase)`,
byte-order tiebreak), **icu** (`icu_collator` 2.3, locale `en`, default options) and plain byte order.

Run: `cargo run --release --bin simple`, `cargo run --release --features icu --bin icu`, `node js.mjs`.

## Results (Windows 11, rustc 1.99 MSVC, release, strip on)

| | bytes | b (simple) | icu |
|---|---|---|---|
| Clean build (median of 3, after `cargo fetch`) | — | 0.98 s | 11.3 s |
| Binary size | — | 196 KiB (200,704 B) | 1.33 MiB (1,389,568 B), **+1.13 MiB** |
| Crates in `cargo tree -e normal` (incl. root) | — | 1 (0 deps) | 33 (32 deps) |
| Collator init | — | — | ~0.03 ms (compiled data) |
| Sort 100k strings (3 runs) | — | 250–280 ms | 340–360 ms |
| Positions differing from JS (36 samples) | 30 | 31 | **0** |
| Positions differing, ASCII-only subset (23) | 23 | 12 | **0** |

b's ASCII mismatches come from only two things: punctuation order (JS: `_private` before `-dash`;
b: `-` before `_`) and case tiebreak (JS lowercase-first `a A`, `coop Coop`; b uppercase-first). Letter
order itself agrees. Non-ASCII is where b fails badly (`Äpfel`, `éclair`, `Über` sort after `Zebra`).

## Conclusion

`icu_collator` reproduces JS `localeCompare` exactly (0/36 mismatches) for +1.1 MiB, 32 deps and ~10 s
extra clean build; runtime cost is negligible. The b comparator is close on ASCII only and wrong for accented text, so use ICU if parity with semantic-release matters.
