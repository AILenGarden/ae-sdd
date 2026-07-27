#[path = "../src/lifecycle_authority.rs"]
mod lifecycle_authority;

use ae_sdd_domain::{AgentRole, StateRevision};
use ae_sdd_operations::{Confirmation, OperationName};
use ae_sdd_protocol::StableErrorCode;
use lifecycle_authority::{
    LifecycleAuthorityDisposition, PermittedLifecycleMutation, apply_exact_after_image,
    preflight_lifecycle_confirmation, prepare_lifecycle_mutation,
};
use serde_json::{Value, json};

const EVALUATION_UNIX_MS: u64 = 1_753_392_000_000;
const PRD_ID: &str = "PRD-C1-001";

#[test]
fn flat_complete_prd_normalizes_terminal_fields_and_preserves_children() {
    let mut state = flat_prd_state();
    state["completedSteps"] = json!(["coding", "coding", "code-reviewed"]);
    state["pendingOutputs"] = json!(["summary", "compact"]);
    state["codingRound"] = json!("r0");
    state["pausedFromPhase"] = json!("code-reviewed");
    state["pausedFrom"] = json!("code-reviewed");
    state["pauseReason"] = json!("legacy");
    state["unrelated"] = json!({"keep":true});
    let before = state.clone();

    let permitted = confirmed_prd_completion(&state);
    assert_eq!(
        permitted.intents()[0].event.kind.as_str(),
        "lifecycle.prd-completed"
    );
    assert_eq!(
        permitted.intents()[1].event.kind.as_str(),
        "lifecycle.prd-completed"
    );
    let after =
        apply_exact_after_image(&state, PRD_ID, &permitted).expect("flat CompletePrd after-image");

    assert_eq!(after["phase"], "completed");
    assert_eq!(after["currentPhase"], "completed");
    assert_eq!(after["currentStep"], "completed");
    assert_eq!(after["completedSteps"], json!(["coding", "code-reviewed"]));
    assert_eq!(after["pendingOutputs"], json!([]));
    assert_eq!(after["codingRound"], 1);
    assert_eq!(after["prdStatus"], "awaiting_compact");
    for removed in ["pausedFromPhase", "pausedFrom", "pauseReason"] {
        assert!(after.get(removed).is_none(), "field {removed}");
    }
    assert_eq!(after["storyStates"], before["storyStates"]);
    assert_eq!(after["unrelated"], before["unrelated"]);
    assert_eq!(state, before);
}

#[test]
fn nested_complete_prd_updates_only_prd_authority_and_required_root_mirrors() {
    let state = nested_prd_state();
    let before = state.clone();
    let permitted = confirmed_prd_completion(&state);
    let after = apply_exact_after_image(&state, PRD_ID, &permitted)
        .expect("nested CompletePrd after-image");

    assert_eq!(after["prdState"]["phase"], "completed");
    assert_eq!(after["prdState"]["currentPhase"], "completed");
    assert_eq!(after["prdState"]["currentStep"], "completed");
    assert_eq!(after["prdState"]["pendingOutputs"], json!({}));
    assert_eq!(after["prdState"]["codingRound"], "r2");
    assert_eq!(after["phase"], "completed");
    assert_eq!(after["currentPhase"], "completed");
    assert_eq!(after["currentStep"], "completed");
    assert_eq!(after["prdStatus"], "awaiting_compact");
    assert_eq!(after["storyStates"], before["storyStates"]);
    assert_eq!(after["drStates"], before["drStates"]);
    assert_eq!(after["unrelated"], before["unrelated"]);
    assert_eq!(state, before);
}

#[test]
fn prd_projection_rejects_conflicting_mirrors_and_duplicate_story_ownership() {
    let mut prd_conflict = nested_prd_state();
    prd_conflict["currentPhase"] = json!("coding");

    let mut story_conflict = nested_prd_state();
    story_conflict["drStates"]["DR-C1-001"]["storyStates"]["STORY-C1-001"]["codingRound"] =
        json!(3);

    let mut duplicate_owner = nested_prd_state();
    duplicate_owner["drStates"]["DR-C1-002"] = json!({
        "drId":"DR-C1-002",
        "phase":"completed",
        "storyStates":{
            "STORY-C1-001": completed_story()
        }
    });

    for state in [prd_conflict, story_conflict, duplicate_owner] {
        let before = state.clone();
        let error = preflight_lifecycle_confirmation(
            &state,
            PRD_ID,
            OperationName::WorkItemComplete,
            &json!({}),
            StateRevision::new(7),
            // No completion milestone is projected by this fixture.
            None,
            AgentRole::Root,
            None,
            EVALUATION_UNIX_MS,
        )
        .expect_err("invalid PRD projection must fail before planning");
        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
        assert_eq!(state, before);
    }
}

#[test]
fn completed_child_requires_explicit_terminal_mirrors_pending_and_round() {
    for missing in [
        "currentPhase",
        "currentStep",
        "pendingOutputs",
        "codingRound",
    ] {
        let mut state = flat_prd_state();
        state["storyStates"]["STORY-C1-001"]
            .as_object_mut()
            .expect("Story object")
            .remove(missing);
        let before = state.clone();
        let outcome = preflight_lifecycle_confirmation(
            &state,
            PRD_ID,
            OperationName::WorkItemComplete,
            &json!({}),
            StateRevision::new(7),
            // No completion milestone is projected by this fixture.
            None,
            AgentRole::Root,
            None,
            EVALUATION_UNIX_MS,
        )
        .expect("incomplete child is a semantic lifecycle outcome");
        assert_eq!(outcome.disposition(), LifecycleAuthorityDisposition::Denied);
        let error = outcome
            .into_permitted()
            .expect_err("incomplete child cannot complete its PRD");
        assert_eq!(
            error.code(),
            StableErrorCode::GateBlocked,
            "field {missing}"
        );
        assert_eq!(state, before);
    }
}

