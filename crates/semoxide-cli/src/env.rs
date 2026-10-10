//! The process environment, captured once and passed to the library (OBSERVABILITY P3).

use semoxide::Env;

/// A snapshot of this process's environment variables.
#[expect(
    clippy::disallowed_methods,
    reason = "the CLI captures the process env once for the library (OBSERVABILITY P3)"
)]
pub(crate) fn process_env() -> Env {
    std::env::vars_os().collect()
}
