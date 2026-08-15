use std::{fs, path::Path, sync::Arc};

use ae_sdd_domain::{AgentRole, BootId, CapabilityId, OperationId, ProjectPathScope, ScopedGrant};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_operations::OperationName;
use ae_sdd_protocol::{
    ClientKind, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, GrantPathWire, PersistencePort,
    RuntimeDelegationAttestationRecord, RuntimeDelegationHostActionRecord, RuntimeDelegationRecord,
    RuntimeIdentityKind, RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeSessionRecord,
    RuntimeWorkspaceRecord, ScopedGrantWire, WireAgentRole,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

// The authoritative Review input fingerprint spans locked state plus workspace
// source inventory. The test reuses the daemon implementation instead of
// duplicating it so depth fixtures cannot drift from production hashing.
#[allow(dead_code)]
#[path = "../src/gate_source/mod.rs"]
mod gate_source;
#[allow(dead_code)]
#[path = "../src/persistence.rs"]
mod persistence;
#[allow(dead_code)]
#[path = "../src/review_authority.rs"]
mod review_authority;

use review_authority::authoritative_review_workspace_input_fingerprint;

const WORK_ITEM: &str = "STORY-REVIEW-CONTROL-001";
const PROJECT_KEY: &str = "review-control";
const INPUT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const RULESET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const POLICY: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const GATES: [&str; 3] = ["G-09B", "G-REVIEW-LOOP", "G-REVIEW-DEPTH"];

const WORKSPACE_ID: &str = "00000000-0000-0000-0000-000000000001";
const ROOT_SESSION: &str = "00000000-0000-0000-0000-000000000010";
const SERIES_SESSION: &str = "00000000-0000-0000-0000-000000000020";
const AUTHOR_SESSION: &str = "00000000-0000-0000-0000-000000000030";
const REVIEWER_SESSION: &str = "00000000-0000-0000-0000-000000000040";
const SERIES_DELEGATION: &str = "10000000-0000-0000-0000-000000000020";
const AUTHOR_DELEGATION: &str = "10000000-0000-0000-0000-000000000030";
const REVIEWER_DELEGATION: &str = "10000000-0000-0000-0000-000000000040";

struct Fixture {
    workspace_root: TempDir,
    runtime_root: TempDir,
    state_path: std::path::PathBuf,
    database: std::path::PathBuf,
    boot_id: BootId,
    event_store_id: ae_sdd_domain::EventStoreId,
    persistence: Arc<SqliteRuntimePersistence>,
}

impl Fixture {
    fn new() -> Self {
        Self::with_scale("small")
    }

    fn with_scale(scale: &str) -> Self {
        let workspace_root = TempDir::new().expect("workspace tempdir");
        let state_dir = workspace_root
            .path()
            .join(".auto-engineering/review-control");
        fs::create_dir_all(&state_dir).expect("state directory");
        let state_path = state_dir.join("state.json");
        write_state(
            &state_path,
            &json!({
                "stateMachineName":"PRD-REVIEW-CONTROL-001",
                "activeStory":WORK_ITEM,
                "revision":7,
                "lastFencingToken":0,
                "scale":scale,
                "selectedDesign":"DR",
                "phase":"test-running",
                "currentPhase":"test-running",
                "inputFingerprint":INPUT,
                "rulesetFingerprint":RULESET,
                "policyDigest":POLICY,
                "inventoryGeneration":3,
                "executionPlan":{"changedPaths":["src/lib.rs"]},
                "storyStates":{
                    WORK_ITEM:{"phase":"test-running","currentPhase":"test-running"}
                }
            }),
        );
        write_source(
            workspace_root.path(),
            "src/lib.rs",
            "pub fn v() -> u8 { 1 }\n",
        );

        let runtime_root = TempDir::new().expect("runtime tempdir");
        let database = runtime_root.path().join("runtime.sqlite3");
        let persistence =
            Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence opens"));
        let event_store_id = persistence.event_store_id().expect("event store identity");
        let boot_id = BootId::from_uuid(Uuid::from_u128(500));
        seed_identity_authority(persistence.as_ref(), workspace_root.path(), boot_id);
        Self {
            workspace_root,
            runtime_root,
            state_path,
            database,
            boot_id,
            event_store_id,
            persistence,
        }
    }

    /// Seals a finalized evidence manifest against the authoritative Review
    /// input fingerprint computed from current state plus workspace inventory.
    fn seal_evidence(&self, evidence_id: &str) {
        let state = read_state(&self.state_path);
        let input =
            authoritative_review_workspace_input_fingerprint(&self.reviewer_workspace(), &state)
                .expect("authoritative review input fingerprint");
        write_finalized_manifest(self.workspace_root.path(), &input.to_string(), evidence_id);
    }

    fn adapter(&self) -> NativeBusinessAdapter {
        let persistence: Arc<dyn PersistencePort> = self.persistence.clone();
        NativeBusinessAdapter::new(
            self.database.clone(),
            self.event_store_id,
            self.boot_id,
            POLICY.to_owned(),
            persistence,
        )
    }

    fn reviewer_workspace(&self) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Reviewer,
            reviewer_domain_grant(),
        )
    }

    fn root_workspace(&self) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Root,
            ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot]),
        )
    }
}

