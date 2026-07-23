use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ae_sdd_protocol::{
    ClientKind, ENDPOINT_MANIFEST_SCHEMA_V1, EndpointManifest, HandshakeRequest,
    HandshakeResponse, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, MAX_FRAME_BYTES,
    PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, SecretString,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{ClientError, ClientResult, ClientTransport};

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Thin authenticated daemon client.
pub struct DaemonClient {
    manifest_path: PathBuf,
    client_kind: ClientKind,
    transport: Arc<dyn ClientTransport>,
    timeout: Duration,
}

impl DaemonClient {
    /// Creates a client over an injectable local transport.
    #[must_use]
    pub fn new(
        manifest_path: impl Into<PathBuf>,
        client_kind: ClientKind,
        transport: Arc<dyn ClientTransport>,
        timeout: Duration,
    ) -> Self {
        Self {
            manifest_path: manifest_path.into(),
            client_kind,
            transport,
            timeout,
        }
    }

    /// Endpoint manifest path used for each atomic reconnect snapshot.
    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Performs handshake and one typed call on the same local connection.
    pub async fn call<R: DeserializeOwned>(
        &self,
        method: RpcMethod,
        params: RequestParams<Value>,
    ) -> ClientResult<R> {
        let manifest = read_manifest(&self.manifest_path).await?;
        validate_manifest(&manifest)?;
        let effective_timeout = self
            .timeout
            .min(Duration::from_millis(params.deadline_ms.max(1)));
        let handshake_id = next_request_id("handshake");
        let request_id = next_request_id("request");
        let handshake = JsonRpcRequest::new(
            handshake_id.clone(),
            RpcMethod::RuntimeHandshake,
            HandshakeRequest {
                protocol_range: PROTOCOL_RANGE_V1.to_owned(),
                client_build: crate::CLIENT_BUILD.to_owned(),
                client_kind: self.client_kind,
                endpoint_token: SecretString::new(manifest.endpoint_token.expose_secret()),
                expected_boot_id: manifest.boot_id.clone(),
                expected_policy_digest: manifest.policy_digest.clone(),
            },
        );
        let request = JsonRpcRequest::new(request_id.clone(), method, params);
        let payloads = vec![
            serde_json::to_vec(&handshake).map_err(|_| ClientError::Protocol)?,
            serde_json::to_vec(&request).map_err(|_| ClientError::Protocol)?,
        ];
        let responses = self
            .transport
            .exchange(&manifest.endpoint, &payloads, effective_timeout)
            .await?;
        if responses.len() != 2 {
            return Err(ClientError::Protocol);
        }
        let negotiated: HandshakeResponse = decode_response(&handshake_id, &responses[0])?;
        if negotiated.protocol_version != PROTOCOL_VERSION_V1
            || negotiated.boot_id != manifest.boot_id
            || negotiated.event_store_id != manifest.event_store_id
            || negotiated.policy_digest != manifest.policy_digest
            || negotiated.capability_key_id != manifest.capability_key_id
            || negotiated.capability_public_key != manifest.capability_public_key
            || negotiated.daemon_build.is_empty()
            || !is_lower_hex_digest(&negotiated.operation_schema_digest)
            || !valid_limits(&negotiated)
            || required_capability(method)
                .is_some_and(|required| !negotiated.capabilities.contains(required))
        {
            return Err(ClientError::Protocol);
        }
        decode_response(&request_id, &responses[1])
    }

    /// Reads one protected endpoint manifest snapshot for offline verification.
    pub async fn endpoint_manifest(&self) -> ClientResult<EndpointManifest> {
        read_manifest(&self.manifest_path).await
    }
}

async fn read_manifest(path: &Path) -> ClientResult<EndpointManifest> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| ClientError::EndpointManifest)?;
    serde_json::from_slice(&bytes).map_err(|_| ClientError::EndpointManifest)
}

fn validate_manifest(manifest: &EndpointManifest) -> ClientResult<()> {
    let identity_is_valid = uuid::Uuid::parse_str(&manifest.boot_id).is_ok()
        && uuid::Uuid::parse_str(&manifest.event_store_id).is_ok();
    if manifest.schema_version != ENDPOINT_MANIFEST_SCHEMA_V1
        || manifest.protocol_range != PROTOCOL_RANGE_V1
        || !identity_is_valid
        || manifest.pid == 0
        || manifest.endpoint.is_empty()
        || manifest.endpoint_token.expose_secret().is_empty()
        || manifest.daemon_version.is_empty()
        || !is_lower_hex_digest(&manifest.policy_digest)
        || !is_lower_hex_digest(&manifest.capability_key_id)
        || !is_lower_hex_digest(&manifest.capability_public_key)
    {
        return Err(ClientError::EndpointManifest);
    }
    Ok(())
}

fn valid_limits(response: &HandshakeResponse) -> bool {
    let limits = &response.limits;
    (1..=MAX_FRAME_BYTES as u64).contains(&limits.max_frame_bytes)
        && limits.max_agent_depth > 0
        && limits.max_string_bytes > 0
        && limits.max_string_bytes <= limits.max_frame_bytes
        && limits.max_collection_items > 0
        && limits.max_deadline_ms > 0
        && (1..=limits.max_deadline_ms).contains(&limits.hook_deadline_ms)
        && limits.max_child_summary_bytes > 0
        && limits.max_child_summary_bytes <= limits.max_child_result_bytes
        && limits.max_child_result_bytes <= limits.max_frame_bytes
        && limits.max_context_projection_bytes > 0
        && limits.max_context_projection_bytes <= limits.max_frame_bytes
}

fn required_capability(method: RpcMethod) -> Option<&'static str> {
    match method {
        RpcMethod::HookUserPrompt
        | RpcMethod::HookPreTool
        | RpcMethod::HookPostTool
        | RpcMethod::HookStop => Some("hook-fail-closed-v1"),
        RpcMethod::EventsSubscribe => Some("event-cursor-v1"),
        RpcMethod::DelegationCreate
        | RpcMethod::DelegationStatus
        | RpcMethod::DelegationAccept
        | RpcMethod::DelegationReport
        | RpcMethod::DelegationCollect
        | RpcMethod::DelegationCancel => Some("physical-delegation-v1"),
        RpcMethod::ContextGet | RpcMethod::ContextProject => Some("context-projection-v1"),
        RpcMethod::CompactRequest | RpcMethod::CompactStatus => {
            Some("compact-ack-rehydrate-v1")
        }
        _ => None,
    }
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn next_request_id(prefix: &str) -> String {
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{sequence}")
}

fn decode_response<R: DeserializeOwned>(request_id: &str, bytes: &[u8]) -> ClientResult<R> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ClientError::Protocol)?;
    if value.get("error").is_some() {
        let response: JsonRpcErrorResponse =
            serde_json::from_value(value).map_err(|_| ClientError::Protocol)?;
        if response.id != request_id {
            return Err(ClientError::Protocol);
        }
        return Err(ClientError::Remote {
            code: response.error.data.stable_code,
            message: response.error.message,
        });
    }
    let response: JsonRpcResponse<R> =
        serde_json::from_value(value).map_err(|_| ClientError::Protocol)?;
    if response.id != request_id {
        return Err(ClientError::Protocol);
    }
    Ok(response.result)
}
