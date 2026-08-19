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

use ae_sdd_domain::{
    AgentRole, BootId, CapabilityId, InputFingerprint, OperationId, ProjectPathScope, ScopedGrant,
    SessionId,
};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_operations::OperationName;
use ae_sdd_protocol::{
    ClientKind, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_protocol::{HandshakeRequest, PROTOCOL_RANGE_V1, SecretString};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, ClockPort, ConnectionState,
    CurrentBootSessionReceipt, GrantPathWire, PersistencePort, RuntimeConfig,
    RuntimeDelegationAttestationRecord, RuntimeDelegationHostActionRecord, RuntimeDelegationRecord,
    RuntimeIdentityKind, RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeService,
    RuntimeSessionRecord, RuntimeWorkspaceRecord, ScopedGrantWire, WireAgentRole,
    WorkspaceResolverPort,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

#[path = "../../ae-sdd-runtime/tests/support/mod.rs"]
mod support;
use support::params;

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

// Test support types from runtime
use std::sync::atomic::{AtomicU64, Ordering};

struct TestClock(AtomicU64);

impl TestClock {
    fn new(now: u64) -> Self {
        Self(AtomicU64::new(now))
    }
}

impl ClockPort for TestClock {
    fn now_unix_ms(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

struct TestResolver;

impl WorkspaceResolverPort for TestResolver {
    fn resolve(
        &self,
        requested_root: &str,
    ) -> ae_sdd_runtime::RuntimeResult<ae_sdd_runtime::ResolvedWorkspace> {
        Ok(ae_sdd_runtime::ResolvedWorkspace {
            canonical_root: requested_root.to_owned(),
            inside_allowed_root: true,
        })
    }
}

use review_authority::{
    authoritative_review_workspace_input_fingerprint, boots_with_receipt_fallback,
    validate_finalized_review_evidence,
};

const WORK_ITEM: &str = "STORY-REVIEW-CONTRIBUTION-001";
const ROUTE_WORK_ITEM: &str = "ROUTE-REVIEW-CONTRIBUTION-001";
const ROUTE_ACTIVE_STORY: &str = "STORY-REVIEW-CONTRIBUTION-ACTIVE";
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
    work_item_id: &'static str,
    story_id: &'static str,
}

impl Fixture {
    /// `scale` selects the Review tier: `small` -> tier1 (`general`),
    /// `medium` -> tier2 (`be` + `ar`).
    fn new(scale: &str) -> Self {
        Self::new_scoped(scale, WORK_ITEM, WORK_ITEM)
    }

    fn new_scoped(scale: &str, work_item_id: &'static str, story_id: &'static str) -> Self {
        let workspace_root = TempDir::new().expect("workspace tempdir");
        let state_dir = workspace_root
            .path()
            .join(".auto-engineering/review-contribution");
        fs::create_dir_all(&state_dir).expect("state directory");
        let state_path = state_dir.join("state.json");
        write_state(&state_path, &initial_state(scale, work_item_id, story_id));
        write_source(
            workspace_root.path(),
            "src/lib.rs",
            "pub fn v() -> u8 { 1 }\n",
        );
        write_source(
            workspace_root.path(),
            &format!("ae-sdd-doc/Story/{story_id}.md"),
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
        seed_identity_authority(
            persistence.as_ref(),
            workspace_root.path(),
            boot_id,
            work_item_id,
        );
        Self {
            workspace_root,
            runtime_root,
            state_path,
            database,
            boot_id,
            event_store_id,
            persistence,
            work_item_id,
            story_id,
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

    fn author_workspace(&self) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Task,
            ScopedGrant::new(
                [
                    "lease.acquire",
                    "lease.release",
                    "evidence.record",
                    "evidence.finalize",
                ]
                .into_iter()
                .map(|value| OperationId::new(value).expect("operation id")),
                [],
                [ProjectPathScope::ProjectRoot],
            ),
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
        write_finalized_manifest(
            self.workspace_root.path(),
            self.story_id,
            &input.to_string(),
            evidence_id,
        );
    }

    fn evaluate_review_gates(&self, key: &str) -> Value {
        self.adapter()
            .execute(
                RpcMethod::GateEvaluate,
                &gate_params_for_work_item(self.work_item_id, key),
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
fn execution_plan(story_id: &str) -> Value {
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
            format!("ae-sdd-doc/Story/{story_id}.md")
        ]
    })
}

/// Story document whose AC ids are a subset of the plan verification matrix.
const STORY_DOCUMENT: &str = "# Story\n\nAC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7\nAC-8 AC-9 AC-10 AC-11 AC-12 AC-13 AC-14\n\n## verification\n";

fn initial_state(scale: &str, work_item_id: &str, story_id: &str) -> Value {
    let mut state = json!({
        "stateMachineName":work_item_id,
        "activeStory":story_id,
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
        "executionPlan":execution_plan(story_id),
        "documentPaths":{
            "story":format!("ae-sdd-doc/Story/{story_id}.md")
        },
        "storyStates":{}
    });
    state["storyStates"][story_id] = json!({
        "phase":"test-running",
        "currentPhase":"test-running",
        "docPath":format!("ae-sdd-doc/Story/{story_id}.md")
    });
    state
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

fn seed_identity_authority(
    persistence: &SqliteRuntimePersistence,
    root: &Path,
    boot_id: BootId,
    work_item_id: &str,
) {
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
            current_boot_receipt: None,
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
                work_item_id,
            )),
            delegation: None,
            host_action: None,
            attestation: None,
            current_boot_receipt: None,
            response: json!({"sessionId":ROOT_SESSION}),
            replayed: false,
        },
    );
    commit_child_identity(
        persistence,
        &workspace,
        boot_id,
        work_item_id,
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
        work_item_id,
        ChildIdentity {
            session_id: AUTHOR_SESSION,
            agent_id: "author-agent",
            delegation_id: AUTHOR_DELEGATION,
            parent_session_id: SERIES_SESSION,
            parent_delegation_id: Some(SERIES_DELEGATION),
            role: WireAgentRole::Task,
            grant: ScopedGrantWire {
                operations: vec![
                    "document.save".to_owned(),
                    "evidence.finalize".to_owned(),
                    "evidence.record".to_owned(),
                    "lease.acquire".to_owned(),
                    "lease.release".to_owned(),
                ],
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
            work_item_id,
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
    work_item_id: &str,
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
        work_item_id,
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
                host_action_id: action_id.clone(),
                host_ack_id: ack_id,
                action_digest,
                ack_digest,
                claim_digest: digest(format!("claim-{sequence}")),
                grant: grant.clone(),
                attestation_ref: format!("delegation:{delegation_id}"),
                attestation_digest: digest(format!("attestation-{sequence}")),
                accepted_boot_id: boot_id.to_string(),
                accepted_at_unix_ms: 1_000,
                expires_at_unix_ms: 4_102_444_800_000,
            }),
            current_boot_receipt: None,
            response: json!({
                "delegationId":delegation_id,
                "status":"running",
                "grant":grant,
                "childRole":role,
                "actionId":action_id,
                "childSessionId":session_id
            }),
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
            current_boot_receipt: None,
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
    work_item_id: &str,
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
        current_work_item: Some(work_item_id.to_owned()),
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
    operation_params_for_work_item(WORK_ITEM, agent, session, operation, payload, key)
}

fn operation_params_for_work_item(
    work_item_id: &str,
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
        work_item_id: Some(work_item_id.to_owned()),
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

fn gate_params_for_work_item(work_item_id: &str, key: &str) -> RequestParams<Value> {
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

/// Acquires one writer lease for the root/finalizer and returns
/// `(leaseId, fencingToken)`.
fn acquire_root_lease(fixture: &Fixture, key: &str) -> (String, u64) {
    let lease = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &operation_params_for_work_item(
                fixture.work_item_id,
                "root-agent",
                ROOT_SESSION,
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

fn record_and_finalize_story_evidence(fixture: &Fixture, key: &str) -> (String, InputFingerprint) {
    let artifact_path = format!("results/{key}.json");
    write_source(
        fixture.workspace_root.path(),
        &artifact_path,
        "{\"pass\":true}\n",
    );
    let workspace = fixture.author_workspace();
    let initial_state = read_state(&fixture.state_path);
    let input = authoritative_review_workspace_input_fingerprint(&workspace, &initial_state)
        .expect("review input fingerprint before evidence");

    let lease = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &operation_params_for_work_item(
                fixture.work_item_id,
                "author-agent",
                AUTHOR_SESSION,
                "lease.acquire",
                json!({"owner":{"role":"task"},"ttlSeconds":300}),
                &format!("{key}-lease"),
            ),
            Some(&workspace),
        )
        .expect("author evidence lease acquires");
    let lease_id = lease["data"]["leaseId"].as_str().expect("lease id");
    let fencing_token = lease["data"]["fencingToken"]
        .as_u64()
        .expect("fencing token");

    let mut record = operation_params_for_work_item(
        fixture.work_item_id,
        "author-agent",
        AUTHOR_SESSION,
        "evidence.record",
        json!({
            "artifactPath":artifact_path,
            "inputFingerprint":input.to_string(),
            "kind":"focused-test",
            "command":["cargo","test"],
            "toolchainFingerprint":"rust-1",
            "exitCode":0,
            "summary":{"verification":"V-012"},
            "logicalKey":format!("review/{key}")
        }),
        &format!("{key}-record"),
    );
    record.lease_id = Some(lease_id.to_owned());
    record.fencing_token = Some(fencing_token);
    record.expected_revision = Some(fixture.revision());
    let recorded = fixture
        .adapter()
        .execute(RpcMethod::OperationExecute, &record, Some(&workspace))
        .expect("Story-scoped evidence records");
    let evidence_id = recorded["data"]["evidenceId"]
        .as_str()
        .expect("evidence id")
        .to_owned();

    let mut finalize = operation_params_for_work_item(
        fixture.work_item_id,
        "author-agent",
        AUTHOR_SESSION,
        "evidence.finalize",
        json!({}),
        &format!("{key}-finalize"),
    );
    finalize.lease_id = Some(lease_id.to_owned());
    finalize.fencing_token = Some(fencing_token);
    finalize.expected_revision = Some(fixture.revision());
    fixture
        .adapter()
        .execute(RpcMethod::OperationExecute, &finalize, Some(&workspace))
        .expect("Story-scoped evidence finalizes");

    let mut release = operation_params_for_work_item(
        fixture.work_item_id,
        "author-agent",
        AUTHOR_SESSION,
        "lease.release",
        json!({"owner":{"role":"task"}}),
        &format!("{key}-release"),
    );
    release.lease_id = Some(lease_id.to_owned());
    release.fencing_token = Some(fencing_token);
    fixture
        .adapter()
        .execute(RpcMethod::OperationExecute, &release, Some(&workspace))
        .expect("author evidence lease releases");

    let finalized_state = read_state(&fixture.state_path);
    let finalized_input =
        authoritative_review_workspace_input_fingerprint(&workspace, &finalized_state)
            .expect("review input fingerprint after evidence");
    assert_eq!(finalized_input, input);
    (evidence_id, input)
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
    let mut request = operation_params_for_work_item(
        fixture.work_item_id,
        reviewer.agent_id,
        reviewer.session_id,
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
    let mut request = operation_params_for_work_item(
        fixture.work_item_id,
        "root-agent",
        ROOT_SESSION,
        "review.finalize",
        json!({}),
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

fn write_finalized_manifest(root: &Path, story_id: &str, input: &str, evidence_id: &str) {
    let mut manifest = json!({
        "schemaVersion":1,
        "storyId":story_id,
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
        ".auto-engineering/{story_id}/evidence/manifest.json"
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

#[test]
fn route_work_item_review_resolves_active_story_evidence_manifest() {
    let fixture = Fixture::new_scoped("small", ROUTE_WORK_ITEM, ROUTE_ACTIVE_STORY);
    let (evidence_id, input_fingerprint) =
        record_and_finalize_story_evidence(&fixture, "route-story-scope");
    let state = read_state(&fixture.state_path);
    assert_eq!(
        state["evidenceAuthority"]["manifestRef"],
        format!(".auto-engineering/{ROUTE_ACTIVE_STORY}/evidence/manifest.json")
    );
    assert_eq!(
        state["evidenceAuthority"]["ledgerRef"],
        format!(".auto-engineering/{ROUTE_ACTIVE_STORY}/evidence/ledger.jsonl")
    );

    validate_finalized_review_evidence(
        &fixture.reviewer_workspace(&GENERAL_REVIEWER),
        &state,
        ROUTE_WORK_ITEM,
        input_fingerprint,
    )
    .expect("terminal Review verification resolves the active Story evidence scope");

    let mut contribute = operation_params_for_work_item(
        ROUTE_WORK_ITEM,
        GENERAL_REVIEWER.agent_id,
        GENERAL_REVIEWER.session_id,
        "review.contribute",
        clean_review_payload(&evidence_id),
        "route-story-scope-contribute",
    );
    contribute.expected_revision = Some(fixture.revision());
    let result = fixture
        .adapter()
        .execute(
            RpcMethod::OperationExecute,
            &contribute,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect("clean Review contribution resolves the active Story evidence scope");
    assert_eq!(result["changed"], true);
    assert_eq!(result["data"]["status"], "pending");
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

/// RED test: Boot A→B reattachment must permit historical attestations.
///
/// Scenario: All sessions were accepted in boot A, daemon restarts to boot B,
/// session.reopen writes current_boot_receipt with boot_id=B for all four
/// lineage sessions (Root, Series, Task, Reviewer). When review.contribute
/// runs in boot B with a historical accepted_boot_id=A attestation,
/// boots_with_receipt_fallback must return AttestationBoots permitting boot A.
///
/// Expected: review.contribute succeeds and commits the contribution.
/// Actual (current broken code): FAILS because committed=Some(B) rejects A.
#[test]
fn boot_restart_reattachment_permits_historical_attestation() {
    let boot_a = BootId::from_uuid(Uuid::from_u128(800));
    let boot_b = BootId::from_uuid(Uuid::from_u128(801));

    let workspace_root = TempDir::new().expect("workspace tempdir");
    let state_dir = workspace_root
        .path()
        .join(".auto-engineering/review-contribution");
    fs::create_dir_all(&state_dir).expect("state directory");
    let state_path = state_dir.join("state.json");
    write_state(&state_path, &initial_state("small", WORK_ITEM, WORK_ITEM));
    write_source(
        workspace_root.path(),
        "src/lib.rs",
        "pub fn v() -> u8 { 1 }\n",
    );
    write_source(
        workspace_root.path(),
        &format!("ae-sdd-doc/Story/{WORK_ITEM}.md"),
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

    // Phase 1: Seed identity in boot A (original acceptance)
    seed_identity_authority(
        persistence.as_ref(),
        workspace_root.path(),
        boot_a,
        WORK_ITEM,
    );

    // Phase 2: Daemon restart into boot B — real session.open for all four sessions
    let adapter_boot_b = {
        let persistence_port: Arc<dyn PersistencePort> = persistence.clone();
        NativeBusinessAdapter::new(
            database.clone(),
            event_store_id,
            boot_b,
            POLICY.to_owned(),
            Arc::clone(&persistence_port),
        )
    };

    let now_real = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let clock = Arc::new(TestClock::new(now_real));
    let runtime = Arc::new(RuntimeService::new(
        RuntimeConfig::default(),
        boot_b,
        "test-endpoint-token".to_owned(),
        persistence.clone(),
        clock.clone(),
        Arc::new(TestResolver),
        Arc::new(adapter_boot_b.clone()),
    ));

    // Recover workspace and session state from boot A
    runtime.recover().expect("runtime recovery succeeds");

    // Workspace was already registered in boot A, so we don't re-register in boot B

    // Reopen Root session in boot B
    let mut connection_root = ConnectionState::default();

    // Perform handshake first
    let handshake_request = serde_json::to_value(HandshakeRequest {
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        client_build: "test/review-contribution".to_owned(),
        client_kind: ClientKind::Cli,
        endpoint_token: SecretString::new("test-endpoint-token".to_owned()),
        expected_boot_id: boot_b.to_string(),
        expected_policy_digest: ae_sdd_policy::policy_digest().to_hex(),
        adapter_id: None,
    })
    .unwrap();
    let handshake_bytes = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "runtime.handshake".to_owned(),
        RpcMethod::RuntimeHandshake,
        handshake_request,
    ))
    .unwrap();
    let handshake_response = runtime.handle_payload(&mut connection_root, &handshake_bytes);
    let handshake_result: Value = serde_json::from_slice(&handshake_response).unwrap();
    assert!(
        handshake_result.get("result").is_some(),
        "handshake: {handshake_result}"
    );

    let root_reopen = {
        let mut p = params(
            json!({
                "externalKey": format!("external-{}", ROOT_SESSION),
                "role": "root",
                "engaged": true,
            }),
            30_000,
        );
        p.workspace_id = Some(WORKSPACE_ID.to_owned());
        p.agent_id = Some("root-agent".to_owned());
        p.idempotency_key = Some("reopen-root-boot-b".to_owned());
        p
    };
    let root_bytes = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "session.open".to_owned(),
        RpcMethod::SessionOpen,
        serde_json::to_value(&root_reopen).unwrap(),
    ))
    .unwrap();
    let root_response = runtime.handle_payload(&mut connection_root, &root_bytes);
    let root_result: Value = serde_json::from_slice(&root_response).unwrap();
    assert!(
        root_result.get("result").is_some(),
        "root reopen: {root_result}"
    );
    eprintln!(
        "Root session reopen response: {}",
        serde_json::to_string_pretty(&root_result).unwrap()
    );

    // Reopen Series session in boot B
    let mut connection_series = ConnectionState::default();
    let handshake_request_series = serde_json::to_value(HandshakeRequest {
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        client_build: "test/review-contribution".to_owned(),
        client_kind: ClientKind::Cli,
        endpoint_token: SecretString::new("test-endpoint-token".to_owned()),
        expected_boot_id: boot_b.to_string(),
        expected_policy_digest: ae_sdd_policy::policy_digest().to_hex(),
        adapter_id: None,
    })
    .unwrap();
    let handshake_bytes_series = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "runtime.handshake".to_owned(),
        RpcMethod::RuntimeHandshake,
        handshake_request_series,
    ))
    .unwrap();
    let _ = runtime.handle_payload(&mut connection_series, &handshake_bytes_series);

    let series_reopen = {
        let mut p = params(
            json!({
                "externalKey": format!("external-{}", SERIES_SESSION),
                "role": "series",
                "engaged": true,
                "delegationId": SERIES_DELEGATION,
            }),
            30_000,
        );
        p.workspace_id = Some(WORKSPACE_ID.to_owned());
        p.agent_id = Some("series-agent".to_owned());
        p.session_id = Some(SERIES_SESSION.to_owned());
        p.idempotency_key = Some("reopen-series-boot-b".to_owned());
        p
    };
    let series_bytes = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "session.open".to_owned(),
        RpcMethod::SessionOpen,
        serde_json::to_value(&series_reopen).unwrap(),
    ))
    .unwrap();
    let series_response = runtime.handle_payload(&mut connection_series, &series_bytes);
    let series_result: Value = serde_json::from_slice(&series_response).unwrap();
    eprintln!(
        "Series session reopen response: {}",
        serde_json::to_string_pretty(&series_result).unwrap()
    );
    assert!(
        series_result.get("result").is_some(),
        "series reopen: {series_result}"
    );

    // Reopen author Task session in boot B
    let mut connection_author = ConnectionState::default();
    let handshake_request_author = serde_json::to_value(HandshakeRequest {
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        client_build: "test/review-contribution".to_owned(),
        client_kind: ClientKind::Cli,
        endpoint_token: SecretString::new("test-endpoint-token".to_owned()),
        expected_boot_id: boot_b.to_string(),
        expected_policy_digest: ae_sdd_policy::policy_digest().to_hex(),
        adapter_id: None,
    })
    .unwrap();
    let handshake_bytes_author = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "runtime.handshake".to_owned(),
        RpcMethod::RuntimeHandshake,
        handshake_request_author,
    ))
    .unwrap();
    let _ = runtime.handle_payload(&mut connection_author, &handshake_bytes_author);

    let author_reopen = {
        let mut p = params(
            json!({
                "externalKey": format!("external-{}", AUTHOR_SESSION),
                "role": "task",
                "engaged": true,
                "delegationId": AUTHOR_DELEGATION,
            }),
            30_000,
        );
        p.workspace_id = Some(WORKSPACE_ID.to_owned());
        p.agent_id = Some("author-agent".to_owned());
        p.session_id = Some(AUTHOR_SESSION.to_owned());
        p.idempotency_key = Some("reopen-author-boot-b".to_owned());
        p
    };
    let author_bytes = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "session.open".to_owned(),
        RpcMethod::SessionOpen,
        serde_json::to_value(&author_reopen).unwrap(),
    ))
    .unwrap();
    let author_response = runtime.handle_payload(&mut connection_author, &author_bytes);
    let author_result: Value = serde_json::from_slice(&author_response).unwrap();
    assert!(
        author_result.get("result").is_some(),
        "author reopen: {author_result}"
    );

    // Reopen Reviewer session in boot B
    let mut connection_reviewer = ConnectionState::default();
    let handshake_request_reviewer = serde_json::to_value(HandshakeRequest {
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        client_build: "test/review-contribution".to_owned(),
        client_kind: ClientKind::Cli,
        endpoint_token: SecretString::new("test-endpoint-token".to_owned()),
        expected_boot_id: boot_b.to_string(),
        expected_policy_digest: ae_sdd_policy::policy_digest().to_hex(),
        adapter_id: None,
    })
    .unwrap();
    let handshake_bytes_reviewer = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "runtime.handshake".to_owned(),
        RpcMethod::RuntimeHandshake,
        handshake_request_reviewer,
    ))
    .unwrap();
    let _ = runtime.handle_payload(&mut connection_reviewer, &handshake_bytes_reviewer);

    let reviewer_reopen = {
        let mut p = params(
            json!({
                "externalKey": format!("external-{}", GENERAL_REVIEWER.session_id),
                "role": "reviewer",
                "engaged": true,
                "delegationId": GENERAL_REVIEWER.delegation_id,
            }),
            30_000,
        );
        p.workspace_id = Some(WORKSPACE_ID.to_owned());
        p.agent_id = Some(GENERAL_REVIEWER.agent_id.to_owned());
        p.session_id = Some(GENERAL_REVIEWER.session_id.to_owned());
        p.idempotency_key = Some("reopen-reviewer-boot-b".to_owned());
        p
    };
    let reviewer_bytes = serde_json::to_vec(&ae_sdd_protocol::JsonRpcRequest::new(
        "session.open".to_owned(),
        RpcMethod::SessionOpen,
        serde_json::to_value(&reviewer_reopen).unwrap(),
    ))
    .unwrap();
    let reviewer_response = runtime.handle_payload(&mut connection_reviewer, &reviewer_bytes);
    let reviewer_result: Value = serde_json::from_slice(&reviewer_response).unwrap();
    assert!(
        reviewer_result.get("result").is_some(),
        "reviewer reopen: {reviewer_result}"
    );

    // Phase 3: Attempt review.contribute in boot B with historical attestation from boot A
    let adapter = adapter_boot_b;

    let reviewer_workspace = business_workspace(
        workspace_root.path(),
        AgentRole::Reviewer,
        reviewer_domain_grant(GENERAL_REVIEWER.specialty),
    );

    // Seal evidence for the contribution - must use author workspace to record evidence first
    let author_workspace = business_workspace(
        workspace_root.path(),
        AgentRole::Task,
        ScopedGrant::new(
            [
                "document.save".parse().unwrap(),
                "evidence.record".parse().unwrap(),
                "evidence.finalize".parse().unwrap(),
                "lease.acquire".parse().unwrap(),
                "lease.release".parse().unwrap(),
            ],
            [],
            [ProjectPathScope::ProjectRoot],
        ),
    );

    // Record and finalize evidence as author (Task session)
    let state = read_state(&state_path);
    let artifact_path = "results/restart-evidence.json";
    write_source(workspace_root.path(), artifact_path, "{\"pass\":true}\n");
    let input = authoritative_review_workspace_input_fingerprint(&author_workspace, &state)
        .expect("review input fingerprint before evidence");

    let lease = adapter
        .execute(
            RpcMethod::OperationExecute,
            &operation_params(
                "author-agent",
                AUTHOR_SESSION,
                "lease.acquire",
                json!({"owner":{"role":"task"},"ttlSeconds":300}),
                "restart-evidence-lease",
            ),
            Some(&author_workspace),
        )
        .expect("author evidence lease acquires");
    let lease_id = lease["data"]["leaseId"].as_str().expect("lease id");
    let fencing_token = lease["data"]["fencingToken"]
        .as_u64()
        .expect("fencing token");

    let mut record = operation_params(
        "author-agent",
        AUTHOR_SESSION,
        "evidence.record",
        json!({
            "artifactPath":artifact_path,
            "inputFingerprint":input.to_string(),
            "kind":"focused-test",
            "command":["cargo","test"],
            "toolchainFingerprint":"rust-1",
            "exitCode":0,
            "summary":{"verification":"V-012"},
            "logicalKey":"review/restart-evidence"
        }),
        "restart-record",
    );
    record.lease_id = Some(lease_id.to_owned());
    record.fencing_token = Some(fencing_token);
    record.expected_revision = Some(7);
    let recorded = adapter
        .execute(
            RpcMethod::OperationExecute,
            &record,
            Some(&author_workspace),
        )
        .expect("Story-scoped evidence records");
    let evidence_id = recorded["data"]["evidenceId"]
        .as_str()
        .expect("evidence id")
        .to_owned();

    let revision_after_record = recorded["revisionAfter"].as_u64().expect("revisionAfter");

    let mut finalize = operation_params(
        "author-agent",
        AUTHOR_SESSION,
        "evidence.finalize",
        json!({}),
        "restart-finalize",
    );
    finalize.lease_id = Some(lease_id.to_owned());
    finalize.fencing_token = Some(fencing_token);
    finalize.expected_revision = Some(revision_after_record);
    let finalized = adapter
        .execute(
            RpcMethod::OperationExecute,
            &finalize,
            Some(&author_workspace),
        )
        .expect("Story-scoped evidence finalizes");

    let revision_after_finalize = finalized["revisionAfter"].as_u64().expect("revisionAfter");

    let mut release = operation_params(
        "author-agent",
        AUTHOR_SESSION,
        "lease.release",
        json!({"owner":{"role":"task"}}),
        "restart-release",
    );
    release.lease_id = Some(lease_id.to_owned());
    release.fencing_token = Some(fencing_token);
    adapter
        .execute(
            RpcMethod::OperationExecute,
            &release,
            Some(&author_workspace),
        )
        .expect("author evidence lease releases");

    let mut request = operation_params(
        GENERAL_REVIEWER.agent_id,
        GENERAL_REVIEWER.session_id,
        "review.contribute",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":[evidence_id]
        }),
        "restart-contribute",
    );
    request.capability_token = Some(format!(
        "attestation:{}:delegation:{}",
        boot_a, // Historical attestation from boot A
        GENERAL_REVIEWER.delegation_id
    ));
    request.expected_revision = Some(revision_after_finalize);

    // This MUST succeed: all four receipts exist in boot B, should permit boot A attestation
    let response = adapter
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&reviewer_workspace),
        )
        .expect("review.contribute must succeed after boot restart with valid receipts");

    assert_eq!(response["changed"], true, "contribution must be committed");

    // P1-4: a successful boot-B contribution must not rewrite the historical
    // attestation it relied on — reattachment happens through the separate
    // current_boot_receipt mechanism, not by mutating accepted_boot_id.
    let delegation_snapshots = persistence
        .list_identity_snapshots(RuntimeIdentityKind::Delegation)
        .expect("load delegation snapshots after contribution");
    let author_attestation = delegation_snapshots
        .iter()
        .filter_map(|snap| snap.attestation.as_ref())
        .find(|attestation| attestation.delegation_id == AUTHOR_DELEGATION)
        .expect("author delegation attestation still exists");
    assert_eq!(
        author_attestation.accepted_boot_id,
        boot_a.to_string(),
        "author's historical accepted_boot_id must remain boot A after a successful boot-B contribution"
    );
    let reviewer_attestation = delegation_snapshots
        .iter()
        .filter_map(|snap| snap.attestation.as_ref())
        .find(|attestation| attestation.delegation_id == GENERAL_REVIEWER.delegation_id)
        .expect("reviewer delegation attestation still exists");
    assert_eq!(
        reviewer_attestation.accepted_boot_id,
        boot_a.to_string(),
        "reviewer's historical accepted_boot_id must remain boot A after a successful boot-B contribution"
    );
}

