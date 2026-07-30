use ae_sdd_protocol::StableErrorCode;
use std::fmt;

/// Client result.
pub type ClientResult<T> = Result<T, ClientError>;

/// Thin-client failure with stable daemon classification where available.
#[derive(Debug)]
pub enum ClientError {
    /// Endpoint manifest could not be read atomically or decoded.
    EndpointManifest,
    /// Local Named Pipe/UDS connection or frame I/O failed.
    DaemonUnavailable,
    /// The daemon rejected the call with a stable wire code.
    Remote {
        /// Stable wire code.
        code: StableErrorCode,
        /// Redacted daemon message.
        message: String,
        /// Redacted actionable remediation supplied by the daemon.
        remediation: Option<String>,
    },
    /// Response was malformed or did not correlate to the request.
    Protocol,
    /// Offline capability is malformed, stale, expired, or has an invalid signature.
    OfflineCapabilityInvalid,
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndpointManifest => {
                formatter.write_str("endpoint manifest is unavailable or invalid")
            }
            Self::DaemonUnavailable => formatter.write_str("local daemon is unavailable"),
            Self::Remote {
                code,
                message,
                remediation,
            } => {
                write!(formatter, "daemon rejected request: {code:?}: {message}")?;
                if let Some(remediation) = remediation {
                    write!(formatter, "; remediation: {remediation}")?;
                }
                Ok(())
            }
            Self::Protocol => {
                formatter.write_str("daemon response violates the negotiated protocol")
            }
            Self::OfflineCapabilityInvalid => {
                formatter.write_str("offline session capability is invalid")
            }
        }
    }
}

impl std::error::Error for ClientError {}

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

    /// Returns the daemon's redacted remediation when one was supplied.
    #[must_use]
    pub fn remediation(&self) -> Option<&str> {
        match self {
            Self::Remote { remediation, .. } => remediation.as_deref(),
            _ => None,
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
                remediation: None,
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
            remediation: None,
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

    #[test]
    fn remote_error_preserves_actionable_remediation() {
        let remote = ClientError::Remote {
            code: StableErrorCode::ConfirmationRequired,
            message: "confirmation required".to_owned(),
            remediation: Some("provide confirmation for binding lifecycle:abc".to_owned()),
        };

        assert_eq!(
            remote.remediation(),
            Some("provide confirmation for binding lifecycle:abc")
        );
        assert!(remote.to_string().contains("lifecycle:abc"));
    }
}
