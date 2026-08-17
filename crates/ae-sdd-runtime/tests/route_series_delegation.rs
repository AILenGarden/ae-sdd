//! Root-to-Series delegation consumes a committed daemon flow reference.

mod support;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use ae_sdd_domain::{AgentRole, BootId, EventStoreId};
use ae_sdd_protocol::{
    ClientKind, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1, RequestParams, RpcMethod,
    SecretString,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, ConnectionState, ContextProjectionInput,
    DurableEvent, ExecutionCheckpointRecord, ExecutionCheckpointScope,
    ExecutionResourceLeaseOutcomeV1, ExecutionResourceLeaseRequestV1, IdempotencyReceipt,
    MemoryPersistence, PersistencePort, PreparedExecutionHookV1, ResolvedWorkspace, RuntimeConfig,
    RuntimeIdentityKind, RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeJobRecord,
    RuntimeJobTransition, RuntimeResult, RuntimeService, SessionResult, WorkspaceResolverPort,
    WorkspaceResult,
};
use serde_json::{Value, json};
use uuid::Uuid;

use support::{
    Harness, TestBusiness, TestClock, create_root_series_delegation, open_root_session, params,
    register_workspace, result, session_params, stable_error,
};

const ADAPTER: &str = "host-series-intent";
const WORK_ITEM: &str = "WORK";
const DECISION: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const RETRY_CHILD: &str = "00000000-0000-0000-0000-000000001201";
const ROLE_SAFE_CHILD: &str = "00000000-0000-0000-0000-000000001301";

struct AssetRefBusiness {
    inner: TestBusiness,
    asset_revision: AtomicU64,
    change_assets_after_flow_next: AtomicU64,
}

impl AssetRefBusiness {
    fn new() -> Self {
        Self {
            inner: TestBusiness::default(),
            asset_revision: AtomicU64::new(1),
            change_assets_after_flow_next: AtomicU64::new(0),
        }
    }

    fn set_flow_next_result(&self, value: Value) {
        self.inner.set_flow_next_result(value);
    }

    fn set_asset_revision(&self, revision: u64) {
        self.asset_revision.store(revision, Ordering::Release);
    }

    fn change_assets_after_flow_next(&self) {
        self.change_assets_after_flow_next
            .store(1, Ordering::Release);
    }
}

impl BusinessOperationPort for AssetRefBusiness {
    fn execute(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        let result = self.inner.execute(method, params, workspace)?;
        if method == RpcMethod::FlowNext
            && self.change_assets_after_flow_next.load(Ordering::Acquire) != 0
        {
            self.set_asset_revision(2);
        }
        Ok(result)
    }

    fn project_context(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput> {
        let asset_revision = self.asset_revision.load(Ordering::Acquire);
        Ok(ContextProjectionInput {
            session_id: session_id.to_owned(),
            source_revision: 7,
            projection: json!({
                "workspaceId":workspace.workspace_id,
                "workItemId":work_item_id,
                "role":format!("{role:?}").to_lowercase(),
                "assetRefs":[
                    {
                        "kind":"constraints-index",
                        "path":"constraints/README.md",
                        "sha256":"a".repeat(64),
                        "byteLength":42
                    },
                    {
                        "kind":"methodology-skill",
                        "path":"source/skills/phase1-design/story-generate-skill.md",
                        "sha256":if asset_revision == 1 { "b".repeat(64) } else { "c".repeat(64) },
                        "byteLength":84
                    }
                ]
            }),
        })
    }

    fn execute_job(
        &self,
        workspace: &BusinessWorkspace,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        self.inner.execute_job(workspace, entrypoint, arguments)
    }

    fn validate_delegation_artifacts(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
    ) -> RuntimeResult<Value> {
        self.inner
            .validate_delegation_artifacts(workspace, delegation_id, result)
    }

    fn cleanup_delegation_memory(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
        artifact_receipt: &Value,
    ) -> RuntimeResult<Value> {
        self.inner
            .cleanup_delegation_memory(workspace, delegation_id, result, artifact_receipt)
    }
}

struct AssetRefResolver;

impl WorkspaceResolverPort for AssetRefResolver {
    fn resolve(&self, requested_root: &str) -> RuntimeResult<ResolvedWorkspace> {
        Ok(ResolvedWorkspace {
            canonical_root: requested_root.to_owned(),
            inside_allowed_root: true,
        })
    }
}

struct IntentOrderingBusiness {
    inner: TestBusiness,
    persistence: Arc<MemoryPersistence>,
    calls: AtomicU64,
    second_saw_committed_intent: AtomicBool,
}

impl IntentOrderingBusiness {
    fn new(persistence: Arc<MemoryPersistence>) -> Self {
        Self {
            inner: TestBusiness::default(),
            persistence,
            calls: AtomicU64::new(0),
            second_saw_committed_intent: AtomicBool::new(false),
        }
    }
}

impl BusinessOperationPort for IntentOrderingBusiness {
    fn execute(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        if method != RpcMethod::FlowNext {
            return self.inner.execute(method, params, workspace);
        }
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if call != 0 {
            let workspace_id = params.workspace_id.as_deref().expect("workspace is bound");
            let work_item_id = params.work_item_id.as_deref().expect("Work Item is bound");
            let key = format!("{workspace_id}\0{work_item_id}\0{DECISION}");
            self.second_saw_committed_intent.store(
                self.persistence
                    .load_record("flow-delegation-intent/v1", &key)?
                    .is_some(),
                Ordering::Release,
            );
        }
        Ok(json!({
            "schemaVersion":"flow-decision/v1",
            "decisionDigest":DECISION,
            "inputFingerprint":"f".repeat(64),
            "stateRevision":7,
            "phase":"requirement-analyzed",
            "assetRefs":[{
                "kind":"methodology-skill",
                "path":"source/skills/phase1-design/story-generate-skill.md",
                "sha256":if call == 0 { "b".repeat(64) } else { "c".repeat(64) },
                "byteLength":84
            }],
            "nextAction":{
                "kind":"delegate-series",
                "seriesKind":"story",
                "requiredArtifacts":["STORY"]
            }
        }))
    }

    fn project_context(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput> {
        self.inner
            .project_context(workspace, work_item_id, session_id, role)
    }

    fn execute_job(
        &self,
        workspace: &BusinessWorkspace,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        self.inner.execute_job(workspace, entrypoint, arguments)
    }

    fn validate_delegation_artifacts(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
    ) -> RuntimeResult<Value> {
        self.inner
            .validate_delegation_artifacts(workspace, delegation_id, result)
    }

    fn cleanup_delegation_memory(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
        artifact_receipt: &Value,
    ) -> RuntimeResult<Value> {
        self.inner
            .cleanup_delegation_memory(workspace, delegation_id, result, artifact_receipt)
    }
}

struct IntentLoadBarrierPersistence {
    inner: Arc<MemoryPersistence>,
    first_load_entered: AtomicBool,
    first_load_count: AtomicU64,
    release: (Mutex<bool>, Condvar),
}

impl IntentLoadBarrierPersistence {
    fn new(inner: Arc<MemoryPersistence>) -> Self {
        Self {
            inner,
            first_load_entered: AtomicBool::new(false),
            first_load_count: AtomicU64::new(0),
            release: (Mutex::new(false), Condvar::new()),
        }
    }

    fn release_first_load(&self) {
        let (released, ready) = &self.release;
        *released.lock().expect("barrier lock") = true;
        ready.notify_all();
    }
}

impl PersistencePort for IntentLoadBarrierPersistence {
    fn event_store_id(&self) -> RuntimeResult<EventStoreId> {
        self.inner.event_store_id()
    }

    fn latest_event_sequence(&self) -> RuntimeResult<u64> {
        self.inner.latest_event_sequence()
    }

    fn append_event(&self, event: DurableEvent) -> RuntimeResult<DurableEvent> {
        self.inner.append_event(event)
    }

    fn commit_event_and_receipt(
        &self,
        event: DurableEvent,
        receipt: IdempotencyReceipt,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt)> {
        self.inner.commit_event_and_receipt(event, receipt)
    }

    fn commit_prepared_execution_hook(
        &self,
        event: DurableEvent,
        receipt: IdempotencyReceipt,
        record: PreparedExecutionHookV1,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt, PreparedExecutionHookV1)> {
        self.inner
            .commit_prepared_execution_hook(event, receipt, record)
    }

    fn events_after(&self, after: u64, limit: usize) -> RuntimeResult<Vec<DurableEvent>> {
        self.inner.events_after(after, limit)
    }

    fn oldest_event_sequence(&self) -> RuntimeResult<u64> {
        self.inner.oldest_event_sequence()
    }

    fn load_receipt(&self, scope: &str, key: &str) -> RuntimeResult<Option<IdempotencyReceipt>> {
        self.inner.load_receipt(scope, key)
    }

    fn store_receipt(&self, receipt: &IdempotencyReceipt) -> RuntimeResult<()> {
        self.inner.store_receipt(receipt)
    }

