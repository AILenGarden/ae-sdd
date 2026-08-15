#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ae_sdd_domain::{AgentRole, BootId, EventStoreId};
use ae_sdd_protocol::{
    ClientKind, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1,
    RequestParams, RpcMethod, SecretString, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, ClockPort, ConnectionState, ContextProjectionInput,
    MemoryPersistence, PersistencePort, ResolvedWorkspace, RuntimeConfig, RuntimeError,
    RuntimeResult, RuntimeService, SessionResult, WorkspaceParityEvidence, WorkspaceResolverPort,
    WorkspaceResult,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct TestClock(pub AtomicU64);

impl TestClock {
    pub fn new(now: u64) -> Self {
        Self(AtomicU64::new(now))
    }

    pub fn set(&self, now: u64) {
        self.0.store(now, Ordering::Release);
    }
}

impl ClockPort for TestClock {
    fn now_unix_ms(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

pub struct TestResolver;

impl WorkspaceResolverPort for TestResolver {
    fn resolve(&self, requested_root: &str) -> RuntimeResult<ResolvedWorkspace> {
        Ok(ResolvedWorkspace {
            canonical_root: requested_root.to_owned(),
            inside_allowed_root: true,
        })
    }
}

pub struct TestBusiness {
    pub operation_calls: AtomicUsize,
    pub operation_delay_ms: AtomicU64,
    pub projection_bytes: AtomicUsize,
    pub pass_guard: AtomicUsize,
    pub artifact_validation_calls: AtomicUsize,
    pub series_completed_calls: AtomicUsize,
    flow_next_result: Mutex<Option<Value>>,
    persistence: Mutex<Option<Arc<MemoryPersistence>>>,
}

impl Default for TestBusiness {
    fn default() -> Self {
        Self {
            operation_calls: AtomicUsize::new(0),
            operation_delay_ms: AtomicU64::new(0),
            projection_bytes: AtomicUsize::new(0),
            pass_guard: AtomicUsize::new(0),
            artifact_validation_calls: AtomicUsize::new(0),
            series_completed_calls: AtomicUsize::new(0),
            flow_next_result: Mutex::new(None),
            persistence: Mutex::new(None),
        }
    }
}

impl TestBusiness {
    fn with_persistence(persistence: Arc<MemoryPersistence>) -> Self {
        Self {
            persistence: Mutex::new(Some(persistence)),
            ..Self::default()
        }
    }

    pub fn set_flow_next_result(&self, value: Value) {
        *self.flow_next_result.lock().expect("flow next result lock") = Some(value);
    }
}

impl BusinessOperationPort for TestBusiness {
    fn execute(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        _workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        self.operation_calls.fetch_add(1, Ordering::AcqRel);
        let delay = self.operation_delay_ms.load(Ordering::Acquire);
        if delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
        if method == RpcMethod::FlowNext
            && let Some(result) = self
                .flow_next_result
                .lock()
                .expect("flow next result lock")
                .clone()
        {
            return Ok(result);
        }
        // `workitem.create` is Workspace-scoped: the caller cannot name the
        // Work Item it creates, so the business authority mints the business
        // key and returns it in the result, echoing an explicit name when one
        // was supplied.
        if params.payload.get("operation").and_then(Value::as_str) == Some("workitem.create") {
            let work_item_id = params
                .work_item_id
                .clone()
                .unwrap_or_else(|| "WORK-MINTED".to_owned());
            return Ok(json!({"ok":true,"data":{"workItemId":work_item_id}}));
        }
        Ok(json!({"ok":true}))
    }

    fn project_context(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput> {
        let requested = self.projection_bytes.load(Ordering::Acquire);
        if requested > 0 {
            return Ok(ContextProjectionInput {
                session_id: session_id.to_owned(),
                source_revision: 1,
                projection: Value::String("x".repeat(requested)),
            });
        }
        let input = "a".repeat(64);
        let policy = ae_sdd_policy::policy_digest().to_hex();
        let guard = (self.pass_guard.load(Ordering::Acquire) != 0).then(|| {
            json!({
                "outcome":"PASS",
                "stateRevision":1,
                "policyDigest":policy,
                "inventoryGeneration":workspace.inventory_generation,
                "inputFingerprint":input,
            })
        });
        Ok(ContextProjectionInput {
            session_id: session_id.to_owned(),
            source_revision: 1,
            projection: json!({
                "workspaceId":workspace.workspace_id,
                "workItemId":work_item_id,
                "role":format!("{role:?}").to_lowercase(),
                "stateRevision":1,
                "policyDigest":policy,
                "inventoryGeneration":workspace.inventory_generation,
                "inputFingerprint":input,
                "hookGuard":guard,
            }),
        })
    }

    fn execute_job(
        &self,
        _workspace: &BusinessWorkspace,
        _entrypoint: &str,
        _arguments: &Value,
    ) -> RuntimeResult<Value> {
        Ok(json!({"outcome":"PASS"}))
    }

    fn record_series_completed(
        &self,
        _workspace: &BusinessWorkspace,
        _work_item_id: &str,
        _session_id: &str,
        _delegation_id: &str,
        _idempotency_key: &str,
    ) -> RuntimeResult<()> {
        self.series_completed_calls.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn validate_delegation_artifacts(
        &self,
        _workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
    ) -> RuntimeResult<Value> {
        self.artifact_validation_calls
            .fetch_add(1, Ordering::AcqRel);
        if result
            .get("deliverables")
            .and_then(Value::as_array)
            .is_some_and(|deliverables| deliverables.iter().any(|item| !item.is_object()))
        {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "child deliverable must be an object",
            ));
        }
        Ok(json!({
            "schemaVersion":"delegation-artifact-validation/v1",
            "delegationId":delegation_id,
            "resultDigest":hex::encode(Sha256::digest(serde_json::to_vec(result).expect("result serializes"))),
            "artifacts":result.get("deliverables").cloned().unwrap_or_else(|| json!([])),
        }))
    }

    fn cleanup_delegation_memory(
        &self,
        _workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
        _artifact_receipt: &Value,
    ) -> RuntimeResult<Value> {
        let snapshot = result
            .get("memorySnapshotDigest")
            .cloned()
            .unwrap_or_else(|| json!("a".repeat(64)));
        if let Some(persistence) = self
            .persistence
            .lock()
            .expect("test persistence lock")
            .as_ref()
        {
            let mut namespace = persistence
                .load_record("delegation-memory/v1", delegation_id)?
                .expect("test namespace exists");
            namespace["status"] = json!("cleaned");
            namespace["payloadPurged"] = json!(true);
            namespace["memorySnapshotDigest"] = snapshot.clone();
            persistence.store_record("delegation-memory/v1", delegation_id, &namespace)?;
        }
        Ok(json!({
            "schemaVersion":"delegation-memory-cleanup/v1",
            "delegationId":delegation_id,
            "memorySnapshotDigest":snapshot,
            "cleanupDigest":"b".repeat(64),
            "cleanedAtUnixMs":1_000,
        }))
    }
}

pub struct Harness {
    pub runtime: Arc<RuntimeService>,
    pub clock: Arc<TestClock>,
    pub business: Arc<TestBusiness>,
    pub persistence: Arc<MemoryPersistence>,
    token: String,
}

impl Harness {
    pub fn new(mut config: RuntimeConfig) -> Self {
        config.policy_digest = ae_sdd_policy::policy_digest().to_hex();
        let token = "endpoint-test-token".to_owned();
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
            Uuid::from_u128(10),
        )));
        Self::with_persistence(config, persistence, 11, token)
    }

    pub fn with_persistence(
        mut config: RuntimeConfig,
        persistence: Arc<MemoryPersistence>,
        boot_id: u128,
        token: String,
    ) -> Self {
        config.policy_digest = ae_sdd_policy::policy_digest().to_hex();
        let clock = Arc::new(TestClock::new(1_000));
        let business = Arc::new(TestBusiness::with_persistence(persistence.clone()));
        let runtime = Arc::new(RuntimeService::new(
            config,
            BootId::from_uuid(Uuid::from_u128(boot_id)),
            token.clone(),
            persistence.clone(),
            clock.clone(),
            Arc::new(TestResolver),
            business.clone(),
        ));
        Self {
            runtime,
            clock,
            business,
            persistence,
            token,
        }
    }

    pub fn host_credential(&self) -> String {
        self.token.clone()
    }

    pub fn connection(&self, kind: ClientKind) -> ConnectionState {
        self.connection_handshake_only(kind, None)
    }

    /// Handshakes as `kind` and, when `adapter_id` is given, attaches it with
    /// a real, explicit `host.register` call right after -- the handshake
    /// itself attaches nothing. This is the helper existing runtime tests
    /// use when they want an already-attached host connection; it performs
    /// the same two RPCs a real HostAdapter bridge performs, just back to
    /// back instead of across a real reconnect.
    pub fn connection_as(&self, kind: ClientKind, adapter_id: Option<&str>) -> ConnectionState {
        let mut connection = self.connection_handshake_only(kind, adapter_id);
        if let Some(adapter_id) = adapter_id {
            let mut register = params(json!({"adapterId": adapter_id}), 1_000);
            register.capability_token = Some(self.host_credential());
            register.idempotency_key = Some(format!("connection-as-register-{adapter_id}"));
            let response = self.call(&mut connection, RpcMethod::HostRegister, register);
            assert!(response.get("result").is_some(), "{response}");
        }
        connection
    }

    /// Handshakes as `kind` and returns the connection with nothing attached,
    /// regardless of `adapter_id`.
    ///
    /// `HandshakeRequest.adapter_id` is `#[serde(skip_serializing)]`, so
    /// building the request as a typed struct and serializing it -- as this
    /// helper used to do -- can never put `adapterId` on the wire at all,
    /// modern client or legacy. That made a "legacy client still sends
    /// `adapterId`" test vacuous: it would pass by never exercising the
    /// decode-only field in the first place. This helper instead serializes
    /// the struct for its other fields, then splices a raw `adapterId` key
    /// into the resulting JSON object when `adapter_id` is given, which is
    /// exactly what a pre-C2 client's request bytes looked like on the wire.
    pub fn connection_handshake_only(
        &self,
        kind: ClientKind,
        adapter_id: Option<&str>,
    ) -> ConnectionState {
        let mut connection = ConnectionState::default();
        let mut request = serde_json::to_value(HandshakeRequest {
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            client_build: "test/client".to_owned(),
            client_kind: kind,
            endpoint_token: SecretString::new(self.token.clone()),
            expected_boot_id: self.runtime.boot_id().to_string(),
            expected_policy_digest: self.runtime.policy_digest().to_owned(),
            adapter_id: None,
        })
        .expect("handshake serializes");
        if let Some(adapter_id) = adapter_id {
            request
                .as_object_mut()
                .expect("handshake request serializes as an object")
                .insert("adapterId".to_owned(), json!(adapter_id));
        }
        let response = self.raw(&mut connection, RpcMethod::RuntimeHandshake, request);
        assert!(response.get("result").is_some(), "{response}");
        connection
    }

    /// Proves that `connection` has no adapter identity bound to it: an
    /// [`RpcMethod::HostPressureReport`] naming `adapter_id` must be refused
    /// by the connection-scoped check in `authorize_host_connection`
    /// (`connection.adapter_id.as_deref() != Some(requested)`), not by
    /// anything durable in `HostCoordinator`. `connection.adapter_id` is set
    /// only by an explicit, successful `host.register` on the same
    /// connection, so this is a direct proof, not an inference through
    /// `delegation.create`'s policy-selecting `codex`-default/`len() == 1`
    /// branches. `workspace_id` only satisfies the envelope-level admission
    /// check that runs before authorization; it need not be the workspace
    /// this delegation eventually uses.
    pub fn assert_connection_has_no_adapter_bound(
        &self,
        connection: &mut ConnectionState,
        adapter_id: &str,
        workspace_id: &str,
    ) {
        let mut request = params(
            json!({
                "adapterId": adapter_id,
                "contextGeneration": 0,
                "sampleSeq": 1,
                "usedTokens": 0,
                "contextWindowTokens": 1,
                "observedAtUnixMs": 1,
            }),
            1_000,
        );
        request.workspace_id = Some(workspace_id.to_owned());
        request.idempotency_key = Some(format!("assert-no-adapter-bound-{adapter_id}"));
        let response = self.call(connection, RpcMethod::HostPressureReport, request);
        assert_eq!(
            stable_error(&response),
            "ENDPOINT_AUTH_FAILED",
            "a connection with nothing bound to it must be refused before any durable \
             registration state is even consulted: {response}"
        );
    }

    pub fn call(
        &self,
        connection: &mut ConnectionState,
        method: RpcMethod,
        params: RequestParams<Value>,
    ) -> Value {
        self.raw(
            connection,
            method,
            serde_json::to_value(params).expect("params serialize"),
        )
    }

    fn raw(&self, connection: &mut ConnectionState, method: RpcMethod, params: Value) -> Value {
        let request = JsonRpcRequest::new(format!("{}-test", method.as_str()), method, params);
        let bytes = serde_json::to_vec(&request).expect("request serializes");
        serde_json::from_slice(&self.runtime.handle_payload(connection, &bytes))
            .expect("response is JSON")
    }
}

