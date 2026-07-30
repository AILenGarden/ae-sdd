//! P1 incremental governance end-to-end coverage (V-EFF-012b).
//!
//! One approved execution plan carries three slices to `Completed` through the
//! real typed operations: each slice only writes its changed-path source file
//! and re-evaluates the full Gate set through the long-lived authoritative
//! scheduler, a green verification record closes `ImplementationVerified`,
//! evidence finalization closes `ReviewReady`, two reviewers contribute and
//! the root finalizer aggregates them into `GovernanceClosed`, and the
//! terminal `workitem.complete` commits `Completed`. The incremental Gate DAG
//! is asserted on the scheduler counters: slices two and three re-evaluate
//! exactly the changed-path-affected Gates and never the RA/Story/CodingPlan
//! nodes. A changed-path edit after the Review rolls the milestone back, so
//! the terminal transition is denied.

use std::{fs, path::Path, sync::Arc};

use ae_sdd_domain::{AgentRole, BootId, CapabilityId, OperationId, ProjectPathScope, ScopedGrant};
use ae_sdd_gates::{GateRegistry, GateSchedulerStats};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_protocol::{
    ClientKind, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, PersistencePort, RuntimeDelegationAttestationRecord,
    RuntimeDelegationHostActionRecord, RuntimeDelegationRecord, RuntimeIdentityKind,
    RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeSessionRecord,
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

const WORK_ITEM: &str = "PRD-E2E-COMPLETION";
const STORY: &str = "STORY-E2E-COMPLETION";
const PROJECT_KEY: &str = "e2e-completion";
const POLICY: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const STORY_DOC: &str = "ae-sdd-doc/Story/prd-e2e-completion.md";
const SLICES: [&str; 3] = ["src/alpha.rs", "src/beta.rs", "src/gamma.rs"];
/// Gates whose declared selectors include `ChangedPaths`; exactly these may
/// re-evaluate when a slice writes its source file.
const CHANGED_PATH_GATES: [&str; 4] = ["G-09", "G-11", "G-CODE-1", "G-CODEPLAN-SRC"];

const WORKSPACE_ID: &str = "00000000-0000-0000-0000-0000000000e2";
const ROOT_SESSION: &str = "00000000-0000-0000-0000-00000000e201";
const SERIES_SESSION: &str = "00000000-0000-0000-0000-00000000e202";
const AUTHOR_SESSION: &str = "00000000-0000-0000-0000-00000000e203";
const SERIES_DELEGATION: &str = "10000000-0000-0000-0000-00000000e202";
const AUTHOR_DELEGATION: &str = "10000000-0000-0000-0000-00000000e203";

struct Reviewer {
    session_id: &'static str,
    delegation_id: &'static str,
    agent_id: &'static str,
    specialty: &'static str,
    sequence: u64,
}

const BE_REVIEWER: Reviewer = Reviewer {
    session_id: "00000000-0000-0000-0000-00000000e204",
    delegation_id: "10000000-0000-0000-0000-00000000e204",
    agent_id: "reviewer-be",
    specialty: "be",
    sequence: 4,
};
const AR_REVIEWER: Reviewer = Reviewer {
    session_id: "00000000-0000-0000-0000-00000000e205",
    delegation_id: "10000000-0000-0000-0000-00000000e205",
    agent_id: "reviewer-ar",
    specialty: "ar",
    sequence: 5,
};

struct Fixture {
    workspace_root: TempDir,
    _runtime_root: TempDir,
    state_path: std::path::PathBuf,
    adapter: NativeBusinessAdapter,
}

impl Fixture {
    fn new() -> Self {
        let workspace_root = TempDir::new().expect("workspace tempdir");
        let state_dir = workspace_root
            .path()
            .join(".auto-engineering/e2e-completion");
        fs::create_dir_all(&state_dir).expect("state directory");
        let state_path = state_dir.join("state.json");
        write_workspace(workspace_root.path(), &state_path);

        let runtime_root = TempDir::new().expect("runtime tempdir");
        let database = runtime_root.path().join("runtime.sqlite3");
        let persistence =
            Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence opens"));
        let event_store_id = persistence.event_store_id().expect("event store identity");
        let boot_id = BootId::from_uuid(Uuid::from_u128(702));
        seed_identity_authority(persistence.as_ref(), workspace_root.path(), boot_id);
        let adapter = NativeBusinessAdapter::new(
            database,
            event_store_id,
            boot_id,
            POLICY.to_owned(),
            persistence,
        );
        Self {
            workspace_root,
            _runtime_root: runtime_root,
            state_path,
            adapter,
        }
    }

    fn root_workspace(&self) -> BusinessWorkspace {
        business_workspace(self.workspace_root.path(), AgentRole::Root, root_grant())
    }

    /// The delegated author task executes the semantic evidence operations the
    /// root orchestrator may no longer run itself.
    fn author_workspace(&self) -> BusinessWorkspace {
        business_workspace(self.workspace_root.path(), AgentRole::Task, author_grant())
    }

    fn reviewer_workspace(&self, reviewer: &Reviewer) -> BusinessWorkspace {
        business_workspace(
            self.workspace_root.path(),
            AgentRole::Reviewer,
            reviewer_domain_grant(reviewer.specialty),
        )
    }

    fn revision(&self) -> u64 {
        read_state(&self.state_path)["revision"]
            .as_u64()
            .expect("state revision")
    }

    fn milestone(&self) -> String {
        read_state(&self.state_path)
            .pointer("/executionRuntime/completionMilestone")
            .and_then(Value::as_str)
            .expect("recorded completion milestone")
            .to_owned()
    }

    /// Evaluates `gate_ids` in one batch through the root session and returns
    /// the cumulative scheduler counters of the long-lived runtime.
    fn gate_sweep(&self, gate_ids: &[String], key: &str) -> GateSchedulerStats {
        let mut params = root_params(&json!({"gateIds": gate_ids}), key);
        params.idempotency_key = Some(key.to_owned());
        let result = self
            .adapter
            .execute(
                RpcMethod::GateEvaluate,
                &params,
                Some(&self.root_workspace()),
            )
            .unwrap_or_else(|error| panic!("{key} sweep fails: {error:?}"));
        let results = result["results"].as_array().expect("gate result array");
        let scheduler = results
            .last()
            .and_then(|item| item.get("scheduler"))
            .unwrap_or_else(|| panic!("{key} sweep carries scheduler stats: {result}"));
        GateSchedulerStats {
            cache_hits: scheduler["cacheHits"].as_u64().expect("cache hits"),
            cache_misses: scheduler["cacheMisses"].as_u64().expect("cache misses"),
            gates_evaluated: scheduler["gatesEvaluated"]
                .as_u64()
                .expect("gates evaluated"),
        }
    }

    fn next_action(&self, key: &str) -> Value {
        let result = self
            .adapter
            .execute(
                RpcMethod::FlowSnapshot,
                &root_params(&json!({}), key),
                Some(&self.root_workspace()),
            )
            .unwrap_or_else(|error| panic!("{key} flow snapshot fails: {error:?}"));
        result["nextAction"].clone()
    }

    fn operate(
        &self,
        workspace: &BusinessWorkspace,
        agent: &str,
        session: &str,
        operation: &str,
        payload: Value,
        key: &str,
    ) -> Result<Value, ae_sdd_runtime::RuntimeError> {
        let mut params = operation_params(agent, session, operation, payload, key);
        params.expected_revision = Some(self.revision());
        self.adapter
            .execute(RpcMethod::OperationExecute, &params, Some(workspace))
    }
}

fn all_gate_ids() -> Vec<String> {
    GateRegistry::all()
        .iter()
        .map(|gate| gate.id.to_owned())
        .collect()
}

fn unaffected_gate_ids() -> Vec<String> {
    GateRegistry::all()
        .iter()
        .map(|gate| gate.id)
        .filter(|id| !CHANGED_PATH_GATES.contains(id))
        .map(str::to_owned)
        .collect()
}

fn changed_path_gate_ids() -> Vec<String> {
    CHANGED_PATH_GATES.map(str::to_owned).to_vec()
}

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
        "goal":"drive three slices to governance-closed completion",
        "changedPaths":SLICES,
        "verification":verification,
        "risks":["fixture risk"],
        "approved":true,
        "approvedAt":"2026-07-27T00:00:00Z",
        "approvedBy":"user:test",
        "sourceReads":[
            "ae-sdd-doc/RA/ra-e2e-completion.md",
            "ae-sdd-doc/DR/e2e-completion.md",
            STORY_DOC
        ]
    })
}

