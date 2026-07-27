#[allow(unused_imports)]
#[path = "typed_operations_cli_e2e/support.rs"]
mod support;

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

use std::fs;
use std::path::Path;

use ae_sdd_domain::{AgentRole, ProjectPathScope, ScopedGrant};
use ae_sdd_protocol::{ClientKind, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{BusinessWorkspace, PersistencePort};
use review_authority::authoritative_review_workspace_input_fingerprint;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use support::*;

const PRD_ID: &str = "PRD-C1-E2E";
const STORY_ID: &str = "STORY-C1-E2E";
const STORY_DOC: &str = "ae-sdd-doc/Story/prd-c1-e2e.md";
/// Declares every AC id the plan verification matrix cites, so the Tier 2
/// deterministic proof can align the CodingPlan with the Story (`G-14`).
const STORY_CONTENT: &str = "# Story\n\nAC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7\nAC-8 AC-9 AC-10 AC-11 AC-12 AC-13 AC-14\n\n## verification\n";

#[test]
fn complete_prd_commits_and_replays_one_exact_project_mutation() {
    // Real time is required: the review lineage this fixture drives is only
    // live when session expiry is measured against the same clock the
    // operations observe.
    let harness = Harness::new_realtime();
    write_prd_state(&harness);
    // A Tier 2 clean batch is sealed only once the deterministic final proof
    // (G-CODEPLAN-SRC, G-14, G-08) passes, which needs an approved plan with the
    // full verification matrix, a Story that declares those AC ids and source
    // reads that exist. Seeded before the child snapshot so the completion
    // assertion compares against the state the review starts from.
    let child_before = read_state(&harness)["storyStates"].clone();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root_for_work_item(
        &harness,
        &mut cli,
        &workspace,
        PRD_ID,
        "lifecycle-prd-root",
        "lifecycle-agent",
    );
    let identity = identity_for_work_item(&workspace, &root, PRD_ID, "lifecycle-agent");

    // The milestone chain runs through the real operations: a green
    // verification record closes ImplementationVerified, evidence
    // finalization closes ReviewReady, and two reviewer records close
    // GovernanceClosed. Hand-built review state can never do this because the
    // Review Gate joins the durable SQLite projection written by the real
    // review operations.
    let (evidence_lease, evidence_fencing) =
        acquire_lease(&harness, &mut cli, &identity, "lifecycle-evidence-lease");
    let recorded_evidence = record_evidence(
        &harness,
        &mut cli,
        &identity,
        &evidence_lease,
        evidence_fencing,
    );
    finalize_evidence(
        &harness,
        &mut cli,
        &identity,
        &evidence_lease,
        evidence_fencing,
    );
    release_lease(
        &harness,
        &mut cli,
        &identity,
        &evidence_lease,
        evidence_fencing,
    );
    assert_eq!(completion_milestone(&harness), "review-ready");
    assert_eq!(
        flow_next_action(&harness, &mut cli, &identity)["kind"],
        "collect-review-contributions"
    );

    // The Review cycle re-seals the recorded ledger entry against the current
    // Review input; the append-only ledger keeps backing the cited evidence.
    reseal_manifest_for_review(&harness, workspace.inventory_generation);
    install_completed_review_authority(
        &harness,
        &mut cli,
        &workspace,
        &identity,
        &recorded_evidence,
    );
    assert_eq!(completion_milestone(&harness), "governance-closed");

    let (lease_id, fencing) = acquire_lease(&harness, &mut cli, &identity, "lifecycle-prd-lease");
    record_completion_intent_and_gates(&harness, &mut cli, &identity, fencing);

    let completion_revision = current_revision(&harness);
    let mut pending = operation_params(&identity, "workitem.complete", json!({}));
    bind_write(
        &mut pending,
        &lease_id,
        fencing,
        completion_revision,
        "complete-prd-once",
    );
    let confirmation_required = call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        pending,
    );
    assert_eq!(
        stable_error(&confirmation_required),
        "CONFIRMATION_REQUIRED"
    );
    let binding = confirmation_required["error"]["data"]["remediation"]
        .as_str()
        .and_then(|value| value.split_whitespace().last())
        .expect("confirmation binding")
        .to_owned();

    let events_before = harness
        .persistence
        .latest_event_sequence()
        .expect("event cursor before completion");
    let journals_before = journal_snapshot(&harness);
    let mut complete = operation_params(&identity, "workitem.complete", json!({}));
    bind_write(
        &mut complete,
        &lease_id,
        fencing,
        completion_revision,
        "complete-prd-once",
    );
    complete.confirmation = Some(confirmation_ref(
        &binding,
        "user:owner",
        "2026-07-25T00:00:00Z",
    ));
    let committed = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        complete,
    ));

    assert_eq!(committed["changed"], true);
    assert_eq!(committed["revisionBefore"], completion_revision);
    assert_eq!(committed["revisionAfter"], completion_revision + 1);
    assert_eq!(committed["data"]["phase"], "completed");
    let state = read_state(&harness);
    assert_eq!(state["phase"], "completed");
    assert_eq!(state["currentPhase"], "completed");
    assert_eq!(state["currentStep"], "completed");
    assert_eq!(state["prdStatus"], "awaiting_compact");
    assert_eq!(state["storyStates"], child_before);

    let journals_after = journal_snapshot(&harness);
    assert_eq!(journals_after.len(), journals_before.len() + 1);
    let events_after_commit = harness
        .persistence
        .latest_event_sequence()
        .expect("event cursor after completion");
    let committed_events = harness
        .persistence
        .events_after(events_before, 10)
        .expect("completion events");
    assert_eq!(events_after_commit, events_before + 2);
    assert_eq!(committed_events.len(), 2);
    assert_eq!(
        committed_events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["workitem.complete", "flow.transition_committed"]
    );

    harness.runtime.recover().expect("runtime recovery");
    let reopened = open_root_for_work_item(
        &harness,
        &mut cli,
        &workspace,
        PRD_ID,
        "lifecycle-prd-root",
        "lifecycle-agent",
    );
    assert_eq!(reopened.session_id, root.session_id);
    let replay_identity = identity_for_work_item(&workspace, &reopened, PRD_ID, "lifecycle-agent");
    let mut replay_request = operation_params(&replay_identity, "workitem.complete", json!({}));
    bind_write(
        &mut replay_request,
        &lease_id,
        fencing,
        completion_revision,
        "complete-prd-once",
    );
    replay_request.confirmation = Some(confirmation_ref(
        &binding,
        "user:owner",
        "2026-07-25T00:00:00Z",
    ));
    let replay = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        replay_request,
    ));
    assert_eq!(replay["changed"], false);
    assert_eq!(replay["revisionBefore"], completion_revision);
    assert_eq!(replay["revisionAfter"], completion_revision + 1);
    assert_eq!(journal_snapshot(&harness), journals_after);
    assert_eq!(read_state(&harness), state);
    assert_eq!(
        harness
            .persistence
            .latest_event_sequence()
            .expect("event cursor after replay"),
        events_after_commit
    );
}