#[test]
fn localized_and_english_scales_derive_the_same_review_tiers() {
    for (index, (scale, expected_tier, expected_specialties)) in [
        ("large", "tier3", json!(["be", "ar", "qa"])),
        ("\u{5927}", "tier3", json!(["be", "ar", "qa"])),
        ("medium", "tier2", json!(["be", "ar"])),
        ("\u{4e2d}", "tier2", json!(["be", "ar"])),
        ("small", "tier1", json!(["general"])),
        ("\u{5c0f}", "tier1", json!(["general"])),
        ("micro", "tier1", json!(["general"])),
        ("\u{5fae}", "tier1", json!(["general"])),
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = Fixture::with_scale(scale);
        let reviewer = fixture.reviewer_workspace();
        let adapter = fixture.adapter();
        let lease = adapter
            .execute(
                RpcMethod::OperationExecute,
                &operation_params(
                    "lease.acquire",
                    json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
                    &format!("scale-{index}-lease"),
                ),
                Some(&reviewer),
            )
            .unwrap_or_else(|error| panic!("scale {scale} lease failed: {error:?}"));
        fixture.seal_evidence("ev-review-scale");
        let mut request = operation_params(
            "review.record",
            json!({
                "status":"passed",
                "findings":[],
                "reviewedPaths":["src/lib.rs"],
                "evidenceIds":["ev-review-scale"]
            }),
            &format!("scale-{index}-review"),
        );
        bind_write(
            &mut request,
            lease["data"]["leaseId"].as_str().expect("lease id"),
            lease["data"]["fencingToken"]
                .as_u64()
                .expect("fencing token"),
            7,
        );

        adapter
            .execute(RpcMethod::OperationExecute, &request, Some(&reviewer))
            .unwrap_or_else(|error| panic!("scale {scale} review failed: {error:?}"));
        let state = read_state(&fixture.state_path);

        assert_eq!(state["reviewSession"]["tier"], expected_tier, "{scale}");
        assert_eq!(
            state["reviewSession"]["requiredSpecialties"], expected_specialties,
            "{scale}"
        );
    }
}

fn business_workspace(root: &Path, role: AgentRole, grant: ScopedGrant) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: WORKSPACE_ID.to_owned(),
        canonical_root: fs::canonicalize(root)
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned(),
        project_key: PROJECT_KEY.to_owned(),
        mode: WorkspaceMode::RustCanary,
        agent_role: Some(role),
        agent_grant: Some(grant),
        caller_kind: Some(ClientKind::Cli),
        inventory_generation: 3,
    }
}

fn reviewer_domain_grant() -> ScopedGrant {
    ScopedGrant::new(
        ["lease.acquire", "review.record"]
            .into_iter()
            .map(|value| OperationId::new(value).expect("operation id")),
        [CapabilityId::new("review.specialty.general").expect("specialty capability")],
        [ProjectPathScope::ProjectRoot],
    )
}

fn reviewer_wire_grant() -> ScopedGrantWire {
    ScopedGrantWire::from_domain(&reviewer_domain_grant())
}

