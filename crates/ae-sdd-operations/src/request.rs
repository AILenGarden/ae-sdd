use std::collections::BTreeSet;

use ae_sdd_domain::{
    FencingToken, LeaseId, OperationId, ProjectKey, SessionId, StateRevision, StoryId, WorkItemId,
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
        let payload_digest = Self::canonical_idempotency_digest(&request)?;
        Ok(Self {
            operation_id: OperationId::new(request.operation.as_str())
                .expect("frozen operation names are valid domain IDs"),
            request,
            payload_digest,
        })
    }

    /// Hashes every caller-controlled value that can change the authority of
    /// a write while preserving the historical payload-only digest for
    /// operations without a confirmation.
    pub fn canonical_idempotency_digest(
        request: &OperationRequest,
    ) -> Result<[u8; 32], OperationRequestError> {
        let canonical_value = match request.confirmation.as_ref() {
            Some(confirmation) => serde_json::json!({
                "confirmation": {
                    "approvedAt": confirmation.approved_at(),
                    "approvedBy": confirmation.approved_by(),
                    "confirmationId": confirmation.confirmation_id(),
                },
                "payload": request.payload.clone(),
            }),
            None => request.payload.clone(),
        };
        let canonical = serde_json::to_vec(&canonical_value)
            .map_err(OperationRequestError::CanonicalizePayload)?;
        Ok(Sha256::digest(canonical).into())
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
                validate_top_level_field_semantics(field.kind, field.name, value)?;
                validate_semantics(spec.operation, field.name, value)?;
                validate_nested_shape(field, value, field.name)?;
            }
            Some(_) => return Err(OperationRequestError::PayloadFieldType(field.name)),
        }
    }
    if spec.operation == OperationName::WorkItemCreate {
        validate_workitem_create(object)?;
    }
    if spec.operation == OperationName::DocumentSave {
        validate_document_save(object)?;
    }
    Ok(())
}

fn validate_document_save(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), OperationRequestError> {
    if payload.get("intent").and_then(Value::as_str) != Some("STORY") {
        return Ok(());
    }
    let doc_id = payload
        .get("docId")
        .and_then(Value::as_str)
        .ok_or(OperationRequestError::RequiredPayloadField("docId"))?;
    if doc_id
        .strip_prefix("STORY-")
        .is_none_or(|suffix| suffix.is_empty())
        || StoryId::new(doc_id).is_err()
    {
        return Err(OperationRequestError::InvalidStoryDocumentId(
            doc_id.to_owned(),
        ));
    }
    Ok(())
}

fn validate_nested_shape(
    field: &crate::FieldSpec,
    value: &Value,
    path: &str,
) -> Result<(), OperationRequestError> {
    let Some(item_kind) = field.item_kind else {
        return Ok(());
    };
    let Some(array) = value.as_array() else {
        return Err(OperationRequestError::NestedPayloadSchema {
            path: path.to_owned(),
            problem: "field has the wrong type",
        });
    };

    for (index, item) in array.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        if !field_matches(item_kind, item) {
            return Err(OperationRequestError::NestedPayloadSchema {
                path: item_path,
                problem: "array item has the wrong type",
            });
        }
        if let Some(fields) = field.items {
            validate_nested_object(fields, item, &item_path)?;
        }
    }
    Ok(())
}

fn validate_nested_object(
    fields: &[crate::FieldSpec],
    value: &Value,
    path: &str,
) -> Result<(), OperationRequestError> {
    let Some(object) = value.as_object() else {
        return Err(OperationRequestError::NestedPayloadSchema {
            path: path.to_owned(),
            problem: "field has the wrong type",
        });
    };
    let allowed: BTreeSet<_> = fields.iter().map(|field| field.name).collect();
    if let Some(unknown) = object
        .keys()
        .find(|field| !allowed.contains(field.as_str()))
    {
        return Err(OperationRequestError::NestedPayloadSchema {
            path: format!("{path}.{unknown}"),
            problem: "unknown field",
        });
    }

    for field in fields {
        let field_path = format!("{path}.{}", field.name);
        match object.get(field.name) {
            None if field.required => {
                return Err(OperationRequestError::NestedPayloadSchema {
                    path: field_path,
                    problem: "required field is missing",
                });
            }
            None => {}
            Some(value) if field_matches(field.kind, value) => {
                validate_nested_field_semantics(field.kind, value, &field_path)?;
                validate_nested_shape(field, value, &field_path)?;
            }
            Some(_) => {
                return Err(OperationRequestError::NestedPayloadSchema {
                    path: field_path,
                    problem: "field has the wrong type",
                });
            }
        }
    }
    Ok(())
}