/// Records one green verification evidence through the real operation and
/// returns the ledger-backed evidence id the Review cycle cites.
fn record_evidence(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
    lease_id: &str,
    fencing: u64,
) -> String {
    let mut record = operation_params(
        identity,
        "evidence.record",
        json!({
            "artifactPath":"evidence/result.json",
            "inputFingerprint":"lifecycle-verification-input",
            "kind":"focused-test",
            "command":["cargo","test","-p","lifecycle"],
            "exitCode":0
        }),
    );
    bind_write(
        &mut record,
        lease_id,
        fencing,
        current_revision(harness),
        "lifecycle-evidence-record",
    );
    let recorded = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        record,
    ));
    assert_eq!(recorded["changed"], true, "{recorded}");
    assert_eq!(
        completion_milestone(harness),
        "implementation-verified",
        "a green verification record must close ImplementationVerified"
    );
    assert_eq!(
        flow_next_action(harness, cli, identity)["kind"],
        "finalize-execution-evidence"
    );
    recorded["data"]["evidenceId"]
        .as_str()
        .expect("recorded evidence id")
        .to_owned()
}

fn finalize_evidence(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
    lease_id: &str,
    fencing: u64,
) {
    let mut finalize = operation_params(identity, "evidence.finalize", json!({}));
    bind_write(
        &mut finalize,
        lease_id,
        fencing,
        current_revision(harness),
        "lifecycle-evidence-finalize",
    );
    let finalized = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        finalize,
    ));
    assert_eq!(finalized["changed"], true, "{finalized}");
}

