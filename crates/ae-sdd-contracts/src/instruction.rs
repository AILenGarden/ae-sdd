//! The verifiable instruction a daemon issues to one child Agent.
//!
//! `ae-sdd-daemon-design.md` §10.1 forbids the daemon from editing an Agent
//! prompt. Instead it returns an [`InstructionEnvelope`] that a Hook or Host
//! adapter maps onto the host's native injection, and §10.1 further requires
//! that a Series not enter its executing state when the host cannot prove the
//! injection landed. That makes the envelope a contract, not a message: it has
//! to carry enough binding for a recipient to verify it is still valid.
//!
//! §10.2's closing line names the four bindings — state revision, policy digest,
//! role and deadline — that stop an old instruction replaying against new state.
//! All four are required fields here rather than options, so an unbindable
//! envelope cannot be constructed at all.

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ContextProjectionId, DecisionDigest, DelegationId, EpochMillis,
    FlowRunId, InputFingerprint, InstructionId, PolicyDigest, SeriesRunId, StateRevision,
    WorkItemId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BoundedText, MAIN_NODE_SERIES_KINDS, SchemaVersion, SeriesId, SeriesKind, SeriesSubNode,
    SkillId, serde_domain,
};

/// Why an [`InstructionEnvelope`] or its parts could not be built.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InstructionError {
    /// `main_node` was well-formed but is not a frozen main node.
    #[error("{value} is not a frozen main node")]
    MainNodeNotFrozen {
        /// The rejected value.
        value: String,
    },
    /// The transaction named no required outputs.
    #[error("an instruction transaction must name at least one required output")]
    NoRequiredOutputs,
    /// The envelope granted no actions.
    #[error("an instruction envelope must grant at least one allowed action")]
    NoAllowedActions,
    /// The envelope cited no skill asset.
    #[error("an instruction envelope must cite at least one skill ref")]
    NoSkillRefs,
}

/// A methodology skill slice, bound to the content version that was compiled.
///
/// The `id` is a [`SkillId`], not a [`SeriesKind`]. §10.2's example shows
/// `story-generate`, which is a legal skill identity and explicitly *not* a legal
/// main node (§11.1) — the same string means different things in the two fields,
/// and conflating them is what let `{kind}-generate` reach `seriesKind`.
///
/// The digest is mandatory because §9.1 requires a Series transaction to name
/// the SKILL assets it adopts *and their digests*: citing a skill by name alone
/// would let the asset change under a running Series.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillRef {
    id: SkillId,
    #[serde(with = "serde_domain::artifact_digest")]
    digest: ArtifactDigest,
}

impl SkillRef {
    /// Binds a skill identity to a content version.
    pub const fn new(id: SkillId, digest: ArtifactDigest) -> Self {
        Self { id, digest }
    }

    /// Returns the skill identity.
    pub const fn id(&self) -> &SkillId {
        &self.id
    }

    /// Returns the digest of the bound version.
    pub const fn digest(&self) -> ArtifactDigest {
        self.digest
    }
}

/// A reference to the context projection this instruction was issued against.
///
/// A reference rather than the projection itself, because §10.3 caps a
/// projection's size and forbids prompts, child transcripts, unbounded logs,
/// credentials and Agent reasoning from entering it. Embedding the projection
/// would put an unbounded payload inside every envelope; carrying `id` plus
/// `digest` keeps the envelope small while still pinning exactly which
/// projection was in force.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextProjectionRef {
    #[serde(with = "serde_domain::context_projection_id")]
    id: ContextProjectionId,
    #[serde(with = "serde_domain::artifact_digest")]
    digest: ArtifactDigest,
}

impl ContextProjectionRef {
    /// Binds a projection identity to a content version.
    pub const fn new(id: ContextProjectionId, digest: ArtifactDigest) -> Self {
        Self { id, digest }
    }

    /// Returns the projection identity.
    pub const fn id(&self) -> ContextProjectionId {
        self.id
    }

    /// Returns the digest of the projected content.
    pub const fn digest(&self) -> ArtifactDigest {
        self.digest
    }
}

