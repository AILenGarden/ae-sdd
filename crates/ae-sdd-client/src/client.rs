use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ae_sdd_protocol::{
    ClientKind, ENDPOINT_MANIFEST_SCHEMA_V1, EndpointManifest, HandshakeRequest, HandshakeResponse,
    JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, MAX_FRAME_BYTES, PROTOCOL_RANGE_V1,
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, SecretString,
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

    /// Calls `method`, and if the first attempt fails because the local
    /// runtime is unreachable (`EndpointManifest`/`DaemonUnavailable`), runs
    /// `ensure` exactly once and replays the same call exactly once.
    ///
    /// `ensure` is only invoked on that recovery branch; a `Remote`/`Protocol`/
    /// `OfflineCapabilityInvalid` error is returned immediately without
    /// triggering recovery. The replay reuses `params` verbatim (including
    /// its `idempotency_key`) — this method never mints a new request
    /// identity, so recovery does not turn one caller intent into two
    /// distinct daemon-visible requests.
    pub async fn call_with_ensure<R, F>(
        &self,
        method: RpcMethod,
        params: RequestParams<Value>,
        ensure: impl FnOnce() -> F,
    ) -> ClientResult<R>
    where
        R: DeserializeOwned,
        F: std::future::Future<Output = ClientResult<()>>,
    {
        let first_attempt = duplicate_params(&params);
        match self.call(method, first_attempt).await {
            Ok(result) => Ok(result),
            Err(error) if is_runtime_unavailable(&error) => {
                ensure().await?;
                self.call(method, params).await
            }
            Err(error) => Err(error),
        }
    }
}

fn is_runtime_unavailable(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::EndpointManifest | ClientError::DaemonUnavailable
    )
}

