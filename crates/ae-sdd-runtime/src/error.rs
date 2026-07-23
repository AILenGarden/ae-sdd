use ae_sdd_protocol::StableErrorCode;
use thiserror::Error;

/// Runtime result using a stable protocol-classified failure.
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Redacted daemon application failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{code:?}: {message}")]
pub struct RuntimeError {
    code: StableErrorCode,
    message: String,
    remediation: Option<String>,
}

impl RuntimeError {
    /// Creates a stable, redacted failure.
    #[must_use]
    pub fn new(code: StableErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            remediation: None,
        }
    }

    /// Adds caller-safe remediation text.
    #[must_use]
    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    /// Stable protocol code.
    #[must_use]
    pub const fn code(&self) -> StableErrorCode {
        self.code
    }

    /// Redacted human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Optional caller-safe remediation.
    #[must_use]
    pub fn remediation(&self) -> Option<&str> {
        self.remediation.as_deref()
    }
}
