#[allow(dead_code, unused_imports)]
#[path = "../src/legacy/mod.rs"]
mod legacy;

use ae_sdd_protocol::{PROTOCOL_VERSION_V1, RequestParams, RpcMethod};
use serde_json::{Value, json};

use legacy::{
    LegacyRequestSource, LegacyRpcAdapter, LegacyTarget, adapt_job_submission,
    parse_rpc_invocation, resolve_command_id, validate_job_terminal_status,
};

#[test]
fn three_diagnostics_are_strict_work_item_bound_job_routes() {
    for (id, entrypoint) in [
        ("gate doc-storage", "gate.doc-storage"),
        ("iteration-check", "iteration-check"),
        ("update-check", "update-check"),
    ] {
        let route = resolve_command_id(id).expect("diagnostic route");
        assert!(route.identity_workspace, "{id}");
        assert!(route.identity_work_item, "{id}");
        assert!(route.identity_session, "{id}");
        assert_eq!(
            route.target,
            LegacyTarget::Rpc {
                method: RpcMethod::JobSubmit,
                adapter: LegacyRpcAdapter::JobSubmission {
                    job: legacy::NativeJobKind::Admin,
                    entrypoint: entrypoint.to_owned(),
                },
            }
        );
    }
}

#[test]
fn eight_memory_commands_are_strict_session_bound_job_routes() {
    for (id, entrypoint) in [
        ("memory clean", "memory.clean"),
        ("memory clean-all", "memory.clean-all"),
        ("memory common", "memory.common"),
        ("memory create", "memory.create"),
        ("memory read", "memory.read"),
        ("memory search", "memory.search"),
        ("memory summarize", "memory.summarize"),
        ("memory update", "memory.update"),
    ] {
        let route = resolve_command_id(id).expect("memory route");
        assert!(route.identity_workspace, "{id}");
        assert!(route.identity_work_item, "{id}");
        assert!(route.identity_session, "{id}");
        assert_eq!(
            route.target,
            LegacyTarget::Rpc {
                method: RpcMethod::JobSubmit,
                adapter: LegacyRpcAdapter::JobSubmission {
                    job: legacy::NativeJobKind::Admin,
                    entrypoint: entrypoint.to_owned(),
                },
            }
        );
    }
}

#[test]
fn memory_job_schemas_preserve_entity_arguments_and_reject_ambiguity() {
    let route = resolve_command_id("memory create").expect("create route");
    let mut create = parsed(
        &route,
        &[
            "--entity-type",
            "story",
            "--entity-id",
            "STORY-1",
            "--sources",
            "constraints=constraints/security.md",
            "--context-json",
            "{\"next_step\":\"coding\"}",
        ],
    );
    adapt_job_submission(&route, "memory.create", &mut create, 1_000)
        .expect("memory create adapts");
    assert_eq!(create.payload["arguments"]["entityType"], "story");
    assert_eq!(create.payload["arguments"]["entityId"], "STORY-1");

    let route = resolve_command_id("memory update").expect("update route");
    let mut ambiguous = parsed(
        &route,
        &[
            "--slice",
            "context",
            "--content",
            "inline",
            "--content-file",
            "memory.md",
        ],
    );
    assert!(
        adapt_job_submission(&route, "memory.update", &mut ambiguous, 1_000)
            .expect_err("content sources are exclusive")
            .to_string()
            .contains("cannot be combined")
    );
}

#[test]
fn diagnostic_job_schemas_reject_missing_or_ambiguous_arguments() {
    let route = resolve_command_id("gate doc-storage").expect("route");
    let mut missing = parsed(&route, &[]);
    assert!(
        adapt_job_submission(&route, "gate.doc-storage", &mut missing, 1_000)
            .expect_err("path is required")
            .to_string()
            .contains("path")
    );

    let route = resolve_command_id("update-check").expect("route");
    let mut ambiguous = parsed(
        &route,
        &["--only", "UC-01", "--affected", "source/SKILL.md"],
    );
    assert!(
        adapt_job_submission(&route, "update-check", &mut ambiguous, 1_000)
            .expect_err("modes are exclusive")
            .to_string()
            .contains("cannot be combined")
    );
}

#[test]
fn poll_context_preserves_trusted_work_item_and_uses_bounded_status_deadlines() {
    let route = resolve_command_id("iteration-check").expect("route");
    let mut submission = parsed(&route, &[]);
    adapt_job_submission(&route, "iteration-check", &mut submission, 1_000)
        .expect("job submission");
    let poll = legacy::LegacyJobPollContext::from_submission(&submission).expect("poll context");
    let status = poll.status_request("job-1", 1_500).expect("status request");
    assert_eq!(status.workspace_id.as_deref(), Some("workspace-1"));
    assert_eq!(status.work_item_id.as_deref(), Some("STORY-1"));
    assert_eq!(status.agent_id.as_deref(), Some("agent-1"));
    assert_eq!(status.session_id.as_deref(), Some("session-1"));
    assert_eq!(status.capability_token.as_deref(), Some("capability-1"));
    assert_eq!(status.deadline_ms, 1_000);
    assert_eq!(status.payload, json!({"jobId":"job-1"}));
}

#[test]
fn only_a_real_pass_is_logical_command_success() {
    assert!(
        !validate_job_terminal_status("iteration-check", &json!({"status":"running"}))
            .expect("running is pending")
    );
    assert!(
        validate_job_terminal_status(
            "iteration-check",
            &json!({"status":"pass","result":{"outcome":"PASS"}})
        )
        .expect("pass is success")
    );
    for status in ["fail", "error", "timeout", "cancelled", "stale"] {
        assert!(
            validate_job_terminal_status("iteration-check", &json!({"status":status})).is_err(),
            "{status} must fail"
        );
    }
    assert!(
        validate_job_terminal_status(
            "iteration-check",
            &json!({"status":"pass","result":{"outcome":"FAIL"}})
        )
        .is_err()
    );
}

fn parsed(route: &legacy::LegacyCommandRoute, business: &[&str]) -> RequestParams<Value> {
    let mut arguments = business
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    arguments.extend([
        "--workspace-id".to_owned(),
        "workspace-1".to_owned(),
        "--work-item-id".to_owned(),
        "STORY-1".to_owned(),
        "--agent-id".to_owned(),
        "agent-1".to_owned(),
        "--session-id".to_owned(),
        "session-1".to_owned(),
        "--capability-token".to_owned(),
        "capability-1".to_owned(),
        "--idempotency-key".to_owned(),
        "diagnostic-key".to_owned(),
    ]);
    let invocation = parse_rpc_invocation(route, RpcMethod::JobSubmit, &arguments, |_| None)
        .expect("strict argv");
    match invocation.request {
        LegacyRequestSource::Synthesized(params) => *params,
        LegacyRequestSource::ExplicitJson(_) => panic!("synthesized request expected"),
    }
}

#[allow(dead_code)]
fn params(payload: Value) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some("workspace-1".to_owned()),
        agent_id: None,
        session_id: None,
        capability_token: None,
        turn_id: None,
        work_item_id: Some("STORY-1".to_owned()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: Some("diagnostic-key".to_owned()),
        confirmation: None,
        deadline_ms: 10_000,
        payload,
    }
}
