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
        payload,
    }
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
