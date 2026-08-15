use std::collections::{BTreeMap, VecDeque};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use ae_sdd_context::PressureDecision;
use ae_sdd_contracts::diagnostics::{DiagnosticRecord, NodeRecord};
use ae_sdd_domain::{
    AgentRole, BootId, CapabilityId, EventStoreId, GateOutcome, ScopedGrant, SessionId,
};
use ae_sdd_host::{BootCapabilitySigner, CapabilityClaims, CapabilityToken, GrantDigest};
use ae_sdd_policy::{HookAction, HookPoint, HookPolicy};
use ae_sdd_protocol::{
    ClientKind, GateOutcomeKind, HandshakeLimits, HandshakeRequest, HandshakeResponse,
    HookDecision, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, OperationScope,
    PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1, RequestParams, RpcErrorObject, RpcMethod,
    StableErrorCode, WorkspaceMode,
};
use ae_sdd_session::{PureSessionBootstrap, SessionBootstrapPort};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::diagnostics;
use crate::{
    BusinessOperationPort, BusinessWorkspace, ClockPort, CompactRequestPayload, CompactResult,
    ContextCache, ContextProjectPayload, DaemonLifecycle, DelegationAcceptPayload,
    DelegationCreatePayload, DelegationReportPayload, DelegationSupervisor, DurableEvent,
    EventBatch, EventSubscriptionPayload, FlowSupervisor, HookPayload, HookResult, HostAckPayload,
    HostCoordinator, HostPressurePayload, HostRegisterPayload, IdempotencyReceipt, PersistencePort,
    RuntimeConfig, RuntimeError, RuntimeIdentityKind, RuntimeIdentitySnapshot,
    RuntimeIdentityTransition, RuntimeJobRecord, RuntimeJobStatus, RuntimeJobTransition,
    RuntimeResult, RuntimeSessionRecord, RuntimeStatus, RuntimeWorkspaceRecord, ScopedGrantWire,
    SessionOpenPayload, SessionResult, WireAgentRole, WorkItemActors,
    WorkspaceModeTransitionPayload, WorkspaceParityEvidence, WorkspaceRegisterPayload,
    WorkspaceResolverPort, WorkspaceResult,
};

/// Typed operations that move a work item through the flow.
///
/// Deliberately a whitelist, not everything that writes: the point of the node
/// history is to read how the work item advanced, so reads, queries and lease
/// bookkeeping stay out.  `lease.break` is the exception that earns its place —
/// it is a high-privilege override, and `constraints/security.md` requires its
/// actor and confirmation to be recoverable.
const NODE_OPERATIONS: &[&str] = &[
    "state.transition",
    "workitem.create",
    "workitem.complete",
    "execution.plan.set",
    "execution.plan.approve",
    "execution.slice.start",
    "execution.slice.record",
    "execution.resume",
    "review.finalize",
    "evidence.finalize",
    "gate.check",
    "lease.break",
];

#[path = "execution_supervisor.rs"]
mod execution_supervisor;
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

pub use execution_supervisor::ExecutionSessionBinding;

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
    external_key_hash: String,
    current_work_item: Option<String>,
    result: SessionResult,
    delegation_id: Option<String>,
    #[serde(default)]
    grant: ScopedGrantWire,
    current_turn_id: Option<String>,
    current_turn_seq: u64,
    active: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySessionRecord {
    workspace_id: String,
    agent_id: String,
    external_key: String,
    current_work_item: Option<String>,
    result: SessionResult,
    delegation_id: Option<String>,
    #[allow(dead_code)]
    current_turn_id: Option<String>,
    #[allow(dead_code)]
    current_turn_seq: u64,
    active: bool,
}

