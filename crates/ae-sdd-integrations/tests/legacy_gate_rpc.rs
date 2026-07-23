#[allow(dead_code, unused_imports)]
#[path = "../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;

use std::fs;
use std::sync::Arc;

use ae_sdd_domain::{AgentRole, BootId, EventStoreId};
use ae_sdd_integrations::NativeBusinessAdapter;
use ae_sdd_protocol::{PROTOCOL_VERSION_V1, RequestParams, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, MemoryPersistence, PersistencePort,
};
use legacy::{
    LegacyRequestSource, adapt_passthrough_request, parse_rpc_invocation, resolve_command_id,
    validate_passthrough_result,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

const WORK_ITEM: &str = "STORY-AE-SDD-RUST-DAEMON-001";
const VALID_RA: &str = include_str!("../../../ae-sdd-doc/RA/RA-AE-SDD-RUST-DAEMON-001.md");

fn setup(coding_source: &str) -> (TempDir, NativeBusinessAdapter, BusinessWorkspace) {
    setup_with_ra(coding_source, VALID_RA)
}

fn setup_with_ra(
    coding_source: &str,
    ra_source: &str,
) -> (TempDir, NativeBusinessAdapter, BusinessWorkspace) {
    let root = TempDir::new().expect("workspace tempdir");
    let state_directory = root.path().join(".auto-engineering/legacy-gate");
    fs::create_dir_all(&state_directory).expect("state directory");
    fs::write(
        state_directory.join("state.json"),
        serde_json::to_vec_pretty(&json!({
            "stateMachineName":"PRD-LEGACY-GATE",
            "activeStory":WORK_ITEM,
            "revision":1,
            "lastFencingToken":0,
            "scale":"large",
            "selectedDesign":"DR",
            "currentPhase":"requirement-analyzed",
            "storyStates":{
                (WORK_ITEM):{"currentPhase":"requirement-analyzed"}
            }
        }))
        .expect("state JSON"),
    )
    .expect("state write");
    let source = root.path().join("src/lib.rs");
    fs::create_dir_all(source.parent().expect("source parent")).expect("source directory");
    fs::write(source, coding_source).expect("source write");
    let ra = root
        .path()
        .join("ae-sdd-doc/RA/RA-AE-SDD-RUST-DAEMON-001.md");
    fs::create_dir_all(ra.parent().expect("RA parent")).expect("RA directory");
    fs::write(ra, ra_source).expect("RA write");
    fs::write(root.path().join("Cargo.toml"), "[workspace]\n").expect("Cargo.toml");

    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(900));
    let persistence = Arc::new(MemoryPersistence::new(event_store_id));
    let port: Arc<dyn PersistencePort> = persistence;
    let adapter = NativeBusinessAdapter::new(
        root.path().join("runtime.sqlite3"),
        event_store_id,
        BootId::from_uuid(Uuid::from_u128(901)),
        ae_sdd_policy::policy_digest().to_string(),
        port,
    );
    let workspace = BusinessWorkspace {
        workspace_id: Uuid::from_u128(902).to_string(),
        canonical_root: root.path().to_string_lossy().into_owned(),
        project_key: "legacy-gate".to_owned(),
        mode: WorkspaceMode::RustCanary,
        agent_role: Some(AgentRole::Root),
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 1,
    };
    (root, adapter, workspace)
}

fn adapted_gate_params(
    command_id: &str,
    root: &TempDir,
    workspace: &BusinessWorkspace,
) -> RequestParams<Value> {
    let route = resolve_command_id(command_id).expect("known command");
    let arguments = [
        "--workspace-id".to_owned(),
        workspace.workspace_id.clone(),
        "--agent-id".to_owned(),
        "root-agent".to_owned(),
        "--session-id".to_owned(),
        Uuid::from_u128(903).to_string(),
        "--capability-token".to_owned(),
        "test-capability".to_owned(),
        "--work-item".to_owned(),
        WORK_ITEM.to_owned(),
        if command_id.starts_with("gate ") {
            "--project".to_owned()
        } else {
            "--root".to_owned()
        },
        root.path().to_string_lossy().into_owned(),
    ];
    let invocation = parse_rpc_invocation(&route, RpcMethod::GateEvaluate, &arguments, |_| None)
        .expect("legacy Gate argv parses");
    let LegacyRequestSource::Synthesized(mut params) = invocation.request else {
        panic!("synthesized params")
    };
    adapt_passthrough_request(command_id, RpcMethod::GateEvaluate, &mut params)
        .expect("legacy Gate adapts");
    *params
}