    fn load_record(&self, namespace: &str, key: &str) -> RuntimeResult<Option<Value>> {
        if namespace == "flow-delegation-intent/v1"
            && self.first_load_count.fetch_add(1, Ordering::AcqRel) == 0
        {
            self.first_load_entered.store(true, Ordering::Release);
            let (released, ready) = &self.release;
            let mut released = released.lock().expect("barrier lock");
            while !*released {
                released = ready.wait(released).expect("barrier wait");
            }
        }
        self.inner.load_record(namespace, key)
    }

    fn list_records(&self, namespace: &str) -> RuntimeResult<Vec<(String, Value)>> {
        self.inner.list_records(namespace)
    }

    fn store_record(&self, namespace: &str, key: &str, value: &Value) -> RuntimeResult<()> {
        self.inner.store_record(namespace, key, value)
    }

    fn delete_record(&self, namespace: &str, key: &str) -> RuntimeResult<()> {
        self.inner.delete_record(namespace, key)
    }

    fn acquire_execution_resource_lease(
        &self,
        request: &ExecutionResourceLeaseRequestV1,
    ) -> RuntimeResult<ExecutionResourceLeaseOutcomeV1> {
        self.inner.acquire_execution_resource_lease(request)
    }

    fn release_execution_resource_lease(
        &self,
        resource: &str,
        boot_id: &str,
        session_id: &str,
    ) -> RuntimeResult<()> {
        self.inner
            .release_execution_resource_lease(resource, boot_id, session_id)
    }

    fn commit_identity_bundle(
        &self,
        transition: RuntimeIdentityTransition,
    ) -> RuntimeResult<RuntimeIdentitySnapshot> {
        self.inner.commit_identity_bundle(transition)
    }

    fn list_identity_snapshots(
        &self,
        kind: RuntimeIdentityKind,
    ) -> RuntimeResult<Vec<RuntimeIdentitySnapshot>> {
        self.inner.list_identity_snapshots(kind)
    }

    fn commit_job_transition(
        &self,
        transition: RuntimeJobTransition,
    ) -> RuntimeResult<RuntimeJobRecord> {
        self.inner.commit_job_transition(transition)
    }

    fn load_job(&self, job_id: &str) -> RuntimeResult<Option<RuntimeJobRecord>> {
        self.inner.load_job(job_id)
    }

    fn list_jobs(&self) -> RuntimeResult<Vec<RuntimeJobRecord>> {
        self.inner.list_jobs()
    }

    fn load_execution_checkpoint(
        &self,
        scope: &ExecutionCheckpointScope,
    ) -> RuntimeResult<Option<ExecutionCheckpointRecord>> {
        self.inner.load_execution_checkpoint(scope)
    }

    fn store_execution_checkpoint(&self, record: &ExecutionCheckpointRecord) -> RuntimeResult<()> {
        self.inner.store_execution_checkpoint(record)
    }

    fn discard_execution_checkpoint(&self, scope: &ExecutionCheckpointScope) -> RuntimeResult<()> {
        self.inner.discard_execution_checkpoint(scope)
    }
}

fn asset_ref_call(
    runtime: &RuntimeService,
    connection: &mut ConnectionState,
    method: RpcMethod,
    params: RequestParams<Value>,
) -> Value {
    let request = JsonRpcRequest::new(
        format!("{}-asset-ref-test", method.as_str()),
        method,
        serde_json::to_value(params).expect("params serialize"),
    );
    serde_json::from_slice(&runtime.handle_payload(
        connection,
        &serde_json::to_vec(&request).expect("request serializes"),
    ))
    .expect("response is JSON")
}

fn asset_ref_connection(
    runtime: &RuntimeService,
    token: &str,
    kind: ClientKind,
) -> ConnectionState {
    let mut connection = ConnectionState::default();
    let handshake = JsonRpcRequest::new(
        "asset-ref-handshake",
        RpcMethod::RuntimeHandshake,
        HandshakeRequest {
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            client_build: "test/client".to_owned(),
            client_kind: kind,
            endpoint_token: SecretString::new(token.to_owned()),
            expected_boot_id: runtime.boot_id().to_string(),
            expected_policy_digest: runtime.policy_digest().to_owned(),
            adapter_id: None,
        },
    );
    let response: Value = serde_json::from_slice(&runtime.handle_payload(
        &mut connection,
        &serde_json::to_vec(&handshake).expect("handshake serializes"),
    ))
    .expect("handshake response is JSON");
    assert!(response.get("result").is_some(), "{response}");
    connection
}

#[test]
fn root_series_delegation_carries_authoritative_asset_refs() {
    let config = RuntimeConfig {
        policy_digest: ae_sdd_policy::policy_digest().to_hex(),
        ..RuntimeConfig::default()
    };
    let token = "asset-ref-endpoint-token";
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(210),
    )));
    let clock = Arc::new(TestClock::new(1_000));
    let business = Arc::new(AssetRefBusiness::new());
    let runtime = RuntimeService::new(
        config,
        BootId::from_uuid(Uuid::from_u128(211)),
        token.to_owned(),
        persistence,
        clock,
        Arc::new(AssetRefResolver),
        business.clone(),
    );
    let mut root_connection = asset_ref_connection(&runtime, token, ClientKind::Hook);
    let mut register = params(
        json!({
            "projectRoot":"C:/ae-sdd-tests/asset-ref-series",
            "projectKey":"project-asset-ref-series"
        }),
        1_000,
    );
    register.idempotency_key = Some("asset-ref-workspace".to_owned());
    let workspace: WorkspaceResult = serde_json::from_value(result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::WorkspaceRegister,
        register,
    )))
    .expect("workspace result decodes");

    let mut open = params(
        json!({"externalKey":"asset-ref-root","role":"root","engaged":false}),
        1_000,
    );
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.agent_id = Some("asset-ref-root-agent".to_owned());
    open.work_item_id = Some(WORK_ITEM.to_owned());
    open.idempotency_key = Some("asset-ref-root-open".to_owned());
    let root: SessionResult = serde_json::from_value(result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::SessionOpen,
        open,
    )))
    .expect("root session decodes");

    let mut host = asset_ref_connection(&runtime, token, ClientKind::HostAdapter);
    let mut host_register = params(json!({"adapterId":ADAPTER}), 1_000);
    host_register.capability_token = Some(token.to_owned());
    host_register.idempotency_key = Some("asset-ref-host-register".to_owned());
    result(&asset_ref_call(
        &runtime,
        &mut host,
        RpcMethod::HostRegister,
        host_register,
    ));

    business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":"f".repeat(64),
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "assetRefs":[
            {
                "kind":"constraints-index",
                "path":"constraints/README.md",
                "sha256":"a".repeat(64),
                "byteLength":42
            },
            {
                "kind":"methodology-skill",
                "path":"source/skills/phase1-design/story-generate-skill.md",
                "sha256":"b".repeat(64),
                "byteLength":84
            }
        ],
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"story",
            "requiredArtifacts":["STORY"]
        }
    }));
    business.change_assets_after_flow_next();
    let mut next = session_params(&workspace, &root, "asset-ref-root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::FlowNext,
        next,
    ));

    let mut create = session_params(
        &workspace,
        &root,
        "asset-ref-root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("asset-ref-series-create".to_owned());
    let delegation = result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create,
    ));

    assert_eq!(
        delegation["assetRefs"],
        json!([
            {
                "kind":"constraints-index",
                "path":"constraints/README.md",
                "sha256":"a".repeat(64),
                "byteLength":42
            },
            {
                "kind":"methodology-skill",
                "path":"source/skills/phase1-design/story-generate-skill.md",
                "sha256":"b".repeat(64),
                "byteLength":84
            }
        ]),
        "Root-to-Series delegation must carry the bounded authoritative projection"
    );
}

