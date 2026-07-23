#[allow(dead_code, unused_imports)]
#[path = "../src/legacy/mod.rs"]
mod legacy;

use std::collections::BTreeMap;
use std::path::PathBuf;

use ae_sdd_protocol::RpcMethod;
use legacy::{
    LegacyNativeRequestSource, LegacyRequestSource, LegacyTarget, parse_native_invocation,
    parse_rpc_invocation, resolve_command_id, verify_offline_request,
};
use serde_json::json;

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

#[test]
fn daemon_flags_and_controlled_environment_build_strict_request_params() {
    let route = resolve_command_id("lease acquire").expect("known route");
    let invocation = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments(&[
            "--workspace-id",
            "workspace-1",
            "--work-item",
            "WORK-1",
            "--session-id",
            "session-1",
            "--lease-id",
            "lease-1",
            "--fencing-token",
            "9",
            "--expected-revision",
            "12",
            "--idempotency-key",
            "request-1",
            "--deadline-ms",
            "2500",
            "--owner-agent",
            "root",
            "--ttl-seconds",
            "120",
            "--force",
        ]),
        environment(&[
            ("AE_SDD_AGENT_ID", "agent-1"),
            ("AE_SDD_CAPABILITY_TOKEN", "capability-1"),
            ("AE_SDD_MANIFEST", "C:/state/endpoint.json"),
        ]),
    )
    .expect("strict synthesized request");
    assert_eq!(
        invocation.manifest,
        Some(PathBuf::from("C:/state/endpoint.json"))
    );
    let LegacyRequestSource::Synthesized(params) = invocation.request else {
        panic!("expected synthesized request");
    };
    assert_eq!(params.workspace_id.as_deref(), Some("workspace-1"));
    assert_eq!(params.work_item_id.as_deref(), Some("WORK-1"));
    assert_eq!(params.session_id.as_deref(), Some("session-1"));
    assert_eq!(params.agent_id.as_deref(), Some("agent-1"));
    assert_eq!(params.capability_token.as_deref(), Some("capability-1"));
    assert_eq!(params.lease_id.as_deref(), Some("lease-1"));
    assert_eq!(params.fencing_token, Some(9));
    assert_eq!(params.expected_revision, Some(12));
    assert_eq!(params.deadline_ms, 2500);
    assert_eq!(
        params.payload,
        json!({"ownerAgent":"root","ttlSeconds":120,"force":true})
    );
}

#[test]
fn required_identity_duplicate_alias_and_malformed_argv_fail_closed() {
    let route = resolve_command_id("lease acquire").expect("known route");
    let base_environment = environment(&[
        ("AE_SDD_AGENT_ID", "agent-1"),
        ("AE_SDD_CAPABILITY_TOKEN", "capability-1"),
    ]);
    let missing = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments(&["--workspace-id", "workspace-1", "--work-item-id", "WORK-1"]),
        &base_environment,
    )
    .expect_err("missing session must fail");
    assert!(missing.to_string().contains("session identity"));

    let duplicate_alias = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments(&[
            "--workspace-id",
            "workspace-1",
            "--workspace",
            "workspace-2",
            "--work-item-id",
            "WORK-1",
            "--session-id",
            "session-1",
        ]),
        &base_environment,
    )
    .expect_err("aliases cannot conflict");
    assert!(duplicate_alias.to_string().contains("ambiguous aliases"));

    let repeated = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments(&["--workspace-id", "one", "--workspace-id", "two"]),
        &base_environment,
    )
    .expect_err("duplicate flags must fail");
    assert!(repeated.to_string().contains("duplicate legacy flag"));

    let positional = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments(&["unexpected-positional"]),
        &base_environment,
    )
    .expect_err("daemon routes reject positionals");
    assert!(positional.to_string().contains("only named"));
}

#[test]
fn direct_mutation_requires_idempotency_and_advanced_json_remains_available() {
    let route = resolve_command_id("automation enable").expect("known job route");
    let missing = parse_rpc_invocation(
        &route,
        RpcMethod::JobSubmit,
        &arguments(&["--workspace-id", "workspace-1"]),
        environment(&[]),
    )
    .expect_err("job submit requires idempotency");
    assert!(missing.to_string().contains("idempotency key"));

    let invocation = parse_rpc_invocation(
        &route,
        RpcMethod::JobSubmit,
        &arguments(&[
            "--request-json",
            "-",
            "--manifest",
            "C:/state/endpoint.json",
        ]),
        environment(&[]),
    )
    .expect("advanced request source");
    assert!(matches!(
        invocation.request,
        LegacyRequestSource::ExplicitJson(ref value) if value == "-"
    ));
    assert_eq!(
        invocation.manifest,
        Some(PathBuf::from("C:/state/endpoint.json"))
    );
}

