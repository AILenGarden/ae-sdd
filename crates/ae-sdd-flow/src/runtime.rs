use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;
use ae_sdd_domain::{
    AgentRole, CompletionMilestone, DecisionDigest, DesignRoute, EventSequence, ProcessPhase,
    WorkScale,
};
use ae_sdd_policy::{
    GateDirective, GateTruth, InfrastructureImpact, RequiredGate, RoleOperation, TransitionContext,
    TransitionPolicy, TransitionPolicyError,
};

use crate::{
    CompactAdviceReason, ExecutionCursor, FlowDecision, FlowError, FlowEvent, FlowEventKind,
    FlowInput, FlowSnapshot, NextAction, SupervisorDegradation, SupervisorHealth, canonical,
};

/// Pure deterministic flow reducer and replay entry point.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlowRuntime;

impl FlowRuntime {
    /// Starts a checkpoint from explicit authoritative inputs.
    pub fn start(input: FlowInput) -> FlowDecision {
        let mut decision = FlowDecision {
            environment: input.environment(),
            snapshot: input.snapshot(),
            pending_transition: None,
            required_gates: Vec::new(),
            passed_gates: BTreeSet::new(),
            health: SupervisorHealth::Healthy,
            next_action: NextAction::AwaitAgentWork,
            execution_cursor: input.environment().execution_cursor(),
            review_contributions_ready: false,
            last_cursor: None,
            last_event_fingerprint: None,
            decision_digest: DecisionDigest::from_array([0; 32]),
        };
        decision.next_action = idle_action(&decision);
        decision.decision_digest = digest_decision(None, &decision);
        decision
    }

    /// Reduces one committed event against a durable decision checkpoint.
    ///
    /// Global event sequences need only increase for this Work Item; gaps may
    /// represent unrelated events already skipped by the durable subscription.
    /// An exact duplicate is a no-op and preserves the decision digest.
    pub fn apply(checkpoint: &FlowDecision, event: &FlowEvent) -> Result<FlowDecision, FlowError> {
        validate_event(checkpoint, event)?;

        let provenance = event.provenance();
        if let Some(cursor) = checkpoint.last_cursor {
            if provenance.cursor().sequence() < cursor.sequence() {
                return Ok(checkpoint.clone());
            }
            if provenance.cursor().sequence() == cursor.sequence() {
                return if checkpoint.last_event_fingerprint == Some(provenance.event_fingerprint())
                {
                    Ok(checkpoint.clone())
                } else {
                    Err(FlowError::EventSequenceConflict {
                        sequence: cursor.sequence(),
                    })
                };
            }
        }

        let mut next = checkpoint.clone();
        reduce_kind(&mut next, event.kind())?;
        next.last_cursor = Some(provenance.cursor());
        next.last_event_fingerprint = Some(provenance.event_fingerprint());
        next.decision_digest = digest_decision(Some(checkpoint.decision_digest), &next);
        Ok(next)
    }

    /// Sorts, de-duplicates, and replays a committed event batch.
    ///
    /// Reordered input converges to the same decision as ordered input. Reusing
    /// one immutable sequence with different content fails closed.
    pub fn replay(
        input: FlowInput,
        events: impl IntoIterator<Item = FlowEvent>,
    ) -> Result<FlowDecision, FlowError> {
        let mut ordered = BTreeMap::<u64, FlowEvent>::new();
        for event in events {
            let sequence = event.provenance().cursor().sequence().get();
            match ordered.entry(sequence) {
                Entry::Vacant(entry) => {
                    entry.insert(event);
                }
                Entry::Occupied(entry) if entry.get() == &event => {}
                Entry::Occupied(_) => {
                    return Err(FlowError::EventSequenceConflict {
                        sequence: EventSequence::new(sequence),
                    });
                }
            }
        }

        let mut decision = Self::start(input);
        for event in ordered.values() {
            decision = Self::apply(&decision, event)?;
        }
        Ok(decision)
    }
}