fn initial_state() -> Value {
    json!({
        "stateMachineName":WORK_ITEM,
        "activeStory":WORK_ITEM,
        "revision":1,
        "lastFencingToken":0,
        "scale":"medium",
        "selectedDesign":"Story",
        "phase":"code-reviewed",
        "currentPhase":"code-reviewed",
        "currentStep":"code-reviewed",
        "executionPlan":execution_plan(),
        "executionRuntime":{
            "schemaVersion":1,
            "queueRef":format!(".auto-engineering/{STORY}/execution/queue.json"),
            "queueDigest":format!("sha256:{}", digest("e2e-queue")),
            "activeSliceOrdinal":0,
            "completionMilestone":"none"
        },
        "prdCompletion":{
            "dependenciesSatisfied":true,
            "residualRisksCleared":true,
            "gatesPassed":true,
            "reviewPassed":true
        },
        "documentPaths":{"story":STORY_DOC},
        "storyStates":{
            STORY:{
                "phase":"completed",
                "currentPhase":"completed",
                "currentStep":"completed",
                "completedSteps":["code-reviewed"],
                "pendingOutputs":{},
                "codingRound":1,
                "docPath":STORY_DOC
            }
        },
        "evidenceRefs":[{
            "evidenceId":"e2e-prd-evidence",
            "verificationId":"V-001",
            "path":".ae-sdd/evidence/e2e-completion.json",
            "digest":digest("e2e-prd-evidence"),
            "byteLength":1
        }]
    })
}

