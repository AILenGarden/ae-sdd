use ae_sdd_protocol::{CAPABILITY_TOKEN_SCHEMA_V1, CapabilityTokenWire};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::{ClientError, ClientResult};

/// Verified offline claims needed by a thin Hook adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineCapabilityClaims {
    /// Bound daemon boot.
    pub boot_id: String,
    /// Bound trusted session.
    pub session_id: String,
    /// Absolute expiry.
    pub expires_at_unix_ms: u64,
    /// Whether this daemon-derived claim represents an engaged session.
    pub engaged: bool,
}

/// Ed25519 verifier reconstructed from one protected endpoint manifest snapshot.
pub struct OfflineCapabilityVerifier {
    boot_id: String,
    key_id: String,
    key: VerifyingKey,
}

impl OfflineCapabilityVerifier {
    /// Reconstructs and key-ID-checks a manifest verification key.
    pub fn from_manifest(
        boot_id: impl Into<String>,
        key_id: impl Into<String>,
        public_key_hex: &str,
    ) -> ClientResult<Self> {
        let boot_id = boot_id.into();
        let key_id = key_id.into();
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(public_key_hex, &mut bytes)
            .map_err(|_| ClientError::OfflineCapabilityInvalid)?;
        let key =
            VerifyingKey::from_bytes(&bytes).map_err(|_| ClientError::OfflineCapabilityInvalid)?;
        if hex::encode(Sha256::digest(key.to_bytes())) != key_id {
            return Err(ClientError::OfflineCapabilityInvalid);
        }
        Ok(Self {
            boot_id,
            key_id,
            key,
        })
    }

    /// Verifies signature, boot/key/session binding, and lifetime.
    pub fn verify(
        &self,
        encoded_token: &str,
        expected_session_id: &str,
        now_unix_ms: u64,
    ) -> ClientResult<OfflineCapabilityClaims> {
        let token = CapabilityTokenWire::decode_json(encoded_token)
            .map_err(|_| ClientError::OfflineCapabilityInvalid)?;
        if token.schema_version() != CAPABILITY_TOKEN_SCHEMA_V1
            || token.key_id() != self.key_id
            || token.boot_id() != self.boot_id
            || token.session_id() != expected_session_id
            || !matches!(token.capability_id(), "hook.engaged" | "hook.unengaged")
            || now_unix_ms < token.issued_at_unix_ms()
            || now_unix_ms >= token.expires_at_unix_ms()
            || token.expires_at_unix_ms() <= token.issued_at_unix_ms()
            || !matches!(token.role(), "root" | "series" | "task" | "reviewer")
            || (token.role() == "root" && token.delegation_id().is_some())
            || (token.role() != "root" && token.delegation_id().is_none())
        {
            return Err(ClientError::OfflineCapabilityInvalid);
        }
        let canonical = token
            .canonical_claims_bytes()
            .map_err(|_| ClientError::OfflineCapabilityInvalid)?;
        let mut signature = [0_u8; 64];
        hex::decode_to_slice(token.signature(), &mut signature)
            .map_err(|_| ClientError::OfflineCapabilityInvalid)?;
        self.key
            .verify(&canonical, &Signature::from_bytes(&signature))
            .map_err(|_| ClientError::OfflineCapabilityInvalid)?;
        Ok(OfflineCapabilityClaims {
            boot_id: token.boot_id().to_owned(),
            session_id: token.session_id().to_owned(),
            expires_at_unix_ms: token.expires_at_unix_ms(),
            engaged: token.capability_id() == "hook.engaged",
        })
    }
}