/// The transaction an envelope carries, per §10.2's nested `transaction` object.
///
/// Distinct from [`ae_sdd_domain::DeliverableContract`], which binds
/// requirements to a digest and revision in order to *validate* a returned
/// result. This states the assignment going out; that contract judges what comes
/// back. Keeping them separate is what lets §11.4 return "missing items" without
/// the outgoing instruction being rewritten.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    try_from = "InstructionTransactionWire",
    into = "InstructionTransactionWire"
)]
pub struct InstructionTransaction {
    objective: BoundedText<512>,
    required_outputs: Vec<BoundedText<128>>,
    report_schema: BoundedText<128>,
}

impl InstructionTransaction {
    /// Builds a transaction, rejecting one with no required outputs.
    ///
    /// §9.1 requires a Series transaction to name its deliverables, and §11.4
    /// puts a mismatched result into `correction` by returning the missing ones.
    /// With no required outputs nothing can be missing, so that correction path
    /// could never fire and the Series would have no definition of done.
    pub fn new(
        objective: BoundedText<512>,
        required_outputs: Vec<BoundedText<128>>,
        report_schema: BoundedText<128>,
    ) -> Result<Self, InstructionError> {
        if required_outputs.is_empty() {
            return Err(InstructionError::NoRequiredOutputs);
        }
        Ok(Self {
            objective,
            required_outputs,
            report_schema,
        })
    }

    /// Returns the stated objective.
    pub const fn objective(&self) -> &BoundedText<512> {
        &self.objective
    }

    /// Returns the deliverables this transaction requires.
    pub fn required_outputs(&self) -> &[BoundedText<128>] {
        &self.required_outputs
    }

    /// Returns the schema the child must report against.
    pub const fn report_schema(&self) -> &BoundedText<128> {
        &self.report_schema
    }
}

/// Wire form of [`InstructionTransaction`], validated on the way in.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstructionTransactionWire {
    objective: BoundedText<512>,
    required_outputs: Vec<BoundedText<128>>,
    report_schema: BoundedText<128>,
}

impl From<InstructionTransaction> for InstructionTransactionWire {
    fn from(value: InstructionTransaction) -> Self {
        Self {
            objective: value.objective,
            required_outputs: value.required_outputs,
            report_schema: value.report_schema,
        }
    }
}

impl TryFrom<InstructionTransactionWire> for InstructionTransaction {
    type Error = InstructionError;

    fn try_from(value: InstructionTransactionWire) -> Result<Self, Self::Error> {
        Self::new(value.objective, value.required_outputs, value.report_schema)
    }
}

/// The identities an envelope binds, grouped to keep the constructor readable.
///
/// Seventeen positional parameters invite silent transposition between
/// same-typed ids; naming them at the call site makes a swapped `SeriesId` and
/// `SeriesRunId` a compile-time concern rather than a runtime mystery.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstructionIdentity {
    /// This issued instruction.
    #[serde(with = "serde_domain::instruction_id")]
    pub instruction_id: InstructionId,
    /// The business task this instruction serves.
    #[serde(with = "serde_domain::work_item_id")]
    pub work_item_id: WorkItemId,
    /// The main-flow run instance.
    #[serde(with = "serde_domain::flow_run_id")]
    pub flow_run_id: FlowRunId,
    /// The logical Series, stable across retries.
    pub series_id: SeriesId,
    /// The physical execution attempt, new on each retry (§4.1).
    #[serde(with = "serde_domain::series_run_id")]
    pub series_run_id: SeriesRunId,
    /// The delegation edge authorising this child, daemon-minted (§9.3).
    #[serde(with = "serde_domain::delegation_id")]
    pub delegation_id: DelegationId,
}

