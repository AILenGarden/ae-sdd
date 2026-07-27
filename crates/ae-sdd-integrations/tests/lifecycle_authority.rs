#[path = "../src/lifecycle_authority.rs"]
mod lifecycle_authority;

use ae_sdd_domain::{AgentRole, ProcessPhase, StateRevision};
use ae_sdd_operations::{Confirmation, OperationName};
use ae_sdd_protocol::StableErrorCode;
use lifecycle_authority::{
    LifecycleAuthorityDisposition, apply_exact_after_image, preflight_lifecycle_confirmation,
    prepare_lifecycle_mutation, validate_exact_intents,
};
use serde_json::{Value, json};

const EVALUATION_UNIX_MS: u64 = 1_753_392_000_000;

#[test]
fn illegal_phase_transition_is_gate_blocked_with_remediation_and_no_intents() {
    let state = story_state("initialized", Vec::new());
    let before = state.clone();

    let outcome = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"completed"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("semantic denial is a lifecycle outcome");

    assert_eq!(outcome.disposition(), LifecycleAuthorityDisposition::Denied);
    assert!(outcome.intents().is_empty());
    assert!(!outcome.remediation().is_empty());
    let error = outcome
        .into_permitted()
        .expect_err("denied lifecycle plan cannot mutate");
    assert_eq!(error.code(), StableErrorCode::GateBlocked);
    assert!(error.remediation().is_some());
    assert_eq!(state, before);
}

#[test]
fn stale_revision_is_reported_as_revision_conflict_during_preflight() {
    let state = story_state("coding-process", coding_gate_evidence());
    let before = state.clone();

    let error = preflight_lifecycle_confirmation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"coding"}),
        StateRevision::new(6),
        // No completion milestone is projected by this fixture.
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect_err("stale lifecycle preflight must fail before confirmation planning");

    assert_eq!(error.code(), StableErrorCode::RevisionConflict);
    assert_eq!(state, before);
}