fn validate_event(checkpoint: &FlowDecision, event: &FlowEvent) -> Result<(), FlowError> {
    let provenance = event.provenance();
    let cursor = provenance.cursor();
    if cursor.sequence() == EventSequence::ZERO {
        return Err(FlowError::InvalidEventSequence);
    }
    if cursor.event_store_id() != checkpoint.environment.event_store_id() {
        return Err(FlowError::EventStoreMismatch {
            expected: checkpoint.environment.event_store_id(),
            actual: cursor.event_store_id(),
        });
    }
    if provenance.policy_digest() != checkpoint.environment.policy_digest() {
        return Err(FlowError::PolicyDigestMismatch {
            expected: checkpoint.environment.policy_digest(),
            actual: provenance.policy_digest(),
        });
    }
    if provenance.input_fingerprint() != checkpoint.environment.input_fingerprint() {
        return Err(FlowError::InputFingerprintMismatch {
            expected: checkpoint.environment.input_fingerprint(),
            actual: provenance.input_fingerprint(),
        });
    }
    Ok(())
}

fn reduce_kind(decision: &mut FlowDecision, event: &FlowEventKind) -> Result<(), FlowError> {
    match event {
        FlowEventKind::PromptAccepted => {
            // Prompt text is neither process evidence nor a correction signal.
        }
        FlowEventKind::TransitionRequested { actor_role, target } => {
            if let Some(pending) = decision.pending_transition {
                if pending == *target && *actor_role == AgentRole::Root {
                    return Ok(());
                }
                return Err(FlowError::TransitionAlreadyPending {
                    pending,
                    requested: *target,
                });
            }
            let current = decision.snapshot.phase();
            let route = decision.environment.route();
            if current == ProcessPhase::RequirementAnalyzed
                && *target == ProcessPhase::RouteSelected
                && matches!(route, crate::RouteLifecycle::Pending)
            {
                return Err(FlowError::RouteCandidateMissing { target: *target });
            }
            if phase_consumes_frozen_route(current)
                && *target != ProcessPhase::Paused
                && !matches!(route, crate::RouteLifecycle::Frozen(_))
            {
                return Err(FlowError::RouteNotFrozen { target: *target });
            }
            let selection = decision.environment.route().selection().unwrap_or_else(|| {
                // TransitionPolicy ignores these sentinels for the route-less
                // Initialized -> RequirementAnalyzed edge.
                crate::RouteSelection::new(WorkScale::Micro, DesignRoute::CodingPlan)
            });
            let context = TransitionContext {
                actor_role: *actor_role,
                current: decision.snapshot.phase(),
                target: *target,
                scale: selection.scale(),
                design_route: selection.design_route(),
                paused_from: decision.snapshot.paused_from(),
            };
            match TransitionPolicy::authorize(context) {
                Ok(permit) => {
                    let completion_denied = if *target == ProcessPhase::Completed {
                        TransitionPolicy::authorize_completion(
                            decision.snapshot.completion_milestone(),
                        )
                        .err()
                    } else {
                        None
                    };
                    if let Some(reason) = completion_denied {
                        decision.next_action = NextAction::TransitionDenied {
                            target: *target,
                            reason,
                        };
                    } else {
                        decision.pending_transition = Some(*target);
                        decision.required_gates = permit.required_gates().to_vec();
                        decision.passed_gates.clear();
                        decision.next_action = if decision.required_gates.is_empty() {
                            NextAction::ApplyTransition { target: *target }
                        } else {
                            NextAction::EvaluateGates {
                                target: *target,
                                required_gates: decision.required_gates.clone(),
                            }
                        };
                    }
                }
                Err(reason) => {
                    decision.next_action = NextAction::TransitionDenied {
                        target: *target,
                        reason,
                    };
                }
            }
        }
        FlowEventKind::GateCompleted { gate, outcome } => {
            let Some(target) = decision.pending_transition else {
                return Err(FlowError::UnexpectedGateOutcome);
            };
            if !decision.required_gates.contains(gate) {
                return Err(FlowError::UnexpectedGate { gate: *gate });
            }
            let judgement = GateTruth::judge(outcome);
            if judgement.correction_delta() != 0 {
                let Some(correction_count) = decision
                    .snapshot
                    .correction_count()
                    .checked_add(judgement.correction_delta())
                else {
                    return Err(FlowError::CorrectionOverflow);
                };
                decision.snapshot = decision.snapshot.with_correction_count(correction_count);
            }
            if judgement.infrastructure_impact() == InfrastructureImpact::Degraded {
                decision.health = match outcome {
                    ae_sdd_domain::GateOutcome::Timeout(_) => {
                        SupervisorHealth::Degraded(SupervisorDegradation::GateTimeout)
                    }
                    _ => SupervisorHealth::Degraded(SupervisorDegradation::GateError),
                };
            }
            decision.next_action = match judgement.directive() {
                GateDirective::Proceed => {
                    decision.passed_gates.insert(*gate);
                    let remaining: Vec<_> = decision
                        .required_gates
                        .iter()
                        .copied()
                        .filter(|required| !decision.passed_gates.contains(required))
                        .collect();
                    if remaining.is_empty() {
                        NextAction::ApplyTransition { target }
                    } else {
                        NextAction::EvaluateGates {
                            target,
                            required_gates: remaining,
                        }
                    }
                }
                GateDirective::Correct => NextAction::ProvideCorrection,
                GateDirective::Retry => NextAction::RetryGate,
                GateDirective::Halt => NextAction::HaltForGateError,
                GateDirective::AwaitCancellationResolution => {
                    NextAction::AwaitCancellationResolution
                }
                GateDirective::Reevaluate => {
                    decision.passed_gates.clear();
                    NextAction::ReevaluateGate
                }
            };
        }
        FlowEventKind::TransitionCommitted {
            phase,
            state_revision,
        } => {
            if decision.pending_transition != Some(*phase) {
                return Err(FlowError::UnexpectedTransitionCommit {
                    pending: decision.pending_transition,
                    committed: *phase,
                });
            }
            if !matches!(
                decision.next_action,
                NextAction::ApplyTransition { target } if target == *phase
            ) {
                return Err(FlowError::TransitionNotReady { target: *phase });
            }
            if *state_revision <= decision.snapshot.state_revision() {
                return Err(FlowError::NonMonotonicStateRevision {
                    current: decision.snapshot.state_revision(),
                    committed: *state_revision,
                });
            }
            if *phase == ProcessPhase::Completed
                && TransitionPolicy::authorize_completion(decision.snapshot.completion_milestone())
                    .is_err()
            {
                return Err(FlowError::TransitionNotReady { target: *phase });
            }
            let paused_from = if *phase == ProcessPhase::Paused {
                Some(decision.snapshot.phase())
            } else {
                None
            };
            decision.snapshot = FlowSnapshot::new(
                *phase,
                *state_revision,
                decision.snapshot.correction_count(),
            )
            .with_completion_milestone(
                decision.snapshot.completion_milestone(),
                decision.snapshot.completion_bound(),
            );
            if let Some(paused_from) = paused_from {
                decision.snapshot = decision.snapshot.with_paused_from(paused_from);
            }
            decision.pending_transition = None;
            decision.required_gates.clear();
            decision.passed_gates.clear();
            decision.next_action = idle_action(decision);
        }
        FlowEventKind::ExecutionQueueApproved { cursor } => {
            let previous = decision.execution_cursor;
            decision.execution_cursor = Some(*cursor);
            // A pending transition keeps owning the action until it commits.
            if decision.pending_transition.is_none() {
                decision.next_action = match previous {
                    Some(previous) if previous.queue_digest() != cursor.queue_digest() => {
                        NextAction::ResumeApprovedExecution
                    }
                    _ => idle_action(decision),
                };
            }
        }
        FlowEventKind::VerificationFreshnessObserved {
            code_digest,
            verification_digest,
        } => {
            if decision.snapshot.completion_milestone() == CompletionMilestone::None {
                let bound = decision
                    .snapshot
                    .completion_bound()
                    .with_code_digest(*code_digest)
                    .with_verification_digest(*verification_digest);
                decision.snapshot = decision
                    .snapshot
                    .with_completion_milestone(CompletionMilestone::ImplementationVerified, bound);
            }
            refresh_idle_action(decision);
        }
        FlowEventKind::ExecutionEvidenceFinalized { evidence_digest } => {
            if decision.snapshot.completion_milestone()
                == CompletionMilestone::ImplementationVerified
            {
                let bound = decision
                    .snapshot
                    .completion_bound()
                    .with_evidence_digest(*evidence_digest);
                decision.snapshot = decision
                    .snapshot
                    .with_completion_milestone(CompletionMilestone::ReviewReady, bound);
                decision.review_contributions_ready = false;
            }
            refresh_idle_action(decision);
        }
        FlowEventKind::ReviewContributionsCollected => {
            if decision.snapshot.completion_milestone() == CompletionMilestone::ReviewReady {
                decision.review_contributions_ready = true;
            }
            refresh_idle_action(decision);
        }
        FlowEventKind::GovernanceFinalized {
            review_input_digest,
            gate_digest,
        } => {
            if decision.snapshot.completion_milestone() == CompletionMilestone::ReviewReady
                && decision.review_contributions_ready
            {
                let bound = decision
                    .snapshot
                    .completion_bound()
                    .with_review_input_digest(*review_input_digest)
                    .with_gate_digest(*gate_digest);
                decision.snapshot = decision
                    .snapshot
                    .with_completion_milestone(CompletionMilestone::GovernanceClosed, bound);
                decision.review_contributions_ready = false;
            }
            refresh_idle_action(decision);
        }
        FlowEventKind::CompletionInputsChanged { observed } => {
            let current = decision.snapshot.completion_milestone();
            let rolled = current.invalidate(&decision.snapshot.completion_bound(), observed);
            if rolled != current {
                decision.snapshot = decision
                    .snapshot
                    .with_completion_milestone(rolled, decision.snapshot.completion_bound());
                decision.review_contributions_ready = false;
            }
            refresh_idle_action(decision);
        }
        FlowEventKind::BackgroundFault(fault) => {
            decision.health = SupervisorHealth::Degraded(SupervisorDegradation::Background(*fault));
        }
        FlowEventKind::BackgroundRecovered => {
            decision.health = SupervisorHealth::Healthy;
        }
        FlowEventKind::SeriesCompleted => {
            // Advisory only: pending transitions, execution slices, and
            // completion-chain work keep owning the action. The suggestion
            // never starts a compact cycle by itself.
            if decision.pending_transition.is_none()
                && decision.next_action == NextAction::AwaitAgentWork
            {
                decision.next_action = NextAction::SuggestCompact {
                    reason: CompactAdviceReason::SeriesBoundary,
                };
            }
        }
    }
    Ok(())
}

