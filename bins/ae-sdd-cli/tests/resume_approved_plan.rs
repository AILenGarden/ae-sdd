//! `ae-sdd resume-approved-plan` thin-CLI contract (plan P0 Task 12, AC-006).
//!
//! The command only normalizes identity fields, passes the caller's
//! `knownCapsuleDigest`/`knownContextRevision` cursor through an
//! `operation.execute` envelope for `execution.resume`, and renders the
//! daemon-owned `projectionKind`/`capsule`/`nextAction` projection. Without
//! a reachable daemon it must fail closed: the CLI never reads Story,
//! constraints, state or source files to fabricate a resume projection.

#[allow(dead_code, unused_imports)]
#[path = "../src/main.rs"]
mod cli_main;

use std::process::Command;

use serde_json::{Value, json};

const CAPSULE_DIGEST: &str =
    "sha256:dcc847246f3c417901a94dd303163cde29414f89c31361f8f3ae6affe5b97e64";
const MISSING_MANIFEST: &str = "C:/path/that/does/not/exist/ae-sdd-endpoint.json";

fn request_json() -> Value {
    json!({
        "workspaceId": "11111111-1111-4111-8111-111111111111",
        "agentId": "kimi:instance",
        "sessionId": "22222222-2222-4222-8222-222222222222",
        "turnId": "33333333-3333-4333-8333-333333333333",
        "capabilityToken": "capability-1",
        "workItemId": "PRD-AE-SDD-EXECUTION-EFFICIENCY-001",
        "knownCapsuleDigest": CAPSULE_DIGEST,
        "knownContextRevision": 4,
        "deadlineMs": 5_000
    })
}

fn assemble(request: &Value) -> Result<ae_sdd_protocol::RequestParams<Value>, String> {
    let decoded: cli_main::ResumeApprovedPlanRequest =
        serde_json::from_value(request.clone()).map_err(|error| error.to_string())?;
    cli_main::assemble_resume_request(&decoded)
}

fn daemon_full_response() -> Value {
    json!({
        "projectionKind": "full",
        "contextRevision": 4,
        "capsuleDigest": CAPSULE_DIGEST,
        "capsule": {"schemaVersion": 1, "activeSlice": {"ordinal": 2}},
        "nextAction": {"kind": "execute-approved-slice", "activeOrdinal": 2},
        "authorityRefreshCount": 1
    })
}

#[test]
fn assembles_operation_execute_envelope_with_cursor_passthrough() {
    let params = assemble(&request_json()).expect("valid resume request assembles");
    assert_eq!(
        params.workspace_id.as_deref(),
        Some("11111111-1111-4111-8111-111111111111")
    );
    assert_eq!(params.agent_id.as_deref(), Some("kimi:instance"));
    assert_eq!(
        params.session_id.as_deref(),
        Some("22222222-2222-4222-8222-222222222222")
    );
    assert_eq!(
        params.turn_id.as_deref(),
        Some("33333333-3333-4333-8333-333333333333")
    );
    assert_eq!(params.capability_token.as_deref(), Some("capability-1"));
    assert_eq!(
        params.work_item_id.as_deref(),
        Some("PRD-AE-SDD-EXECUTION-EFFICIENCY-001")
    );
    assert_eq!(params.deadline_ms, 5_000);
    assert!(params.lease_id.is_none(), "execution.resume is lease-free");
    assert!(params.fencing_token.is_none());
    assert!(params.expected_revision.is_none());
    assert!(params.idempotency_key.is_none());
    assert!(params.confirmation.is_none());
    assert_eq!(
        params.payload,
        json!({
            "operation": "execution.resume",
            "dryRun": false,
            "payload": {
                "knownCapsuleDigest": CAPSULE_DIGEST,
                "knownContextRevision": 4,
            }
        }),
        "CLI only assembles operation.execute for execution.resume"
    );
}

#[test]
fn omitted_cursor_fields_send_an_empty_payload_object() {
    let mut request = request_json();
    let object = request.as_object_mut().expect("request object");
    object.remove("knownCapsuleDigest");
    object.remove("knownContextRevision");
    let params = assemble(&request).expect("cursor-free request assembles");
    assert_eq!(params.payload["payload"], json!({}));
    assert_eq!(params.payload["operation"], "execution.resume");
    assert_eq!(params.payload["dryRun"], false);
}

