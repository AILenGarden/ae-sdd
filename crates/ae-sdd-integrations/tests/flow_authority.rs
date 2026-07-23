use std::fs;
use std::str::FromStr;
use std::sync::Arc;

use ae_sdd_domain::{
    AgentRole, BootId, EventStoreId, LeaseId, OperationId, ProjectPathScope, ProjectRelativePath,
    ScopedGrant,
};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_operations::OperationName;
use ae_sdd_protocol::{
    ConfirmationRef, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, MemoryPersistence, PersistencePort,
};
use ae_sdd_store::{
    LeaseOwner, ProjectMutationStore, ProjectStorePaths, SqliteRuntimeRepository,
    StdCrossProcessLock, StdDurableFileSystem, UtcTimestamp,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

fn workspace(root: &TempDir, role: Option<AgentRole>) -> BusinessWorkspace {
    let grant = role.map(|_| {
        ScopedGrant::new(
            OperationName::ALL
                .into_iter()
                .filter(|operation| *operation != OperationName::LeaseBreak)
                .map(|operation| OperationId::new(operation.as_str()).expect("operation")),
            [],
            [ProjectPathScope::ProjectRoot],
        )
    });
    BusinessWorkspace {
        workspace_id: Uuid::from_u128(11).to_string(),
        canonical_root: root.path().to_string_lossy().into_owned(),
        project_key: "flow-test".to_owned(),
        mode: WorkspaceMode::RustCanary,
        agent_role: role,
        agent_grant: grant,
        caller_kind: None,
        inventory_generation: 4,
    }
}

fn state(root: &TempDir) {
    let directory = root.path().join(".auto-engineering/flow-test");
    fs::create_dir_all(&directory).expect("state directory");
    fs::write(
        directory.join("state.json"),
        serde_json::to_vec_pretty(&json!({
            "stateMachineName":"PRD-FLOW-001",
            "activeStory":"STORY-FLOW-001",
            "revision":7,
            "lastFencingToken":3,
            "scale":"large",
            "selectedDesign":"DR",
            "phase":"completed",
            "currentPhase":"completed",
            "documentPaths":{"STORY":"ae-sdd-doc/Story/STORY-FLOW-001.md"},
            "nextActions":[{"kind":"poisoned-caller-projection"}],
            "gateResults":{"G-00":"PASS"},
            "storyStates":{
                "STORY-FLOW-001":{
                    "phase":"coding",
                    "currentPhase":"coding",
                    "nextActions":[{"kind":"also-poisoned"}]
                }
            }
        }))
        .expect("state JSON"),
    )
    .expect("write state");
}

fn complete_project_assets(root: &TempDir) {
    let constraints = root.path().join("constraints");
    fs::create_dir_all(&constraints).expect("constraints directory");
    for index in 0..5 {
        fs::write(
            constraints.join(format!("constraint-{index}.md")),
            "# constraint",
        )
        .expect("constraint file");
    }
    fs::write(root.path().join("Cargo.toml"), "[workspace]\n").expect("Cargo.toml");
}

fn params(payload: Value, key: Option<&str>) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(Uuid::from_u128(11).to_string()),
        agent_id: Some("agent-root".to_owned()),
        session_id: Some(Uuid::from_u128(12).to_string()),
        capability_token: None,
        turn_id: None,
        work_item_id: Some("STORY-FLOW-001".to_owned()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: key.map(str::to_owned),
        confirmation: None,
        deadline_ms: 1_000,
        payload,
    }
}

fn adapter(root: &TempDir) -> (NativeBusinessAdapter, Arc<MemoryPersistence>) {
    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(13));
    let persistence = Arc::new(MemoryPersistence::new(event_store_id));
    let port: Arc<dyn PersistencePort> = persistence.clone();
    (
        NativeBusinessAdapter::new(
            root.path().join("runtime.sqlite3"),
            event_store_id,
            BootId::from_uuid(Uuid::from_u128(14)),
            "0".repeat(64),
            port,
        ),
        persistence,
    )
}

