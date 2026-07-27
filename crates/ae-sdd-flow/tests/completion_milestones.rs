mod support;

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, CompletionDigestSet, CompletionMilestone, ProcessPhase,
    StateRevision,
};
use ae_sdd_flow::{
    FlowDecision, FlowEvent, FlowEventKind, FlowInput, FlowRuntime, FlowSnapshot, NextAction,
};
use ae_sdd_policy::{RequiredGate, TransitionPolicyError};
use support::{event, gate_for, input, transition_request_to};

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

fn input_with(
    phase: ProcessPhase,
    milestone: CompletionMilestone,
    bound: CompletionDigestSet,
) -> FlowInput {
    let snapshot = FlowSnapshot::new(phase, StateRevision::new(7), 0)
        .with_completion_milestone(milestone, bound);
    FlowInput::new(snapshot, input().environment())
}

fn verification(sequence: u64, code: ArtifactDigest, focused: ArtifactDigest) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::VerificationFreshnessObserved {
            code_digest: code,
            verification_digest: focused,
        },
    )
}

fn evidence_finalized(sequence: u64, evidence: ArtifactDigest) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::ExecutionEvidenceFinalized {
            evidence_digest: evidence,
        },
    )
}

fn contributions(sequence: u64) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::ReviewContributionsCollected,
    )
}

fn governance(sequence: u64, review: ArtifactDigest, gates: ArtifactDigest) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::GovernanceFinalized {
            review_input_digest: review,
            gate_digest: gates,
        },
    )
}

fn inputs_changed(sequence: u64, observed: CompletionDigestSet) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::CompletionInputsChanged { observed },
    )
}

fn apply(checkpoint: &FlowDecision, event: &FlowEvent) -> FlowDecision {
    FlowRuntime::apply(checkpoint, event).expect("milestone event reduces deterministically")
}

fn governance_closed() -> FlowDecision {
    let start = FlowRuntime::start(input_with(
        ProcessPhase::TestRunning,
        CompletionMilestone::None,
        CompletionDigestSet::ZERO,
    ));
    let verified = apply(
        &start,
        &verification(3, digest(b"code-v1"), digest(b"verification-v1")),
    );
    let ready = apply(&verified, &evidence_finalized(5, digest(b"evidence-v1")));
    let collected = apply(&ready, &contributions(8));
    apply(
        &collected,
        &governance(13, digest(b"review-input-v1"), digest(b"final-gates-v1")),
    )
}

#[test]
fn fresh_focused_and_workspace_verification_advances_to_implementation_verified() {
    let start = FlowRuntime::start(input_with(
        ProcessPhase::TestRunning,
        CompletionMilestone::None,
        CompletionDigestSet::ZERO,
    ));

    let decision = apply(
        &start,
        &verification(3, digest(b"code-v1"), digest(b"verification-v1")),
    );

    assert_eq!(
        decision.snapshot().completion_milestone(),
        CompletionMilestone::ImplementationVerified
    );
    assert_eq!(
        decision.snapshot().completion_bound(),
        CompletionDigestSet::ZERO
            .with_code_digest(digest(b"code-v1"))
            .with_verification_digest(digest(b"verification-v1"))
    );
    assert_eq!(
        decision.next_action(),
        &NextAction::FinalizeExecutionEvidence
    );
}

#[test]
fn finalized_evidence_advances_to_review_ready_and_drives_contributions() {
    let start = FlowRuntime::start(input_with(
        ProcessPhase::TestRunning,
        CompletionMilestone::None,
        CompletionDigestSet::ZERO,
    ));
    let verified = apply(
        &start,
        &verification(3, digest(b"code-v1"), digest(b"verification-v1")),
    );

    let ready = apply(&verified, &evidence_finalized(5, digest(b"evidence-v1")));

    assert_eq!(
        ready.snapshot().completion_milestone(),
        CompletionMilestone::ReviewReady
    );
    assert_eq!(ready.next_action(), &NextAction::CollectReviewContributions);

    let collected = apply(&ready, &contributions(8));
    assert_eq!(
        collected.snapshot().completion_milestone(),
        CompletionMilestone::ReviewReady
    );
    assert_eq!(collected.next_action(), &NextAction::FinalizeGovernance);
}

