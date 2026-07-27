use ae_sdd_contracts::{
    LifecycleCommand, LifecycleDisposition, LifecycleInput, ProcessSnapshot, SchemaVersion,
    lifecycle::CompletionMilestoneInput,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, CompletionDigestSet, CompletionMilestone, DesignRoute,
    EvidenceDigest, EvidenceId, EvidenceRef, InputFingerprint, ProcessPhase, ProjectRelativePath,
    StateRevision, VerificationId, WorkItemId, WorkScale,
};
use ae_sdd_lifecycle::LifecycleEngine;
use ae_sdd_protocol::ConfirmationRef;

const NOW: u64 = 1_785_000_000_000;

fn digest(label: &[u8]) -> ArtifactDigest {
    ArtifactDigest::digest(label)
}

fn bound_v1() -> CompletionDigestSet {
    CompletionDigestSet::new(
        digest(b"code-v1"),
        digest(b"verification-v1"),
        digest(b"evidence-v1"),
        digest(b"review-input-v1"),
        digest(b"final-gates-v1"),
    )
}

fn completion(
    milestone: CompletionMilestone,
    observed: CompletionDigestSet,
) -> CompletionMilestoneInput {
    CompletionMilestoneInput::new(milestone, bound_v1(), observed)
}

fn completion_input(
    phase: ProcessPhase,
    confirmations: Vec<ConfirmationRef>,
    evidence_refs: Vec<EvidenceRef>,
    completion: Option<CompletionMilestoneInput>,
) -> LifecycleInput {
    completion_input_to(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::Completed,
        },
        phase,
        confirmations,
        evidence_refs,
        completion,
    )
}

fn completion_input_to(
    command: LifecycleCommand,
    phase: ProcessPhase,
    confirmations: Vec<ConfirmationRef>,
    evidence_refs: Vec<EvidenceRef>,
    completion: Option<CompletionMilestoneInput>,
) -> LifecycleInput {
    let base = LifecycleInput::new(
        SchemaVersion::V1,
        command,
        ProcessSnapshot::new(
            SchemaVersion::V1,
            WorkItemId::new("WORK-ITEM-001").expect("work item"),
            phase,
            None,
            StateRevision::new(7),
            ArtifactDigest::digest(b"authoritative state"),
        ),
        StateRevision::new(7),
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        confirmations,
        evidence_refs,
        Vec::new(),
        NOW,
        InputFingerprint::digest(b"stable lifecycle input"),
    )
    .expect("valid lifecycle input");
    match completion {
        Some(completion) => base.with_completion(completion),
        None => base,
    }
}

fn evidence(verification_id: &str) -> EvidenceRef {
    EvidenceRef::new(
        EvidenceId::new(format!("evidence-{verification_id}")).expect("evidence id"),
        VerificationId::new(verification_id).expect("verification id"),
        ProjectRelativePath::new(format!(".ae-sdd/evidence/{verification_id}.json"))
            .expect("project-relative path"),
        EvidenceDigest::digest(verification_id.as_bytes()),
        32,
    )
}

fn completion_gate_evidence() -> Vec<EvidenceRef> {
    vec![evidence("G-00"), evidence("G-12"), evidence("G-13")]
}

fn confirmation(binding: &str) -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: binding.to_owned(),
        approved_by: "user:owner".to_owned(),
        approved_at: "2026-07-24T00:00:00Z".to_owned(),
    }
}

fn remediation_codes(plan: &ae_sdd_contracts::LifecyclePlan) -> Vec<String> {
    let value = serde_json::to_value(plan).expect("plan wire");
    value["remediation"]
        .as_array()
        .expect("remediation array")
        .iter()
        .map(|item| item["code"].as_str().expect("remediation code").to_owned())
        .collect()
}

#[test]
fn completed_requires_a_completion_milestone_projection() {
    let input = completion_input(
        ProcessPhase::CodeReviewed,
        Vec::new(),
        completion_gate_evidence(),
        None,
    );

    let plan = LifecycleEngine::plan(&input).expect("missing milestone is a denied plan");

    assert_eq!(plan.disposition(), LifecycleDisposition::Denied);
    assert_eq!(
        remediation_codes(&plan),
        vec!["lifecycle.completion-milestone-required".to_owned()]
    );
}

