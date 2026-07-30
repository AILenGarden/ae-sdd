//! Frozen Work Item lifecycle planning contracts.

use std::collections::BTreeSet;

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, CompletionDigestSet, CompletionMilestone, DecisionDigest,
    DesignRoute, EvidenceRef, InputFingerprint, ProcessPhase, ProjectRelativePath, SessionId,
    StateRevision, StoryId, VerificationId, WorkScale,
};
use ae_sdd_protocol::ConfirmationRef;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    LogicalKey, LogicalNamespace, MutationIntentId, PrdId, ProcessSnapshot, ReasonCode,
    Remediation, SchemaVersion, serde_domain,
};

/// Maximum number of Story summaries in a lifecycle input.
pub const MAX_LIFECYCLE_STORIES: usize = 128;
/// Maximum number of confirmations or evidence references in one lifecycle input.
pub const MAX_LIFECYCLE_REFS: usize = 64;
/// Maximum number of file-lock snapshots in one lifecycle input.
pub const MAX_FILE_LOCKS: usize = 256;

/// Typed lifecycle command; no free-form mutation payload is accepted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LifecycleCommand {
    /// Request a policy-authorized phase transition.
    Transition {
        /// Direct target phase.
        #[serde(with = "serde_domain::process_phase")]
        target_phase: ProcessPhase,
    },
    /// Pause the Work Item while preserving the exact source phase.
    Pause,
    /// Resume only to the phase recorded by the authoritative snapshot.
    Resume,
    /// Bind a registered Story document to the Work Item.
    BindStory {
        /// Registered Story identity.
        #[serde(with = "serde_domain::story_id")]
        story_id: StoryId,
        /// Canonical project-relative Story document path.
        #[serde(with = "serde_domain::project_relative_path")]
        document_path: ProjectRelativePath,
    },
    /// Complete one registered Story after its invariants pass.
    CompleteStory {
        /// Story identity.
        #[serde(with = "serde_domain::story_id")]
        story_id: StoryId,
    },
    /// Complete a PRD after the four-layer AND contract passes.
    CompletePrd {
        /// PRD identity.
        prd_id: PrdId,
    },
    /// Acquire a project-relative file lock at an explicit evaluation instant.
    AcquireFileLock {
        /// Locked project-relative path.
        #[serde(with = "serde_domain::project_relative_path")]
        path: ProjectRelativePath,
        /// Authenticated lock owner.
        #[serde(with = "serde_domain::session_id")]
        owner_session_id: SessionId,
        /// Explicit expiry instant supplied by the trusted adapter.
        expires_at_unix_ms: u64,
    },
    /// Release a file lock owned by the authenticated session.
    ReleaseFileLock {
        /// Locked project-relative path.
        #[serde(with = "serde_domain::project_relative_path")]
        path: ProjectRelativePath,
        /// Authenticated lock owner.
        #[serde(with = "serde_domain::session_id")]
        owner_session_id: SessionId,
    },
    /// Archive a terminal Work Item projection.
    ArchiveWorkItem,
}

/// Bounded Story completion summary; never a transcript or full document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorySummary {
    /// Story identity.
    #[serde(with = "serde_domain::story_id")]
    pub story_id: StoryId,
    /// Current Story phase.
    #[serde(with = "serde_domain::process_phase")]
    pub phase: ProcessPhase,
    /// Stable current step code.
    pub current_step: ReasonCode,
    /// Number of pending required outputs.
    pub pending_outputs: u16,
    /// Coding round observed in authoritative state.
    pub coding_round: u32,
    /// Whether the Story is registered under this Work Item.
    pub registered: bool,
}

impl StorySummary {
    /// Returns whether the summary satisfies the terminal Story invariant.
    pub const fn is_complete(&self) -> bool {
        self.registered
            && matches!(self.phase, ProcessPhase::Completed)
            && self.pending_outputs == 0
            && self.coding_round >= 1
    }
}

