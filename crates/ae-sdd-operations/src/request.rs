use std::collections::BTreeSet;

use ae_sdd_domain::{
    FencingToken, LeaseId, OperationId, ProjectKey, SessionId, StateRevision, WorkItemId,
    WorkspaceId,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{FieldKind, OperationName, OperationSpec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirmation {
    confirmation_id: Box<str>,
    approved_by: Box<str>,
    approved_at: Box<str>,
}

impl Confirmation {
    pub fn new(
        confirmation_id: impl Into<Box<str>>,
        approved_by: impl Into<Box<str>>,
        approved_at: impl Into<Box<str>>,
    ) -> Result<Self, OperationRequestError> {
        let value = Self {
            confirmation_id: confirmation_id.into(),
            approved_by: approved_by.into(),
            approved_at: approved_at.into(),
        };
        if value.confirmation_id.is_empty()
            || value.approved_by.is_empty()
            || value.approved_at.is_empty()
        {
            return Err(OperationRequestError::InvalidConfirmation);
        }
        Ok(value)
    }

    #[must_use]
    pub fn confirmation_id(&self) -> &str {
        &self.confirmation_id
    }

    #[must_use]
    pub fn approved_by(&self) -> &str {
        &self.approved_by
    }

    #[must_use]
    pub fn approved_at(&self) -> &str {
        &self.approved_at
    }
}

#[derive(Clone, Debug)]
pub struct OperationRequest {
    pub operation: OperationName,
    pub workspace_id: Option<WorkspaceId>,
    pub project_key: Option<ProjectKey>,
    pub work_item_id: Option<WorkItemId>,
    pub session_id: Option<SessionId>,
    pub lease_id: Option<LeaseId>,
    pub fencing_token: Option<FencingToken>,
    pub expected_revision: Option<StateRevision>,
    pub idempotency_key: Option<Box<str>>,
    pub confirmation: Option<Confirmation>,
    pub dry_run: bool,
    pub payload: Value,
}

#[derive(Clone, Debug)]
pub struct ValidatedOperationRequest {
    request: OperationRequest,
    operation_id: OperationId,
    payload_digest: [u8; 32],
}

impl ValidatedOperationRequest {
    pub fn validate(request: OperationRequest) -> Result<Self, OperationRequestError> {
        let spec = request.operation.spec();
        validate_preconditions(spec, &request)?;
        validate_payload(spec, &request.payload)?;
        let canonical = serde_json::to_vec(&request.payload)
            .map_err(OperationRequestError::CanonicalizePayload)?;
        Ok(Self {
            operation_id: OperationId::new(request.operation.as_str())
                .expect("frozen operation names are valid domain IDs"),
            request,
            payload_digest: Sha256::digest(canonical).into(),
        })
    }

    #[must_use]
    pub const fn operation(&self) -> OperationName {
        self.request.operation
    }

    #[must_use]
    pub const fn spec(&self) -> &'static OperationSpec {
        self.request.operation.spec()
    }

    #[must_use]
    pub const fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    #[must_use]
    pub const fn payload_digest(&self) -> &[u8; 32] {
        &self.payload_digest
    }

    #[must_use]
    pub const fn request(&self) -> &OperationRequest {
        &self.request
    }
}

/// Validates only the operation-specific business payload. Transport clients
/// may use this as an early fail-closed check; the daemon still validates the
/// complete request and remains authoritative.
pub fn validate_operation_payload(
    operation: OperationName,
    payload: &Value,
) -> Result<(), OperationRequestError> {
    validate_payload(operation.spec(), payload)
}

fn validate_preconditions(
    spec: &OperationSpec,
    request: &OperationRequest,
) -> Result<(), OperationRequestError> {
    required(spec.requires_workspace, request.workspace_id, "workspaceId")?;
    required(
        spec.requires_workspace,
        request.project_key.as_ref(),
        "projectKey",
    )?;
    required(
        spec.requires_work_item,
        request.work_item_id.as_ref(),
        "workItemId",
    )?;
    required(spec.requires_lease, request.lease_id, "leaseId")?;
    required(spec.requires_lease, request.fencing_token, "fencingToken")?;
    required(
        spec.requires_revision,
        request.expected_revision,
        "expectedRevision",
    )?;
    required(
        spec.requires_idempotency,
        request.idempotency_key.as_ref(),
        "idempotencyKey",
    )?;
    required(
        spec.requires_confirmation,
        request.confirmation.as_ref(),
        "confirmation",
    )?;
    if request
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.is_empty() || key.len() > 256)
    {
        return Err(OperationRequestError::InvalidIdempotencyKey);
    }
    Ok(())
}

