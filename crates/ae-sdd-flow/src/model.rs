use std::collections::BTreeSet;

use ae_sdd_domain::{
    AgentRole, DecisionDigest, DesignRoute, EventSequence, EventStoreId, GateOutcome,
    InputFingerprint, PolicyDigest, ProcessPhase, StateRevision, WorkScale,
};
use ae_sdd_policy::{RequiredGate, TransitionPolicy, TransitionPolicyError};

/// Store-scoped position in the durable global event stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventCursor {
    event_store_id: EventStoreId,
    sequence: EventSequence,
}

impl EventCursor {
    /// Creates a cursor. Sequence zero is rejected when the event is reduced.
    pub const fn new(event_store_id: EventStoreId, sequence: EventSequence) -> Self {
        Self {
            event_store_id,
            sequence,
        }
    }

    /// Returns the durable store epoch that owns the sequence.
    pub const fn event_store_id(self) -> EventStoreId {
        self.event_store_id
    }

    /// Returns the globally monotonic sequence inside the store epoch.
    pub const fn sequence(self) -> EventSequence {
        self.sequence
    }
}

/// Immutable provenance bound to every typed flow event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventProvenance {
    cursor: EventCursor,
    policy_digest: PolicyDigest,
    input_fingerprint: InputFingerprint,
    event_fingerprint: InputFingerprint,
}

impl EventProvenance {
    /// Creates provenance previously validated by the durable event adapter.
    pub const fn new(
        cursor: EventCursor,
        policy_digest: PolicyDigest,
        input_fingerprint: InputFingerprint,
        event_fingerprint: InputFingerprint,
    ) -> Self {
        Self {
            cursor,
            policy_digest,
            input_fingerprint,
            event_fingerprint,
        }
    }

    /// Returns the event cursor.
    pub const fn cursor(self) -> EventCursor {
        self.cursor
    }

    /// Returns the policy digest captured when the event was committed.
    pub const fn policy_digest(self) -> PolicyDigest {
        self.policy_digest
    }

    /// Returns the flow input fingerprint captured at commit.
    pub const fn input_fingerprint(self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the canonical typed-payload fingerprint validated by the store.
    pub const fn event_fingerprint(self) -> InputFingerprint {
        self.event_fingerprint
    }
}

/// Infrastructure fault classes that do not count as business corrections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SupervisorFault {
    GateWorker,
    EventStore,
    ArtifactProjection,
    HostAdapter,
}

/// Typed event payload consumed by the pure supervisor reducer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowEventKind {
    /// A prompt was accepted as an input signal, never as failure evidence.
    PromptAccepted,
    /// A daemon-trusted Agent role requests a global phase transition.
    TransitionRequested {
        actor_role: AgentRole,
        target: ProcessPhase,
    },
    /// One required Gate evaluation completed with its six-state outcome.
    GateCompleted {
        gate: RequiredGate,
        outcome: GateOutcome,
    },
    /// The authoritative store committed the pending phase transition.
    TransitionCommitted {
        phase: ProcessPhase,
        state_revision: StateRevision,
    },
    /// A background adapter reported an infrastructure fault.
    BackgroundFault(SupervisorFault),
    /// Recovery evidence cleared the current infrastructure degradation.
    BackgroundRecovered,
}

/// A typed durable event and its immutable provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowEvent {
    provenance: EventProvenance,
    kind: FlowEventKind,
}

impl FlowEvent {
    /// Creates an event from store-validated provenance and typed payload.
    pub const fn new(provenance: EventProvenance, kind: FlowEventKind) -> Self {
        Self { provenance, kind }
    }

    /// Returns the immutable event provenance.
    pub const fn provenance(&self) -> EventProvenance {
        self.provenance
    }

    /// Returns the typed event payload.
    pub const fn kind(&self) -> &FlowEventKind {
        &self.kind
    }
}

/// Route inputs selected before the flow reducer runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteSelection {
    scale: WorkScale,
    design_route: DesignRoute,
}

impl RouteSelection {
    /// Creates a route selection validated later by `TransitionPolicy`.
    pub const fn new(scale: WorkScale, design_route: DesignRoute) -> Self {
        Self {
            scale,
            design_route,
        }
    }

    /// Returns the work scale.
    pub const fn scale(self) -> WorkScale {
        self.scale
    }

    /// Returns the selected design route.
    pub const fn design_route(self) -> DesignRoute {
        self.design_route
    }
}

/// Authoritative process snapshot used as the reducer baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowSnapshot {
    phase: ProcessPhase,
    paused_from: Option<ProcessPhase>,
    state_revision: StateRevision,
    correction_count: u64,
}

impl FlowSnapshot {
    /// Creates a non-paused flow snapshot.
    pub const fn new(
        phase: ProcessPhase,
        state_revision: StateRevision,
        correction_count: u64,
    ) -> Self {
        Self {
            phase,
            paused_from: None,
            state_revision,
            correction_count,
        }
    }

    /// Records the exact phase from which a paused snapshot may resume.
    pub const fn with_paused_from(mut self, paused_from: ProcessPhase) -> Self {
        self.paused_from = Some(paused_from);
        self
    }

    /// Returns the authoritative phase.
    pub const fn phase(self) -> ProcessPhase {
        self.phase
    }

