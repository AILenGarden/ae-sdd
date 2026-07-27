mod support;

use ae_sdd_delegation::{
    ArtifactValidationReceipt, ArtifactVerifier, ChildFinding, ChildOutcome, ChildResult,
    Delegation, DelegationError, DelegationRequest, DelegationStatus, MemoryCleanupReceipt,
};
use ae_sdd_domain::{
    AgentLineage, AgentRole, ArtifactDigest, ArtifactRef, ClaimId, DeliverableContract,
    DeliverableId, DeliverableRequirement, FindingCode, HostAckId, HostActionId, InputFingerprint,
    OperationId, ProjectPathScope, ProjectRelativePath, ScopedGrant, StateRevision,
};
use ae_sdd_host::{
    ChildClaim, HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterId, HostTaskId,
    PhysicalSessionProof,
};
use uuid::Uuid;

use support::{create_action, delegation, grant, physical_proof, requested_delegation, session};

struct AcceptAll;

impl ArtifactVerifier for AcceptAll {
    fn verify(
        &self,
        _artifact: &ArtifactRef,
    ) -> Result<(), ae_sdd_delegation::ArtifactValidationError> {
        Ok(())
    }
}

fn child_result() -> ChildResult {
    ChildResult::new(
        ChildOutcome::Succeeded,
        "child completed its assignment",
        vec![
            ChildFinding::new(
                FindingCode::new("F-DELEGATION-001").expect("valid finding code"),
                "validated the requested behavior",
            )
            .expect("valid finding"),
        ],
        vec![],
        vec![],
        Some(OperationId::new("state.transition").expect("valid operation")),
        ArtifactDigest::digest(b"child-memory-snapshot"),
        &DeliverableContract::bounded_default([]).expect("valid contract"),
    )
    .expect("valid child result")
}

fn running_delegation() -> Delegation {
    let mut value = requested_delegation();
    let action = create_action();
    value.dispatch_create(&action).expect("dispatch create");
    value
        .attest(physical_proof(&action))
        .expect("attest child session");
    value
}

fn action_for(delegation_id: ae_sdd_domain::DelegationId, action_seed: u128) -> HostAction {
    HostAction::new(
        HostActionId::from_uuid(Uuid::from_u128(action_seed)),
        HostAdapterId::new("codex").expect("valid adapter"),
        1,
        HostActionKind::Create,
        Some(delegation_id),
        None,
        None,
        None,
        2_000,
        [7; 32],
    )
    .expect("valid create action")
}

fn proof_for(
    action: &HostAction,
    delegation_id: ae_sdd_domain::DelegationId,
    child_seed: u128,
) -> PhysicalSessionProof {
    let child_session = session(child_seed);
    let ack = HostAck::new(
        HostAckId::from_uuid(Uuid::from_u128(child_seed + 100)),
        action.action_id(),
        action.adapter_id().clone(),
        action.command_seq(),
        HostAckOutcome::Accepted,
        Some(HostTaskId::new(format!("task-{child_seed}")).expect("valid host task")),
        Some(child_session),
    )
    .expect("valid ACK");
    let claim = ChildClaim::new(
        ClaimId::from_uuid(Uuid::from_u128(child_seed + 200)),
        delegation_id,
        action.action_id(),
        child_session,
        1_900,
    )
    .expect("valid child claim");
    PhysicalSessionProof::establish(action, &ack, &claim, 1_500).expect("physical proof")
}