#[test]
fn all_thirteen_native_routes_synthesize_typed_offline_requests() {
    let cases: [(&str, &[&str]); 13] = [
        (
            "assets generate",
            &["--project-root", "C:/project", "--project-key", "sample"],
        ),
        (
            "bump",
            &[
                "0.2.0",
                "--repository-root",
                "C:/repo",
                "--expected-version",
                "0.1.0",
            ],
        ),
        (
            "distributor disable",
            &["codex", "--registry-file", "C:/home/distributors.json"],
        ),
        (
            "distributor enable",
            &["codex", "--registry-file", "C:/home/distributors.json"],
        ),
        (
            "distributor list",
            &["--registry-file", "C:/home/distributors.json"],
        ),
        (
            "distributor register",
            &[
                "codex",
                "--protocol",
                "copytree",
                "--target-path",
                "C:/agents/codex",
                "--registry-file",
                "C:/home/distributors.json",
            ],
        ),
        (
            "distributor scan",
            &["--registry-file", "C:/home/distributors.json"],
        ),
        (
            "distributor unregister",
            &["codex", "--registry-file", "C:/home/distributors.json"],
        ),
        ("init", &["C:/project", "sample", "--force"]),
        (
            "init-hooks",
            &[
                "C:/project",
                "--executable",
                "C:/bin/ae-sdd.exe",
                "--hosts",
                "claude,codex",
            ],
        ),
        (
            "plugin init",
            &[
                "--plugins-root",
                "C:/plugins",
                "--name",
                "sample-plugin",
                "--description",
                "Sample plugin",
            ],
        ),
        ("runtime verify", &["--path", "C:/package"]),
        ("version", &[]),
    ];
    for (command_id, argv) in cases {
        let route = resolve_command_id(command_id).expect("known native route");
        let LegacyTarget::NativeBuildJob { entrypoint, .. } = &route.target else {
            panic!("{command_id} must remain native");
        };
        let invocation = parse_native_invocation(
            &route,
            entrypoint,
            &arguments(argv),
            environment(&[("AE_SDD_ACTOR", "test-agent")]),
        )
        .unwrap_or_else(|error| panic!("{command_id}: {error}"));
        let LegacyNativeRequestSource::Generated(request) = invocation.request else {
            panic!("{command_id} should synthesize a request");
        };
        verify_offline_request(entrypoint, &request).expect("route-bound request");
        assert_eq!(request["schemaVersion"], "ae-sdd-offline-build/v1");
        assert_eq!(request["command"], entrypoint.as_str());
        assert_eq!(request["actor"], "test-agent");
        assert!(
            !serde_json::to_string(&request)
                .expect("request JSON")
                .to_ascii_lowercase()
                .contains("python")
        );
    }
}

#[test]
fn native_adapter_rejects_unknown_duplicate_python_and_route_redirection() {
    let route = resolve_command_id("init-hooks").expect("known native route");
    let LegacyTarget::NativeBuildJob { entrypoint, .. } = &route.target else {
        panic!("native route");
    };
    let unknown = parse_native_invocation(
        &route,
        entrypoint,
        &arguments(&["C:/project", "--unknown-flag", "value"]),
        environment(&[]),
    )
    .expect_err("unknown flags fail closed");
    assert!(unknown.to_string().contains("unknown flag"));

    let duplicate = parse_native_invocation(
        &route,
        entrypoint,
        &arguments(&["C:/project", "--hosts", "claude", "--hosts", "codex"]),
        environment(&[]),
    )
    .expect_err("duplicate flags fail closed");
    assert!(duplicate.to_string().contains("duplicate legacy flag"));

    let python = parse_native_invocation(
        &route,
        entrypoint,
        &arguments(&["C:/project", "--use-python"]),
        environment(&[]),
    )
    .expect_err("Python path is forbidden");
    assert!(python.to_string().contains("removed"));

    let redirected = json!({"schemaVersion":"ae-sdd-offline-build/v1","command":"version"});
    assert!(verify_offline_request(entrypoint, &redirected).is_err());
}

#[test]
fn explicit_native_request_file_is_retained_as_an_advanced_path() {
    let route = resolve_command_id("version").expect("known native route");
    let LegacyTarget::NativeBuildJob { entrypoint, .. } = &route.target else {
        panic!("native route");
    };
    let invocation = parse_native_invocation(
        &route,
        entrypoint,
        &arguments(&["--request", "C:/requests/version.json", "--json"]),
        environment(&[]),
    )
    .expect("advanced request path");
    assert!(invocation.output_json);
    assert!(matches!(
        invocation.request,
        LegacyNativeRequestSource::ExplicitFile(ref path)
            if path == &PathBuf::from("C:/requests/version.json")
    ));
}

#[test]
fn bump_infers_the_product_version_from_the_selected_repository() {
    let route = resolve_command_id("bump").expect("known native route");
    let LegacyTarget::NativeBuildJob { entrypoint, .. } = &route.target else {
        panic!("native route");
    };
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root");
    let invocation = parse_native_invocation(
        &route,
        entrypoint,
        &arguments(&[
            "3.15.0",
            "--repository-root",
            &repository.display().to_string(),
            "--dry-run",
        ]),
        environment(&[]),
    )
    .expect("bump request");
    let LegacyNativeRequestSource::Generated(request) = invocation.request else {
        panic!("generated request");
    };
    assert_eq!(request["expectedVersion"], "3.14.0");
    assert_eq!(request["newVersion"], "3.15.0");
}

#[test]
fn generated_idempotency_ignores_output_only_flags() {
    let route = resolve_command_id("version").expect("known native route");
    let LegacyTarget::NativeBuildJob { entrypoint, .. } = &route.target else {
        panic!("native route");
    };
    let parse = |argv: &[&str]| {
        let invocation =
            parse_native_invocation(&route, entrypoint, &arguments(argv), environment(&[]))
                .expect("version request");
        let LegacyNativeRequestSource::Generated(request) = invocation.request else {
            panic!("generated request");
        };
        request
    };
    let plain = parse(&[]);
    let json_output = parse(&["--json"]);
    assert_eq!(plain["idempotencyKey"], json_output["idempotencyKey"]);
}
