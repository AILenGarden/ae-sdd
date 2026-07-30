use std::fs;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use serde_json::{Value, json};

use super::support::*;

#[test]
fn governance_mutations_are_normalized_trusted_and_retry_safe() {
    let harness = Harness::new_realtime();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(
        &harness,
        &mut cli,
        &workspace,
        "governance-root",
        "governance-agent",
    );
    let identity = identity(&workspace, &root, "governance-agent");
    let acquired = success(&invoke(
        &harness,
        &mut cli,
        &identity,
        "lease acquire",
        args(&[
            "--owner",
            "{\"role\":\"root\"}",
            "--ttl-seconds",
            "300",
            "--idempotency-key",
            "governance-lease",
        ]),
    ));
    let lease_id = acquired["data"]["leaseId"].as_str().expect("lease id");
    let fencing = acquired["data"]["fencingToken"]
        .as_u64()
        .expect("fencing token");

    let mut missing_plan = operation_params(
        &identity,
        "execution.plan.approve",
        json!({"approvedBy":"payload-forgery"}),
    );
    bind_write(
        &mut missing_plan,
        lease_id,
        fencing,
        1,
        "approve-missing-plan",
    );
    missing_plan.confirmation = Some(confirmation_ref(
        "approve-missing-plan-confirmation",
        "user:trusted",
        "2026-07-23T08:00:00Z",
    ));
    let before_missing = fs::read(&harness.state_path).expect("state before rejected approval");
    let rejected = call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        missing_plan,
    );
    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");
    assert_eq!(
        fs::read(&harness.state_path).expect("state after rejected approval"),
        before_missing
    );

    let plan_payload = json!({
        "goal":"  Implement governed operations  ",
        "changedPaths":["src\\lib.rs", "src/lib.rs", "  docs/story.md  "],
        "verification":[{"id":"V-1","acId":"AC-1","command":"cargo test"}],
        "risks":["  compatibility  ", "", "compatibility"],
        "sourceReads":["tools\\lib\\state.py", "tools/lib/state.py", "  "],
    });
    let mut set_plan = operation_params(&identity, "execution.plan.set", plan_payload.clone());
    bind_write(&mut set_plan, lease_id, fencing, 1, "set-governed-plan");
    let set_wire = serde_json::to_value(&set_plan).expect("set plan wire");
    let planned = success(&raw_call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        set_wire.clone(),
    ));
    assert_eq!(planned["revisionAfter"], 2);
    assert_eq!(planned["data"]["goal"], "Implement governed operations");
    assert_eq!(
        planned["data"]["changedPaths"],
        json!(["src/lib.rs", "docs/story.md"])
    );
    assert_eq!(planned["data"]["approved"], false);
    assert!(planned["data"]["approvedAt"].is_null());
    assert!(planned["data"]["approvedBy"].is_null());
    let plan_replay = success(&raw_call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        set_wire,
    ));
    assert_eq!(plan_replay["changed"], false);
    assert_eq!(plan_replay["data"], planned["data"]);

    let mut conflicting = operation_params(
        &identity,
        "execution.plan.set",
        json!({
            "goal":"different",
            "changedPaths":["src/lib.rs"],
            "verification":[{"id":"V-1"}],
        }),
    );
    bind_write(&mut conflicting, lease_id, fencing, 1, "set-governed-plan");
    let conflict = call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        conflicting,
    );
    assert_eq!(stable_error(&conflict), "IDEMPOTENCY_KEY_REUSED");

    let mut approve = operation_params(
        &identity,
        "execution.plan.approve",
        json!({"approvedBy":"payload-forgery"}),
    );
    bind_write(&mut approve, lease_id, fencing, 2, "approve-governed-plan");
    approve.confirmation = Some(confirmation_ref(
        "approve-governed-plan-confirmation",
        "user:trusted",
        "2026-07-23T08:01:00Z",
    ));
    let approved = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        approve,
    ));
    assert_eq!(approved["revisionAfter"], 3);
    assert_eq!(approved["data"]["approved"], true);
    assert_eq!(approved["data"]["approvedBy"], "user:trusted");
    assert_eq!(approved["data"]["approvedAt"], "2026-07-23T08:01:00Z");
    assert!(approved["data"].get("approval").is_none());

    assert_success(&invoke(
        &harness,
        &mut cli,
        &identity,
        "lease release",
        lease_args(lease_id, fencing, "release-before-review", false, "root"),
    ));
    let (author, reviewer) = open_review_lineage(
        &harness,
        &mut cli,
        &workspace,
        &identity,
        "general",
        "governance-review",
    );
    let mut review_state: Value = serde_json::from_slice(
        &fs::read(&harness.state_path).expect("state before independent review"),
    )
    .expect("review state JSON");
    review_state["scale"] = json!("small");
    review_state["storyStates"]["STORY-TYPED-E2E"]["authorSessionId"] = json!(author.session_id);
    review_state["review"] = json!({
        "status":"passed",
        "findings":[],
        "zeroFindingsRationale":"legacy clean review",
        "evidenceIds":["legacy-review-evidence"],
    });
    review_state["reviewLoop"] = json!({
        "status":"passed",
        "exitReason":"passed",
        "cleanStreak":2,
    });
    fs::write(
        &harness.state_path,
        serde_json::to_vec_pretty(&review_state).expect("review state serializes"),
    )
    .expect("review state");
    let reviewer_identity = identity_for_work_item(
        &workspace,
        &reviewer,
        "STORY-TYPED-E2E",
        "governance-review-reviewer-agent",
    );
    let mut reviewer_lease_request = operation_params(
        &reviewer_identity,
        "lease.acquire",
        json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
    );
    reviewer_lease_request.idempotency_key = Some("reviewer-lease".to_owned());
    let reviewer_lease = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        reviewer_lease_request,
    ));
    let reviewer_lease_id = reviewer_lease["data"]["leaseId"]
        .as_str()
        .expect("reviewer lease id");
    let reviewer_fencing = reviewer_lease["data"]["fencingToken"]
        .as_u64()
        .expect("reviewer fencing");

    let mut pending = operation_params(
        &reviewer_identity,
        "review.record",
        json!({"status":"pending","findings":[]}),
    );
    bind_write(
        &mut pending,
        reviewer_lease_id,
        reviewer_fencing,
        3,
        "record-pending-review",
    );
    let pending = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        pending,
    ));
    assert_eq!(pending["revisionAfter"], 4);
    assert_eq!(pending["data"]["status"], "pending");
    let pending_state = read_state(&harness);
    assert_eq!(
        pending_state["review"], pending["data"],
        "a running review session must replace the previous terminal review"
    );
    assert_eq!(pending_state["review"]["batch"]["schemaVersion"], "v2");
    assert_eq!(
        pending_state["review"]["batch"]["latestStatus"],
        "INVALID_INFRA"
    );
    assert!(pending_state["review"].get("receipt").is_none());
    assert!(
        pending_state.get("reviewLoop").is_none(),
        "a daemon review session must invalidate the legacy review loop"
    );
    assert_eq!(pending_state["reviewSession"]["status"], "running");

    for (index, payload) in [
        json!({"status":"passed","findings":[{"severity":"P1"}]}),
        json!({"status":"changes_required","findings":[]}),
        json!({"status":"changes_required","findings":[{"problem":"missing severity"}]}),
        json!({"status":"unknown","findings":[]}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut invalid = operation_params(&reviewer_identity, "review.record", payload);
        bind_write(
            &mut invalid,
            reviewer_lease_id,
            reviewer_fencing,
            4,
            &format!("invalid-review-{index}"),
        );
        let error = call(
            &harness.runtime,
            &mut cli,
            RpcMethod::OperationExecute,
            invalid,
        );
        assert_eq!(stable_error(&error), "OPERATION_SCHEMA_INVALID");
    }

    let mut review = operation_params(
        &reviewer_identity,
        "review.record",
        json!({
            "status":"changes_required",
            "findings":[{"severity":"P1","problem":"Missing guard"}],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["E-1"],
        }),
    );
    bind_write(
        &mut review,
        reviewer_lease_id,
        reviewer_fencing,
        4,
        "record-governed-review",
    );
    let reviewed = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        review,
    ));
    assert_eq!(reviewed["revisionAfter"], 5);
    assert_eq!(reviewed["data"]["status"], "changes_required");
    assert_eq!(reviewed["data"]["batch"]["latestStatus"], "VALID_FINDINGS");
    assert_eq!(reviewed["data"]["nextAction"]["kind"], "remediate");
    let state: Value = serde_json::from_slice(
        &fs::read(&harness.state_path).expect("authoritative governance state"),
    )
    .expect("state JSON");
    assert_eq!(state["executionPlan"], approved["data"]);
    assert_eq!(state["review"], reviewed["data"]);
    assert_eq!(state["reviewSession"]["status"], "remediation_required");
    assert!(!harness.workspace_root.path().join("CodeReview.md").exists());
}

