//! Authoritative Review Gate end-to-end coverage.
//!
//! Every case drives the real `review.record` operation so project state, the
//! durable runtime event, the SQLite Review projection, and the daemon-verified
//! reviewer lineage are produced by production code. Hand-built state can never
//! release a Review Gate, so each FAIL case tampers with exactly one authority
//! dependency and asserts the Gate refuses.

use std::{fs, path::Path, str::FromStr, sync::Arc};

use ae_sdd_domain::{AgentRole, BootId, CapabilityId, OperationId, ProjectPathScope, ScopedGrant};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_protocol::{
    ClientKind, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, GrantPathWire, PersistencePort,
    RuntimeDelegationAttestationRecord, RuntimeDelegationHostActionRecord, RuntimeDelegationRecord,
    RuntimeIdentityKind, RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeSessionRecord,
    RuntimeWorkspaceRecord, ScopedGrantWire, WireAgentRole,
};
use ae_sdd_store::UtcTimestamp;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

// The authoritative Review input fingerprint spans locked state plus workspace
// source inventory. Reuse the daemon implementation so fixtures cannot drift
// from production hashing.
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

const WORK_ITEM: &str = "STORY-REVIEW-GATE-001";
const ROUTE_WORK_ITEM: &str = "ROUTE-REVIEW-GATE-001";
const PROJECT_KEY: &str = "review-gate";
const INPUT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const RULESET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const POLICY: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const GATES: [&str; 3] = ["G-09B", "G-REVIEW-LOOP", "G-REVIEW-DEPTH"];

const WORKSPACE_ID: &str = "00000000-0000-0000-0000-00000000000a";
const ROOT_SESSION: &str = "00000000-0000-0000-0000-000000000010";
const SERIES_SESSION: &str = "00000000-0000-0000-0000-000000000020";
const AUTHOR_SESSION: &str = "00000000-0000-0000-0000-000000000030";
const SERIES_DELEGATION: &str = "10000000-0000-0000-0000-000000000020";
const AUTHOR_DELEGATION: &str = "10000000-0000-0000-0000-000000000030";

/// One seeded reviewer: physical session, delegation, and exact specialty grant.
struct Reviewer {
    session_id: &'static str,
    delegation_id: &'static str,
    agent_id: &'static str,
    specialty: &'static str,
    sequence: u64,
}

