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

#[cfg(test)]
mod tests {
    use super::*;

    /// `stable_code` is what CLI/Hook policy branches on, so a wrong mapping
    /// silently misroutes recovery behaviour rather than failing loudly.
    #[test]
    fn stable_code_maps_every_variant_to_its_policy_class() {
        // Both local-reachability failures must look like an unavailable daemon
        // so callers take the ensure/retry path.
        assert_eq!(
            ClientError::EndpointManifest.stable_code(),
            StableErrorCode::DaemonUnavailable
        );
        assert_eq!(
            ClientError::DaemonUnavailable.stable_code(),
            StableErrorCode::DaemonUnavailable
        );
        // A remote rejection passes the daemon's own code through untouched.
        assert_eq!(
            ClientError::Remote {
                code: StableErrorCode::SessionExpired,
                message: "redacted".to_owned(),
            }
            .stable_code(),
            StableErrorCode::SessionExpired
        );
        assert_eq!(
            ClientError::Protocol.stable_code(),
            StableErrorCode::ProtocolVersionUnsupported
        );
        assert_eq!(
            ClientError::OfflineCapabilityInvalid.stable_code(),
            StableErrorCode::SessionExpired
        );
    }

    #[test]
    fn display_messages_are_non_empty_and_do_not_leak_remote_detail_verbatim() {
        let remote = ClientError::Remote {
            code: StableErrorCode::SessionExpired,
            message: "already-redacted".to_owned(),
        };
        for error in [
            ClientError::EndpointManifest,
            ClientError::DaemonUnavailable,
            ClientError::Protocol,
            ClientError::OfflineCapabilityInvalid,
        ] {
            assert!(!error.to_string().trim().is_empty());
        }
        // The remote variant is the only one that surfaces daemon-supplied text.
        assert!(remote.to_string().contains("already-redacted"));
    }
}