#[test]
fn root_series_delegation_replay_keeps_flow_committed_asset_refs() {
    let config = RuntimeConfig {
        policy_digest: ae_sdd_policy::policy_digest().to_hex(),
        ..RuntimeConfig::default()
    };
    let token = "asset-ref-replay-endpoint-token";
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(212),
    )));
    let business = Arc::new(AssetRefBusiness::new());
    let clock = Arc::new(TestClock::new(1_000));
    let runtime = RuntimeService::new(
        config,
        BootId::from_uuid(Uuid::from_u128(213)),
        token.to_owned(),
        persistence,
        clock.clone(),
        Arc::new(AssetRefResolver),
        business.clone(),
    );
    let mut root_connection = asset_ref_connection(&runtime, token, ClientKind::Hook);
    let mut register = params(
        json!({
            "projectRoot":"C:/ae-sdd-tests/asset-ref-replay",
            "projectKey":"project-asset-ref-replay"
        }),
        1_000,
    );
    register.idempotency_key = Some("asset-ref-replay-workspace".to_owned());
    let workspace: WorkspaceResult = serde_json::from_value(result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::WorkspaceRegister,
        register,
    )))
    .expect("workspace result decodes");
    let mut open = params(
        json!({"externalKey":"asset-ref-replay-root","role":"root","engaged":false}),
        1_000,
    );
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.agent_id = Some("asset-ref-replay-root-agent".to_owned());
    open.work_item_id = Some(WORK_ITEM.to_owned());
    open.idempotency_key = Some("asset-ref-replay-root-open".to_owned());
    let root: SessionResult = serde_json::from_value(result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::SessionOpen,
        open,
    )))
    .expect("root session decodes");
    let mut host = asset_ref_connection(&runtime, token, ClientKind::HostAdapter);
    let mut host_register = params(json!({"adapterId":ADAPTER}), 1_000);
    host_register.capability_token = Some(token.to_owned());
    host_register.idempotency_key = Some("asset-ref-replay-host-register".to_owned());
    result(&asset_ref_call(
        &runtime,
        &mut host,
        RpcMethod::HostRegister,
        host_register,
    ));
    business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":"f".repeat(64),
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "assetRefs":[
            {
                "kind":"constraints-index",
                "path":"constraints/README.md",
                "sha256":"a".repeat(64),
                "byteLength":42
            },
            {
                "kind":"methodology-skill",
                "path":"source/skills/phase1-design/story-generate-skill.md",
                "sha256":"b".repeat(64),
                "byteLength":84
            }
        ],
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"story",
            "requiredArtifacts":["STORY"]
        }
    }));
    let mut next = session_params(
        &workspace,
        &root,
        "asset-ref-replay-root-agent",
        json!({}),
        1_000,
    );
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::FlowNext,
        next,
    ));
    let mut create = session_params(
        &workspace,
        &root,
        "asset-ref-replay-root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("asset-ref-replay-series-create".to_owned());
    let first = result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create,
    ));

    let first_action = result(&asset_ref_call(
        &runtime,
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack":{
                "ackId":"00000000-0000-0000-0000-000000000214",
                "actionId":first_action["actionId"],
                "commandSeq":first_action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"asset-ref-replay-host-task",
                "sessionId":"00000000-0000-0000-0000-000000000215"
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("asset-ref-replay-host-ack".to_owned());
    result(&asset_ref_call(
        &runtime,
        &mut host,
        RpcMethod::HostActionAck,
        ack,
    ));

    business.set_asset_revision(2);
    clock.set(2_000);
    let mut replay = session_params(
        &workspace,
        &root,
        "asset-ref-replay-root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    replay.work_item_id = Some(WORK_ITEM.to_owned());
    replay.idempotency_key = Some("asset-ref-replay-series-create".to_owned());
    let second = result(&asset_ref_call(
        &runtime,
        &mut root_connection,
        RpcMethod::DelegationCreate,
        replay,
    ));

    assert_eq!(second, first, "same-key replay must be byte-identical");
    assert_eq!(
        second["assetRefs"][1]["sha256"],
        "b".repeat(64),
        "the delegation must retain the refs frozen with flow.next"
    );
    assert!(
        result(&asset_ref_call(
            &runtime,
            &mut host,
            RpcMethod::HostActionNext,
            params(json!({"adapterId":ADAPTER}), 2_000),
        ))
        .is_null(),
        "same-key replay must not enqueue a second Host action"
    );
}

#[test]
fn root_series_delegation_rejects_legacy_intent_without_frozen_asset_refs() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "legacy-asset-refs");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "legacy-asset-root-agent",
        "legacy-asset-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"story",
            "requiredArtifacts":["STORY"]
        }
    }));
    let mut next = session_params(
        &workspace,
        &root,
        "legacy-asset-root-agent",
        json!({}),
        1_000,
    );
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let intent_key = format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION);
    let mut legacy = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("intent read succeeds")
        .expect("flow.next stores an intent");
    legacy
        .as_object_mut()
        .expect("intent is an object")
        .remove("assetRefs");
    harness
        .persistence
        .store_record("flow-delegation-intent/v1", &intent_key, &legacy)
        .expect("legacy intent is seeded");

    let mut create = session_params(
        &workspace,
        &root,
        "legacy-asset-root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("legacy-asset-series-create".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create,)),
        "DELEGATION_ATTESTATION_FAILED"
    );
}

#[test]
fn root_series_delegation_rejects_persisted_asset_refs_without_byte_length() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "legacy-byte-length");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "legacy-byte-root-agent",
        "legacy-byte-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":"f".repeat(64),
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "assetRefs":[{
            "kind":"constraints-index",
            "path":"constraints/README.md",
            "sha256":"a".repeat(64),
            "byteLength":42
        }],
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"story",
            "requiredArtifacts":["STORY"]
        }
    }));
    let mut next = session_params(
        &workspace,
        &root,
        "legacy-byte-root-agent",
        json!({}),
        1_000,
    );
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let intent_key = format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION);
    let mut legacy = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("intent read succeeds")
        .expect("flow.next stores an intent");
    legacy["assetRefs"][0]
        .as_object_mut()
        .expect("asset ref object")
        .remove("byteLength");
    harness
        .persistence
        .store_record("flow-delegation-intent/v1", &intent_key, &legacy)
        .expect("legacy intent persists");

    let mut create = session_params(
        &workspace,
        &root,
        "legacy-byte-root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("legacy-byte-create".to_owned());
    let error = harness.call(&mut root_connection, RpcMethod::DelegationCreate, create);
    assert_eq!(stable_error(&error), "DELEGATION_ATTESTATION_FAILED");
    assert!(error.to_string().contains("byteLength"), "{error}");

    let mut refresh = session_params(
        &workspace,
        &root,
        "legacy-byte-root-agent",
        json!({}),
        1_000,
    );
    refresh.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, refresh));
    let mut create = session_params(
        &workspace,
        &root,
        "legacy-byte-root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("legacy-byte-create-after-refresh".to_owned());
    let error = harness.call(&mut root_connection, RpcMethod::DelegationCreate, create);
    assert_eq!(stable_error(&error), "DELEGATION_ATTESTATION_FAILED");
    assert!(error.to_string().contains("byteLength"), "{error}");
}

#[test]
fn flow_next_commits_intent_before_the_next_actor_message_runs() {
    let token = "intent-ordering-endpoint-token";
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(216),
    )));
    let barrier_persistence = Arc::new(IntentLoadBarrierPersistence::new(persistence.clone()));
    let business = Arc::new(IntentOrderingBusiness::new(persistence.clone()));
    let runtime = Arc::new(RuntimeService::new(
        RuntimeConfig {
            policy_digest: ae_sdd_policy::policy_digest().to_hex(),
            work_item_mailbox_capacity: 1,
            ..RuntimeConfig::default()
        },
        BootId::from_uuid(Uuid::from_u128(217)),
        token.to_owned(),
        barrier_persistence.clone(),
        Arc::new(TestClock::new(1_000)),
        Arc::new(AssetRefResolver),
        business.clone(),
    ));
    let mut setup = asset_ref_connection(&runtime, token, ClientKind::Hook);
    let mut register = params(
        json!({"projectRoot":"C:/ae-sdd-tests/intent-ordering","projectKey":"intent-ordering"}),
        1_000,
    );
    register.idempotency_key = Some("intent-ordering-workspace".to_owned());
    let workspace: WorkspaceResult = serde_json::from_value(result(&asset_ref_call(
        &runtime,
        &mut setup,
        RpcMethod::WorkspaceRegister,
        register,
    )))
    .expect("workspace decodes");

    let mut open = params(
        json!({"externalKey":"intent-ordering-root","role":"root","engaged":false}),
        1_000,
    );
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.agent_id = Some("intent-ordering-root-agent".to_owned());
    open.work_item_id = Some(WORK_ITEM.to_owned());
    open.idempotency_key = Some("intent-ordering-root-open".to_owned());
    let root: SessionResult = serde_json::from_value(result(&asset_ref_call(
        &runtime,
        &mut setup,
        RpcMethod::SessionOpen,
        open,
    )))
    .expect("root session decodes");

    let run_flow = |runtime: Arc<RuntimeService>, mut connection: ConnectionState, key: &str| {
        let workspace = workspace.clone();
        let root = root.clone();
        let key = key.to_owned();
        std::thread::spawn(move || {
            let mut next = session_params(
                &workspace,
                &root,
                "intent-ordering-root-agent",
                json!({}),
                1_000,
            );
            next.work_item_id = Some(WORK_ITEM.to_owned());
            next.idempotency_key = Some(key);
            result(&asset_ref_call(
                &runtime,
                &mut connection,
                RpcMethod::FlowNext,
                next,
            ))
        })
    };

    let first = run_flow(
        runtime.clone(),
        asset_ref_connection(&runtime, token, ClientKind::Hook),
        "intent-ordering-flow-a",
    );
    while !barrier_persistence
        .first_load_entered
        .load(Ordering::Acquire)
    {
        std::thread::yield_now();
    }
    let mut second_connection = asset_ref_connection(&runtime, token, ClientKind::Hook);
    let mut second_next = session_params(
        &workspace,
        &root,
        "intent-ordering-root-agent",
        json!({}),
        1_000,
    );
    second_next.work_item_id = Some(WORK_ITEM.to_owned());
    second_next.idempotency_key = Some("intent-ordering-flow-b".to_owned());
    let second_while_capture_blocked = asset_ref_call(
        &runtime,
        &mut second_connection,
        RpcMethod::FlowNext,
        second_next,
    );
    assert_eq!(
        stable_error(&second_while_capture_blocked),
        "SUBSCRIBER_BACKPRESSURE",
        "the first request must still occupy the actor while its intent is persisted"
    );
    assert_eq!(
        business.calls.load(Ordering::Acquire),
        1,
        "a second actor message crossed the blocked first intent capture"
    );
    barrier_persistence.release_first_load();
    first.join().expect("first flow.next completes");

    let second_after_commit = run_flow(
        runtime.clone(),
        asset_ref_connection(&runtime, token, ClientKind::Hook),
        "intent-ordering-flow-b-retry",
    );
    second_after_commit
        .join()
        .expect("second flow.next completes after the committed intent");
    assert!(
        business.second_saw_committed_intent.load(Ordering::Acquire),
        "the next actor message must not run before the first intent is committed"
    );
    let key = format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION);
    let intent = persistence
        .load_record("flow-delegation-intent/v1", &key)
        .expect("intent lookup succeeds")
        .expect("intent is committed");
    assert_eq!(intent["assetRefs"][0]["sha256"], "b".repeat(64));
}