fn root_wire_grant() -> ScopedGrantWire {
    ScopedGrantWire::from_domain(&ScopedGrant::new(
        OperationName::ALL
            .into_iter()
            .filter(|operation| *operation != OperationName::LeaseBreak)
            .map(|operation| OperationId::new(operation.as_str()).expect("operation id")),
        ["general", "be", "ar", "qa"].into_iter().map(|specialty| {
            CapabilityId::new(format!("review.specialty.{specialty}"))
                .expect("specialty capability")
        }),
        [ProjectPathScope::ProjectRoot],
    ))
}

fn seed_identity_authority(persistence: &SqliteRuntimePersistence, root: &Path, boot_id: BootId) {
    let workspace = RuntimeWorkspaceRecord {
        workspace_id: WORKSPACE_ID.to_owned(),
        canonical_root: fs::canonicalize(root)
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned(),
        project_key: PROJECT_KEY.to_owned(),
        mode: WorkspaceMode::RustCanary,
        inventory_generation: 3,
        dirty: false,
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    };
    commit_identity(
        persistence,
        "workspace.register",
        "workspace-register",
        RuntimeIdentitySnapshot {
            identity_kind: RuntimeIdentityKind::Workspace,
            workspace: workspace.clone(),
            session: None,
            delegation: None,
            host_action: None,
            attestation: None,
            response: json!({"workspaceId":WORKSPACE_ID}),
            replayed: false,
        },
    );

    let root_grant = root_wire_grant();
    commit_identity(
        persistence,
        "session.open",
        "root-session-open",
        RuntimeIdentitySnapshot {
            identity_kind: RuntimeIdentityKind::Session,
            workspace: workspace.clone(),
            session: Some(session_record(
                ROOT_SESSION,
                "root-agent",
                WireAgentRole::Root,
                ROOT_SESSION,
                None,
                None,
                root_grant,
            )),
            delegation: None,
            host_action: None,
            attestation: None,
            response: json!({"sessionId":ROOT_SESSION}),
            replayed: false,
        },
    );

    let series_grant = ScopedGrantWire {
        operations: vec![
            "document.save".to_owned(),
            "lease.acquire".to_owned(),
            "review.record".to_owned(),
        ],
        capabilities: vec!["review.specialty.general".to_owned()],
        paths: vec![GrantPathWire::ProjectRoot],
    };
    commit_child_identity(
        persistence,
        &workspace,
        boot_id,
        ChildIdentity {
            session_id: SERIES_SESSION,
            agent_id: "series-agent",
            delegation_id: SERIES_DELEGATION,
            parent_session_id: ROOT_SESSION,
            parent_delegation_id: None,
            role: WireAgentRole::Series,
            grant: series_grant,
            sequence: 1,
        },
    );
    commit_child_identity(
        persistence,
        &workspace,
        boot_id,
        ChildIdentity {
            session_id: AUTHOR_SESSION,
            agent_id: "author-agent",
            delegation_id: AUTHOR_DELEGATION,
            parent_session_id: SERIES_SESSION,
            parent_delegation_id: Some(SERIES_DELEGATION),
            role: WireAgentRole::Task,
            grant: ScopedGrantWire {
                operations: vec!["document.save".to_owned()],
                capabilities: Vec::new(),
                paths: vec![GrantPathWire::ProjectRoot],
            },
            sequence: 2,
        },
    );
    commit_child_identity(
        persistence,
        &workspace,
        boot_id,
        ChildIdentity {
            session_id: REVIEWER_SESSION,
            agent_id: "reviewer-agent",
            delegation_id: REVIEWER_DELEGATION,
            parent_session_id: SERIES_SESSION,
            parent_delegation_id: Some(SERIES_DELEGATION),
            role: WireAgentRole::Reviewer,
            grant: reviewer_wire_grant(),
            sequence: 3,
        },
    );
}

struct ChildIdentity {
    session_id: &'static str,
    agent_id: &'static str,
    delegation_id: &'static str,
    parent_session_id: &'static str,
    parent_delegation_id: Option<&'static str>,
    role: WireAgentRole,
    grant: ScopedGrantWire,
    sequence: u64,
}