/// Boot-B workspace after-image shared by the direct `boots_with_receipt_fallback`
/// tests below, mirroring `boot_restart_reattachment_permits_historical_attestation`'s
/// inline workspace but reusable across many small fixtures.
fn boot_b_workspace(fixture: &Fixture) -> RuntimeWorkspaceRecord {
    RuntimeWorkspaceRecord {
        workspace_id: WORKSPACE_ID.to_owned(),
        canonical_root: fs::canonicalize(fixture.workspace_root.path())
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned(),
        project_key: PROJECT_KEY.to_owned(),
        mode: WorkspaceMode::RustCanary,
        inventory_generation: 3,
        dirty: false,
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 2_000,
    }
}

fn series_wire_grant() -> ScopedGrantWire {
    ScopedGrantWire {
        operations: vec![
            "document.save".to_owned(),
            "lease.acquire".to_owned(),
            "review.contribute".to_owned(),
            "review.record".to_owned(),
        ],
        capabilities: vec!["review.specialty.general".to_owned()],
        paths: vec![GrantPathWire::ProjectRoot],
    }
}

fn author_wire_grant() -> ScopedGrantWire {
    ScopedGrantWire {
        operations: vec![
            "document.save".to_owned(),
            "evidence.finalize".to_owned(),
            "evidence.record".to_owned(),
            "lease.acquire".to_owned(),
            "lease.release".to_owned(),
        ],
        capabilities: Vec::new(),
        paths: vec![GrantPathWire::ProjectRoot],
    }
}