fn write_workspace(root: &Path, state_path: &Path) {
    fs::create_dir_all(root.join("src")).expect("src directory");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("Cargo.toml");
    let constraints = root.join("constraints");
    fs::create_dir_all(&constraints).expect("constraints directory");
    for index in 0..5 {
        fs::write(
            constraints.join(format!("constraint-{index}.md")),
            "# constraint\n",
        )
        .expect("constraint file");
    }
    for (relative, body) in [
        (
            STORY_DOC,
            "# Story\n\nAC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7\nAC-8 AC-9 AC-10 AC-11 AC-12 AC-13 AC-14\n\n## verification\n",
        ),
        (
            "ae-sdd-doc/RA/ra-e2e-completion.md",
            "# RA\n\nformal RA fixture\n",
        ),
        ("ae-sdd-doc/DR/e2e-completion.md", "# DR\n"),
        (
            "tests/slice_test.rs",
            "#[test]\nfn slice_test() { assert_eq!(1, 1); }\n",
        ),
        (
            "results/focused.json",
            "{\"suite\":\"focused\",\"pass\":true}\n",
        ),
    ] {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("document parent")).expect("document directory");
        fs::write(path, body).expect("document file");
    }
    fs::write(
        state_path,
        serde_json::to_vec_pretty(&initial_state()).expect("state serializes"),
    )
    .expect("initial state");
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
        ["lease.acquire", "lease.release", "review.contribute"]
            .into_iter()
            .map(|value| OperationId::new(value).expect("operation id")),
        [
            CapabilityId::new(format!("review.specialty.{specialty}"))
                .expect("specialty capability"),
        ],
        [ProjectPathScope::ProjectRoot],
    )
}

fn root_grant() -> ScopedGrant {
    ScopedGrant::new(
        ae_sdd_operations::OperationName::ALL
            .into_iter()
            .filter(|operation| *operation != ae_sdd_operations::OperationName::LeaseBreak)
            .map(|operation| OperationId::new(operation.as_str()).expect("operation id")),
        ["general", "be", "ar", "qa"].into_iter().map(|specialty| {
            CapabilityId::new(format!("review.specialty.{specialty}"))
                .expect("specialty capability")
        }),
        [ProjectPathScope::ProjectRoot],
    )
}