#[test]
fn state_transition_and_workitem_completion_require_durable_root_intent() {
    pause_transition_is_committed_and_replayed();
    completed_transition_rechecks_all_required_gates();
}

fn pause_transition_is_committed_and_replayed() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(&harness, &mut cli, &workspace, "pause-root", "pause-agent");
    let identity = identity(&workspace, &root, "pause-agent");
    let (lease_id, fencing) = acquire_lease(&harness, &mut cli, &identity, "pause-lease");

    let mut intent = trusted_params(&identity, json!({"targetPhase":"paused"}));
    intent.idempotency_key = Some("pause-transition-intent".to_owned());
    assert_success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::FlowNext,
        intent,
    ));
    let mut pending = operation_params(
        &identity,
        "state.transition",
        json!({"targetPhase":"paused"}),
    );
    bind_write(
        &mut pending,
        &lease_id,
        fencing,
        1,
        "pause-transition-commit",
    );
    let confirmation_required = call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        pending,
    );
    assert_eq!(
        stable_error(&confirmation_required),
        "CONFIRMATION_REQUIRED",
        "{confirmation_required}"
    );
    let binding = confirmation_required["error"]["data"]["remediation"]
        .as_str()
        .and_then(|value| value.split_whitespace().last())
        .expect("pause remediation carries the engine binding")
        .to_owned();
    let mut transition = operation_params(
        &identity,
        "state.transition",
        json!({"targetPhase":"paused"}),
    );
    bind_write(
        &mut transition,
        &lease_id,
        fencing,
        1,
        "pause-transition-commit",
    );
    transition.confirmation = Some(confirmation_ref(
        &binding,
        "user:trusted",
        "2026-07-23T08:02:00Z",
    ));
    let wire = serde_json::to_value(&transition).expect("transition wire");
    let committed = success(&raw_call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        wire.clone(),
    ));
    let replay = success(&raw_call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        wire,
    ));
    assert_eq!(committed["revisionAfter"], 2);
    assert_eq!(committed["data"]["phase"], "paused");
    assert!(committed["data"]["planDigest"].is_string());
    assert_eq!(replay["changed"], false);
    let state = read_state(&harness);
    assert_eq!(state["storyStates"]["STORY-TYPED-E2E"]["phase"], "paused");
}

