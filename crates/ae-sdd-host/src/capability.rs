use ae_sdd_domain::{AgentRole, BootId, CapabilityId, DelegationId, SessionId};
use ae_sdd_protocol::{CAPABILITY_TOKEN_SCHEMA_V1, CapabilityTokenWire};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GrantDigest([u8; 32]);

impl GrantDigest {
    #[must_use]
    pub fn digest(bytes: impl AsRef<[u8]>) -> Self {
        Self(Sha256::digest(bytes.as_ref()).into())
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    fn from_hex(value: &str) -> Result<Self, CapabilityError> {
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(value, &mut bytes).map_err(|_| CapabilityError::InvalidGrantDigest)?;
        Ok(Self(bytes))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityClaims {
    key_id: Box<str>,
    boot_id: BootId,
    capability_id: CapabilityId,
    session_id: SessionId,
    role: AgentRole,
    delegation_id: Option<DelegationId>,
    grant_digest: GrantDigest,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

impl CapabilityClaims {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key_id: impl Into<Box<str>>,
        boot_id: BootId,
        capability_id: CapabilityId,
        session_id: SessionId,
        role: AgentRole,
        delegation_id: Option<DelegationId>,
        grant_digest: GrantDigest,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<Self, CapabilityError> {
        if expires_at_unix_ms <= issued_at_unix_ms {
            return Err(CapabilityError::InvalidLifetime);
        }
        if matches!(role, AgentRole::Root) && delegation_id.is_some() {
            return Err(CapabilityError::RootHasDelegation);
        }
        if !matches!(role, AgentRole::Root) && delegation_id.is_none() {
            return Err(CapabilityError::ChildMissingDelegation);
        }
        Ok(Self {
            key_id: key_id.into(),
            boot_id,
            capability_id,
            session_id,
            role,
            delegation_id,
            grant_digest,
            issued_at_unix_ms,
            expires_at_unix_ms,
        })
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub const fn boot_id(&self) -> BootId {
        self.boot_id
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    #[must_use]
    pub const fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    #[must_use]
    pub const fn role(&self) -> AgentRole {
        self.role
    }

    #[must_use]
    pub const fn delegation_id(&self) -> Option<DelegationId> {
        self.delegation_id
    }

    #[must_use]
    pub const fn grant_digest(&self) -> GrantDigest {
        self.grant_digest
    }

    #[must_use]
    pub const fn issued_at_unix_ms(&self) -> u64 {
        self.issued_at_unix_ms
    }

    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, CapabilityError> {
        token_wire(self, "")
            .canonical_claims_bytes()
            .map_err(CapabilityError::Canonicalize)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityToken {
    claims: CapabilityClaims,
    signature: [u8; 64],
}

impl CapabilityToken {
    #[must_use]
    pub const fn claims(&self) -> &CapabilityClaims {
        &self.claims
    }

    #[must_use]
    pub const fn signature(&self) -> &[u8; 64] {
        &self.signature
    }

    pub fn encode(&self) -> Result<String, CapabilityError> {
        token_wire(&self.claims, hex::encode(self.signature))
            .encode_json()
            .map_err(CapabilityError::EncodeToken)
    }

    pub fn decode(value: &str) -> Result<Self, CapabilityError> {
        let wire = CapabilityTokenWire::decode_json(value).map_err(CapabilityError::DecodeToken)?;
        if wire.schema_version() != CAPABILITY_TOKEN_SCHEMA_V1 {
            return Err(CapabilityError::UnsupportedTokenSchema);
        }
        let role = match wire.role() {
            "root" => AgentRole::Root,
            "series" => AgentRole::Series,
            "task" => AgentRole::Task,
            "reviewer" => AgentRole::Reviewer,
            _ => return Err(CapabilityError::InvalidRole),
        };
        let claims = CapabilityClaims::new(
            wire.key_id(),
            wire.boot_id()
                .parse()
                .map_err(|_| CapabilityError::InvalidIdentity)?,
            CapabilityId::new(wire.capability_id())
                .map_err(|_| CapabilityError::InvalidIdentity)?,
            wire.session_id()
                .parse()
                .map_err(|_| CapabilityError::InvalidIdentity)?,
            role,
            wire.delegation_id()
                .map(|value| value.parse().map_err(|_| CapabilityError::InvalidIdentity))
                .transpose()?,
            GrantDigest::from_hex(wire.grant_digest())?,
            wire.issued_at_unix_ms(),
            wire.expires_at_unix_ms(),
        )?;
        let mut signature = [0_u8; 64];
        hex::decode_to_slice(wire.signature(), &mut signature)
            .map_err(|_| CapabilityError::InvalidSignatureEncoding)?;
        Ok(Self { claims, signature })
    }
}

fn token_wire(claims: &CapabilityClaims, signature: impl Into<String>) -> CapabilityTokenWire {
    CapabilityTokenWire::new_v1(
        claims.key_id(),
        claims.boot_id.to_string(),
        claims.capability_id.as_str(),
        claims.session_id.to_string(),
        role_name(claims.role),
        claims.delegation_id.map(|value| value.to_string()),
        claims.grant_digest.to_hex(),
        claims.issued_at_unix_ms,
        claims.expires_at_unix_ms,
        signature,
    )
}

pub struct BootCapabilitySigner {
    boot_id: BootId,
    key_id: Box<str>,
    signing_key: SigningKey,
}

impl BootCapabilitySigner {
    #[must_use]
    pub fn generate(boot_id: BootId) -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let key_id = key_id(&signing_key.verifying_key()).into_boxed_str();
        Self {
            boot_id,
            key_id,
            signing_key,
        }
    }

    #[must_use]
    pub const fn boot_id(&self) -> BootId {
        self.boot_id
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub fn public_key(&self) -> CapabilityPublicKey {
        CapabilityPublicKey {
            boot_id: self.boot_id,
            key_id: self.key_id.clone(),
            verifying_key: self.signing_key.verifying_key(),
        }
    }

    pub fn sign(&self, claims: CapabilityClaims) -> Result<CapabilityToken, CapabilityError> {
        if claims.boot_id != self.boot_id {
            return Err(CapabilityError::BootMismatch);
        }
        if claims.key_id() != self.key_id() {
            return Err(CapabilityError::KeyMismatch);
        }
        let signature = self.signing_key.sign(&claims.canonical_bytes()?).to_bytes();
        Ok(CapabilityToken { claims, signature })
    }
}

#[derive(Clone)]
pub struct CapabilityPublicKey {
    boot_id: BootId,
    key_id: Box<str>,
    verifying_key: VerifyingKey,
}

impl CapabilityPublicKey {
    pub fn from_hex(
        boot_id: BootId,
        expected_key_id: impl Into<Box<str>>,
        public_key_hex: &str,
    ) -> Result<Self, CapabilityError> {
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(public_key_hex, &mut bytes)
            .map_err(|_| CapabilityError::InvalidPublicKey)?;
        let verifying_key =
            VerifyingKey::from_bytes(&bytes).map_err(|_| CapabilityError::InvalidPublicKey)?;
        let expected_key_id = expected_key_id.into();
        if key_id(&verifying_key) != expected_key_id.as_ref() {
            return Err(CapabilityError::KeyMismatch);
        }
        Ok(Self {
            boot_id,
            key_id: expected_key_id,
            verifying_key,
        })
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.to_bytes())
    }

    pub fn verify<'a>(
        &self,
        token: &'a CapabilityToken,
        now_unix_ms: u64,
    ) -> Result<&'a CapabilityClaims, CapabilityError> {
        if token.claims.boot_id != self.boot_id {
            return Err(CapabilityError::BootMismatch);
        }
        if token.claims.key_id() != self.key_id() {
            return Err(CapabilityError::KeyMismatch);
        }
        let signature = Signature::from_bytes(&token.signature);
        self.verifying_key
            .verify(&token.claims.canonical_bytes()?, &signature)
            .map_err(|_| CapabilityError::InvalidSignature)?;
        if now_unix_ms < token.claims.issued_at_unix_ms {
            return Err(CapabilityError::NotYetValid);
        }
        if now_unix_ms >= token.claims.expires_at_unix_ms {
            return Err(CapabilityError::Expired);
        }
        Ok(&token.claims)
    }
}

#[derive(Debug, Error)]
pub enum CapabilityError {
    #[error("capability expiry must be later than issue time")]
    InvalidLifetime,
    #[error("root capability must not carry a delegation")]
    RootHasDelegation,
    #[error("child capability must carry a delegation")]
    ChildMissingDelegation,
    #[error("capability boot does not match the active daemon boot")]
    BootMismatch,
    #[error("capability key ID does not match the active boot key")]
    KeyMismatch,
    #[error("capability signature is invalid")]
    InvalidSignature,
    #[error("capability is not valid yet")]
    NotYetValid,
    #[error("capability has expired")]
    Expired,
    #[error("failed to canonicalize capability claims: {0}")]
    Canonicalize(serde_json::Error),
    #[error("capability token schema is unsupported")]
    UnsupportedTokenSchema,
    #[error("capability token contains an invalid role")]
    InvalidRole,
    #[error("capability token contains an invalid typed identity")]
    InvalidIdentity,
    #[error("capability token contains an invalid grant digest")]
    InvalidGrantDigest,
    #[error("capability token contains invalid signature encoding")]
    InvalidSignatureEncoding,
    #[error("capability public key is invalid")]
    InvalidPublicKey,
    #[error("failed to encode capability token: {0}")]
    EncodeToken(serde_json::Error),
    #[error("failed to decode capability token: {0}")]
    DecodeToken(serde_json::Error),
}

fn key_id(key: &VerifyingKey) -> String {
    hex::encode(Sha256::digest(key.to_bytes()))
}

const fn role_name(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Root => "root",
        AgentRole::Series => "series",
        AgentRole::Task => "task",
        AgentRole::Reviewer => "reviewer",
    }
}

#[cfg(test)]
mod tests {
    use ae_sdd_domain::{CapabilityId, DelegationId};
    use uuid::Uuid;

    use super::*;

    fn claims(signer: &BootCapabilitySigner) -> CapabilityClaims {
        CapabilityClaims::new(
            signer.key_id(),
            signer.boot_id(),
            CapabilityId::new("flow.read").expect("valid capability"),
            SessionId::from_uuid(Uuid::from_u128(2)),
            AgentRole::Series,
            Some(DelegationId::from_uuid(Uuid::from_u128(3))),
            GrantDigest::digest(b"scoped grant"),
            1_000,
            2_000,
        )
        .expect("valid claims")
    }

    #[test]
    fn boot_key_signs_tokens_that_can_be_verified_offline() {
        let signer = BootCapabilitySigner::generate(BootId::from_uuid(Uuid::from_u128(1)));
        let verifier = signer.public_key();
        let token = signer.sign(claims(&signer)).expect("signed token");

        assert_eq!(
            verifier.verify(&token, 1_500).expect("valid token").role(),
            AgentRole::Series
        );
        assert!(matches!(
            verifier.verify(&token, 2_000),
            Err(CapabilityError::Expired)
        ));
    }

    #[test]
    fn a_different_boot_key_cannot_verify_a_token() {
        let signer = BootCapabilitySigner::generate(BootId::from_uuid(Uuid::from_u128(1)));
        let other = BootCapabilitySigner::generate(BootId::from_uuid(Uuid::from_u128(1)));
        let token = signer.sign(claims(&signer)).expect("signed token");

        assert!(matches!(
            other.public_key().verify(&token, 1_500),
            Err(CapabilityError::KeyMismatch)
        ));
    }

    #[test]
    fn token_and_manifest_public_key_round_trip_for_offline_verification() {
        let signer = BootCapabilitySigner::generate(BootId::from_uuid(Uuid::from_u128(1)));
        let published = signer.public_key();
        let verifier = CapabilityPublicKey::from_hex(
            signer.boot_id(),
            signer.key_id(),
            &published.public_key_hex(),
        )
        .expect("manifest key parses");
        let encoded = signer
            .sign(claims(&signer))
            .expect("signed token")
            .encode()
            .expect("token encodes");
        let decoded = CapabilityToken::decode(&encoded).expect("token decodes");

        assert_eq!(
            verifier
                .verify(&decoded, 1_500)
                .expect("offline token verifies")
                .session_id(),
            SessionId::from_uuid(Uuid::from_u128(2))
        );
    }

    /// Builds claims varying only role/delegation/lifetime, so each assertion
    /// below isolates a single `CapabilityClaims::new` rejection rule.
    fn claims_with(
        role: AgentRole,
        delegation_id: Option<DelegationId>,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<CapabilityClaims, CapabilityError> {
        CapabilityClaims::new(
            "key-1",
            BootId::from_uuid(Uuid::from_u128(1)),
            CapabilityId::new("flow.read").expect("valid capability"),
            SessionId::from_uuid(Uuid::from_u128(2)),
            role,
            delegation_id,
            GrantDigest::digest(b"scoped grant"),
            issued_at_unix_ms,
            expires_at_unix_ms,
        )
    }

    #[test]
    fn claims_reject_a_non_positive_lifetime() {
        let delegation = Some(DelegationId::from_uuid(Uuid::from_u128(3)));

        // Expiry equal to issuance is not a valid window.
        assert!(matches!(
            claims_with(AgentRole::Series, delegation, 1_000, 1_000),
            Err(CapabilityError::InvalidLifetime)
        ));
        // Expiry before issuance.
        assert!(matches!(
            claims_with(AgentRole::Series, delegation, 2_000, 1_000),
            Err(CapabilityError::InvalidLifetime)
        ));
        assert!(claims_with(AgentRole::Series, delegation, 1_000, 1_001).is_ok());
    }

    #[test]
    fn claims_enforce_the_root_delegation_invariant_both_ways() {
        let delegation = Some(DelegationId::from_uuid(Uuid::from_u128(3)));

        // Root must not carry a delegation.
        assert!(matches!(
            claims_with(AgentRole::Root, delegation, 1_000, 2_000),
            Err(CapabilityError::RootHasDelegation)
        ));
        // Every non-root role must carry one.
        for role in [AgentRole::Series, AgentRole::Task, AgentRole::Reviewer] {
            assert!(
                matches!(
                    claims_with(role, None, 1_000, 2_000),
                    Err(CapabilityError::ChildMissingDelegation)
                ),
                "{role:?} without a delegation must be rejected"
            );
        }
        // The two legal shapes.
        assert!(claims_with(AgentRole::Root, None, 1_000, 2_000).is_ok());
        assert!(claims_with(AgentRole::Series, delegation, 1_000, 2_000).is_ok());
    }

    #[test]
    fn signing_rejects_claims_bound_to_another_boot_or_key() {
        let signer = BootCapabilitySigner::generate(BootId::from_uuid(Uuid::from_u128(1)));
        let delegation = Some(DelegationId::from_uuid(Uuid::from_u128(3)));
        let build = |key_id: &str, boot_id: BootId| {
            CapabilityClaims::new(
                key_id,
                boot_id,
                CapabilityId::new("flow.read").expect("valid capability"),
                SessionId::from_uuid(Uuid::from_u128(2)),
                AgentRole::Series,
                delegation,
                GrantDigest::digest(b"scoped grant"),
                1_000,
                2_000,
            )
            .expect("claims themselves are valid")
        };

        // Claims minted against a different boot generation.
        assert!(matches!(
            signer.sign(build(
                signer.key_id(),
                BootId::from_uuid(Uuid::from_u128(9))
            )),
            Err(CapabilityError::BootMismatch)
        ));
        // Right boot, wrong key id.
        assert!(matches!(
            signer.sign(build("not-this-signers-key", signer.boot_id())),
            Err(CapabilityError::KeyMismatch)
        ));
        assert!(
            signer
                .sign(build(signer.key_id(), signer.boot_id()))
                .is_ok()
        );
    }
}
