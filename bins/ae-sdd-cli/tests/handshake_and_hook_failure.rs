use std::process::Command;

use serde_json::Value;

#[test]
fn cli_hook_emits_fail_closed_host_decision_when_manifest_is_unavailable() {
    let request = serde_json::json!({
        "params": {
            "protocolVersion":"1.0",
            "workspaceId":"workspace",
            "agentId":"agent",
            "sessionId":"00000000-0000-0000-0000-000000000003",
            "turnId":"turn",
            "workItemId":"WORK",
            "deadlineMs":250,
            "payload": {
                "hookEventId":"event",
                "turnSeq":1,
                "hostPayload":{}
            }
        },
        "engaged":true,
        "offlineCapability":"not-a-valid-token",
        "nowUnixMs":1
    });
    let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd"))
        .args([
            "hook",
            "--method",
            "hook.pre_tool",
            "--request-json",
            &serde_json::to_string(&request).expect("Hook request serializes"),
            "--manifest",
            "C:/path/that/does/not/exist/ae-sdd-endpoint.json",
            "--timeout-ms",
            "250",
        ])
        .output()
        .expect("CLI process starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("CLI emits JSON");
    assert_eq!(value["decision"], "deny");
    assert_eq!(value["offline"], true);
    assert_eq!(value["engaged"], false);
}

#[test]
fn cli_accepts_native_host_json_and_blocks_stop_without_trusted_binding() {
    let host_event = serde_json::json!({
        "hook_event_name":"Stop",
        "last_assistant_message":"bounded host message"
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd"));
    command.args([
        "hook",
        "--method",
        "hook.stop",
        "--request-json",
        &serde_json::to_string(&host_event).expect("host event serializes"),
        "--manifest",
        "C:/path/that/does/not/exist/ae-sdd-endpoint.json",
    ]);
    for name in [
        "AE_SDD_WORKSPACE_ID",
        "AE_SDD_AGENT_ID",
        "AE_SDD_SESSION_ID",
        "AE_SDD_CAPABILITY_TOKEN",
        "AE_SDD_TURN_ID",
        "AE_SDD_WORK_ITEM_ID",
    ] {
        command.env_remove(name);
    }
    let output = command.output().expect("CLI process starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("CLI emits host JSON");
    assert_eq!(value["decision"], "block");
    assert!(
        value["reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty())
    );
}

/// ROUTE-702d576a Task 2 (Host payload RED): a real `SubagentStart` payload
/// missing its `agent_id` must fail closed rather than being treated as an
/// ordinary host event with a synthesized identity. `agent_id` is the only
/// field the host mints unforgeably (crypto.randomBytes in-process); a
/// missing one means the event cannot be correlated at all.
#[test]
fn cli_rejects_subagent_start_missing_agent_id() {
    let host_event = serde_json::json!({
        "hook_event_name":"SubagentStart",
        "session_id":"11111111-1111-1111-1111-111111111111",
        "agent_type":"general-purpose"
    });
    let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd"))
        .args([
            "hook",
            "--method",
            "hook.subagent_start",
            "--request-json",
            &serde_json::to_string(&host_event).expect("host event serializes"),
            "--manifest",
            "C:/path/that/does/not/exist/ae-sdd-endpoint.json",
            "--timeout-ms",
            "250",
        ])
        .output()
        .expect("CLI process starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("CLI emits host JSON");
    assert_eq!(value["decision"], "deny");
    assert!(
        value["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("agent_id") || reason.contains("agentId"))
    );
}

/// A `SubagentStart` payload missing `session_id` (parent correlation) must
/// also fail closed: without it the daemon cannot bind the child to the
/// parent root session's host-execution binding (Plan §0.6 Q6).
#[test]
fn cli_rejects_subagent_start_missing_session_id() {
    let host_event = serde_json::json!({
        "hook_event_name":"SubagentStart",
        "agent_id":"a1234567890abcde",
        "agent_type":"general-purpose"
    });
    let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd"))
        .args([
            "hook",
            "--method",
            "hook.subagent_start",
            "--request-json",
            &serde_json::to_string(&host_event).expect("host event serializes"),
            "--manifest",
            "C:/path/that/does/not/exist/ae-sdd-endpoint.json",
            "--timeout-ms",
            "250",
        ])
        .output()
        .expect("CLI process starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("CLI emits host JSON");
    assert_eq!(value["decision"], "deny");
    assert!(
        value["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("session_id") || reason.contains("sessionId"))
    );
}

/// A `SubagentStart` payload carrying every required field passes shape
/// validation, proving the two rejects above are about missing fields and
/// not about the method name itself being unrecognized. The CLI still
/// reports `decision: "deny"` -- this is not a daemon-reachability failure
/// (the manifest path is never consulted for this method) but a permanent
/// design boundary: `hook.subagent_start` validates event shape only, and
/// never originates `delegation.accept` or `session.open` itself, because a
/// `SubagentStart` payload never carries the daemon-issued `claimId` that
/// those calls require (design doc §9.3: `Host`, i.e. root's own connection
/// in the A2 model, receives the claim first; the child cannot originate
/// its own claim consumption).
#[test]
fn cli_accepts_well_formed_subagent_start_payload_shape() {
    let host_event = serde_json::json!({
        "hook_event_name":"SubagentStart",
        "session_id":"11111111-1111-1111-1111-111111111111",
        "agent_id":"a1234567890abcde",
        "agent_type":"general-purpose",
        "cwd":"D:/Item/ae-sdd"
    });
    let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd"))
        .args([
            "hook",
            "--method",
            "hook.subagent_start",
            "--request-json",
            &serde_json::to_string(&host_event).expect("host event serializes"),
            "--manifest",
            "C:/path/that/does/not/exist/ae-sdd-endpoint.json",
            "--timeout-ms",
            "250",
        ])
        .output()
        .expect("CLI process starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("CLI emits host JSON");
    assert_eq!(value["decision"], "deny");
    assert_eq!(
        value["reason"],
        "hook.subagent_start validates event shape only; delegation.accept and session.open \
         are root-side orchestration and are not performed by this hook"
    );
}