/// A daemon-issued, verifiable instruction to one child Agent (§10.2).
///
/// `role` is absent from §10.2's JSON example but normative in its prose: the
/// section's closing line requires the envelope bind a role. §10.3 then projects
/// differently for root, series, task and reviewer, so a recipient cannot
/// validate its own projection without knowing which role it was issued as.
///
/// Deserialization routes through [`Self::issue`] so the wire cannot produce an
/// envelope the constructor would refuse.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "InstructionEnvelopeWire", into = "InstructionEnvelopeWire")]
pub struct InstructionEnvelope {
    schema_version: SchemaVersion,
    identity: InstructionIdentity,
    state_revision: StateRevision,
    /// The route decision this instruction executes under.
    ///
    /// §9.1 line 454 requires the envelope carry "前置状态 revision 与 route decision
    /// digest". `state_revision` supplied the first; without the second an envelope
    /// named no authority for the work it authorised, so an instruction issued
    /// against a superseded route was indistinguishable from a current one.
    decision_digest: DecisionDigest,
    /// The input fingerprint the decision was computed from.
    ///
    /// Carried alongside the digest, not in place of it: the digest identifies
    /// *which decision*, the fingerprint identifies *what it was decided from*.
    /// §5.5's re-route rule turns on the inputs changing, so an envelope that
    /// cited only the digest could not show its inputs still hold.
    input_fingerprint: InputFingerprint,
    main_node: SeriesKind,
    sub_node: SeriesSubNode,
    role: AgentRole,
    transaction: InstructionTransaction,
    skill_refs: Vec<SkillRef>,
    context_projection_ref: ContextProjectionRef,
    allowed_actions: Vec<BoundedText<128>>,
    expires_at: EpochMillis,
    policy_digest: PolicyDigest,
}

impl InstructionEnvelope {
    /// Issues an envelope, enforcing the three invariants the wire must also hold.
    ///
    /// `main_node` is checked against [`MAIN_NODE_SERIES_KINDS`] rather than
    /// accepting any well-formed [`SeriesKind`], because §11.1 makes
    /// `currentMainNode` and this field draw from one list. Accepting
    /// `story-generate` here would reproduce, in the envelope, exactly the
    /// divergence that left six of eight main nodes unresolvable in the
    /// methodology catalog.
    ///
    /// An empty `allowed_actions` is refused because §11.4 answers an
    /// unauthorized request by refusing it without advancing the node: an
    /// envelope granting nothing could only ever produce refusals, so a Series
    /// holding one would be structurally unable to progress.
    ///
    /// An empty `skill_refs` is refused because §9.1 requires the transaction to
    /// name the SKILL assets it adopts. A Series with no method asset has no
    /// basis for its output, and §2.2 makes the Agent the semantic executor of a
    /// *given* method rather than the author of one.
    ///
    /// The identities are already grouped into [`InstructionIdentity`]; the
    /// remainder are independent bindings that share no natural grouping, and
    /// bundling them purely to satisfy an argument count would hide which
    /// bindings §10.2 actually requires.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        identity: InstructionIdentity,
        state_revision: StateRevision,
        decision_digest: DecisionDigest,
        input_fingerprint: InputFingerprint,
        main_node: SeriesKind,
        sub_node: SeriesSubNode,
        role: AgentRole,
        transaction: InstructionTransaction,
        skill_refs: Vec<SkillRef>,
        context_projection_ref: ContextProjectionRef,
        allowed_actions: Vec<BoundedText<128>>,
        expires_at: EpochMillis,
        policy_digest: PolicyDigest,
    ) -> Result<Self, InstructionError> {
        if !MAIN_NODE_SERIES_KINDS.contains(&main_node.as_str()) {
            return Err(InstructionError::MainNodeNotFrozen {
                value: main_node.to_string(),
            });
        }
        if allowed_actions.is_empty() {
            return Err(InstructionError::NoAllowedActions);
        }
        if skill_refs.is_empty() {
            return Err(InstructionError::NoSkillRefs);
        }
        Ok(Self {
            schema_version: SchemaVersion::V1,
            identity,
            state_revision,
            decision_digest,
            input_fingerprint,
            main_node,
            sub_node,
            role,
            transaction,
            skill_refs,
            context_projection_ref,
            allowed_actions,
            expires_at,
            policy_digest,
        })
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the route decision digest this envelope executes under.
    pub const fn decision_digest(&self) -> DecisionDigest {
        self.decision_digest
    }

    /// Returns the input fingerprint the route decision was computed from.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the bound identities.
    pub const fn identity(&self) -> &InstructionIdentity {
        &self.identity
    }

    /// Returns the state revision this instruction was issued against.
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns the main node, always a frozen [`MAIN_NODE_SERIES_KINDS`] value.
    pub const fn main_node(&self) -> &SeriesKind {
        &self.main_node
    }

    /// Returns the position inside the Series.
    pub const fn sub_node(&self) -> SeriesSubNode {
        self.sub_node
    }

    /// Returns the role this envelope was issued to (§10.3).
    pub const fn role(&self) -> AgentRole {
        self.role
    }

    /// Returns the transaction.
    pub const fn transaction(&self) -> &InstructionTransaction {
        &self.transaction
    }

    /// Returns the cited skill assets and their digests.
    pub fn skill_refs(&self) -> &[SkillRef] {
        &self.skill_refs
    }

    /// Returns the context projection reference.
    pub const fn context_projection_ref(&self) -> &ContextProjectionRef {
        &self.context_projection_ref
    }

    /// Returns the granted actions.
    pub fn allowed_actions(&self) -> &[BoundedText<128>] {
        &self.allowed_actions
    }

    /// Returns the deadline.
    pub const fn expires_at(&self) -> EpochMillis {
        self.expires_at
    }

    /// Returns the policy digest this instruction is bound to.
    pub const fn policy_digest(&self) -> PolicyDigest {
        self.policy_digest
    }

    /// Whether this envelope has expired at `now`.
    ///
    /// Inclusive of the deadline instant, matching [`EpochMillis::is_expired_at`]:
    /// §10.2 uses the deadline to stop stale replay, so the boundary fails closed.
    pub const fn is_expired_at(&self, now: EpochMillis) -> bool {
        self.expires_at.is_expired_at(now)
    }
}