#[test]
fn root_references_committed_flow_intent_instead_of_supplying_authority_fields() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let flow = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));
    assert_eq!(flow["decisionDigest"], DECISION);

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-intent-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    assert_eq!(delegation["childRole"], "series");
    assert!(
        delegation["grant"]["operations"]
            .as_array()
            .is_some_and(|operations| operations.iter().any(|value| value == "document.save")),
        "daemon policy must derive the semantic Series grant: {delegation}"
    );
    assert_eq!(delegation["status"], "spawning");

    let mut forged = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({
            "childRole":"series",
            "parentDelegationId":null,
            "inputRevision":7,
            "inputFingerprint":DECISION,
            "deadlineUnixMs":5_000,
            "adapterId":ADAPTER,
            "grant":{"operations":[],"capabilities":[],"paths":[]}
        }),
        1_000,
    );
    forged.work_item_id = Some(WORK_ITEM.to_owned());
    forged.idempotency_key = Some("series-intent-forged-authority".to_owned());
    let rejected = harness.call(&mut root_connection, RpcMethod::DelegationCreate, forged);
    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");
}

#[test]
fn series_capability_cannot_execute_root_transition() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "series-role-safe");
    let root = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "series-role-root-agent",
        "series-role-root-external",
        Some(WORK_ITEM),
    );
    let delegation = create_root_series_delegation(
        &harness,
        &mut connection,
        &workspace,
        &root,
        "series-role-root-agent",
        WORK_ITEM,
        "coding-plan",
        &["CODING_PLAN"],
        "series-role-create",
    );
    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack":{
                "ackId":"00000000-0000-0000-0000-000000001302",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"series-role-safe-host-task",
                "sessionId":ROLE_SAFE_CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("series-role-safe-ack".to_owned());
    result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));
    let mut accept = params(
        json!({
            "delegationId":delegation["delegationId"],
            "claimId":action["claimId"],
            "actionId":action["actionId"],
            "childSessionId":ROLE_SAFE_CHILD,
            "expiresAtUnixMs":delegation["deadlineUnixMs"].as_u64().expect("deadline") - 100
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some(WORK_ITEM.to_owned());
    accept.idempotency_key = Some("series-role-safe-accept".to_owned());
    result(&harness.call(&mut connection, RpcMethod::DelegationAccept, accept));
    let mut child_open = params(
        json!({
            "externalKey":"series-role-safe-child",
            "role":"series",
            "engaged":false,
            "delegationId":delegation["delegationId"]
        }),
        1_000,
    );
    child_open.workspace_id = Some(workspace.workspace_id.clone());
    child_open.agent_id = Some("series-role-safe-agent".to_owned());
    child_open.session_id = Some(ROLE_SAFE_CHILD.to_owned());
    child_open.work_item_id = Some(WORK_ITEM.to_owned());
    child_open.idempotency_key = Some("series-role-safe-open".to_owned());
    let child: SessionResult = serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        child_open,
    )))
    .expect("child session decodes");

    let calls_before = harness.business.operation_calls.load(Ordering::Acquire);
    let mut transition = session_params(
        &workspace,
        &child,
        "series-role-safe-agent",
        json!({"operation":"state.transition","payload":{"targetPhase":"completed"}}),
        1_000,
    );
    transition.work_item_id = Some(WORK_ITEM.to_owned());
    transition.idempotency_key = Some("series-role-safe-transition".to_owned());
    let denied = harness.call(&mut connection, RpcMethod::OperationExecute, transition);

    assert_eq!(stable_error(&denied), "ROLE_OPERATION_FORBIDDEN");
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        calls_before,
        "a forbidden Root operation must not reach the business adapter"
    );
}