fn commit_child_identity(
    persistence: &SqliteRuntimePersistence,
    workspace: &RuntimeWorkspaceRecord,
    boot_id: BootId,
    child: ChildIdentity,
) {
    let adapter_id = "review-control-adapter";
    persistence
        .store_record(
            "host-adapter/v1",
            adapter_id,
            &json!({"schemaVersion":"host-adapter/v1","capabilities":["create","attest"]}),
        )
        .expect("host adapter mirror");
    let action_id = format!("20000000-0000-0000-0000-{:012x}", child.sequence);
    let ack_id = format!("30000000-0000-0000-0000-{:012x}", child.sequence);
    let action = json!({
        "actionId":action_id,
        "adapterId":adapter_id,
        "commandSeq":child.sequence,
        "kind":"create",
        "delegationId":child.delegation_id,
        "compactId":null,
        "sessionId":null,
        "contextGeneration":null,
        "deadlineUnixMs":4_102_444_800_000_u64
    });
    persistence
        .store_record("host-action/v1", &action_id, &action)
        .expect("host action mirror");
    let ack = json!({
        "ackId":ack_id,
        "actionId":action_id,
        "commandSeq":child.sequence,
        "outcome":"accepted",
        "hostTaskId":format!("host-task-{}", child.sequence),
        "sessionId":child.session_id
    });
    persistence
        .store_record("host-ack/v1", &ack_id, &ack)
        .expect("host ACK mirror");
    let action_digest = digest(serde_json::to_vec(&action).expect("action serializes"));
    let ack_digest = digest(serde_json::to_vec(&ack).expect("ACK serializes"));
    let ChildIdentity {
        session_id,
        agent_id,
        delegation_id,
        parent_session_id,
        parent_delegation_id,
        role,
        grant,
        sequence,
    } = child;
    let session = session_record(
        session_id,
        agent_id,
        role,
        ROOT_SESSION,
        Some(parent_session_id),
        Some(delegation_id),
        grant.clone(),
    );
    let snapshot = RuntimeIdentitySnapshot {
        identity_kind: RuntimeIdentityKind::Delegation,
        workspace: workspace.clone(),
        session: Some(session.clone()),
        delegation: Some(RuntimeDelegationRecord {
            delegation_id: delegation_id.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            work_item_id: Some(WORK_ITEM.to_owned()),
            root_session_id: ROOT_SESSION.to_owned(),
            parent_session_id: parent_session_id.to_owned(),
            child_session_id: Some(session_id.to_owned()),
            parent_delegation_id: parent_delegation_id.map(str::to_owned),
            role,
            input_revision: 7,
            input_fingerprint: INPUT.to_owned(),
            status: "running".to_owned(),
            deadline_unix_ms: 4_102_444_800_000,
            receipt_digest: digest(format!("delegation-receipt-{sequence}")),
            created_at_unix_ms: 1_000,
            updated_at_unix_ms: 1_000,
        }),
        host_action: Some(RuntimeDelegationHostActionRecord {
            workspace_id: WORKSPACE_ID.to_owned(),
            delegation_id: delegation_id.to_owned(),
            host_action_id: action_id.clone(),
            parent_session_id: parent_session_id.to_owned(),
            action_digest: action_digest.clone(),
            created_at_unix_ms: 1_000,
        }),
        attestation: Some(RuntimeDelegationAttestationRecord {
            workspace_id: WORKSPACE_ID.to_owned(),
            delegation_id: delegation_id.to_owned(),
            physical_session_id: session_id.to_owned(),
            host_action_id: action_id,
            host_ack_id: ack_id,
            action_digest,
            ack_digest,
            claim_digest: digest(format!("claim-{sequence}")),
            grant,
            attestation_ref: format!("delegation:{delegation_id}"),
            attestation_digest: digest(format!("attestation-{sequence}")),
            accepted_boot_id: boot_id.to_string(),
            accepted_at_unix_ms: 1_000,
            expires_at_unix_ms: 4_102_444_800_000,
        }),
        response: json!({"delegationId":delegation_id,"status":"running"}),
        replayed: false,
    };
    commit_identity(
        persistence,
        "delegation.accept",
        &format!("delegation-{sequence}"),
        snapshot,
    );
    commit_identity(
        persistence,
        "session.open",
        &format!("child-session-{sequence}"),
        RuntimeIdentitySnapshot {
            identity_kind: RuntimeIdentityKind::Session,
            workspace: workspace.clone(),
            session: Some(session),
            delegation: None,
            host_action: None,
            attestation: None,
            response: json!({"sessionId":session_id}),
            replayed: false,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn session_record(
    session_id: &str,
    agent_id: &str,
    role: WireAgentRole,
    root_session_id: &str,
    parent_session_id: Option<&str>,
    delegation_id: Option<&str>,
    grant: ScopedGrantWire,
) -> RuntimeSessionRecord {
    RuntimeSessionRecord {
        session_id: session_id.to_owned(),
        agent_id: agent_id.to_owned(),
        workspace_id: WORKSPACE_ID.to_owned(),
        external_key_hash: digest(format!("external-{session_id}")),
        role,
        root_session_id: root_session_id.to_owned(),
        parent_session_id: parent_session_id.map(str::to_owned),
        delegation_id: delegation_id.map(str::to_owned),
        engaged: true,
        current_work_item: Some(WORK_ITEM.to_owned()),
        grant,
        context_generation: 0,
        expires_at_unix_ms: 4_102_444_800_000,
        status: "active".to_owned(),
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    }
}

fn commit_identity(
    persistence: &SqliteRuntimePersistence,
    operation: &str,
    key: &str,
    snapshot: RuntimeIdentitySnapshot,
) {
    persistence
        .commit_identity_bundle(RuntimeIdentityTransition {
            operation: operation.to_owned(),
            scope_digest: digest(format!("scope-{key}")),
            idempotency_key: key.to_owned(),
            request_digest: digest(format!("request-{key}")),
            expected_workspace_mode: None,
            expected_inventory_generation: None,
            expected_session_status: None,
            expected_delegation_status: None,
            expected_context_generation: None,
            snapshot,
            committed_at_unix_ms: 1_000,
        })
        .unwrap_or_else(|error| panic!("{operation}/{key}: {error:?}"));
}

fn operation_params(operation: &str, payload: Value, key: &str) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(WORKSPACE_ID.to_owned()),
        agent_id: Some("reviewer-agent".to_owned()),
        session_id: Some(REVIEWER_SESSION.to_owned()),
        capability_token: None,
        turn_id: None,
        work_item_id: Some(WORK_ITEM.to_owned()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: Some(key.to_owned()),
        confirmation: None,
        deadline_ms: 10_000,
        payload: json!({"operation":operation,"payload":payload}),
    }
}

fn gate_params(key: &str) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(WORKSPACE_ID.to_owned()),
        agent_id: Some("root-agent".to_owned()),
        session_id: Some(ROOT_SESSION.to_owned()),
        capability_token: None,
        turn_id: None,
        work_item_id: Some(WORK_ITEM.to_owned()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: Some(key.to_owned()),
        confirmation: None,
        deadline_ms: 10_000,
        payload: json!({"gateIds":GATES}),
    }
}

fn bind_write(
    request: &mut RequestParams<Value>,
    lease_id: &str,
    fencing_token: u64,
    revision: u64,
) {
    request.lease_id = Some(lease_id.to_owned());
    request.fencing_token = Some(fencing_token);
    request.expected_revision = Some(revision);
}

fn write_source(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
    fs::write(path, content).expect("source file");
}

/// Writes a finalized evidence manifest whose entry is fresh for `input`.
fn write_finalized_manifest(root: &Path, input: &str, evidence_id: &str) {
    let mut manifest = json!({
        "schemaVersion":1,
        "storyId":WORK_ITEM,
        "entries":[{
            "evidenceId":evidence_id,
            "kind":"focused-test",
            "inputFingerprint":input,
            "status":"active",
            "exitCode":0,
            "reusable":true,
            "artifacts":[]
        }]
    });
    let mut payload = manifest.clone();
    payload
        .as_object_mut()
        .expect("manifest object")
        .retain(|key, _| key != "contentHash" && !key.starts_with('_'));
    manifest["contentHash"] = json!(format!(
        "sha256:{}",
        digest(serde_json::to_vec(&payload).expect("manifest canonical JSON"))
    ));
    let path = root.join(format!(
        ".auto-engineering/{WORK_ITEM}/evidence/manifest.json"
    ));
    fs::create_dir_all(path.parent().expect("manifest parent")).expect("manifest directory");
    fs::write(path, serde_json::to_vec(&manifest).expect("manifest JSON")).expect("manifest file");
}

fn read_state(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("state reads")).expect("state JSON")
}