#[test]
fn review_pass_with_fresh_final_gates_closes_governance() {
    let closed = governance_closed();

    assert_eq!(
        closed.snapshot().completion_milestone(),
        CompletionMilestone::GovernanceClosed
    );
    assert_eq!(closed.snapshot().completion_bound(), bound_v1());
    assert_eq!(closed.next_action(), &NextAction::AwaitAgentWork);
}

#[test]
fn milestone_chain_cannot_skip_stages() {
    let start = FlowRuntime::start(input_with(
        ProcessPhase::TestRunning,
        CompletionMilestone::None,
        CompletionDigestSet::ZERO,
    ));

    let after_evidence = apply(&start, &evidence_finalized(3, digest(b"evidence-v1")));
    assert_eq!(
        after_evidence.snapshot().completion_milestone(),
        CompletionMilestone::None,
        "evidence finalization must not skip verification",
    );
    let after_governance = apply(
        &after_evidence,
        &governance(5, digest(b"review-input-v1"), digest(b"final-gates-v1")),
    );
    assert_eq!(
        after_governance.snapshot().completion_milestone(),
        CompletionMilestone::None,
        "governance finalization must not skip evidence",
    );
    let after_contributions = apply(&after_governance, &contributions(8));
    assert!(
        !after_contributions.review_contributions_ready(),
        "contributions must not stick before ReviewReady",
    );

    let verified = apply(
        &after_contributions,
        &verification(13, digest(b"code-v1"), digest(b"verification-v1")),
    );
    let skipped = apply(
        &verified,
        &governance(21, digest(b"review-input-v1"), digest(b"final-gates-v1")),
    );
    assert_eq!(
        skipped.snapshot().completion_milestone(),
        CompletionMilestone::ImplementationVerified,
        "governance finalization must not skip ReviewReady",
    );
}

#[test]
fn only_governance_closed_may_commit_completed() {
    for milestone in [
        CompletionMilestone::None,
        CompletionMilestone::ImplementationVerified,
        CompletionMilestone::ReviewReady,
    ] {
        let start = FlowRuntime::start(input_with(
            ProcessPhase::CodeReviewed,
            milestone,
            bound_v1(),
        ));
        let denied = apply(
            &start,
            &transition_request_to(29, AgentRole::Root, ProcessPhase::Completed),
        );
        assert_eq!(
            denied.next_action(),
            &NextAction::TransitionDenied {
                target: ProcessPhase::Completed,
                reason: TransitionPolicyError::CompletionMilestoneOpen { milestone },
            },
            "milestone {milestone:?} must not commit Completed",
        );
    }

    let closed = FlowRuntime::start(input_with(
        ProcessPhase::CodeReviewed,
        CompletionMilestone::GovernanceClosed,
        bound_v1(),
    ));
    let pending = apply(
        &closed,
        &transition_request_to(29, AgentRole::Root, ProcessPhase::Completed),
    );
    let NextAction::EvaluateGates {
        target: ProcessPhase::Completed,
        required_gates,
    } = pending.next_action()
    else {
        panic!("GovernanceClosed must open the Completed Gate evaluation")
    };
    assert_eq!(
        required_gates,
        &[RequiredGate::G00, RequiredGate::G12, RequiredGate::G13]
    );

    let first = apply(
        &pending,
        &gate_for(31, RequiredGate::G00, ae_sdd_domain::GateOutcome::Pass),
    );
    let second = apply(
        &first,
        &gate_for(37, RequiredGate::G12, ae_sdd_domain::GateOutcome::Pass),
    );
    let third = apply(
        &second,
        &gate_for(41, RequiredGate::G13, ae_sdd_domain::GateOutcome::Pass),
    );
    assert_eq!(
        third.next_action(),
        &NextAction::ApplyTransition {
            target: ProcessPhase::Completed,
        }
    );

    let committed = apply(&third, &support::commit_to(43, 8, ProcessPhase::Completed));
    assert_eq!(committed.snapshot().phase(), ProcessPhase::Completed);
    assert_eq!(
        committed.snapshot().completion_milestone(),
        CompletionMilestone::GovernanceClosed,
        "the milestone survives the terminal transition commit",
    );
}