/// Bounded PRD completion projection used by the pure lifecycle planner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrdSummary {
    /// PRD identity.
    pub prd_id: PrdId,
    /// Registered Story identities.
    #[serde(with = "story_ids")]
    pub registered_story_ids: Vec<StoryId>,
    /// Story identities whose authoritative child state is terminal.
    #[serde(with = "story_ids")]
    pub completed_story_ids: Vec<StoryId>,
    /// Cross-Story dependency contract passed.
    pub dependencies_satisfied: bool,
    /// Residual-risk registry is empty or explicitly resolved.
    pub residual_risks_cleared: bool,
    /// PRD-level Gates are fresh PASS.
    pub gates_passed: bool,
    /// Independent PRD review is valid PASS.
    pub review_passed: bool,
}

impl PrdSummary {
    /// Returns whether the four-layer PRD completion AND is satisfied.
    pub fn is_complete(&self) -> bool {
        !self.registered_story_ids.is_empty()
            && self.registered_story_ids.len() == self.completed_story_ids.len()
            && self
                .registered_story_ids
                .iter()
                .all(|story| self.completed_story_ids.contains(story))
            && self.dependencies_satisfied
            && self.residual_risks_cleared
            && self.gates_passed
            && self.review_passed
    }
}

/// Authoritative file-lock snapshot evaluated without reading the wall clock.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileLockSnapshot {
    /// Canonical project-relative locked path.
    #[serde(with = "serde_domain::project_relative_path")]
    pub path: ProjectRelativePath,
    /// Authenticated owner session.
    #[serde(with = "serde_domain::session_id")]
    pub owner_session_id: SessionId,
    /// Explicit expiry instant.
    pub expires_at_unix_ms: u64,
    /// False when legacy metadata could not be parsed; planners fail closed.
    pub metadata_valid: bool,
}

/// Bounded completion-milestone freshness projection asserted by the trusted adapter.
///
/// `bound` records the input digests observed when the authoritative state
/// reached `milestone`; `observed` carries the digests the trusted adapter
/// sees right now. The effective milestone is the recorded one rolled back to
/// the earliest still-fresh point, so a stale input can never keep
/// `GovernanceClosed`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    from = "CompletionMilestoneInputWire",
    into = "CompletionMilestoneInputWire"
)]
pub struct CompletionMilestoneInput {
    milestone: CompletionMilestone,
    bound: CompletionDigestSet,
    observed: CompletionDigestSet,
}

impl CompletionMilestoneInput {
    /// Constructs a milestone freshness projection.
    pub const fn new(
        milestone: CompletionMilestone,
        bound: CompletionDigestSet,
        observed: CompletionDigestSet,
    ) -> Self {
        Self {
            milestone,
            bound,
            observed,
        }
    }

    /// Returns the milestone recorded in authoritative state.
    pub const fn milestone(&self) -> CompletionMilestone {
        self.milestone
    }

    /// Returns the digests bound when the milestone was reached.
    pub const fn bound(&self) -> CompletionDigestSet {
        self.bound
    }

    /// Returns the currently observed input digests.
    pub const fn observed(&self) -> CompletionDigestSet {
        self.observed
    }