fn required<T>(
    required: bool,
    value: Option<T>,
    field: &'static str,
) -> Result<(), OperationRequestError> {
    if required && value.is_none() {
        Err(OperationRequestError::RequiredPrecondition(field))
    } else {
        Ok(())
    }
}

fn validate_payload(spec: &OperationSpec, payload: &Value) -> Result<(), OperationRequestError> {
    let object = payload
        .as_object()
        .ok_or(OperationRequestError::PayloadMustBeObject)?;
    let allowed: BTreeSet<_> = spec.fields.iter().map(|field| field.name).collect();
    if let Some(unknown) = object
        .keys()
        .find(|field| !allowed.contains(field.as_str()))
    {
        return Err(OperationRequestError::UnknownPayloadField(unknown.clone()));
    }
    for field in spec.fields {
        match object.get(field.name) {
            None if field.required => {
                return Err(OperationRequestError::RequiredPayloadField(field.name));
            }
            None => {}
            Some(value) if field_matches(field.kind, value) => {
                validate_semantics(spec.operation, field.name, value)?;
            }
            Some(_) => return Err(OperationRequestError::PayloadFieldType(field.name)),
        }
    }
    Ok(())
}

fn field_matches(kind: FieldKind, value: &Value) -> bool {
    match kind {
        FieldKind::String => value.is_string(),
        FieldKind::Object => value.is_object(),
        FieldKind::Array => value.is_array(),
        FieldKind::Boolean => value.is_boolean(),
        FieldKind::Integer => value.is_i64() || value.is_u64(),
        FieldKind::StringOrArray => value.is_string() || value.is_array(),
        FieldKind::StringOrObject => value.is_string() || value.is_object(),
    }
}

fn validate_semantics(
    operation: OperationName,
    field: &'static str,
    value: &Value,
) -> Result<(), OperationRequestError> {
    if value.as_str().is_some_and(str::is_empty) {
        return Err(OperationRequestError::EmptyString(field));
    }
    if matches!(
        operation,
        OperationName::LeaseAcquire | OperationName::LeaseRenew
    ) && field == "ttlSeconds"
    {
        let ttl = value
            .as_u64()
            .ok_or(OperationRequestError::PayloadFieldType(field))?;
        if !(30..=3_600).contains(&ttl) {
            return Err(OperationRequestError::InvalidLeaseTtl);
        }
    }
    if matches!(
        (operation, field),
        (
            OperationName::ExecutionPlanSet,
            "changedPaths" | "verification"
        ) | (OperationName::VerificationPlan, "changedPaths")
    ) && value.as_array().is_some_and(Vec::is_empty)
    {
        return Err(OperationRequestError::EmptyArray(field));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum OperationRequestError {
    #[error("operation requires request field {0}")]
    RequiredPrecondition(&'static str),
    #[error("idempotency key must be in 1..=256 bytes")]
    InvalidIdempotencyKey,
    #[error("confirmation fields must be non-empty")]
    InvalidConfirmation,
    #[error("operation payload must be a JSON object")]
    PayloadMustBeObject,
    #[error("operation payload contains unknown field {0}")]
    UnknownPayloadField(String),
    #[error("operation payload requires field {0}")]
    RequiredPayloadField(&'static str),
    #[error("operation payload field {0} has the wrong type")]
    PayloadFieldType(&'static str),
    #[error("operation payload string field {0} must not be empty")]
    EmptyString(&'static str),
    #[error("operation payload array field {0} must not be empty")]
    EmptyArray(&'static str),
    #[error("lease TTL must be in 30..=3600 seconds")]
    InvalidLeaseTtl,
    #[error("failed to canonicalize operation payload: {0}")]
    CanonicalizePayload(serde_json::Error),
}