fn author_grant() -> ScopedGrant {
    ScopedGrant::new(
        [
            "document.save",
            "evidence.finalize",
            "evidence.record",
            "lease.acquire",
            "lease.release",
        ]
        .into_iter()
        .map(|operation| OperationId::new(operation).expect("operation id")),
        [],
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
                ScopedGrantWire::from_domain(&root_grant()),
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
                    "evidence.finalize".to_owned(),
                    "evidence.record".to_owned(),
                    "lease.acquire".to_owned(),
                    "lease.release".to_owned(),
                    "review.contribute".to_owned(),
                ],
                capabilities: vec![
                    "review.specialty.be".to_owned(),
                    "review.specialty.ar".to_owned(),
                    "review.specialty.qa".to_owned(),
                ],
                paths: vec![ae_sdd_runtime::GrantPathWire::ProjectRoot],
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
                operations: vec![
                    "document.save".to_owned(),
                    "evidence.finalize".to_owned(),
                    "evidence.record".to_owned(),
                    "lease.acquire".to_owned(),
                    "lease.release".to_owned(),
                ],
                capabilities: Vec::new(),
                paths: vec![ae_sdd_runtime::GrantPathWire::ProjectRoot],
            },
            sequence: 2,
        },
    );
    for reviewer in [&BE_REVIEWER, &AR_REVIEWER] {
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
    let adapter_id = "e2e-completion-adapter";
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
                input_revision: 1,
                input_fingerprint: digest("e2e-input"),
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

fn base_params(agent: &str, session: &str, payload: Value, key: &str) -> RequestParams<Value> {
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
        payload,
    }
}

fn operation_params(
    agent: &str,
    session: &str,
    operation: &str,
    payload: Value,
    key: &str,
) -> RequestParams<Value> {
    base_params(
        agent,
        session,
        json!({"operation":operation,"payload":payload}),
        key,
    )
}

fn root_params(payload: &Value, key: &str) -> RequestParams<Value> {
    base_params("root-agent", ROOT_SESSION, payload.clone(), key)
}

fn acquire_lease(
    fixture: &Fixture,
    workspace: &BusinessWorkspace,
    agent: &str,
    session: &str,
    role_label: &str,
    key: &str,
) -> (String, u64) {
    let lease = fixture
        .operate(
            workspace,
            agent,
            session,
            "lease.acquire",
            json!({"owner":{"role":role_label},"ttlSeconds":300}),
            key,
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

fn release_lease(
    fixture: &Fixture,
    workspace: &BusinessWorkspace,
    agent: &str,
    session: &str,
    role_label: &str,
    lease: &(String, u64),
    key: &str,
) {
    let mut params = operation_params(
        agent,
        session,
        "lease.release",
        json!({"owner":{"role":role_label}}),
        key,
    );
    params.lease_id = Some(lease.0.clone());
    params.fencing_token = Some(lease.1);
    params.expected_revision = Some(fixture.revision());
    fixture
        .adapter
        .execute(RpcMethod::OperationExecute, &params, Some(workspace))
        .unwrap_or_else(|error| panic!("{key} release failed: {error:?}"));
}

fn read_state(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("state reads")).expect("state JSON")
}

fn digest(value: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(value.as_ref()))
}

fn write_source(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
    fs::write(path, content).expect("source file");
}