#[test]
fn completed_is_denied_below_governance_closed() {
    for milestone in [
        CompletionMilestone::None,
        CompletionMilestone::ImplementationVerified,
        CompletionMilestone::ReviewReady,
    ] {
        let input = completion_input(
            ProcessPhase::CodeReviewed,
            Vec::new(),
            completion_gate_evidence(),
            Some(completion(milestone, bound_v1())),
        );

        let plan = LifecycleEngine::plan(&input).expect("milestone denial is a plan");

        assert_eq!(
            plan.disposition(),
            LifecycleDisposition::Denied,
            "milestone {milestone:?} must not plan Completed",
        );
        assert_eq!(
            remediation_codes(&plan),
            vec!["lifecycle.completion-milestone-open".to_owned()]
        );
    }
}

#[test]
fn governance_closed_with_fresh_digests_awaits_then_accepts_confirmation() {
    let pending = completion_input(
        ProcessPhase::CodeReviewed,
        Vec::new(),
        completion_gate_evidence(),
        Some(completion(
            CompletionMilestone::GovernanceClosed,
            bound_v1(),
        )),
    );
    let plan = LifecycleEngine::plan(&pending).expect("fresh governance close plans");

    assert_eq!(
        plan.disposition(),
        LifecycleDisposition::AwaitingConfirmation,
        "Completed stays a protected transition even at GovernanceClosed",
    );

    let value = serde_json::to_value(&plan).expect("plan wire");
    let binding = value["confirmationRequirement"]["bindingDigest"]
        .as_str()
        .expect("binding digest")
        .to_owned();
    let confirmed = completion_input(
        ProcessPhase::CodeReviewed,
        vec![confirmation(&binding)],
        completion_gate_evidence(),
        Some(completion(
            CompletionMilestone::GovernanceClosed,
            bound_v1(),
        )),
    );

    let permitted = LifecycleEngine::plan(&confirmed).expect("confirmed completion plans");

    assert_eq!(permitted.disposition(), LifecycleDisposition::Permitted);
    assert!(
        !permitted.intents().is_empty(),
        "a permitted completion emits mutation intents",
    );
}

#[test]
fn stale_input_digests_roll_back_and_deny_completed() {
    let bound = bound_v1();
    for (name, observed) in [
        ("code", bound.with_code_digest(digest(b"code-v2"))),
        (
            "verification",
            bound.with_verification_digest(digest(b"verification-v2")),
        ),
        (
            "evidence",
            bound.with_evidence_digest(digest(b"evidence-v2")),
        ),
        (
            "review input",
            bound.with_review_input_digest(digest(b"review-input-v2")),
        ),
        (
            "final gates",
            bound.with_gate_digest(digest(b"final-gates-v2")),
        ),
    ] {
        let input = completion_input(
            ProcessPhase::CodeReviewed,
            Vec::new(),
            completion_gate_evidence(),
            Some(completion(CompletionMilestone::GovernanceClosed, observed)),
        );

        let plan = LifecycleEngine::plan(&input).expect("stale digest denial is a plan");

        assert_eq!(
            plan.disposition(),
            LifecycleDisposition::Denied,
            "stale {name} digest must roll back GovernanceClosed",
        );
        assert_eq!(
            remediation_codes(&plan),
            vec!["lifecycle.completion-milestone-open".to_owned()],
            "stale {name} digest must surface the milestone remediation",
        );
    }
}

#[test]
fn non_completed_transitions_do_not_consume_the_milestone() {
    let input = completion_input_to(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::TestcaseGenerated,
        },
        ProcessPhase::StoryGenerated,
        Vec::new(),
        Vec::new(),
        None,
    );

    let plan = LifecycleEngine::plan(&input).expect("regular transition plans");

    assert_eq!(plan.disposition(), LifecycleDisposition::Permitted);
}

#[test]
fn completion_projection_wire_round_trip_and_legacy_default() {
    let input = completion_input(
        ProcessPhase::CodeReviewed,
        Vec::new(),
        completion_gate_evidence(),
        Some(completion(
            CompletionMilestone::GovernanceClosed,
            bound_v1(),
        )),
    );

    let wire = serde_json::to_value(&input).expect("input wire");
    assert_eq!(
        wire["completion"]["milestone"].as_str(),
        Some("governance_closed"),
        "the milestone wire value is an explicit stable string",
    );
    let decoded: LifecycleInput =
        serde_json::from_value(wire).expect("completion projection round-trips");
    assert_eq!(decoded, input);

    let legacy = completion_input(
        ProcessPhase::CodeReviewed,
        Vec::new(),
        completion_gate_evidence(),
        None,
    );
    let legacy_wire = serde_json::to_value(&legacy).expect("legacy input wire");
    assert!(
        legacy_wire.get("completion").is_none(),
        "a missing milestone projection stays off the wire",
    );
    let decoded_legacy: LifecycleInput =
        serde_json::from_value(legacy_wire).expect("legacy payload still decodes");
    assert_eq!(decoded_legacy, legacy);
}