const GENERAL_REVIEWER: Reviewer = Reviewer {
    session_id: "00000000-0000-0000-0000-000000000040",
    delegation_id: "10000000-0000-0000-0000-000000000040",
    agent_id: "reviewer-general",
    specialty: "general",
    sequence: 3,
};
const BE_REVIEWER: Reviewer = Reviewer {
    session_id: "00000000-0000-0000-0000-000000000050",
    delegation_id: "10000000-0000-0000-0000-000000000050",
    agent_id: "reviewer-be",
    specialty: "be",
    sequence: 4,
};
const AR_REVIEWER: Reviewer = Reviewer {
    session_id: "00000000-0000-0000-0000-000000000060",
    delegation_id: "10000000-0000-0000-0000-000000000060",
    agent_id: "reviewer-ar",
    specialty: "ar",
    sequence: 5,
};

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
    /// `scale` selects the Review tier: `small` -> tier1 (`general`),
    /// `medium` -> tier2 (`be` + `ar`), `large` -> tier3.
    fn new(scale: &str) -> Self {
        let workspace_root = TempDir::new().expect("workspace tempdir");
        let state_dir = workspace_root.path().join(".auto-engineering/review-gate");
        fs::create_dir_all(&state_dir).expect("state directory");
        let state_path = state_dir.join("state.json");
        write_state(&state_path, &initial_state(scale));
        write_source(
            workspace_root.path(),
            "src/lib.rs",
            "pub fn v() -> u8 { 1 }\n",
        );
        write_source(
            workspace_root.path(),
            "ae-sdd-doc/Story/STORY-REVIEW-GATE-001.md",
            STORY_DOCUMENT,
        );

        let runtime_root = TempDir::new().expect("runtime tempdir");
        let database = runtime_root.path().join("runtime.sqlite3");
        let persistence =
            Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence opens"));
        let event_store_id = persistence.event_store_id().expect("event store identity");
        let boot_id = BootId::from_uuid(Uuid::from_u128(700));
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

    fn reviewer_workspace(&self, reviewer: &Reviewer) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Reviewer,
            reviewer_domain_grant(reviewer.specialty),
        )
    }

    fn root_workspace(&self) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Root,
            ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot]),
        )
    }

    /// Seals a finalized evidence manifest bound to the authoritative Review
    /// input fingerprint of current state plus workspace inventory.
    fn seal_evidence(&self, evidence_id: &str) {
        let state = read_state(&self.state_path);
        let workspace = self.reviewer_workspace(&GENERAL_REVIEWER);
        let input = authoritative_review_workspace_input_fingerprint(&workspace, &state)
            .expect("authoritative review input fingerprint");
        write_finalized_manifest(self.workspace_root.path(), &input.to_string(), evidence_id);
    }

    fn evaluate_review_gates(&self, key: &str) -> Value {
        self.evaluate_review_gates_for(WORK_ITEM, key)
    }

    fn evaluate_review_gates_for(&self, work_item_id: &str, key: &str) -> Value {
        self.adapter()
            .execute(
                RpcMethod::GateEvaluate,
                &gate_params(work_item_id, key),
                Some(&self.root_workspace()),
            )
            .expect("review Gates return typed outcomes")
    }

    fn connection(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.database).expect("runtime database opens")
    }

    /// Reads back the durable session record for one seeded reviewer. Identity
    /// snapshots are digest-verified, so tamper cases must supersede them
    /// through the port instead of editing rows.
    fn reviewer_session_record(&self, reviewer: &Reviewer) -> RuntimeSessionRecord {
        self.persistence
            .list_identity_snapshots(RuntimeIdentityKind::Session)
            .expect("session snapshots load")
            .into_iter()
            .filter_map(|snapshot| snapshot.session)
            .find(|session| session.session_id == reviewer.session_id)
            .expect("seeded reviewer session exists")
    }

    fn supersede_session(&self, session: RuntimeSessionRecord, key: &str) {
        let workspace = self.workspace_record();
        commit_identity(
            self.persistence.as_ref(),
            "session.open",
            key,
            RuntimeIdentitySnapshot {
                identity_kind: RuntimeIdentityKind::Session,
                workspace,
                session: Some(session),
                delegation: None,
                host_action: None,
                attestation: None,
                response: json!({"status":"superseded"}),
                replayed: false,
            },
        );
    }

    /// Supersedes the reviewer delegation snapshot as revoked while keeping the
    /// same physical attestation, so only the revocation can explain a FAIL.
    fn revoke_reviewer_delegation(&self, reviewer: &Reviewer, key: &str) {
        let mut snapshot =
            self.persistence
                .list_identity_snapshots(RuntimeIdentityKind::Delegation)
                .expect("delegation snapshots load")
                .into_iter()
                .find(|snapshot| {
                    snapshot.delegation.as_ref().is_some_and(|delegation| {
                        delegation.delegation_id == reviewer.delegation_id
                    })
                })
                .expect("seeded reviewer delegation exists");
        snapshot
            .delegation
            .as_mut()
            .expect("delegation record")
            .status = "revoked".to_owned();
        // Revocation must not re-insert the physical attestation: `write_attestation`
        // uses an unconditional INSERT, so replaying the identical attestation row
        // under a new idempotency key trips the delegation_attestation_v1 UNIQUE
        // constraint. Revoking only the delegation after-image leaves the original
        // attestation intact, which is what the Gate validator must then reject.
        snapshot.attestation = None;
        snapshot.host_action = None;
        snapshot.response = json!({"delegationId":reviewer.delegation_id,"status":"revoked"});
        commit_identity(
            self.persistence.as_ref(),
            "delegation.accept",
            key,
            snapshot,
        );
    }

    fn seed_unrelated_unattested_delegation(&self, key: &str) {
        let mut snapshot = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Delegation)
            .expect("delegation snapshots load")
            .into_iter()
            .find(|snapshot| snapshot.delegation.is_some())
            .expect("seeded delegation exists");
        let delegation = snapshot.delegation.as_mut().expect("delegation record");
        delegation.delegation_id = "10000000-0000-0000-0000-000000000099".to_owned();
        delegation.child_session_id = None;
        delegation.status = "cancelled".to_owned();
        snapshot.session = None;
        snapshot.host_action = None;
        snapshot.attestation = None;
        snapshot.response = json!({
            "delegationId":delegation.delegation_id,
            "status":"cancelled"
        });
        commit_identity(
            self.persistence.as_ref(),
            "delegation.cancel",
            key,
            snapshot,
        );
    }

    fn workspace_record(&self) -> RuntimeWorkspaceRecord {
        self.persistence
            .list_identity_snapshots(RuntimeIdentityKind::Workspace)
            .expect("workspace snapshots load")
            .into_iter()
            .map(|snapshot| snapshot.workspace)
            .find(|workspace| workspace.workspace_id == WORKSPACE_ID)
            .expect("seeded workspace exists")
    }
}