/// Drives the milestone chain from `None` to `GovernanceClosed` through the
/// real operations: the delegated author task records and finalizes the green
/// verification evidence, two reviewers contribute and the root finalizer
/// aggregates them into `GovernanceClosed`.
fn drive_to_governance_closed(fixture: &Fixture, key_prefix: &str) {
    let author = fixture.author_workspace();
    let review_input_fingerprint = authoritative_review_workspace_input_fingerprint(
        &fixture.reviewer_workspace(&BE_REVIEWER),
        &read_state(&fixture.state_path),
    )
    .expect("authoritative review input fingerprint")
    .to_string();
    let lease = acquire_lease(
        fixture,
        &author,
        "author-agent",
        AUTHOR_SESSION,
        "task",
        &format!("{key_prefix}-evidence-lease"),
    );
    let mut record = operation_params(
        "author-agent",
        AUTHOR_SESSION,
        "evidence.record",
        json!({
            "artifactPath":"results/focused.json",
            "inputFingerprint":review_input_fingerprint.clone(),
            "kind":"focused-test",
            "command":["cargo","test","-p","slice"],
            "exitCode":0
        }),
        &format!("{key_prefix}-evidence-record"),
    );
    record.lease_id = Some(lease.0.clone());
    record.fencing_token = Some(lease.1);
    record.expected_revision = Some(fixture.revision());
    let recorded = fixture
        .adapter
        .execute(RpcMethod::OperationExecute, &record, Some(&author))
        .unwrap_or_else(|error| panic!("{key_prefix} evidence record failed: {error:?}"));
    assert_eq!(recorded["changed"], true, "{recorded}");
    let evidence_id = recorded["data"]["evidenceId"]
        .as_str()
        .expect("recorded evidence id")
        .to_owned();
    assert_eq!(
        fixture.milestone(),
        "implementation-verified",
        "a green verification record must close ImplementationVerified"
    );
    assert_eq!(
        fixture.next_action(&format!("{key_prefix}-flow-verified"))["kind"],
        "finalize-execution-evidence"
    );

    let mut second_record = operation_params(
        "author-agent",
        AUTHOR_SESSION,
        "evidence.record",
        json!({
            "artifactPath":"results/focused.json",
            "inputFingerprint":review_input_fingerprint,
            "kind":"focused-test",
            "command":["cargo","test","-p","slice"],
            "exitCode":0,
            "summary":{"sequence":2}
        }),
        &format!("{key_prefix}-evidence-record-2"),
    );
    second_record.lease_id = Some(lease.0.clone());
    second_record.fencing_token = Some(lease.1);
    second_record.expected_revision = Some(fixture.revision());
    fixture
        .adapter
        .execute(RpcMethod::OperationExecute, &second_record, Some(&author))
        .unwrap_or_else(|error| panic!("{key_prefix} second evidence record failed: {error:?}"));
    assert_eq!(
        fixture.milestone(),
        "implementation-verified",
        "every green evidence append must rebind ImplementationVerified to the latest manifest"
    );

    let mut finalize = operation_params(
        "author-agent",
        AUTHOR_SESSION,
        "evidence.finalize",
        json!({}),
        &format!("{key_prefix}-evidence-finalize"),
    );
    finalize.lease_id = Some(lease.0.clone());
    finalize.fencing_token = Some(lease.1);
    finalize.expected_revision = Some(fixture.revision());
    let finalized = fixture
        .adapter
        .execute(RpcMethod::OperationExecute, &finalize, Some(&author))
        .unwrap_or_else(|error| panic!("{key_prefix} evidence finalize failed: {error:?}"));
    assert_eq!(finalized["changed"], true, "{finalized}");
    assert_eq!(
        fixture.milestone(),
        "review-ready",
        "evidence finalization must close ReviewReady"
    );
    assert_eq!(
        fixture.next_action(&format!("{key_prefix}-flow-ready"))["kind"],
        "collect-review-contributions"
    );
    release_lease(
        fixture,
        &author,
        "author-agent",
        AUTHOR_SESSION,
        "task",
        &lease,
        &format!("{key_prefix}-evidence-release"),
    );

    for reviewer in [&BE_REVIEWER, &AR_REVIEWER] {
        let contributed = fixture
            .operate(
                &fixture.reviewer_workspace(reviewer),
                reviewer.agent_id,
                reviewer.session_id,
                "review.contribute",
                json!({
                    "status":"passed",
                    "findings":[],
                    "reviewedPaths":SLICES,
                    "evidenceIds":[evidence_id]
                }),
                &format!("{key_prefix}-contribute-{}", reviewer.specialty),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{key_prefix} contribute {} failed: {error:?}",
                    reviewer.specialty
                )
            });
        assert_eq!(contributed["changed"], true, "{contributed}");
        assert_eq!(contributed["data"]["status"], "pending", "{contributed}");
    }

    let lease = acquire_lease(
        fixture,
        &fixture.root_workspace(),
        "root-agent",
        ROOT_SESSION,
        "root",
        &format!("{key_prefix}-finalize-lease"),
    );
    let mut finalize = operation_params(
        "root-agent",
        ROOT_SESSION,
        "review.finalize",
        json!({}),
        &format!("{key_prefix}-review-finalize"),
    );
    finalize.lease_id = Some(lease.0.clone());
    finalize.fencing_token = Some(lease.1);
    finalize.expected_revision = Some(fixture.revision());
    let finalized = fixture
        .adapter
        .execute(
            RpcMethod::OperationExecute,
            &finalize,
            Some(&fixture.root_workspace()),
        )
        .unwrap_or_else(|error| panic!("{key_prefix} review finalize failed: {error:?}"));
    assert_eq!(finalized["changed"], true, "{finalized}");
    let state = read_state(&fixture.state_path);
    assert_eq!(
        state["review"]["batch"]["latestStatus"], "VALID_CLEAN",
        "{state}"
    );
    assert_eq!(state["reviewSession"]["status"], "completed", "{state}");
    assert_eq!(
        fixture.milestone(),
        "governance-closed",
        "a terminal clean Review must close GovernanceClosed"
    );
    release_lease(
        fixture,
        &fixture.root_workspace(),
        "root-agent",
        ROOT_SESSION,
        "root",
        &lease,
        &format!("{key_prefix}-finalize-release"),
    );
}

