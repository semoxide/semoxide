//! Library API of semoxide, a release tool: builder, run, read-only queries, `RunReport`.

#![forbid(unsafe_code)]
// Public types of the stable API are `#[non_exhaustive]` (CODE-ARCHITECTURE P14).
#![warn(clippy::exhaustive_enums, clippy::exhaustive_structs)]
