//! Fuzz target for [`semoxide_fuzz::version`].

#![cfg_attr(fuzzing, no_main)]
#![forbid(unsafe_code)]

#[cfg(fuzzing)]
libfuzzer_sys::fuzz_target!(|input: &str| semoxide_fuzz::version(input));

/// Built without `cargo fuzz`, the target only checks that the code compiles.
#[cfg(not(fuzzing))]
fn main() {}