/// Approved plan with the 14 verification rows, aligned AC ids and real source
/// reads so the Tier 2 deterministic final proof (G-CODEPLAN-SRC, G-14, G-08)
/// can reach PASS.
fn execution_plan() -> Value {
    let verification: Vec<Value> = (1..=14)
        .map(|index| {
            json!({
                "id":format!("V-{index:03}"),
                "acId":format!("AC-{index}"),
                "boundary":"unit",
                "command":"cargo test",
                "expected":"pass"
            })
        })
        .collect();
    json!({
        "goal":"authoritative review gate fixture",
        "changedPaths":["src/lib.rs"],
        "verification":verification,
        "risks":["fixture risk"],
        "approved":true,
        "sourceReads":[
            "src/lib.rs",
            "ae-sdd-doc/RA/review-gate.md",
            "ae-sdd-doc/DR/review-gate.md",
            "ae-sdd-doc/Story/STORY-REVIEW-GATE-001.md"
        ]
    })
}

/// Story document whose AC ids are a subset of the plan verification matrix.
const STORY_DOCUMENT: &str = "# Story\n\nAC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7\nAC-8 AC-9 AC-10 AC-11 AC-12 AC-13 AC-14\n\n## verification\n";

fn initial_state(scale: &str) -> Value {
    json!({
        "stateMachineName":ROUTE_WORK_ITEM,
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
        "executionPlan":execution_plan(),
        "documentPaths":{
            "story":"ae-sdd-doc/Story/STORY-REVIEW-GATE-001.md"
        },
        "storyStates":{
            WORK_ITEM:{
                "phase":"test-running",
                "currentPhase":"test-running",
                "docPath":"ae-sdd-doc/Story/STORY-REVIEW-GATE-001.md"
            }
        }
    })
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

fn reviewer_domain_grant(specialty: &str) -> ScopedGrant {
    ScopedGrant::new(
        // `ManageOwnLease` covers release for the Reviewer role, so the grant must
        // carry it too; Tier 2+ serializes specialties by handing the exclusive
        // Work Item lease from one reviewer to the next.
        ["lease.acquire", "lease.release", "review.record"]
            .into_iter()
            .map(|value| OperationId::new(value).expect("operation id")),
        [
            CapabilityId::new(format!("review.specialty.{specialty}"))
                .expect("specialty capability"),
        ],
        [ProjectPathScope::ProjectRoot],
    )
}

fn reviewer_wire_grant(specialty: &str) -> ScopedGrantWire {
    ScopedGrantWire::from_domain(&reviewer_domain_grant(specialty))
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
                None,
                None,
                root_wire_grant(),
            )),
            delegation: None,
            host_action: None,
            attestation: None,
            response: json!({"sessionId":ROOT_SESSION}),
            replayed: false,
        },
    );
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
            grant: ScopedGrantWire {
                operations: vec![
                    "document.save".to_owned(),
                    "lease.acquire".to_owned(),
                    "review.record".to_owned(),
                ],
                capabilities: vec!["review.specialty.general".to_owned()],
                paths: vec![GrantPathWire::ProjectRoot],
            },
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
    for reviewer in [&GENERAL_REVIEWER, &BE_REVIEWER, &AR_REVIEWER] {
        commit_child_identity(
            persistence,
            &workspace,
            boot_id,
            ChildIdentity {
                session_id: reviewer.session_id,
                agent_id: reviewer.agent_id,
                delegation_id: reviewer.delegation_id,
                parent_session_id: SERIES_SESSION,
                parent_delegation_id: Some(SERIES_DELEGATION),
                role: WireAgentRole::Reviewer,
                grant: reviewer_wire_grant(reviewer.specialty),
                sequence: reviewer.sequence,
            },
        );
    }
}