pub fn params(payload: Value, deadline_ms: u64) -> RequestParams<Value> {
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
        idempotency_key: None,
        confirmation: None,
        deadline_ms,
        payload,
    }
}

pub fn result(response: &Value) -> Value {
    response
        .get("result")
        .cloned()
        .unwrap_or_else(|| panic!("{response}"))
}

pub fn stable_error(response: &Value) -> &str {
    response["error"]["data"]["stableCode"]
        .as_str()
        .unwrap_or_else(|| panic!("{response}"))
}

#[allow(clippy::too_many_arguments)]
pub fn create_root_series_delegation(
    harness: &Harness,
    connection: &mut ConnectionState,
    workspace: &WorkspaceResult,
    root: &SessionResult,
    agent_id: &str,
    work_item_id: &str,
    series_kind: &str,
    required_artifacts: &[&str],
    key: &str,
) -> Value {
    let decision_digest = flow_decision_digest(key);
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":decision_digest,
        "stateRevision":1,
        "phase":"initialized",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":series_kind,
            "requiredArtifacts":required_artifacts,
        }
    }));
    let mut next = session_params(workspace, root, agent_id, json!({}), 1_000);
    next.work_item_id = Some(work_item_id.to_owned());
    result(&harness.call(connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        workspace,
        root,
        agent_id,
        json!({"flowDecisionDigest":decision_digest}),
        1_000,
    );
    create.work_item_id = Some(work_item_id.to_owned());
    create.idempotency_key = Some(key.to_owned());
    result(&harness.call(connection, RpcMethod::DelegationCreate, create))
}