/// Writes one lineage role's boot-B `session.reopen` receipt, matching the
/// identity `seed_identity_authority` already established for that role.
/// `corrupt` runs after the valid receipt is built so field-corruption cases
/// can mutate exactly one field before it is committed.
fn write_reopen_receipt(
    persistence: &SqliteRuntimePersistence,
    workspace: &RuntimeWorkspaceRecord,
    boot_b: BootId,
    role: WireAgentRole,
    work_item_id: &str,
    key: &str,
    corrupt: impl FnOnce(&mut CurrentBootSessionReceipt),
) {
    let (session_id, agent_id, parent_session_id, delegation_id, grant) = match role {
        WireAgentRole::Root => (ROOT_SESSION, "root-agent", None, None, root_wire_grant()),
        WireAgentRole::Series => (
            SERIES_SESSION,
            "series-agent",
            Some(ROOT_SESSION),
            Some(SERIES_DELEGATION),
            series_wire_grant(),
        ),
        WireAgentRole::Task => (
            AUTHOR_SESSION,
            "author-agent",
            Some(SERIES_SESSION),
            Some(AUTHOR_DELEGATION),
            author_wire_grant(),
        ),
        WireAgentRole::Reviewer => (
            GENERAL_REVIEWER.session_id,
            GENERAL_REVIEWER.agent_id,
            Some(SERIES_SESSION),
            Some(GENERAL_REVIEWER.delegation_id),
            reviewer_wire_grant(GENERAL_REVIEWER.specialty),
        ),
    };
    let mut receipt = CurrentBootSessionReceipt {
        boot_id: boot_b.to_string(),
        workspace_id: WORKSPACE_ID.to_owned(),
        session_id: session_id.to_owned(),
        agent_id: agent_id.to_owned(),
        role,
        root_session_id: ROOT_SESSION.to_owned(),
        parent_session_id: parent_session_id.map(str::to_owned),
        delegation_id: delegation_id.map(str::to_owned),
        work_item_id: Some(work_item_id.to_owned()),
        grant: grant.clone(),
        expires_at_unix_ms: 4_102_444_800_000,
        created_at_unix_ms: 2_000,
    };
    corrupt(&mut receipt);
    commit_identity(
        persistence,
        "session.reopen",
        key,
        RuntimeIdentitySnapshot {
            identity_kind: RuntimeIdentityKind::Session,
            workspace: workspace.clone(),
            session: Some(session_record(
                session_id,
                agent_id,
                role,
                parent_session_id,
                delegation_id,
                grant,
                work_item_id,
            )),
            delegation: None,
            host_action: None,
            attestation: None,
            current_boot_receipt: Some(receipt),
            response: json!({"sessionId":session_id}),
            replayed: false,
        },
    );
}