fn root_wire_grant() -> ScopedGrantWire {
    ScopedGrantWire::from_domain(&ScopedGrant::new(
        ae_sdd_operations::OperationName::ALL
            .into_iter()
            .filter(|operation| *operation != ae_sdd_operations::OperationName::LeaseBreak)
            .map(|operation| OperationId::new(operation.as_str()).expect("operation id")),
        ["general", "be", "ar", "qa"].into_iter().map(|specialty| {
            CapabilityId::new(format!("review.specialty.{specialty}"))
                .expect("specialty capability")
        }),
        [ProjectPathScope::ProjectRoot],
    ))
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
    let adapter_id = "review-gate-adapter";
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
        Some(parent_session_id),
        Some(delegation_id),
        grant.clone(),
    );
    commit_identity(
        persistence,
        "delegation.accept",
        &format!("delegation-{sequence}"),
        RuntimeIdentitySnapshot {
            identity_kind: RuntimeIdentityKind::Delegation,
            workspace: workspace.clone(),
            session: Some(session.clone()),
            delegation: Some(RuntimeDelegationRecord {
                delegation_id: delegation_id.to_owned(),
                workspace_id: WORKSPACE_ID.to_owned(),
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
        },
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

fn session_record(
    session_id: &str,
    agent_id: &str,
    role: WireAgentRole,
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
        root_session_id: ROOT_SESSION.to_owned(),
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

fn operation_params(
    reviewer: &Reviewer,
    operation: &str,
    payload: Value,
    key: &str,
) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(WORKSPACE_ID.to_owned()),
        agent_id: Some(reviewer.agent_id.to_owned()),
        session_id: Some(reviewer.session_id.to_owned()),
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

fn gate_params(work_item_id: &str, key: &str) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(WORKSPACE_ID.to_owned()),
        agent_id: Some("root-agent".to_owned()),
        session_id: Some(ROOT_SESSION.to_owned()),
        capability_token: None,
        turn_id: None,
        work_item_id: Some(work_item_id.to_owned()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: Some(key.to_owned()),
        confirmation: None,
        deadline_ms: 10_000,
        payload: json!({"gateIds":GATES}),
    }
}

/// Acquires one reviewer lease and returns `(leaseId, fencingToken)`.
fn acquire_lease(fixture: &Fixture, reviewer: &Reviewer, key: &str) -> (String, u64) {
    let lease = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &operation_params(
                reviewer,
                "lease.acquire",
                json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
                key,
            ),
            Some(&fixture.reviewer_workspace(reviewer)),
        )
        .unwrap_or_else(|error| panic!("{key} lease failed: {error:?}"));
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

/// Releases a reviewer lease so the next specialty can hold it in turn.
fn release_lease(fixture: &Fixture, reviewer: &Reviewer, lease: &(String, u64), key: &str) {
    let mut request = operation_params(
        reviewer,
        "lease.release",
        json!({"owner":{"role":"reviewer"}}),
        key,
    );
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(reviewer)),
        )
        .unwrap_or_else(|error| panic!("{key} lease release failed: {error:?}"));
}

/// Records one clean `review.record` contribution through daemon authority.
fn record_clean_review(
    fixture: &Fixture,
    reviewer: &Reviewer,
    lease: &(String, u64),
    revision: u64,
    key: &str,
) -> Value {
    let evidence_id = format!("ev-{key}");
    fixture.seal_evidence(&evidence_id);
    let mut request = operation_params(
        reviewer,
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":[evidence_id]
        }),
        key,
    );
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    request.expected_revision = Some(revision);
    fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(reviewer)),
        )
        .unwrap_or_else(|error| panic!("{key} review failed: {error:?}"))
}

fn write_source(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
    fs::write(path, content).expect("source file");
}

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

/// Asserts every requested Review Gate reached `expected`, and for FAIL that the
/// finding carries the structured Review authority denial evidence.
fn assert_all(result: &Value, expected: &str) {
    assert_eq!(result["allPass"], expected == "PASS", "{result}");
    let results = result["results"].as_array().expect("Gate result array");
    assert_eq!(results.len(), GATES.len());
    for (item, gate_id) in results.iter().zip(GATES) {
        assert_eq!(item["gateId"], gate_id);
        assert_eq!(item["outcome"]["kind"], expected, "{gate_id}: {item}");
        if expected == "FAIL" {
            assert!(
                denial_reason(item).is_some(),
                "{gate_id} must report why it denied: {item}"
            );
        }
    }
}