/// Clones a `RequestParams<Value>` field-by-field. `RequestParams` does not
/// derive `Clone` (it is a `ae-sdd-protocol` wire DTO), so `call_with_ensure`
/// needs an explicit copy for its first attempt while keeping the original
/// `params` — including its `idempotency_key` — available, unmodified, for
/// the replay after `ensure` succeeds.
fn duplicate_params(params: &RequestParams<Value>) -> RequestParams<Value> {
    RequestParams {
        protocol_version: params.protocol_version.clone(),
        workspace_id: params.workspace_id.clone(),
        agent_id: params.agent_id.clone(),
        session_id: params.session_id.clone(),
        capability_token: params.capability_token.clone(),
        turn_id: params.turn_id.clone(),
        work_item_id: params.work_item_id.clone(),
        lease_id: params.lease_id.clone(),
        fencing_token: params.fencing_token,
        expected_revision: params.expected_revision,
        idempotency_key: params.idempotency_key.clone(),
        confirmation: params.confirmation.clone(),
        deadline_ms: params.deadline_ms,
        payload: params.payload.clone(),
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

#[cfg(test)]
mod manifest_tests {
    use super::*;

    fn valid_manifest() -> EndpointManifest {
        EndpointManifest {
            schema_version: ENDPOINT_MANIFEST_SCHEMA_V1.to_owned(),
            pid: 4321,
            boot_id: "00000000-0000-0000-0000-0000000000aa".to_owned(),
            event_store_id: "00000000-0000-0000-0000-0000000000bb".to_owned(),
            endpoint: "ae-sdd-test-endpoint".to_owned(),
            endpoint_token: SecretString::new("token"),
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            daemon_version: "ae-sdd-daemon/test".to_owned(),
            policy_digest: "a".repeat(64),
            capability_key_id: "b".repeat(64),
            capability_public_key: "c".repeat(64),
            started_at: "2024-01-01T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn a_complete_manifest_validates() {
        assert!(validate_manifest(&valid_manifest()).is_ok());
    }

    /// `validate_manifest` gates every reconnect, so each clause is asserted
    /// individually: a single missing check would let an untrustworthy manifest
    /// through, and a whole-struct assertion could not tell them apart.
    #[test]
    fn each_manifest_clause_is_independently_required() {
        /// One labelled mutation that should break manifest validation.
        type ManifestMutation = (&'static str, Box<dyn Fn(&mut EndpointManifest)>);

        let mutations: Vec<ManifestMutation> = vec![
            (
                "unknown schema version",
                Box::new(|m: &mut EndpointManifest| m.schema_version = "v0".to_owned()),
            ),
            (
                "unsupported protocol range",
                Box::new(|m: &mut EndpointManifest| m.protocol_range = "9-9".to_owned()),
            ),
            (
                "non-uuid boot id",
                Box::new(|m: &mut EndpointManifest| m.boot_id = "not-a-uuid".to_owned()),
            ),
            (
                "non-uuid event store id",
                Box::new(|m: &mut EndpointManifest| m.event_store_id = "not-a-uuid".to_owned()),
            ),
            ("zero pid", Box::new(|m: &mut EndpointManifest| m.pid = 0)),
            (
                "empty endpoint",
                Box::new(|m: &mut EndpointManifest| m.endpoint = String::new()),
            ),
            (
                "empty endpoint token",
                Box::new(|m: &mut EndpointManifest| m.endpoint_token = SecretString::new("")),
            ),
            (
                "empty daemon version",
                Box::new(|m: &mut EndpointManifest| m.daemon_version = String::new()),
            ),
            (
                "policy digest not lower-hex",
                Box::new(|m: &mut EndpointManifest| m.policy_digest = "A".repeat(64)),
            ),
            (
                "policy digest wrong length",
                Box::new(|m: &mut EndpointManifest| m.policy_digest = "a".repeat(63)),
            ),
            (
                "capability key id not lower-hex",
                Box::new(|m: &mut EndpointManifest| m.capability_key_id = "zz".repeat(32)),
            ),
            (
                "capability public key not lower-hex",
                Box::new(|m: &mut EndpointManifest| {
                    m.capability_public_key = "C".repeat(64);
                }),
            ),
        ];

        for (label, mutate) in mutations {
            let mut manifest = valid_manifest();
            mutate(&mut manifest);
            assert!(
                matches!(
                    validate_manifest(&manifest),
                    Err(ClientError::EndpointManifest)
                ),
                "must reject: {label}"
            );
        }
    }

    #[tokio::test]
    async fn an_unreadable_or_malformed_manifest_file_is_an_endpoint_manifest_error() {
        let missing = std::env::temp_dir().join("ae-sdd-nonexistent-manifest-xyz.json");
        assert!(matches!(
            read_manifest(&missing).await,
            Err(ClientError::EndpointManifest)
        ));

        let malformed = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(malformed.path(), b"{not json").expect("write");
        assert!(matches!(
            read_manifest(malformed.path()).await,
            Err(ClientError::EndpointManifest)
        ));
    }
}

#[cfg(test)]
mod limits_tests {
    use ae_sdd_protocol::HandshakeLimits;

    use super::*;

    fn response_with(limits: HandshakeLimits) -> HandshakeResponse {
        HandshakeResponse {
            protocol_version: "1".to_owned(),
            boot_id: "00000000-0000-0000-0000-0000000000aa".to_owned(),
            event_store_id: "00000000-0000-0000-0000-0000000000bb".to_owned(),
            daemon_build: "ae-sdd-daemon/test".to_owned(),
            capabilities: Default::default(),
            policy_digest: "a".repeat(64),
            operation_schema_digest: "b".repeat(64),
            limits,
            capability_key_id: "c".repeat(64),
            capability_public_key: "d".repeat(64),
        }
    }

    fn sane_limits() -> HandshakeLimits {
        HandshakeLimits {
            max_frame_bytes: 1_048_576,
            max_agent_depth: 2,
            max_string_bytes: 65_536,
            max_collection_items: 128,
            max_deadline_ms: 30_000,
            hook_deadline_ms: 250,
            max_child_result_bytes: 65_536,
            max_child_summary_bytes: 4_096,
            max_context_projection_bytes: 65_536,
        }
    }

    #[test]
    fn sane_negotiated_limits_are_accepted() {
        assert!(valid_limits(&response_with(sane_limits())));
    }

    /// The client refuses to operate under limits it cannot honour. Several
    /// clauses are *relational* (a sub-budget must not exceed its parent), which
    /// is exactly the kind of rule a single happy-path test cannot protect.
    #[test]
    fn each_limit_clause_is_independently_enforced() {
        /// One labelled mutation that should break limit negotiation.
        type LimitMutation = (&'static str, Box<dyn Fn(&mut HandshakeLimits)>);

        let mutations: Vec<LimitMutation> = vec![
            (
                "zero frame bytes",
                Box::new(|l: &mut HandshakeLimits| l.max_frame_bytes = 0),
            ),
            (
                "frame bytes above the protocol ceiling",
                Box::new(|l: &mut HandshakeLimits| l.max_frame_bytes = MAX_FRAME_BYTES as u64 + 1),
            ),
            (
                "zero agent depth",
                Box::new(|l: &mut HandshakeLimits| l.max_agent_depth = 0),
            ),
            (
                "zero string bytes",
                Box::new(|l: &mut HandshakeLimits| l.max_string_bytes = 0),
            ),
            (
                "string bytes exceeding the frame",
                Box::new(|l: &mut HandshakeLimits| l.max_string_bytes = l.max_frame_bytes + 1),
            ),
            (
                "zero collection items",
                Box::new(|l: &mut HandshakeLimits| l.max_collection_items = 0),
            ),
            (
                "zero deadline",
                Box::new(|l: &mut HandshakeLimits| l.max_deadline_ms = 0),
            ),
            (
                "zero hook deadline",
                Box::new(|l: &mut HandshakeLimits| l.hook_deadline_ms = 0),
            ),
            (
                "hook deadline above the caller ceiling",
                Box::new(|l: &mut HandshakeLimits| l.hook_deadline_ms = l.max_deadline_ms + 1),
            ),
            (
                "zero child summary bytes",
                Box::new(|l: &mut HandshakeLimits| l.max_child_summary_bytes = 0),
            ),
            (
                "child summary exceeding the child result budget",
                Box::new(|l: &mut HandshakeLimits| {
                    l.max_child_summary_bytes = l.max_child_result_bytes + 1;
                }),
            ),
            (
                "child result exceeding the frame",
                Box::new(|l: &mut HandshakeLimits| {
                    l.max_child_result_bytes = l.max_frame_bytes + 1
                }),
            ),
            (
                "zero context projection bytes",
                Box::new(|l: &mut HandshakeLimits| l.max_context_projection_bytes = 0),
            ),
            (
                "context projection exceeding the frame",
                Box::new(|l: &mut HandshakeLimits| {
                    l.max_context_projection_bytes = l.max_frame_bytes + 1;
                }),
            ),
        ];

        for (label, mutate) in mutations {
            let mut limits = sane_limits();
            mutate(&mut limits);
            assert!(
                !valid_limits(&response_with(limits)),
                "must reject: {label}"
            );
        }
    }

    /// A wrong entry here means a call is sent without the capability gate the
    /// daemon expects, so the mapping is pinned rather than inferred.
    #[test]
    fn capability_requirements_are_pinned_per_method_family() {
        for method in [
            RpcMethod::HookUserPrompt,
            RpcMethod::HookPreTool,
            RpcMethod::HookPostTool,
            RpcMethod::HookStop,
        ] {
            assert_eq!(required_capability(method), Some("hook-fail-closed-v1"));
        }
        assert_eq!(
            required_capability(RpcMethod::EventsSubscribe),
            Some("event-cursor-v1")
        );
        for method in [
            RpcMethod::DelegationCreate,
            RpcMethod::DelegationStatus,
            RpcMethod::DelegationAccept,
            RpcMethod::DelegationReport,
            RpcMethod::DelegationCollect,
            RpcMethod::DelegationCancel,
        ] {
            assert_eq!(required_capability(method), Some("physical-delegation-v1"));
        }
        for method in [RpcMethod::ContextGet, RpcMethod::ContextProject] {
            assert_eq!(required_capability(method), Some("context-projection-v1"));
        }
        for method in [RpcMethod::CompactRequest, RpcMethod::CompactStatus] {
            assert_eq!(
                required_capability(method),
                Some("compact-ack-rehydrate-v1")
            );
        }
        for method in [
            RpcMethod::JobSubmit,
            RpcMethod::JobStatus,
            RpcMethod::JobCancel,
        ] {
            assert_eq!(
                required_capability(method),
                Some("bounded-job-scheduler-v1")
            );
        }
        // A method outside those families must not demand a capability.
        assert_eq!(required_capability(RpcMethod::RuntimeStatus), None);
        assert_eq!(required_capability(RpcMethod::RuntimeHandshake), None);
    }
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
        RpcMethod::CompactRequest | RpcMethod::CompactStatus => Some("compact-ack-rehydrate-v1"),
        RpcMethod::JobSubmit | RpcMethod::JobStatus | RpcMethod::JobCancel => {
            Some("bounded-job-scheduler-v1")
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