/// Records the root completion intent and evaluates the three terminal Gates
/// so the flow decision reaches `ApplyTransition`.
fn record_completion_intent_and_gates(fixture: &Fixture, key_prefix: &str) {
    let mut intent = root_params(&json!({"targetPhase":"completed"}), key_prefix);
    intent.idempotency_key = Some(format!("{key_prefix}-completion-intent"));
    let decision = fixture
        .adapter
        .execute(
            RpcMethod::FlowNext,
            &intent,
            Some(&fixture.root_workspace()),
        )
        .unwrap_or_else(|error| panic!("{key_prefix} completion intent fails: {error:?}"));
    assert_eq!(
        decision["nextAction"]["kind"], "evaluate-gates",
        "GovernanceClosed must open the terminal Gate evaluation: {decision}"
    );
    assert_eq!(
        decision["nextAction"]["requiredGates"],
        json!(["G-00", "G-12", "G-13"]),
        "{decision}"
    );

    for gate_id in ["G-00", "G-12", "G-13"] {
        let mut gate = root_params(&json!({"gateId":gate_id}), key_prefix);
        gate.idempotency_key = Some(format!("{key_prefix}-gate-{gate_id}"));
        let result = fixture
            .adapter
            .execute(
                RpcMethod::GateEvaluate,
                &gate,
                Some(&fixture.root_workspace()),
            )
            .unwrap_or_else(|error| panic!("{key_prefix} {gate_id} evaluation fails: {error:?}"));
        assert_eq!(result["outcome"]["kind"], "PASS", "{gate_id}: {result}");
    }
    let ready = fixture.next_action(&format!("{key_prefix}-flow-apply"));
    assert_eq!(ready["kind"], "apply-transition", "{ready}");
}

