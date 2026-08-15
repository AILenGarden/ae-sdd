//! Review contribution/finalize split end-to-end coverage.
//!
//! Every case drives the real typed operations through daemon authority:
//! `review.contribute` appends one reviewer contribution without a
//! cross-reviewer writer lease, `review.finalize` aggregates the pending
//! projection exactly once under a short root writer lease, and the legacy
//! `review.record` adapter keeps its compat behavior. Hand-built state can
//! never release a Review Gate, so the final PASS assertion also proves the
//! durable event, the SQLite projection, and the reviewer lineage all join.

use std::{fs, path::Path, str::FromStr, sync::Arc};

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

const WORK_ITEM: &str = "STORY-REVIEW-CONTRIBUTION-001";
const PROJECT_KEY: &str = "review-contribution";
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
    /// `medium` -> tier2 (`be` + `ar`).
    fn new(scale: &str) -> Self {
        let workspace_root = TempDir::new().expect("workspace tempdir");
        let state_dir = workspace_root
            .path()
            .join(".auto-engineering/review-contribution");
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
            "ae-sdd-doc/Story/STORY-REVIEW-CONTRIBUTION-001.md",
            STORY_DOCUMENT,
        );
        write_source(
            workspace_root.path(),
            "ae-sdd-doc/RA/review-contribution.md",
            "# RA review-contribution\n",
        );
        write_source(
            workspace_root.path(),
            "ae-sdd-doc/DR/review-contribution.md",
            "# DR review-contribution\n",
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

    fn root_gate_workspace(&self) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Root,
            ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot]),
        )
    }

    /// The root/finalizer workspace: daemon-derived Root role plus a scoped
    /// grant that carries the writer-lease and `review.finalize` operations.
    fn finalizer_workspace(&self) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Root,
            ScopedGrant::new(
                ["lease.acquire", "lease.release", "review.finalize"]
                    .into_iter()
                    .map(|value| OperationId::new(value).expect("operation id")),
                [],
                [ProjectPathScope::ProjectRoot],
            ),
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
        self.adapter()
            .execute(
                RpcMethod::GateEvaluate,
                &gate_params(key),
                Some(&self.root_gate_workspace()),
            )
            .expect("review Gates return typed outcomes")
    }

    fn revision(&self) -> u64 {
        read_state(&self.state_path)["revision"]
            .as_u64()
            .expect("state revision")
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
        "goal":"authoritative review contribution fixture",
        "changedPaths":["src/lib.rs"],
        "verification":verification,
        "risks":["fixture risk"],
        "approved":true,
        "sourceReads":[
            "src/lib.rs",
            "ae-sdd-doc/RA/review-contribution.md",
            "ae-sdd-doc/DR/review-contribution.md",
            "ae-sdd-doc/Story/STORY-REVIEW-CONTRIBUTION-001.md"
        ]
    })
}

/// Story document whose AC ids are a subset of the plan verification matrix.
const STORY_DOCUMENT: &str = "# Story\n\nAC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7\nAC-8 AC-9 AC-10 AC-11 AC-12 AC-13 AC-14\n\n## verification\n";