fn validate_top_level_field_semantics(
    kind: FieldKind,
    field: &'static str,
    value: &Value,
) -> Result<(), OperationRequestError> {
    if kind != FieldKind::StringOrArray {
        return Ok(());
    }
    if value.as_str().is_some_and(str::is_empty) {
        return Err(OperationRequestError::EmptyString(field));
    }
    if let Some(items) = value.as_array() {
        if items.is_empty() {
            return Err(OperationRequestError::EmptyArray(field));
        }
        if items
            .iter()
            .any(|item| item.as_str().is_none_or(str::is_empty))
        {
            return Err(OperationRequestError::PayloadFieldType(field));
        }
    }
    Ok(())
}

fn validate_nested_field_semantics(
    kind: FieldKind,
    value: &Value,
    path: &str,
) -> Result<(), OperationRequestError> {
    let invalid =
        |path: String, problem| OperationRequestError::NestedPayloadSchema { path, problem };
    if matches!(
        kind,
        FieldKind::String | FieldKind::StringOrArray | FieldKind::StringOrObject
    ) && value.as_str().is_some_and(str::is_empty)
    {
        return Err(invalid(path.to_owned(), "field must be non-empty"));
    }
    if kind == FieldKind::StringOrArray
        && let Some(items) = value.as_array()
    {
        if items.is_empty() {
            return Err(invalid(path.to_owned(), "array must be non-empty"));
        }
        for (index, item) in items.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            let Some(item) = item.as_str() else {
                return Err(invalid(item_path, "array item must be a string"));
            };
            if item.is_empty() {
                return Err(invalid(item_path, "array item must be non-empty"));
            }
        }
    }
    Ok(())
}