/// RED-001 (rewritten): calls the production `boots_with_receipt_fallback`
/// directly instead of a test-file copy of its policy enum, so a regression
/// back to fail-open would actually fail this test.
///
/// P0-1 Fix verification: whichever single lineage role's boot-B receipt is
/// missing, the function must fall back to `live()`, which rejects boot A's
/// historical `accepted_boot_id` while still permitting the current boot.
#[test]
fn boots_with_receipt_fallback_rejects_when_any_role_receipt_is_missing() {
    let now_ms = 2_000u64;
    for missing in [
        WireAgentRole::Root,
        WireAgentRole::Series,
        WireAgentRole::Task,
        WireAgentRole::Reviewer,
    ] {
        let fixture = Fixture::new("small");
        let boot_b = BootId::from_uuid(Uuid::from_u128(710));
        let workspace = boot_b_workspace(&fixture);

        for role in [
            WireAgentRole::Root,
            WireAgentRole::Series,
            WireAgentRole::Task,
            WireAgentRole::Reviewer,
        ] {
            if role == missing {
                continue;
            }
            write_reopen_receipt(
                fixture.persistence.as_ref(),
                &workspace,
                boot_b,
                role,
                fixture.work_item_id,
                &format!("reopen-{role:?}-missing-{missing:?}"),
                |_| {},
            );
        }

        let boot_b_str = boot_b.to_string();
        let boots = boots_with_receipt_fallback(
            &boot_b_str,
            fixture.persistence.as_ref(),
            WORKSPACE_ID,
            SessionId::from_str(GENERAL_REVIEWER.session_id).expect("reviewer session id"),
            now_ms,
        );

        assert!(
            boots.permits(&boot_b_str),
            "current boot must always be permitted ({missing:?} missing)"
        );
        assert!(
            !boots.permits(&fixture.boot_id.to_string()),
            "missing {missing:?} receipt must fail closed and reject the historical boot"
        );
    }
}

