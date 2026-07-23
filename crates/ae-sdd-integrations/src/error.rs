use std::io;

use thiserror::Error;

/// Integration adapter result.
pub type IntegrationResult<T> = Result<T, IntegrationError>;

/// Redacted platform adapter failure.
#[derive(Debug, Error)]
pub enum IntegrationError {
    /// Filesystem, process, or local IPC failure.
    #[error("platform I/O failed: {0}")]
    Io(#[from] io::Error),
    /// Endpoint manifest is invalid.
    #[error("endpoint manifest is invalid: {0}")]
    Manifest(#[from] serde_json::Error),
    /// Another daemon holds the per-user singleton lock.
    #[error("another ae-sdd daemon instance owns the user runtime lock")]
    AlreadyRunning,
    /// SQLite persistence failed without exposing query contents or secrets.
    #[error("runtime SQLite persistence failed")]
    Sqlite,
    /// Runtime directory or endpoint protection could not be established.
    #[error("per-user runtime endpoint protection could not be established")]
    EndpointProtection,
    /// A typed external command exceeded its deadline.
    #[error("typed external command exceeded its deadline")]
    CommandTimeout,
    /// A typed external command exceeded its output budget.
    #[error("typed external command exceeded its output budget")]
    CommandOutputTooLarge,
}
