#[allow(dead_code, unused_imports)]
#[path = "../src/legacy/mod.rs"]
mod legacy;

use std::collections::BTreeMap;

use ae_sdd_protocol::RpcMethod;
use legacy::{
    LegacyRequestSource, LegacyRpcAdapter, LegacyTarget, adapt_typed_operation_request,
    parse_rpc_invocation, resolve_command_id,
};

const VERIFICATION_PAYLOAD: &str = r#"{"toolsetJobId":"job-1","plan":{},"receiptId":"receipt-1","receiptDigest":"receipt-digest","sourceRevision":1,"planDigest":"plan-digest","methodologyDigest":"methodology-digest","policyDigest":"policy-digest","inputFingerprint":"input-fingerprint","changedPaths":["src/lib.rs"],"persist":true}"#;

fn environment() -> impl Fn(&str) -> Option<String> {
    let values = BTreeMap::from([
        ("AE_SDD_AGENT_ID".to_owned(), "agent-root".to_owned()),
        (
            "AE_SDD_CAPABILITY_TOKEN".to_owned(),
            "capability".to_owned(),
        ),
    ]);
    move |name| values.get(name).cloned()
}

fn adapted(command: &str, business: &[&str]) -> ae_sdd_protocol::RequestParams<serde_json::Value> {
    let route = resolve_command_id(command).expect("known route");
    let mut arguments = vec![
        "--workspace-id".to_owned(),
        "00000000-0000-0000-0000-000000000001".to_owned(),
        "--work-item".to_owned(),
        "STORY-1".to_owned(),
        "--session-id".to_owned(),
        "00000000-0000-0000-0000-000000000002".to_owned(),
    ];
    arguments.extend(business.iter().map(|value| (*value).to_owned()));
    let invocation = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments,
        environment(),
    )
    .unwrap_or_else(|error| panic!("{command}: {error}"));
    let LegacyRequestSource::Synthesized(mut params) = invocation.request else {
        panic!("synthesized request")
    };
    let LegacyTarget::Rpc {
        adapter: LegacyRpcAdapter::TypedOperation { operation },
        ..
    } = &route.target
    else {
        panic!("typed operation route")
    };
    adapt_typed_operation_request(operation, &mut params)
        .unwrap_or_else(|error| panic!("{command}: {error}"));
    *params
}

#[test]
fn all_thirteen_typed_routes_build_registry_valid_operation_envelopes() {
    let cases: [(&str, &[&str]); 13] = [
        ("doc resolve", &["--intent", "STORY"]),
        (
            "doc save",
            &[
                "--intent",
                "STORY",
                "--doc-id",
                "STORY-1",
                "--content-file",
                "draft.md",
                "--lease-id",
                "00000000-0000-0000-0000-000000000003",
                "--fencing-token",
                "1",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "doc-save-1",
            ],
        ),
        (
            "evidence finalize",
            &[
                "--lease-id",
                "00000000-0000-0000-0000-000000000003",
                "--fencing-token",
                "1",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "evidence-finalize-1",
            ],
        ),
        (
            "evidence record",
            &[
                "--artifact-path",
                "evidence.json",
                "--input-fingerprint",
                "input-1",
                "--lease-id",
                "00000000-0000-0000-0000-000000000003",
                "--fencing-token",
                "1",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "evidence-record-1",
            ],
        ),
        ("gates check", &["--gate-ids", "[]"]),
        (
            "lease acquire",
            &[
                "--owner",
                "{\"role\":\"root\"}",
                "--ttl-seconds",
                "300",
                "--idempotency-key",
                "lease-acquire-1",
            ],
        ),
        (
            "lease break",
            &[
                "--actor",
                "{\"role\":\"admin\"}",
                "--reason",
                "recovery",
                "--idempotency-key",
                "lease-break-1",
                "--confirmation-id",
                "confirmation-1",
                "--approved-by",
                "user:test",
                "--approved-at",
                "2026-07-23T00:00:00Z",
            ],
        ),
        (
            "lease release",
            &[
                "--owner",
                "{\"role\":\"root\"}",
                "--lease-id",
                "00000000-0000-0000-0000-000000000003",
                "--fencing-token",
                "1",
                "--idempotency-key",
                "lease-release-1",
            ],
        ),
        (
            "lease renew",
            &[
                "--owner",
                "{\"role\":\"root\"}",
                "--ttl-seconds",
                "600",
                "--lease-id",
                "00000000-0000-0000-0000-000000000003",
                "--fencing-token",
                "1",
                "--idempotency-key",
                "lease-renew-1",
            ],
        ),
        ("lease status", &[]),
        ("state next-step", &[]),
        ("state read", &[]),
        (
            "verify plan",
            &[
                "--payload-json",
                VERIFICATION_PAYLOAD,
                "--lease-id",
                "00000000-0000-0000-0000-000000000003",
                "--fencing-token",
                "1",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "verification-plan-1",
            ],
        ),
    ];

    for (command, arguments) in cases {
        let params = adapted(command, arguments);
        assert!(params.payload["operation"].is_string(), "{command}");
        assert!(params.payload["payload"].is_object(), "{command}");
        assert_eq!(params.payload["dryRun"], false, "{command}");
    }
}

#[test]
fn mutation_preconditions_and_dry_run_control_fail_closed_before_ipc() {
    let route = resolve_command_id("doc save").expect("route");
    let arguments = [
        "--workspace-id",
        "00000000-0000-0000-0000-000000000001",
        "--work-item",
        "STORY-1",
        "--session-id",
        "00000000-0000-0000-0000-000000000002",
        "--intent",
        "STORY",
        "--doc-id",
        "STORY-1",
        "--content-file",
        "draft.md",
        "--idempotency-key",
        "doc-save-dry-1",
        "--dry-run",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let invocation = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments,
        environment(),
    )
    .expect("argv parses");
    let LegacyRequestSource::Synthesized(mut params) = invocation.request else {
        panic!("synthesized")
    };
    let error = adapt_typed_operation_request("document.save", &mut params)
        .expect_err("dry-run still requires lease and fencing");
    assert!(error.to_string().contains("lease-id"));
}