    /// Returns the recorded resume phase for a paused flow.
    pub const fn paused_from(self) -> Option<ProcessPhase> {
        self.paused_from
    }

    /// Returns the authoritative state revision.
    pub const fn state_revision(self) -> StateRevision {
        self.state_revision
    }

    /// Returns the durable business correction count.
    pub const fn correction_count(self) -> u64 {
        self.correction_count
    }

    pub(crate) const fn with_correction_count(mut self, correction_count: u64) -> Self {
        self.correction_count = correction_count;
        self
    }
}

/// Immutable environment included in every deterministic decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowEnvironment {
    event_store_id: EventStoreId,
    policy_digest: PolicyDigest,
    input_fingerprint: InputFingerprint,
    route: RouteSelection,
}

impl FlowEnvironment {
    /// Creates a reducer environment without reading globals or adapters.
    pub fn new(
        event_store_id: EventStoreId,
        input_fingerprint: InputFingerprint,
        route: RouteSelection,
    ) -> Self {
        Self {
            event_store_id,
            policy_digest: TransitionPolicy::digest(),
            input_fingerprint,
            route,
        }
    }

    /// Returns the durable event store epoch.
    pub const fn event_store_id(self) -> EventStoreId {
        self.event_store_id
    }

    /// Returns the policy revision digest.
    pub const fn policy_digest(self) -> PolicyDigest {
        self.policy_digest
    }

    /// Returns the full flow input fingerprint.
    pub const fn input_fingerprint(self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the selected route.
    pub const fn route(self) -> RouteSelection {
        self.route
    }
}

/// Complete explicit input for starting or replaying one flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowInput {
    snapshot: FlowSnapshot,
    environment: FlowEnvironment,
}

impl FlowInput {
    /// Creates a flow input from authoritative state and immutable environment.
    pub const fn new(snapshot: FlowSnapshot, environment: FlowEnvironment) -> Self {
        Self {
            snapshot,
            environment,
        }
    }

    /// Returns the authoritative baseline snapshot.
    pub const fn snapshot(self) -> FlowSnapshot {
        self.snapshot
    }

    /// Returns the immutable reducer environment.
    pub const fn environment(self) -> FlowEnvironment {
        self.environment
    }
}

/// Reason recorded when the supervisor is not healthy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorDegradation {
    GateError,
    GateTimeout,
    Background(SupervisorFault),
}

/// Infrastructure health tracked separately from business correction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorHealth {
    Healthy,
    Degraded(SupervisorDegradation),
}

/// Side-effect-free action for the runtime/application layer to execute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NextAction {
    /// Wait for committed work or Agent input.
    AwaitAgentWork,
    /// Evaluate all entry Gates from a fresh immutable snapshot.
    EvaluateGates {
        target: ProcessPhase,
        required_gates: Vec<RequiredGate>,
    },
    /// Apply the pending transition through the mutation authority.
    ApplyTransition { target: ProcessPhase },
    /// Return business findings without performing a transition.
    ProvideCorrection,
    /// Retry a retryable or timed-out Gate.
    RetryGate,
    /// Stop automatic Gate retries for a terminal infrastructure error.
    HaltForGateError,
    /// Await explicit resolution after Gate cancellation.
    AwaitCancellationResolution,
    /// Rebuild a Gate snapshot and evaluate it again.
    ReevaluateGate,
    /// Reject an illegal or non-root transition intent.
    TransitionDenied {
        target: ProcessPhase,
        reason: TransitionPolicyError,
    },
}

/// Durable pure decision that doubles as the next supervisor checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowDecision {
    pub(crate) environment: FlowEnvironment,
    pub(crate) snapshot: FlowSnapshot,
    pub(crate) pending_transition: Option<ProcessPhase>,
    pub(crate) required_gates: Vec<RequiredGate>,
    pub(crate) passed_gates: BTreeSet<RequiredGate>,
    pub(crate) health: SupervisorHealth,
    pub(crate) next_action: NextAction,
    pub(crate) last_cursor: Option<EventCursor>,
    pub(crate) last_event_fingerprint: Option<InputFingerprint>,
    pub(crate) decision_digest: DecisionDigest,
}

impl FlowDecision {
    /// Returns the current authoritative state projection.
    pub const fn snapshot(&self) -> FlowSnapshot {
        self.snapshot
    }

    /// Returns the transition waiting for Gate/commit evidence.
    pub const fn pending_transition(&self) -> Option<ProcessPhase> {
        self.pending_transition
    }

    /// Returns the complete bounded Gate set for the pending transition.
    pub fn required_gates(&self) -> &[RequiredGate] {
        &self.required_gates
    }

    /// Returns Gate identities that already produced fresh `Pass`.
    pub const fn passed_gates(&self) -> &BTreeSet<RequiredGate> {
        &self.passed_gates
    }

    /// Returns the infrastructure health, independent from correction count.
    pub const fn health(&self) -> SupervisorHealth {
        self.health
    }

    /// Returns the next deterministic runtime action.
    pub const fn next_action(&self) -> &NextAction {
        &self.next_action
    }

    /// Returns the last applied global event cursor.
    pub const fn last_cursor(&self) -> Option<EventCursor> {
        self.last_cursor
    }

    /// Returns the deterministic digest of state, ordered events, and action.
    pub const fn decision_digest(&self) -> DecisionDigest {
        self.decision_digest
    }
}