fn release_lease(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
    lease_id: &str,
    fencing: u64,
) {
    let mut release = operation_params(identity, "lease.release", json!({"owner":{"role":"root"}}));
    bind_write(
        &mut release,
        lease_id,
        fencing,
        current_revision(harness),
        "lifecycle-lease-release",
    );
    assert_success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        release,
    ));
}

/// Re-seals the finalized manifest against the current Review input
/// fingerprint, exactly the way a Review-cycle evidence seal binds the
/// already-recorded ledger entries to the current authority.
fn reseal_manifest_for_review(harness: &Harness, inventory_generation: u64) {
    let workspace = BusinessWorkspace {
        workspace_id: "00000000-0000-0000-0000-0000000000c1".to_owned(),
        canonical_root: fs::canonicalize(harness.workspace_root.path())
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned(),
        project_key: "typed-e2e".to_owned(),
        mode: WorkspaceMode::RustCanary,
        agent_role: Some(AgentRole::Root),
        agent_grant: Some(ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot])),
        caller_kind: Some(ClientKind::Cli),
        inventory_generation,
    };
    let state = read_state(harness);
    let input = authoritative_review_workspace_input_fingerprint(&workspace, &state)
        .expect("authoritative review input fingerprint");
    let path = harness
        .workspace_root
        .path()
        .join(format!(".auto-engineering/{PRD_ID}/evidence/manifest.json"));
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).expect("finalized manifest"))
        .expect("manifest JSON");
    for entry in manifest["entries"]
        .as_array_mut()
        .expect("manifest entries")
    {
        entry["inputFingerprint"] = json!(input.to_string());
    }
    let mut payload = manifest.clone();
    payload
        .as_object_mut()
        .expect("manifest object")
        .retain(|key, _| key != "contentHash" && !key.starts_with('_'));
    manifest["contentHash"] = json!(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&payload).expect("manifest canonical JSON")
        ))
    ));
    fs::write(&path, serde_json::to_vec(&manifest).expect("manifest JSON"))
        .expect("resealed manifest");
}

