use std::collections::{BTreeMap, VecDeque};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use ae_sdd_context::PressureDecision;
use ae_sdd_domain::{
    AgentRole, BootId, CapabilityId, DelegationId, EventStoreId, GateOutcome, SessionId,
};
use ae_sdd_host::{BootCapabilitySigner, CapabilityClaims, CapabilityToken, GrantDigest};
use ae_sdd_policy::{HookAction, HookPoint, HookPolicy};
use ae_sdd_protocol::{
    ClientKind, GateOutcomeKind, HandshakeLimits, HandshakeRequest, HandshakeResponse,
    HookDecision, JobStatus, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, OperationScope,
    PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1, RequestParams, RpcErrorObject, RpcMethod,
    StableErrorCode, WorkspaceMode,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    BusinessOperationPort, BusinessWorkspace, ClockPort, CompactRequestPayload, CompactResult,
    ContextCache, ContextProjectPayload, DaemonLifecycle, DelegationAcceptPayload,
    DelegationCreatePayload, DelegationReportPayload, DelegationSupervisor, DurableEvent,
    EventBatch, EventSubscriptionPayload, FlowSupervisor, HookPayload, HookResult, HostAckPayload,
    HostCoordinator, HostPressurePayload, HostRegisterPayload, IdempotencyReceipt, PersistencePort,
    RuntimeConfig, RuntimeError, RuntimeResult, RuntimeStatus, SessionOpenPayload, SessionResult,
    WireAgentRole, WorkItemActors, WorkspaceModeTransitionPayload, WorkspaceParityEvidence,
    WorkspaceRegisterPayload, WorkspaceResolverPort, WorkspaceResult,
};

#[path = "service_hook_context.rs"]
mod hook_context;
#[path = "service_host.rs"]
mod host_methods;
#[path = "service_jobs.rs"]
mod job_methods;
#[path = "service_lifecycle.rs"]
mod lifecycle;
#[path = "service_protocol.rs"]
mod protocol_methods;
#[path = "service_sessions.rs"]
mod session_methods;
#[path = "service_support.rs"]
mod support;
#[path = "service_workspace.rs"]
mod workspace_methods;

/// Per-connection protocol state owned by the local IPC server.
#[derive(Clone, Debug, Default)]
pub struct ConnectionState {
    handshaken: bool,
    client_kind: Option<ClientKind>,
    adapter_id: Option<String>,
}

impl ConnectionState {
    /// Whether endpoint authentication and protocol negotiation succeeded.
    #[must_use]
    pub const fn is_handshaken(&self) -> bool {
        self.handshaken
    }
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
struct WorkspaceRecord {
    result: WorkspaceResult,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionRecord {
    workspace_id: String,
    agent_id: String,
    external_key: String,
    current_work_item: Option<String>,
    result: SessionResult,
    delegation_id: Option<String>,
    current_turn_id: Option<String>,
    current_turn_seq: u64,
    active: bool,
}

#[derive(Default)]
struct RuntimeState {
    workspaces: BTreeMap<String, WorkspaceRecord>,
    workspace_by_root: BTreeMap<String, String>,
    sessions: BTreeMap<String, SessionRecord>,
    session_by_external: BTreeMap<(String, String), String>,
    jobs: BTreeMap<String, JobRecord>,
    job_queue: VecDeque<String>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct JobRecord {
    job_id: String,
    workspace: BusinessWorkspaceWire,
    work_item_id: Option<String>,
    entrypoint: String,
    arguments: Value,
    deadline_unix_ms: u64,
    status: JobStatus,
    result: Option<Value>,
    error_code: Option<StableErrorCode>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BusinessWorkspaceWire {
    workspace_id: String,
    canonical_root: String,
    project_key: String,
    mode: WorkspaceMode,
    inventory_generation: u64,
}

struct Admission<'a> {
    count: &'a AtomicUsize,
}

impl Drop for Admission<'_> {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Stateful daemon application service, independent of transport and adapters.
pub struct RuntimeService {
    config: RuntimeConfig,
    boot_id: BootId,
    endpoint_token: String,
    capability_signer: BootCapabilitySigner,
    persistence: Arc<dyn PersistencePort>,
    clock: Arc<dyn ClockPort>,
    resolver: Arc<dyn WorkspaceResolverPort>,
    business: Arc<dyn BusinessOperationPort>,
    lifecycle: RwLock<DaemonLifecycle>,
    state: Mutex<RuntimeState>,
    admitted: AtomicUsize,
    actors: WorkItemActors,
    context: Arc<ContextCache>,
    host: Arc<HostCoordinator>,
    delegation: DelegationSupervisor,
    flow: FlowSupervisor,
}

impl RuntimeService {
    /// Creates one daemon boot over injected platform and durable ports.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: RuntimeConfig,
        boot_id: BootId,
        endpoint_token: impl Into<String>,
        persistence: Arc<dyn PersistencePort>,
        clock: Arc<dyn ClockPort>,
        resolver: Arc<dyn WorkspaceResolverPort>,
        business: Arc<dyn BusinessOperationPort>,
    ) -> Self {
        let capability_signer = BootCapabilitySigner::generate(boot_id);
        let actors = WorkItemActors::new(
            config.work_item_mailbox_capacity,
            config.max_work_item_actors,
            config.max_work_item_actors_per_workspace,
            config.work_item_actor_idle_ms,
        );
        let context = Arc::new(ContextCache::new(config.max_context_projection_bytes));
        let host = Arc::new(HostCoordinator::new(Arc::clone(&persistence)));
        let delegation = DelegationSupervisor::new(Arc::clone(&persistence), Arc::clone(&host));
        let flow = FlowSupervisor::new(Arc::clone(&persistence));
        Self {
            config,
            boot_id,
            endpoint_token: endpoint_token.into(),
            capability_signer,
            persistence,
            clock,
            resolver,
            business,
            lifecycle: RwLock::new(DaemonLifecycle::Running),
            state: Mutex::new(RuntimeState::default()),
            admitted: AtomicUsize::new(0),
            actors,
            context,
            host,
            delegation,
            flow,
        }
    }