#[test]
fn snapshot_uses_nested_story_and_ignores_stored_next_actions() {
    let root = TempDir::new().expect("tempdir");
    state(&root);
    let (adapter, _) = adapter(&root);
    let projection = adapter
        .execute(
            RpcMethod::FlowSnapshot,
            &params(json!({}), None),
            Some(&workspace(&root, None)),
        )
        .expect("flow projection");

    assert_eq!(projection["phase"], "coding");
    assert_eq!(projection["nextAction"]["kind"], "await-agent-work");
    assert_ne!(
        projection["nextAction"]["kind"],
        "poisoned-caller-projection"
    );
}

#[test]
fn root_transition_is_durable_and_same_key_replays_once() {
    let root = TempDir::new().expect("tempdir");
    state(&root);
    let (adapter, persistence) = adapter(&root);
    let request = params(
        json!({"targetPhase":"test-running"}),
        Some("flow-transition-1"),
    );
    let trusted = workspace(&root, Some(AgentRole::Root));
    let first = adapter
        .execute(RpcMethod::FlowNext, &request, Some(&trusted))
        .expect("transition request");
    let second = adapter
        .execute(RpcMethod::FlowNext, &request, Some(&trusted))
        .expect("idempotent replay");

    assert_eq!(first, second);
    assert_eq!(first["pendingTransition"], "test-running");
    assert_eq!(first["requiredGates"], json!(["G-00"]));
    assert_eq!(persistence.latest_event_sequence().expect("sequence"), 1);
}

#[test]
fn non_root_cannot_request_transition() {
    let root = TempDir::new().expect("tempdir");
    state(&root);
    let (adapter, _) = adapter(&root);
    let error = adapter
        .execute(
            RpcMethod::FlowNext,
            &params(
                json!({"targetPhase":"test-running"}),
                Some("flow-transition-series"),
            ),
            Some(&workspace(&root, Some(AgentRole::Series))),
        )
        .expect_err("series transition must fail closed");

    assert_eq!(
        error.code(),
        ae_sdd_protocol::StableErrorCode::RoleOperationForbidden
    );
}

#[test]
fn requested_operation_cannot_self_grant_or_escape_the_trusted_path_scope() {
    let root = TempDir::new().expect("tempdir");
    state(&root);
    let story = root.path().join("ae-sdd-doc/Story/STORY-FLOW-001.md");
    fs::create_dir_all(story.parent().expect("story parent")).expect("story directory");
    fs::write(&story, "# Story\n").expect("story source");
    let (adapter, _) = adapter(&root);
    let request = params(
        json!({
            "operation":"document.resolve",
            "payload":{"intent":"STORY"}
        }),
        None,
    );

    let mut trusted = workspace(&root, Some(AgentRole::Task));
    trusted.agent_grant = Some(ScopedGrant::new(
        [OperationId::new("workitem.get").expect("operation")],
        [],
        [ProjectPathScope::ProjectRoot],
    ));
    let operation_denied = adapter
        .execute(RpcMethod::OperationExecute, &request, Some(&trusted))
        .expect_err("request payload cannot manufacture an operation grant");
    assert_eq!(
        operation_denied.code(),
        ae_sdd_protocol::StableErrorCode::RoleOperationForbidden
    );

    trusted.agent_grant = Some(ScopedGrant::new(
        [OperationId::new("document.resolve").expect("operation")],
        [],
        [ProjectPathScope::Subtree(
            ProjectRelativePath::new("crates").expect("path"),
        )],
    ));
    let path_denied = adapter
        .execute(RpcMethod::OperationExecute, &request, Some(&trusted))
        .expect_err("operation grant cannot widen its path scope");
    assert_eq!(
        path_denied.code(),
        ae_sdd_protocol::StableErrorCode::RoleOperationForbidden
    );

    trusted.agent_grant = Some(ScopedGrant::new(
        [OperationId::new("document.resolve").expect("operation")],
        [],
        [ProjectPathScope::Subtree(
            ProjectRelativePath::new("ae-sdd-doc/Story").expect("path"),
        )],
    ));
    let resolved = adapter
        .execute(RpcMethod::OperationExecute, &request, Some(&trusted))
        .expect("narrow authorized document resolves");
    assert_eq!(resolved["changed"], false);
    assert_eq!(
        resolved["data"]["path"],
        "ae-sdd-doc/Story/STORY-FLOW-001.md"
    );
}