fn write_state(path: &Path, state: &Value) {
    fs::write(
        path,
        serde_json::to_vec_pretty(state).expect("state serializes"),
    )
    .expect("state writes");
}

fn digest(value: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(value.as_ref()))
}

/// A closed findings batch followed by a committed input-changing remediation
/// must project the remediation against the PARENT review and its findings
/// batch while the attempt/session belong to the CHILD review.
#[test]
fn committed_remediation_projects_against_the_parent_review_and_findings_batch() {
    let fixture = Fixture::new();
    let reviewer = fixture.reviewer_workspace();
    let adapter = fixture.adapter();
    let (lease_id, fencing_token) = acquire_lease(&adapter, &reviewer, "remediation-lease");

    let mut findings = operation_params(
        "review.record",
        json!({
            "status":"changes_required",
            "findings":[{
                "code":"REVIEW-REMEDIATE-001",
                "severity":"minor",
                "summary":"remediation fixture finding"
            }]
        }),
        "review-findings-1",
    );
    bind_write(&mut findings, &lease_id, fencing_token, 7);
    let recorded = adapter
        .execute(RpcMethod::OperationExecute, &findings, Some(&reviewer))
        .expect("findings review commits");
    assert_eq!(recorded["changed"], true);

    let after_findings = read_state(&fixture.state_path);
    let parent_review_id = after_findings["reviewSession"]["reviewId"]
        .as_str()
        .expect("parent reviewId")
        .to_owned();
    let findings_batch_id = after_findings["review"]["batch"]["batchId"]
        .as_str()
        .expect("findings batchId")
        .to_owned();
    assert_eq!(
        after_findings["reviewSession"]["status"],
        "remediation_required"
    );
    assert_eq!(after_findings["review"]["batch"]["closed"], true);
    assert_eq!(
        after_findings["review"]["batch"]["latestStatus"],
        "VALID_FINDINGS"
    );

    // Commit the input-changing remediation: the reviewed source changes, so the
    // next review derives a different authoritative input fingerprint.
    write_source(
        fixture.workspace_root.path(),
        "src/lib.rs",
        "pub fn v() -> u8 { 2 }\n",
    );
    fixture.seal_evidence("ev-review-remediated");
    let mut clean = operation_params(
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["ev-review-remediated"]
        }),
        "review-remediated-1",
    );
    bind_write(&mut clean, &lease_id, fencing_token, 8);
    let remediated = adapter
        .execute(RpcMethod::OperationExecute, &clean, Some(&reviewer))
        .expect("remediated review commits with its durable projection");
    assert_eq!(remediated["changed"], true);

    let after_remediation = read_state(&fixture.state_path);
    let child_review_id = after_remediation["reviewSession"]["reviewId"]
        .as_str()
        .expect("child reviewId")
        .to_owned();
    assert_eq!(
        after_remediation["reviewSession"]["parentReviewId"],
        json!(parent_review_id)
    );
    assert_ne!(child_review_id, parent_review_id);
    assert_eq!(
        after_remediation["review"]["attempt"]["remediation"]["findingBatchId"],
        json!(findings_batch_id)
    );
    assert_eq!(
        after_remediation["review"]["attempt"]["remediation"]["nextReviewId"],
        json!(child_review_id)
    );

    // The projected remediation row must key on the parent review and its
    // findings batch, and must advance from the parent revision to the child.
    let rows = remediation_rows(&fixture.database);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let (review_id, batch_id, next_review_id, source_revision, target_revision) = &rows[0];
    assert_eq!(review_id, &parent_review_id, "{rows:?}");
    assert_eq!(batch_id, &findings_batch_id, "{rows:?}");
    assert_eq!(next_review_id, &child_review_id, "{rows:?}");
    assert_eq!(*source_revision, 7, "{rows:?}");
    assert_eq!(*target_revision, 8, "{rows:?}");

    // Replaying the same idempotency key must repair/keep exactly one row.
    let replay = adapter
        .execute(RpcMethod::OperationExecute, &clean, Some(&reviewer))
        .expect("remediated review replays idempotently");
    assert_eq!(replay["changed"], false);
    assert_eq!(remediation_rows(&fixture.database), rows);
}