/// Extracts the stable Review authority denial reason from a Gate finding.
fn denial_reason(result: &Value) -> Option<String> {
    result["outcome"]["findings"]
        .as_array()?
        .iter()
        .flat_map(|finding| finding["evidence"].as_array().into_iter().flatten())
        .find(|evidence| evidence["evidenceId"] == "review-authority-denied")
        .and_then(|evidence| evidence["verificationId"].as_str())
        .map(str::to_owned)
}

/// Asserts every Gate failed and that at least one denial mentions `needle`.
fn assert_all_denied_with(result: &Value, needle: &str) {
    assert_all(result, "FAIL");
    let reasons: Vec<_> = result["results"]
        .as_array()
        .expect("Gate result array")
        .iter()
        .filter_map(denial_reason)
        .collect();
    assert!(
        reasons.iter().any(|reason| reason.contains(needle)),
        "expected a denial containing {needle}, got {reasons:?}"
    );
}

/// Drives a tier1 clean review to its terminal PASS authority.
fn passing_tier1_fixture() -> Fixture {
    let fixture = Fixture::new("small");
    let lease = acquire_lease(&fixture, &GENERAL_REVIEWER, "tier1-lease");
    let committed = record_clean_review(&fixture, &GENERAL_REVIEWER, &lease, 7, "tier1-clean");
    assert_eq!(committed["changed"], true);
    let state = read_state(&fixture.state_path);
    assert_eq!(state["reviewSession"]["status"], "completed");
    assert_eq!(state["review"]["batch"]["latestStatus"], "VALID_CLEAN");
    fixture
}

/// Terminal Review authority backed by state, the durable event, the SQLite
/// projection and live reviewer lineage releases every Review Gate.
#[test]
fn complete_authoritative_review_authority_releases_all_review_gates() {
    let fixture = passing_tier1_fixture();
    assert!(fixture.runtime_root.path().is_dir());
    assert_all(&fixture.evaluate_review_gates("tier1-gates"), "PASS");
}

/// A root Route lifecycle key resolves the Review authority anchored below its
/// active Story instead of looking for a second projection under the Route id.
#[test]
fn route_work_item_resolves_active_story_review_projection() {
    let fixture = passing_tier1_fixture();
    assert_all(
        &fixture.evaluate_review_gates_for(ROUTE_WORK_ITEM, "route-story-projection"),
        "PASS",
    );
}

/// An incomplete delegation outside the selected Root -> Series -> Reviewer
/// lineage cannot poison live admission or terminal authority revalidation.
#[test]
fn unrelated_unattested_delegation_does_not_block_review() {
    let fixture = Fixture::new("small");
    fixture.seed_unrelated_unattested_delegation("unrelated-cancelled-delegation");
    let lease = acquire_lease(&fixture, &GENERAL_REVIEWER, "unrelated-lease");
    let committed = record_clean_review(&fixture, &GENERAL_REVIEWER, &lease, 7, "unrelated-clean");
    assert_eq!(committed["changed"], true);
    assert_all(&fixture.evaluate_review_gates("unrelated-gates"), "PASS");
}

/// (a) Valid project state with no SQLite Review projection must FAIL. Project
/// state alone is never sufficient Review authority.
#[test]
fn valid_state_without_the_sqlite_projection_fails_every_review_gate() {
    let fixture = passing_tier1_fixture();
    let connection = fixture.connection();
    // Simulate durable projection loss. Foreign keys are suspended for the
    // tamper itself so the delete models a missing projection rather than a
    // partially consistent one.
    connection
        .execute_batch("PRAGMA foreign_keys=OFF")
        .expect("foreign keys suspend");
    for table in [
        "review_exit_receipt_v2_projection",
        "review_effective_contribution_v2_projection",
        "review_finding_v2_projection",
        "review_remediation_v2_projection",
        "review_attempt_v2_projection",
        "review_batch_v2_projection",
        "review_session_v2_projection",
    ] {
        connection
            .execute(
                &format!("DELETE FROM {table} WHERE workspace_id=?1"),
                rusqlite::params![WORKSPACE_ID],
            )
            .unwrap_or_else(|error| panic!("{table} delete: {error:?}"));
    }
    drop(connection);

    assert_all_denied_with(
        &fixture.evaluate_review_gates("missing-projection"),
        "projection-is-missing",
    );
}