#[test]
fn missing_nested_target_does_not_fall_back_to_the_root_state() {
    let state = json!({
        "stateMachineName":"PRD-C1-ROOT",
        "revision":7,
        "scale":"large",
        "selectedDesign":"Story",
        "phase":"initialized",
        "currentPhase":"initialized",
        "storyStates":{
            "STORY-C1-OTHER":{
                "phase":"initialized",
                "currentPhase":"initialized",
                "currentStep":"initialized",
                "pendingOutputs":0,
                "codingRound":1
            }
        }
    });

    let error = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-MISSING",
        OperationName::StateTransition,
        &json!({"targetPhase":"route-selected"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect_err("a missing nested target must not inherit the root phase");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
}

#[test]
fn phase_is_authoritative_and_a_conflicting_current_phase_is_rejected() {
    let mut state = story_state("initialized", coding_gate_evidence());
    state["storyStates"]["STORY-C1-001"]["currentPhase"] = json!("route-selected");
    let before = state.clone();

    let error = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"route-selected"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect_err("a conflicting phase mirror must fail closed");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    assert_eq!(state, before);
}

#[test]
fn transition_to_paused_dispatches_the_pause_command() {
    let state = story_state("initialized", Vec::new());

    let permitted = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"paused"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("pause planning succeeds")
    .into_permitted()
    .expect("pause is permitted");

    assert_eq!(permitted.target_phase(), Some(ProcessPhase::Paused));
    assert_eq!(permitted.intents().len(), 2);
    assert_eq!(
        permitted.intents()[0].event.kind.as_str(),
        "lifecycle.paused"
    );
    assert_eq!(
        permitted.intents()[1].event.kind.as_str(),
        "lifecycle.paused"
    );
}

#[test]
fn pause_after_image_preserves_every_unrelated_field_and_exact_source() {
    let mut state = story_state("coding-process", Vec::new());
    state["storyStates"]["STORY-C1-001"]["completedSteps"] =
        json!(["initialized", "route-selected"]);
    state["storyStates"]["STORY-C1-001"]["pendingOutputs"] = json!({"review":"open"});
    state["storyStates"]["STORY-C1-001"]["codingRound"] = json!(4);
    state["storyStates"]["STORY-C1-001"]["unrelated"] = json!({"keep":true});
    let before = state.clone();

    let permitted = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"paused"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("pause planning succeeds")
    .into_permitted()
    .expect("pause is permitted");
    let after = apply_exact_after_image(&state, "STORY-C1-001", &permitted)
        .expect("exact pause after-image is valid");

    let child_before = &before["storyStates"]["STORY-C1-001"];
    let child_after = &after["storyStates"]["STORY-C1-001"];
    assert_eq!(child_after["phase"], "paused");
    assert_eq!(child_after["currentPhase"], "paused");
    assert_eq!(child_after["pausedFromPhase"], "coding-process");
    assert_eq!(child_after["pauseReason"], "user-manual");
    for field in [
        "currentStep",
        "completedSteps",
        "pendingOutputs",
        "codingRound",
        "unrelated",
    ] {
        assert_eq!(child_after[field], child_before[field], "field {field}");
    }
    assert_eq!(state, before, "the reducer must not mutate its input");
}

#[test]
fn exact_intent_validator_rejects_delete_insert_swap_and_binding_tamper() {
    let state = story_state("initialized", Vec::new());
    let permitted = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"paused"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("pause planning succeeds")
    .into_permitted()
    .expect("pause is permitted");
    let digest = permitted.plan_digest().to_string();
    let exact = permitted.intents().to_vec();

    validate_exact_intents(&permitted, &digest, &exact).expect("exact plan is accepted");

    let mut variants = Vec::new();
    variants.push(exact[..1].to_vec());
    let mut inserted = exact.clone();
    inserted.push(exact[1].clone());
    variants.push(inserted);
    let mut swapped = exact.clone();
    swapped.swap(0, 1);
    variants.push(swapped);
    let mut revision_tampered = exact.clone();
    revision_tampered[0].expected_revision = StateRevision::new(8);
    variants.push(revision_tampered);

    for candidate in variants {
        let error = validate_exact_intents(&permitted, &digest, &candidate)
            .expect_err("any intent sequence change must fail closed");
        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    }
    let error = validate_exact_intents(&permitted, &"0".repeat(64), &exact)
        .expect_err("plan digest tamper must fail closed");
    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
}

#[test]
fn resume_uses_only_paused_from_phase_and_restores_the_exact_after_image() {
    let mut state = story_state("paused", Vec::new());
    let target = &mut state["storyStates"]["STORY-C1-001"];
    target["pausedFromPhase"] = json!("coding-process");
    target["pausedFrom"] = json!("coding-process");
    target["pauseReason"] = json!("user-manual");
    target["currentStep"] = json!("coding-process");
    target["completedSteps"] = json!(["initialized", "route-selected"]);
    target["pendingOutputs"] = json!({"review":"open"});
    target["codingRound"] = json!(4);
    target["unrelated"] = json!({"keep":true});
    let before = state.clone();

    let permitted = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"coding-process"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("resume planning succeeds")
    .into_permitted()
    .expect("resume is permitted");

    assert_eq!(permitted.target_phase(), Some(ProcessPhase::CodingProcess));
    assert_eq!(permitted.intents().len(), 2);
    assert_eq!(
        permitted.intents()[0].event.kind.as_str(),
        "lifecycle.resumed"
    );
    assert_eq!(
        permitted.intents()[1].event.kind.as_str(),
        "lifecycle.resumed"
    );

    let after = apply_exact_after_image(&state, "STORY-C1-001", &permitted)
        .expect("exact resume after-image is valid");
    let child_before = &before["storyStates"]["STORY-C1-001"];
    let child_after = &after["storyStates"]["STORY-C1-001"];
    assert_eq!(child_after["phase"], "coding-process");
    assert_eq!(child_after["currentPhase"], "coding-process");
    for removed in ["pausedFromPhase", "pausedFrom", "pauseReason"] {
        assert!(child_after.get(removed).is_none(), "field {removed}");
    }
    for preserved in [
        "currentStep",
        "completedSteps",
        "pendingOutputs",
        "codingRound",
        "unrelated",
    ] {
        assert_eq!(
            child_after[preserved], child_before[preserved],
            "field {preserved}"
        );
    }
    assert_eq!(state, before, "the reducer must not mutate its input");
}

#[test]
fn resume_rejects_legacy_only_conflicting_and_malformed_sources() {
    let mut legacy_only = story_state("paused", Vec::new());
    legacy_only["storyStates"]["STORY-C1-001"]["pausedFrom"] = json!("coding-process");

    let mut conflicting = legacy_only.clone();
    conflicting["storyStates"]["STORY-C1-001"]["pausedFromPhase"] = json!("coding");

    let mut malformed = story_state("paused", Vec::new());
    malformed["storyStates"]["STORY-C1-001"]["pausedFromPhase"] = json!({"phase":"coding"});

    for state in [legacy_only, conflicting, malformed] {
        let before = state.clone();
        let error = prepare_lifecycle_mutation(
            &state,
            "STORY-C1-001",
            OperationName::StateTransition,
            &json!({"targetPhase":"coding-process"}),
            StateRevision::new(7),
            // No completion milestone is projected by this fixture.
            None,
            None,
            AgentRole::Root,
            None,
            EVALUATION_UNIX_MS,
        )
        .expect_err("invalid resume source must fail before planning");

        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
        assert_eq!(state, before);
    }
}

#[test]
fn transition_after_image_updates_steps_without_duplicates_and_normalizes_round() {
    let mut state = story_state("coding-process", coding_gate_evidence());
    let target = &mut state["storyStates"]["STORY-C1-001"];
    target["completedSteps"] = json!(["initialized", "initialized"]);
    target["pendingOutputs"] = json!(["test", "review"]);
    target["codingRound"] = json!(0);
    target["unrelated"] = json!({"keep":true});
    let before = state.clone();

    let preflight = preflight_lifecycle_confirmation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"coding"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("protected transition preflight");
    let confirmation = Confirmation::new(
        preflight
            .confirmation_binding()
            .expect("transition binding")
            .to_owned(),
        "user:owner".to_owned(),
        "2026-07-25T00:00:00Z".to_owned(),
    )
    .expect("confirmation");
    let permitted = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"coding"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        Some(&confirmation),
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("confirmed transition plans")
    .into_permitted()
    .expect("confirmed transition is permitted");

    let after = apply_exact_after_image(&state, "STORY-C1-001", &permitted)
        .expect("exact transition after-image is valid");
    let child_after = &after["storyStates"]["STORY-C1-001"];
    assert_eq!(child_after["phase"], "coding");
    assert_eq!(child_after["currentPhase"], "coding");
    assert_eq!(child_after["currentStep"], "coding");
    assert_eq!(
        child_after["completedSteps"],
        json!(["initialized", "coding-process"])
    );
    assert_eq!(child_after["codingRound"], 1);
    assert_eq!(
        child_after["pendingOutputs"],
        before["storyStates"]["STORY-C1-001"]["pendingOutputs"]
    );
    assert_eq!(
        child_after["unrelated"],
        before["storyStates"]["STORY-C1-001"]["unrelated"]
    );
    assert_eq!(state, before, "the reducer must not mutate its input");
}

#[test]
fn unprotected_transition_exposes_binding_and_validates_every_supplied_confirmation() {
    let state = story_state("initialized", coding_gate_evidence());
    let before = state.clone();
    let preflight = preflight_lifecycle_confirmation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"route-selected"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("unprotected transition preflight");
    assert_eq!(
        preflight.disposition(),
        LifecycleAuthorityDisposition::Permitted
    );
    let binding = preflight
        .confirmation_binding()
        .expect("permitted preflight exposes the engine action binding")
        .to_owned();

    let bound_confirmation = Confirmation::new(
        binding,
        "user:owner".to_owned(),
        "2026-07-25T00:00:00Z".to_owned(),
    )
    .expect("bound confirmation shape");
    let permitted = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"route-selected"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        Some(&bound_confirmation),
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("digest-bound confirmation is accepted")
    .into_permitted()
    .expect("confirmed unprotected transition remains permitted");
    assert_eq!(permitted.target_phase(), Some(ProcessPhase::RouteSelected));

    let arbitrary_confirmation = Confirmation::new(
        "not-the-engine-binding".to_owned(),
        "user:owner".to_owned(),
        "2026-07-25T00:00:00Z".to_owned(),
    )
    .expect("confirmation shape");

    let error = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"route-selected"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        Some(&arbitrary_confirmation),
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect_err("every supplied confirmation must be bound to the engine decision");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    assert_eq!(state, before);
}

#[test]
fn protected_transition_requires_digest_bound_confirmation_before_it_is_permitted() {
    let state = story_state("coding-process", coding_gate_evidence());
    let before = state.clone();

    let pending = preflight_lifecycle_confirmation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"coding"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("missing confirmation is a lifecycle outcome");

    assert_eq!(
        pending.disposition(),
        LifecycleAuthorityDisposition::AwaitingConfirmation
    );
    assert!(pending.intents().is_empty());
    let binding = pending
        .confirmation_binding()
        .expect("engine supplies a confirmation binding")
        .to_owned();
    let error = pending
        .into_permitted()
        .expect_err("missing confirmation cannot mutate");
    assert_eq!(error.code(), StableErrorCode::ConfirmationRequired);

    let confirmation = Confirmation::new(
        binding,
        "user:owner".to_owned(),
        "2026-07-25T00:00:00Z".to_owned(),
    )
    .expect("confirmation");
    let permitted = prepare_lifecycle_mutation(
        &state,
        "STORY-C1-001",
        OperationName::StateTransition,
        &json!({"targetPhase":"coding"}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        Some(&confirmation),
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("confirmed transition plans")
    .into_permitted()
    .expect("confirmed transition is permitted");

    assert!(!permitted.intents().is_empty());
    assert_eq!(permitted.target_phase(), Some(ProcessPhase::Coding));
    assert_eq!(permitted.data()["phase"], "coding");
    assert!(permitted.data()["planDigest"].is_string());
    assert_eq!(state, before);
}

#[test]
fn prd_completion_is_denied_while_a_registered_child_is_incomplete() {
    let state = json!({
        "stateMachineName":"PRD-C1-001",
        "revision":7,
        "scale":"large",
        "selectedDesign":"Story",
        "phase":"completed",
        "currentPhase":"completed",
        "prdState":{"prdId":"PRD-C1-001","phase":"completed"},
        "prdCompletion":{
            "dependenciesSatisfied":true,
            "residualRisksCleared":true,
            "gatesPassed":true,
            "reviewPassed":true
        },
        "storyStates":{
            "STORY-C1-CHILD-001":{
                "phase":"coding",
                "currentPhase":"coding",
                "currentStep":"coding",
                "pendingOutputs":0,
                "codingRound":1
            }
        }
    });
    let before = state.clone();

    let outcome = prepare_lifecycle_mutation(
        &state,
        "PRD-C1-001",
        OperationName::WorkItemComplete,
        &json!({}),
        StateRevision::new(7),
        // No completion milestone is projected by this fixture.
        None,
        None,
        AgentRole::Root,
        None,
        EVALUATION_UNIX_MS,
    )
    .expect("incomplete PRD is a semantic denial");

    assert_eq!(outcome.disposition(), LifecycleAuthorityDisposition::Denied);
    assert!(outcome.intents().is_empty());
    let error = outcome
        .into_permitted()
        .expect_err("incomplete PRD cannot mutate");
    assert_eq!(error.code(), StableErrorCode::GateBlocked);
    assert_eq!(state, before);
    assert_eq!(state["phase"], "completed");
}

fn story_state(phase: &str, evidence_refs: Vec<Value>) -> Value {
    json!({
        "stateMachineName":"PRD-C1-ROOT",
        "activeStory":"STORY-C1-001",
        "revision":7,
        "scale":"large",
        "selectedDesign":"Story",
        "phase":"requirement-analyzed",
        "currentPhase":"requirement-analyzed",
        "storyStates":{
            "STORY-C1-001":{
                "phase":phase,
                "currentPhase":phase,
                "currentStep":phase,
                "pendingOutputs":0,
                "codingRound":1
            }
        },
        "evidenceRefs":evidence_refs
    })
}

fn coding_gate_evidence() -> Vec<Value> {
    ["G-00", "G-07", "G-CODEPLAN-SRC", "G-14", "G-08", "G-HTTP-1"]
        .into_iter()
        .enumerate()
        .map(|(index, gate)| {
            json!({
                "evidenceId":format!("evidence-{index}"),
                "verificationId":gate,
                "path":format!(".ae-sdd/evidence/{index}.json"),
                "digest":format!("{index:064x}"),
                "byteLength":1
            })
        })
        .collect()
}