/// AC-002 at the unit level: once all four lineage receipts are valid for
/// the current boot, boot A's historical `accepted_boot_id` must be permitted.
#[test]
fn boots_with_receipt_fallback_permits_historical_boot_when_all_four_receipts_valid() {
    let fixture = Fixture::new("small");
    let boot_b = BootId::from_uuid(Uuid::from_u128(711));
    let workspace = boot_b_workspace(&fixture);
    let now_ms = 2_000u64;

    for role in [
        WireAgentRole::Root,
        WireAgentRole::Series,
        WireAgentRole::Task,
        WireAgentRole::Reviewer,
    ] {
        write_reopen_receipt(
            fixture.persistence.as_ref(),
            &workspace,
            boot_b,
            role,
            fixture.work_item_id,
            &format!("reopen-{role:?}-all-valid"),
            |_| {},
        );
    }

    let boot_b_str = boot_b.to_string();
    let boots = boots_with_receipt_fallback(
        &boot_b_str,
        fixture.persistence.as_ref(),
        WORKSPACE_ID,
        SessionId::from_str(GENERAL_REVIEWER.session_id).expect("reviewer session id"),
        now_ms,
    );

    assert!(boots.permits(&boot_b_str), "current boot must be permitted");
    assert!(
        boots.permits(&fixture.boot_id.to_string()),
        "all four valid receipts must permit the historical boot A attestation"
    );
}