    /// Returns the milestone after rolling back to the earliest still-fresh point.
    pub fn effective_milestone(&self) -> CompletionMilestone {
        self.milestone.invalidate(&self.bound, &self.observed)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompletionMilestoneInputWire {
    #[serde(with = "completion_milestone")]
    milestone: CompletionMilestone,
    bound: CompletionDigestSetWire,
    observed: CompletionDigestSetWire,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompletionDigestSetWire {
    #[serde(with = "serde_domain::artifact_digest")]
    code_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    verification_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    evidence_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    review_input_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    gate_digest: ArtifactDigest,
}

impl From<CompletionMilestoneInputWire> for CompletionMilestoneInput {
    fn from(value: CompletionMilestoneInputWire) -> Self {
        Self::new(value.milestone, value.bound.into(), value.observed.into())
    }
}

impl From<CompletionMilestoneInput> for CompletionMilestoneInputWire {
    fn from(value: CompletionMilestoneInput) -> Self {
        Self {
            milestone: value.milestone(),
            bound: value.bound().into(),
            observed: value.observed().into(),
        }
    }
}

impl From<CompletionDigestSetWire> for CompletionDigestSet {
    fn from(value: CompletionDigestSetWire) -> Self {
        Self::new(
            value.code_digest,
            value.verification_digest,
            value.evidence_digest,
            value.review_input_digest,
            value.gate_digest,
        )
    }
}

impl From<CompletionDigestSet> for CompletionDigestSetWire {
    fn from(value: CompletionDigestSet) -> Self {
        Self {
            code_digest: value.code_digest(),
            verification_digest: value.verification_digest(),
            evidence_digest: value.evidence_digest(),
            review_input_digest: value.review_input_digest(),
            gate_digest: value.gate_digest(),
        }
    }
}

/// Error returned when a lifecycle input violates bounds or snapshot identity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum LifecycleInputError {
    /// Expected revision did not match the authoritative snapshot revision.
    #[error("lifecycle expected revision does not match the process snapshot")]
    RevisionMismatch,
    /// A bounded input collection exceeded its frozen v1 limit.
    #[error("lifecycle input exceeds a frozen v1 collection limit")]
    CollectionLimitExceeded,
    /// The same Story identity appeared more than once.
    #[error("lifecycle input contains a duplicate Story summary")]
    DuplicateStory,
    /// The same daemon Gate pass appeared more than once.
    #[error("lifecycle input contains a duplicate Gate pass")]
    DuplicateGatePass,
}

/// Complete, bounded input to the pure lifecycle planner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "LifecycleInputWire", into = "LifecycleInputWire")]
pub struct LifecycleInput {
    schema_version: SchemaVersion,
    command: LifecycleCommand,
    snapshot: ProcessSnapshot,
    expected_revision: StateRevision,
    actor_role: AgentRole,
    scale: WorkScale,
    design_route: DesignRoute,
    story_summaries: Vec<StorySummary>,
    prd_summary: Option<PrdSummary>,
    confirmation_refs: Vec<ConfirmationRef>,
    evidence_refs: Vec<EvidenceRef>,
    passed_gate_ids: Vec<VerificationId>,
    file_locks: Vec<FileLockSnapshot>,
    completion: Option<CompletionMilestoneInput>,
    evaluation_unix_ms: u64,
    input_fingerprint: InputFingerprint,
}

impl LifecycleInput {
    /// Constructs and validates bounded lifecycle input.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        command: LifecycleCommand,
        snapshot: ProcessSnapshot,
        expected_revision: StateRevision,
        actor_role: AgentRole,
        scale: WorkScale,
        design_route: DesignRoute,
        story_summaries: Vec<StorySummary>,
        prd_summary: Option<PrdSummary>,
        confirmation_refs: Vec<ConfirmationRef>,
        evidence_refs: Vec<EvidenceRef>,
        file_locks: Vec<FileLockSnapshot>,
        evaluation_unix_ms: u64,
        input_fingerprint: InputFingerprint,
    ) -> Result<Self, LifecycleInputError> {
        if expected_revision != snapshot.state_revision {
            return Err(LifecycleInputError::RevisionMismatch);
        }
        if story_summaries.len() > MAX_LIFECYCLE_STORIES
            || confirmation_refs.len() > MAX_LIFECYCLE_REFS
            || evidence_refs.len() > MAX_LIFECYCLE_REFS
            || file_locks.len() > MAX_FILE_LOCKS
        {
            return Err(LifecycleInputError::CollectionLimitExceeded);
        }
        let unique: BTreeSet<&StoryId> = story_summaries
            .iter()
            .map(|summary| &summary.story_id)
            .collect();
        if unique.len() != story_summaries.len() {
            return Err(LifecycleInputError::DuplicateStory);
        }
        Ok(Self {
            schema_version,
            command,
            snapshot,
            expected_revision,
            actor_role,
            scale,
            design_route,
            story_summaries,
            prd_summary,
            confirmation_refs,
            evidence_refs,
            passed_gate_ids: Vec::new(),
            file_locks,
            completion: None,
            evaluation_unix_ms,
            input_fingerprint,
        })
    }

    /// Binds the completion-milestone freshness projection asserted by the
    /// trusted adapter; terminal `Completed` planning fails closed without it.
    #[must_use]
    pub const fn with_completion(mut self, completion: CompletionMilestoneInput) -> Self {
        self.completion = Some(completion);
        self
    }

    /// Binds fresh daemon-owned Gate passes to the lifecycle plan without
    /// misrepresenting them as file-backed verification evidence.
    pub fn with_passed_gate_ids(
        mut self,
        mut passed_gate_ids: Vec<VerificationId>,
    ) -> Result<Self, LifecycleInputError> {
        if passed_gate_ids.len() > MAX_LIFECYCLE_REFS {
            return Err(LifecycleInputError::CollectionLimitExceeded);
        }
        let unique: BTreeSet<&VerificationId> = passed_gate_ids.iter().collect();
        if unique.len() != passed_gate_ids.len() {
            return Err(LifecycleInputError::DuplicateGatePass);
        }
        passed_gate_ids.sort();
        self.passed_gate_ids = passed_gate_ids;
        Ok(self)
    }

    /// Returns the optional completion-milestone freshness projection.
    pub const fn completion(&self) -> Option<CompletionMilestoneInput> {
        self.completion
    }

    /// Returns the typed command.
    pub const fn command(&self) -> &LifecycleCommand {
        &self.command
    }

    /// Returns the authoritative process snapshot.
    pub const fn snapshot(&self) -> &ProcessSnapshot {
        &self.snapshot
    }

    /// Returns the authenticated actor role.
    pub const fn actor_role(&self) -> AgentRole {
        self.actor_role
    }

    /// Returns the domain-owned work scale used by transition policy.
    pub const fn scale(&self) -> WorkScale {
        self.scale
    }

    /// Returns the selected direct design route used by transition policy.
    pub const fn design_route(&self) -> DesignRoute {
        self.design_route
    }

    /// Returns bounded Story summaries.
    pub fn story_summaries(&self) -> &[StorySummary] {
        &self.story_summaries
    }

    /// Returns the optional PRD projection.
    pub const fn prd_summary(&self) -> Option<&PrdSummary> {
        self.prd_summary.as_ref()
    }

    /// Returns bounded confirmation references asserted by the trusted adapter.
    pub fn confirmation_refs(&self) -> &[ConfirmationRef] {
        &self.confirmation_refs
    }

    /// Returns bounded evidence references.
    pub fn evidence_refs(&self) -> &[EvidenceRef] {
        &self.evidence_refs
    }

    /// Returns fresh Gate identities asserted by the daemon Flow authority.
    pub fn passed_gate_ids(&self) -> &[VerificationId] {
        &self.passed_gate_ids
    }

    /// Returns the authoritative bounded file-lock projection.
    pub fn file_locks(&self) -> &[FileLockSnapshot] {
        &self.file_locks
    }

    /// Returns the explicit evaluation instant.
    pub const fn evaluation_unix_ms(&self) -> u64 {
        self.evaluation_unix_ms
    }

    /// Returns the input fingerprint.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }
}

impl<'de> Deserialize<'de> for LifecycleInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        LifecycleInputWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LifecycleInputWire {
    schema_version: SchemaVersion,
    command: LifecycleCommand,
    snapshot: ProcessSnapshot,
    #[serde(with = "serde_domain::state_revision")]
    expected_revision: StateRevision,
    #[serde(with = "serde_domain::agent_role")]
    actor_role: AgentRole,
    #[serde(with = "serde_domain::work_scale")]
    scale: WorkScale,
    #[serde(with = "serde_domain::design_route")]
    design_route: DesignRoute,
    story_summaries: Vec<StorySummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prd_summary: Option<PrdSummary>,
    confirmation_refs: Vec<ConfirmationRef>,
    #[serde(with = "serde_domain::evidence_refs")]
    evidence_refs: Vec<EvidenceRef>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        with = "serde_domain::verification_ids"
    )]
    passed_gate_ids: Vec<VerificationId>,
    file_locks: Vec<FileLockSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completion: Option<CompletionMilestoneInput>,
    evaluation_unix_ms: u64,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
}