/// Step 1 + 2: three slices reuse the long-lived scheduler so only the
/// changed-path-affected Gates re-evaluate, then the milestone chain drives
/// evidence finalize, Review contributions and governance close into a real
/// `Completed` commit.
#[test]
fn three_slices_reach_completed_through_incremental_governance() {
    let fixture = Fixture::new();

    // Baseline: every one of the 36 registered Gates evaluates once.
    let baseline = fixture.gate_sweep(&all_gate_ids(), "sweep-baseline");
    assert_eq!(baseline.gates_evaluated, 36, "{baseline:?}");

    // Slice 1 writes its changed path; only the ChangedPaths Gates re-run.
    write_source(
        fixture.workspace_root.path(),
        SLICES[0],
        "pub fn alpha() -> u8 { 1 }\n",
    );
    let untouched = fixture.gate_sweep(&unaffected_gate_ids(), "sweep-slice-1-plan");
    assert_eq!(
        untouched.gates_evaluated, baseline.gates_evaluated,
        "slice 1 must not re-run RA/Story/CodingPlan or any unaffected Gate: {untouched:?}"
    );
    let affected = fixture.gate_sweep(&changed_path_gate_ids(), "sweep-slice-1-affected");
    assert_eq!(
        affected.gates_evaluated,
        baseline.gates_evaluated + 4,
        "slice 1 re-runs exactly the changed-path-affected Gates: {affected:?}"
    );

    // Slices 2 and 3 repeat the same incremental behavior.
    let mut evaluated = affected.gates_evaluated;
    for (ordinal, path) in [(2, SLICES[1]), (3, SLICES[2])] {
        write_source(
            fixture.workspace_root.path(),
            path,
            &format!("pub fn slice_{ordinal}() -> u8 {{ {ordinal} }}\n"),
        );
        let before = fixture.gate_sweep(
            &unaffected_gate_ids(),
            &format!("sweep-slice-{ordinal}-plan"),
        );
        assert_eq!(
            before.gates_evaluated, evaluated,
            "slice {ordinal} must not re-run RA/Story/CodingPlan or any unaffected Gate: {before:?}"
        );
        let after = fixture.gate_sweep(
            &changed_path_gate_ids(),
            &format!("sweep-slice-{ordinal}-affected"),
        );
        assert_eq!(
            after.gates_evaluated,
            evaluated + 4,
            "slice {ordinal} re-runs exactly the changed-path-affected Gates: {after:?}"
        );
        evaluated = after.gates_evaluated;
    }

    drive_to_governance_closed(&fixture, "chain");
    record_completion_intent_and_gates(&fixture, "chain");

    let completion = complete_work_item(&fixture, "chain");
    assert_eq!(completion.result["changed"], true, "{completion:?}");
    let state = read_state(&fixture.state_path);
    assert_eq!(state["phase"], "completed", "{state}");
    assert_eq!(state["currentPhase"], "completed", "{state}");

    // The same-key replay returns the committed receipt without a second
    // mutation, exactly like the lifecycle control-plane replay.
    let mut replay = operation_params(
        "root-agent",
        ROOT_SESSION,
        "workitem.complete",
        json!({}),
        "chain-complete",
    );
    replay.lease_id = Some(completion.lease_id.clone());
    replay.fencing_token = Some(completion.fencing_token);
    replay.expected_revision = Some(completion.expected_revision);
    replay.confirmation = Some(completion.confirmation.clone());
    let replayed = fixture
        .adapter
        .execute(
            RpcMethod::OperationExecute,
            &replay,
            Some(&fixture.root_workspace()),
        )
        .expect("same-key replay returns the committed receipt");
    assert_eq!(replayed["changed"], false, "{replayed}");
    assert_eq!(
        replayed["revisionAfter"], completion.result["revisionAfter"],
        "the replay returns the original receipt: {replayed}"
    );
}

struct CompletedWorkItem {
    result: Value,
    lease_id: String,
    fencing_token: u64,
    expected_revision: u64,
    confirmation: ae_sdd_protocol::ConfirmationRef,
}

fn complete_work_item(fixture: &Fixture, key_prefix: &str) -> CompletedWorkItem {
    let lease = acquire_lease(
        fixture,
        &fixture.root_workspace(),
        "root-agent",
        ROOT_SESSION,
        "root",
        &format!("{key_prefix}-complete-lease"),
    );
    let expected_revision = fixture.revision();
    let mut complete = operation_params(
        "root-agent",
        ROOT_SESSION,
        "workitem.complete",
        json!({}),
        &format!("{key_prefix}-complete"),
    );
    complete.lease_id = Some(lease.0.clone());
    complete.fencing_token = Some(lease.1);
    complete.expected_revision = Some(expected_revision);
    let required = fixture
        .adapter
        .execute(
            RpcMethod::OperationExecute,
            &complete,
            Some(&fixture.root_workspace()),
        )
        .expect_err("completion without confirmation must be refused");
    assert_eq!(
        required.code(),
        StableErrorCode::ConfirmationRequired,
        "{required:?}"
    );
    let binding = required
        .remediation()
        .and_then(|remediation| remediation.split_whitespace().last().map(str::to_owned))
        .expect("confirmation binding");
    complete.confirmation = Some(ae_sdd_protocol::ConfirmationRef {
        confirmation_id: binding,
        approved_by: "user:owner".to_owned(),
        approved_at: "2026-07-27T00:00:00Z".to_owned(),
    });
    let result = fixture
        .adapter
        .execute(
            RpcMethod::OperationExecute,
            &complete,
            Some(&fixture.root_workspace()),
        )
        .unwrap_or_else(|error| panic!("{key_prefix} completion failed: {error:?}"));
    CompletedWorkItem {
        result,
        lease_id: lease.0,
        fencing_token: lease.1,
        expected_revision,
        confirmation: complete.confirmation.expect("confirmation was set"),
    }
}