const fn phase_consumes_frozen_route(phase: ProcessPhase) -> bool {
    matches!(
        phase,
        ProcessPhase::RouteSelected
            | ProcessPhase::DrGenerated
            | ProcessPhase::StoryGenerated
            | ProcessPhase::TestcaseGenerated
            | ProcessPhase::CodingProcess
            | ProcessPhase::Coding
            | ProcessPhase::TestRunning
            | ProcessPhase::CodeReviewed
            | ProcessPhase::Completed
    )
}

/// Derives the execution-surface action for the policy-owned execution phase.
///
/// An open approved slice keeps executing; anything else leaves the caller's
/// fallback action untouched so no second phase state machine appears.
fn execution_action(phase: ProcessPhase, cursor: Option<ExecutionCursor>) -> Option<NextAction> {
    if !TransitionPolicy::is_execution_phase(phase) {
        return None;
    }
    let cursor = cursor?;
    let active_slice_status = cursor.active_slice_status();
    let next_slice_transition = next_slice_transition(active_slice_status)?;
    Some(NextAction::ExecuteApprovedSlice {
        active_ordinal: cursor.active_ordinal(),
        queue_digest: cursor.queue_digest(),
        capsule_digest: cursor.capsule_digest(),
        active_slice_status,
        next_slice_transition,
    })
}