#[test]
fn changed_input_digests_roll_back_to_the_earliest_affected_point() {
    let bound = bound_v1();
    let cases: [(&str, CompletionDigestSet, CompletionMilestone); 7] = [
        (
            "code",
            bound.with_code_digest(digest(b"code-v2")),
            CompletionMilestone::None,
        ),
        (
            "verification",
            bound.with_verification_digest(digest(b"verification-v2")),
            CompletionMilestone::None,
        ),
        (
            "evidence",
            bound.with_evidence_digest(digest(b"evidence-v2")),
            CompletionMilestone::ImplementationVerified,
        ),
        (
            "review input",
            bound.with_review_input_digest(digest(b"review-input-v2")),
            CompletionMilestone::ReviewReady,
        ),
        (
            "final gates",
            bound.with_gate_digest(digest(b"final-gates-v2")),
            CompletionMilestone::ReviewReady,
        ),
        (
            "evidence and review",
            bound
                .with_evidence_digest(digest(b"evidence-v2"))
                .with_review_input_digest(digest(b"review-input-v2")),
            CompletionMilestone::ImplementationVerified,
        ),
        (
            "verification and gates",
            bound
                .with_verification_digest(digest(b"verification-v2"))
                .with_gate_digest(digest(b"final-gates-v2")),
            CompletionMilestone::None,
        ),
    ];

    for (name, observed, expected) in cases {
        let closed = FlowRuntime::start(input_with(
            ProcessPhase::CodeReviewed,
            CompletionMilestone::GovernanceClosed,
            bound,
        ));
        let rolled = apply(&closed, &inputs_changed(47, observed));
        assert_eq!(
            rolled.snapshot().completion_milestone(),
            expected,
            "changed {name} must roll back to the earliest affected point",
        );
        assert_ne!(
            rolled.snapshot().completion_milestone(),
            CompletionMilestone::GovernanceClosed,
            "changed {name} must never keep GovernanceClosed",
        );
    }
}

#[test]
fn unchanged_input_digests_keep_governance_closed() {
    let closed = FlowRuntime::start(input_with(
        ProcessPhase::CodeReviewed,
        CompletionMilestone::GovernanceClosed,
        bound_v1(),
    ));

    let kept = apply(&closed, &inputs_changed(47, bound_v1()));

    assert_eq!(
        kept.snapshot().completion_milestone(),
        CompletionMilestone::GovernanceClosed
    );
}

#[test]
fn review_input_change_resets_collected_contributions() {
    let ready = apply(
        &apply(
            &governance_closed(),
            &inputs_changed(
                47,
                bound_v1().with_review_input_digest(digest(b"review-input-v2")),
            ),
        ),
        &event(53, b"noop", FlowEventKind::PromptAccepted),
    );

    assert_eq!(
        ready.snapshot().completion_milestone(),
        CompletionMilestone::ReviewReady
    );
    assert!(
        !ready.review_contributions_ready(),
        "stale review inputs must reset the collected contribution marker",
    );
    assert_eq!(ready.next_action(), &NextAction::CollectReviewContributions);
}

#[test]
fn a_rolled_back_milestone_drives_the_chain_to_governance_closed_again() {
    let closed = governance_closed();
    let rolled = apply(
        &closed,
        &inputs_changed(47, bound_v1().with_evidence_digest(digest(b"evidence-v2"))),
    );
    assert_eq!(
        rolled.snapshot().completion_milestone(),
        CompletionMilestone::ImplementationVerified
    );
    assert_eq!(rolled.next_action(), &NextAction::FinalizeExecutionEvidence);

    let ready = apply(&rolled, &evidence_finalized(53, digest(b"evidence-v2")));
    assert_eq!(
        ready.snapshot().completion_milestone(),
        CompletionMilestone::ReviewReady
    );
    let collected = apply(&ready, &contributions(59));
    assert_eq!(collected.next_action(), &NextAction::FinalizeGovernance);
    let reclosed = apply(
        &collected,
        &governance(61, digest(b"review-input-v1"), digest(b"final-gates-v1")),
    );
    assert_eq!(
        reclosed.snapshot().completion_milestone(),
        CompletionMilestone::GovernanceClosed
    );
}