impl TryFrom<LifecycleInputWire> for LifecycleInput {
    type Error = LifecycleInputError;

    fn try_from(value: LifecycleInputWire) -> Result<Self, Self::Error> {
        let input = Self::new(
            value.schema_version,
            value.command,
            value.snapshot,
            value.expected_revision,
            value.actor_role,
            value.scale,
            value.design_route,
            value.story_summaries,
            value.prd_summary,
            value.confirmation_refs,
            value.evidence_refs,
            value.file_locks,
            value.evaluation_unix_ms,
            value.input_fingerprint,
        )?;
        let input = input.with_passed_gate_ids(value.passed_gate_ids)?;
        Ok(match value.completion {
            Some(completion) => input.with_completion(completion),
            None => input,
        })
    }
}

impl From<LifecycleInput> for LifecycleInputWire {
    fn from(value: LifecycleInput) -> Self {
        Self {
            schema_version: value.schema_version,
            command: value.command,
            snapshot: value.snapshot,
            expected_revision: value.expected_revision,
            actor_role: value.actor_role,
            scale: value.scale,
            design_route: value.design_route,
            story_summaries: value.story_summaries,
            prd_summary: value.prd_summary,
            confirmation_refs: value.confirmation_refs,
            evidence_refs: value.evidence_refs,
            passed_gate_ids: value.passed_gate_ids,
            file_locks: value.file_locks,
            completion: value.completion,
            evaluation_unix_ms: value.evaluation_unix_ms,
            input_fingerprint: value.input_fingerprint,
        }
    }
}