/// Returns the sole legal forward transition for an open execution slice.
const fn next_slice_transition(status: ExecutionSliceStatus) -> Option<ExecutionSliceStatus> {
    match status {
        ExecutionSliceStatus::Pending => Some(ExecutionSliceStatus::Running),
        ExecutionSliceStatus::Running => Some(ExecutionSliceStatus::RedObserved),
        ExecutionSliceStatus::RedObserved => Some(ExecutionSliceStatus::Patched),
        ExecutionSliceStatus::Patched => Some(ExecutionSliceStatus::FocusedGreen),
        ExecutionSliceStatus::FocusedGreen => Some(ExecutionSliceStatus::EvidenceBound),
        ExecutionSliceStatus::EvidenceBound => Some(ExecutionSliceStatus::Completed),
        ExecutionSliceStatus::Completed | ExecutionSliceStatus::Blocked => None,
    }
}

/// Recomputes the idle action unless a pending transition owns the decision.
fn refresh_idle_action(decision: &mut FlowDecision) {
    if decision.pending_transition.is_none() {
        decision.next_action = idle_action(decision);
    }
}

/// Derives the idle action from the execution surface and the completion
/// milestone chain; the execution surface always wins over milestone work.
fn idle_action(decision: &FlowDecision) -> NextAction {
    execution_action(decision.snapshot.phase(), decision.execution_cursor)
        .or_else(|| {
            completion_action(
                decision.snapshot.completion_milestone(),
                decision.review_contributions_ready,
            )
        })
        .unwrap_or(NextAction::AwaitAgentWork)
}

