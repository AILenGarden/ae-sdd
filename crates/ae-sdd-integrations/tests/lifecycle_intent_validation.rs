#[allow(dead_code)]
#[path = "../src/lifecycle_authority.rs"]
mod lifecycle_authority;

use ae_sdd_domain::{AgentRole, StateRevision};
use ae_sdd_operations::OperationName;
use ae_sdd_protocol::StableErrorCode;
use lifecycle_authority::{prepare_lifecycle_mutation, validate_exact_intents};
use serde_json::json;

const EVALUATION_UNIX_MS: u64 = 1_753_392_000_000;

#[test]
fn exact_lifecycle_intents_reject_sequence_and_binding_tampering_without_mutation() {
    let state = json!({
        "stateMachineName":"PRD-C1-ROOT",
        "activeStory":"STORY-C1-001",
        "revision":7,
        "scale":"large",
        "selectedDesign":"Story",
        "phase":"requirement-analyzed",
        "currentPhase":"requirement-analyzed",
        "storyStates":{
            "STORY-C1-001":{
                "phase":"initialized",
                "currentPhase":"initialized",
                "currentStep":"initialized",
                "pendingOutputs":0,
                "codingRound":1
            }
        }
    });
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
    .expect("pause planning")
    .into_permitted()
    .expect("pause permitted");
    let digest = permitted.plan_digest().to_string();
    let exact = permitted.intents().to_vec();
    validate_exact_intents(&permitted, &digest, &exact).expect("exact sequence");

    let mut candidates = vec![exact[..1].to_vec()];
    let mut inserted = exact.clone();
    inserted.push(exact[1].clone());
    candidates.push(inserted);
    let mut swapped = exact.clone();
    swapped.swap(0, 1);
    candidates.push(swapped);
    let mut revision_tampered = exact.clone();
    revision_tampered[0].expected_revision = StateRevision::new(8);
    candidates.push(revision_tampered);

    for candidate in candidates {
        let error = validate_exact_intents(&permitted, &digest, &candidate)
            .expect_err("tampered intent sequence must fail closed");
        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    }
    let error = validate_exact_intents(&permitted, &"0".repeat(64), &exact)
        .expect_err("tampered plan digest must fail closed");
    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    assert_eq!(state, before);
}