    /// Returns the daemon boot identity.
    #[must_use]
    pub const fn boot_id(&self) -> BootId {
        self.boot_id
    }

    /// Returns the durable event-store epoch.
    pub fn event_store_id(&self) -> RuntimeResult<EventStoreId> {
        self.persistence.event_store_id()
    }

    /// Returns the boot-scoped offline verification key metadata.
    #[must_use]
    pub fn capability_key(&self) -> (String, String) {
        let key = self.capability_signer.public_key();
        (key.key_id().to_owned(), key.public_key_hex())
    }

    /// Returns the active policy digest.
    #[must_use]
    pub fn policy_digest(&self) -> &str {
        &self.config.policy_digest
    }

    /// Restores durable workspace and external-session indexes for a new boot.
    ///
    /// Recovered sessions are intentionally inactive: callers must reopen so
    /// the daemon can issue a capability signed by the current boot key.
    pub fn recover(&self) -> RuntimeResult<()> {
        let workspace_values = self.persistence.list_records("workspace/v1")?;
        let session_values = self.persistence.list_records("session/v1")?;
        let mut state = self.lock_state()?;
        state.workspaces.clear();
        state.workspace_by_root.clear();
        state.sessions.clear();
        state.session_by_external.clear();
        state.jobs.clear();
        state.job_queue.clear();
        for (key, value) in workspace_values {
            let result: WorkspaceResult = decode_value(value)?;
            if key != result.workspace_id || state.workspaces.len() >= self.config.max_workspaces {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "durable workspace projection is inconsistent or exceeds capacity",
                ));
            }
            state
                .workspace_by_root
                .insert(result.canonical_root.clone(), key.clone());
            state.workspaces.insert(key, WorkspaceRecord { result });
        }
        for (key, value) in session_values {
            let mut record: SessionRecord = decode_value(value)?;
            if key != record.result.session_id
                || !state.workspaces.contains_key(&record.workspace_id)
                || state.sessions.len() >= self.config.max_sessions
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "durable session projection is inconsistent or exceeds capacity",
                ));
            }
            record.active = false;
            record.result.capability_token.clear();
            state.session_by_external.insert(
                (record.workspace_id.clone(), record.external_key.clone()),
                key.clone(),
            );
            state.sessions.insert(key, record);
        }
        for (key, value) in self.persistence.list_records("job/v1")? {
            let mut record: JobRecord = decode_value(value)?;
            if key != record.job_id || state.jobs.len() >= self.config.max_jobs {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "durable job projection is inconsistent or exceeds capacity",
                ));
            }
            let normalized = matches!(record.status, JobStatus::Queued | JobStatus::Running);
            if normalized {
                record.status = JobStatus::Queued;
                state.job_queue.push_back(key.clone());
            }
            if normalized {
                self.persistence
                    .store_record("job/v1", &key, &to_value(&record)?)?;
            }
            state.jobs.insert(key, record);
        }
        drop(state);
        self.host.recover()?;
        Ok(())
    }

    /// Handles one unframed JSON-RPC payload and returns one unframed response.
    ///
    /// The transport owns the four-byte frame and invokes this method for each
    /// payload on the same connection.
    #[must_use]
    pub fn handle_payload(&self, connection: &mut ConnectionState, payload: &[u8]) -> Vec<u8> {
        let request_id = serde_json::from_slice::<Value>(payload)
            .ok()
            .and_then(|value| value.get("id").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        match self.handle_payload_inner(connection, payload) {
            Ok(response) => response,
            Err(error) => serialize_error(&request_id, error),
        }
    }

    fn handle_payload_inner(
        &self,
        connection: &mut ConnectionState,
        payload: &[u8],
    ) -> RuntimeResult<Vec<u8>> {
        if payload.len() > self.config.max_frame_bytes {
            return Err(RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "JSON-RPC payload exceeds the negotiated frame limit",
            ));
        }
        let request: JsonRpcRequest<Value> = serde_json::from_slice(payload).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "request is not a strict registered JSON-RPC envelope",
            )
        })?;

        if !connection.handshaken {
            if request.method != RpcMethod::RuntimeHandshake {
                return Err(RuntimeError::new(
                    StableErrorCode::HandshakeRequired,
                    "runtime.handshake must be the first request on a connection",
                ));
            }
            let handshake: HandshakeRequest = decode_value(request.params)?;
            let response = self.handshake(&handshake)?;
            connection.handshaken = true;
            connection.client_kind = Some(handshake.client_kind);
            return encode_success(request.id, response);
        }

        if request.method == RpcMethod::RuntimeHandshake {
            let handshake: HandshakeRequest = decode_value(request.params)?;
            let response = self.handshake(&handshake)?;
            return encode_success(request.id, response);
        }

        let params: RequestParams<Value> = decode_value(request.params)?;
        self.validate_request(request.method, &params)?;
        authorize_client_kind(connection.client_kind, request.method)?;
        authorize_host_connection(connection, request.method, &params)?;
        if requires_session_capability(request.method) {
            let identity = self.session_identity(&params, is_hook(request.method))?;
            if !capability_allows(&identity.capability_id, request.method) {
                return Err(RuntimeError::new(
                    StableErrorCode::RoleOperationForbidden,
                    "session capability does not authorize this RPC method",
                ));
            }
        }
        let _admission = self.admit()?;
        let result = self.dispatch(request.method, &params, connection.client_kind)?;
        if request.method == RpcMethod::HostRegister {
            connection.adapter_id = params
                .payload
                .get("adapterId")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        encode_success(request.id, result)
    }

    fn dispatch(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        client_kind: Option<ClientKind>,
    ) -> RuntimeResult<Value> {
        match method {
            RpcMethod::RuntimeHandshake => unreachable!("handshake handled before dispatch"),
            RpcMethod::RuntimeStatus => to_value(self.status()?),
            RpcMethod::RuntimeDrain => self.runtime_drain(params),
            RpcMethod::WorkspaceRegister => self.workspace_register(params),
            RpcMethod::WorkspaceModeTransition => {
                self.workspace_mode_transition(params, client_kind)
            }
            RpcMethod::WorkspaceSnapshot => self.workspace_snapshot(params),
            RpcMethod::SessionOpen => self.session_open(params),
            RpcMethod::SessionHeartbeat => self.session_heartbeat(params),
            RpcMethod::SessionClose => self.session_close(params),
            RpcMethod::HookUserPrompt
            | RpcMethod::HookPreTool
            | RpcMethod::HookPostTool
            | RpcMethod::HookStop => self.hook(method, params),
            RpcMethod::EventsSubscribe => self.events_subscribe(params),
            RpcMethod::ContextGet => self.context_get(params),
            RpcMethod::ContextProject => self.context_project(params),
            RpcMethod::HostRegister => self.host_register(params),
            RpcMethod::HostCapabilities => self.host_capabilities(params),
            RpcMethod::HostActionNext => self.host_action_next(params),
            RpcMethod::HostActionAck => self.host_action_ack(params),
            RpcMethod::HostPressureReport => self.host_pressure(params),
            RpcMethod::DelegationCreate => self.delegation_create(params),
            RpcMethod::DelegationStatus => self.delegation_status(params),
            RpcMethod::DelegationAccept => self.delegation_accept(params),
            RpcMethod::DelegationReport => self.delegation_report(params),
            RpcMethod::DelegationCollect => self.delegation_collect(params),
            RpcMethod::DelegationCancel => self.delegation_cancel(params),
            RpcMethod::CompactRequest => self.compact_request(params),
            RpcMethod::CompactStatus => self.compact_status(params),
            RpcMethod::FlowSnapshot
            | RpcMethod::FlowNext
            | RpcMethod::OperationDescribe
            | RpcMethod::OperationExecute
            | RpcMethod::GateEvaluate => self.authoritative_business(method, params),
            RpcMethod::JobSubmit => self.job_submit(params),
            RpcMethod::JobStatus => self.job_status(params),
            RpcMethod::JobCancel => self.job_cancel(params),
        }
    }

    /// Exposes the flow supervisor to authoritative operation adapters.
    #[must_use]
    pub const fn flow_supervisor(&self) -> &FlowSupervisor {
        &self.flow
    }

    /// Exposes the delegation supervisor to verifier/cleaner integrations.
    #[must_use]
    pub const fn delegation_supervisor(&self) -> &DelegationSupervisor {
        &self.delegation
    }
}