fn completed_transition_rechecks_all_required_gates() {
    // Real time is required: the review lineage the fixture below drives is
    // only live when session expiry is measured against the clock the
    // operations observe.
    let harness = Harness::new_realtime();
    let mut state = read_state(&harness);
    state["storyStates"]["STORY-TYPED-E2E"]["phase"] = json!("code-reviewed");
    state["storyStates"]["STORY-TYPED-E2E"]["currentPhase"] = json!("code-reviewed");
    state["scale"] = json!("medium");
    // The review projection is installed by the real `review.record` operation
    // below, so no hand-written `review` object is seeded here.
    state["executionPlan"] = json!({
        "goal":"complete",
        "changedPaths":["src/lib.rs"],
        "verification":[{"id":"V-1","acId":"AC-1"}],
        "sourceReads":["ae-sdd-doc/RA/x.md","ae-sdd-doc/DR/x.md","ae-sdd-doc/Story/x.md"],
        "approved":true,
    });
    state["evidenceRefs"] = json!([
        {
            "evidenceId":"complete-g00",
            "verificationId":"G-00",
            "path":".ae-sdd/evidence/complete-g00.json",
            "digest":"0000000000000000000000000000000000000000000000000000000000000000",
            "byteLength":1
        },
        {
            "evidenceId":"complete-g12",
            "verificationId":"G-12",
            "path":".ae-sdd/evidence/complete-g12.json",
            "digest":"1212121212121212121212121212121212121212121212121212121212121212",
            "byteLength":1
        },
        {
            "evidenceId":"complete-g13",
            "verificationId":"G-13",
            "path":".ae-sdd/evidence/complete-g13.json",
            "digest":"1313131313131313131313131313131313131313131313131313131313131313",
            "byteLength":1
        }
    ]);
    fs::write(
        &harness.state_path,
        serde_json::to_vec_pretty(&state).expect("completion state serializes"),
    )
    .expect("completion state");
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(
        &harness,
        &mut cli,
        &workspace,
        "complete-root",
        "complete-agent",
    );
    let identity = identity(&workspace, &root, "complete-agent");
    install_tier2_review_prerequisites(&harness, "STORY-TYPED-E2E", "STORY-TYPED-E2E");
    install_completed_review_authority(
        &harness,
        &mut cli,
        &workspace,
        &identity,
        "STORY-TYPED-E2E",
        "complete-review",
    );
    let (lease_id, fencing) = acquire_lease(&harness, &mut cli, &identity, "complete-lease");

    let mut intent = trusted_params(&identity, json!({"targetPhase":"completed"}));
    intent.idempotency_key = Some("complete-transition-intent".to_owned());
    assert_success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::FlowNext,
        intent,
    ));
    for gate_id in ["G-00", "G-12", "G-13"] {
        let mut gate = trusted_params(&identity, json!({"gateId":gate_id}));
        gate.fencing_token = Some(fencing);
        gate.idempotency_key = Some(format!("complete-{gate_id}"));
        let result = success(&call(
            &harness.runtime,
            &mut cli,
            RpcMethod::GateEvaluate,
            gate,
        ));
        assert_eq!(result["outcome"]["kind"], "PASS", "{gate_id}: {result}");
    }
    // The review authority above is earned through real contributions, so the
    // completion writes against whatever revision those commits produced.
    let revision = read_state(&harness)["revision"]
        .as_u64()
        .expect("state revision before completion");
    let mut complete = operation_params(&identity, "workitem.complete", json!({}));
    bind_write(
        &mut complete,
        &lease_id,
        fencing,
        revision,
        "complete-workitem",
    );
    let confirmation_required = call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        complete,
    );
    assert_eq!(
        stable_error(&confirmation_required),
        "CONFIRMATION_REQUIRED",
        "{confirmation_required}"
    );
    let binding = confirmation_required["error"]["data"]["remediation"]
        .as_str()
        .and_then(|value| value.split_whitespace().last())
        .expect("confirmation remediation carries the engine binding");
    let mut complete = operation_params(&identity, "workitem.complete", json!({}));
    bind_write(
        &mut complete,
        &lease_id,
        fencing,
        revision,
        "complete-workitem",
    );
    complete.confirmation = Some(confirmation_ref(
        binding,
        "user:trusted",
        "2026-07-23T08:03:00Z",
    ));
    let completed = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        complete,
    ));
    assert_eq!(completed["revisionAfter"], revision + 1);
    assert_eq!(completed["data"]["phase"], "completed");
    assert!(completed["data"]["planDigest"].is_string());
    let state = read_state(&harness);
    assert_eq!(
        state["storyStates"]["STORY-TYPED-E2E"]["phase"],
        "completed"
    );
}

fn acquire_lease(
    harness: &Harness,
    cli: &mut ae_sdd_runtime::ConnectionState,
    identity: &CliIdentity,
    key: &str,
) -> (String, u64) {
    let acquired = success(&invoke(
        harness,
        cli,
        identity,
        "lease acquire",
        args(&[
            "--owner",
            "{\"role\":\"root\"}",
            "--ttl-seconds",
            "300",
            "--idempotency-key",
            key,
        ]),
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