/// (b) A projection row that drifts from authoritative project state must FAIL
/// even though every state-side record is still internally consistent.
#[test]
fn projection_state_drift_fails_every_review_gate() {
    let fixture = passing_tier1_fixture();
    let connection = fixture.connection();
    // `latest_receipt_digest` cannot be used to inject drift: migration 0009
    // binds it through a composite FK into review_attempt_v2_projection, so any
    // altered value is rejected as an invalid row rather than as drift.
    // `input_fingerprint` is CHECK-shaped but not FK-bound, so it drifts the
    // projection away from state while leaving the row structurally valid.
    let changed = connection
        .execute(
            "UPDATE review_batch_v2_projection SET input_fingerprint=?2 \
             WHERE workspace_id=?1",
            rusqlite::params![WORKSPACE_ID, "0".repeat(64)],
        )
        .expect("batch drift applies");
    assert_eq!(changed, 1);
    drop(connection);

    assert_all_denied_with(
        &fixture.evaluate_review_gates("projection-drift"),
        "batch-projection-differs",
    );
}

/// (c) A clean receipt is authority over the exact reviewed bytes. Changing
/// workspace source afterwards must FAIL the Gates as stale.
#[test]
fn workspace_source_change_after_a_clean_receipt_fails_as_stale() {
    let fixture = passing_tier1_fixture();
    write_source(
        fixture.workspace_root.path(),
        "src/lib.rs",
        "pub fn v() -> u8 { 99 }\n",
    );

    assert_all_denied_with(&fixture.evaluate_review_gates("source-drift"), "stale");
}

/// (d) Terminal Review authority remains valid after the short-lived reviewer
/// session and delegation TTLs expire. Revalidation uses their durable identity
/// structure; live contribution admission still requires current TTLs.
#[test]
fn terminal_review_survives_expired_reviewer_identity_ttls() {
    let fixture = passing_tier1_fixture();
    assert_all(&fixture.evaluate_review_gates("lineage-baseline"), "PASS");

    let mut expired = fixture.reviewer_session_record(&GENERAL_REVIEWER);
    expired.expires_at_unix_ms = 1_500;
    fixture.supersede_session(expired, "expired-reviewer-session");

    assert_all(
        &fixture.evaluate_review_gates("expired-reviewer-identity-ttls"),
        "PASS",
    );
    let state = read_state(&fixture.state_path);
    review_authority::validate_review_gate_authority(
        &fixture.database,
        &fixture.root_workspace(),
        &fixture.state_path,
        &state,
        WORK_ITEM,
        fixture.persistence.as_ref(),
        &fixture.boot_id.to_string(),
        &UtcTimestamp::from_str("2201-01-01T00:00:00Z").expect("future timestamp"),
    )
    .expect("terminal Review revalidation ignores expired identity TTLs");
}

#[test]
fn live_review_record_rejects_an_expired_reviewer_session() {
    let fixture = Fixture::new("small");
    let lease = acquire_lease(&fixture, &GENERAL_REVIEWER, "live-session-lease");
    let mut expired = fixture.reviewer_session_record(&GENERAL_REVIEWER);
    expired.expires_at_unix_ms = 1_500;
    fixture.supersede_session(expired, "live-expired-reviewer-session");
    fixture.seal_evidence("ev-live-expired-session");

    let mut request = operation_params(
        &GENERAL_REVIEWER,
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["ev-live-expired-session"]
        }),
        "live-expired-session-record",
    );
    request.lease_id = Some(lease.0);
    request.fencing_token = Some(lease.1);
    request.expected_revision = Some(7);
    let error = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect_err("live Review admission must reject an expired session");
    assert_eq!(error.code(), StableErrorCode::SessionExpired);
}