fn initial_state(scale: &str) -> Value {
    json!({
        "stateMachineName":"PRD-REVIEW-CONTRIBUTION-001",
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
            "story":"ae-sdd-doc/Story/STORY-REVIEW-CONTRIBUTION-001.md"
        },
        "storyStates":{
            WORK_ITEM:{
                "phase":"test-running",
                "currentPhase":"test-running",
                "docPath":"ae-sdd-doc/Story/STORY-REVIEW-CONTRIBUTION-001.md"
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

/// The reviewer grant after the split: `review.contribute` plus exactly one
/// specialty capability, with `review.record` retained for the compat adapter.
/// `review.finalize` is never part of a reviewer grant.
fn reviewer_domain_grant(specialty: &str) -> ScopedGrant {
    ScopedGrant::new(
        [
            "lease.acquire",
            "lease.release",
            "review.contribute",
            "review.record",
        ]
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
                    "review.contribute".to_owned(),
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
    let adapter_id = "review-contribution-adapter";
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
                work_item_id: session.current_work_item.clone(),
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
    agent: &str,
    session: &str,
    operation: &str,
    payload: Value,
    key: &str,
) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(WORKSPACE_ID.to_owned()),
        agent_id: Some(agent.to_owned()),
        session_id: Some(session.to_owned()),
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

fn reviewer_params(
    reviewer: &Reviewer,
    operation: &str,
    payload: Value,
    key: &str,
) -> RequestParams<Value> {
    operation_params(
        reviewer.agent_id,
        reviewer.session_id,
        operation,
        payload,
        key,
    )
}

fn root_params(operation: &str, payload: Value, key: &str) -> RequestParams<Value> {
    operation_params("root-agent", ROOT_SESSION, operation, payload, key)
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

/// Acquires one writer lease for the root/finalizer and returns
/// `(leaseId, fencingToken)`.
fn acquire_root_lease(fixture: &Fixture, key: &str) -> (String, u64) {
    let lease = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &root_params(
                "lease.acquire",
                json!({"owner":{"role":"root"},"ttlSeconds":300}),
                key,
            ),
            Some(&fixture.finalizer_workspace()),
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

/// Acquires one reviewer lease and returns `(leaseId, fencingToken)`.
fn acquire_reviewer_lease(fixture: &Fixture, reviewer: &Reviewer, key: &str) -> (String, u64) {
    let lease = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &reviewer_params(
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

fn clean_review_payload(evidence_id: &str) -> Value {
    json!({
        "status":"passed",
        "findings":[],
        "reviewedPaths":["src/lib.rs"],
        "evidenceIds":[evidence_id]
    })
}

/// Appends one clean `review.contribute` through daemon authority. The
/// operation carries no lease: reviewers serialize through the Work Item actor
/// and the idempotency/fingerprint CAS, never a cross-reviewer writer lease.
fn contribute_clean_review(
    fixture: &Fixture,
    reviewer: &Reviewer,
    revision: u64,
    key: &str,
) -> Value {
    let evidence_id = format!("ev-{key}");
    fixture.seal_evidence(&evidence_id);
    let mut request = reviewer_params(
        reviewer,
        "review.contribute",
        clean_review_payload(&evidence_id),
        key,
    );
    request.expected_revision = Some(revision);
    fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(reviewer)),
        )
        .unwrap_or_else(|error| panic!("{key} contribute failed: {error:?}"))
}

/// Aggregates the pending contribution projection once through
/// `review.finalize` under the root writer lease.
fn finalize_review(fixture: &Fixture, lease: &(String, u64), revision: u64, key: &str) -> Value {
    let mut request = root_params("review.finalize", json!({}), key);
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    request.expected_revision = Some(revision);
    fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.finalizer_workspace()),
        )
        .unwrap_or_else(|error| panic!("{key} finalize failed: {error:?}"))
}

/// Records one clean legacy `review.record` through daemon authority.
fn record_clean_review(
    fixture: &Fixture,
    reviewer: &Reviewer,
    lease: &(String, u64),
    revision: u64,
    key: &str,
) -> Value {
    let evidence_id = format!("ev-{key}");
    fixture.seal_evidence(&evidence_id);
    let mut request = reviewer_params(
        reviewer,
        "review.record",
        clean_review_payload(&evidence_id),
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

fn pending_contributions(state: &Value) -> &Vec<Value> {
    state["review"]["pendingContributions"]
        .as_array()
        .expect("pending contribution projection")
}

fn assert_all(result: &Value, expected: &str) {
    assert_eq!(result["allPass"], expected == "PASS", "{result}");
    let results = result["results"].as_array().expect("Gate result array");
    assert_eq!(results.len(), GATES.len());
    for (item, gate_id) in results.iter().zip(GATES) {
        assert_eq!(item["gateId"], gate_id);
        assert_eq!(item["outcome"]["kind"], expected, "{gate_id}: {item}");
    }
}

/// Step 1: the operations registry freezes `review.contribute` and
/// `review.finalize` with their exact write/lease/revision/idempotency
/// preconditions, and `review.record` stays registered for the compat adapter.
#[test]
fn registry_freezes_contribute_and_finalize_preconditions() {
    let contribute = OperationName::from_str("review.contribute")
        .expect("review.contribute is registered")
        .spec();
    assert!(contribute.requires_workspace);
    assert!(contribute.requires_work_item);
    assert!(contribute.writes);
    assert!(
        !contribute.requires_lease,
        "review.contribute must not hold a cross-reviewer writer lease"
    );
    assert!(contribute.requires_revision);
    assert!(contribute.requires_idempotency);
    assert!(!contribute.requires_confirmation);
    let contribute_fields: Vec<_> = contribute.fields.iter().map(|field| field.name).collect();
    assert_eq!(
        contribute_fields,
        vec!["status", "findings", "reviewedPaths", "evidenceIds"]
    );

    let finalize = OperationName::from_str("review.finalize")
        .expect("review.finalize is registered")
        .spec();
    assert!(finalize.requires_workspace);
    assert!(finalize.requires_work_item);
    assert!(finalize.writes);
    assert!(
        finalize.requires_lease,
        "review.finalize aggregates under a short writer lease"
    );
    assert!(finalize.requires_revision);
    assert!(finalize.requires_idempotency);
    assert!(!finalize.requires_confirmation);
    assert!(finalize.fields.is_empty());

    assert!(
        OperationName::from_str("review.record").is_ok(),
        "review.record remains registered as the compat adapter"
    );
}

/// Step 2 + AC-001: two reviewers append independent contributions without any
/// writer lease; the root finalizer aggregates them in exactly one attempt;
/// the terminal authority then releases every Review Gate.
#[test]
fn two_reviewers_contribute_independently_then_finalize_aggregates_once() {
    let fixture = Fixture::new("medium");

    let first = contribute_clean_review(&fixture, &BE_REVIEWER, 7, "split-be");
    assert_eq!(first["changed"], true);
    assert_eq!(first["data"]["status"], "pending");
    let state = read_state(&fixture.state_path);
    assert_eq!(state["review"]["status"], "pending");
    assert_eq!(pending_contributions(&state).len(), 1);
    assert!(
        state["review"].get("batch").is_none(),
        "a contribution must not close or even open a batch: {state}"
    );

    // No lease was held by the BE reviewer, so the AR reviewer appends
    // immediately with its own idempotency key.
    let second = contribute_clean_review(&fixture, &AR_REVIEWER, fixture.revision(), "split-ar");
    assert_eq!(second["changed"], true);
    let state = read_state(&fixture.state_path);
    assert_eq!(pending_contributions(&state).len(), 2);
    assert_eq!(state["reviewSession"]["status"], "running");
    assert_eq!(
        state["reviewSession"]["requiredSpecialties"],
        json!(["be", "ar"])
    );

    // Contributions alone never release a Review Gate.
    assert_all(&fixture.evaluate_review_gates("split-partial"), "FAIL");

    let lease = acquire_root_lease(&fixture, "split-finalize-lease");
    let finalized = finalize_review(&fixture, &lease, fixture.revision(), "split-finalize");
    assert_eq!(finalized["changed"], true);
    let state = read_state(&fixture.state_path);
    assert_eq!(state["reviewSession"]["status"], "completed");
    assert_eq!(state["review"]["batch"]["latestStatus"], "VALID_CLEAN");
    assert_eq!(
        state["reviewSession"]["counters"]["attempts"], 1,
        "the finalize must aggregate the pending projection in exactly one attempt"
    );
    assert_eq!(
        state["review"]["batch"]["retainedContributions"]
            .as_array()
            .expect("retained contributions")
            .len(),
        2
    );
    assert!(
        state["review"].get("pendingContributions").is_none(),
        "a committed finalize consumes the pending projection"
    );
    assert_eq!(
        state["review"]["receipt"]["finalProof"]["kind"],
        "deterministic_gates"
    );
    assert!(
        fixture.runtime_root.path().is_dir(),
        "the runtime projection directory stays alive for the Gate evaluation"
    );

    // Replaying the same finalize replays the committed receipt: no second
    // attempt, no second side effect.
    let replayed = finalize_review(&fixture, &lease, fixture.revision(), "split-finalize");
    assert_eq!(replayed["changed"], false);
    let state = read_state(&fixture.state_path);
    assert_eq!(state["reviewSession"]["counters"]["attempts"], 1);

    assert_all(&fixture.evaluate_review_gates("split-complete"), "PASS");
}

/// Replaying one `review.contribute` with the same idempotency key and payload
/// returns the original receipt and appends nothing.
#[test]
fn contribute_replay_with_same_key_and_payload_adds_nothing() {
    let fixture = Fixture::new("small");
    let first = contribute_clean_review(&fixture, &GENERAL_REVIEWER, 7, "replay-general");
    assert_eq!(first["changed"], true);
    let revision_after = fixture.revision();

    let evidence_id = "ev-replay-general";
    fixture.seal_evidence(evidence_id);
    let mut request = reviewer_params(
        &GENERAL_REVIEWER,
        "review.contribute",
        clean_review_payload(evidence_id),
        "replay-general",
    );
    request.expected_revision = Some(7);
    let replayed = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect("same key and payload replays the committed receipt");
    assert_eq!(replayed["changed"], false);
    assert_eq!(fixture.revision(), revision_after);
    let state = read_state(&fixture.state_path);
    assert_eq!(
        pending_contributions(&state).len(),
        1,
        "a replayed contribution must not append a second entry"
    );
}

/// The same idempotency key with a different payload is rejected with the
/// legacy-stable idempotency conflict.
#[test]
fn contribute_same_key_with_a_different_payload_is_rejected() {
    let fixture = Fixture::new("small");
    let first = contribute_clean_review(&fixture, &GENERAL_REVIEWER, 7, "conflict-general");
    assert_eq!(first["changed"], true);

    let mut different = clean_review_payload("ev-conflict-general");
    different["reviewedPaths"] = json!([
        "src/lib.rs",
        "ae-sdd-doc/Story/STORY-REVIEW-CONTRIBUTION-001.md"
    ]);
    let mut request = reviewer_params(
        &GENERAL_REVIEWER,
        "review.contribute",
        different,
        "conflict-general",
    );
    request.expected_revision = Some(fixture.revision());
    let error = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect_err("same key with a different payload must be rejected");
    assert_eq!(error.code(), StableErrorCode::IdempotencyKeyReused);
    let state = read_state(&fixture.state_path);
    assert_eq!(pending_contributions(&state).len(), 1);
}

/// One physical reviewer can only hold one pending contribution per batch: a
/// second contribution under a different idempotency key fails closed instead
/// of poisoning the later aggregation.
#[test]
fn same_reviewer_cannot_queue_a_second_pending_contribution() {
    let fixture = Fixture::new("small");
    let first = contribute_clean_review(&fixture, &GENERAL_REVIEWER, 7, "queue-first");
    assert_eq!(first["changed"], true);

    let mut request = reviewer_params(
        &GENERAL_REVIEWER,
        "review.contribute",
        clean_review_payload("ev-queue-second"),
        "queue-second",
    );
    fixture.seal_evidence("ev-queue-second");
    request.expected_revision = Some(fixture.revision());
    let error = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect_err("a second pending contribution from one reviewer must fail closed");
    assert_eq!(error.code(), StableErrorCode::GateBlocked, "{error:?}");
    let state = read_state(&fixture.state_path);
    assert_eq!(pending_contributions(&state).len(), 1);
}

/// Grant boundary: a reviewer grant carries `review.contribute` and its
/// specialty but never `review.finalize`; only the root/finalizer may
/// aggregate. The daemon-verified role check refuses before any state write.
#[test]
fn reviewer_cannot_finalize_a_batch() {
    let fixture = Fixture::new("small");
    let first = contribute_clean_review(&fixture, &GENERAL_REVIEWER, 7, "forbidden-general");
    assert_eq!(first["changed"], true);
    let lease = acquire_reviewer_lease(&fixture, &GENERAL_REVIEWER, "forbidden-lease");
    let before = fs::read(&fixture.state_path).expect("state before");

    let mut request = reviewer_params(
        &GENERAL_REVIEWER,
        "review.finalize",
        json!({}),
        "forbidden-finalize",
    );
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    request.expected_revision = Some(fixture.revision());
    let error = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect_err("a reviewer must never finalize a review batch");
    assert_eq!(
        error.code(),
        StableErrorCode::RoleOperationForbidden,
        "{error:?}"
    );
    assert_eq!(
        fs::read(&fixture.state_path).expect("state after"),
        before,
        "a refused finalize must not mutate state"
    );
}

/// Finalizing with no pending contribution fails closed; the writer lease and
/// revision cannot manufacture an aggregation out of nothing.
#[test]
fn finalize_without_pending_contributions_is_rejected() {
    let fixture = Fixture::new("small");
    let lease = acquire_root_lease(&fixture, "empty-finalize-lease");
    let before = fs::read(&fixture.state_path).expect("state before");

    let mut request = root_params("review.finalize", json!({}), "empty-finalize");
    request.lease_id = Some(lease.0.clone());
    request.fencing_token = Some(lease.1);
    request.expected_revision = Some(fixture.revision());
    let error = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.finalizer_workspace()),
        )
        .expect_err("finalize requires at least one pending contribution");
    assert_eq!(error.code(), StableErrorCode::GateBlocked, "{error:?}");
    assert_eq!(
        fs::read(&fixture.state_path).expect("state after"),
        before,
        "a refused finalize must not mutate state"
    );
}

/// Compat adapter: `review.record` still appends one contribution and
/// immediately finalizes it, producing the same terminal v2 authority and
/// releasing every Review Gate without a second business implementation.
#[test]
fn legacy_review_record_adapter_still_completes_tier1() {
    let fixture = Fixture::new("small");
    let lease = acquire_reviewer_lease(&fixture, &GENERAL_REVIEWER, "adapter-lease");
    let committed = record_clean_review(&fixture, &GENERAL_REVIEWER, &lease, 7, "adapter-clean");
    assert_eq!(committed["changed"], true);
    let state = read_state(&fixture.state_path);
    assert_eq!(state["reviewSession"]["status"], "completed");
    assert_eq!(state["review"]["batch"]["latestStatus"], "VALID_CLEAN");
    assert!(
        state["review"].get("pendingContributions").is_none(),
        "the adapter finalizes its own contribution immediately"
    );

    assert_all(&fixture.evaluate_review_gates("adapter-gates"), "PASS");
}
