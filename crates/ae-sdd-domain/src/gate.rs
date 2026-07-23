use std::collections::BTreeSet;

use thiserror::Error;

use crate::{
    CancellationCode, ConfigDigest, ErrorCode, EvidenceRef, FencingToken, FindingCode, GateId,
    GateImplementationDigest, InputFingerprint, InventoryGeneration, PolicyDigest, StateRevision,
    StoryId, ToolchainDigest, WorkItemId, WorkspaceId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateFinding {
    code: FindingCode,
    evidence: Vec<EvidenceRef>,
}

impl GateFinding {
    pub fn new(code: FindingCode, evidence: impl IntoIterator<Item = EvidenceRef>) -> Self {
        Self {
            code,
            evidence: evidence.into_iter().collect(),
        }
    }

    pub const fn code(&self) -> &FindingCode {
        &self.code
    }

    pub fn evidence(&self) -> &[EvidenceRef] {
        &self.evidence
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateFailure {
    findings: Vec<GateFinding>,
}

impl GateFailure {
    pub fn new(findings: impl IntoIterator<Item = GateFinding>) -> Result<Self, GateOutcomeError> {
        let findings: Vec<_> = findings.into_iter().collect();
        if findings.is_empty() {
            return Err(GateOutcomeError::EmptyFindings);
        }
        Ok(Self { findings })
    }

    pub fn findings(&self) -> &[GateFinding] {
        &self.findings
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateError {
    code: ErrorCode,
    retryable: bool,
}

impl GateError {
    pub const fn new(code: ErrorCode, retryable: bool) -> Self {
        Self { code, retryable }
    }

    pub const fn code(&self) -> &ErrorCode {
        &self.code
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateTimeout {
    deadline_ms: u64,
}

impl GateTimeout {
    pub fn new(deadline_ms: u64) -> Result<Self, GateOutcomeError> {
        if deadline_ms == 0 {
            return Err(GateOutcomeError::ZeroDeadline);
        }
        Ok(Self { deadline_ms })
    }

    pub const fn deadline_ms(self) -> u64 {
        self.deadline_ms
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateCancellation {
    reason: CancellationCode,
}

impl GateCancellation {
    pub const fn new(reason: CancellationCode) -> Self {
        Self { reason }
    }

    pub const fn reason(&self) -> &CancellationCode {
        &self.reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FreshnessDimension {
    GateId,
    GateImplementation,
    Policy,
    Workspace,
    WorkItem,
    Story,
    StateRevision,
    FencingToken,
    InventoryGeneration,
    Toolchain,
    Configuration,
    Input,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaleGate {
    changed: BTreeSet<FreshnessDimension>,
}

impl StaleGate {
    pub fn new(
        changed: impl IntoIterator<Item = FreshnessDimension>,
    ) -> Result<Self, GateOutcomeError> {
        let changed: BTreeSet<_> = changed.into_iter().collect();
        if changed.is_empty() {
            return Err(GateOutcomeError::EmptyFreshnessChange);
        }
        Ok(Self { changed })
    }

    pub fn changed(&self) -> &BTreeSet<FreshnessDimension> {
        &self.changed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateOutcome {
    Pass,
    Fail(GateFailure),
    Error(GateError),
    Timeout(GateTimeout),
    Cancelled(GateCancellation),
    Stale(StaleGate),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GateOutcomeError {
    #[error("FAIL requires at least one finding")]
    EmptyFindings,
    #[error("TIMEOUT deadline must be greater than zero")]
    ZeroDeadline,
    #[error("STALE requires at least one changed freshness dimension")]
    EmptyFreshnessChange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateKey {
    gate_id: GateId,
    gate_implementation: GateImplementationDigest,
    policy: PolicyDigest,
    workspace_id: WorkspaceId,
    work_item_id: WorkItemId,
    story_id: Option<StoryId>,
    state_revision: StateRevision,
    fencing_token: FencingToken,
    inventory_generation: InventoryGeneration,
    toolchain: ToolchainDigest,
    configuration: ConfigDigest,
    input: InputFingerprint,
}

impl GateKey {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        gate_id: GateId,
        gate_implementation: GateImplementationDigest,
        policy: PolicyDigest,
        workspace_id: WorkspaceId,
        work_item_id: WorkItemId,
        story_id: Option<StoryId>,
        state_revision: StateRevision,
        fencing_token: FencingToken,
        inventory_generation: InventoryGeneration,
        toolchain: ToolchainDigest,
        configuration: ConfigDigest,
        input: InputFingerprint,
    ) -> Self {
        Self {
            gate_id,
            gate_implementation,
            policy,
            workspace_id,
            work_item_id,
            story_id,
            state_revision,
            fencing_token,
            inventory_generation,
            toolchain,
            configuration,
            input,
        }
    }

    pub const fn gate_id(&self) -> &GateId {
        &self.gate_id
    }

    pub const fn gate_implementation(&self) -> GateImplementationDigest {
        self.gate_implementation
    }

    pub const fn policy(&self) -> PolicyDigest {
        self.policy
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    pub const fn story_id(&self) -> Option<&StoryId> {
        self.story_id.as_ref()
    }

    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    pub const fn fencing_token(&self) -> FencingToken {
        self.fencing_token
    }

    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }

    pub const fn toolchain(&self) -> ToolchainDigest {
        self.toolchain
    }

    pub const fn configuration(&self) -> ConfigDigest {
        self.configuration
    }

    pub const fn input(&self) -> InputFingerprint {
        self.input
    }

    pub fn freshness_against(&self, current: &Self) -> GateFreshness {
        let mut changed = BTreeSet::new();
        compare(
            &mut changed,
            FreshnessDimension::GateId,
            &self.gate_id,
            &current.gate_id,
        );
        compare(
            &mut changed,
            FreshnessDimension::GateImplementation,
            &self.gate_implementation,
            &current.gate_implementation,
        );
        compare(
            &mut changed,
            FreshnessDimension::Policy,
            &self.policy,
            &current.policy,
        );
        compare(
            &mut changed,
            FreshnessDimension::Workspace,
            &self.workspace_id,
            &current.workspace_id,
        );
        compare(
            &mut changed,
            FreshnessDimension::WorkItem,
            &self.work_item_id,
            &current.work_item_id,
        );
        compare(
            &mut changed,
            FreshnessDimension::Story,
            &self.story_id,
            &current.story_id,
        );
        compare(
            &mut changed,
            FreshnessDimension::StateRevision,
            &self.state_revision,
            &current.state_revision,
        );
        compare(
            &mut changed,
            FreshnessDimension::FencingToken,
            &self.fencing_token,
            &current.fencing_token,
        );
        compare(
            &mut changed,
            FreshnessDimension::InventoryGeneration,
            &self.inventory_generation,
            &current.inventory_generation,
        );
        compare(
            &mut changed,
            FreshnessDimension::Toolchain,
            &self.toolchain,
            &current.toolchain,
        );
        compare(
            &mut changed,
            FreshnessDimension::Configuration,
            &self.configuration,
            &current.configuration,
        );
        compare(
            &mut changed,
            FreshnessDimension::Input,
            &self.input,
            &current.input,
        );

        if changed.is_empty() {
            GateFreshness::Fresh
        } else {
            GateFreshness::Stale(StaleGate { changed })
        }
    }
}

fn compare<T: PartialEq>(
    changed: &mut BTreeSet<FreshnessDimension>,
    dimension: FreshnessDimension,
    snapshot: &T,
    current: &T,
) {
    if snapshot != current {
        changed.insert(dimension);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateFreshness {
    Fresh,
    Stale(StaleGate),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateResult {
    key: GateKey,
    outcome: GateOutcome,
}

impl GateResult {
    pub const fn new(key: GateKey, outcome: GateOutcome) -> Self {
        Self { key, outcome }
    }

    pub const fn key(&self) -> &GateKey {
        &self.key
    }

    pub const fn outcome(&self) -> &GateOutcome {
        &self.outcome
    }

    pub fn outcome_against(&self, current: &GateKey) -> GateOutcome {
        match self.key.freshness_against(current) {
            GateFreshness::Fresh => self.outcome.clone(),
            GateFreshness::Stale(stale) => GateOutcome::Stale(stale),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use uuid::Uuid;

    use super::*;

    fn key(revision: u64, inventory: u64, input: &[u8]) -> GateKey {
        GateKey::new(
            GateId::new("G-14").expect("valid gate ID"),
            GateImplementationDigest::digest(b"gate-v1"),
            PolicyDigest::digest(b"policy-v1"),
            WorkspaceId::from_uuid(Uuid::from_u128(1)),
            WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid work item ID"),
            Some(StoryId::new("STORY-AE-SDD-RUST-DAEMON-001").expect("valid story ID")),
            StateRevision::new(revision),
            FencingToken::new(8),
            InventoryGeneration::new(inventory),
            ToolchainDigest::digest(b"rustc-1.97.1"),
            ConfigDigest::digest(b"config-v1"),
            InputFingerprint::digest(input),
        )
    }

    #[test]
    fn gate_outcome_six_classes_are_distinct_and_validated() {
        let finding = GateFinding::new(
            FindingCode::new("GATE_FAILED").expect("valid finding code"),
            [],
        );
        let outcomes = [
            GateOutcome::Pass,
            GateOutcome::Fail(GateFailure::new([finding]).expect("non-empty findings")),
            GateOutcome::Error(GateError::new(
                ErrorCode::new("GATE_ERROR").expect("valid error code"),
                true,
            )),
            GateOutcome::Timeout(GateTimeout::new(250).expect("positive deadline")),
            GateOutcome::Cancelled(GateCancellation::new(
                CancellationCode::new("CALLER_CANCELLED").expect("valid cancellation code"),
            )),
            GateOutcome::Stale(
                StaleGate::new([FreshnessDimension::StateRevision])
                    .expect("non-empty changed dimensions"),
            ),
        ];

        assert_eq!(outcomes.len(), 6);
        assert!(GateFailure::new([]).is_err());
        assert!(StaleGate::new([]).is_err());
        assert!(GateTimeout::new(0).is_err());
    }

    #[test]
    fn fresh_gate_key_preserves_the_recorded_outcome() {
        let snapshot = key(9, 3, b"input-v1");
        let result = GateResult::new(snapshot.clone(), GateOutcome::Pass);

        assert_eq!(snapshot.freshness_against(&snapshot), GateFreshness::Fresh);
        assert_eq!(result.outcome_against(&snapshot), GateOutcome::Pass);
    }

    #[test]
    fn changed_gate_key_becomes_stale_instead_of_reusing_pass() {
        let snapshot = key(9, 3, b"input-v1");
        let current = key(10, 4, b"input-v2");
        let result = GateResult::new(snapshot, GateOutcome::Pass);
        let GateOutcome::Stale(stale) = result.outcome_against(&current) else {
            panic!("changed freshness must produce STALE");
        };

        assert_eq!(
            stale.changed(),
            &BTreeSet::from([
                FreshnessDimension::StateRevision,
                FreshnessDimension::InventoryGeneration,
                FreshnessDimension::Input,
            ])
        );
    }

    proptest! {
        #[test]
        fn any_state_revision_change_invalidates_gate_pass(
            snapshot_revision in 0_u64..u64::MAX,
            delta in 1_u64..1_000,
        ) {
            let Some(current_revision) = snapshot_revision.checked_add(delta) else {
                return Ok(());
            };
            let snapshot = key(snapshot_revision, 3, b"input");
            let current = key(current_revision, 3, b"input");
            let result = GateResult::new(snapshot, GateOutcome::Pass);

            prop_assert!(matches!(result.outcome_against(&current), GateOutcome::Stale(_)));
        }
    }
}