/// Drives one clean `review.record` per required tier-2 specialty through the
/// real operation: each reviewer opens its own delegated lineage, appends its
/// contribution and the adapter immediately finalizes it, so the durable
/// event, the SQLite projection and the reviewer lineage all join.
fn install_completed_review_authority(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    workspace: &ae_sdd_runtime::WorkspaceResult,
    root_identity: &CliIdentity,
    evidence_id: &str,
) {
    // Every specialty of one review must hang off a single Root->Series
    // lineage. Opening a Series per specialty makes the second contribution a
    // different reviewer lineage than the one that opened the session, which
    // the daemon rejects as a turn identity mismatch.
    const SPECIALTIES: [&str; 2] = ["be", "ar"];
    let (_author, reviewers) = open_review_lineage_for_specialties(
        harness,
        cli,
        workspace,
        root_identity,
        &SPECIALTIES,
        "lifecycle-prd",
    );
    for (specialty, reviewer) in SPECIALTIES.iter().zip(reviewers) {
        let lineage_key = format!("lifecycle-prd-{specialty}");
        // `open_delegated_child` registers the reviewer session under
        // `{child key}-agent`, and the reviewer child key is `{lineage}-reviewer`.
        let reviewer_identity = identity_for_work_item(
            workspace,
            &reviewer,
            PRD_ID,
            &format!(
                "{}-agent",
                reviewer_child_key("lifecycle-prd", &SPECIALTIES, specialty)
            ),
        );

        let mut lease_request = operation_params(
            &reviewer_identity,
            "lease.acquire",
            json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
        );
        lease_request.idempotency_key = Some(format!("{lineage_key}-lease"));
        let lease = success(&call(
            &harness.runtime,
            cli,
            RpcMethod::OperationExecute,
            lease_request,
        ));
        let lease_id = lease["data"]["leaseId"]
            .as_str()
            .expect("reviewer lease id")
            .to_owned();
        let fencing = lease["data"]["fencingToken"]
            .as_u64()
            .expect("reviewer fencing token");

        let mut record = operation_params(
            &reviewer_identity,
            "review.record",
            json!({
                "status":"passed",
                "findings":[],
                "reviewedPaths":["src/lib.rs"],
                "evidenceIds":[evidence_id]
            }),
        );
        bind_write(
            &mut record,
            &lease_id,
            fencing,
            current_revision(harness),
            &format!("{lineage_key}-record"),
        );
        let recorded = success(&call(
            &harness.runtime,
            cli,
            RpcMethod::OperationExecute,
            record,
        ));
        assert_eq!(recorded["changed"], true, "{recorded}");

        let mut release = operation_params(
            &reviewer_identity,
            "lease.release",
            json!({"owner":{"role":"reviewer"}}),
        );
        bind_write(
            &mut release,
            &lease_id,
            fencing,
            current_revision(harness),
            &format!("{lineage_key}-release"),
        );
        assert_success(&call(
            &harness.runtime,
            cli,
            RpcMethod::OperationExecute,
            release,
        ));
    }
    let state = read_state(harness);
    assert_eq!(
        state["reviewSession"]["status"], "completed",
        "the second specialty must complete the tier-2 review session: {state}"
    );
    assert_eq!(
        state["review"]["batch"]["latestStatus"], "VALID_CLEAN",
        "the terminal batch must be clean: {state}"
    );
}

fn record_completion_intent_and_gates(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
    fencing: u64,
) {
    let mut intent = trusted_params(identity, json!({"targetPhase":"completed"}));
    intent.idempotency_key = Some("lifecycle-prd-completion-intent".to_owned());
    let decision = success(&call(&harness.runtime, cli, RpcMethod::FlowNext, intent));
    assert_eq!(
        decision["nextAction"]["kind"], "evaluate-gates",
        "GovernanceClosed must open the terminal Gate evaluation: {decision}"
    );

    for gate_id in ["G-00", "G-12", "G-13"] {
        let mut gate = trusted_params(identity, json!({"gateId":gate_id}));
        gate.fencing_token = Some(fencing);
        gate.idempotency_key = Some(format!("lifecycle-prd-{gate_id}"));
        let result = success(&call(&harness.runtime, cli, RpcMethod::GateEvaluate, gate));
        assert_eq!(result["outcome"]["kind"], "PASS", "{gate_id}: {result}");
    }
    let ready = flow_next_action(harness, cli, identity);
    assert_eq!(ready["kind"], "apply-transition", "{ready}");
}

fn flow_next_action(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
) -> Value {
    let projection = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::FlowSnapshot,
        trusted_params(identity, json!({})),
    ));
    projection["nextAction"].clone()
}

fn completion_milestone(harness: &Harness) -> String {
    read_state(harness)
        .pointer("/executionRuntime/completionMilestone")
        .and_then(Value::as_str)
        .expect("recorded completion milestone")
        .to_owned()
}

fn current_revision(harness: &Harness) -> u64 {
    read_state(harness)["revision"]
        .as_u64()
        .expect("state revision")
}

