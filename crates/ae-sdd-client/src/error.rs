use ae_sdd_protocol::StableErrorCode;
use thiserror::Error;

/// Client result.
pub type ClientResult<T> = Result<T, ClientError>;

/// Thin-client failure with stable daemon classification where available.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Endpoint manifest could not be read atomically or decoded.
    #[error("endpoint manifest is unavailable or invalid")]
    EndpointManifest,
    /// Local Named Pipe/UDS connection or frame I/O failed.
    #[error("local daemon is unavailable")]
    DaemonUnavailable,
    /// The daemon rejected the call with a stable wire code.
    #[error("daemon rejected request: {code:?}: {message}")]
    Remote {
        /// Stable wire code.
        code: StableErrorCode,
        /// Redacted daemon message.
        message: String,
    },
    /// Response was malformed or did not correlate to the request.
    #[error("daemon response violates the negotiated protocol")]
    Protocol,
    /// Offline capability is malformed, stale, expired, or has an invalid signature.
    #[error("offline session capability is invalid")]
    OfflineCapabilityInvalid,
}

impl ClientError {
    /// Returns the stable code exposed to CLI/Hook policy.
    #[must_use]
    pub const fn stable_code(&self) -> StableErrorCode {
        match self {
            Self::EndpointManifest | Self::DaemonUnavailable => StableErrorCode::DaemonUnavailable,
            Self::Remote { code, .. } => *code,
            Self::Protocol => StableErrorCode::ProtocolVersionUnsupported,
            Self::OfflineCapabilityInvalid => StableErrorCode::SessionExpired,
        }
    }
}