/// Maps the completion milestone to the next chain-driving action.
///
/// `None` and `GovernanceClosed` produce no action: the chain still needs
/// verification freshness, or is already complete and waits for the regular
/// transition flow.
fn completion_action(
    milestone: CompletionMilestone,
    review_contributions_ready: bool,
) -> Option<NextAction> {
    match milestone {
        CompletionMilestone::None | CompletionMilestone::GovernanceClosed => None,
        CompletionMilestone::ImplementationVerified => Some(NextAction::FinalizeExecutionEvidence),
        CompletionMilestone::ReviewReady if review_contributions_ready => {
            Some(NextAction::FinalizeGovernance)
        }
        CompletionMilestone::ReviewReady => Some(NextAction::CollectReviewContributions),
    }
}

fn digest_decision(previous: Option<DecisionDigest>, decision: &FlowDecision) -> DecisionDigest {
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(b"ae-sdd-flow-decision/v2\0");
    encode_option_digest(&mut bytes, previous);
    bytes.extend_from_slice(decision.environment.event_store_id().as_uuid().as_bytes());
    bytes.extend_from_slice(decision.environment.policy_digest().as_bytes());
    bytes.extend_from_slice(decision.environment.input_fingerprint().as_bytes());
    match decision.environment.route() {
        crate::RouteLifecycle::Pending => bytes.push(0),
        crate::RouteLifecycle::Candidate(selection) => {
            bytes.push(1);
            bytes.push(scale_tag(selection.scale()));
            bytes.push(route_tag(selection.design_route()));
        }
        crate::RouteLifecycle::Frozen(selection) => {
            bytes.push(2);
            bytes.push(scale_tag(selection.scale()));
            bytes.push(route_tag(selection.design_route()));
        }
    }
    bytes.push(phase_tag(decision.snapshot.phase()));
    encode_optional_phase(&mut bytes, decision.snapshot.paused_from());
    bytes.extend_from_slice(&decision.snapshot.state_revision().get().to_be_bytes());
    bytes.extend_from_slice(&decision.snapshot.correction_count().to_be_bytes());
    canonical::execution_cursor(&mut bytes, decision.execution_cursor);
    canonical::completion(
        &mut bytes,
        decision.snapshot.completion_milestone(),
        &decision.snapshot.completion_bound(),
        decision.review_contributions_ready,
    );
    encode_optional_phase(&mut bytes, decision.pending_transition);
    encode_gates(&mut bytes, &decision.required_gates);
    encode_gates(
        &mut bytes,
        &decision.passed_gates.iter().copied().collect::<Vec<_>>(),
    );
    encode_health(&mut bytes, decision.health);
    encode_action(&mut bytes, &decision.next_action);
    match decision.last_cursor {
        Some(cursor) => {
            bytes.push(1);
            bytes.extend_from_slice(cursor.event_store_id().as_uuid().as_bytes());
            bytes.extend_from_slice(&cursor.sequence().get().to_be_bytes());
        }
        None => bytes.push(0),
    }
    match decision.last_event_fingerprint {
        Some(fingerprint) => {
            bytes.push(1);
            bytes.extend_from_slice(fingerprint.as_bytes());
        }
        None => bytes.push(0),
    }
    DecisionDigest::digest(bytes)
}