#[test]
fn strict_request_rejects_unknown_fields_bad_types_and_missing_identity() {
    let mut unknown = request_json();
    unknown
        .as_object_mut()
        .expect("request object")
        .insert("storyPath".to_owned(), json!("ae-sdd-doc/Story/x.md"));
    assert!(
        assemble(&unknown).is_err(),
        "unknown fields must fail closed before IPC"
    );

    let mut bad_revision = request_json();
    bad_revision
        .as_object_mut()
        .expect("request object")
        .insert("knownContextRevision".to_owned(), json!("4"));
    assert!(
        assemble(&bad_revision).is_err(),
        "non-integer knownContextRevision must fail closed"
    );

    let mut missing_identity = request_json();
    missing_identity
        .as_object_mut()
        .expect("request object")
        .remove("sessionId");
    assert!(
        assemble(&missing_identity).is_err(),
        "session identity is required by the registry scope"
    );
}

#[test]
fn renders_full_and_no_change_projections() {
    let full = cli_main::render_resume_projection(&daemon_full_response()).expect("full renders");
    assert_eq!(full["projectionKind"], "full");
    assert_eq!(full["capsule"]["activeSlice"]["ordinal"], 2);
    assert_eq!(full["nextAction"]["kind"], "execute-approved-slice");
    assert_eq!(full["authorityRefreshCount"], 1);

    let mut no_change = daemon_full_response();
    let object = no_change.as_object_mut().expect("response object");
    object.insert("projectionKind".to_owned(), json!("no-change"));
    object.insert("capsule".to_owned(), Value::Null);
    let rendered = cli_main::render_resume_projection(&no_change).expect("no-change renders");
    assert_eq!(rendered["projectionKind"], "no-change");
    assert!(rendered["capsule"].is_null());
    assert_eq!(rendered["nextAction"]["kind"], "execute-approved-slice");
}

#[test]
fn renders_projection_from_the_operation_execute_receipt() {
    let response = json!({
        "changed": true,
        "data": daemon_full_response(),
        "receiptDigest": "a".repeat(64),
        "revisionBefore": 150,
        "revisionAfter": 151
    });

    let rendered = cli_main::render_resume_projection(&response)
        .expect("typed operation receipt data renders");
    assert_eq!(rendered["projectionKind"], "full");
    assert_eq!(rendered["nextAction"]["kind"], "execute-approved-slice");
}

#[test]
fn rendered_projection_exposes_exactly_the_frozen_keys() {
    let mut response = daemon_full_response();
    response
        .as_object_mut()
        .expect("response object")
        .insert("internalDebug".to_owned(), json!("drop me"));
    let rendered = cli_main::render_resume_projection(&response).expect("renders");
    let mut keys: Vec<&str> = rendered
        .as_object()
        .expect("projection object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "authorityRefreshCount",
            "capsule",
            "capsuleDigest",
            "contextRevision",
            "nextAction",
            "projectionKind"
        ]
    );
}

#[test]
fn render_fails_closed_when_daemon_response_drifts() {
    let mut drifted = daemon_full_response();
    drifted
        .as_object_mut()
        .expect("response object")
        .remove("nextAction");
    assert!(cli_main::render_resume_projection(&drifted).is_err());
    assert!(cli_main::render_resume_projection(&json!([])).is_err());
}

#[test]
fn missing_daemon_fails_closed_instead_of_fabricating_a_projection() {
    let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd"))
        .args([
            "resume-approved-plan",
            "--request",
            &serde_json::to_string(&request_json()).expect("request serializes"),
            "--manifest",
            MISSING_MANIFEST,
            "--timeout-ms",
            "250",
        ])
        .output()
        .expect("CLI process starts");
    assert!(
        !output.status.success(),
        "resume without a daemon must fail closed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("projectionKind"),
        "CLI must not fabricate a projection without the daemon: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ae-sdd:"),
        "failure must surface the client error: {stderr}"
    );
}

#[test]
fn malformed_request_is_rejected_before_any_ipc() {
    let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd"))
        .args([
            "resume-approved-plan",
            "--request",
            "not json",
            "--manifest",
            MISSING_MANIFEST,
        ])
        .output()
        .expect("CLI process starts");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ae-sdd:"),
        "malformed --request must fail with a CLI error: {stderr}"
    );
}
