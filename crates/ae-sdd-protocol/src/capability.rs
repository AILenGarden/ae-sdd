use serde::{Deserialize, Serialize};

/// Schema identifier for the boot-scoped Ed25519 capability token.
pub const CAPABILITY_TOKEN_SCHEMA_V1: &str = "1";

/// Flat, transport-independent representation of a signed capability token.
///
/// The signature covers [`Self::canonical_claims_bytes`], not the encoded
/// token. Keeping this representation in the protocol crate gives the daemon,
/// CLI and Hook adapters one canonical field order and one strict JSON schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityTokenWire {
    schema_version: String,
    key_id: String,
    boot_id: String,
    capability_id: String,
    session_id: String,
    role: String,
    delegation_id: Option<String>,
    grant_digest: String,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    signature: String,
}

impl CapabilityTokenWire {
    /// Constructs a v1 token from already validated textual claims.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new_v1(
        key_id: impl Into<String>,
        boot_id: impl Into<String>,
        capability_id: impl Into<String>,
        session_id: impl Into<String>,
        role: impl Into<String>,
        delegation_id: Option<String>,
        grant_digest: impl Into<String>,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        signature: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: CAPABILITY_TOKEN_SCHEMA_V1.to_owned(),
            key_id: key_id.into(),
            boot_id: boot_id.into(),
            capability_id: capability_id.into(),
            session_id: session_id.into(),
            role: role.into(),
            delegation_id,
            grant_digest: grant_digest.into(),
            issued_at_unix_ms,
            expires_at_unix_ms,
            signature: signature.into(),
        }
    }

    /// Decodes the strict JSON representation.
    pub fn decode_json(encoded: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(encoded)
    }

    /// Encodes the strict JSON representation.
    pub fn encode_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Returns the deterministic byte sequence covered by the signature.
    pub fn canonical_claims_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CanonicalClaims<'a> {
            schema_version: &'a str,
            key_id: &'a str,
            boot_id: &'a str,
            capability_id: &'a str,
            session_id: &'a str,
            role: &'a str,
            delegation_id: Option<&'a str>,
            grant_digest: &'a str,
            issued_at_unix_ms: u64,
            expires_at_unix_ms: u64,
        }

        serde_json::to_vec(&CanonicalClaims {
            schema_version: self.schema_version(),
            key_id: self.key_id(),
            boot_id: self.boot_id(),
            capability_id: self.capability_id(),
            session_id: self.session_id(),
            role: self.role(),
            delegation_id: self.delegation_id(),
            grant_digest: self.grant_digest(),
            issued_at_unix_ms: self.issued_at_unix_ms(),
            expires_at_unix_ms: self.expires_at_unix_ms(),
        })
    }

    /// Returns the schema identifier.
    #[must_use]
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// Returns the boot key identifier.
    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Returns the daemon boot identifier.
    #[must_use]
    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }

    /// Returns the granted capability identifier.
    #[must_use]
    pub fn capability_id(&self) -> &str {
        &self.capability_id
    }

    /// Returns the bound session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the role name.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Returns the optional physical delegation identifier.
    #[must_use]
    pub fn delegation_id(&self) -> Option<&str> {
        self.delegation_id.as_deref()
    }

    /// Returns the digest of the scoped grant.
    #[must_use]
    pub fn grant_digest(&self) -> &str {
        &self.grant_digest
    }

    /// Returns the inclusive issue time in Unix milliseconds.
    #[must_use]
    pub const fn issued_at_unix_ms(&self) -> u64 {
        self.issued_at_unix_ms
    }

    /// Returns the exclusive expiry time in Unix milliseconds.
    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    /// Returns the hex-encoded Ed25519 signature.
    #[must_use]
    pub fn signature(&self) -> &str {
        &self.signature
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn token() -> CapabilityTokenWire {
        CapabilityTokenWire::new_v1(
            "key",
            "boot",
            "flow.read",
            "session",
            "series",
            Some("delegation".to_owned()),
            "grant",
            1_000,
            2_000,
            "signature",
        )
    }

    #[test]
    fn canonical_claims_have_a_frozen_order_and_exclude_signature() {
        assert_eq!(
            String::from_utf8(token().canonical_claims_bytes().expect("canonical JSON"))
                .expect("UTF-8"),
            r#"{"schemaVersion":"1","keyId":"key","bootId":"boot","capabilityId":"flow.read","sessionId":"session","role":"series","delegationId":"delegation","grantDigest":"grant","issuedAtUnixMs":1000,"expiresAtUnixMs":2000}"#
        );
    }

    #[test]
    fn token_json_is_strict_and_round_trips() {
        let token = token();
        assert_eq!(
            CapabilityTokenWire::decode_json(&token.encode_json().expect("encoded token"))
                .expect("decoded token"),
            token
        );

        let mut value = serde_json::to_value(&token).expect("token value");
        value
            .as_object_mut()
            .expect("token object")
            .insert("unexpected".to_owned(), json!(true));
        assert!(serde_json::from_value::<CapabilityTokenWire>(value).is_err());
    }
}