/// AC-003 field matrix: `verify_receipt_match` (review_authority.rs:1953) is
/// one shared function called identically for all four lineage roles, so
/// exercising its per-field rejection once — on the Reviewer receipt — covers
/// the matching logic for every role. Per-role wiring is already covered by
/// `boots_with_receipt_fallback_rejects_when_any_role_receipt_is_missing`.
#[test]
fn boots_with_receipt_fallback_rejects_corrupted_reviewer_receipt_fields() {
    let cases: Vec<(&str, fn(&mut CurrentBootSessionReceipt))> = vec![
        ("boot_id", |r| {
            r.boot_id = BootId::from_uuid(Uuid::from_u128(999)).to_string();
        }),
        ("workspace_id", |r| {
            r.workspace_id = "00000000-0000-0000-0000-0000000000ff".to_owned();
        }),
        ("session_id", |r| {
            r.session_id = "00000000-0000-0000-0000-0000000000ee".to_owned();
        }),
        ("agent_id", |r| {
            r.agent_id = "wrong-agent".to_owned();
        }),
        ("role", |r| {
            r.role = WireAgentRole::Task;
        }),
        ("root_session_id", |r| {
            r.root_session_id = "wrong-root".to_owned();
        }),
        ("parent_session_id", |r| {
            r.parent_session_id = Some(ROOT_SESSION.to_owned());
        }),
        ("delegation_id", |r| {
            r.delegation_id = Some("wrong-delegation".to_owned());
        }),
        ("work_item_id", |r| {
            r.work_item_id = Some("wrong-work-item".to_owned());
        }),
        ("grant", |r| {
            r.grant.operations.push("extra.operation".to_owned());
        }),
        ("expires_at_unix_ms", |r| {
            r.expires_at_unix_ms = 2_000;
        }),
    ];

    for (label, corrupt) in cases {
        let fixture = Fixture::new("small");
        let boot_b = BootId::from_uuid(Uuid::from_u128(712));
        let workspace = boot_b_workspace(&fixture);

        for role in [
            WireAgentRole::Root,
            WireAgentRole::Series,
            WireAgentRole::Task,
        ] {
            write_reopen_receipt(
                fixture.persistence.as_ref(),
                &workspace,
                boot_b,
                role,
                fixture.work_item_id,
                &format!("reopen-{role:?}-{label}"),
                |_| {},
            );
        }
        write_reopen_receipt(
            fixture.persistence.as_ref(),
            &workspace,
            boot_b,
            WireAgentRole::Reviewer,
            fixture.work_item_id,
            &format!("reopen-reviewer-{label}"),
            corrupt,
        );

        let boot_b_str = boot_b.to_string();
        let boots = boots_with_receipt_fallback(
            &boot_b_str,
            fixture.persistence.as_ref(),
            WORKSPACE_ID,
            SessionId::from_str(GENERAL_REVIEWER.session_id).expect("reviewer session id"),
            2_000,
        );

        assert!(
            !boots.permits(&fixture.boot_id.to_string()),
            "corrupted reviewer receipt field `{label}` must reject the historical boot"
        );
    }
}