#[test]
fn nested_delegation_inherits_root_host_adapter() {
    const DEDICATED_ADAPTER: &str = "host-lineage-dedicated";
    const SERIES_SESSION: &str = "00000000-0000-0000-0000-000000001401";

    let harness = Harness::new(RuntimeConfig::default());
    let mut default_host = harness.connection_as(ClientKind::HostAdapter, Some("codex"));
    let mut dedicated_host =
        harness.connection_as(ClientKind::HostAdapter, Some(DEDICATED_ADAPTER));
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "nested-host-lineage");

    let mut root_open = params(
        json!({
            "externalKey":"nested-host-lineage-root",
            "role":"root",
            "engaged":false,
            "hostAdapterId":DEDICATED_ADAPTER
        }),
        1_000,
    );
    root_open.workspace_id = Some(workspace.workspace_id.clone());
    root_open.agent_id = Some("nested-host-lineage-root-agent".to_owned());
    root_open.work_item_id = Some(WORK_ITEM.to_owned());
    root_open.idempotency_key = Some("nested-host-lineage-root-open".to_owned());
    let root: SessionResult = serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        root_open,
    )))
    .expect("root session decodes");

    let series = create_root_series_delegation(
        &harness,
        &mut connection,
        &workspace,
        &root,
        "nested-host-lineage-root-agent",
        WORK_ITEM,
        "coding-plan",
        &["CODING_PLAN"],
        "nested-host-lineage-series-create",
    );
    let series_action = result(&harness.call(
        &mut dedicated_host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":DEDICATED_ADAPTER}), 1_000),
    ));
    assert_eq!(series_action["delegationId"], series["delegationId"]);
    assert!(
        result(&harness.call(
            &mut default_host,
            RpcMethod::HostActionNext,
            params(json!({"adapterId":"codex"}), 1_000),
        ))
        .is_null(),
        "the Root binding must override the global codex default"
    );

    let mut ack = params(
        json!({
            "adapterId":DEDICATED_ADAPTER,
            "ack":{
                "ackId":"00000000-0000-0000-0000-000000001402",
                "actionId":series_action["actionId"],
                "commandSeq":series_action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"nested-host-lineage-series",
                "sessionId":SERIES_SESSION
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("nested-host-lineage-series-ack".to_owned());
    result(&harness.call(&mut dedicated_host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":series["delegationId"],
            "claimId":series_action["claimId"],
            "actionId":series_action["actionId"],
            "childSessionId":SERIES_SESSION,
            "expiresAtUnixMs":series["deadlineUnixMs"].as_u64().expect("deadline") - 100
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some(WORK_ITEM.to_owned());
    accept.idempotency_key = Some("nested-host-lineage-series-accept".to_owned());
    result(&harness.call(&mut connection, RpcMethod::DelegationAccept, accept));

    let mut series_open = params(
        json!({
            "externalKey":"nested-host-lineage-series",
            "role":"series",
            "engaged":false,
            "delegationId":series["delegationId"]
        }),
        1_000,
    );
    series_open.workspace_id = Some(workspace.workspace_id.clone());
    series_open.agent_id = Some("nested-host-lineage-series-agent".to_owned());
    series_open.session_id = Some(SERIES_SESSION.to_owned());
    series_open.work_item_id = Some(WORK_ITEM.to_owned());
    series_open.idempotency_key = Some("nested-host-lineage-series-open".to_owned());
    let series_session: SessionResult = serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        series_open,
    )))
    .expect("Series session decodes");

    let mut task_create = session_params(
        &workspace,
        &series_session,
        "nested-host-lineage-series-agent",
        json!({
            "childRole":"task",
            "parentDelegationId":series["delegationId"],
            "inputRevision":2,
            "inputFingerprint":"d".repeat(64),
            "deadlineUnixMs":4_500,
            "grant":{"operations":[],"capabilities":[],"paths":[]},
            "briefing":"prove nested Host adapter lineage"
        }),
        1_000,
    );
    task_create.work_item_id = Some(WORK_ITEM.to_owned());
    task_create.idempotency_key = Some("nested-host-lineage-task-create".to_owned());
    let task = result(&harness.call(&mut connection, RpcMethod::DelegationCreate, task_create));

    let task_action = result(&harness.call(
        &mut dedicated_host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":DEDICATED_ADAPTER}), 1_000),
    ));
    assert_eq!(
        task_action["delegationId"], task["delegationId"],
        "a nested Task must remain on the Root-bound Host adapter: {task_action}"
    );
    assert!(
        result(&harness.call(
            &mut default_host,
            RpcMethod::HostActionNext,
            params(json!({"adapterId":"codex"}), 1_000),
        ))
        .is_null(),
        "the global codex adapter must not capture a nested Task"
    );
}

#[test]
fn root_can_delegate_daemon_committed_coding_work_without_supplying_authority() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "coding-series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "coding-root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":9,
        "phase":"coding",
        "nextAction":{"kind":"await-agent-work"}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("coding-series-intent-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(delegation["childRole"], "series");
    assert_eq!(
        delegation["briefing"],
        "Execute the daemon-committed coding Series"
    );
}

#[test]
fn root_can_delegate_daemon_committed_testing_work_without_supplying_authority() {
    const TEST_DECISION: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "testing-series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "testing-root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":TEST_DECISION,
        "stateRevision":10,
        "phase":"test-running",
        "nextAction":{"kind":"await-agent-work"}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":TEST_DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("testing-series-intent-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(delegation["childRole"], "series");
    assert_eq!(
        delegation["briefing"],
        "Execute the daemon-committed testing Series"
    );
}

#[test]
fn review_ready_flow_commits_a_review_series_intent() {
    const REVIEW_DECISION: &str =
        "abababababababababababababababababababababababababababababababab";
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "review-series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "review-root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":REVIEW_DECISION,
        "inputFingerprint":"e".repeat(64),
        "stateRevision":11,
        "phase":"coding",
        "nextAction":{"kind":"collect-review-contributions"}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":REVIEW_DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("review-series-intent-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(delegation["childRole"], "series");
    assert_eq!(
        delegation["briefing"],
        "Execute the daemon-committed review Series"
    );
    assert!(
        delegation["grant"]["capabilities"]
            .as_array()
            .is_some_and(|capabilities| capabilities
                .iter()
                .any(|capability| capability == "review.specialty.be")),
        "Review Series must be able to narrow a physical reviewer grant: {delegation}"
    );
}

#[test]
fn review_ready_flow_rejects_an_older_coding_series_intent() {
    const CODING_DECISION: &str =
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
    const REVIEW_DECISION: &str =
        "efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef";
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "review-rejects-coding");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "review-rejects-coding-agent",
        "review-rejects-coding-root",
        Some(WORK_ITEM),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":CODING_DECISION,
        "inputFingerprint":"c".repeat(64),
        "stateRevision":10,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let mut coding_next = session_params(
        &workspace,
        &root,
        "review-rejects-coding-agent",
        json!({}),
        1_000,
    );
    coding_next.work_item_id = Some(WORK_ITEM.to_owned());
    coding_next.idempotency_key = Some("capture-coding-intent".to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, coding_next));

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":REVIEW_DECISION,
        "inputFingerprint":"e".repeat(64),
        "stateRevision":11,
        "phase":"coding",
        "nextAction":{"kind":"collect-review-contributions"}
    }));
    let mut review_next = session_params(
        &workspace,
        &root,
        "review-rejects-coding-agent",
        json!({}),
        1_000,
    );
    review_next.work_item_id = Some(WORK_ITEM.to_owned());
    review_next.idempotency_key = Some("capture-review-intent".to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, review_next));

    let mut stale = session_params(
        &workspace,
        &root,
        "review-rejects-coding-agent",
        json!({"flowDecisionDigest":CODING_DECISION}),
        1_000,
    );
    stale.work_item_id = Some(WORK_ITEM.to_owned());
    stale.idempotency_key = Some("reject-stale-coding-intent".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, stale)),
        "DELEGATION_ATTESTATION_FAILED"
    );

    let mut current = session_params(
        &workspace,
        &root,
        "review-rejects-coding-agent",
        json!({"flowDecisionDigest":REVIEW_DECISION}),
        1_000,
    );
    current.work_item_id = Some(WORK_ITEM.to_owned());
    current.idempotency_key = Some("accept-current-review-intent".to_owned());
    let created = result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, current));
    assert_eq!(
        created["briefing"],
        "Execute the daemon-committed review Series"
    );
}

#[test]
fn a_newer_nondelegable_flow_decision_invalidates_the_prior_series_intent() {
    const DELEGABLE_DECISION: &str =
        "1212121212121212121212121212121212121212121212121212121212121212";
    const NONDELEGABLE_DECISION: &str =
        "3434343434343434343434343434343434343434343434343434343434343434";
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "nondelegable-invalidates");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "nondelegable-invalidates-agent",
        "nondelegable-invalidates-root",
        Some(WORK_ITEM),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DELEGABLE_DECISION,
        "inputFingerprint":DELEGABLE_DECISION,
        "stateRevision":10,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let mut delegable = session_params(
        &workspace,
        &root,
        "nondelegable-invalidates-agent",
        json!({}),
        1_000,
    );
    delegable.work_item_id = Some(WORK_ITEM.to_owned());
    delegable.idempotency_key = Some("capture-delegable-before-pause".to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, delegable));

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":NONDELEGABLE_DECISION,
        "inputFingerprint":NONDELEGABLE_DECISION,
        "stateRevision":11,
        "phase":"coding",
        "nextAction":{"kind":"finalize-governance"}
    }));
    let mut paused = session_params(
        &workspace,
        &root,
        "nondelegable-invalidates-agent",
        json!({}),
        1_000,
    );
    paused.work_item_id = Some(WORK_ITEM.to_owned());
    paused.idempotency_key = Some("capture-nondelegable-pause".to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, paused));

    let mut stale = session_params(
        &workspace,
        &root,
        "nondelegable-invalidates-agent",
        json!({"flowDecisionDigest":DELEGABLE_DECISION}),
        1_000,
    );
    stale.work_item_id = Some(WORK_ITEM.to_owned());
    stale.idempotency_key = Some("reject-intent-before-pause".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, stale)),
        "DELEGATION_ATTESTATION_FAILED"
    );
}

#[test]
fn an_expired_frozen_series_intent_cannot_create_a_delegation() {
    let harness = Harness::new(RuntimeConfig {
        session_ttl_ms: 4_000_000,
        ..RuntimeConfig::default()
    });
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "expired-series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "expired-series-intent-agent",
        "expired-series-intent-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":12,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let mut next = session_params(
        &workspace,
        &root,
        "expired-series-intent-agent",
        json!({}),
        1_000,
    );
    next.work_item_id = Some(WORK_ITEM.to_owned());
    next.idempotency_key = Some("capture-expiring-series-intent".to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let intent_key = format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION);
    let intent = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("intent lookup succeeds")
        .expect("flow.next commits the intent");
    let deadline = intent["deadlineUnixMs"]
        .as_u64()
        .expect("intent freezes a deadline");
    harness.clock.set(deadline.saturating_add(1));

    let mut create = session_params(
        &workspace,
        &root,
        "expired-series-intent-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("reject-expired-series-intent".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create)),
        "DELEGATION_ATTESTATION_FAILED"
    );
    assert_eq!(
        result(&harness.call(
            &mut host,
            RpcMethod::HostActionNext,
            params(json!({"adapterId":ADAPTER}), 1_000),
        )),
        Value::Null,
        "an expired intent must not enqueue a Host create action"
    );
}

#[test]
fn a_committed_series_create_replays_after_its_frozen_deadline() {
    let harness = Harness::new(RuntimeConfig {
        session_ttl_ms: 4_000_000,
        ..RuntimeConfig::default()
    });
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "expired-create-replay");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "expired-create-replay-agent",
        "expired-create-replay-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":13,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let mut next = session_params(
        &workspace,
        &root,
        "expired-create-replay-agent",
        json!({}),
        1_000,
    );
    next.work_item_id = Some(WORK_ITEM.to_owned());
    next.idempotency_key = Some("capture-create-replay-intent".to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let create_request = || {
        let mut create = session_params(
            &workspace,
            &root,
            "expired-create-replay-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some(WORK_ITEM.to_owned());
        create.idempotency_key = Some("committed-create-before-expiry".to_owned());
        create
    };
    let created = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_request(),
    ));
    let deadline = created["deadlineUnixMs"]
        .as_u64()
        .expect("created delegation carries the frozen deadline");
    harness.clock.set(deadline.saturating_add(1));

    let replayed = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_request(),
    ));
    assert_eq!(
        replayed, created,
        "an exact retry must replay the committed receipt"
    );
}