/// (d) A revoked physical delegation attestation must FAIL. The attestation is
/// the only accepted proof that the daemon ever admitted this reviewer.
#[test]
fn revoked_reviewer_delegation_fails_every_review_gate() {
    let fixture = passing_tier1_fixture();
    assert_all(&fixture.evaluate_review_gates("revoke-baseline"), "PASS");
    fixture.revoke_reviewer_delegation(&GENERAL_REVIEWER, "revoked-reviewer-delegation");

    assert_all(&fixture.evaluate_review_gates("revoked-delegation"), "FAIL");
}

/// (e) A reviewer whose durable specialty grant no longer matches the projected
/// contribution must FAIL. The projection records `general`; re-granting the
/// same physical session to `qa` breaks the specialty join.
#[test]
fn wrong_reviewer_specialty_fails_every_review_gate() {
    let fixture = passing_tier1_fixture();
    assert_all(&fixture.evaluate_review_gates("specialty-baseline"), "PASS");

    let mut regranted = fixture.reviewer_session_record(&GENERAL_REVIEWER);
    regranted.grant = reviewer_wire_grant("qa");
    fixture.supersede_session(regranted, "regranted-reviewer-specialty");

    assert_all(&fixture.evaluate_review_gates("wrong-specialty"), "FAIL");
}

/// (e) The Task author can never be its own reviewer: the author session is not
/// a Reviewer role, so daemon admission refuses before any state is written.
#[test]
fn task_author_cannot_act_as_its_own_reviewer() {
    let fixture = Fixture::new("small");
    let author = Reviewer {
        session_id: AUTHOR_SESSION,
        delegation_id: AUTHOR_DELEGATION,
        agent_id: "author-agent",
        specialty: "general",
        sequence: 2,
    };
    // Author-as-reviewer is refused by `bind_reviewer` during `review.record`,
    // not by `lease.acquire`; the lease ledger has no reviewer-identity opinion.
    let lease = acquire_lease(&fixture, &author, "author-as-reviewer-lease");
    let before = fs::read(&fixture.state_path).expect("state before");
    let evidence_id = "ev-author-as-reviewer";
    fixture.seal_evidence(evidence_id);
    let mut request = operation_params(
        &author,
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":[evidence_id]
        }),
        "author-as-reviewer",
    );
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    request.expected_revision = Some(7);
    let error = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&author)),
        )
        .expect_err("the Task author cannot act as a reviewer");
    assert!(
        matches!(
            error.code(),
            StableErrorCode::TurnIdentityMismatch
                | StableErrorCode::DelegationAttestationFailed
                | StableErrorCode::RoleOperationForbidden
        ),
        "{error:?}"
    );
    assert_eq!(
        fs::read(&fixture.state_path).expect("state after"),
        before,
        "a refused author-as-reviewer attempt must not mutate state"
    );
}

/// (e) A duplicate physical reviewer session cannot satisfy two specialties:
/// terminal v2 authority requires one distinct physical session per specialty.
#[test]
fn duplicate_physical_reviewer_session_cannot_complete_tier2() {
    let fixture = Fixture::new("medium");
    let lease = acquire_lease(&fixture, &BE_REVIEWER, "duplicate-lease");
    let first = record_clean_review(&fixture, &BE_REVIEWER, &lease, 7, "duplicate-be");
    assert_eq!(first["changed"], true);
    let after_first = read_state(&fixture.state_path);

    // Re-grant the SAME physical session to the second required specialty.
    let mut regranted = fixture.reviewer_session_record(&BE_REVIEWER);
    regranted.grant = reviewer_wire_grant("ar");
    fixture.supersede_session(regranted, "duplicate-physical-session");
    let mut request = operation_params(
        &BE_REVIEWER,
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["ev-duplicate-ar"]
        }),
        "duplicate-ar",
    );
    fixture.seal_evidence("ev-duplicate-ar");
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    request.expected_revision = after_first["revision"].as_u64();
    let outcome = fixture.adapter().execute(
        RpcMethod::OperationExecute,
        &request,
        Some(&fixture.reviewer_workspace(&BE_REVIEWER)),
    );

    // Either admission refuses the duplicate physical session, or the batch
    // never reaches terminal PASS authority. Both must leave the Gates closed.
    if let Err(error) = outcome {
        assert!(!error.message().is_empty(), "{error:?}");
    }
    assert_all(&fixture.evaluate_review_gates("duplicate-session"), "FAIL");
}