#[derive(Default)]
struct RuntimeState {
    workspaces: BTreeMap<String, WorkspaceRecord>,
    workspace_by_root: BTreeMap<String, String>,
    sessions: BTreeMap<String, SessionRecord>,
    session_by_external: BTreeMap<(String, String), String>,
    jobs: BTreeMap<String, RuntimeJobRecord>,
    job_queue: VecDeque<String>,
    execution_bindings: BTreeMap<String, ExecutionSessionBinding>,
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
    session_bootstrap: Arc<dyn SessionBootstrapPort + Send + Sync>,
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
        let delegation = DelegationSupervisor::new(
            Arc::clone(&persistence),
            Arc::clone(&host),
            Arc::clone(&clock),
        );
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
            session_bootstrap: Arc::new(PureSessionBootstrap),
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
        self.import_legacy_root_identities()?;
        let workspace_snapshots = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Workspace)?;
        let session_snapshots = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Session)?;
        let mut state = self.lock_state()?;
        state.workspaces.clear();
        state.workspace_by_root.clear();
        state.sessions.clear();
        state.session_by_external.clear();
        state.jobs.clear();
        state.job_queue.clear();
        state.execution_bindings.clear();
        // §9.4: rebuild the host-execution binding ledger from durable rows so
        // a daemon restart observes the same liveness state it left behind.
        self.delegation.bindings().recover(&*self.persistence)?;
        for snapshot in workspace_snapshots {
            let workspace = snapshot.workspace;
            let result = WorkspaceResult {
                workspace_id: workspace.workspace_id,
                canonical_root: workspace.canonical_root,
                project_key: workspace.project_key,
                mode: workspace.mode,
                inventory_generation: workspace.inventory_generation,
            };
            if state.workspaces.len() >= self.config.max_workspaces {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "durable workspace projection is inconsistent or exceeds capacity",
                ));
            }
            state
                .workspace_by_root
                .insert(result.canonical_root.clone(), result.workspace_id.clone());
            state
                .workspaces
                .insert(result.workspace_id.clone(), WorkspaceRecord { result });
        }
        for snapshot in session_snapshots {
            let session = snapshot.session.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed session snapshot lacks its session row",
                )
            })?;
            if !state.workspaces.contains_key(&session.workspace_id) {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "durable session projection references an unknown workspace",
                ));
            }
            let key = session.session_id.clone();
            let record = SessionRecord {
                workspace_id: session.workspace_id.clone(),
                agent_id: session.agent_id,
                external_key_hash: session.external_key_hash,
                current_work_item: session.current_work_item,
                result: SessionResult {
                    session_id: session.session_id,
                    role: session.role,
                    engaged: session.engaged,
                    expires_at_unix_ms: session.expires_at_unix_ms,
                    context_generation: session.context_generation,
                    capability_token: String::new(),
                },
                delegation_id: session.delegation_id,
                grant: session.grant,
                current_turn_id: None,
                current_turn_seq: 0,
                active: false,
            };
            state.session_by_external.insert(
                (
                    record.workspace_id.clone(),
                    record.external_key_hash.clone(),
                ),
                key.clone(),
            );
            state.sessions.insert(key, record);
        }
        for mut record in self.persistence.list_jobs()? {
            if state.jobs.len() >= self.config.max_jobs
                || !state.workspaces.contains_key(&record.workspace_id)
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "durable job projection is inconsistent or exceeds capacity",
                ));
            }
            let identity_bound = record.session_id.is_some();
            let active = matches!(
                record.status,
                RuntimeJobStatus::Queued | RuntimeJobStatus::Running
            );
            let old_boot = identity_bound
                && record.submission_boot_id.as_deref() != Some(self.boot_id.to_string().as_str());
            if active && old_boot {
                let expected_status = record.status;
                let expected_row_version = record.row_version;
                let recovery_at = self.clock.now_unix_ms();
                if record.status == RuntimeJobStatus::Queued {
                    record.started_at_unix_ms = Some(recovery_at);
                }
                record.status = RuntimeJobStatus::Stale;
                record.row_version = record.row_version.saturating_add(1);
                record.result = Some(json!({"errorCode":StableErrorCode::SessionExpired.as_str()}));
                record.error_code = None;
                record.finished_at_unix_ms = Some(recovery_at);
                record.updated_at_unix_ms = recovery_at;
                record = self
                    .persistence
                    .commit_job_transition(RuntimeJobTransition {
                        event: self.job_event(
                            "job.stale",
                            &record,
                            json!({"errorCode":StableErrorCode::SessionExpired.as_str()}),
                        )?,
                        record,
                        expected_status: Some(expected_status),
                        expected_row_version: Some(expected_row_version),
                    })?;
            } else if active {
                state.job_queue.push_back(record.job_id.clone());
            }
            state.jobs.insert(record.job_id.clone(), record);
        }
        drop(state);
        self.host.recover()?;
        self.delegation.recover()?;
        Ok(())
    }

    fn import_legacy_root_identities(&self) -> RuntimeResult<()> {
        let now = self.clock.now_unix_ms();
        let mut workspaces = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Workspace)?
            .into_iter()
            .map(|snapshot| (snapshot.workspace.workspace_id.clone(), snapshot.workspace))
            .collect::<BTreeMap<_, _>>();
        for (key, value) in self.persistence.list_records("workspace/v1")? {
            if workspaces.contains_key(&key) {
                continue;
            }
            let result: WorkspaceResult = decode_value(value.clone())?;
            if result.workspace_id != key {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "legacy workspace identity is inconsistent",
                ));
            }
            let workspace = RuntimeWorkspaceRecord {
                workspace_id: result.workspace_id.clone(),
                canonical_root: result.canonical_root.clone(),
                project_key: result.project_key.clone(),
                mode: result.mode,
                inventory_generation: result.inventory_generation,
                dirty: false,
                created_at_unix_ms: now,
                updated_at_unix_ms: now,
            };
            let snapshot = self
                .persistence
                .commit_identity_bundle(RuntimeIdentityTransition {
                    operation: "runtime.legacy.workspace.import".to_owned(),
                    scope_digest: canonical_digest(&json!({
                        "domain":"runtime.legacy.workspace.import/v1",
                        "workspaceId":result.workspace_id,
                    }))?,
                    idempotency_key: format!("legacy-workspace-{}", result.workspace_id),
                    request_digest: canonical_digest(&json!({
                        "legacy":value,
                        "typed":workspace,
                    }))?,
                    expected_workspace_mode: None,
                    expected_inventory_generation: None,
                    expected_session_status: None,
                    expected_delegation_status: None,
                    expected_context_generation: None,
                    snapshot: RuntimeIdentitySnapshot {
                        identity_kind: RuntimeIdentityKind::Workspace,
                        workspace,
                        session: None,
                        delegation: None,
                        host_action: None,
                        attestation: None,
                        response: to_value(&result)?,
                        replayed: false,
                    },
                    committed_at_unix_ms: now,
                })?;
            workspaces.insert(key, snapshot.workspace);
        }

        let mut sessions = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Session)?
            .into_iter()
            .filter_map(|snapshot| snapshot.session)
            .map(|session| (session.session_id.clone(), session))
            .collect::<BTreeMap<_, _>>();
        for (key, value) in self.persistence.list_records("session/v1")? {
            if sessions.contains_key(&key) {
                continue;
            }
            let legacy: LegacySessionRecord = decode_value(value.clone())?;
            if legacy.result.session_id != key {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "legacy session identity is inconsistent",
                ));
            }
            if legacy.result.role != WireAgentRole::Root {
                continue;
            }
            let workspace = workspaces
                .get(&legacy.workspace_id)
                .cloned()
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "legacy root session references a missing typed workspace",
                    )
                })?;
            if legacy.delegation_id.is_some() {
                return Err(RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "legacy root session cannot carry a delegation",
                ));
            }
            let external_key_hash = hex::encode(Sha256::digest(legacy.external_key.as_bytes()));
            let session = RuntimeSessionRecord {
                session_id: legacy.result.session_id.clone(),
                agent_id: legacy.agent_id.clone(),
                workspace_id: legacy.workspace_id.clone(),
                external_key_hash,
                role: WireAgentRole::Root,
                root_session_id: legacy.result.session_id.clone(),
                parent_session_id: None,
                delegation_id: None,
                engaged: legacy.result.engaged,
                current_work_item: legacy.current_work_item.clone(),
                grant: crate::grant::root_grant(),
                context_generation: legacy.result.context_generation,
                expires_at_unix_ms: legacy.result.expires_at_unix_ms,
                status: if legacy.active {
                    "active".to_owned()
                } else {
                    "closed".to_owned()
                },
                created_at_unix_ms: now,
                updated_at_unix_ms: now,
            };
            let response = json!({
                "sessionId":legacy.result.session_id,
                "role":legacy.result.role,
                "engaged":legacy.result.engaged,
                "expiresAtUnixMs":legacy.result.expires_at_unix_ms,
                "contextGeneration":legacy.result.context_generation,
            });
            let snapshot = self
                .persistence
                .commit_identity_bundle(RuntimeIdentityTransition {
                    operation: "runtime.legacy.root-session.import".to_owned(),
                    scope_digest: canonical_digest(&json!({
                        "domain":"runtime.legacy.root-session.import/v1",
                        "workspaceId":legacy.workspace_id,
                        "sessionId":legacy.result.session_id,
                    }))?,
                    idempotency_key: format!("legacy-session-{}", legacy.result.session_id),
                    request_digest: canonical_digest(&json!({
                        "legacy":value,
                        "typed":session,
                    }))?,
                    expected_workspace_mode: Some(workspace.mode),
                    expected_inventory_generation: Some(workspace.inventory_generation),
                    expected_session_status: None,
                    expected_delegation_status: None,
                    expected_context_generation: None,
                    snapshot: RuntimeIdentitySnapshot {
                        identity_kind: RuntimeIdentityKind::Session,
                        workspace,
                        session: Some(session),
                        delegation: None,
                        host_action: None,
                        attestation: None,
                        response,
                        replayed: false,
                    },
                    committed_at_unix_ms: now,
                })?;
            let imported = snapshot.session.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "legacy session import returned no session",
                )
            })?;
            sessions.insert(key, imported);
        }
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
            // `adapter_id` is decode-only here: a HostAdapter connection is
            // attached solely by an explicit `host.register` call further
            // down this dispatch, never as a side effect of the handshake
            // that merely proves the boot credential.
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
        let admin_lease_break =
            is_admin_lease_break(request.method, &params, connection.client_kind);
        if requires_session_capability(request.method) && !admin_lease_break {
            // A Hook no longer has to carry a turn: it runs as a stateless host
            // subprocess and the daemon allocates the turn during dispatch. This
            // pre-dispatch pass only proves the session capability, so demanding
            // a turn here would reject the very bootstrap event again.
            let identity = self.session_identity(&params, false)?;
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
            RpcMethod::WorkspaceRegister => self.workspace_register(params, client_kind),
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
            RpcMethod::HostActionNext => self.host_action_next(params),
            RpcMethod::HostActionAck => self.host_action_ack(params),
            RpcMethod::HostPressureReport => self.host_pressure(params),
            RpcMethod::DelegationCreate => self.delegation_create(params),
            RpcMethod::DelegationStatus => self.delegation_status(params),
            RpcMethod::DelegationAccept => self.delegation_accept(params),
            RpcMethod::DelegationChildClaim => self.delegation_child_claim(params),
            RpcMethod::DelegationReport => self.delegation_report(params),
            RpcMethod::DelegationCollect => self.delegation_collect(params),
            RpcMethod::DelegationCancel => self.delegation_cancel(params),
            RpcMethod::DelegationRenew => self.delegation_renew(params),
            RpcMethod::CompactRequest => self.compact_request(params),
            RpcMethod::CompactStatus => self.compact_status(params),
            RpcMethod::FlowSnapshot
            | RpcMethod::FlowNext
            | RpcMethod::OperationDescribe
            | RpcMethod::GateEvaluate => self.authoritative_business(method, params, client_kind),
            RpcMethod::OperationExecute => {
                let started = Instant::now();
                let result = self
                    .authoritative_business(method, params, client_kind)
                    .and_then(|value| {
                        self.bind_execution_resume(params, &value)?;
                        self.bind_created_work_item(params, &value)?;
                        Ok(value)
                    });
                self.emit_node(params, result.as_ref(), started);
                result
            }
            RpcMethod::JobSubmit => self.job_submit(params),
            RpcMethod::JobStatus => self.job_status(params),
            RpcMethod::JobCancel => self.job_cancel(params),
        }
    }

    /// Records a task node transition, when this operation is one.
    ///
    /// Reads and queries are filtered out here rather than at the reader: a node
    /// file that also carries every `document.resolve` stops being a readable
    /// history of how the work item moved.
    fn emit_node(
        &self,
        params: &RequestParams<Value>,
        result: Result<&Value, &RuntimeError>,
        started: Instant,
    ) {
        let Some(operation) = params.payload.get("operation").and_then(Value::as_str) else {
            return;
        };
        if !NODE_OPERATIONS.contains(&operation) {
            return;
        }
        let inner = params.payload.get("payload");
        let value = result.ok();
        diagnostics::emit(DiagnosticRecord::Node(NodeRecord {
            ts: diagnostics::now_ms(),
            op: operation.to_owned(),
            wsid: params.workspace_id.clone().unwrap_or_default(),
            wid: params.work_item_id.clone(),
            to: inner
                .and_then(|inner| inner.get("targetPhase"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            sid: params.session_id.clone(),
            tid: params.turn_id.clone(),
            hid: None,
            rev: value
                .and_then(|value| value.get("revisionAfter"))
                .and_then(Value::as_u64),
            es: value
                .and_then(|value| value.get("data"))
                .and_then(|data| data.get("eventSeq"))
                .and_then(Value::as_u64),
            // Best effort: the capability is the daemon-verified actor, but a
            // failure may be exactly that the identity could not be trusted, so
            // an unresolvable identity records as absent rather than blocking
            // the line that explains the failure.
            actor: self
                .session_identity(params, false)
                .ok()
                .map(|identity| identity.capability_id),
            reason: params
                .confirmation
                .as_ref()
                .map(|confirmation| confirmation.approved_by.clone()),
            conf: params
                .confirmation
                .as_ref()
                .map(|confirmation| confirmation.confirmation_id.clone()),
            ok: result.is_ok(),
            err: result.err().map(|error| format!("{:?}", error.code())),
            ms: diagnostics::elapsed_ms(started),
        }));
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

    pub(super) fn session_bootstrap(&self) -> &(dyn SessionBootstrapPort + Send + Sync) {
        self.session_bootstrap.as_ref()
    }
}

#[derive(Clone, Debug)]
struct TrustedSession {
    workspace_id: String,
    session_id: String,
    role: WireAgentRole,
    grant: ScopedGrant,
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
    ) && !matches!(
        method,
        RpcMethod::SessionOpen | RpcMethod::DelegationAccept | RpcMethod::DelegationChildClaim
    )
}

fn is_admin_lease_break(
    method: RpcMethod,
    params: &RequestParams<Value>,
    client_kind: Option<ClientKind>,
) -> bool {
    method == RpcMethod::OperationExecute
        && client_kind == Some(ClientKind::Admin)
        && params.payload.get("operation").and_then(Value::as_str) == Some("lease.break")
}

fn capability_allows(capability_id: &str, method: RpcMethod) -> bool {
    matches!(capability_id, "hook.engaged" | "hook.unengaged")
        && requires_session_capability(method)
}

fn authorize_client_kind(client_kind: Option<ClientKind>, method: RpcMethod) -> RuntimeResult<()> {
    let authorized = match method {
        RpcMethod::RuntimeDrain => client_kind == Some(ClientKind::Admin),
        RpcMethod::WorkspaceModeTransition => {
            matches!(client_kind, Some(ClientKind::Admin | ClientKind::Hook))
        }
        RpcMethod::HostRegister | RpcMethod::HostActionNext | RpcMethod::HostActionAck => {
            client_kind == Some(ClientKind::HostAdapter)
        }
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
        RpcMethod::HostActionNext | RpcMethod::HostActionAck | RpcMethod::HostPressureReport
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
