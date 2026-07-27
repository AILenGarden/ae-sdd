#[allow(dead_code, unused_imports)]
#[path = "../src/legacy/mod.rs"]
mod legacy;

use std::collections::BTreeMap;

use ae_sdd_protocol::{RequestParams, RpcMethod};
use legacy::{
    LegacyRequestSource, TemporaryJsonRequest, adapt_passthrough_request, parse_rpc_invocation,
    resolve_command_id, validate_passthrough_result,
};
use serde_json::{Value, json};

fn environment(values: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let values = values
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect::<BTreeMap<_, _>>();
    move |name| values.get(name).cloned()
}

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn parsed_params(
    command_id: &str,
    method: RpcMethod,
    args: &[&str],
    env: &[(&str, &str)],
) -> RequestParams<Value> {
    let route = resolve_command_id(command_id).expect("known route");
    let invocation = parse_rpc_invocation(&route, method, &arguments(args), environment(env))
        .unwrap_or_else(|error| panic!("{command_id}: {error}"));
    let LegacyRequestSource::Synthesized(params) = invocation.request else {
        panic!("expected synthesized request")
    };
    *params
}

#[test]
fn scanner_and_ra_batch_commands_map_to_exact_native_gate_ids() {
    let env = [
        ("AE_SDD_AGENT_ID", "root-agent"),
        ("AE_SDD_CAPABILITY_TOKEN", "capability"),
        // `gate.evaluate` writes its outcome, so the method spec requires an
        // idempotency key; the environment channel is the documented fallback.
        ("AE_SDD_IDEMPOTENCY_KEY", "legacy-gate-adapter-fixture"),
    ];
    let mut coding = parsed_params(
        "gate coding-required",
        RpcMethod::GateEvaluate,
        &[
            "--workspace-id",
            "workspace-1",
            "--work-item",
            "STORY-1",
            "--session-id",
            "session-1",
            "--project",
            ".",
        ],
        &env,
    );
    adapt_passthrough_request("gate coding-required", RpcMethod::GateEvaluate, &mut coding)
        .expect("coding Gate adapts");
    assert_eq!(coding.payload["gateId"], "G-CODE-1");
    assert!(coding.payload["expectedProjectRoot"].is_string());

    let mut batch = parsed_params(
        "gate ra-required",
        RpcMethod::GateEvaluate,
        &[
            "--workspace-id",
            "workspace-1",
            "--work-item",
            "STORY-1",
            "--session-id",
            "session-1",
        ],
        &env,
    );
    adapt_passthrough_request("gate ra-required", RpcMethod::GateEvaluate, &mut batch)
        .expect("RA batch adapts");
    assert_eq!(batch.payload["gateIds"].as_array().map(Vec::len), Some(7));
    assert_eq!(batch.payload["gateIds"][3], "G-RA-4");
}

#[test]
fn gate_result_requires_real_pass_and_rejects_collapsed_or_non_pass_results() {
    validate_passthrough_result(
        "gate coding-required",
        RpcMethod::GateEvaluate,
        &json!({"outcome":{"kind":"PASS"}}),
    )
    .expect("PASS succeeds");
    assert!(
        validate_passthrough_result(
            "gate coding-required",
            RpcMethod::GateEvaluate,
            &json!({"outcome":{"kind":"FAIL","findings":[]}}),
        )
        .expect_err("FAIL must produce a non-zero CLI result")
        .to_string()
        .contains("LEGACY_GATE_NON_PASS")
    );
    assert!(
        validate_passthrough_result(
            "gate ra-required",
            RpcMethod::GateEvaluate,
            &json!({"allPass":false,"results":[]}),
        )
        .is_err()
    );
    assert!(
        validate_passthrough_result(
            "gate coding-required",
            RpcMethod::GateEvaluate,
            &json!({"ok":true}),
        )
        .is_err()
    );
}

#[test]
fn delegation_aliases_require_explicit_physical_delegation_identity() {
    let env = [
        ("AE_SDD_AGENT_ID", "root-agent"),
        ("AE_SDD_CAPABILITY_TOKEN", "capability"),
    ];
    let mut cancel = parsed_params(
        "review abort",
        RpcMethod::DelegationCancel,
        &[
            "--workspace-id",
            "workspace-1",
            "--work-item",
            "STORY-1",
            "--session-id",
            "session-1",
            "--idempotency-key",
            "cancel-1",
            "--delegation-id",
            "delegation-1",
            "--reason",
            "user-request",
        ],
        &env,
    );
    adapt_passthrough_request("review abort", RpcMethod::DelegationCancel, &mut cancel)
        .expect("cancel adapts");
    assert_eq!(
        cancel.payload,
        json!({"delegationId":"delegation-1","reason":"user-request"})
    );

    for (command_id, method) in [
        ("review abort", RpcMethod::DelegationCancel),
        ("review collect", RpcMethod::DelegationCollect),
        ("review-loop collect", RpcMethod::DelegationCollect),
    ] {
        let mut missing = parsed_params(
            command_id,
            method,
            &[
                "--workspace-id",
                "workspace-1",
                "--work-item",
                "STORY-1",
                "--session-id",
                "session-1",
                "--idempotency-key",
                "missing-delegation",
            ],
            &env,
        );
        assert!(
            adapt_passthrough_request(command_id, method, &mut missing)
                .expect_err("missing physical identity must fail before IPC")
                .to_string()
                .contains("--delegation-id"),
            "{command_id}"
        );
    }
}