/// RED-002 (rewritten): P1-3 — exactly-one-Task enforcement lives inside
/// `boots_with_receipt_fallback` itself (review_authority.rs:1905-1919).
/// `bind_reviewer` has its own, separate exactly-one-*session* check
/// (review_authority.rs:2113-2130) that would reject two Task sessions for
/// an unrelated reason, so a full `review.contribute` RPC call cannot
/// isolate this specific fix — it would pass or fail regardless of whether
/// this fix regressed. Calling the production function directly is the only
/// way to prove it.
#[test]
fn boots_with_receipt_fallback_rejects_ambiguous_task_delegation_under_series() {
    let fixture = Fixture::new("small");
    let boot_b = BootId::from_uuid(Uuid::from_u128(713));
    let workspace = boot_b_workspace(&fixture);
    let now_ms = 2_000u64;

    // A second, independently valid Task delegation under the same Series.
    commit_child_identity(
        fixture.persistence.as_ref(),
        &workspace,
        fixture.boot_id,
        fixture.work_item_id,
        ChildIdentity {
            session_id: "00000000-0000-0000-0000-000000000031",
            agent_id: "author-agent-2",
            delegation_id: "10000000-0000-0000-0000-000000000031",
            parent_session_id: SERIES_SESSION,
            parent_delegation_id: Some(SERIES_DELEGATION),
            role: WireAgentRole::Task,
            grant: author_wire_grant(),
            sequence: 6,
        },
    );

    for role in [
        WireAgentRole::Root,
        WireAgentRole::Series,
        WireAgentRole::Task,
        WireAgentRole::Reviewer,
    ] {
        write_reopen_receipt(
            fixture.persistence.as_ref(),
            &workspace,
            boot_b,
            role,
            fixture.work_item_id,
            &format!("reopen-{role:?}-ambiguous"),
            |_| {},
        );
    }

    let boot_b_str = boot_b.to_string();
    let boots = boots_with_receipt_fallback(
        &boot_b_str,
        fixture.persistence.as_ref(),
        WORKSPACE_ID,
        SessionId::from_str(GENERAL_REVIEWER.session_id).expect("reviewer session id"),
        now_ms,
    );

    assert!(
        boots.permits(&boot_b_str),
        "current boot must still be permitted"
    );
    assert!(
        !boots.permits(&fixture.boot_id.to_string()),
        "two Task delegations under the same Series must fail closed, not silently pick one"
    );
}