pub fn flow_decision_digest(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

pub fn register_workspace(
    harness: &Harness,
    connection: &mut ConnectionState,
    suffix: &str,
) -> WorkspaceResult {
    let mut request = params(
        json!({
            "projectRoot": format!("C:/ae-sdd-tests/{suffix}"),
            "projectKey": format!("project-{suffix}"),
        }),
        1_000,
    );
    request.idempotency_key = Some(format!("workspace-{suffix}"));
    serde_json::from_value(result(&harness.call(
        connection,
        RpcMethod::WorkspaceRegister,
        request,
    )))
    .expect("workspace result decodes")
}

pub fn canary_workspace(harness: &Harness, suffix: &str) -> WorkspaceResult {
    let mut connection = harness.connection(ClientKind::Admin);
    let workspace = register_workspace(harness, &mut connection, suffix);
    let confirmation = || ae_sdd_protocol::ConfirmationRef {
        confirmation_id: format!("confirmation-{suffix}"),
        approved_by: "user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    };
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some(format!("drain-{suffix}"));
    drain.confirmation = Some(confirmation());
    result(&harness.call(&mut connection, RpcMethod::RuntimeDrain, drain));
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some(format!("mode-{suffix}"));
    transition.confirmation = Some(confirmation());
    serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace result decodes")
}

pub fn open_root_session(
    harness: &Harness,
    connection: &mut ConnectionState,
    workspace: &WorkspaceResult,
    agent_id: &str,
    external_key: &str,
    work_item_id: Option<&str>,
) -> SessionResult {
    let mut request = params(
        json!({
            "externalKey": external_key,
            "role": "root",
            "engaged": matches!(workspace.mode, WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter),
        }),
        1_000,
    );
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.agent_id = Some(agent_id.to_owned());
    request.work_item_id = work_item_id.map(str::to_owned);
    request.idempotency_key = Some(format!(
        "session-open-{external_key}-{}",
        serde_json::to_string(&workspace.mode).expect("workspace mode serializes")
    ));
    serde_json::from_value(result(&harness.call(
        connection,
        RpcMethod::SessionOpen,
        request,
    )))
    .expect("session result decodes")
}

pub fn session_params(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    agent_id: &str,
    payload: Value,
    deadline_ms: u64,
) -> RequestParams<Value> {
    let mut request = params(payload, deadline_ms);
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.agent_id = Some(agent_id.to_owned());
    request.session_id = Some(session.session_id.clone());
    request.capability_token = Some(session.capability_token.clone());
    request
}

pub fn parity_transition_payload(target_mode: WorkspaceMode, now_unix_ms: u64) -> Value {
    let observation_digest = "a".repeat(64);
    let parity = WorkspaceParityEvidence {
        comparison_count: 10,
        mismatch_count: 0,
        source_revision: 1,
        legacy_digest: observation_digest.clone(),
        rust_digest: observation_digest,
        observed_at_unix_ms: now_unix_ms,
    };
    let parity_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&parity).expect("parity evidence serializes"),
    ));
    json!({
        "targetMode": target_mode,
        "reason": "verified typed parity evidence",
        "parityDigest": parity_digest,
        "parity": parity,
    })
}