#[test]
fn native_gate_pass_advances_the_pending_flow_without_stored_gate_results() {
    let root = TempDir::new().expect("tempdir");
    state(&root);
    complete_project_assets(&root);
    let (adapter, _) = adapter(&root);
    let trusted = workspace(&root, Some(AgentRole::Root));
    adapter
        .execute(
            RpcMethod::FlowNext,
            &params(
                json!({"targetPhase":"test-running"}),
                Some("flow-transition-native-gate"),
            ),
            Some(&trusted),
        )
        .expect("transition intent");

    let evaluated = adapter
        .execute(
            RpcMethod::GateEvaluate,
            &params(json!({"gateId":"G-00"}), Some("native-gate-g00")),
            Some(&trusted),
        )
        .expect("native Gate evaluation");

    assert_eq!(evaluated["outcome"]["kind"], "PASS");
    assert_eq!(evaluated["flow"]["nextAction"]["kind"], "apply-transition");
}

#[test]
fn stored_pass_cannot_bypass_the_durable_transition_intent() {
    let root = TempDir::new().expect("tempdir");
    let runtime_dir = TempDir::new().expect("runtime tempdir");
    state(&root);
    complete_project_assets(&root);
    let database = runtime_dir.path().join("runtime.sqlite3");
    let persistence =
        Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence"));
    let event_store_id = persistence.event_store_id().expect("event store id");
    let port: Arc<dyn PersistencePort> = persistence;
    let adapter = NativeBusinessAdapter::new(
        database.clone(),
        event_store_id,
        BootId::from_uuid(Uuid::from_u128(14)),
        ae_sdd_policy::policy_digest().to_string(),
        port,
    );
    let paths = ProjectStorePaths::new(
        root.path(),
        ProjectRelativePath::new(".auto-engineering/flow-test/state.json".to_owned())
            .expect("state path"),
    )
    .expect("store paths");
    let repository = SqliteRuntimeRepository::open(
        &database,
        event_store_id,
        &UtcTimestamp::from_str("2026-07-23T03:00:00Z").expect("timestamp"),
    )
    .expect("project repository");
    let store =
        ProjectMutationStore::new(paths, StdDurableFileSystem, StdCrossProcessLock, repository);
    let lease_id = LeaseId::from_uuid(Uuid::from_u128(15));
    let lease = store
        .acquire_lease(
            lease_id,
            LeaseOwner::new(Uuid::from_u128(12).to_string()).expect("lease owner"),
            UtcTimestamp::from_str("2026-07-23T03:00:00Z").expect("timestamp"),
            UtcTimestamp::from_str("2030-07-23T04:00:00Z").expect("expiry"),
        )
        .expect("lease acquire");
    let trusted = workspace(&root, Some(AgentRole::Root));
    let mut request = params(
        json!({
            "operation":"state.transition",
            "payload":{"targetPhase":"test-running"}
        }),
        Some("state-transition-with-forged-pass"),
    );
    request.expected_revision = Some(7);
    request.fencing_token = Some(lease.fencing_token().get());
    request.lease_id = Some(lease_id.to_string());
    request.confirmation = Some(ConfirmationRef {
        confirmation_id: "confirmation-1".to_owned(),
        approved_by: "user:test".to_owned(),
        approved_at: "2026-07-23T03:00:00Z".to_owned(),
    });

    let error = adapter
        .execute(RpcMethod::OperationExecute, &request, Some(&trusted))
        .expect_err("stored PASS without a durable root intent must fail closed");
    assert_eq!(
        error.code(),
        ae_sdd_protocol::StableErrorCode::GateBlocked,
        "{error:?}"
    );
}

