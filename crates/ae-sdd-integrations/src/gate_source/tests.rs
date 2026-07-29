use std::{fs, path::Path, time::Duration};

use ae_sdd_domain::{FreshnessDimension, GateOutcome, GateResult};
use ae_sdd_protocol::WorkspaceMode;
use ae_sdd_runtime::BusinessWorkspace;
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

use super::{
    AuthoritativeGateRuntime, contracts::plan_contract_complete, gate_result_json,
    predicate::ac_ids,
};

fn workspace(root: &Path) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: Uuid::from_u128(7).to_string(),
        canonical_root: root.to_string_lossy().into_owned(),
        project_key: "gate-test".to_owned(),
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

fn verification_row(index: u32) -> Value {
    json!({
        "id":format!("V-{index:03}"),
        "acId":format!("AC-{index}"),
        "boundary":"unit",
        "command":"cargo test",
        "expected":"pass"
    })
}

fn approved_plan(verification: Vec<Value>) -> Value {
    json!({
        "goal":"implement the story",
        "changedPaths":["src/lib.rs"],
        "verification":verification,
        "risks":["fixture risk"],
        "approved":true,
        "sourceReads":["src/lib.rs"]
    })
}

fn complete_plan() -> Value {
    approved_plan((1..=9).map(verification_row).collect())
}

#[test]
fn plan_contract_complete_accepts_nine_complete_verification_rows() {
    // Regression: G-08 is a plan-completeness check, not a row count; a Story
    // with fewer than fourteen ACs must still pass.
    assert!(plan_contract_complete(&complete_plan()));
}

#[test]
fn plan_contract_complete_rejects_an_empty_verification_matrix() {
    let plan = approved_plan(Vec::new());
    assert!(!plan_contract_complete(&plan));
}

#[test]
fn plan_contract_complete_rejects_a_row_with_an_empty_field() {
    let mut verification: Vec<Value> = (1..=9).map(verification_row).collect();
    verification[3]["expected"] = json!("");
    assert!(!plan_contract_complete(&approved_plan(verification)));
}

#[test]
fn plan_contract_complete_rejects_an_unapproved_plan() {
    let mut plan = complete_plan();
    plan["approved"] = json!(false);
    assert!(!plan_contract_complete(&plan));
}

#[test]
fn plan_contract_complete_rejects_missing_or_blank_source_reads() {
    let mut missing = complete_plan();
    missing.as_object_mut().expect("plan").remove("sourceReads");
    assert!(!plan_contract_complete(&missing));

    let mut empty = complete_plan();
    empty["sourceReads"] = json!([]);
    assert!(!plan_contract_complete(&empty));

    let mut blank = complete_plan();
    blank["sourceReads"] = json!(["  "]);
    assert!(!plan_contract_complete(&blank));
}

/// Installs a Story document declaring `acs` and points the state at it.
fn install_story(root: &Path, acs: &str) {
    let path = root.join("ae-sdd-doc/Story/STORY-001.md");
    fs::create_dir_all(path.parent().expect("story parent")).expect("story directory");
    fs::write(path, format!("# Story\n\n{acs}\n")).expect("story document");
}

fn story_state(plan: Value) -> Value {
    json!({
        "executionPlan":plan,
        "storyStates":{"STORY-001":{"docPath":"ae-sdd-doc/Story/STORY-001.md"}}
    })
}

#[test]
fn g08_passes_with_nine_rows_covering_every_story_ac() {
    let temp = TempDir::new().expect("temp");
    install_story(temp.path(), "AC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7 AC-8 AC-9");
    write_state(temp.path(), 1, story_state(complete_plan()));

    let result = runtime(&temp)
        .evaluate("G-08", Duration::from_secs(1))
        .expect("Gate evaluation");
    assert!(matches!(result.outcome(), GateOutcome::Pass));
}

#[test]
fn g08_fails_when_the_plan_misses_a_story_ac() {
    let temp = TempDir::new().expect("temp");
    install_story(
        temp.path(),
        "AC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7 AC-8 AC-9 AC-10",
    );
    write_state(temp.path(), 1, story_state(complete_plan()));

    let result = runtime(&temp)
        .evaluate("G-08", Duration::from_secs(1))
        .expect("Gate evaluation");
    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn g08_skips_story_coverage_without_an_active_story() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({"activeStory":null, "executionPlan":complete_plan()}),
    );

    let result = runtime(&temp)
        .evaluate("G-08", Duration::from_secs(1))
        .expect("Gate evaluation");
    assert!(matches!(result.outcome(), GateOutcome::Pass));
}

#[test]
fn ac_ids_accepts_descriptive_and_numeric_suffixes() {
    let ids = ac_ids("AC-1 AC-001 AC-NAME-01 AC-DC AC-");

    assert!(ids.contains("AC-1"));
    assert!(ids.contains("AC-001"));
    assert!(ids.contains("AC-NAME-01"));
    assert!(!ids.contains("AC-DC"));
    assert_eq!(ids.len(), 3);
}
