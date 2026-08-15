use ae_sdd_domain::{
    AgentRole, FencingToken, LeaseId, OperationId, ProjectKey, ProjectPathScope, ScopedGrant,
    SessionId, StateRevision, WorkItemId, WorkspaceId,
};
use ae_sdd_operations::{
    Confirmation, ExecutionIdentity, OperationBackend, OperationName, OperationRequest,
    OperationRequestError, OperationResponse, OperationService, OperationServiceError,
    ValidatedOperationRequest,
};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;
use uuid::Uuid;

fn transition(payload: serde_json::Value) -> OperationRequest {
    OperationRequest {
        operation: OperationName::StateTransition,
        workspace_id: Some(WorkspaceId::from_uuid(Uuid::from_u128(1))),
        project_key: Some(ProjectKey::new("ae-sdd").expect("valid project")),
        work_item_id: Some(WorkItemId::new("PRD-1").expect("valid work item")),
        session_id: Some(SessionId::from_uuid(Uuid::from_u128(2))),
        lease_id: Some(LeaseId::from_uuid(Uuid::from_u128(3))),
        fencing_token: Some(FencingToken::new(8)),
        expected_revision: Some(StateRevision::new(12)),
        idempotency_key: Some("transition-1".into()),
        confirmation: Some(
            Confirmation::new("confirmation-1", "user", "2026-07-23T00:00:00Z")
                .expect("valid confirmation"),
        ),
        dry_run: false,
        payload,
    }
}

fn plan_approval(confirmation_id: &str) -> OperationRequest {
    let mut request = transition(json!({"approvedBy":"payload"}));
    request.operation = OperationName::ExecutionPlanApprove;
    request.confirmation = Some(
        Confirmation::new(confirmation_id, "user", "2026-07-23T00:00:00Z")
            .expect("valid confirmation"),
    );
    request
}

#[test]
fn canonical_payload_fingerprint_is_stable_and_changes_on_mutation() {
    let first = ValidatedOperationRequest::validate(transition(json!({
        "targetPhase": "test-running"
    })))
    .expect("valid request");
    let retry = ValidatedOperationRequest::validate(transition(json!({
        "targetPhase": "test-running"
    })))
    .expect("valid retry");
    let mutated = ValidatedOperationRequest::validate(transition(json!({
        "targetPhase": "completed"
    })))
    .expect("well-formed mutation");

    assert_eq!(first.payload_digest(), retry.payload_digest());
    assert_ne!(first.payload_digest(), mutated.payload_digest());
}

#[test]
fn confirmation_is_part_of_canonical_operation_fingerprint() {
    let first = ValidatedOperationRequest::validate(plan_approval("plan:first"))
        .expect("valid first approval");
    let retry = ValidatedOperationRequest::validate(plan_approval("plan:first"))
        .expect("valid retry approval");
    let changed = ValidatedOperationRequest::validate(plan_approval("plan:second"))
        .expect("valid changed approval");

    assert_eq!(first.payload_digest(), retry.payload_digest());
    assert_ne!(
        first.payload_digest(),
        changed.payload_digest(),
        "changing confirmation must change the idempotency fingerprint"
    );
}

#[test]
fn protected_operation_requires_every_registry_precondition() {
    let mut request = transition(json!({"targetPhase": "test-running"}));
    request.lease_id = None;
    assert!(matches!(
        ValidatedOperationRequest::validate(request),
        Err(OperationRequestError::RequiredPrecondition("leaseId"))
    ));
}

#[derive(Debug, Error)]
#[error("backend should not run for denied identity")]
struct BackendError;

struct NeverBackend;

impl OperationBackend for NeverBackend {
    type Error = BackendError;

    fn read(&self, _request: &ValidatedOperationRequest) -> Result<OperationResponse, Self::Error> {
        Err(BackendError)
    }

    fn mutate(
        &self,
        _request: &ValidatedOperationRequest,
    ) -> Result<OperationResponse, Self::Error> {
        Err(BackendError)
    }

    fn dry_run(
        &self,
        _request: &ValidatedOperationRequest,
    ) -> Result<OperationResponse, Self::Error> {
        Err(BackendError)
    }
}

#[test]
fn trusted_grant_not_client_role_controls_authorization() {
    let task_grant = ScopedGrant::new(
        [OperationId::new("evidence.record").expect("valid operation")],
        [],
        [ProjectPathScope::ProjectRoot],
    );
    let result = OperationService::execute(
        ExecutionIdentity::Agent {
            role: AgentRole::Task,
            grant: &task_grant,
        },
        transition(json!({"targetPhase": "test-running"})),
        &NeverBackend,
    );
    assert!(matches!(
        result,
        Err(OperationServiceError::RoleOperationForbidden)
    ));
}

#[derive(Default)]
struct DryRunSpy {
    mutate_calls: AtomicUsize,
    dry_run_calls: AtomicUsize,
}

impl OperationBackend for DryRunSpy {
    type Error = BackendError;

    fn read(&self, _request: &ValidatedOperationRequest) -> Result<OperationResponse, Self::Error> {
        Err(BackendError)
    }

    fn mutate(
        &self,
        _request: &ValidatedOperationRequest,
    ) -> Result<OperationResponse, Self::Error> {
        self.mutate_calls.fetch_add(1, Ordering::AcqRel);
        Err(BackendError)
    }

    fn dry_run(
        &self,
        request: &ValidatedOperationRequest,
    ) -> Result<OperationResponse, Self::Error> {
        self.dry_run_calls.fetch_add(1, Ordering::AcqRel);
        Ok(OperationResponse {
            changed: false,
            revision_before: request.request().expected_revision,
            revision_after: request.request().expected_revision,
            receipt_digest: None,
            data: json!({"dryRun":true}),
        })
    }
}

#[test]
fn dry_run_dispatch_never_calls_the_mutation_backend() {
    let grant = ScopedGrant::new(
        [OperationId::new("state.transition").expect("operation")],
        [],
        [],
    );
    let backend = DryRunSpy::default();
    let mut request = transition(json!({"targetPhase":"test-running"}));
    request.dry_run = true;
    let response = OperationService::execute(
        ExecutionIdentity::Agent {
            role: AgentRole::Root,
            grant: &grant,
        },
        request,
        &backend,
    )
    .expect("dry-run validates");

    assert!(!response.changed);
    assert_eq!(backend.dry_run_calls.load(Ordering::Acquire), 1);
    assert_eq!(backend.mutate_calls.load(Ordering::Acquire), 0);
}
