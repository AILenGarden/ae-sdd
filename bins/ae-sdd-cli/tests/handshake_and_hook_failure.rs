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
