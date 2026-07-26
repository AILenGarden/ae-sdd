//! V-EFF-008a: one `AuthoritativeGateRuntime` must keep a long-lived
//! scheduler so repeated evaluation of an unchanged Gate key reuses the fresh
//! cached outcome instead of re-running the executor.

use std::{fs, path::Path, time::Duration};

use ae_sdd_domain::GateOutcome;
use ae_sdd_gates::GateInputSelector;
use ae_sdd_integrations::AuthoritativeGateRuntime;
use ae_sdd_protocol::WorkspaceMode;
use ae_sdd_runtime::BusinessWorkspace;
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

const DEADLINE: Duration = Duration::from_secs(5);

fn workspace(root: &Path) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: Uuid::from_u128(7).to_string(),
        canonical_root: root.to_string_lossy().into_owned(),
        project_key: "gate-cache-lifecycle".to_owned(),
        mode: WorkspaceMode::Shadow,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
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
fn repeated_evaluate_of_same_key_keeps_executor_count_at_one() {
    let temp = TempDir::new().expect("temp");
    write_state(temp.path(), 1, json!({}));
    let runtime = runtime(&temp);

    let first = runtime
        .evaluate("G-14", DEADLINE)
        .expect("first evaluation");
    let second = runtime
        .evaluate("G-14", DEADLINE)
        .expect("second evaluation");

    assert!(matches!(first.outcome(), GateOutcome::Fail(_)));
    assert!(matches!(second.outcome(), GateOutcome::Fail(_)));
    let stats = runtime.stats();
    assert_eq!(
        stats.gates_evaluated, 1,
        "a second evaluate of the same key must reuse the cached outcome"
    );
    assert_eq!(stats.cache_hits, 1);
    assert_eq!(stats.cache_misses, 1);
}

#[test]
fn unchanged_gate_key_reuses_fresh_pass() {
    let temp = TempDir::new().expect("temp");
    write_state(temp.path(), 1, json!({}));
    let runtime = runtime(&temp);

    // G-AUTO-CONSENSUS passes deterministically when automation mode is off.
    let first = runtime
        .evaluate("G-AUTO-CONSENSUS", DEADLINE)
        .expect("first evaluation");
    let second = runtime
        .evaluate("G-AUTO-CONSENSUS", DEADLINE)
        .expect("second evaluation");

    assert!(matches!(first.outcome(), GateOutcome::Pass));
    assert!(matches!(second.outcome(), GateOutcome::Pass));
    assert_eq!(runtime.stats().gates_evaluated, 1);
}

#[test]
fn review_selector_invalidation_reruns_only_review_path_gates() {
    let temp = TempDir::new().expect("temp");
    write_state(temp.path(), 1, json!({}));
    let runtime = runtime(&temp);

    // G-14 is a CodingPlan Gate (Story/ExecutionPlan selectors); G-12 is a
    // Review Gate (ReviewBatch selector). Both fail closed in this lightweight
    // fixture and their outcomes are cached.
    let plan_gate = runtime.evaluate("G-14", DEADLINE).expect("G-14 evaluation");
    let review_gate = runtime.evaluate("G-12", DEADLINE).expect("G-12 evaluation");
    assert!(matches!(plan_gate.outcome(), GateOutcome::Fail(_)));
    assert!(matches!(review_gate.outcome(), GateOutcome::Fail(_)));
    assert_eq!(runtime.stats().gates_evaluated, 2);

    let affected = runtime.invalidate_selectors(&[GateInputSelector::ReviewBatch]);
    assert!(affected.contains(&"G-12"));
    assert!(!affected.contains(&"G-14"));

    let unaffected = runtime
        .evaluate("G-14", DEADLINE)
        .expect("G-14 re-evaluation");
    assert!(
        matches!(unaffected.outcome(), GateOutcome::Fail(_)),
        "the cached fresh outcome is reused"
    );
    assert_eq!(
        runtime.stats().gates_evaluated,
        2,
        "G-14 does not depend on the review batch and must stay cached"
    );

    runtime
        .evaluate("G-12", DEADLINE)
        .expect("G-12 re-evaluation");
    assert_eq!(
        runtime.stats().gates_evaluated,
        3,
        "G-12 depends on the review batch and must re-evaluate"
    );
}