fn validate_workitem_create(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), OperationRequestError> {
    let Some(requested_intent) = payload.get("requestedIntent").and_then(Value::as_str) else {
        return Ok(());
    };
    let entry_node = payload
        .get("entryNode")
        .and_then(Value::as_str)
        .map(str::trim);
    let normalized = requested_intent.trim().to_ascii_uppercase();
    if entry_node != Some("ROUTE") || !matches!(normalized.as_str(), "DR" | "STORY" | "CODING_PLAN")
    {
        return Err(OperationRequestError::InvalidRequestedIntent(
            requested_intent.to_owned(),
        ));
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
    if matches!(
        (operation, field),
        (OperationName::WorkItemCreate, "providedDocuments")
    ) {
        validate_provided_documents(value)?;
    }
    Ok(())
}

/// Maximum number of documents a single `workitem.create` may adopt.
const MAX_PROVIDED_DOCUMENTS: usize = 64;

/// Validates the JSON-level contract of a `workitem.create` adoption tree:
/// entry shape, intent vocabulary, docId uniqueness, and parent references.
/// Path traversal screening and file existence need the workspace root, so the
/// create path enforces them separately.
fn validate_provided_documents(value: &Value) -> Result<(), OperationRequestError> {
    let invalid = |reason: &str| OperationRequestError::InvalidProvidedDocuments(reason.to_owned());
    let entries = value
        .as_array()
        .ok_or_else(|| invalid("providedDocuments must be an array"))?;
    if entries.len() > MAX_PROVIDED_DOCUMENTS {
        return Err(invalid("providedDocuments is limited to 64 entries"));
    }
    let mut intents = Vec::with_capacity(entries.len());
    let mut doc_ids = BTreeSet::new();
    for entry in entries {
        let entry = entry
            .as_object()
            .ok_or_else(|| invalid("each providedDocuments entry must be an object"))?;
        if let Some(unknown) = entry
            .keys()
            .find(|key| !matches!(key.as_str(), "intent" | "docId" | "path" | "parentDocId"))
        {
            return Err(invalid(&format!(
                "providedDocuments entry contains unknown field {unknown}"
            )));
        }
        let intent = entry
            .get("intent")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("providedDocuments.intent must be a string"))?;
        if !matches!(intent, "PRD" | "DR" | "STORY") {
            return Err(invalid(
                "providedDocuments.intent must be PRD, DR, or STORY",
            ));
        }
        let doc_id = entry
            .get("docId")
            .and_then(Value::as_str)
            .filter(|doc_id| !doc_id.is_empty())
            .ok_or_else(|| invalid("providedDocuments.docId must be non-empty text"))?;
        if !doc_ids.insert(doc_id) {
            return Err(invalid("providedDocuments.docId must be unique"));
        }
        if entry
            .get("path")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(invalid("providedDocuments.path must be non-empty text"));
        }
        let parent = entry.get("parentDocId");
        if parent.is_some() && intent == "PRD" {
            return Err(invalid("a PRD document cannot declare a parentDocId"));
        }
        if parent.is_some_and(|parent| parent.as_str().is_none_or(str::is_empty)) {
            return Err(invalid(
                "providedDocuments.parentDocId must be non-empty text",
            ));
        }
        intents.push((intent, doc_id, parent.and_then(Value::as_str)));
    }
    for (intent, doc_id, parent) in &intents {
        let Some(parent) = parent else {
            continue;
        };
        let parent_intent = intents
            .iter()
            .find(|(_, parent_id, _)| parent_id == parent)
            .map(|(intent, _, _)| *intent)
            .ok_or_else(|| {
                invalid("providedDocuments.parentDocId must reference a provided document")
            })?;
        let expected = if *intent == "STORY" { "DR" } else { "PRD" };
        if parent_intent != expected {
            return Err(invalid(&format!(
                "providedDocuments.parentDocId of {doc_id} must reference a {expected} document"
            )));
        }
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
    #[error("operation payload schema is invalid at {path}: {problem}")]
    NestedPayloadSchema { path: String, problem: &'static str },
    #[error("operation payload string field {0} must not be empty")]
    EmptyString(&'static str),
    #[error("operation payload array field {0} must not be empty")]
    EmptyArray(&'static str),
    #[error("operation payload field providedDocuments is invalid: {0}")]
    InvalidProvidedDocuments(String),
    #[error(
        "operation payload requestedIntent must be DR, STORY, or CODING_PLAN and requires entryNode=ROUTE: {0}"
    )]
    InvalidRequestedIntent(String),
    #[error("operation payload field docId must be a valid STORY-* identifier: {0}")]
    InvalidStoryDocumentId(String),
    #[error("lease TTL must be in 30..=3600 seconds")]
    InvalidLeaseTtl,
    #[error("failed to canonicalize operation payload: {0}")]
    CanonicalizePayload(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_operation_payload;
    use crate::OperationName;

    #[test]
    fn story_document_save_requires_a_valid_story_doc_id() {
        for (label, payload) in [
            (
                "missing",
                json!({"intent":"STORY","contentFile":"story.md"}),
            ),
            (
                "null",
                json!({"intent":"STORY","contentFile":"story.md","docId":null}),
            ),
            (
                "empty",
                json!({"intent":"STORY","contentFile":"story.md","docId":""}),
            ),
            (
                "wrong prefix",
                json!({"intent":"STORY","contentFile":"story.md","docId":"DR-001"}),
            ),
            (
                "missing suffix",
                json!({"intent":"STORY","contentFile":"story.md","docId":"STORY-"}),
            ),
        ] {
            let error = match validate_operation_payload(OperationName::DocumentSave, &payload) {
                Ok(()) => panic!("{label} Story docId must fail"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("docId"), "{label}: {error}");
        }

        validate_operation_payload(
            OperationName::DocumentSave,
            &json!({
                "intent":"STORY",
                "contentFile":"story.md",
                "docId":"STORY-ROUTE-001"
            }),
        )
        .expect("valid Story identity");
    }

    #[test]
    fn non_story_document_save_keeps_doc_id_optional() {
        for intent in ["RA", "DR", "TESTCASE", "CODING_PLAN"] {
            validate_operation_payload(
                OperationName::DocumentSave,
                &json!({"intent":intent,"contentFile":"document.md"}),
            )
            .unwrap_or_else(|error| panic!("{intent} docId remains optional: {error}"));
        }
    }

    #[test]
    fn story_admission_does_not_leak_doc_id_requirements_into_testcase() {
        validate_operation_payload(
            OperationName::DocumentSave,
            &json!({
                "intent":"STORY",
                "contentFile":"story.md",
                "docId":"STORY-ROUTE-001"
            }),
        )
        .expect("Story identity is explicit");

        validate_operation_payload(
            OperationName::DocumentSave,
            &json!({"intent":"TESTCASE","contentFile":"testcase.md"}),
        )
        .expect("TestCase remains scoped by authoritative activeStory state");
    }
}