/// Wire form of [`InstructionEnvelope`], validated on the way in.
///
/// `schemaVersion` is accepted from the wire and then re-asserted by
/// [`InstructionEnvelope::issue`], which always stamps [`SchemaVersion::V1`].
/// A future version therefore cannot be silently carried through this type as
/// data; it has to be handled explicitly when it is introduced.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstructionEnvelopeWire {
    schema_version: SchemaVersion,
    identity: InstructionIdentity,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::decision_digest")]
    decision_digest: DecisionDigest,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    main_node: SeriesKind,
    sub_node: SeriesSubNode,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    transaction: InstructionTransaction,
    skill_refs: Vec<SkillRef>,
    context_projection_ref: ContextProjectionRef,
    allowed_actions: Vec<BoundedText<128>>,
    #[serde(with = "serde_domain::epoch_millis")]
    expires_at: EpochMillis,
    #[serde(with = "serde_domain::policy_digest")]
    policy_digest: PolicyDigest,
}

impl From<InstructionEnvelope> for InstructionEnvelopeWire {
    fn from(value: InstructionEnvelope) -> Self {
        Self {
            schema_version: value.schema_version,
            identity: value.identity,
            state_revision: value.state_revision,
            decision_digest: value.decision_digest,
            input_fingerprint: value.input_fingerprint,
            main_node: value.main_node,
            sub_node: value.sub_node,
            role: value.role,
            transaction: value.transaction,
            skill_refs: value.skill_refs,
            context_projection_ref: value.context_projection_ref,
            allowed_actions: value.allowed_actions,
            expires_at: value.expires_at,
            policy_digest: value.policy_digest,
        }
    }
}

impl TryFrom<InstructionEnvelopeWire> for InstructionEnvelope {
    type Error = InstructionError;

    fn try_from(value: InstructionEnvelopeWire) -> Result<Self, Self::Error> {
        Self::issue(
            value.identity,
            value.state_revision,
            value.decision_digest,
            value.input_fingerprint,
            value.main_node,
            value.sub_node,
            value.role,
            value.transaction,
            value.skill_refs,
            value.context_projection_ref,
            value.allowed_actions,
            value.expires_at,
            value.policy_digest,
        )
    }
}