#[test]
fn a_committed_series_create_replays_after_its_intent_is_renewed() {
    let harness = Harness::new(RuntimeConfig {
        session_ttl_ms: 4_000_000,
        ..RuntimeConfig::default()
    });
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "renewed-create-replay");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "renewed-create-replay-agent",
        "renewed-create-replay-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":14,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let flow_request = |key: &str| {
        let mut next = session_params(
            &workspace,
            &root,
            "renewed-create-replay-agent",
            json!({}),
            1_000,
        );
        next.work_item_id = Some(WORK_ITEM.to_owned());
        next.idempotency_key = Some(key.to_owned());
        next
    };
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("capture-renewed-create-replay-intent"),
    ));

    let create_request = || {
        let mut create = session_params(
            &workspace,
            &root,
            "renewed-create-replay-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some(WORK_ITEM.to_owned());
        create.idempotency_key = Some("committed-create-before-renewal".to_owned());
        create
    };
    let created = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_request(),
    ));
    let deadline = created["deadlineUnixMs"]
        .as_u64()
        .expect("created delegation carries the frozen deadline");
    harness.clock.set(deadline.saturating_add(1));
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("renew-create-replay-intent"),
    ));

    let replayed = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_request(),
    ));
    assert_eq!(
        replayed, created,
        "renewing daemon-derived intent authority must not change the caller request digest"
    );
}

#[test]
fn a_committed_series_create_replays_after_a_newer_flow_intent() {
    let harness = Harness::new(RuntimeConfig {
        session_ttl_ms: 4_000_000,
        ..RuntimeConfig::default()
    });
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "superseded-create-replay");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "superseded-create-replay-agent",
        "superseded-create-replay-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":15,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let flow_request = |key: &str| {
        let mut next = session_params(
            &workspace,
            &root,
            "superseded-create-replay-agent",
            json!({}),
            1_000,
        );
        next.work_item_id = Some(WORK_ITEM.to_owned());
        next.idempotency_key = Some(key.to_owned());
        next
    };
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("capture-first-create-replay-intent"),
    ));

    let create_request = || {
        let mut create = session_params(
            &workspace,
            &root,
            "superseded-create-replay-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some(WORK_ITEM.to_owned());
        create.idempotency_key = Some("committed-create-before-supersession".to_owned());
        create
    };
    let created = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_request(),
    ));

    let newer_decision = "d".repeat(64);
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":newer_decision,
        "inputFingerprint":"e".repeat(64),
        "stateRevision":16,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":2}
    }));
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("capture-newer-create-replay-intent"),
    ));

    let replayed = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_request(),
    ));
    assert_eq!(
        replayed, created,
        "an exact retry must replay after current flow authority advances"
    );
}

#[test]
fn rerunning_flow_next_renews_an_expired_series_intent() {
    let harness = Harness::new(RuntimeConfig {
        session_ttl_ms: 4_000_000,
        ..RuntimeConfig::default()
    });
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "expired-intent-renewal");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "expired-intent-renewal-agent",
        "expired-intent-renewal-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":14,
        "phase":"coding",
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let flow_request = |key: &str| {
        let mut next = session_params(
            &workspace,
            &root,
            "expired-intent-renewal-agent",
            json!({}),
            1_000,
        );
        next.work_item_id = Some(WORK_ITEM.to_owned());
        next.idempotency_key = Some(key.to_owned());
        next
    };
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("capture-expiring-renewal-intent"),
    ));
    let intent_key = format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION);
    let first = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("intent lookup succeeds")
        .expect("first intent exists");
    let first_deadline = first["deadlineUnixMs"]
        .as_u64()
        .expect("first intent freezes a deadline");
    harness.clock.set(first_deadline.saturating_add(1));

    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("renew-expired-series-intent"),
    ));
    let renewed = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("renewed intent lookup succeeds")
        .expect("renewed intent exists");
    let renewed_deadline = renewed["deadlineUnixMs"]
        .as_u64()
        .expect("renewed intent freezes a deadline");
    assert!(renewed_deadline > first_deadline);

    let mut create = session_params(
        &workspace,
        &root,
        "expired-intent-renewal-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("create-from-renewed-intent".to_owned());
    let created = result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    assert_eq!(created["deadlineUnixMs"], renewed_deadline);
}

#[test]
fn renewing_an_expired_series_intent_preserves_its_frozen_authority() {
    let harness = Harness::new(RuntimeConfig {
        session_ttl_ms: 4_000_000,
        ..RuntimeConfig::default()
    });
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "frozen-intent-renewal");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "frozen-intent-renewal-agent",
        "frozen-intent-renewal-root",
        Some(WORK_ITEM),
    );
    let first_asset = json!({
        "kind":"methodology-skill",
        "path":"source/skills/phase2-coding/coding-skill.md",
        "sha256":"a".repeat(64),
        "byteLength":128
    });
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":17,
        "phase":"coding",
        "flowRunId":"flow-original",
        "retryOfSeriesRunId":"series-original",
        "assetRefs":[first_asset],
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let flow_request = |key: &str| {
        let mut next = session_params(
            &workspace,
            &root,
            "frozen-intent-renewal-agent",
            json!({}),
            1_000,
        );
        next.work_item_id = Some(WORK_ITEM.to_owned());
        next.idempotency_key = Some(key.to_owned());
        next
    };
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("capture-frozen-renewal-intent"),
    ));
    let intent_key = format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION);
    let first = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("first intent lookup succeeds")
        .expect("first intent exists");
    let first_deadline = first["deadlineUnixMs"]
        .as_u64()
        .expect("first intent freezes a deadline");
    harness.clock.set(first_deadline.saturating_add(1));

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":17,
        "phase":"coding",
        "flowRunId":"flow-drifted",
        "retryOfSeriesRunId":"series-drifted",
        "assetRefs":[{
            "kind":"methodology-skill",
            "path":"source/skills/phase3-review/code-review-skill.md",
            "sha256":"b".repeat(64),
            "byteLength":256
        }],
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("renew-with-drifted-projection"),
    ));
    let renewed = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("renewed intent lookup succeeds")
        .expect("renewed intent exists");

    assert!(renewed["deadlineUnixMs"].as_u64().unwrap() > first_deadline);
    for field in [
        "workspaceId",
        "workItemId",
        "inputFingerprint",
        "decisionDigest",
        "seriesKind",
        "stateRevision",
        "retryOfSeriesRunId",
        "flowRunId",
        "requiredArtifacts",
        "assetRefs",
    ] {
        assert_eq!(renewed[field], first[field], "{field} must remain frozen");
    }
}

#[test]
fn renewing_a_legacy_expired_series_intent_does_not_backfill_frozen_authority() {
    let harness = Harness::new(RuntimeConfig {
        session_ttl_ms: 4_000_000,
        ..RuntimeConfig::default()
    });
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "legacy-renewal-authority");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "legacy-renewal-authority-agent",
        "legacy-renewal-authority-root",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":18,
        "phase":"coding",
        "flowRunId":"flow-original",
        "retryOfSeriesRunId":"series-original",
        "assetRefs":[{
            "kind":"methodology-skill",
            "path":"source/skills/phase2-coding/coding-skill.md",
            "sha256":"a".repeat(64),
            "byteLength":128
        }],
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    let flow_request = |key: &str| {
        let mut next = session_params(
            &workspace,
            &root,
            "legacy-renewal-authority-agent",
            json!({}),
            1_000,
        );
        next.work_item_id = Some(WORK_ITEM.to_owned());
        next.idempotency_key = Some(key.to_owned());
        next
    };
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("capture-legacy-renewal-authority"),
    ));
    let intent_key = format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION);
    let mut legacy = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("intent lookup succeeds")
        .expect("initial intent exists");
    let first_deadline = legacy["deadlineUnixMs"]
        .as_u64()
        .expect("initial deadline exists");
    legacy
        .as_object_mut()
        .expect("intent is an object")
        .remove("assetRefs");
    harness
        .persistence
        .store_record("flow-delegation-intent/v1", &intent_key, &legacy)
        .expect("legacy intent is seeded");
    harness.clock.set(first_deadline.saturating_add(1));

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":DECISION,
        "stateRevision":18,
        "phase":"coding",
        "flowRunId":"flow-drifted",
        "retryOfSeriesRunId":"series-drifted",
        "assetRefs":[{
            "kind":"methodology-skill",
            "path":"source/skills/phase3-review/code-review-skill.md",
            "sha256":"b".repeat(64),
            "byteLength":256
        }],
        "nextAction":{"kind":"execute-approved-slice","activeOrdinal":1}
    }));
    result(&harness.call(
        &mut root_connection,
        RpcMethod::FlowNext,
        flow_request("renew-legacy-with-drifted-authority"),
    ));
    let renewed = harness
        .persistence
        .load_record("flow-delegation-intent/v1", &intent_key)
        .expect("renewed intent lookup succeeds")
        .expect("renewed intent exists");

    assert!(renewed["deadlineUnixMs"].as_u64().unwrap() > first_deadline);
    assert!(
        renewed["assetRefs"].is_null(),
        "legacy assetRefs must remain absent"
    );
    assert_eq!(renewed["retryOfSeriesRunId"], "series-original");
    assert_eq!(renewed["flowRunId"], "flow-original");
}