/// Full-stack complement to the direct unit tests above: with zero boot-B
/// receipts written at all, the real `review.contribute` RPC path — caller
/// authentication, `boots_with_receipt_fallback`, and `bind_reviewer` wired
/// together exactly as production calls them — must reject rather than
/// silently falling back to permissive semantics anywhere in that chain.
#[test]
fn review_contribute_rejects_when_boot_b_has_no_reopen_receipts() {
    let fixture = Fixture::new("small");
    let boot_b = BootId::from_uuid(Uuid::from_u128(714));

    let adapter = {
        let persistence_port: Arc<dyn PersistencePort> = fixture.persistence.clone();
        NativeBusinessAdapter::new(
            fixture.database.clone(),
            fixture.event_store_id,
            boot_b,
            POLICY.to_owned(),
            persistence_port,
        )
    };

    let mut request = operation_params_for_work_item(
        fixture.work_item_id,
        GENERAL_REVIEWER.agent_id,
        GENERAL_REVIEWER.session_id,
        "review.contribute",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":[]
        }),
        "no-receipts-contribute",
    );
    request.expected_revision = Some(fixture.revision());

    let error = adapter
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect_err("review.contribute must reject boot B with no reopen receipts at all");
    assert_eq!(
        error.code(),
        StableErrorCode::DelegationAttestationFailed,
        "{error:?}"
    );
}

#[test]
fn boot_restart_reattachment_rejects_work_item_mismatch() {
    let fixture = Fixture::new("small");
    let boot_b = BootId::from_uuid(Uuid::from_u128(815));
    let workspace = RuntimeWorkspaceRecord {
        workspace_id: WORKSPACE_ID.to_owned(),
        canonical_root: fs::canonicalize(fixture.workspace_root.path())
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned(),
        project_key: PROJECT_KEY.to_owned(),
        mode: WorkspaceMode::RustCanary,
        inventory_generation: 3,
        dirty: false,
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 2_000,
    };

    let wrong_work_item = "ROUTE-00000000-0000-0000-0000-000000009999";

    // Create state file for wrong_work_item so read_state can find it
    let wrong_state_dir = fixture
        .workspace_root
        .path()
        .join(".auto-engineering/route-wrong");
    fs::create_dir_all(&wrong_state_dir).expect("wrong state directory");
    write_state(
        &wrong_state_dir.join("state.json"),
        &initial_state("small", wrong_work_item, wrong_work_item),
    );

    seed_identity_authority(
        fixture.persistence.as_ref(),
        fixture.workspace_root.path(),
        fixture.boot_id,
        fixture.work_item_id,
    );

    // Reopen all sessions in boot_b. Create Reviewer session.current_work_item
    // mismatch: delegation.work_item_id (from boot_a seed) = fixture.work_item_id,
    // but session.current_work_item (from boot_b reopen) = wrong_work_item.
    // Root, Series, Task remain correct. This proves validate_child_authority
    // detects Reviewer delegation.work_item_id != session.current_work_item.
    write_reopen_receipt(
        fixture.persistence.as_ref(),
        &workspace,
        boot_b,
        WireAgentRole::Root,
        fixture.work_item_id,
        "reopen-root-correct",
        |_| {},
    );

    write_reopen_receipt(
        fixture.persistence.as_ref(),
        &workspace,
        boot_b,
        WireAgentRole::Series,
        fixture.work_item_id,
        "reopen-series-correct",
        |_| {},
    );

    write_reopen_receipt(
        fixture.persistence.as_ref(),
        &workspace,
        boot_b,
        WireAgentRole::Task,
        fixture.work_item_id,
        "reopen-task-correct",
        |_| {},
    );

    write_reopen_receipt(
        fixture.persistence.as_ref(),
        &workspace,
        boot_b,
        WireAgentRole::Reviewer,
        wrong_work_item,
        "reopen-reviewer-mismatch",
        |_| {},
    );

    // Execute review.contribute through real RPC path
    let adapter = {
        let persistence_port: Arc<dyn PersistencePort> = fixture.persistence.clone();
        NativeBusinessAdapter::new(
            fixture.database.clone(),
            fixture.event_store_id,
            boot_b,
            POLICY.to_owned(),
            persistence_port,
        )
    };

    let mut request = operation_params_for_work_item(
        wrong_work_item,
        GENERAL_REVIEWER.agent_id,
        GENERAL_REVIEWER.session_id,
        "review.contribute",
        json!({
            "status":"passed",
            "findings":[],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":[]
        }),
        "reviewer-delegation-work-item-mismatch",
    );
    request.expected_revision = Some(fixture.revision());

    let error = adapter
        .execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&fixture.reviewer_workspace(&GENERAL_REVIEWER)),
        )
        .expect_err("review.contribute must reject Reviewer delegation/session work_item mismatch");
    assert_eq!(
        error.code(),
        StableErrorCode::DelegationAttestationFailed,
        "Reviewer delegation.work_item_id != session.current_work_item must fail: {error:?}"
    );
}
