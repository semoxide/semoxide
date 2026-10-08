//! Error codes of this crate. `ALL` feeds the error-code registry check and `semoxide schema`.

use semoxide_error::ErrorCode;

/// A section semoxide knows but doesn't support yet (`[packages]`, `config.extends`).
pub const CONFIG_UNSUPPORTED_SECTION: ErrorCode =
    ErrorCode::from_static("config::unsupported_section");
/// A key no domain defines.
pub const CONFIG_UNKNOWN_KEY: ErrorCode = ErrorCode::from_static("config::unknown_key");
/// A value of the wrong type or format.
pub const CONFIG_INVALID_VALUE: ErrorCode = ErrorCode::from_static("config::invalid_value");
/// Keys that can't appear together.
pub const CONFIG_CONFLICTING_KEYS: ErrorCode = ErrorCode::from_static("config::conflicting_keys");
/// Options or a step order for a plugin missing from `steps.plugins`.
pub const CONFIG_PLUGIN_NOT_ENABLED: ErrorCode =
    ErrorCode::from_static("config::plugin_not_enabled");

/// Every error code this crate can produce.
pub const ALL: &[ErrorCode] = &[
    CONFIG_UNSUPPORTED_SECTION,
    CONFIG_UNKNOWN_KEY,
    CONFIG_INVALID_VALUE,
    CONFIG_CONFLICTING_KEYS,
    CONFIG_PLUGIN_NOT_ENABLED,
];