fn adapted_rpc_params(
    command_id: &str,
    method: RpcMethod,
    mut arguments: Vec<String>,
) -> RequestParams<Value> {
    let route = resolve_command_id(command_id).expect("known command");
    arguments.extend([
        "--agent-id".to_owned(),
        "root-agent".to_owned(),
        "--session-id".to_owned(),
        Uuid::from_u128(903).to_string(),
        "--capability-token".to_owned(),
        "test-capability".to_owned(),
    ]);
    let invocation =
        parse_rpc_invocation(&route, method, &arguments, |_| None).expect("legacy RPC argv parses");
    let LegacyRequestSource::Synthesized(mut params) = invocation.request else {
        panic!("synthesized params")
    };
    adapt_passthrough_request(command_id, method, &mut params).expect("legacy RPC adapts");
    *params
}

fn direct_params(payload: Value, workspace: &BusinessWorkspace) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(workspace.workspace_id.clone()),
        agent_id: Some("root-agent".to_owned()),
        session_id: None,
        capability_token: None,
        turn_id: None,
        work_item_id: Some(WORK_ITEM.to_owned()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: None,
        confirmation: None,
        deadline_ms: 10_000,
        payload,
    }
}

#[test]
fn six_legacy_gate_entrypoints_reach_native_gate_runtime_and_pass() {
    let (root, adapter, workspace) = setup("pub fn answer() -> u32 { 42 }\n");
    for command_id in [
        "flow-violation-scan",
        "gate coding-required",
        "gate ra-required",
        "ra-authenticity-scan",
        "ra-depth-scan",
        "ra-implementation-scan",
    ] {
        let params = adapted_gate_params(command_id, &root, &workspace);
        let result = adapter
            .execute(RpcMethod::GateEvaluate, &params, Some(&workspace))
            .unwrap_or_else(|error| panic!("{command_id}: {error:?}"));
        validate_passthrough_result(command_id, RpcMethod::GateEvaluate, &result)
            .unwrap_or_else(|error| panic!("{command_id}: {error}; result={result}"));
    }
}

#[test]
fn coding_gate_failure_remains_typed_non_pass_and_cli_failure() {
    let (_root, adapter, workspace) = setup("const TOKEN: &str = \"secret-value\";\n");
    let result = adapter
        .execute(
            RpcMethod::GateEvaluate,
            &direct_params(json!({"gateId":"G-CODE-1"}), &workspace),
            Some(&workspace),
        )
        .expect("Gate evaluation returns typed outcome");

    assert_eq!(result["outcome"]["kind"], "FAIL");
    assert!(
        validate_passthrough_result("gate coding-required", RpcMethod::GateEvaluate, &result,)
            .is_err(),
        "non-PASS must become a non-zero legacy CLI result"
    );
}

#[test]
fn each_ra_gate_alias_preserves_a_real_non_pass_outcome() {
    let (root, adapter, workspace) = setup_with_ra(
        "pub fn answer() -> u32 { 42 }\n",
        "# RequirementAnalysisModel\n## Gap\netc.\n",
    );
    for command_id in [
        "flow-violation-scan",
        "gate ra-required",
        "ra-authenticity-scan",
        "ra-depth-scan",
        "ra-implementation-scan",
    ] {
        let params = adapted_gate_params(command_id, &root, &workspace);
        let result = adapter
            .execute(RpcMethod::GateEvaluate, &params, Some(&workspace))
            .unwrap_or_else(|error| panic!("{command_id}: {error:?}"));
        assert!(
            validate_passthrough_result(command_id, RpcMethod::GateEvaluate, &result).is_err(),
            "{command_id} must preserve non-PASS: {result}"
        );
    }
}

