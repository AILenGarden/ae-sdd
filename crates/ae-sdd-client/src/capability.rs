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

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    const BOOT: &str = "00000000-0000-0000-0000-0000000000aa";
    const SESSION: &str = "00000000-0000-0000-0000-0000000000bb";
    const DELEGATION: &str = "00000000-0000-0000-0000-0000000000cc";

    /// A fixed (non-random) key keeps every assertion below reproducible.
    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    fn key_id_for(key: &SigningKey) -> String {
        hex::encode(Sha256::digest(key.verifying_key().to_bytes()))
    }

    fn verifier() -> OfflineCapabilityVerifier {
        let key = signing_key();
        OfflineCapabilityVerifier::from_manifest(
            BOOT,
            key_id_for(&key),
            &hex::encode(key.verifying_key().to_bytes()),
        )
        .expect("manifest key reconstructs")
    }

    /// Mints a genuinely signed token, so every rejection asserted below is
    /// attributable to the claim being checked rather than a bad signature.
    #[allow(clippy::too_many_arguments)]
    fn signed_token(
        key_id: &str,
        boot_id: &str,
        capability_id: &str,
        session_id: &str,
        role: &str,
        delegation_id: Option<String>,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> String {
        let key = signing_key();
        let unsigned = CapabilityTokenWire::new_v1(
            key_id,
            boot_id,
            capability_id,
            session_id,
            role,
            delegation_id,
            "d".repeat(64),
            issued_at_unix_ms,
            expires_at_unix_ms,
            String::new(),
        );
        let canonical = unsigned.canonical_claims_bytes().expect("canonical claims");
        let signature = hex::encode(key.sign(&canonical).to_bytes());
        CapabilityTokenWire::new_v1(
            unsigned.key_id(),
            unsigned.boot_id(),
            unsigned.capability_id(),
            unsigned.session_id(),
            unsigned.role(),
            unsigned.delegation_id().map(ToOwned::to_owned),
            unsigned.grant_digest(),
            unsigned.issued_at_unix_ms(),
            unsigned.expires_at_unix_ms(),
            signature,
        )
        .encode_json()
        .expect("token encodes")
    }

    /// The default: a valid engaged root token inside its lifetime.
    fn valid_root_token() -> String {
        let key = signing_key();
        signed_token(
            &key_id_for(&key),
            BOOT,
            "hook.engaged",
            SESSION,
            "root",
            None,
            1_000,
            2_000,
        )
    }

    #[test]
    fn a_genuine_token_verifies_and_reports_its_engaged_flag() {
        let key = signing_key();
        let claims = verifier()
            .verify(&valid_root_token(), SESSION, 1_500)
            .expect("a genuine in-lifetime token must verify");

        assert_eq!(claims.boot_id, BOOT);
        assert_eq!(claims.session_id, SESSION);
        assert_eq!(claims.expires_at_unix_ms, 2_000);
        assert!(claims.engaged, "hook.engaged must map to engaged=true");

        // The unengaged capability is equally valid but flips the flag.
        let unengaged = signed_token(
            &key_id_for(&key),
            BOOT,
            "hook.unengaged",
            SESSION,
            "root",
            None,
            1_000,
            2_000,
        );
        let claims = verifier()
            .verify(&unengaged, SESSION, 1_500)
            .expect("unengaged token verifies");
        assert!(!claims.engaged);
    }

    #[test]
    fn manifest_key_reconstruction_rejects_bad_hex_bad_point_and_wrong_key_id() {
        let key = signing_key();
        let good_hex = hex::encode(key.verifying_key().to_bytes());

        // Not hex / wrong length.
        assert!(matches!(
            OfflineCapabilityVerifier::from_manifest(BOOT, key_id_for(&key), "nothex"),
            Err(ClientError::OfflineCapabilityInvalid)
        ));
        // Right length, not a valid curve point.
        assert!(matches!(
            OfflineCapabilityVerifier::from_manifest(BOOT, key_id_for(&key), &"ff".repeat(32)),
            Err(ClientError::OfflineCapabilityInvalid)
        ));
        // Valid key, but the manifest's key_id does not digest to it: this is
        // the check that stops a swapped-key manifest from being trusted.
        assert!(matches!(
            OfflineCapabilityVerifier::from_manifest(BOOT, "a".repeat(64), &good_hex),
            Err(ClientError::OfflineCapabilityInvalid)
        ));
    }

    #[test]
    fn verification_rejects_every_unsatisfied_binding_and_lifetime_rule() {
        let key = signing_key();
        let key_id = key_id_for(&key);
        // (label, token, expected_session, now) — each row violates exactly one
        // rule in `verify`'s guard chain while staying genuinely signed.
        let cases: Vec<(&str, String, &str, u64)> = vec![
            (
                "wrong key id",
                signed_token(
                    &"b".repeat(64),
                    BOOT,
                    "hook.engaged",
                    SESSION,
                    "root",
                    None,
                    1_000,
                    2_000,
                ),
                SESSION,
                1_500,
            ),
            (
                "wrong boot id",
                signed_token(
                    &key_id,
                    "00000000-0000-0000-0000-0000000000ff",
                    "hook.engaged",
                    SESSION,
                    "root",
                    None,
                    1_000,
                    2_000,
                ),
                SESSION,
                1_500,
            ),
            (
                "session mismatch",
                valid_root_token(),
                "00000000-0000-0000-0000-0000000000ee",
                1_500,
            ),
            (
                "capability outside the hook allowlist",
                signed_token(
                    &key_id,
                    BOOT,
                    "flow.read",
                    SESSION,
                    "root",
                    None,
                    1_000,
                    2_000,
                ),
                SESSION,
                1_500,
            ),
            ("not yet issued", valid_root_token(), SESSION, 999),
            (
                "expired at the boundary",
                valid_root_token(),
                SESSION,
                2_000,
            ),
            (
                "non-positive lifetime",
                signed_token(
                    &key_id,
                    BOOT,
                    "hook.engaged",
                    SESSION,
                    "root",
                    None,
                    2_000,
                    2_000,
                ),
                SESSION,
                2_000,
            ),
            (
                "unknown role",
                signed_token(
                    &key_id,
                    BOOT,
                    "hook.engaged",
                    SESSION,
                    "superuser",
                    None,
                    1_000,
                    2_000,
                ),
                SESSION,
                1_500,
            ),
            (
                "root carrying a delegation",
                signed_token(
                    &key_id,
                    BOOT,
                    "hook.engaged",
                    SESSION,
                    "root",
                    Some(DELEGATION.to_owned()),
                    1_000,
                    2_000,
                ),
                SESSION,
                1_500,
            ),
            (
                "non-root missing its delegation",
                signed_token(
                    &key_id,
                    BOOT,
                    "hook.engaged",
                    SESSION,
                    "task",
                    None,
                    1_000,
                    2_000,
                ),
                SESSION,
                1_500,
            ),
        ];

        for (label, token, expected_session, now) in cases {
            assert!(
                matches!(
                    verifier().verify(&token, expected_session, now),
                    Err(ClientError::OfflineCapabilityInvalid)
                ),
                "must reject: {label}"
            );
        }

        // A non-root role *with* its delegation is the legal counterpart, so the
        // rule above rejects the missing binding rather than the role itself.
        let delegated = signed_token(
            &key_id,
            BOOT,
            "hook.engaged",
            SESSION,
            "task",
            Some(DELEGATION.to_owned()),
            1_000,
            2_000,
        );
        assert!(verifier().verify(&delegated, SESSION, 1_500).is_ok());
    }

    #[test]
    fn verification_rejects_malformed_json_and_forged_signatures() {
        // Not a token at all.
        assert!(matches!(
            verifier().verify("{not json", SESSION, 1_500),
            Err(ClientError::OfflineCapabilityInvalid)
        ));

        // Structurally valid claims, unusable signature encoding.
        let key = signing_key();
        let unsigned_sig = CapabilityTokenWire::new_v1(
            key_id_for(&key),
            BOOT,
            "hook.engaged",
            SESSION,
            "root",
            None,
            "d".repeat(64),
            1_000,
            2_000,
            "nothex",
        )
        .encode_json()
        .expect("encodes");
        assert!(matches!(
            verifier().verify(&unsigned_sig, SESSION, 1_500),
            Err(ClientError::OfflineCapabilityInvalid)
        ));

        // Well-formed signature bytes that simply are not this key's signature
        // over these claims — the actual forgery case.
        let forged = CapabilityTokenWire::new_v1(
            key_id_for(&key),
            BOOT,
            "hook.engaged",
            SESSION,
            "root",
            None,
            "d".repeat(64),
            1_000,
            2_000,
            "00".repeat(64),
        )
        .encode_json()
        .expect("encodes");
        assert!(matches!(
            verifier().verify(&forged, SESSION, 1_500),
            Err(ClientError::OfflineCapabilityInvalid)
        ));
    }

    #[test]
    fn tampering_with_a_signed_claim_invalidates_the_token() {
        // Take a genuine token and swap one claim the signature covers. The
        // signature no longer matches the canonical bytes, so it must fail.
        let genuine = valid_root_token();
        let mut token = CapabilityTokenWire::decode_json(&genuine).expect("decodes");
        token = CapabilityTokenWire::new_v1(
            token.key_id(),
            token.boot_id(),
            token.capability_id(),
            token.session_id(),
            token.role(),
            token.delegation_id().map(ToOwned::to_owned),
            token.grant_digest(),
            token.issued_at_unix_ms(),
            9_999, // extended expiry, not covered by the original signature
            token.signature(),
        );
        let tampered = token.encode_json().expect("encodes");

        assert!(
            matches!(
                verifier().verify(&tampered, SESSION, 1_500),
                Err(ClientError::OfflineCapabilityInvalid)
            ),
            "an extended expiry must not survive signature verification"
        );
    }
}
