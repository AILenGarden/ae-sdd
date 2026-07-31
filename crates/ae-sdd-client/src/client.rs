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
    /// Adapter this client speaks for, sent with every handshake so the
    /// reconnect that precedes each call re-establishes addressing on its own.
    adapter_id: Option<String>,
    transport: Arc<dyn ClientTransport>,
    timeout: Duration,
}

impl DaemonClient {
    /// Performs handshake, one connection-scoped prerequisite call, and the
    /// requested typed call on the same local connection.
    ///
    /// Host adapters use this to replay `host.register` before every host
    /// action because adapter identity is deliberately bound to the physical
    /// connection rather than trusted from request payload alone.
    pub async fn call_after<R: DeserializeOwned>(
        &self,
        prerequisite_method: RpcMethod,
        prerequisite_params: RequestParams<Value>,
        method: RpcMethod,
        params: RequestParams<Value>,
    ) -> ClientResult<R> {
        if prerequisite_method != RpcMethod::HostRegister || !is_host_followup(method) {
            return Err(ClientError::Protocol);
        }
        let manifest = read_manifest(&self.manifest_path).await?;
        validate_manifest(&manifest)?;
        let prerequisite_params =
            bind_host_credential(&manifest, prerequisite_method, prerequisite_params);
        let params = bind_host_credential(&manifest, method, params);
        let effective_timeout = self.timeout.min(Duration::from_millis(
            prerequisite_params
                .deadline_ms
                .min(params.deadline_ms)
                .max(1),
        ));
        let handshake_id = next_request_id("handshake");
        let prerequisite_id = next_request_id("prerequisite");
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
                adapter_id: self.adapter_id.clone(),
            },
        );
        let prerequisite = JsonRpcRequest::new(
            prerequisite_id.clone(),
            prerequisite_method,
            prerequisite_params,
        );
        let request = JsonRpcRequest::new(request_id.clone(), method, params);
        let payloads = vec![
            serde_json::to_vec(&handshake).map_err(|_| ClientError::Protocol)?,
            serde_json::to_vec(&prerequisite).map_err(|_| ClientError::Protocol)?,
            serde_json::to_vec(&request).map_err(|_| ClientError::Protocol)?,
        ];
        let responses = self
            .transport
            .exchange(&manifest.endpoint, &payloads, effective_timeout)
            .await?;
        if responses.len() != 3 {
            return Err(ClientError::Protocol);
        }
        let negotiated: HandshakeResponse = decode_response(&handshake_id, &responses[0])?;
        validate_handshake(&manifest, method, &negotiated)?;
        let _: Value = decode_response(&prerequisite_id, &responses[1])?;
        decode_response(&request_id, &responses[2])
    }

    /// Replays one connection-scoped Host sequence after ensuring a missing
    /// default runtime exactly once.
    ///
    /// Both requests retain their original idempotency keys. Remote and
    /// protocol failures are returned immediately because only endpoint or
    /// daemon unavailability authorizes runtime recovery.
    pub async fn call_after_with_ensure<R, F>(
        &self,
        prerequisite_method: RpcMethod,
        prerequisite_params: RequestParams<Value>,
        method: RpcMethod,
        params: RequestParams<Value>,
        ensure: impl FnOnce() -> F,
    ) -> ClientResult<R>
    where
        R: DeserializeOwned,
        F: std::future::Future<Output = ClientResult<()>>,
    {
        let first_prerequisite = duplicate_params(&prerequisite_params);
        let first_request = duplicate_params(&params);
        match self
            .call_after(
                prerequisite_method,
                first_prerequisite,
                method,
                first_request,
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(error) if is_runtime_unavailable(&error) => {
                ensure().await?;
                self.call_after(prerequisite_method, prerequisite_params, method, params)
                    .await
            }
            Err(error) => Err(error),
        }
    }

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
            adapter_id: None,
            transport,
            timeout,
        }
    }

    /// Names the adapter this client speaks for.
    ///
    /// Host adapters set this so each handshake carries their identity; other
    /// client kinds have no adapter to name and leave it unset.
    #[must_use]
    pub fn with_adapter_id(mut self, adapter_id: impl Into<String>) -> Self {
        self.adapter_id = Some(adapter_id.into());
        self
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
        let params = bind_host_credential(&manifest, method, params);
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
                adapter_id: self.adapter_id.clone(),
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
        validate_handshake(&manifest, method, &negotiated)?;
        decode_response(&request_id, &responses[1])
    }

    /// Reads one protected endpoint manifest snapshot for offline verification.
    pub async fn endpoint_manifest(&self) -> ClientResult<EndpointManifest> {
        read_manifest(&self.manifest_path).await
    }

    /// Performs `host.register` against the local daemon.
    ///
    /// The client binds the boot-scoped credential from the endpoint manifest
    /// in memory: `params.capability_token` is overwritten with the current
    /// boot's endpoint token, so callers must not supply `capabilityToken`
    /// themselves — any supplied value is discarded and never reaches the
    /// daemon. The secret is only ever held in process memory and placed into
    /// the request frame; it never appears in argv, stdout, stderr, or logs.
    pub async fn host_register(&self, params: RequestParams<Value>) -> ClientResult<Value> {
        self.call(RpcMethod::HostRegister, params).await
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

/// Binds the boot-scoped endpoint credential into `host.register` params.
///
/// The daemon requires `host.register` to carry the current boot's endpoint
/// token, and the harness security contract forbids agents from reading that
/// credential themselves — the client owns manifest/auth handling. The token
/// is injected here straight from the manifest snapshot that was re-read for
/// this call, so boot-token rotation is picked up automatically. The
/// overwrite is unconditional: a caller-supplied (possibly forged) value must
/// never reach the daemon. Other methods pass through unchanged.
fn bind_host_credential(
    manifest: &EndpointManifest,
    method: RpcMethod,
    mut params: RequestParams<Value>,
) -> RequestParams<Value> {
    if method == RpcMethod::HostRegister {
        params.capability_token = Some(manifest.endpoint_token.expose_secret().to_owned());
    }
    params
}

fn is_host_followup(method: RpcMethod) -> bool {
    matches!(
        method,
        RpcMethod::HostActionNext
            | RpcMethod::HostActionAck
            | RpcMethod::HostPressureReport
    )
}

fn validate_handshake(
    manifest: &EndpointManifest,
    method: RpcMethod,
    negotiated: &HandshakeResponse,
) -> ClientResult<()> {
    if negotiated.protocol_version != PROTOCOL_VERSION_V1
        || negotiated.boot_id != manifest.boot_id
        || negotiated.event_store_id != manifest.event_store_id
        || negotiated.policy_digest != manifest.policy_digest
        || negotiated.capability_key_id != manifest.capability_key_id
        || negotiated.capability_public_key != manifest.capability_public_key
        || negotiated.daemon_build.is_empty()
        || !is_lower_hex_digest(&negotiated.operation_schema_digest)
        || !valid_limits(negotiated)
        || required_capability(method)
            .is_some_and(|required| !negotiated.capabilities.contains(required))
    {
        return Err(ClientError::Protocol);
    }
    Ok(())
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
            remediation: response.error.data.remediation,
        });
    }
    let response: JsonRpcResponse<R> =
        serde_json::from_value(value).map_err(|_| ClientError::Protocol)?;
    if response.id != request_id {
        return Err(ClientError::Protocol);
    }
    Ok(response.result)
}