/// Lifecycle planning disposition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleDisposition {
    /// The command may be applied transactionally by C1.
    Permitted,
    /// The command violates lifecycle policy or current state.
    Denied,
    /// The command is otherwise valid but awaits user confirmation.
    AwaitingConfirmation,
}

/// Confirmation binding carried by a lifecycle plan.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmationRequirement {
    /// Whether explicit confirmation is required before apply.
    pub required: bool,
    /// Stable reason when confirmation is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<ReasonCode>,
    /// Digest that a confirmation must bind.
    #[serde(with = "serde_domain::decision_digest")]
    pub binding_digest: DecisionDigest,
}

impl ConfirmationRequirement {
    /// Constructs a requirement that needs no additional confirmation.
    pub const fn not_required(binding_digest: DecisionDigest) -> Self {
        Self {
            required: false,
            reason_code: None,
            binding_digest,
        }
    }

    /// Constructs an explicit confirmation requirement.
    pub const fn required(reason_code: ReasonCode, binding_digest: DecisionDigest) -> Self {
        Self {
            required: true,
            reason_code: Some(reason_code),
            binding_digest,
        }
    }
}

/// Logical or project-relative mutation target; never an absolute host path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationTarget {
    /// Logical state namespace.
    pub namespace: LogicalNamespace,
    /// Optional project-relative file target.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_project_relative_path"
    )]
    pub relative_path: Option<ProjectRelativePath>,
    /// Optional logical-record key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_key: Option<LogicalKey>,
}

impl MutationTarget {
    /// Constructs a project-file mutation target.
    pub const fn project_file(
        namespace: LogicalNamespace,
        relative_path: ProjectRelativePath,
    ) -> Self {
        Self {
            namespace,
            relative_path: Some(relative_path),
            logical_key: None,
        }
    }

    /// Constructs a logical-record mutation target.
    pub const fn logical_record(namespace: LogicalNamespace, logical_key: LogicalKey) -> Self {
        Self {
            namespace,
            relative_path: None,
            logical_key: Some(logical_key),
        }
    }
}

