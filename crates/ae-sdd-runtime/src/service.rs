use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use ae_sdd_context::PressureDecision;
use ae_sdd_domain::{AgentRole, BootId, CapabilityId, DelegationId, EventStoreId, SessionId};
use ae_sdd_host::{BootCapabilitySigner, CapabilityClaims, CapabilityToken, GrantDigest};
use ae_sdd_policy::{HookAction, HookPoint, HookPolicy};
use ae_sdd_protocol::{
    ClientKind, HandshakeLimits, HandshakeRequest, HandshakeResponse, HookDecision,
    JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, OperationScope, PROTOCOL_RANGE_V1,
    PROTOCOL_VERSION_V1, RequestParams, RpcErrorObject, RpcMethod, StableErrorCode, WorkspaceMode,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    BusinessOperationPort, BusinessWorkspace, ClockPort, CompactRequestPayload, CompactResult,
    ContextCache, ContextProjectPayload, DaemonLifecycle,
    DelegationAcceptPayload, DelegationCreatePayload, DelegationReportPayload,
    DelegationSupervisor, DurableEvent, EventBatch, EventSubscriptionPayload, FlowSupervisor,
    HookPayload, HookResult, HostAckPayload, HostCoordinator, HostPressurePayload,
    HostRegisterPayload, IdempotencyReceipt, PersistencePort, RuntimeConfig, RuntimeError,
    RuntimeResult, RuntimeStatus, SessionOpenPayload, SessionResult, WireAgentRole, WorkItemActors,
    WorkspaceRegisterPayload, WorkspaceResolverPort, WorkspaceResult,
};

#[path = "service_hook_context.rs"]
mod hook_context;
#[path = "service_host.rs"]
mod host_methods;
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
}

impl ConnectionState {
    /// Whether endpoint authentication and protocol negotiation succeeded.
    #[must_use]
    pub const fn is_handshaken(&self) -> bool {
        self.handshaken
    }
}

#[derive(Clone, Debug)]
struct WorkspaceRecord {
    result: WorkspaceResult,
}

#[derive(Clone, Debug)]
struct SessionRecord {
    workspace_id: String,
    agent_id: String,
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
        let actors = WorkItemActors::new(config.work_item_mailbox_capacity);
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
        let result = self.dispatch(request.method, &params)?;
        encode_success(request.id, result)
    }

    fn dispatch(&self, method: RpcMethod, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        match method {
            RpcMethod::RuntimeHandshake => unreachable!("handshake handled before dispatch"),
            RpcMethod::RuntimeStatus => to_value(self.status()?),
            RpcMethod::RuntimeDrain => self.runtime_drain(params),
            RpcMethod::WorkspaceRegister => self.workspace_register(params),
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
            | RpcMethod::GateEvaluate
            | RpcMethod::JobStatus
            | RpcMethod::JobCancel => self.authoritative_business(method, params),
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

fn hook_decision(method: RpcMethod, engaged: bool, context: Option<&Value>) -> HookDecision {
    let point = match method {
        RpcMethod::HookUserPrompt => HookPoint::UserPrompt,
        RpcMethod::HookPreTool => HookPoint::PreTool,
        RpcMethod::HookPostTool => HookPoint::PostTool,
        RpcMethod::HookStop => HookPoint::Stop,
        _ => return HookDecision::Deny,
    };
    let guard = context
        .and_then(|value| value.get("hookGuard"))
        .and_then(|value| serde_json::from_value(value.clone()).ok());
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