#[cfg(test)]
mod call_after_tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ae_sdd_protocol::HandshakeLimits;
    use serde_json::json;

    use super::*;

    #[test]
    fn rpc_error_decode_preserves_daemon_remediation() {
        let bytes = serde_json::to_vec(&json!({
            "jsonrpc":"2.0",
            "id":"request-1",
            "error":{
                "code":-32025,
                "message":"lifecycle authority requires confirmation",
                "data":{
                    "schemaVersion":"ae-sdd-error/v1",
                    "stableCode":"CONFIRMATION_REQUIRED",
                    "retryable":false,
                    "remediation":"provide lifecycle confirmation for binding lifecycle:abc",
                    "details":{}
                }
            }
        }))
        .expect("error response serializes");

        let error = decode_response::<Value>("request-1", &bytes)
            .expect_err("daemon error must remain an error");
        assert_eq!(
            error.remediation(),
            Some("provide lifecycle confirmation for binding lifecycle:abc")
        );
        assert!(error.to_string().contains("lifecycle:abc"));
    }

    struct OrderedTransport {
        methods: Mutex<Vec<String>>,
        handshake: HandshakeResponse,
    }

    impl ClientTransport for OrderedTransport {
        fn exchange<'a>(
            &'a self,
            _endpoint: &'a str,
            payloads: &'a [Vec<u8>],
            _timeout: Duration,
        ) -> Pin<Box<dyn Future<Output = ClientResult<Vec<Vec<u8>>>> + Send + 'a>> {
            Box::pin(async move {
                let mut responses = Vec::with_capacity(payloads.len());
                let mut methods = self.methods.lock().expect("method record lock");
                for (index, payload) in payloads.iter().enumerate() {
                    let request: Value =
                        serde_json::from_slice(payload).map_err(|_| ClientError::Protocol)?;
                    let id = request["id"].clone();
                    methods.push(
                        request["method"]
                            .as_str()
                            .ok_or(ClientError::Protocol)?
                            .to_owned(),
                    );
                    let result = match index {
                        0 => serde_json::to_value(&self.handshake)
                            .map_err(|_| ClientError::Protocol)?,
                        1 => json!({"adapterId":"adapter-1"}),
                        2 => json!({"actionId":"action-1"}),
                        _ => return Err(ClientError::Protocol),
                    };
                    responses.push(
                        serde_json::to_vec(&json!({
                            "jsonrpc":"2.0",
                            "id":id,
                            "result":result,
                        }))
                        .map_err(|_| ClientError::Protocol)?,
                    );
                }
                Ok(responses)
            })
        }
    }

    #[tokio::test]
    async fn host_registration_and_action_share_one_ordered_exchange() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let manifest = EndpointManifest {
            schema_version: ENDPOINT_MANIFEST_SCHEMA_V1.to_owned(),
            pid: 1,
            boot_id: "00000000-0000-0000-0000-0000000000aa".to_owned(),
            event_store_id: "00000000-0000-0000-0000-0000000000bb".to_owned(),
            endpoint: "test-endpoint".to_owned(),
            endpoint_token: SecretString::new("endpoint-token"),
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            daemon_version: "ae-sddd/test".to_owned(),
            policy_digest: "a".repeat(64),
            capability_key_id: "b".repeat(64),
            capability_public_key: "c".repeat(64),
            started_at: "2026-07-29T00:00:00Z".to_owned(),
        };
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).expect("manifest serializes"),
        )
        .expect("manifest writes");
        let transport = Arc::new(OrderedTransport {
            methods: Mutex::new(Vec::new()),
            handshake: HandshakeResponse {
                protocol_version: PROTOCOL_VERSION_V1.to_owned(),
                boot_id: manifest.boot_id.clone(),
                event_store_id: manifest.event_store_id.clone(),
                daemon_build: "ae-sddd/test".to_owned(),
                capabilities: ["physical-delegation-v1".to_owned()].into_iter().collect(),
                policy_digest: manifest.policy_digest.clone(),
                operation_schema_digest: "d".repeat(64),
                limits: HandshakeLimits {
                    max_frame_bytes: 1_048_576,
                    max_agent_depth: 2,
                    max_string_bytes: 65_536,
                    max_collection_items: 128,
                    max_deadline_ms: 30_000,
                    hook_deadline_ms: 250,
                    max_child_result_bytes: 65_536,
                    max_child_summary_bytes: 4_096,
                    max_context_projection_bytes: 65_536,
                },
                capability_key_id: manifest.capability_key_id.clone(),
                capability_public_key: manifest.capability_public_key.clone(),
            },
        });
        let client = DaemonClient::new(
            &manifest_path,
            ClientKind::HostAdapter,
            transport.clone(),
            Duration::from_secs(1),
        );
        let register = RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: None,
            agent_id: None,
            session_id: None,
            capability_token: Some("endpoint-token".to_owned()),
            turn_id: None,
            work_item_id: None,
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: Some("register-1".to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({"adapterId":"adapter-1","capabilities":["create","attest"]}),
        };
        let action = RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: None,
            agent_id: None,
            session_id: None,
            capability_token: Some("endpoint-token".to_owned()),
            turn_id: None,
            work_item_id: None,
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: Some("next-1".to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({"adapterId":"adapter-1"}),
        };

        let result: Value = client
            .call_after(
                RpcMethod::HostRegister,
                register,
                RpcMethod::HostActionNext,
                action,
            )
            .await
            .expect("registered host action succeeds");

        assert_eq!(result["actionId"], "action-1");
        assert_eq!(
            *transport.methods.lock().expect("method record lock"),
            ["runtime.handshake", "host.register", "host.action_next"]
        );
    }

    #[tokio::test]
    async fn call_after_rejects_a_non_host_prerequisite_before_dispatch() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let manifest = test_manifest("endpoint-token");
        write_manifest(&manifest_path, &manifest);
        let (client, transport) = recording_client(&manifest_path, &manifest);

        let error = client
            .call_after::<Value>(
                RpcMethod::WorkspaceRegister,
                register_params(None),
                RpcMethod::OperationExecute,
                action_params(),
            )
            .await
            .expect_err("call_after is reserved for connection-scoped host registration");

        assert!(matches!(error, ClientError::Protocol));
        assert!(transport.frames().is_empty());
    }

    struct RecoveringTransport {
        attempts: AtomicUsize,
        handshake: HandshakeResponse,
    }

    impl ClientTransport for RecoveringTransport {
        fn exchange<'a>(
            &'a self,
            _endpoint: &'a str,
            payloads: &'a [Vec<u8>],
            _timeout: Duration,
        ) -> Pin<Box<dyn Future<Output = ClientResult<Vec<Vec<u8>>>> + Send + 'a>> {
            Box::pin(async move {
                if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err(ClientError::DaemonUnavailable);
                }
                let results = [
                    serde_json::to_value(&self.handshake).map_err(|_| ClientError::Protocol)?,
                    json!({"adapterId":"adapter-1"}),
                    json!({"actionId":"action-1"}),
                ];
                payloads
                    .iter()
                    .zip(results)
                    .map(|(payload, result)| {
                        let request: Value =
                            serde_json::from_slice(payload).map_err(|_| ClientError::Protocol)?;
                        serde_json::to_vec(&json!({
                            "jsonrpc":"2.0",
                            "id":request["id"],
                            "result":result,
                        }))
                        .map_err(|_| ClientError::Protocol)
                    })
                    .collect()
            })
        }
    }

    #[tokio::test]
    async fn call_after_with_ensure_recovers_once_and_replays_once() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let manifest = test_manifest("endpoint-token");
        write_manifest(&manifest_path, &manifest);
        let transport = Arc::new(RecoveringTransport {
            attempts: AtomicUsize::new(0),
            handshake: RecordingTransport::new(&manifest).handshake,
        });
        let client = DaemonClient::new(
            &manifest_path,
            ClientKind::HostAdapter,
            transport.clone(),
            Duration::from_secs(1),
        );
        let ensure_calls = AtomicUsize::new(0);

        let result: Value = client
            .call_after_with_ensure(
                RpcMethod::HostRegister,
                register_params(None),
                RpcMethod::HostActionNext,
                action_params(),
                || async {
                    ensure_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
            .await
            .expect("runtime recovery replays the host sequence");

        assert_eq!(result["actionId"], "action-1");
        assert_eq!(ensure_calls.load(Ordering::SeqCst), 1);
        assert_eq!(transport.attempts.load(Ordering::SeqCst), 2);
    }

    /// Records every request frame it receives so tests can assert exactly
    /// what the client put on the wire, including injected credentials.
    struct RecordingTransport {
        frames: Mutex<Vec<Value>>,
        handshake: HandshakeResponse,
    }

    impl RecordingTransport {
        fn new(manifest: &EndpointManifest) -> Self {
            Self {
                frames: Mutex::new(Vec::new()),
                handshake: HandshakeResponse {
                    protocol_version: PROTOCOL_VERSION_V1.to_owned(),
                    boot_id: manifest.boot_id.clone(),
                    event_store_id: manifest.event_store_id.clone(),
                    daemon_build: "ae-sdd-daemon/test".to_owned(),
                    capabilities: Default::default(),
                    policy_digest: manifest.policy_digest.clone(),
                    operation_schema_digest: "d".repeat(64),
                    limits: HandshakeLimits {
                        max_frame_bytes: 1_048_576,
                        max_agent_depth: 2,
                        max_string_bytes: 65_536,
                        max_collection_items: 128,
                        max_deadline_ms: 30_000,
                        hook_deadline_ms: 250,
                        max_child_result_bytes: 65_536,
                        max_child_summary_bytes: 4_096,
                        max_context_projection_bytes: 65_536,
                    },
                    capability_key_id: manifest.capability_key_id.clone(),
                    capability_public_key: manifest.capability_public_key.clone(),
                },
            }
        }

        fn frames(&self) -> Vec<Value> {
            self.frames.lock().expect("frame record lock").clone()
        }
    }

    impl ClientTransport for RecordingTransport {
        fn exchange<'a>(
            &'a self,
            _endpoint: &'a str,
            payloads: &'a [Vec<u8>],
            _timeout: Duration,
        ) -> Pin<Box<dyn Future<Output = ClientResult<Vec<Vec<u8>>>> + Send + 'a>> {
            Box::pin(async move {
                let mut responses = Vec::with_capacity(payloads.len());
                let mut frames = self.frames.lock().expect("frame record lock");
                for (index, payload) in payloads.iter().enumerate() {
                    let request: Value =
                        serde_json::from_slice(payload).map_err(|_| ClientError::Protocol)?;
                    let id = request["id"].clone();
                    frames.push(request);
                    let result = match index {
                        0 => serde_json::to_value(&self.handshake)
                            .map_err(|_| ClientError::Protocol)?,
                        1 => json!({"adapterId":"adapter-1"}),
                        2 => json!({"actionId":"action-1"}),
                        _ => return Err(ClientError::Protocol),
                    };
                    responses.push(
                        serde_json::to_vec(&json!({
                            "jsonrpc":"2.0",
                            "id":id,
                            "result":result,
                        }))
                        .map_err(|_| ClientError::Protocol)?,
                    );
                }
                Ok(responses)
            })
        }
    }

    fn test_manifest(endpoint_token: &str) -> EndpointManifest {
        EndpointManifest {
            schema_version: ENDPOINT_MANIFEST_SCHEMA_V1.to_owned(),
            pid: 4321,
            boot_id: "00000000-0000-0000-0000-0000000000aa".to_owned(),
            event_store_id: "00000000-0000-0000-0000-0000000000bb".to_owned(),
            endpoint: "test-endpoint".to_owned(),
            endpoint_token: SecretString::new(endpoint_token),
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            daemon_version: "ae-sdd-daemon/test".to_owned(),
            policy_digest: "a".repeat(64),
            capability_key_id: "b".repeat(64),
            capability_public_key: "c".repeat(64),
            started_at: "2026-07-29T00:00:00Z".to_owned(),
        }
    }

    fn write_manifest(path: &Path, manifest: &EndpointManifest) {
        std::fs::write(
            path,
            serde_json::to_vec(manifest).expect("manifest serializes"),
        )
        .expect("manifest writes");
    }

    fn register_params(capability_token: Option<&str>) -> RequestParams<Value> {
        RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: None,
            agent_id: None,
            session_id: None,
            capability_token: capability_token.map(str::to_owned),
            turn_id: None,
            work_item_id: None,
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: Some("register-1".to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({"adapterId":"adapter-1","capabilities":["create","attest"]}),
        }
    }

    fn action_params() -> RequestParams<Value> {
        RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: None,
            agent_id: None,
            session_id: None,
            capability_token: None,
            turn_id: None,
            work_item_id: None,
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: Some("next-1".to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({"adapterId":"adapter-1"}),
        }
    }

    fn recording_client(
        manifest_path: &Path,
        manifest: &EndpointManifest,
    ) -> (DaemonClient, Arc<RecordingTransport>) {
        let transport = Arc::new(RecordingTransport::new(manifest));
        let client = DaemonClient::new(
            manifest_path,
            ClientKind::HostAdapter,
            transport.clone(),
            Duration::from_secs(1),
        );
        (client, transport)
    }

    /// A caller that omits `capabilityToken` (as the security contract
    /// demands) still satisfies the daemon: the client binds the manifest's
    /// boot-scoped endpoint token into the register frame itself.
    #[tokio::test]
    async fn host_register_binds_the_boot_scoped_endpoint_token() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let manifest = test_manifest("endpoint-token");
        write_manifest(&manifest_path, &manifest);
        let (client, transport) = recording_client(&manifest_path, &manifest);

        let result = client
            .host_register(register_params(None))
            .await
            .expect("host register succeeds");

        assert_eq!(result["adapterId"], "adapter-1");
        let frames = transport.frames();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1]["method"], "host.register");
        assert_eq!(frames[1]["params"]["capabilityToken"], "endpoint-token");
    }

    /// A caller-supplied credential is untrusted input: the client must
    /// overwrite it so a forged token never reaches the daemon.
    #[tokio::test]
    async fn a_forged_capability_token_is_overwritten_with_the_manifest_token() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let manifest = test_manifest("endpoint-token");
        write_manifest(&manifest_path, &manifest);
        let (client, transport) = recording_client(&manifest_path, &manifest);

        client
            .call::<Value>(RpcMethod::HostRegister, register_params(Some("forged")))
            .await
            .expect("host register succeeds");

        let frames = transport.frames();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1]["params"]["capabilityToken"], "endpoint-token");
        let raw_frame = serde_json::to_string(&frames[1]).expect("frame serializes");
        assert!(!raw_frame.contains("forged"));
    }

    /// `call_after` replays `host.register` as the connection-scoped
    /// prerequisite before every host action, so the injection must apply to
    /// the prerequisite frame as well.
    #[tokio::test]
    async fn call_after_binds_the_token_into_the_register_prerequisite() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let manifest = test_manifest("endpoint-token");
        write_manifest(&manifest_path, &manifest);
        let (client, transport) = recording_client(&manifest_path, &manifest);

        let result: Value = client
            .call_after(
                RpcMethod::HostRegister,
                register_params(None),
                RpcMethod::HostActionNext,
                action_params(),
            )
            .await
            .expect("registered host action succeeds");

        assert_eq!(result["actionId"], "action-1");
        let frames = transport.frames();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[1]["method"], "host.register");
        assert_eq!(frames[1]["params"]["capabilityToken"], "endpoint-token");
    }

    /// The manifest is re-read on every call, so a rotated boot token is
    /// picked up without rebuilding the client.
    #[tokio::test]
    async fn a_rotated_boot_token_is_used_on_the_next_call() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let manifest = test_manifest("boot-token-1");
        write_manifest(&manifest_path, &manifest);
        let (client, transport) = recording_client(&manifest_path, &manifest);

        client
            .host_register(register_params(None))
            .await
            .expect("first host register succeeds");
        // Only the token rotates; boot identity stays identical so the stub
        // handshake response still validates against the manifest.
        write_manifest(&manifest_path, &test_manifest("boot-token-2"));
        client
            .host_register(register_params(None))
            .await
            .expect("second host register succeeds");

        let frames = transport.frames();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[1]["params"]["capabilityToken"], "boot-token-1");
        assert_eq!(frames[3]["params"]["capabilityToken"], "boot-token-2");
    }

    /// The endpoint token is a secret: the error path must never surface it,
    /// even when the manifest carrying it is the reason the call failed.
    #[tokio::test]
    async fn a_manifest_error_never_exposes_the_endpoint_token() {
        let directory = tempfile::tempdir().expect("temporary manifest directory");
        let manifest_path = directory.path().join("endpoint.v1.json");
        let mut manifest = test_manifest("super-secret-endpoint-token");
        manifest.pid = 0;
        write_manifest(&manifest_path, &manifest);
        let (client, _) = recording_client(&manifest_path, &manifest);

        let error = client
            .host_register(register_params(None))
            .await
            .expect_err("an invalid manifest is rejected");

        assert!(matches!(error, ClientError::EndpointManifest));
        assert!(!error.to_string().contains("super-secret-endpoint-token"));
        assert!(!format!("{error:?}").contains("super-secret-endpoint-token"));
    }
}