/// Typed mutation operation planned by the lifecycle engine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationOperation {
    /// Create a missing target.
    Create,
    /// Replace an existing target under CAS.
    Replace,
    /// Delete an existing target under CAS.
    Delete,
    /// Append a typed event to an authoritative journal.
    AppendEvent,
}

/// Typed event descriptor with a digest-bound payload held elsewhere.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventIntent {
    /// Stable event kind.
    pub kind: ReasonCode,
    /// Digest of the canonical typed event payload.
    #[serde(with = "serde_domain::artifact_digest")]
    pub payload_digest: ArtifactDigest,
}

impl EventIntent {
    /// Constructs a typed event descriptor.
    pub const fn new(kind: ReasonCode, payload_digest: ArtifactDigest) -> Self {
        Self {
            kind,
            payload_digest,
        }
    }
}

/// One ordered, side-effect-free mutation intent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationIntent {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Stable intent identity.
    pub intent_id: MutationIntentId,
    /// Logical or project-relative target.
    pub target: MutationTarget,
    /// Typed operation.
    pub operation: MutationOperation,
    /// Revision that must still be current when C1 applies the intent.
    #[serde(with = "serde_domain::state_revision")]
    pub expected_revision: StateRevision,
    /// Optional before-image digest for CAS.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_artifact_digest"
    )]
    pub expected_digest: Option<ArtifactDigest>,
    /// Typed journal event emitted with the mutation.
    pub event: EventIntent,
}

impl MutationIntent {
    /// Constructs a mutation intent.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        schema_version: SchemaVersion,
        intent_id: MutationIntentId,
        target: MutationTarget,
        operation: MutationOperation,
        expected_revision: StateRevision,
        expected_digest: Option<ArtifactDigest>,
        event: EventIntent,
    ) -> Self {
        Self {
            schema_version,
            intent_id,
            target,
            operation,
            expected_revision,
            expected_digest,
            event,
        }
    }
}

/// Error returned when lifecycle plan fields contradict its disposition.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum LifecyclePlanError {
    /// A permitted plan had no mutation intent.
    #[error("permitted lifecycle plan must contain at least one mutation intent")]
    MissingIntent,
    /// A non-permitted plan attempted to carry mutation intents.
    #[error("denied or awaiting-confirmation lifecycle plan cannot carry mutation intents")]
    UnexpectedIntent,
    /// Confirmation state contradicted the plan disposition.
    #[error("lifecycle confirmation requirement contradicts the plan disposition")]
    ConfirmationMismatch,
    /// The intent list exceeded its frozen v1 limit.
    #[error("lifecycle plan exceeds the frozen v1 intent limit")]
    IntentLimitExceeded,
}

/// Maximum number of mutation intents in one lifecycle plan.
pub const MAX_MUTATION_INTENTS: usize = 64;

/// Pure lifecycle planner output consumed later by the C1 mutation authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "LifecyclePlanWire", into = "LifecyclePlanWire")]
pub struct LifecyclePlan {
    schema_version: SchemaVersion,
    disposition: LifecycleDisposition,
    intents: Vec<MutationIntent>,
    expected_revision: StateRevision,
    confirmation_requirement: ConfirmationRequirement,
    plan_digest: DecisionDigest,
    remediation: Vec<Remediation>,
}