/// `ae-sdd-daemon-audit-report.md` F-10: the flow decision carries
/// `inputFingerprint` and `decisionDigest` as separate proofs, but the committed
/// intent copied the decision digest into the fingerprint slot and then required
/// the two to be equal. That makes input freshness unobservable — the same
/// decision taken against newer Spec content produces an identical fingerprint,
/// so nothing can tell the two apart.
///
/// Here the flow reports a fingerprint that genuinely differs from the decision
/// digest, which is the normal case once the fingerprint is computed from state
/// revision, DocumentVersion refs, context bundle, policy digest and inventory
/// generation.
#[test]
fn committed_intent_preserves_a_fingerprint_distinct_from_the_decision_digest() {
    const FRESHNESS: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-fingerprint");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":FRESHNESS,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let flow = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));
    assert_eq!(flow["decisionDigest"], DECISION);

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-fingerprint-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(
        delegation["status"], "spawning",
        "a fingerprint that differs from the decision digest is the normal case, \
         not an attestation failure: {delegation}"
    );
    let record = harness
        .persistence
        .load_record(
            "delegation/v1",
            delegation["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("the delegation was committed");

    assert_eq!(
        record["inputFingerprint"].as_str(),
        Some(FRESHNESS),
        "the committed intent must carry the flow's own fingerprint, not a copy \
         of the decision digest: {record}"
    );
    assert_ne!(
        record["inputFingerprint"].as_str(),
        Some(DECISION),
        "conflating the two proofs makes input freshness unobservable"
    );
}

#[test]
fn v2_flow_digest_replaces_a_legacy_current_intent_at_the_same_revision() {
    const FRESHNESS: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const LEGACY_DECISION: &str =
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-digest-upgrade");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness
        .persistence
        .store_record(
            "flow-delegation-current/v1",
            &format!("{}\0{}", workspace.workspace_id, WORK_ITEM),
            &json!({
                "schemaVersion":"flow-delegation-current/v1",
                "workspaceId":workspace.workspace_id,
                "workItemId":WORK_ITEM,
                "decisionDigest":LEGACY_DECISION,
                "stateRevision":7,
                "inputFingerprint":FRESHNESS
            }),
        )
        .expect("legacy current intent stores");
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigestVersion":"v2",
        "decisionDigest":DECISION,
        "inputFingerprint":FRESHNESS,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{"kind":"delegate-series","seriesKind":"design-review","requiredArtifacts":["DR"]}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let flow = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));
    assert_eq!(flow["decisionDigest"], DECISION);
    let current = harness
        .persistence
        .load_record(
            "flow-delegation-current/v1",
            &format!("{}\0{}", workspace.workspace_id, WORK_ITEM),
        )
        .expect("current loads")
        .expect("current exists");
    assert_eq!(current["decisionDigest"], DECISION);
    assert_eq!(current["decisionDigestVersion"], "v2");
}

#[test]
fn v2_flow_digest_conflict_at_the_same_revision_stays_fail_closed() {
    const FRESHNESS: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const PRIOR_DECISION: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let harness = Harness::new(RuntimeConfig::default());
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-v2-conflict");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.persistence.store_record("flow-delegation-current/v1", &format!("{}\0{}", workspace.workspace_id, WORK_ITEM), &json!({
        "schemaVersion":"flow-delegation-current/v1","workspaceId":workspace.workspace_id,"workItemId":WORK_ITEM,
        "decisionDigest":PRIOR_DECISION,"decisionDigestVersion":"v2","stateRevision":7,"inputFingerprint":FRESHNESS
    })).expect("v2 current intent stores");
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigestVersion":"v2","decisionDigest":DECISION,
        "inputFingerprint":FRESHNESS,"stateRevision":7,"phase":"requirement-analyzed",
        "nextAction":{"kind":"delegate-series","seriesKind":"design-review","requiredArtifacts":["DR"]}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let rejected = harness.call(&mut root_connection, RpcMethod::FlowNext, next);
    assert_eq!(stable_error(&rejected), "DELEGATION_ATTESTATION_FAILED");
    assert!(
        harness
            .persistence
            .load_record(
                "flow-delegation-intent/v1",
                &format!("{}\0{}\0{}", workspace.workspace_id, WORK_ITEM, DECISION),
            )
            .expect("candidate intent lookup succeeds")
            .is_none(),
        "a rejected current-pointer conflict must not leave an orphan delegation intent"
    );
}

#[test]
fn same_revision_series_boundary_refreshes_the_same_semantic_delegation_intent() {
    const FIRST_DECISION: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const FIRST_FINGERPRINT: &str =
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const NEXT_FINGERPRINT: &str =
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let harness = Harness::new(RuntimeConfig::default());
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "same-revision-boundary");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigestVersion":"v2",
        "decisionDigest":FIRST_DECISION,
        "inputFingerprint":FIRST_FINGERPRINT,
        "stateRevision":9,
        "phase":"requirement-analyzed",
        "nextAction":{"kind":"delegate-series","seriesKind":"story","requiredArtifacts":["STORY"]}
    }));
    let mut first = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    first.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, first));

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigestVersion":"v2",
        "decisionDigest":DECISION,
        "inputFingerprint":NEXT_FINGERPRINT,
        "stateRevision":9,
        "phase":"requirement-analyzed",
        "nextAction":{"kind":"delegate-series","seriesKind":"story","requiredArtifacts":["STORY"]}
    }));
    let mut after_boundary = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    after_boundary.work_item_id = Some(WORK_ITEM.to_owned());
    let refreshed =
        result(&harness.call(&mut root_connection, RpcMethod::FlowNext, after_boundary));

    assert_eq!(refreshed["decisionDigest"], DECISION);
    let current = harness
        .persistence
        .load_record(
            "flow-delegation-current/v1",
            &format!("{}\0{}", workspace.workspace_id, WORK_ITEM),
        )
        .expect("current loads")
        .expect("current exists");
    assert_eq!(current["decisionDigest"], DECISION);
    assert_eq!(current["inputFingerprint"], NEXT_FINGERPRINT);
}

#[test]
fn same_revision_cannot_replace_a_different_series_intent() {
    const FIRST_DECISION: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let harness = Harness::new(RuntimeConfig::default());
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "same-revision-series-swap");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigestVersion":"v2",
        "decisionDigest":FIRST_DECISION,"inputFingerprint":"e".repeat(64),
        "stateRevision":9,"phase":"requirement-analyzed",
        "nextAction":{"kind":"delegate-series","seriesKind":"design-review","requiredArtifacts":["DR"]}
    }));
    let mut first = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    first.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, first));

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigestVersion":"v2",
        "decisionDigest":DECISION,"inputFingerprint":"f".repeat(64),
        "stateRevision":9,"phase":"requirement-analyzed",
        "nextAction":{"kind":"delegate-series","seriesKind":"story","requiredArtifacts":["STORY"]}
    }));
    let mut swapped = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    swapped.work_item_id = Some(WORK_ITEM.to_owned());
    let rejected = harness.call(&mut root_connection, RpcMethod::FlowNext, swapped);

    assert_eq!(stable_error(&rejected), "DELEGATION_ATTESTATION_FAILED");
}