#[test]
fn a_stale_governance_close_cannot_commit_completed() {
    let closed = FlowRuntime::start(input_with(
        ProcessPhase::CodeReviewed,
        CompletionMilestone::GovernanceClosed,
        bound_v1(),
    ));
    let pending = apply(
        &closed,
        &transition_request_to(29, AgentRole::Root, ProcessPhase::Completed),
    );
    let first = apply(
        &pending,
        &gate_for(31, RequiredGate::G00, ae_sdd_domain::GateOutcome::Pass),
    );
    let second = apply(
        &first,
        &gate_for(37, RequiredGate::G12, ae_sdd_domain::GateOutcome::Pass),
    );
    let ready = apply(
        &second,
        &gate_for(41, RequiredGate::G13, ae_sdd_domain::GateOutcome::Pass),
    );
    let stale = apply(
        &ready,
        &inputs_changed(
            43,
            bound_v1().with_review_input_digest(digest(b"review-input-v2")),
        ),
    );
    assert_eq!(
        stale.snapshot().completion_milestone(),
        CompletionMilestone::ReviewReady
    );

    let commit = FlowRuntime::apply(&stale, &support::commit_to(47, 8, ProcessPhase::Completed));

    assert!(
        matches!(
            commit,
            Err(ae_sdd_flow::FlowError::TransitionNotReady {
                target: ProcessPhase::Completed
            })
        ),
        "a Completed commit after a governance rollback must fail closed",
    );
}

#[test]
fn completion_milestone_takes_part_in_the_replay_digest() {
    let plain = FlowRuntime::start(input());
    let verified = FlowRuntime::start(input_with(
        ProcessPhase::Initialized,
        CompletionMilestone::ImplementationVerified,
        bound_v1(),
    ));

    assert_ne!(
        plain.decision_digest(),
        verified.decision_digest(),
        "the milestone dimension must enter the canonical decision digest",
    );
}

#[test]
fn completion_log_replay_digest_is_identical_across_runs_and_orderings() {
    let log = || {
        vec![
            verification(3, digest(b"code-v1"), digest(b"verification-v1")),
            evidence_finalized(5, digest(b"evidence-v1")),
            contributions(8),
            governance(13, digest(b"review-input-v1"), digest(b"final-gates-v1")),
            inputs_changed(17, bound_v1()),
        ]
    };
    let input = input_with(
        ProcessPhase::TestRunning,
        CompletionMilestone::None,
        CompletionDigestSet::ZERO,
    );

    let first = FlowRuntime::replay(input, log()).expect("ordered log is valid");
    let second = FlowRuntime::replay(input, log()).expect("the same log replays");
    let reordered =
        FlowRuntime::replay(input, log().into_iter().rev()).expect("reordered delivery converges");

    assert_eq!(first, second);
    assert_eq!(first.decision_digest(), second.decision_digest());
    assert_eq!(first.decision_digest(), reordered.decision_digest());
    assert_eq!(
        first.snapshot().completion_milestone(),
        CompletionMilestone::GovernanceClosed
    );
}

#[test]
fn milestone_events_do_not_hijack_a_pending_transition() {
    let start = FlowRuntime::start(input_with(
        ProcessPhase::CodeReviewed,
        CompletionMilestone::None,
        CompletionDigestSet::ZERO,
    ));
    let pending = apply(
        &start,
        &transition_request_to(29, AgentRole::Root, ProcessPhase::Completed),
    );
    assert!(matches!(
        pending.next_action(),
        NextAction::TransitionDenied { .. }
    ));

    let verified = apply(
        &start,
        &verification(3, digest(b"code-v1"), digest(b"verification-v1")),
    );
    let requested = apply(
        &verified,
        &transition_request_to(29, AgentRole::Root, ProcessPhase::Paused),
    );
    let advanced = apply(&requested, &evidence_finalized(31, digest(b"evidence-v1")));

    assert_eq!(
        advanced.snapshot().completion_milestone(),
        CompletionMilestone::ReviewReady,
        "milestone events still advance while a transition is pending",
    );
    assert_eq!(
        advanced.next_action(),
        &NextAction::ApplyTransition {
            target: ProcessPhase::Paused,
        },
        "the pending transition keeps owning the next action",
    );
}