impl std::fmt::Debug for CompletedWorkItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompletedWorkItem")
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}

/// Step 3: a changed-path edit after the Review rolls `GovernanceClosed`
/// back, so a fresh completion intent is denied on the open milestone and
/// `workitem.complete` fails closed instead of committing a stale
/// `Completed`.
#[test]
fn changed_path_after_review_invalidates_governance_close() {
    let fixture = Fixture::new();
    write_source(
        fixture.workspace_root.path(),
        SLICES[0],
        "pub fn alpha() -> u8 { 1 }\n",
    );
    write_source(
        fixture.workspace_root.path(),
        SLICES[1],
        "pub fn beta() -> u8 { 2 }\n",
    );
    write_source(
        fixture.workspace_root.path(),
        SLICES[2],
        "pub fn gamma() -> u8 { 3 }\n",
    );
    drive_to_governance_closed(&fixture, "stale");
    assert_eq!(fixture.milestone(), "governance-closed");

    // The stale edit never touches state; only the observed code digest moves,
    // which rolls the effective milestone back to the earliest affected point.
    write_source(
        fixture.workspace_root.path(),
        SLICES[1],
        "pub fn beta() -> u8 { 42 }\n",
    );

    let mut intent = root_params(&json!({"targetPhase":"completed"}), "stale-intent");
    intent.idempotency_key = Some("stale-completion-intent".to_owned());
    let denied = fixture
        .adapter
        .execute(
            RpcMethod::FlowNext,
            &intent,
            Some(&fixture.root_workspace()),
        )
        .unwrap_or_else(|error| panic!("stale intent projection fails: {error:?}"));
    assert_eq!(
        denied["nextAction"]["kind"], "transition-denied",
        "a rolled-back GovernanceClosed must deny the completion intent: {denied}"
    );
    let reason = denied["nextAction"]["reason"]
        .as_str()
        .expect("denial reason");
    assert!(
        reason.contains("milestone"),
        "the denial must name the rolled completion milestone: {reason}"
    );

    // The terminal mutation fails closed too: the denied intent can never
    // produce a pending transition with fresh terminal Gates.
    let lease = acquire_lease(
        &fixture,
        &fixture.root_workspace(),
        "root-agent",
        ROOT_SESSION,
        "root",
        "stale-complete-lease",
    );
    let mut complete = operation_params(
        "root-agent",
        ROOT_SESSION,
        "workitem.complete",
        json!({}),
        "stale-complete",
    );
    complete.lease_id = Some(lease.0.clone());
    complete.fencing_token = Some(lease.1);
    complete.expected_revision = Some(fixture.revision());
    let required = fixture
        .adapter
        .execute(
            RpcMethod::OperationExecute,
            &complete,
            Some(&fixture.root_workspace()),
        )
        .expect_err("completion without confirmation must be refused");
    assert_eq!(
        required.code(),
        StableErrorCode::ConfirmationRequired,
        "{required:?}"
    );
    let binding = required
        .remediation()
        .and_then(|remediation| remediation.split_whitespace().last().map(str::to_owned))
        .expect("confirmation binding");
    complete.confirmation = Some(ae_sdd_protocol::ConfirmationRef {
        confirmation_id: binding,
        approved_by: "user:owner".to_owned(),
        approved_at: "2026-07-27T00:00:00Z".to_owned(),
    });
    let error = fixture
        .adapter
        .execute(
            RpcMethod::OperationExecute,
            &complete,
            Some(&fixture.root_workspace()),
        )
        .expect_err("a stale completion must never commit Completed");
    assert_eq!(error.code(), StableErrorCode::GateBlocked, "{error:?}");
    let state = read_state(&fixture.state_path);
    assert_eq!(state["phase"], "code-reviewed", "{state}");
}
