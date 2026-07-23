use ae_sdd_domain::{FreshnessDimension, GateOutcome, GateResult, StoryId};
use ae_sdd_gates::canonical_gate_key_digest;
use serde_json::{Value, json};

/// Projects a typed Gate result without collapsing FAIL/ERROR/TIMEOUT/CANCELLED/STALE.
#[must_use]
pub fn gate_result_json(result: &GateResult) -> Value {
    let key = result.key();
    json!({
        "gateId": key.gate_id().as_str(),
        "key": {
            "digest": canonical_gate_key_digest(key).to_string(),
            "gateImplementationDigest": key.gate_implementation().to_string(),
            "policyDigest": key.policy().to_string(),
            "workspaceId": key.workspace_id().to_string(),
            "workItemId": key.work_item_id().as_str(),
            "storyId": key.story_id().map(StoryId::as_str),
            "stateRevision": key.state_revision().get(),
            "fencingToken": key.fencing_token().get(),
            "inventoryGeneration": key.inventory_generation().get(),
            "toolchainDigest": key.toolchain().to_string(),
            "configDigest": key.configuration().to_string(),
            "inputFingerprint": key.input().to_string(),
        },
        "outcome": outcome_json(result.outcome()),
    })
}

fn outcome_json(outcome: &GateOutcome) -> Value {
    match outcome {
        GateOutcome::Pass => json!({"kind":"PASS"}),
        GateOutcome::Fail(failure) => json!({
            "kind":"FAIL",
            "findings": failure.findings().iter().map(|finding| json!({
                "code":finding.code().as_str(),
                "evidence":finding.evidence().iter().map(|item| json!({
                    "evidenceId":item.evidence_id().as_str(),
                    "verificationId":item.verification_id().as_str(),
                    "path":item.path().as_str(),
                    "digest":item.digest().to_string(),
                    "byteLength":item.byte_length()
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        }),
        GateOutcome::Error(error) => {
            json!({"kind":"ERROR","code":error.code().as_str(),"retryable":error.retryable()})
        }
        GateOutcome::Timeout(timeout) => {
            json!({"kind":"TIMEOUT","deadlineMs":timeout.deadline_ms()})
        }
        GateOutcome::Cancelled(cancelled) => {
            json!({"kind":"CANCELLED","reason":cancelled.reason().as_str()})
        }
        GateOutcome::Stale(stale) => json!({
            "kind":"STALE",
            "changed":stale.changed().iter().map(freshness_name).collect::<Vec<_>>()
        }),
    }
}

fn freshness_name(value: &FreshnessDimension) -> &'static str {
    match value {
        FreshnessDimension::GateId => "gateId",
        FreshnessDimension::GateImplementation => "gateImplementation",
        FreshnessDimension::Policy => "policy",
        FreshnessDimension::Workspace => "workspace",
        FreshnessDimension::WorkItem => "workItem",
        FreshnessDimension::Story => "story",
        FreshnessDimension::StateRevision => "stateRevision",
        FreshnessDimension::FencingToken => "fencingToken",
        FreshnessDimension::InventoryGeneration => "inventoryGeneration",
        FreshnessDimension::Toolchain => "toolchain",
        FreshnessDimension::Configuration => "configuration",
        FreshnessDimension::Input => "input",
    }
}