#[test]
fn gate_payload_rejects_unknown_fields_and_wrong_project_assertion() {
    let (root, adapter, workspace) = setup("pub fn answer() -> u32 { 42 }\n");
    let unknown = adapter
        .execute(
            RpcMethod::GateEvaluate,
            &direct_params(json!({"gateId":"G-CODE-1","ignored":true}), &workspace),
            Some(&workspace),
        )
        .expect_err("unknown Gate payload fields fail closed");
    assert_eq!(
        unknown.code(),
        ae_sdd_protocol::StableErrorCode::OperationSchemaInvalid
    );
    let duplicate = adapter
        .execute(
            RpcMethod::GateEvaluate,
            &direct_params(json!({"gateIds":["G-CODE-1","G-CODE-1"]}), &workspace),
            Some(&workspace),
        )
        .expect_err("duplicate Gate batch fails closed");
    assert_eq!(
        duplicate.code(),
        ae_sdd_protocol::StableErrorCode::OperationSchemaInvalid
    );

    let other = TempDir::new().expect("other root");
    let mismatch = adapter
        .execute(
            RpcMethod::GateEvaluate,
            &direct_params(
                json!({
                    "gateId":"G-CODE-1",
                    "expectedProjectRoot":other.path().to_string_lossy()
                }),
                &workspace,
            ),
            Some(&workspace),
        )
        .expect_err("wrong project assertion fails closed");
    assert_eq!(
        mismatch.code(),
        ae_sdd_protocol::StableErrorCode::ProjectMismatch
    );
    assert!(root.path().is_dir());
}

#[test]
fn ops_describe_filter_and_ops_next_reach_authoritative_business_runtime() {
    let (root, adapter, workspace) = setup("pub fn answer() -> u32 { 42 }\n");
    let described = adapter
        .execute(
            RpcMethod::OperationDescribe,
            &adapted_rpc_params(
                "ops describe",
                RpcMethod::OperationDescribe,
                vec!["--operation".to_owned(), "state.next_actions".to_owned()],
            ),
            None,
        )
        .expect("filtered operation registry");
    assert_eq!(described.as_array().map(Vec::len), Some(1));
    assert_eq!(described[0]["operation"], "state.next_actions");

    let next = adapter
        .execute(
            RpcMethod::FlowNext,
            &adapted_rpc_params(
                "ops next",
                RpcMethod::FlowNext,
                vec![
                    "--workspace-id".to_owned(),
                    workspace.workspace_id.clone(),
                    "--work-item".to_owned(),
                    WORK_ITEM.to_owned(),
                    "--project".to_owned(),
                    root.path().to_string_lossy().into_owned(),
                    "--story".to_owned(),
                    WORK_ITEM.to_owned(),
                ],
            ),
            Some(&workspace),
        )
        .expect("authoritative next action");
    assert_eq!(next["phase"], "requirement-analyzed");
    assert!(next["nextAction"]["kind"].is_string());
}

#[test]
fn operation_filter_and_flow_story_assertions_fail_closed() {
    let (root, adapter, workspace) = setup("pub fn answer() -> u32 { 42 }\n");
    let unknown = adapter
        .execute(
            RpcMethod::OperationDescribe,
            &direct_params(json!({"operation":"state.patch"}), &workspace),
            None,
        )
        .expect_err("unknown operation filter fails closed");
    assert_eq!(
        unknown.code(),
        ae_sdd_protocol::StableErrorCode::OperationNotRegistered
    );

    let mismatch = adapter
        .execute(
            RpcMethod::FlowNext,
            &adapted_rpc_params(
                "ops next",
                RpcMethod::FlowNext,
                vec![
                    "--workspace-id".to_owned(),
                    workspace.workspace_id.clone(),
                    "--work-item".to_owned(),
                    WORK_ITEM.to_owned(),
                    "--project".to_owned(),
                    root.path().to_string_lossy().into_owned(),
                    "--story".to_owned(),
                    "STORY-WRONG".to_owned(),
                ],
            ),
            Some(&workspace),
        )
        .expect_err("wrong story assertion fails closed");
    assert_eq!(
        mismatch.code(),
        ae_sdd_protocol::StableErrorCode::ProjectMismatch
    );
}
