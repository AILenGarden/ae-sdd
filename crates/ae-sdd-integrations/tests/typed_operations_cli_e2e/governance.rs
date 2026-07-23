use std::fs;

use ae_sdd_domain::{AgentRole, OperationId, ProjectPathScope, ScopedGrant};
use ae_sdd_protocol::{
    ClientKind, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{BusinessOperationPort, BusinessWorkspace};
use serde_json::{Value, json};

use super::support::*;

#[test]
fn governance_mutations_are_normalized_trusted_and_retry_safe() {
    let harness = Harness::new();
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
        lease_args(lease_id, fencing, "release-before-review", false),
    ));
    let reviewer_session = "00000000-0000-0000-0000-000000000901";
    let reviewer_workspace = BusinessWorkspace {
        workspace_id: workspace.workspace_id.clone(),
        canonical_root: fs::canonicalize(harness.workspace_root.path())
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned(),
        project_key: "typed-e2e".to_owned(),
        mode: WorkspaceMode::RustCanary,
        agent_role: Some(AgentRole::Reviewer),
        agent_grant: Some(ScopedGrant::new(
            [
                OperationId::new("lease.acquire").expect("lease operation"),
                OperationId::new("review.record").expect("review operation"),
            ],
            [],
            [ProjectPathScope::ProjectRoot],
        )),
        caller_kind: Some(ClientKind::Cli),
        inventory_generation: 1,
    };
    let adapter = harness.business_adapter();
    let reviewer_lease = adapter
        .execute(
            RpcMethod::OperationExecute,
            &direct_operation_params(
                &workspace.workspace_id,
                reviewer_session,
                "lease.acquire",
                json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
                "reviewer-lease",
            ),
            Some(&reviewer_workspace),
        )
        .expect("reviewer lease");
    let reviewer_lease_id = reviewer_lease["data"]["leaseId"]
        .as_str()
        .expect("reviewer lease id");
    let reviewer_fencing = reviewer_lease["data"]["fencingToken"]
        .as_u64()
        .expect("reviewer fencing");

    for (index, payload) in [
        json!({"status":"passed","findings":[{"severity":"P1"}]}),
        json!({"status":"changes_required","findings":[]}),
        json!({"status":"changes_required","findings":[{"problem":"missing severity"}]}),
        json!({"status":"unknown","findings":[]}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut invalid = direct_operation_params(
            &workspace.workspace_id,
            reviewer_session,
            "review.record",
            payload,
            &format!("invalid-review-{index}"),
        );
        bind_write(
            &mut invalid,
            reviewer_lease_id,
            reviewer_fencing,
            3,
            &format!("invalid-review-{index}"),
        );
        let error = adapter
            .execute(
                RpcMethod::OperationExecute,
                &invalid,
                Some(&reviewer_workspace),
            )
            .expect_err("invalid review must fail closed");
        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    }

    let mut review = direct_operation_params(
        &workspace.workspace_id,
        reviewer_session,
        "review.record",
        json!({
            "status":"changes_required",
            "findings":[{"severity":"P1","problem":"Missing guard"}],
            "reviewedPaths":["src/lib.rs"],
            "evidenceIds":["E-1"],
        }),
        "record-governed-review",
    );
    bind_write(
        &mut review,
        reviewer_lease_id,
        reviewer_fencing,
        3,
        "record-governed-review",
    );
    let reviewed = adapter
        .execute(
            RpcMethod::OperationExecute,
            &review,
            Some(&reviewer_workspace),
        )
        .expect("review records");
    assert_eq!(reviewed["revisionAfter"], 4);
    assert_eq!(reviewed["data"]["status"], "changes_required");
    assert_eq!(
        reviewed["data"].as_object().expect("review result").len(),
        2
    );
    let state: Value = serde_json::from_slice(
        &fs::read(&harness.state_path).expect("authoritative governance state"),
    )
    .expect("state JSON");
    assert_eq!(state["executionPlan"], approved["data"]);
    assert_eq!(state["review"], reviewed["data"]);
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
        "pause-transition-confirmation",
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
    assert_eq!(committed["data"], json!({"phase":"paused"}));
    assert_eq!(replay["changed"], false);
    let state = read_state(&harness);
    assert_eq!(state["storyStates"]["STORY-TYPED-E2E"]["phase"], "paused");
}

fn completed_transition_rechecks_all_required_gates() {
    let harness = Harness::new();
    let mut state = read_state(&harness);
    state["storyStates"]["STORY-TYPED-E2E"]["phase"] = json!("code-reviewed");
    state["storyStates"]["STORY-TYPED-E2E"]["currentPhase"] = json!("code-reviewed");
    state["scale"] = json!("medium");
    state["review"] = json!({"status":"passed","findings":[]});
    state["executionPlan"] = json!({
        "goal":"complete",
        "changedPaths":["src/lib.rs"],
        "verification":[{"id":"V-1","acId":"AC-1"}],
        "sourceReads":["ae-sdd-doc/RA/x.md","ae-sdd-doc/DR/x.md","ae-sdd-doc/Story/x.md"],
        "approved":true,
    });
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
    let mut complete = operation_params(&identity, "workitem.complete", json!({}));
    bind_write(&mut complete, &lease_id, fencing, 1, "complete-workitem");
    complete.confirmation = Some(confirmation_ref(
        "complete-workitem-confirmation",
        "user:trusted",
        "2026-07-23T08:03:00Z",
    ));
    let completed = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::OperationExecute,
        complete,
    ));
    assert_eq!(completed["revisionAfter"], 2);
    assert_eq!(completed["data"], json!({"phase":"completed"}));
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

fn direct_operation_params(
    workspace_id: &str,
    session_id: &str,
    operation: &str,
    payload: Value,
    idempotency_key: &str,
) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(workspace_id.to_owned()),
        agent_id: Some("reviewer-agent".to_owned()),
        session_id: Some(session_id.to_owned()),
        capability_token: None,
        turn_id: None,
        work_item_id: Some("STORY-TYPED-E2E".to_owned()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: Some(idempotency_key.to_owned()),
        confirmation: None,
        deadline_ms: 10_000,
        payload: json!({"operation":operation,"payload":payload}),
    }
}