/// F-06: "当前 delegation ID 不能替代 SeriesRunId，因为一次 Series 重试可能产生
/// 新的物理运行和新的 delegation，但仍属于同一逻辑 Series."
///
/// So a delegation record has to carry three separable facts: which logical
/// Series this is, which *attempt* of it this is, and which attempt it replaces.
/// Without `seriesRunId` the two attempts are only distinguishable by their
/// delegation ids, which says nothing about them being the same Series; without
/// `retryOf` the history is a flat list of unrelated delegations, so "this Series
/// was retried twice" is not answerable after a restart.
#[test]
fn a_series_retry_gets_a_new_run_identity_that_still_names_what_it_replaces() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-retry");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut first_create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    first_create.work_item_id = Some(WORK_ITEM.to_owned());
    first_create.idempotency_key = Some("retry-attempt-1".to_owned());
    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        first_create,
    ));
    let first_record = harness
        .persistence
        .load_record(
            "delegation/v1",
            first["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("first attempt committed");

    let first_run = first_record["seriesRunId"]
        .as_str()
        .expect("a delegation must name the Series attempt it is running");
    assert!(
        first_record["retryOf"].is_null(),
        "a first attempt replaces nothing: {first_record}"
    );
    // F-06 is explicit that "当前 delegation ID 不能替代 SeriesRunId". Aliasing the
    // two would pass a two-attempt inequality check for the wrong reason, while
    // still leaving the attempt indistinguishable from the delegation edge.
    assert_ne!(
        first_record["seriesRunId"].as_str(),
        first_record["delegationId"].as_str(),
        "the attempt identity must be its own key, not an alias of the delegation          edge: {first_record}"
    );

    // ROUTE-C: the per-root-session concurrency guard is gone, so a retry
    // could in principle open while the first attempt is still `spawning`.
    // This test still accepts the first attempt before retrying, because the
    // point under test is the *run identity* change across attempts, not the
    // concurrency model — keeping the sequence linear keeps the assertion
    // honest about what it is proving.
    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001202",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-series-retry",
                "sessionId":RETRY_CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("series-retry-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":first["delegationId"],
            "claimId":claim_id,
            "actionId":action["actionId"],
            "childSessionId":RETRY_CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some(WORK_ITEM.to_owned());
    accept.idempotency_key = Some("series-retry-accept".to_owned());
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    // Retry lineage is authority, so it arrives on the flow decision — a root
    // cannot name its own predecessor. Re-running `flow.next` with the retry edge
    // is what a real retry looks like.
    // The retry is a *new* decision, so it carries its own digest. This is what
    // makes the stable-Series claim testable: an implementation that derived
    // `seriesId` from anything per-decision or per-attempt would now disagree
    // across the two records, while a correct one still resolves to one Series.
    const RETRY_DECISION: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":RETRY_DECISION,
        "stateRevision":8,
        "phase":"requirement-analyzed",
        "retryOfSeriesRunId":first_run,
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut retry_next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    retry_next.work_item_id = Some(WORK_ITEM.to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, retry_next));

    let mut retry_create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":RETRY_DECISION}),
        1_000,
    );
    retry_create.work_item_id = Some(WORK_ITEM.to_owned());
    retry_create.idempotency_key = Some("retry-attempt-2".to_owned());
    let retry = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        retry_create,
    ));
    let retry_record = harness
        .persistence
        .load_record(
            "delegation/v1",
            retry["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("retry committed");

    assert_ne!(
        retry_record["seriesRunId"].as_str(),
        Some(first_run),
        "a retry is a new physical run, so it cannot reuse the run identity"
    );
    assert_eq!(
        retry_record["retryOf"].as_str(),
        Some(first_run),
        "and it must still name the attempt it replaces, or the retry history is \
         a flat list of unrelated delegations: {retry_record}"
    );

    // D-03 item 3 separates a *stable* SeriesId from the per-attempt run. Without
    // it the two attempts carry a `retryOf` edge but no shared owner, so "all
    // attempts of this Series" is not a query — which is precisely the
    // independent-queryability the completion criterion asks for.
    let series = first_record["seriesId"]
        .as_str()
        .expect("a delegation must name the logical Series it is an attempt of");
    assert_eq!(
        retry_record["seriesId"].as_str(),
        Some(series),
        "a retry is a new run of the *same* logical Series: {retry_record}"
    );
    assert_ne!(
        first_record["seriesId"].as_str(),
        first_record["seriesRunId"].as_str(),
        "the stable Series and the attempt must be separate keys"
    );

    // The separation above is only useful if the projection preserves it. Line 767
    // requires the execution tree not be polluted by retries, and a projection keyed
    // by the logical Series would collapse both attempts onto one row — the defect
    // the `series_plan_projection` table still has, whose primary key is
    // `(workspace_id, series_id)`.
    let runs = harness
        .persistence
        .list_records("series_run/v1")
        .expect("series_run namespace is listable");
    assert_eq!(
        runs.len(),
        2,
        "two attempts at one Series must occupy two rows, not overwrite one: {runs:?}"
    );
    let by_series: Vec<_> = runs
        .iter()
        .filter(|(_, value)| value["seriesId"].as_str() == Some(series))
        .collect();
    assert_eq!(
        by_series.len(),
        2,
        "and both must be reachable from the stable SeriesId, which is what makes          \"every attempt of this Series\" one query instead of a delegation scan"
    );
    let retry_projection = runs
        .iter()
        .find(|(_, value)| value["retryOf"].as_str() == Some(first_run))
        .expect("the replacing attempt is identifiable by the run it replaces");
    assert_ne!(
        retry_projection.1["seriesRunId"].as_str(),
        Some(first_run),
        "the retry row is the new attempt, not a rewrite of the old one"
    );
}

/// The pre-F-10 compatibility read, pinned as deliberate rather than incidental.
///
/// A flow decision minted before F-10 emits no `inputFingerprint`, so
/// `service_host.rs` falls back to the decision digest to fill the slot. That
/// fallback is load-bearing: the committed intent is rejected when the
/// fingerprint is empty, so without it every legacy decision would fail
/// delegation with an attestation error that names nothing about compatibility.
///
/// Five other tests in this file already omit `inputFingerprint` and therefore
/// exercise this path, but none assert it, so deleting the fallback would surface
/// as an unrelated attestation failure rather than a compatibility regression.
/// This test states the contract: a legacy decision still delegates, and the
/// fingerprint it lands on is the digest — which is exactly why such a decision
/// carries no freshness signal and must not be treated as if it does.
#[test]
fn a_pre_f10_decision_without_a_fingerprint_still_delegates_via_the_digest() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-legacy");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let flow = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));
    assert!(
        flow.get("inputFingerprint").is_none(),
        "this fixture models a pre-F-10 decision, which emits no fingerprint: {flow}"
    );

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-legacy-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(
        delegation["status"], "spawning",
        "a legacy decision must remain delegable, or the compatibility read is \
         not actually compatible: {delegation}"
    );
    let record = harness
        .persistence
        .load_record(
            "delegation/v1",
            delegation["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("the delegation was committed");

    assert_eq!(
        record["inputFingerprint"].as_str(),
        Some(DECISION),
        "with no fingerprint of its own, a legacy decision lands on the digest: \
         {record}"
    );
}

/// D-03 item 5 and §4.2: the execution flow tree must be queryable apart from the
/// delegation tree.
///
/// The delegation record cannot answer this on its own — it is keyed by
/// `delegationId`, so "every attempt of this Series" means listing all delegations
/// and filtering their payloads. This projection is keyed by `seriesRunId` and
/// carries `seriesId`, which is what makes the query direct rather than a scan.
#[test]
fn creating_a_series_delegation_publishes_a_queryable_series_run_projection() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-run-proj");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "flowRunId":"0192f0c0-1111-7000-8000-000000000001",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-run-proj-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    let records = harness
        .persistence
        .list_records("series_run/v1")
        .expect("series_run namespace is listable");
    assert_eq!(
        records.len(),
        1,
        "one Series delegation is one attempt, so one projection"
    );
    let (key, projection) = &records[0];
    // Read from the durable record, not the response: `project_delegation` exposes
    // ten fields and the attempt identity is not among them, so a caller currently
    // cannot correlate its delegation to the Series Run. That is a separate gap from
    // D-03 item 5, which asks only that the runs themselves be queryable.
    let stored = harness
        .persistence
        .load_record(
            "delegation/v1",
            delegation["delegationId"].as_str().expect("id"),
        )
        .expect("delegation record is readable")
        .expect("the create wrote a delegation record");
    let series_run_id = stored["seriesRunId"]
        .as_str()
        .expect("the delegation record names its attempt")
        .to_owned();
    let series_run_id = series_run_id.as_str();
    assert!(
        key.contains(series_run_id),
        "the projection is keyed per attempt, not per logical Series: keying by \
         seriesId would make a retry overwrite the attempt it replaces, which is \
         the pollution line 767 forbids"
    );
    assert_eq!(projection["seriesRunId"], json!(series_run_id));
    assert_eq!(
        projection["seriesId"], stored["seriesId"],
        "the stable Series is carried so all attempts of it are one query"
    );
    assert_eq!(
        projection["flowRunId"],
        json!("0192f0c0-1111-7000-8000-000000000001"),
        "§4.2 needs FR -> Series Run, so the attempt names its Flow Run — taken \
         from the committed decision, never from the root payload"
    );
    assert_eq!(
        projection["lifecycleState"],
        json!("spawn_requested"),
        "§7 rule 13 forbids a Series showing as running before the child claims it, \
         so a spawning delegation must not project as running"
    );
}

/// The projection carries a §11.2 lifecycle state as a bare string, so it can drift
/// from the frozen `SeriesLifecycleState` vocabulary without anything noticing. This
/// pins every value the mapping can emit against the typed enum's own wire form.
#[test]
fn the_projected_lifecycle_states_are_all_frozen_contract_spellings() {
    use ae_sdd_contracts::SeriesLifecycleState;

    for (state, expected) in [
        (SeriesLifecycleState::SpawnRequested, "spawn_requested"),
        (SeriesLifecycleState::Running, "running"),
        (SeriesLifecycleState::ResultStaged, "result_staged"),
        (SeriesLifecycleState::Validated, "validated"),
        (SeriesLifecycleState::Completed, "completed"),
        (SeriesLifecycleState::Cancelled, "cancelled"),
    ] {
        assert_eq!(
            serde_json::to_value(state).expect("serialize"),
            json!(expected),
            "the projection emits {expected}, so the frozen contract must spell it \
             that way or the two representations have diverged"
        );
    }
}