fn encode_option_digest(bytes: &mut Vec<u8>, digest: Option<DecisionDigest>) {
    match digest {
        Some(digest) => {
            bytes.push(1);
            bytes.extend_from_slice(digest.as_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_optional_phase(bytes: &mut Vec<u8>, phase: Option<ProcessPhase>) {
    match phase {
        Some(phase) => {
            bytes.push(1);
            bytes.push(phase_tag(phase));
        }
        None => bytes.push(0),
    }
}

fn encode_health(bytes: &mut Vec<u8>, health: SupervisorHealth) {
    use SupervisorDegradation::{Background, GateError, GateTimeout};
    match health {
        SupervisorHealth::Healthy => bytes.push(0),
        SupervisorHealth::Degraded(GateError) => bytes.extend_from_slice(&[1, 0]),
        SupervisorHealth::Degraded(GateTimeout) => bytes.extend_from_slice(&[1, 1]),
        SupervisorHealth::Degraded(Background(fault)) => {
            bytes.extend_from_slice(&[1, 2, fault_tag(fault)]);
        }
    }
}

fn encode_action(bytes: &mut Vec<u8>, action: &NextAction) {
    match action {
        NextAction::AwaitAgentWork => bytes.push(0),
        NextAction::EvaluateGates {
            target,
            required_gates,
        } => {
            bytes.extend_from_slice(&[1, phase_tag(*target)]);
            bytes.extend_from_slice(&(required_gates.len() as u64).to_be_bytes());
            for gate in required_gates {
                let gate = gate.as_str().as_bytes();
                bytes.extend_from_slice(&(gate.len() as u64).to_be_bytes());
                bytes.extend_from_slice(gate);
            }
        }
        NextAction::ApplyTransition { target } => {
            bytes.extend_from_slice(&[2, phase_tag(*target)]);
        }
        NextAction::ProvideCorrection => bytes.push(3),
        NextAction::RetryGate => bytes.push(4),
        NextAction::HaltForGateError => bytes.push(5),
        NextAction::AwaitCancellationResolution => bytes.push(6),
        NextAction::ReevaluateGate => bytes.push(7),
        NextAction::TransitionDenied { target, reason } => {
            bytes.extend_from_slice(&[8, phase_tag(*target)]);
            encode_transition_error(bytes, *reason);
        }
        NextAction::ResumeApprovedExecution => bytes.push(9),
        NextAction::ExecuteApprovedSlice {
            active_ordinal,
            queue_digest,
            capsule_digest,
            active_slice_status,
            next_slice_transition,
        } => {
            bytes.push(10);
            bytes.extend_from_slice(&active_ordinal.to_be_bytes());
            bytes.extend_from_slice(queue_digest.as_bytes());
            bytes.extend_from_slice(capsule_digest.as_bytes());
            bytes.push(canonical::execution_slice_status_tag(*active_slice_status));
            bytes.push(canonical::execution_slice_status_tag(
                *next_slice_transition,
            ));
        }
        NextAction::FinalizeExecutionEvidence => bytes.push(11),
        NextAction::CollectReviewContributions => bytes.push(12),
        NextAction::FinalizeGovernance => bytes.push(13),
        NextAction::SuggestCompact { reason } => {
            bytes.extend_from_slice(&[14, compact_advice_reason_tag(*reason)]);
        }
    }
}

const fn compact_advice_reason_tag(reason: CompactAdviceReason) -> u8 {
    match reason {
        CompactAdviceReason::SeriesBoundary => 0,
    }
}

fn encode_gates(bytes: &mut Vec<u8>, gates: &[RequiredGate]) {
    bytes.extend_from_slice(&(gates.len() as u64).to_be_bytes());
    for gate in gates {
        let gate = gate.as_str().as_bytes();
        bytes.extend_from_slice(&(gate.len() as u64).to_be_bytes());
        bytes.extend_from_slice(gate);
    }
}

fn encode_transition_error(bytes: &mut Vec<u8>, error: TransitionPolicyError) {
    match error {
        TransitionPolicyError::Role(error) => {
            bytes.extend_from_slice(&[
                0,
                role_tag(error.role()),
                role_operation_tag(error.operation()),
            ]);
        }
        TransitionPolicyError::UnsupportedRoute {
            scale,
            design_route,
        } => bytes.extend_from_slice(&[1, scale_tag(scale), route_tag(design_route)]),
        TransitionPolicyError::PhaseOutsideRoute { phase } => {
            bytes.extend_from_slice(&[2, phase_tag(phase)]);
        }
        TransitionPolicyError::IllegalTransition { current, target } => {
            bytes.extend_from_slice(&[3, phase_tag(current), phase_tag(target)]);
        }
        TransitionPolicyError::InvalidResume { recorded, target } => {
            bytes.push(4);
            encode_optional_phase(bytes, recorded);
            bytes.push(phase_tag(target));
        }
        TransitionPolicyError::CompletionMilestoneOpen { milestone } => {
            bytes.extend_from_slice(&[5, canonical::completion_milestone_tag(milestone)]);
        }
    }
}

const fn phase_tag(phase: ProcessPhase) -> u8 {
    match phase {
        ProcessPhase::Initialized => 0,
        ProcessPhase::RouteSelected => 1,
        ProcessPhase::RequirementAnalyzed => 2,
        ProcessPhase::DrGenerated => 3,
        ProcessPhase::StoryGenerated => 4,
        ProcessPhase::TestcaseGenerated => 5,
        ProcessPhase::CodingProcess => 6,
        ProcessPhase::Coding => 7,
        ProcessPhase::TestRunning => 8,
        ProcessPhase::CodeReviewed => 9,
        ProcessPhase::Completed => 10,
        ProcessPhase::Paused => 11,
    }
}

const fn role_tag(role: AgentRole) -> u8 {
    match role {
        AgentRole::Root => 0,
        AgentRole::Series => 1,
        AgentRole::Task => 2,
        AgentRole::Reviewer => 3,
    }
}

const fn scale_tag(scale: WorkScale) -> u8 {
    match scale {
        WorkScale::Large => 0,
        WorkScale::Medium => 1,
        WorkScale::Small => 2,
        WorkScale::Micro => 3,
    }
}

const fn route_tag(route: DesignRoute) -> u8 {
    match route {
        DesignRoute::Dr => 0,
        DesignRoute::Story => 1,
        DesignRoute::CodingPlan => 2,
    }
}

const fn fault_tag(fault: crate::SupervisorFault) -> u8 {
    match fault {
        crate::SupervisorFault::GateWorker => 0,
        crate::SupervisorFault::EventStore => 1,
        crate::SupervisorFault::ArtifactProjection => 2,
        crate::SupervisorFault::HostAdapter => 3,
    }
}

const fn role_operation_tag(operation: RoleOperation) -> u8 {
    match operation {
        RoleOperation::SelectRoute => 0,
        RoleOperation::RequestGlobalTransition => 1,
        RoleOperation::ApproveExecutionPlan => 2,
        RoleOperation::CreateSeriesDelegation => 3,
        RoleOperation::CreateTaskDelegation => 4,
        RoleOperation::CreateReviewerDelegation => 5,
        RoleOperation::CollectChildResult => 6,
        RoleOperation::ReportProgress => 7,
        RoleOperation::ReadBoundedProjection => 8,
        RoleOperation::ReadAuthorizedArtifacts => 9,
        RoleOperation::SubmitChildResult => 10,
        RoleOperation::ModifyAssignedPaths => 11,
        RoleOperation::RunAssignedTests => 12,
        RoleOperation::SubmitEvidence => 13,
        RoleOperation::ReviewAssignedDiff => 14,
        RoleOperation::SubmitReviewFindings => 15,
        RoleOperation::BreakLease => 16,
        RoleOperation::ManageOwnLease => 17,
    }
}