/// A remediation whose parent batch is no longer the parent review's closed
/// findings batch must fail closed instead of projecting a forged parent link.
#[test]
fn remediation_projection_fails_closed_when_the_parent_findings_batch_does_not_join() {
    let fixture = Fixture::new();
    let reviewer = fixture.reviewer_workspace();
    let adapter = fixture.adapter();
    let (lease_id, fencing_token) = acquire_lease(&adapter, &reviewer, "mismatch-lease");

    let mut findings = operation_params(
        "review.record",
        json!({
            "status":"changes_required",
            "findings":[{
                "code":"REVIEW-REMEDIATE-002",
                "severity":"minor",
                "summary":"mismatch fixture finding"
            }]
        }),
        "mismatch-findings-1",
    );
    bind_write(&mut findings, &lease_id, fencing_token, 7);
    adapter
        .execute(RpcMethod::OperationExecute, &findings, Some(&reviewer))
        .expect("findings review commits");

    write_source(
        fixture.workspace_root.path(),
        "src/lib.rs",
        "pub fn v() -> u8 { 3 }\n",
    );
    fixture.seal_evidence("ev-review-mismatch");
    let mut clean = operation_params(
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["ev-review-mismatch"]
        }),
        "mismatch-remediated-1",
    );
    bind_write(&mut clean, &lease_id, fencing_token, 8);
    adapter
        .execute(RpcMethod::OperationExecute, &clean, Some(&reviewer))
        .expect("remediated review commits");
    assert_eq!(remediation_rows(&fixture.database).len(), 1);

    // Break the join: the parent batch is no longer a closed findings batch, and
    // the child's projection receipt is removed so replay must reapply the rows.
    let connection = rusqlite::Connection::open(&fixture.database).expect("runtime database opens");
    connection
        .execute(
            "UPDATE review_batch_v2_projection \
             SET latest_status='invalid_infra',closed=0,valid_batch_ordinal=NULL \
             WHERE workspace_id=?1 AND latest_status='valid_findings'",
            rusqlite::params![WORKSPACE_ID],
        )
        .expect("parent batch downgrade applies");
    let removed = connection
        .execute(
            "DELETE FROM runtime_record_v1 WHERE namespace='review-projection-event/v3' \
             AND key=(SELECT ?1||':'||MAX(CAST(substr(key,length(?1)+2) AS INTEGER)) \
                      FROM runtime_record_v1 WHERE namespace='review-projection-event/v3')",
            rusqlite::params![WORKSPACE_ID],
        )
        .expect("child projection receipt removal applies");
    assert_eq!(removed, 1);
    drop(connection);

    let error = adapter
        .execute(RpcMethod::OperationExecute, &clean, Some(&reviewer))
        .expect_err("a mismatched parent findings batch cannot be projected");
    assert_eq!(
        error.code(),
        StableErrorCode::ExternalStateConflict,
        "{error:?}"
    );
    assert!(
        error
            .message()
            .contains("parent batch is not a closed findings batch"),
        "{error:?}"
    );
}