fn acquire_lease(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
    key: &str,
) -> (String, u64) {
    let mut request = operation_params(
        identity,
        "lease.acquire",
        json!({"owner":{"role":"root"},"ttlSeconds":300}),
    );
    request.idempotency_key = Some(key.to_owned());
    let acquired = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        request,
    ));
    (
        acquired["data"]["leaseId"]
            .as_str()
            .expect("lease id")
            .to_owned(),
        acquired["data"]["fencingToken"]
            .as_u64()
            .expect("fencing token"),
    )
}

fn bind_write(
    request: &mut ae_sdd_protocol::RequestParams<Value>,
    lease_id: &str,
    fencing: u64,
    revision: u64,
    key: &str,
) {
    request.lease_id = Some(lease_id.to_owned());
    request.fencing_token = Some(fencing);
    request.expected_revision = Some(revision);
    request.idempotency_key = Some(key.to_owned());
}

fn read_state(harness: &Harness) -> Value {
    serde_json::from_slice(&fs::read(&harness.state_path).expect("state bytes"))
        .expect("state JSON")
}

fn write_prd_state(harness: &Harness) {
    write_document(harness, STORY_DOC, STORY_CONTENT);
    write_document(
        harness,
        "ae-sdd-doc/RA/ra-c1.md",
        "# RA\n\nformal RA fixture\n",
    );
    write_document(harness, "ae-sdd-doc/DR/c1.md", "# DR\n");
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
    let state = json!({
        "stateMachineName":PRD_ID,
        "activeStory":PRD_ID,
        "revision":1,
        "lastFencingToken":0,
        "scale":"medium",
        "selectedDesign":"Story",
        "phase":"code-reviewed",
        "currentPhase":"code-reviewed",
        "currentStep":"code-reviewed",
        "completedSteps":["coding"],
        "pendingOutputs":[],
        "codingRound":1,
        "executionPlan":{
            "goal":"complete the C1 lifecycle fixture",
            "changedPaths":["src/lib.rs"],
            "verification":verification,
            "risks":["fixture risk"],
            "sourceReads":[
                "ae-sdd-doc/RA/ra-c1.md",
                "ae-sdd-doc/DR/c1.md",
                STORY_DOC
            ],
            "approved":true
        },
        "executionRuntime":{
            "schemaVersion":1,
            "queueRef":format!(".auto-engineering/{PRD_ID}/execution/queue.json"),
            "queueDigest":format!("sha256:{}", "1".repeat(64)),
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
            STORY_ID:{
                "phase":"completed",
                "currentPhase":"completed",
                "currentStep":"completed",
                "completedSteps":["code-reviewed"],
                "pendingOutputs":{},
                "codingRound":1,
                "docPath":STORY_DOC,
                "unrelated":{"keep":true}
            }
        },
        "evidenceRefs":[
            {
                "evidenceId":"lifecycle-prd-evidence",
                "verificationId":"V-012",
                "path":".ae-sdd/evidence/lifecycle-prd.json",
                "digest":"1".repeat(64),
                "byteLength":1
            },
            {
                "evidenceId":"lifecycle-prd-g00",
                "verificationId":"G-00",
                "path":".ae-sdd/evidence/lifecycle-prd-g00.json",
                "digest":"0".repeat(64),
                "byteLength":1
            },
            {
                "evidenceId":"lifecycle-prd-g12",
                "verificationId":"G-12",
                "path":".ae-sdd/evidence/lifecycle-prd-g12.json",
                "digest":"12".repeat(32),
                "byteLength":1
            },
            {
                "evidenceId":"lifecycle-prd-g13",
                "verificationId":"G-13",
                "path":".ae-sdd/evidence/lifecycle-prd-g13.json",
                "digest":"13".repeat(32),
                "byteLength":1
            }
        ]
    });
    fs::write(
        &harness.state_path,
        serde_json::to_vec_pretty(&state).expect("PRD state serializes"),
    )
    .expect("PRD state");
}

fn write_document(harness: &Harness, relative: &str, content: &str) {
    let path = Path::new(harness.workspace_root.path()).join(relative);
    fs::create_dir_all(path.parent().expect("document parent")).expect("document directory");
    fs::write(path, content).expect("document");
}
