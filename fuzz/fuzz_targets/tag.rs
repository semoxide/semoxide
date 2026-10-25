//! Fuzz target for [`semoxide_fuzz::tag`].

#![cfg_attr(fuzzing, no_main)]
#![forbid(unsafe_code)]

#[cfg(fuzzing)]
libfuzzer_sys::fuzz_target!(|input: &str| semoxide_fuzz::tag(input));

/// Built without `cargo fuzz`, the target only checks that the code compiles.
#[cfg(not(fuzzing))]
fn main() {}