#[test]
fn collection_lifecycle_binds_result_artifacts_cleanup_and_projection() {
    let mut value = requested_delegation();
    assert_eq!(value.status(), DelegationStatus::Requested);
    assert_eq!(value.delegation_id(), delegation(10));
    assert_eq!(value.grant(), &grant());
    assert!(value.deliverable_contract().required().is_empty());
    assert_eq!(value.input_revision(), StateRevision::new(7));
    assert_eq!(
        value.input_fingerprint(),
        InputFingerprint::digest(b"assignment")
    );
    assert_eq!(value.deadline_unix_ms(), 2_000);

    let action = create_action();
    value.dispatch_create(&action).expect("dispatch create");
    value
        .attest(physical_proof(&action))
        .expect("attest physical child");

    let result = child_result();
    let staged = result.clone();
    let result_digest = result.digest();
    let memory_snapshot = result.memory_snapshot_digest();
    value
        .stage_result(
            session(2),
            StateRevision::new(7),
            InputFingerprint::digest(b"assignment"),
            result,
        )
        .expect("stage fresh child result");
    assert_eq!(value.status(), DelegationStatus::ResultStaged);

    let artifact_receipt = ArtifactValidationReceipt::validate(
        value.delegation_id(),
        &staged,
        value.deliverable_contract(),
        value.grant(),
        &AcceptAll,
    )
    .expect("artifact validation receipt");
    value
        .record_artifact_validation(artifact_receipt)
        .expect("record artifact validation");
    assert_eq!(value.status(), DelegationStatus::ArtifactsValidated);

    value
        .record_memory_cleanup(
            MemoryCleanupReceipt::new(
                value.delegation_id(),
                "delegation/10/private",
                memory_snapshot,
                ArtifactDigest::digest(b"cleanup"),
                2_100,
            )
            .expect("cleanup receipt"),
        )
        .expect("record memory cleanup");
    assert_eq!(value.status(), DelegationStatus::MemoryCleaned);

    let projection = value.complete().expect("complete collection");
    assert_eq!(value.status(), DelegationStatus::Completed);
    assert_eq!(projection.delegation_id(), delegation(10));
    assert_eq!(projection.outcome(), ChildOutcome::Succeeded);
    assert_eq!(projection.summary(), "child completed its assignment");
    assert_eq!(projection.findings().len(), 1);
    assert_eq!(
        projection
            .requested_action()
            .expect("requested operation")
            .as_str(),
        "state.transition"
    );
    assert_eq!(projection.result_digest(), result_digest);
    assert_eq!(projection.artifact_count(), 0);
    assert_eq!(
        value.mark_terminal(DelegationStatus::Failed),
        Err(DelegationError::AlreadyCompleted)
    );
}

#[test]
fn lifecycle_rejects_wrong_actions_proofs_child_inputs_and_receipts() {
    let mut requested = requested_delegation();
    let wait = HostAction::new(
        HostActionId::from_uuid(Uuid::from_u128(30)),
        HostAdapterId::new("codex").expect("valid adapter"),
        1,
        HostActionKind::Wait,
        Some(delegation(10)),
        None,
        None,
        None,
        2_000,
        [8; 32],
    )
    .expect("valid wait action");
    assert_eq!(
        requested.dispatch_create(&wait),
        Err(DelegationError::HostActionMismatch)
    );

    let action = create_action();
    requested.dispatch_create(&action).expect("dispatch create");
    let other_action = action_for(delegation(99), 31);
    assert_eq!(
        requested.attest(proof_for(&other_action, delegation(99), 3)),
        Err(DelegationError::PhysicalProofMismatch)
    );
    requested
        .attest(physical_proof(&action))
        .expect("matching proof remains valid");

    let result = child_result();
    assert_eq!(
        requested.stage_result(
            session(99),
            StateRevision::new(7),
            InputFingerprint::digest(b"assignment"),
            result.clone(),
        ),
        Err(DelegationError::ChildIdentityMismatch)
    );
    assert_eq!(
        requested.stage_result(
            session(2),
            StateRevision::new(8),
            InputFingerprint::digest(b"assignment"),
            result.clone(),
        ),
        Err(DelegationError::StaleChildResult)
    );
    assert_eq!(
        requested.stage_result(
            session(2),
            StateRevision::new(7),
            InputFingerprint::digest(b"stale"),
            result.clone(),
        ),
        Err(DelegationError::StaleChildResult)
    );
    requested
        .stage_result(
            session(2),
            StateRevision::new(7),
            InputFingerprint::digest(b"assignment"),
            result.clone(),
        )
        .expect("matching child result");

    let wrong_artifact_receipt = ArtifactValidationReceipt::validate(
        delegation(99),
        &result,
        requested.deliverable_contract(),
        requested.grant(),
        &AcceptAll,
    )
    .expect("well-formed receipt for another delegation");
    assert_eq!(
        requested.record_artifact_validation(wrong_artifact_receipt),
        Err(DelegationError::ArtifactReceiptMismatch)
    );
    let artifact_receipt = ArtifactValidationReceipt::validate(
        requested.delegation_id(),
        &result,
        requested.deliverable_contract(),
        requested.grant(),
        &AcceptAll,
    )
    .expect("matching artifact receipt");
    requested
        .record_artifact_validation(artifact_receipt)
        .expect("record matching artifact receipt");

    let wrong_cleanup = MemoryCleanupReceipt::new(
        delegation(99),
        "delegation/99/private",
        result.memory_snapshot_digest(),
        ArtifactDigest::digest(b"cleanup"),
        2_100,
    )
    .expect("well-formed cleanup receipt");
    assert_eq!(
        requested.record_memory_cleanup(wrong_cleanup),
        Err(DelegationError::CleanupReceiptMismatch)
    );
    let stale_cleanup = MemoryCleanupReceipt::new(
        requested.delegation_id(),
        "delegation/10/private",
        ArtifactDigest::digest(b"stale snapshot"),
        ArtifactDigest::digest(b"cleanup"),
        2_100,
    )
    .expect("well-formed stale cleanup receipt");
    assert_eq!(
        requested.record_memory_cleanup(stale_cleanup),
        Err(DelegationError::CleanupReceiptMismatch)
    );
}