#[derive(Clone, Debug)]
struct TrustedSession {
    workspace_id: String,
    session_id: String,
    role: WireAgentRole,
    engaged: bool,
    capability_id: String,
}

impl From<WireAgentRole> for AgentRole {
    fn from(value: WireAgentRole) -> Self {
        match value {
            WireAgentRole::Root => Self::Root,
            WireAgentRole::Series => Self::Series,
            WireAgentRole::Task => Self::Task,
            WireAgentRole::Reviewer => Self::Reviewer,
        }
    }
}

fn is_hook(method: RpcMethod) -> bool {
    matches!(
        method,
        RpcMethod::HookUserPrompt
            | RpcMethod::HookPreTool
            | RpcMethod::HookPostTool
            | RpcMethod::HookStop
    )
}

fn requires_session_capability(method: RpcMethod) -> bool {
    matches!(
        method.spec().scope,
        OperationScope::Session | OperationScope::WorkItem | OperationScope::Delegation
    ) && !matches!(method, RpcMethod::SessionOpen | RpcMethod::DelegationAccept)
}

fn capability_allows(capability_id: &str, method: RpcMethod) -> bool {
    matches!(capability_id, "hook.engaged" | "hook.unengaged")
        && requires_session_capability(method)
}

fn authorize_client_kind(client_kind: Option<ClientKind>, method: RpcMethod) -> RuntimeResult<()> {
    let authorized = match method {
        RpcMethod::RuntimeDrain | RpcMethod::WorkspaceModeTransition => {
            client_kind == Some(ClientKind::Admin)
        }
        RpcMethod::HostRegister
        | RpcMethod::HostCapabilities
        | RpcMethod::HostActionNext
        | RpcMethod::HostActionAck => client_kind == Some(ClientKind::HostAdapter),
        RpcMethod::JobSubmit | RpcMethod::JobStatus | RpcMethod::JobCancel => {
            matches!(client_kind, Some(ClientKind::Cli | ClientKind::Admin))
        }
        _ => true,
    };
    if authorized {
        Ok(())
    } else {
        Err(RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "negotiated client kind does not authorize this RPC method",
        ))
    }
}