/// Reads `(review_id, finding_batch_id, next_review_id, source_revision,
/// target_revision)` for every projected remediation row in stable order.
fn remediation_rows(database: &Path) -> Vec<(String, String, String, i64, i64)> {
    let connection = rusqlite::Connection::open(database).expect("runtime database opens");
    let mut statement = connection
        .prepare(
            "SELECT review_id,finding_batch_id,next_review_id,source_revision,target_revision \
             FROM review_remediation_v2_projection \
             WHERE workspace_id=?1 ORDER BY review_id,finding_batch_id",
        )
        .expect("remediation query prepares");
    statement
        .query_map(rusqlite::params![WORKSPACE_ID], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .expect("remediation rows read")
        .collect::<Result<Vec<_>, _>>()
        .expect("remediation row values")
}

fn acquire_lease(
    adapter: &NativeBusinessAdapter,
    reviewer: &BusinessWorkspace,
    key: &str,
) -> (String, u64) {
    let lease = adapter
        .execute(
            RpcMethod::OperationExecute,
            &operation_params(
                "lease.acquire",
                json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
                key,
            ),
            Some(reviewer),
        )
        .expect("reviewer lease acquires");
    (
        lease["data"]["leaseId"]
            .as_str()
            .expect("lease id")
            .to_owned(),
        lease["data"]["fencingToken"]
            .as_u64()
            .expect("fencing token"),
    )
}

#[test]
fn review_operation_persists_replays_after_restart_and_releases_gates() {
    let fixture = Fixture::new();
    assert!(fixture.runtime_root.path().is_dir());
    let reviewer = fixture.reviewer_workspace();
    let adapter = fixture.adapter();
    let lease = adapter
        .execute(
            RpcMethod::OperationExecute,
            &operation_params(
                "lease.acquire",
                json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
                "reviewer-lease",
            ),
            Some(&reviewer),
        )
        .expect("reviewer lease acquires");
    let lease_id = lease["data"]["leaseId"].as_str().expect("lease id");
    let fencing_token = lease["data"]["fencingToken"]
        .as_u64()
        .expect("fencing token");

    let before_tamper = fs::read(&fixture.state_path).expect("state before tamper");
    let mut tampered = operation_params(
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "physicalSessionId":"payload-forgery",
            "specialty":"qa"
        }),
        "tampered-review-identity",
    );
    bind_write(&mut tampered, lease_id, fencing_token, 7);
    let error = adapter
        .execute(RpcMethod::OperationExecute, &tampered, Some(&reviewer))
        .expect_err("payload identity cannot manufacture review authority");
    assert!(
        matches!(
            error.code(),
            StableErrorCode::OperationSchemaInvalid
                | StableErrorCode::RoleOperationForbidden
                | StableErrorCode::DelegationAttestationFailed
        ),
        "{error:?}"
    );
    assert_eq!(
        fs::read(&fixture.state_path).expect("state after tamper"),
        before_tamper
    );

    fixture.seal_evidence("ev-review-control");
    let mut request = operation_params(
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["ev-review-control"]
        }),
        "review-clean-1",
    );
    bind_write(&mut request, lease_id, fencing_token, 7);
    let committed = adapter
        .execute(RpcMethod::OperationExecute, &request, Some(&reviewer))
        .expect("review commits through public operation authority");
    assert_eq!(committed["changed"], true);
    assert_eq!(committed["revisionAfter"], 8);
    drop(adapter);

    let reopened =
        Arc::new(SqliteRuntimePersistence::open(&fixture.database).expect("persistence reopens"));
    let reopened_port: Arc<dyn PersistencePort> = reopened;
    let restarted = NativeBusinessAdapter::new(
        fixture.database.clone(),
        fixture.event_store_id,
        BootId::from_uuid(Uuid::from_u128(501)),
        POLICY.to_owned(),
        reopened_port,
    );
    let replay = restarted
        .execute(RpcMethod::OperationExecute, &request, Some(&reviewer))
        .expect("same review request replays after restart");
    assert_eq!(replay["changed"], false);
    assert_eq!(replay["revisionAfter"], 8);

    let state = read_state(&fixture.state_path);
    assert_eq!(state["revision"], 8);
    assert_eq!(state["reviewSession"]["schemaVersion"], "v2");
    assert_eq!(state["reviewSession"]["authorSessionId"], AUTHOR_SESSION);
    assert_eq!(state["reviewSession"]["status"], "completed");
    assert_eq!(state["review"]["status"], "passed");
    assert_eq!(state["review"]["findings"], json!([]));
    assert_eq!(state["review"]["batch"]["schemaVersion"], "v2");
    assert_eq!(state["review"]["receipt"]["schemaVersion"], "v2");
    assert_eq!(
        state["review"]["batch"]["retainedContributions"][0]["reviewer"]["physicalSessionId"],
        REVIEWER_SESSION
    );
    assert_eq!(
        state["review"]["batch"]["retainedContributions"][0]["reviewer"]["specialty"],
        "general"
    );

    let gates = restarted
        .execute(
            RpcMethod::GateEvaluate,
            &gate_params("review-gates-after-restart"),
            Some(&fixture.root_workspace()),
        )
        .expect("restarted Gate authority evaluates");
    assert_eq!(gates["allPass"], true, "{gates}");
    for (result, gate_id) in gates["results"]
        .as_array()
        .expect("Gate results")
        .iter()
        .zip(GATES)
    {
        assert_eq!(result["gateId"], gate_id);
        assert_eq!(result["outcome"]["kind"], "PASS", "{gate_id}: {result}");
    }
}
