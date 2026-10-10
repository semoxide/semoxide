//! What every semoxide error tells its users (OBSERVABILITY §8, CLI.md error object).

use crate::{ErrorCode, Location};

/// Where error pages live until the docs site exists; `url()` appends `<slug>.md`.
pub const DOCS_BASE_URL: &str = "https://github.com/semoxide/semoxide/blob/main/docs/errors/";

/// Implemented by every semoxide error. The message is the error's `Display`.
///
/// Only [`code`](ErrorInfo::code) is required; the other methods have defaults.
pub trait ErrorInfo: std::error::Error {
    /// The namespaced code, e.g. `core::no_git_repo`.
    fn code(&self) -> ErrorCode;

    /// A one-line hint on how to fix it.
    fn help(&self) -> Option<String> {
        None
    }

    /// Where the cause is, e.g. the line in `semoxide.toml`, for a pointer in the output.
    fn location(&self) -> Option<Location> {
        None
    }

    /// The docs page for this error: [`DOCS_BASE_URL`] + the code's slug + `.md`.
    fn url(&self) -> String {
        format!("{DOCS_BASE_URL}{}.md", self.code().slug())
    }

    /// A transient failure (network timeout, rate limit) that may succeed when retried.
    fn retryable(&self) -> bool {
        false
    }

    /// Whether anything was pushed or published before the error. Retrying is safe only when
    /// [`retryable`](ErrorInfo::retryable) is true and this is false (CLI.md).
    fn remote_writes_happened(&self) -> bool {
        false
    }
}
