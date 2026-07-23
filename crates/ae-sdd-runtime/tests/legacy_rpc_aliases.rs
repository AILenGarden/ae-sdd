mod support;

#[allow(dead_code, unused_imports)]
#[path = "../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;

use std::collections::BTreeMap;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use legacy::{
    LegacyRequestSource, adapt_passthrough_request, parse_rpc_invocation, resolve_command_id,
};
use serde_json::{Value, json};

use support::{Harness, open_root_session, params, register_workspace, result, session_params};

fn environment(values: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let values = values
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect::<BTreeMap<_, _>>();
    move |name| values.get(name).cloned()
}

fn legacy_params(
    command_id: &str,
    method: RpcMethod,
    arguments: &[String],
    environment_values: &[(&str, &str)],
) -> ae_sdd_protocol::RequestParams<Value> {
    let route = resolve_command_id(command_id).expect("known legacy route");
    let invocation =
        parse_rpc_invocation(&route, method, arguments, environment(environment_values))
            .expect("legacy argv parses");
    let LegacyRequestSource::Synthesized(mut request) = invocation.request else {
        panic!("expected synthesized request")
    };
    adapt_passthrough_request(command_id, method, &mut request).expect("legacy route adapts");
    *request
}

#[test]
fn health_legacy_argv_reaches_real_runtime_status() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let request = legacy_params("health", RpcMethod::RuntimeStatus, &[], &[]);

    let status = result(&harness.call(&mut connection, RpcMethod::RuntimeStatus, request));

    assert_eq!(status["lifecycle"], "running");
    assert_eq!(status["bootId"], harness.runtime.boot_id().to_string());
}

#[test]
fn review_abort_alias_cancels_one_attested_delegation_and_records_reason() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut host_register = params(
        json!({"adapterId":"host-legacy","capabilities":["create","attest"]}),
        1_000,
    );
    host_register.capability_token = Some(harness.host_credential());
    host_register.idempotency_key = Some("host-register-legacy".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, host_register));

    let mut root_connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut root_connection, "legacy-cancel");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "legacy-root",
        Some("WORK"),
    );
    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({
            "childRole":"series",
            "parentDelegationId":null,
            "inputRevision":1,
            "inputFingerprint":"a".repeat(64),
            "deadlineUnixMs":2_000,
            "adapterId":"host-legacy",
            "grant":{"operations":[],"capabilities":[],"paths":[]}
        }),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("legacy-delegation-create".to_owned());
    let created = result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = created["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();

    let arguments = [
        "--workspace-id",
        workspace.workspace_id.as_str(),
        "--work-item",
        "WORK",
        "--session-id",
        root.session_id.as_str(),
        "--idempotency-key",
        "legacy-review-abort",
        "--delegation-id",
        delegation_id.as_str(),
        "--reason",
        "user-request",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let request = legacy_params(
        "review abort",
        RpcMethod::DelegationCancel,
        &arguments,
        &[
            ("AE_SDD_AGENT_ID", "root-agent"),
            ("AE_SDD_CAPABILITY_TOKEN", root.capability_token.as_str()),
        ],
    );
    let cancelled =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCancel, request));

    assert_eq!(cancelled["delegationId"], delegation_id);
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["cancellationReason"], "user-request");
}