#[test]
fn fresh_native_gate_and_lease_commit_only_the_nested_story_phase() {
    let root = TempDir::new().expect("workspace tempdir");
    let runtime_dir = TempDir::new().expect("runtime tempdir");
    state(&root);
    complete_project_assets(&root);
    let database = runtime_dir.path().join("runtime.sqlite3");
    let persistence =
        Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence"));
    let event_store_id = persistence.event_store_id().expect("event store id");
    let port: Arc<dyn PersistencePort> = persistence;
    let adapter = NativeBusinessAdapter::new(
        database.clone(),
        event_store_id,
        BootId::from_uuid(Uuid::from_u128(31)),
        ae_sdd_policy::policy_digest().to_string(),
        port,
    );
    let trusted = workspace(&root, Some(AgentRole::Root));
    let paths = ProjectStorePaths::new(
        root.path(),
        ProjectRelativePath::new(".auto-engineering/flow-test/state.json".to_owned())
            .expect("state path"),
    )
    .expect("store paths");
    let repository = SqliteRuntimeRepository::open(
        &database,
        event_store_id,
        &UtcTimestamp::from_str("2026-07-23T03:00:00Z").expect("timestamp"),
    )
    .expect("project repository");
    let store =
        ProjectMutationStore::new(paths, StdDurableFileSystem, StdCrossProcessLock, repository);
    let lease_id = LeaseId::from_uuid(Uuid::from_u128(32));
    let session_id = Uuid::from_u128(12).to_string();
    let lease = store
        .acquire_lease(
            lease_id,
            LeaseOwner::new(session_id.clone()).expect("lease owner"),
            UtcTimestamp::from_str("2026-07-23T03:00:00Z").expect("timestamp"),
            UtcTimestamp::from_str("2030-07-23T04:00:00Z").expect("expiry"),
        )
        .expect("lease acquire");

    adapter
        .execute(
            RpcMethod::FlowNext,
            &params(
                json!({"targetPhase":"test-running"}),
                Some("positive-transition-intent"),
            ),
            Some(&trusted),
        )
        .expect("transition intent");
    let mut gate_request = params(json!({"gateId":"G-00"}), Some("positive-gate-g00"));
    gate_request.fencing_token = Some(lease.fencing_token().get());
    let evaluated = adapter
        .execute(RpcMethod::GateEvaluate, &gate_request, Some(&trusted))
        .expect("fresh Gate");
    assert_eq!(evaluated["outcome"]["kind"], "PASS");

    let mut transition = params(
        json!({
            "operation":"state.transition",
            "payload":{"targetPhase":"test-running"}
        }),
        Some("positive-state-transition"),
    );
    transition.expected_revision = Some(7);
    transition.fencing_token = Some(lease.fencing_token().get());
    transition.lease_id = Some(lease_id.to_string());
    transition.confirmation = Some(ConfirmationRef {
        confirmation_id: "confirmation-positive".to_owned(),
        approved_by: "user:test".to_owned(),
        approved_at: "2026-07-23T03:00:00Z".to_owned(),
    });
    let committed = adapter
        .execute(RpcMethod::OperationExecute, &transition, Some(&trusted))
        .expect("transition commit");
    let replayed = adapter
        .execute(RpcMethod::OperationExecute, &transition, Some(&trusted))
        .expect("same transition replay");
    let authority: Value = serde_json::from_slice(
        &fs::read(root.path().join(".auto-engineering/flow-test/state.json")).expect("state bytes"),
    )
    .expect("state JSON");

    assert_eq!(committed["revisionAfter"], 8);
    assert_eq!(replayed["changed"], false);
    assert_eq!(replayed["revisionAfter"], 8);
    assert_eq!(authority["revision"], 8);
    assert_eq!(
        authority["storyStates"]["STORY-FLOW-001"]["phase"],
        "test-running"
    );
    assert_eq!(authority["phase"], "completed");
}