#[test]
fn complete_prd_status_matrix_allows_only_pre_compact_states() {
    for status in [None, Some("in_progress"), Some("prd_complete_pending_user")] {
        let mut state = flat_prd_state();
        if let Some(status) = status {
            state["prdStatus"] = json!(status);
        }
        let outcome = preflight_lifecycle_confirmation(
            &state,
            PRD_ID,
            OperationName::WorkItemComplete,
            &json!({}),
            StateRevision::new(7),
            // No completion milestone is projected by this fixture.
            None,
            AgentRole::Root,
            None,
            EVALUATION_UNIX_MS,
        )
        .expect("pre-compact status is accepted for confirmation");
        assert_eq!(
            outcome.disposition(),
            LifecycleAuthorityDisposition::AwaitingConfirmation
        );
    }

    for status in ["awaiting_compact", "compacted", "prd_aborted"] {
        let mut state = flat_prd_state();
        state["prdStatus"] = json!(status);
        let error = preflight_lifecycle_confirmation(
            &state,
            PRD_ID,
            OperationName::WorkItemComplete,
            &json!({}),
            StateRevision::new(7),
            // No completion milestone is projected by this fixture.
            None,
            AgentRole::Root,
            None,
            EVALUATION_UNIX_MS,
        )
        .expect_err("post-completion PRD status cannot be downgraded");
        assert_eq!(
            error.code(),
            StableErrorCode::GateBlocked,
            "status {status}"
        );
    }

    let mut invalid = flat_prd_state();
    invalid["prdStatus"] = json!("unknown");
    let error = preflight_lifecycle_confirmation(
        &invalid,
        PRD_ID,
        OperationName::WorkItemComplete,
        &json!({}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect_err("unknown PRD status is malformed");
    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
}

fn confirmed_prd_completion(state: &Value) -> PermittedLifecycleMutation {
    let preflight = preflight_lifecycle_confirmation(
        state,
        PRD_ID,
        OperationName::WorkItemComplete,
        &json!({}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("CompletePrd preflight");
    let confirmation = Confirmation::new(
        preflight
            .confirmation_binding()
            .expect("CompletePrd binding")
            .to_owned(),
        "user:owner".to_owned(),
        "2026-07-25T00:00:00Z".to_owned(),
    )
    .expect("confirmation");
    prepare_lifecycle_mutation(
        state,
        PRD_ID,
        OperationName::WorkItemComplete,
        &json!({}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        Some(&confirmation),
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("confirmed CompletePrd plans")
    .into_permitted()
    .expect("confirmed CompletePrd is permitted")
}

fn flat_prd_state() -> Value {
    json!({
        "stateMachineName":PRD_ID,
        "revision":7,
        "scale":"large",
        "selectedDesign":"Story",
        "phase":"code-reviewed",
        "currentPhase":"code-reviewed",
        "currentStep":"code-reviewed",
        "pendingOutputs":0,
        "codingRound":2,
        "prdCompletion":{
            "dependenciesSatisfied":true,
            "residualRisksCleared":true,
            "gatesPassed":true,
            "reviewPassed":true
        },
        "storyStates":{
            "STORY-C1-001":completed_story()
        },
        "evidenceRefs":[evidence()],
    })
}

fn nested_prd_state() -> Value {
    let story = completed_story();
    json!({
        "stateMachineName":PRD_ID,
        "revision":7,
        "scale":"large",
        "selectedDesign":"Story",
        "phase":"code-reviewed",
        "currentPhase":"code-reviewed",
        "currentStep":"code-reviewed",
        "prdState":{
            "prdId":PRD_ID,
            "phase":"code-reviewed",
            "currentPhase":"code-reviewed",
            "currentStep":"code-reviewed",
            "completedSteps":["coding"],
            "codingRound":"r2"
        },
        "prdCompletion":{
            "dependenciesSatisfied":true,
            "residualRisksCleared":true,
            "gatesPassed":true,
            "reviewPassed":true
        },
        "storyStates":{
            "STORY-C1-001":story.clone()
        },
        "drStates":{
            "DR-C1-001":{
                "drId":"DR-C1-001",
                "phase":"completed",
                "storyStates":{
                    "STORY-C1-001":story
                }
            }
        },
        "evidenceRefs":[evidence()],
        "unrelated":{"keep":true}
    })
}

fn completed_story() -> Value {
    json!({
        "phase":"completed",
        "currentPhase":"completed",
        "currentStep":"completed",
        "completedSteps":["code-reviewed"],
        "pendingOutputs":{},
        "codingRound":"r2",
        "unrelated":{"keep":true}
    })
}

fn evidence() -> Value {
    json!({
        "evidenceId":"evidence-prd",
        "verificationId":"V-010",
        "path":".ae-sdd/evidence/prd.json",
        "digest":"1".repeat(64),
        "byteLength":1
    })
}
