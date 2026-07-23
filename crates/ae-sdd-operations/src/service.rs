use ae_sdd_domain::{AgentRole, OperationId, ScopedGrant, StateRevision};
use serde_json::Value;
use thiserror::Error;

use crate::{OperationRequest, OperationRequestError, ValidatedOperationRequest};

#[derive(Clone, Debug)]
pub enum ExecutionIdentity<'a> {
    Agent {
        role: AgentRole,
        grant: &'a ScopedGrant,
    },
    Admin,
}

pub trait OperationBackend {
    type Error: std::error::Error + Send + Sync + 'static;

    fn read(&self, request: &ValidatedOperationRequest) -> Result<OperationResponse, Self::Error>;
    fn mutate(&self, request: &ValidatedOperationRequest)
    -> Result<OperationResponse, Self::Error>;
    fn dry_run(
        &self,
        request: &ValidatedOperationRequest,
    ) -> Result<OperationResponse, Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct OperationResponse {
    pub changed: bool,
    pub revision_before: Option<StateRevision>,
    pub revision_after: Option<StateRevision>,
    pub receipt_digest: Option<[u8; 32]>,
    pub data: Value,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OperationService;

impl OperationService {
    pub fn execute<B: OperationBackend>(
        identity: ExecutionIdentity<'_>,
        request: OperationRequest,
        backend: &B,
    ) -> Result<OperationResponse, OperationServiceError> {
        let validated = ValidatedOperationRequest::validate(request)?;
        authorize(identity, validated.operation_id())?;
        let response = if validated.spec().writes && validated.request().dry_run {
            backend.dry_run(&validated)
        } else if validated.spec().writes {
            backend.mutate(&validated)
        } else {
            backend.read(&validated)
        }
        .map_err(|error| OperationServiceError::Backend(Box::new(error)))?;
        validate_response(
            validated.spec().writes,
            validated.request().dry_run,
            &response,
        )?;
        Ok(response)
    }
}

fn authorize(
    identity: ExecutionIdentity<'_>,
    operation: &OperationId,
) -> Result<(), OperationServiceError> {
    match identity {
        ExecutionIdentity::Admin => Ok(()),
        ExecutionIdentity::Agent { role: _, grant } if grant.operations().contains(operation) => {
            Ok(())
        }
        ExecutionIdentity::Agent { .. } => Err(OperationServiceError::RoleOperationForbidden),
    }
}

fn validate_response(
    writes: bool,
    dry_run: bool,
    response: &OperationResponse,
) -> Result<(), OperationServiceError> {
    if writes
        && dry_run
        && (response.changed
            || response.revision_before.is_none()
            || response.revision_before != response.revision_after
            || response.receipt_digest.is_some())
    {
        return Err(OperationServiceError::DryRunReceiptInvalid);
    }
    if writes
        && !dry_run
        && (response.revision_before.is_none()
            || response.revision_after.is_none()
            || response.receipt_digest.is_none())
    {
        return Err(OperationServiceError::MutationReceiptIncomplete);
    }
    if !writes && response.changed {
        return Err(OperationServiceError::ReadReportedMutation);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum OperationServiceError {
    #[error(transparent)]
    InvalidRequest(#[from] OperationRequestError),
    #[error("trusted Agent role/grant does not permit this operation")]
    RoleOperationForbidden,
    #[error("operation backend failed: {0}")]
    Backend(Box<dyn std::error::Error + Send + Sync>),
    #[error("mutation response lacks revision or committed receipt")]
    MutationReceiptIncomplete,
    #[error("dry-run response changed state or exposed a committed receipt")]
    DryRunReceiptInvalid,
    #[error("read-only operation backend reported a mutation")]
    ReadReportedMutation,
}
