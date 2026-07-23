use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{ClientKind, SecretString};

/// Exact protocol version negotiated by the initial daemon implementation.
pub const PROTOCOL_VERSION_V1: &str = "1.0";

/// Protocol range published by a daemon that supports only protocol v1.
pub const PROTOCOL_RANGE_V1: &str = ">=1.0,<2.0";

/// Endpoint-manifest schema written atomically by the daemon.
pub const ENDPOINT_MANIFEST_SCHEMA_V1: &str = "ae-sdd-endpoint/v1";

/// Strict first-request payload used to authenticate and negotiate a connection.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HandshakeRequest {
    /// Client-supported protocol range.
    pub protocol_range: String,
    /// Versioned build identity of the caller.
    pub client_build: String,
    /// Capability and deadline profile of the caller.
    pub client_kind: ClientKind,
    /// Raw endpoint token read from one protected manifest snapshot.
    pub endpoint_token: SecretString,
    /// Boot identity read from the same manifest snapshot as the token.
    pub expected_boot_id: String,
    /// Policy digest read from the same manifest snapshot as the token.
    pub expected_policy_digest: String,
}

/// Negotiated resource limits that let clients fail fast before sending work.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeLimits {
    /// Maximum framed JSON payload size.
    pub max_frame_bytes: u64,
    /// Maximum physical Agent delegation depth.
    pub max_agent_depth: u8,
    /// Maximum size of one ordinary wire string.
    pub max_string_bytes: u64,
    /// Maximum number of elements in an ordinary collection.
    pub max_collection_items: u64,
    /// Maximum accepted caller deadline budget.
    pub max_deadline_ms: u64,
    /// Default effective Hook deadline budget.
    pub hook_deadline_ms: u64,
    /// Maximum canonical bounded ChildResult size.
    pub max_child_result_bytes: u64,
    /// Maximum ChildResult summary size.
    pub max_child_summary_bytes: u64,
    /// Maximum root context projection size.
    pub max_context_projection_bytes: u64,
}

/// Successful endpoint-authenticated protocol negotiation response.
///
/// Unknown additive fields are intentionally ignored during deserialization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeResponse {
    /// Exact selected protocol version.
    pub protocol_version: String,
    /// Unique daemon boot identity.
    pub boot_id: String,
    /// Durable event-store epoch identity.
    pub event_store_id: String,
    /// Versioned daemon build identity.
    pub daemon_build: String,
    /// Negotiated protocol-minor capability names.
    pub capabilities: BTreeSet<String>,
    /// Current policy digest.
    pub policy_digest: String,
    /// Current typed-operation schema digest.
    pub operation_schema_digest: String,
    /// Limits in force for this connection.
    pub limits: HandshakeLimits,
    /// Identifier of the boot-scoped Ed25519 capability verification key.
    pub capability_key_id: String,
    /// Encoded Ed25519 public key used only for offline capability verification.
    pub capability_public_key: String,
}

/// Protected per-user endpoint manifest published atomically by the daemon.
///
/// The raw token exists only in this DACL/0600-protected file and participating
/// process memory. Unknown additive fields are ignored for minor compatibility.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointManifest {
    /// Versioned endpoint-manifest schema identifier.
    pub schema_version: String,
    /// Operating-system process identity of the daemon.
    pub pid: u32,
    /// Unique daemon boot identity.
    pub boot_id: String,
    /// Durable event-store epoch identity.
    pub event_store_id: String,
    /// Platform-specific Named Pipe or Unix Domain Socket address.
    pub endpoint: String,
    /// Raw per-boot endpoint authentication token.
    pub endpoint_token: SecretString,
    /// Protocol range accepted by this endpoint.
    pub protocol_range: String,
    /// Versioned daemon build identity.
    pub daemon_version: String,
    /// Current policy digest.
    pub policy_digest: String,
    /// Identifier of the boot-scoped capability verification key.
    pub capability_key_id: String,
    /// Encoded Ed25519 public capability verification key.
    pub capability_public_key: String,
    /// RFC3339 daemon start timestamp.
    pub started_at: String,
}
