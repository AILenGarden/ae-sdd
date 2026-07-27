//! AC-C08 coverage for the runtime-to-session-bootstrap production wiring.

mod support;

use ae_sdd_protocol::{ClientKind, RequestParams, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{
    PersistencePort, RuntimeConfig, RuntimeIdentityKind, RuntimeIdentityTransition, SessionResult,
    WorkspaceResult,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use support::{Harness, params, register_workspace, session_params, stable_error};

fn session_open_params(
    workspace: &WorkspaceResult,
    external_key: &str,
    idempotency_key: &str,
) -> RequestParams<Value> {
    let mut request = params(
        json!({
            "externalKey": external_key,
            "role": "root",
            "engaged": false,
        }),
        1_000,
    );
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.agent_id = Some("agent-session-bootstrap".to_owned());
    request.idempotency_key = Some(idempotency_key.to_owned());
    request
}

fn session_result(response: &Value) -> &Value {
    response
        .get("result")
        .unwrap_or_else(|| panic!("expected session.open success: {response}"))
}

fn bootstrap_plan_digest(response: &Value) -> &str {
    session_result(response)["bootstrapPlanDigest"]
        .as_str()
        .unwrap_or_else(|| panic!("session receipt lacks bootstrapPlanDigest: {response}"))
}

#[test]
fn existing_external_session_cannot_be_reused_across_workspaces() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let first_workspace = register_workspace(&harness, &mut connection, "bootstrap-first");
    let second_workspace = register_workspace(&harness, &mut connection, "bootstrap-second");

    let first = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&first_workspace, "shared-host-session", "open-first"),
    );
    assert!(first.get("result").is_some(), "{first}");

    let cross_workspace = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&second_workspace, "shared-host-session", "open-second"),
    );

    assert_eq!(stable_error(&cross_workspace), "PROJECT_MISMATCH");
}

#[test]
fn recovered_duplicate_external_session_is_rejected_even_for_an_exact_workspace_match() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let first_workspace = register_workspace(&harness, &mut connection, "bootstrap-recover-first");
    let second_workspace =
        register_workspace(&harness, &mut connection, "bootstrap-recover-second");
    let external_key = "recovered-duplicate-host-session";

    let opened = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&first_workspace, external_key, "open-before-recover"),
    );
    assert!(opened.get("result").is_some(), "{opened}");

    let mut duplicate = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("durable sessions")
        .into_iter()
        .find(|snapshot| {
            snapshot
                .session
                .as_ref()
                .is_some_and(|record| record.workspace_id == first_workspace.workspace_id)
        })
        .expect("opened session is durable");
    let duplicate_session_id = Uuid::new_v4().to_string();
    duplicate.workspace = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Workspace)
        .expect("durable workspaces")
        .into_iter()
        .find(|snapshot| snapshot.workspace.workspace_id == second_workspace.workspace_id)
        .expect("second workspace is durable")
        .workspace;
    let duplicate_session = duplicate.session.as_mut().expect("session row");
    duplicate_session.workspace_id = second_workspace.workspace_id.clone();
    duplicate_session.session_id = duplicate_session_id;
    duplicate.response = json!({"fixture":"cross-workspace-duplicate"});
    harness
        .persistence
        .commit_identity_bundle(RuntimeIdentityTransition {
            operation: "test.session.inject".to_owned(),
            scope_digest: "b".repeat(64),
            idempotency_key: "duplicate-session".to_owned(),
            request_digest: "c".repeat(64),
            expected_workspace_mode: None,
            expected_inventory_generation: None,
            expected_session_status: None,
            expected_delegation_status: None,
            expected_context_generation: None,
            snapshot: duplicate,
            committed_at_unix_ms: 1_000,
        })
        .expect("legacy duplicate session fixture");

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        harness.persistence.clone(),
        12,
        "recovered-bootstrap-token".to_owned(),
    );
    recovered.runtime.recover().expect("runtime recovery");
    let mut recovered_connection = recovered.connection(ClientKind::Cli);
    let response = recovered.call(
        &mut recovered_connection,
        RpcMethod::SessionOpen,
        session_open_params(&first_workspace, external_key, "open-after-recover"),
    );

    assert_eq!(stable_error(&response), "PROJECT_MISMATCH");
}

#[test]
fn legal_bootstrap_digest_is_deterministic_and_external_key_replay_is_unchanged() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "bootstrap-digest");

    let initial = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-initial"),
    );
    let initial_digest = bootstrap_plan_digest(&initial);
    assert_eq!(initial_digest.len(), 64);
    assert!(initial_digest.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let repeated = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-repeat"),
    );
    let same_input = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-same-input"),
    );

    assert_eq!(
        session_result(&initial)["sessionId"],
        session_result(&repeated)["sessionId"],
        "an existing external key must continue to reopen the same session"
    );
    assert_eq!(
        bootstrap_plan_digest(&repeated),
        bootstrap_plan_digest(&same_input),
        "the same request and bootstrap snapshot must have a byte-identical digest"
    );

    harness.clock.set(1_250);
    let idempotent_replay = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-repeat"),
    );
    assert_eq!(
        repeated, idempotent_replay,
        "the existing session.open idempotency receipt must replay unchanged"
    );
}

