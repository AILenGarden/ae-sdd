use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{RpcErrorObject, RpcMethod};

/// Exact JSON-RPC version accepted by the local daemon protocol.
pub const JSON_RPC_VERSION: &str = "2.0";

/// A type-level representation of the exact JSON-RPC `2.0` marker.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct JsonRpcVersion;

impl fmt::Debug for JsonRpcVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JsonRpcVersion(2.0)")
    }
}

impl Serialize for JsonRpcVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(JSON_RPC_VERSION)
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == JSON_RPC_VERSION {
            Ok(Self)
        } else {
            Err(de::Error::custom("jsonrpc must be exactly 2.0"))
        }
    }
}

/// Strict typed JSON-RPC request envelope.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsonRpcRequest<P> {
    /// Exact JSON-RPC version marker.
    pub jsonrpc: JsonRpcVersion,
    /// Connection-unique request identity.
    pub id: String,
    /// Exact registered protocol method.
    pub method: RpcMethod,
    /// Method-specific, typed parameters.
    pub params: P,
}

impl<P> JsonRpcRequest<P> {
    /// Constructs a v2 request with a typed parameter payload.
    #[must_use]
    pub fn new(id: impl Into<String>, method: RpcMethod, params: P) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id: id.into(),
            method,
            params,
        }
    }
}

/// Strict typed JSON-RPC server notification envelope.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsonRpcNotification<P> {
    /// Exact JSON-RPC version marker.
    pub jsonrpc: JsonRpcVersion,
    /// Exact registered notification method.
    pub method: RpcMethod,
    /// Method-specific, typed notification parameters.
    pub params: P,
}

/// Successful JSON-RPC response.
///
/// The decoder intentionally ignores additive unknown fields on the envelope
/// and typed response object so a protocol-minor response remains readable by
/// an older client.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcResponse<R> {
    /// Exact JSON-RPC version marker.
    pub jsonrpc: JsonRpcVersion,
    /// Request identity being answered.
    pub id: String,
    /// Typed method result.
    pub result: R,
    #[serde(default, rename = "error", skip_serializing)]
    reserved_error: Option<ForbiddenResponseField>,
}

impl<R> JsonRpcResponse<R> {
    /// Constructs a successful response for a request identity.
    #[must_use]
    pub fn new(id: impl Into<String>, result: R) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id: id.into(),
            result,
            reserved_error: None,
        }
    }
}

/// Failed JSON-RPC response with a stable ae-sdd error object.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcErrorResponse {
    /// Exact JSON-RPC version marker.
    pub jsonrpc: JsonRpcVersion,
    /// Request identity being answered.
    pub id: String,
    /// Stable, redacted machine-readable failure.
    pub error: RpcErrorObject,
    #[serde(default, rename = "result", skip_serializing)]
    reserved_result: Option<ForbiddenResponseField>,
}

impl JsonRpcErrorResponse {
    /// Constructs an error response for a request identity.
    #[must_use]
    pub fn new(id: impl Into<String>, error: RpcErrorObject) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id: id.into(),
            error,
            reserved_result: None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ForbiddenResponseField;

impl<'de> Deserialize<'de> for ForbiddenResponseField {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(de::Error::custom(
            "JSON-RPC response cannot contain both result and error",
        ))
    }
}

/// Auditable user confirmation reference for a protected operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmationRef {
    /// Stable confirmation identity or digest.
    pub confirmation_id: String,
    /// Actor identity that explicitly approved the action.
    pub approved_by: String,
    /// RFC3339 timestamp recorded by the confirmation source.
    pub approved_at: String,
}

/// Strict post-handshake request context with a method-specific payload.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestParams<P> {
    /// Exact negotiated protocol version, such as `1.0`.
    pub protocol_version: String,
    /// Registered workspace identity when required by the method descriptor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// Stable Agent instance identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Trusted session identity when required by the method descriptor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Boot-signed session capability required by post-open session methods.
    ///
    /// The daemon verifies this proof against the active boot key and binds it
    /// to the supplied session, role, delegation, grant and expiry. Endpoint
    /// authentication alone never authorizes a session-scoped operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_token: Option<String>,
    /// Trusted turn identity for engaged Hook requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Explicit Work Item identity when required by the method descriptor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_item_id: Option<String>,
    /// Active writer lease identity when required by a typed operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,
    /// Active monotonic writer generation when a lease is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fencing_token: Option<u64>,
    /// Expected project-state revision for CAS-protected operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
    /// Semantic request key for retry-safe mutations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Auditable user approval for confirmation-protected operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<ConfirmationRef>,
    /// Caller's remaining deadline budget in milliseconds.
    pub deadline_ms: u64,
    /// Method-specific, versioned payload.
    pub payload: P,
}
