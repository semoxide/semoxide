//! Config loading: merging the layers (defaults, `semoxide.toml`, `--set` flags) into one table,
//! with the source of every value (CONFIG.md §1).

mod flag;
mod load;
mod merge;

pub use flag::{FlagError, parse_flag};
pub use load::{FileError, LoadError, Loaded, load};
pub use merge::{Layer, Merged, Source, merge};