/// (f) Tier 2 requires two independent specialties and a deterministic Gate
/// final proof. Complete Tier 2 authority releases every Review Gate.
#[test]
fn complete_tier2_authority_releases_all_review_gates() {
    let fixture = Fixture::new("medium");
    let be_lease = acquire_lease(&fixture, &BE_REVIEWER, "tier2-be-lease");
    let first = record_clean_review(&fixture, &BE_REVIEWER, &be_lease, 7, "tier2-be");
    assert_eq!(first["changed"], true);
    let after_first = read_state(&fixture.state_path);
    assert_eq!(after_first["reviewSession"]["tier"], "tier2");
    assert_eq!(
        after_first["reviewSession"]["requiredSpecialties"],
        json!(["be", "ar"])
    );

    // The first specialty alone cannot close the batch, so the Gates must deny.
    assert_all(&fixture.evaluate_review_gates("tier2-partial"), "FAIL");

    // The Work Item lease is exclusive AND owner-bound: `mutate_state` derives
    // the LeaseOwner from the authenticated session, so the AR reviewer cannot
    // borrow the BE reviewer's lease. Each specialty must hold the lease in
    // turn, which is exactly the real multi-reviewer serialization.
    release_lease(&fixture, &BE_REVIEWER, &be_lease, "tier2-be-release");
    let ar_lease = acquire_lease(&fixture, &AR_REVIEWER, "tier2-ar-lease");
    let second = record_clean_review(
        &fixture,
        &AR_REVIEWER,
        &ar_lease,
        after_first["revision"].as_u64().expect("revision"),
        "tier2-ar",
    );
    assert_eq!(second["changed"], true);
    let after_second = read_state(&fixture.state_path);
    assert_eq!(after_second["reviewSession"]["status"], "completed");
    assert_eq!(
        after_second["review"]["receipt"]["finalProof"]["kind"],
        "deterministic_gates"
    );

    assert_all(&fixture.evaluate_review_gates("tier2-complete"), "PASS");
}

/// (g) Tier 3 must join the durable verification job, receipt locator, active
/// manifest and COMMITTED journal mutation. Without that proof the daemon
/// refuses to record the terminal review at all, so no Gate can be released.
#[test]
fn tier3_without_final_verification_proof_cannot_release_review_gates() {
    let fixture = Fixture::new("large");
    let lease = acquire_lease(&fixture, &BE_REVIEWER, "tier3-lease");
    fixture.seal_evidence("ev-tier3-be");
    let mut request = operation_params(
        &BE_REVIEWER,
        "review.record",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["ev-tier3-be"]
        }),
        "tier3-be",
    );
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    request.expected_revision = Some(7);
    let first = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&BE_REVIEWER)),
        )
        .expect("a non-completing tier3 contribution is admitted");
    assert_eq!(first["changed"], true);
    let state = read_state(&fixture.state_path);
    assert_eq!(state["reviewSession"]["tier"], "tier3");
    assert_eq!(
        state["reviewSession"]["requiredSpecialties"],
        json!(["be", "ar", "qa"])
    );

    // Tier 3 is not complete and carries no final-verification proof, so every
    // Review Gate must deny.
    assert_all(&fixture.evaluate_review_gates("tier3-no-proof"), "FAIL");
}

/// A legacy v1 review projection can never release the v2 Review Gates, even
/// with no Review authority dependencies to join.
#[test]
fn legacy_v1_review_projection_cannot_release_v2_review_gates() {
    let fixture = Fixture::new("small");
    let mut state = initial_state("small");
    state["reviewSession"] = json!({
        "schemaVersion":"v1",
        "reviewId":"legacy-review",
        "workItemId":WORK_ITEM,
        "tier":1,
        "requiredRoles":["engineering"],
        "inputFingerprint":INPUT,
        "rulesetFingerprint":RULESET,
        "round":1,
        "cleanStreak":1,
        "budget":{"maxRounds":4,"maxFindings":32,"maxDurationMs":60000},
        "status":"completed",
        "authorSessionId":AUTHOR_SESSION,
        "reviewers":[]
    });
    state["review"] = json!({"status":"passed","findings":[]});
    state["reviewLoop"] = json!({"status":"passed","exitReason":"passed","cleanStreak":2});
    write_state(&fixture.state_path, &state);

    assert_all(&fixture.evaluate_review_gates("legacy-v1"), "FAIL");
}
