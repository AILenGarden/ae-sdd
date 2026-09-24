//! Single source for native runtime defaults.
//!
//! Connection-specific values still come from the connection registry. These
//! constants are only the defaults used when a profile omits a field and the
//! daemon's shared admission limits.

pub const DEFAULT_CONNECT_TIMEOUT_SECONDS: u64 = 10;
pub const DEFAULT_STATEMENT_TIMEOUT_SECONDS: u64 = 30;
pub const DEFAULT_MAX_ROWS: usize = 500;
pub const QUERY_MAX_CONCURRENT: usize = 4;
pub const QUERY_MAX_PER_WINDOW: usize = 60;
pub const QUERY_RATE_WINDOW_SECONDS: u64 = 60;
pub const CLIENT_REQUEST_TIMEOUT_SECONDS: u64 = 35;
pub const QUERY_TIMEOUT_GRACE_SECONDS: u64 = 2;