#[test]
fn request_and_terminal_guards_fail_closed() {
    let parent_grant = grant();
    let invalid_deadline = DelegationRequest::new(
        delegation(40),
        AgentLineage::root(session(1)),
        AgentRole::Series,
        &parent_grant,
        grant(),
        DeliverableContract::bounded_default([]).expect("valid contract"),
        StateRevision::new(1),
        InputFingerprint::digest(b"input"),
        0,
    );
    assert!(matches!(
        invalid_deadline,
        Err(DelegationError::InvalidDeadline)
    ));

    let narrow = ScopedGrant::new(
        parent_grant.operations().iter().cloned(),
        parent_grant.capabilities().iter().cloned(),
        [ProjectPathScope::Subtree(
            ProjectRelativePath::new("out").expect("valid path"),
        )],
    );
    let outside_contract = DeliverableContract::bounded_default([DeliverableRequirement::new(
        DeliverableId::new("report").expect("valid id"),
        ae_sdd_domain::ArtifactKind::new("report").expect("valid kind"),
        ProjectRelativePath::new("elsewhere/report.json").expect("valid path"),
    )])
    .expect("valid contract");
    assert!(matches!(
        DelegationRequest::new(
            delegation(41),
            AgentLineage::root(session(1)),
            AgentRole::Series,
            &parent_grant,
            narrow,
            outside_contract,
            StateRevision::new(1),
            InputFingerprint::digest(b"input"),
            100,
        ),
        Err(DelegationError::DeliverableOutsideGrant)
    ));

    let mut value = requested_delegation();
    assert!(matches!(
        value.complete(),
        Err(DelegationError::InvalidTransition {
            from: DelegationStatus::Requested,
            expected: DelegationStatus::MemoryCleaned,
        })
    ));
    assert_eq!(
        value.mark_terminal(DelegationStatus::Running),
        Err(DelegationError::InvalidTerminalStatus)
    );
    value
        .mark_terminal(DelegationStatus::Failed)
        .expect("requested delegation may fail terminally");
    assert_eq!(value.status(), DelegationStatus::Failed);
    assert!(matches!(
        value.dispatch_create(&create_action()),
        Err(DelegationError::InvalidTransition {
            from: DelegationStatus::Failed,
            expected: DelegationStatus::Requested,
        })
    ));

    let mut running = running_delegation();
    running
        .mark_terminal(DelegationStatus::Cancelled)
        .expect("running delegation may be cancelled");
    assert_eq!(running.status(), DelegationStatus::Cancelled);
}
