use ae_sdd_domain::{
    AgentLineage, AgentRole, DelegationId, DeliverableContract, InputFingerprint, OperationId,
    ResultDigest, ScopedGrant, SessionId, StateRevision,
};
use ae_sdd_host::{HostAction, HostActionKind, PhysicalSessionProof};
use thiserror::Error;

use crate::{
    ArtifactValidationReceipt, ChildFinding, ChildOutcome, ChildResult, MemoryCleanupReceipt,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationStatus {
    Requested,
    Spawning,
    Running,
    ResultStaged,
    ArtifactsValidated,
    MemoryCleaned,
    Completed,
    Failed,
    Expired,
    Orphaned,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct DelegationRequest {
    delegation_id: DelegationId,
    parent_lineage: AgentLineage,
    child_role: AgentRole,
    grant: ScopedGrant,
    deliverable_contract: DeliverableContract,
    input_revision: StateRevision,
    input_fingerprint: InputFingerprint,
    deadline_unix_ms: u64,
}

impl DelegationRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        delegation_id: DelegationId,
        parent_lineage: AgentLineage,
        child_role: AgentRole,
        parent_grant: &ScopedGrant,
        grant: ScopedGrant,
        deliverable_contract: DeliverableContract,
        input_revision: StateRevision,
        input_fingerprint: InputFingerprint,
        deadline_unix_ms: u64,
    ) -> Result<Self, DelegationError> {
        if !parent_lineage.current().role().may_spawn(child_role) {
            return Err(DelegationError::RoleCannotDelegate);
        }
        parent_grant
            .validate_child(&grant)
            .map_err(|_| DelegationError::GrantExpansion)?;
        deliverable_contract
            .validate_scope(&grant)
            .map_err(|_| DelegationError::DeliverableOutsideGrant)?;
        if deadline_unix_ms == 0 {
            return Err(DelegationError::InvalidDeadline);
        }
        Ok(Self {
            delegation_id,
            parent_lineage,
            child_role,
            grant,
            deliverable_contract,
            input_revision,
            input_fingerprint,
            deadline_unix_ms,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Delegation {
    request: DelegationRequest,
    status: DelegationStatus,
    create_action_id: Option<ae_sdd_domain::HostActionId>,
    child_lineage: Option<AgentLineage>,
    physical_proof: Option<PhysicalSessionProof>,
    result: Option<ChildResult>,
    artifact_receipt: Option<ArtifactValidationReceipt>,
    cleanup_receipt: Option<MemoryCleanupReceipt>,
}

impl Delegation {
    #[must_use]
    pub const fn new(request: DelegationRequest) -> Self {
        Self {
            request,
            status: DelegationStatus::Requested,
            create_action_id: None,
            child_lineage: None,
            physical_proof: None,
            result: None,
            artifact_receipt: None,
            cleanup_receipt: None,
        }
    }

    #[must_use]
    pub const fn status(&self) -> DelegationStatus {
        self.status
    }

    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.request.delegation_id
    }

    #[must_use]
    pub const fn grant(&self) -> &ScopedGrant {
        &self.request.grant
    }

    #[must_use]
    pub const fn deliverable_contract(&self) -> &DeliverableContract {
        &self.request.deliverable_contract
    }

    #[must_use]
    pub const fn input_revision(&self) -> StateRevision {
        self.request.input_revision
    }

    #[must_use]
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.request.input_fingerprint
    }

    pub fn dispatch_create(&mut self, action: &HostAction) -> Result<(), DelegationError> {
        self.expect(DelegationStatus::Requested)?;
        if action.kind() != HostActionKind::Create
            || action.delegation_id() != Some(self.request.delegation_id)
        {
            return Err(DelegationError::HostActionMismatch);
        }
        self.create_action_id = Some(action.action_id());
        self.status = DelegationStatus::Spawning;
        Ok(())
    }

    pub fn attest(&mut self, proof: PhysicalSessionProof) -> Result<(), DelegationError> {
        self.expect(DelegationStatus::Spawning)?;
        if proof.delegation_id() != self.request.delegation_id
            || Some(proof.action_id()) != self.create_action_id
        {
            return Err(DelegationError::PhysicalProofMismatch);
        }
        let child_lineage = self
            .request
            .parent_lineage
            .spawn_child(
                proof.child_session_id(),
                self.request.delegation_id,
                self.request.child_role,
            )
            .map_err(|_| DelegationError::RoleCannotDelegate)?;
        self.child_lineage = Some(child_lineage);
        self.physical_proof = Some(proof);
        self.status = DelegationStatus::Running;
        Ok(())
    }

    pub fn stage_result(
        &mut self,
        child_session_id: SessionId,
        input_revision: StateRevision,
        input_fingerprint: InputFingerprint,
        result: ChildResult,
    ) -> Result<(), DelegationError> {
        self.expect(DelegationStatus::Running)?;
        if self
            .physical_proof
            .as_ref()
            .is_none_or(|proof| proof.child_session_id() != child_session_id)
        {
            return Err(DelegationError::ChildIdentityMismatch);
        }
        if input_revision != self.request.input_revision
            || input_fingerprint != self.request.input_fingerprint
        {
            return Err(DelegationError::StaleChildResult);
        }
        self.result = Some(result);
        self.status = DelegationStatus::ResultStaged;
        Ok(())
    }

    pub fn record_artifact_validation(
        &mut self,
        receipt: ArtifactValidationReceipt,
    ) -> Result<(), DelegationError> {
        self.expect(DelegationStatus::ResultStaged)?;
        let result = self.result.as_ref().ok_or(DelegationError::ResultMissing)?;
        if receipt.delegation_id() != self.request.delegation_id
            || receipt.result_digest() != result.digest()
        {
            return Err(DelegationError::ArtifactReceiptMismatch);
        }
        self.artifact_receipt = Some(receipt);
        self.status = DelegationStatus::ArtifactsValidated;
        Ok(())
    }

    pub fn record_memory_cleanup(
        &mut self,
        receipt: MemoryCleanupReceipt,
    ) -> Result<(), DelegationError> {
        self.expect(DelegationStatus::ArtifactsValidated)?;
        let result = self.result.as_ref().ok_or(DelegationError::ResultMissing)?;
        if receipt.delegation_id() != self.request.delegation_id
            || receipt.snapshot_digest() != result.memory_snapshot_digest()
        {
            return Err(DelegationError::CleanupReceiptMismatch);
        }
        self.cleanup_receipt = Some(receipt);
        self.status = DelegationStatus::MemoryCleaned;
        Ok(())
    }

    pub fn complete(&mut self) -> Result<CollectProjection, DelegationError> {
        self.expect(DelegationStatus::MemoryCleaned)?;
        let result = self.result.as_ref().ok_or(DelegationError::ResultMissing)?;
        let projection = CollectProjection {
            delegation_id: self.request.delegation_id,
            outcome: result.outcome(),
            summary: result.summary().into(),
            findings: result.findings().to_vec(),
            requested_action: result.requested_action().cloned(),
            result_digest: result.digest(),
            artifact_count: result.deliverables().len(),
        };
        self.status = DelegationStatus::Completed;
        Ok(projection)
    }

    pub fn mark_terminal(&mut self, status: DelegationStatus) -> Result<(), DelegationError> {
        if !matches!(
            status,
            DelegationStatus::Failed
                | DelegationStatus::Expired
                | DelegationStatus::Orphaned
                | DelegationStatus::Cancelled
        ) {
            return Err(DelegationError::InvalidTerminalStatus);
        }
        if matches!(self.status, DelegationStatus::Completed) {
            return Err(DelegationError::AlreadyCompleted);
        }
        self.status = status;
        Ok(())
    }

    #[must_use]
    pub const fn deadline_unix_ms(&self) -> u64 {
        self.request.deadline_unix_ms
    }

    fn expect(&self, expected: DelegationStatus) -> Result<(), DelegationError> {
        if self.status == expected {
            Ok(())
        } else {
            Err(DelegationError::InvalidTransition {
                from: self.status,
                expected,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectProjection {
    delegation_id: DelegationId,
    outcome: ChildOutcome,
    summary: Box<str>,
    findings: Vec<ChildFinding>,
    requested_action: Option<OperationId>,
    result_digest: ResultDigest,
    artifact_count: usize,
}

impl CollectProjection {
    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    #[must_use]
    pub const fn outcome(&self) -> ChildOutcome {
        self.outcome
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn findings(&self) -> &[ChildFinding] {
        &self.findings
    }

    #[must_use]
    pub const fn requested_action(&self) -> Option<&OperationId> {
        self.requested_action.as_ref()
    }

    #[must_use]
    pub const fn result_digest(&self) -> ResultDigest {
        self.result_digest
    }

    #[must_use]
    pub const fn artifact_count(&self) -> usize {
        self.artifact_count
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DelegationError {
    #[error("parent role cannot create the requested child role")]
    RoleCannotDelegate,
    #[error("child grant expands the parent grant")]
    GrantExpansion,
    #[error("deliverable contract references a path outside the child grant")]
    DeliverableOutsideGrant,
    #[error("delegation deadline must be greater than zero")]
    InvalidDeadline,
    #[error("expected delegation state {expected:?}, found {from:?}")]
    InvalidTransition {
        from: DelegationStatus,
        expected: DelegationStatus,
    },
    #[error("host create action does not bind this delegation")]
    HostActionMismatch,
    #[error("physical session proof does not bind this delegation/action")]
    PhysicalProofMismatch,
    #[error("child result came from a session other than the attested child")]
    ChildIdentityMismatch,
    #[error("child result input revision or fingerprint is stale")]
    StaleChildResult,
    #[error("child result is missing")]
    ResultMissing,
    #[error("artifact validation receipt does not bind the staged result")]
    ArtifactReceiptMismatch,
    #[error("memory cleanup receipt does not bind the child snapshot")]
    CleanupReceiptMismatch,
    #[error("requested terminal status is not terminal")]
    InvalidTerminalStatus,
    #[error("completed delegation cannot transition to another terminal state")]
    AlreadyCompleted,
}