impl LifecyclePlan {
    /// Constructs and validates a lifecycle plan.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        disposition: LifecycleDisposition,
        intents: Vec<MutationIntent>,
        expected_revision: StateRevision,
        confirmation_requirement: ConfirmationRequirement,
        plan_digest: DecisionDigest,
        remediation: Vec<Remediation>,
    ) -> Result<Self, LifecyclePlanError> {
        if intents.len() > MAX_MUTATION_INTENTS {
            return Err(LifecyclePlanError::IntentLimitExceeded);
        }
        match disposition {
            LifecycleDisposition::Permitted if intents.is_empty() => {
                return Err(LifecyclePlanError::MissingIntent);
            }
            LifecycleDisposition::Denied | LifecycleDisposition::AwaitingConfirmation
                if !intents.is_empty() =>
            {
                return Err(LifecyclePlanError::UnexpectedIntent);
            }
            _ => {}
        }
        let confirmation_matches = match disposition {
            LifecycleDisposition::AwaitingConfirmation => confirmation_requirement.required,
            LifecycleDisposition::Permitted | LifecycleDisposition::Denied => {
                !confirmation_requirement.required
            }
        };
        if !confirmation_matches {
            return Err(LifecyclePlanError::ConfirmationMismatch);
        }
        Ok(Self {
            schema_version,
            disposition,
            intents,
            expected_revision,
            confirmation_requirement,
            plan_digest,
            remediation,
        })
    }

    /// Returns the plan disposition.
    pub const fn disposition(&self) -> LifecycleDisposition {
        self.disposition
    }

    /// Returns ordered mutation intents.
    pub fn intents(&self) -> &[MutationIntent] {
        &self.intents
    }

    /// Returns the canonical plan digest.
    pub const fn plan_digest(&self) -> DecisionDigest {
        self.plan_digest
    }
}

impl<'de> Deserialize<'de> for LifecyclePlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        LifecyclePlanWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LifecyclePlanWire {
    schema_version: SchemaVersion,
    disposition: LifecycleDisposition,
    intents: Vec<MutationIntent>,
    #[serde(with = "serde_domain::state_revision")]
    expected_revision: StateRevision,
    confirmation_requirement: ConfirmationRequirement,
    #[serde(with = "serde_domain::decision_digest")]
    plan_digest: DecisionDigest,
    remediation: Vec<Remediation>,
}

impl TryFrom<LifecyclePlanWire> for LifecyclePlan {
    type Error = LifecyclePlanError;

    fn try_from(value: LifecyclePlanWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.disposition,
            value.intents,
            value.expected_revision,
            value.confirmation_requirement,
            value.plan_digest,
            value.remediation,
        )
    }
}

impl From<LifecyclePlan> for LifecyclePlanWire {
    fn from(value: LifecyclePlan) -> Self {
        Self {
            schema_version: value.schema_version,
            disposition: value.disposition,
            intents: value.intents,
            expected_revision: value.expected_revision,
            confirmation_requirement: value.confirmation_requirement,
            plan_digest: value.plan_digest,
            remediation: value.remediation,
        }
    }
}

mod completion_milestone {
    use ae_sdd_domain::CompletionMilestone;
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub(super) fn serialize<S>(
        value: &CompletionMilestone,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match value {
            CompletionMilestone::None => "none",
            CompletionMilestone::ImplementationVerified => "implementation_verified",
            CompletionMilestone::ReviewReady => "review_ready",
            CompletionMilestone::GovernanceClosed => "governance_closed",
        })
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<CompletionMilestone, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "none" => Ok(CompletionMilestone::None),
            "implementation_verified" => Ok(CompletionMilestone::ImplementationVerified),
            "review_ready" => Ok(CompletionMilestone::ReviewReady),
            "governance_closed" => Ok(CompletionMilestone::GovernanceClosed),
            other => Err(de::Error::unknown_variant(
                other,
                &[
                    "none",
                    "implementation_verified",
                    "review_ready",
                    "governance_closed",
                ],
            )),
        }
    }
}

mod optional_project_relative_path {
    use ae_sdd_domain::ProjectRelativePath;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ProjectRelativePath>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .as_ref()
            .map(ToString::to_string)
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<ProjectRelativePath>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ProjectRelativePath::new(value).map_err(de::Error::custom))
            .transpose()
    }
}

mod optional_artifact_digest {
    use std::str::FromStr;

    use ae_sdd_domain::ArtifactDigest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ArtifactDigest>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|digest| digest.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ArtifactDigest>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ArtifactDigest::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

mod story_ids {
    use ae_sdd_domain::StoryId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &[StoryId], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<StoryId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| StoryId::new(value).map_err(de::Error::custom))
            .collect()
    }
}