#[test]
fn heartbeat_replay_restores_the_receipt_expiry_and_capability() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "heartbeat-replay");
    let opened = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "heartbeat-host-session", "open-heartbeat"),
    );
    let opened: SessionResult =
        serde_json::from_value(session_result(&opened).clone()).expect("session.open result");

    let mut first_request = session_params(
        &workspace,
        &opened,
        "agent-session-bootstrap",
        json!({}),
        2_000,
    );
    first_request.idempotency_key = Some("heartbeat-stable".to_owned());
    let first = harness.call(&mut connection, RpcMethod::SessionHeartbeat, first_request);
    let first_session: SessionResult =
        serde_json::from_value(session_result(&first).clone()).expect("heartbeat result");

    harness.clock.set(1_250);
    let mut replay_request = session_params(
        &workspace,
        &first_session,
        "agent-session-bootstrap",
        json!({}),
        2_000,
    );
    replay_request.idempotency_key = Some("heartbeat-stable".to_owned());
    let replay = harness.call(&mut connection, RpcMethod::SessionHeartbeat, replay_request);

    assert_eq!(first, replay);
    let durable = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("typed session snapshots")
        .into_iter()
        .find_map(|snapshot| snapshot.session)
        .expect("durable session");
    assert_eq!(durable.expires_at_unix_ms, first_session.expires_at_unix_ms);
}

#[test]
fn new_boot_reuses_session_id_but_issues_only_a_current_boot_capability() {
    let first = Harness::new(RuntimeConfig::default());
    let mut first_connection = first.connection(ClientKind::Cli);
    let workspace = register_workspace(&first, &mut first_connection, "bootstrap-new-boot");
    let initial = harness_session_open(
        &first,
        &mut first_connection,
        &workspace,
        "stable-across-boots",
        "open-first-boot",
    );

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        first.persistence.clone(),
        99,
        "second-boot-token".to_owned(),
    );
    second.runtime.recover().expect("typed identity recovers");
    let mut second_connection = second.connection(ClientKind::Cli);
    let reopened = harness_session_open(
        &second,
        &mut second_connection,
        &workspace,
        "stable-across-boots",
        "open-second-boot",
    );

    assert_eq!(initial["sessionId"], reopened["sessionId"]);
    assert_ne!(initial["capabilityToken"], reopened["capabilityToken"]);
}

#[test]
fn recovery_imports_legacy_root_identity_without_persisting_secrets() {
    let seeded = Harness::new(RuntimeConfig::default());
    let workspace = WorkspaceResult {
        workspace_id: Uuid::new_v4().to_string(),
        canonical_root: "C:/ae-sdd-tests/legacy-import".to_owned(),
        project_key: "project-legacy-import".to_owned(),
        mode: WorkspaceMode::Shadow,
        inventory_generation: 7,
    };
    let session_id = Uuid::new_v4().to_string();
    let external_key = "legacy-physical-session-secret";
    let legacy_capability = "legacy-boot-capability-secret";
    seeded
        .persistence
        .store_record(
            "workspace/v1",
            &workspace.workspace_id,
            &serde_json::to_value(&workspace).expect("legacy workspace serializes"),
        )
        .expect("seed legacy workspace");
    seeded
        .persistence
        .store_record(
            "session/v1",
            &session_id,
            &json!({
                "workspaceId": workspace.workspace_id,
                "agentId": "agent-session-bootstrap",
                "externalKey": external_key,
                "currentWorkItem": null,
                "result": {
                    "sessionId": session_id,
                    "role": "root",
                    "engaged": false,
                    "expiresAtUnixMs": 9_000,
                    "contextGeneration": 4,
                    "capabilityToken": legacy_capability,
                },
                "delegationId": null,
                "currentTurnId": null,
                "currentTurnSeq": 0,
                "active": true,
            }),
        )
        .expect("seed legacy root session");

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        seeded.persistence.clone(),
        101,
        "legacy-import-current-boot-token".to_owned(),
    );
    recovered
        .runtime
        .recover()
        .expect("legacy identity recovers");

    let typed_workspaces = recovered
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Workspace)
        .expect("typed workspaces");
    assert_eq!(typed_workspaces.len(), 1);
    assert_eq!(
        typed_workspaces[0].workspace.workspace_id,
        workspace.workspace_id
    );
    let typed_sessions = recovered
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("typed sessions");
    assert_eq!(typed_sessions.len(), 1);
    let typed_session = typed_sessions[0]
        .session
        .as_ref()
        .expect("typed session row");
    assert_eq!(typed_session.session_id, session_id);
    assert_eq!(
        typed_session.external_key_hash,
        hex::encode(Sha256::digest(external_key.as_bytes()))
    );
    let typed_receipt = serde_json::to_string(&typed_sessions[0]).expect("receipt serializes");
    assert!(!typed_receipt.contains(external_key));
    assert!(!typed_receipt.contains(legacy_capability));
    assert!(typed_sessions[0].response.get("capabilityToken").is_none());

    let mut connection = recovered.connection(ClientKind::Cli);
    let reopened = harness_session_open(
        &recovered,
        &mut connection,
        &workspace,
        external_key,
        "open-imported-legacy-session",
    );
    assert_eq!(reopened["sessionId"], session_id);
    assert_ne!(reopened["capabilityToken"], legacy_capability);
    assert!(
        reopened["capabilityToken"]
            .as_str()
            .is_some_and(|token| !token.is_empty())
    );
}

fn harness_session_open(
    harness: &Harness,
    connection: &mut ae_sdd_runtime::ConnectionState,
    workspace: &WorkspaceResult,
    external_key: &str,
    idempotency_key: &str,
) -> Value {
    session_result(&harness.call(
        connection,
        RpcMethod::SessionOpen,
        session_open_params(workspace, external_key, idempotency_key),
    ))
    .clone()
}