#[test]
fn health_rejects_ignored_business_fields_before_ipc() {
    let mut params = parsed_params(
        "health",
        RpcMethod::RuntimeStatus,
        &["--unexpected", "value"],
        &[],
    );
    assert!(
        adapt_passthrough_request("health", RpcMethod::RuntimeStatus, &mut params)
            .expect_err("runtime status must not ignore business flags")
            .to_string()
            .contains("unsupported legacy business fields")
    );
}

#[test]
fn operation_request_file_becomes_typed_daemon_params_without_scope_override() {
    let env = [
        ("AE_SDD_AGENT_ID", "root-agent"),
        ("AE_SDD_CAPABILITY_TOKEN", "capability"),
    ];
    let request = TemporaryJsonRequest::create(&json!({
        "schemaVersion":"1",
        "operation":"state.next_actions",
        "project":"D:/trusted-by-workspace-registration",
        "projectKey":"sample",
        "workItem":"STORY-1",
        "story":"STORY-1",
        "expectedRevision":9,
        "idempotencyKey":"operation-1",
        "dryRun":false,
        "parameters":{}
    }))
    .expect("temporary operation request");
    let path = request.path().to_string_lossy().into_owned();
    let mut params = parsed_params(
        "ops execute",
        RpcMethod::OperationExecute,
        &[
            "--workspace-id",
            "workspace-1",
            "--work-item",
            "STORY-1",
            "--session-id",
            "session-1",
            "--request-file",
            &path,
        ],
        &env,
    );
    adapt_passthrough_request("ops execute", RpcMethod::OperationExecute, &mut params)
        .expect("operation request adapts");
    assert_eq!(params.work_item_id.as_deref(), Some("STORY-1"));
    assert_eq!(params.expected_revision, Some(9));
    assert_eq!(params.idempotency_key.as_deref(), Some("operation-1"));
    assert_eq!(
        params.payload,
        json!({
            "operation":"state.next_actions",
            "dryRun":false,
            "payload":{},
            "expectedProjectRoot":"D:/trusted-by-workspace-registration",
            "expectedProjectKey":"sample",
            "story":"STORY-1"
        })
    );
}

#[test]
fn operation_request_conflicts_and_dry_run_is_forwarded() {
    let env = [
        ("AE_SDD_AGENT_ID", "root-agent"),
        ("AE_SDD_CAPABILITY_TOKEN", "capability"),
    ];
    let conflict = TemporaryJsonRequest::create(&json!({
        "schemaVersion":"1",
        "operation":"state.next_actions",
        "project":"D:/project",
        "workItem":"STORY-FILE",
        "parameters":{}
    }))
    .expect("temporary request");
    let path = conflict.path().to_string_lossy().into_owned();
    let mut params = parsed_params(
        "ops execute",
        RpcMethod::OperationExecute,
        &[
            "--workspace-id",
            "workspace-1",
            "--work-item",
            "STORY-CLI",
            "--session-id",
            "session-1",
            "--request-file",
            &path,
        ],
        &env,
    );
    assert!(
        adapt_passthrough_request("ops execute", RpcMethod::OperationExecute, &mut params)
            .expect_err("scope conflict")
            .to_string()
            .contains("OPERATION_SCOPE_CONFLICT")
    );

    let dry_run = TemporaryJsonRequest::create(&json!({
        "schemaVersion":"1",
        "operation":"state.next_actions",
        "project":"D:/project",
        "workItem":"STORY-1",
        "dryRun":true,
        "parameters":{}
    }))
    .expect("temporary request");
    let path = dry_run.path().to_string_lossy().into_owned();
    let mut params = parsed_params(
        "ops execute",
        RpcMethod::OperationExecute,
        &[
            "--workspace-id",
            "workspace-1",
            "--work-item",
            "STORY-1",
            "--session-id",
            "session-1",
            "--request-file",
            &path,
        ],
        &env,
    );
    adapt_passthrough_request("ops execute", RpcMethod::OperationExecute, &mut params)
        .expect("dry-run is a native operation.execute control");
    assert_eq!(params.payload["dryRun"], true);
}

#[test]
fn no_equivalent_review_state_machine_routes_are_denied() {
    for command_id in ["review start", "review-loop status"] {
        let route = resolve_command_id(command_id).expect("known removed route");
        assert!(matches!(
            route.target,
            legacy::LegacyTarget::Rejected {
                ref stable_code,
                ref remediation,
            } if stable_code == "LEGACY_COMMAND_REMOVED"
                && !remediation.is_empty()
        ));
    }
}