fn authorize_host_connection(
    connection: &ConnectionState,
    method: RpcMethod,
    params: &RequestParams<Value>,
) -> RuntimeResult<()> {
    let host_bound = matches!(
        method,
        RpcMethod::HostCapabilities
            | RpcMethod::HostActionNext
            | RpcMethod::HostActionAck
            | RpcMethod::HostPressureReport
    );
    if !host_bound {
        return Ok(());
    }
    if connection.client_kind != Some(ClientKind::HostAdapter) {
        return Err(RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "host method requires a host-adapter connection",
        ));
    }
    let requested = params
        .payload
        .get("adapterId")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("adapterId is required"))?;
    if connection.adapter_id.as_deref() != Some(requested) {
        return Err(RuntimeError::new(
            StableErrorCode::EndpointAuthFailed,
            "host adapter identity is not bound to this connection",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HookGuardWire {
    outcome: GateOutcomeKind,
    state_revision: u64,
    policy_digest: String,
    inventory_generation: u64,
    input_fingerprint: String,
}

fn hook_decision(
    method: RpcMethod,
    engaged: bool,
    context: Option<&Value>,
    policy_digest: &str,
    inventory_generation: u64,
) -> HookDecision {
    let point = match method {
        RpcMethod::HookUserPrompt => HookPoint::UserPrompt,
        RpcMethod::HookPreTool => HookPoint::PreTool,
        RpcMethod::HookPostTool => HookPoint::PostTool,
        RpcMethod::HookStop => HookPoint::Stop,
        _ => return HookDecision::Deny,
    };
    let guard = context.and_then(|value| {
        let wire: HookGuardWire = serde_json::from_value(value.get("hookGuard")?.clone()).ok()?;
        let state_revision = value.get("stateRevision")?.as_u64()?;
        let input_fingerprint = value.get("inputFingerprint")?.as_str()?;
        (wire.outcome == GateOutcomeKind::Pass
            && wire.state_revision == state_revision
            && wire.policy_digest == policy_digest
            && wire.inventory_generation == inventory_generation
            && wire.input_fingerprint == input_fingerprint)
            .then_some(GateOutcome::Pass)
    });
    match HookPolicy::decide(point, engaged, guard.as_ref()) {
        HookAction::Allow => HookDecision::Allow,
        HookAction::Deny => HookDecision::Deny,
        HookAction::Block => HookDecision::Block,
        HookAction::Context => HookDecision::Context,
    }
}

fn supports_v1(range: &str) -> bool {
    matches!(range, PROTOCOL_RANGE_V1 | "1.0" | "^1.0" | ">=1.0,<2")
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let maximum = left.len().max(right.len());
    for index in 0..maximum {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

fn canonical_digest(value: &impl serde::Serialize) -> RuntimeResult<String> {
    let bytes = serde_json::to_vec(value).map_err(canonical_error)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn decode_value<T: DeserializeOwned>(value: Value) -> RuntimeResult<T> {
    serde_json::from_value(value).map_err(|_| schema_error("method payload violates its schema"))
}

fn to_value(value: impl serde::Serialize) -> RuntimeResult<Value> {
    serde_json::to_value(value).map_err(canonical_error)
}

fn encode_success(value_id: String, result: impl serde::Serialize) -> RuntimeResult<Vec<u8>> {
    serde_json::to_vec(&JsonRpcResponse::new(value_id, result)).map_err(canonical_error)
}

fn serialize_error(request_id: &str, error: RuntimeError) -> Vec<u8> {
    let object = RpcErrorObject::new(
        error.code(),
        error.message(),
        error.remediation().map(str::to_owned),
        Some(request_id.to_owned()),
    );
    serde_json::to_vec(&JsonRpcErrorResponse::new(request_id, object)).unwrap_or_else(|_| {
        b"{\"jsonrpc\":\"2.0\",\"id\":\"unknown\",\"error\":{\"code\":-32042,\"message\":\"response serialization failed\"}}".to_vec()
    })
}

fn require<'a>(value: &'a Option<String>, field: &str) -> RuntimeResult<&'a str> {
    value
        .as_deref()
        .ok_or_else(|| schema_error(&format!("{field} is required")))
}

fn require_idempotency(params: &RequestParams<Value>) -> RuntimeResult<&str> {
    require(&params.idempotency_key, "idempotencyKey")
}

fn payload_string<'a>(value: &'a Value, field: &str) -> RuntimeResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error(&format!("{field} is required")))
}

fn schema_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

fn project_mismatch(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ProjectMismatch, message)
}

fn session_expired() -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::SessionExpired,
        "trusted session is absent or expired",
    )
}

fn turn_mismatch(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::TurnIdentityMismatch, message)
}

fn canonical_error(_error: serde_json::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime value could not be canonicalized",
    )
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime state lock is poisoned",
    )
}
