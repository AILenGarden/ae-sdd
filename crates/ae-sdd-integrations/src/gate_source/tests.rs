use std::{fs, path::Path, time::Duration};

use ae_sdd_domain::{FreshnessDimension, GateOutcome, GateResult};
use ae_sdd_protocol::WorkspaceMode;
use ae_sdd_runtime::BusinessWorkspace;
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

use super::{AuthoritativeGateRuntime, gate_result_json};

fn workspace(root: &Path) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: Uuid::from_u128(7).to_string(),
        canonical_root: root.to_string_lossy().into_owned(),
        project_key: "gate-test".to_owned(),
        mode: WorkspaceMode::Shadow,
        agent_role: None,
        inventory_generation: 1,
    }
}

fn write_state(root: &Path, revision: u64, extra: Value) {
    let directory = root.join(".auto-engineering/work-item");
    fs::create_dir_all(&directory).expect("state directory");
    let mut value = json!({
        "stateMachineName":"WI-001",
        "currentWorkItem":"WI-001",
        "activeStory":"STORY-001",
        "revision":revision,
        "lastFencingToken":3
    });
    value
        .as_object_mut()
        .expect("object")
        .extend(extra.as_object().expect("extra").clone());
    fs::write(
        directory.join("state.json"),
        serde_json::to_vec(&value).expect("json"),
    )
    .expect("state write");
}

fn runtime(temp: &TempDir) -> AuthoritativeGateRuntime {
    AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "WI-001",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
}

#[test]
fn bare_gate_results_pass_is_never_trusted() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({"gateResults":{"G-14":{"outcome":"PASS"}}}),
    );
    let result = runtime(&temp)
        .evaluate("G-14", Duration::from_secs(1))
        .expect("Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
    assert_eq!(gate_result_json(&result)["outcome"]["kind"], "FAIL");
}

#[test]
fn state_revision_change_invalidates_a_recorded_pass() {
    let temp = TempDir::new().expect("temp");
    write_state(temp.path(), 1, json!({}));
    let runtime = runtime(&temp);
    let snapshot = runtime.snapshot_key("G-14").expect("snapshot");
    write_state(temp.path(), 2, json!({}));
    let current = runtime.current_key("G-14").expect("current");
    let outcome = GateResult::new(snapshot, GateOutcome::Pass).outcome_against(&current);

    let GateOutcome::Stale(stale) = outcome else {
        panic!("revision drift must return STALE");
    };
    assert!(stale.changed().contains(&FreshnessDimension::StateRevision));
}
