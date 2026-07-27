use ae_sdd_contracts::{
    ConfirmationRequirement, ControlPlaneError, ControlPlaneErrorCode, EventIntent,
    LifecycleDisposition, LifecycleInput, LifecyclePlan, LogicalKey, LogicalNamespace, MessageKey,
    MutationIntent, MutationIntentId, MutationOperation, MutationTarget, ReasonCode, Remediation,
    RetryClass, SchemaVersion,
};
use ae_sdd_domain::{ArtifactDigest, DecisionDigest};

use crate::{
    canonical,
    projection::{ConfirmationProjection, LifecycleProjection},
    validation::{self, AuthorizedCommand, TargetSpec},
};

/// Maximum requested cooperative file-lock lifetime accepted by the pure planner.
pub const MAX_FILE_LOCK_TTL_MS: u64 = 86_400_000;

/// Maximum UTF-8 byte length accepted for a raw or `sha256:` confirmation digest.
pub const MAX_CONFIRMATION_ID_BYTES: usize = 71;

/// Maximum UTF-8 byte length accepted for the asserted confirmation actor.
pub const MAX_CONFIRMATION_APPROVED_BY_BYTES: usize = 256;

/// Maximum UTF-8 byte length accepted for a canonical UTC RFC3339 timestamp.
pub const MAX_CONFIRMATION_APPROVED_AT_BYTES: usize = 64;

/// Stateless, deterministic Work Item lifecycle planner.
#[derive(Clone, Copy, Debug, Default)]
pub struct LifecycleEngine;

impl LifecycleEngine {
    /// Plans one typed lifecycle command without reading or mutating external state.
    ///
    /// Semantic policy failures are returned as a `Denied` plan with no mutation
    /// intents. A protected but otherwise valid command without approval returns
    /// `AwaitingConfirmation`. Structurally invalid confirmation binding or an
    /// impossible frozen-contract projection returns a stable control-plane error.
    ///
    /// # File-lock trust boundary
    ///
    /// `owner_session_id` in a file-lock command is a trusted adapter assertion,
    /// not an identity authenticated by this pure engine. Part A only compares
    /// that asserted value with the authoritative owner stored in the supplied
    /// snapshot. The C1 adapter must bind the assertion to the authenticated
    /// caller before invoking this planner.
    pub fn plan(input: &LifecycleInput) -> Result<LifecyclePlan, ControlPlaneError> {
        let projection = LifecycleProjection::from_input(input);
        let binding = canonical::action_binding(input, &projection);

        let authorized = match validation::authorize(input, &projection) {
            Ok(authorized) => authorized,
            Err(validation::ValidationFailure::Denied(remediation)) => {
                return build_plan(
                    input,
                    &projection,
                    binding,
                    LifecycleDisposition::Denied,
                    Vec::new(),
                    ConfirmationRequirement::not_required(binding),
                    remediation,
                );
            }
            Err(validation::ValidationFailure::Contract(error)) => return Err(error),
        };
        if let Err(failure) = validation::validate_evidence(&authorized, &projection) {
            match failure {
                validation::ValidationFailure::Denied(remediation) => {
                    return build_plan(
                        input,
                        &projection,
                        binding,
                        LifecycleDisposition::Denied,
                        Vec::new(),
                        ConfirmationRequirement::not_required(binding),
                        remediation,
                    );
                }
                validation::ValidationFailure::Contract(error) => return Err(error),
            }
        }

        if authorized.confirmation_required {
            match confirmation_status(binding, &projection.confirmations)? {
                ConfirmationStatus::Missing => {
                    let reason = reason("confirmation.lifecycle-protected")?;
                    return build_plan(
                        input,
                        &projection,
                        binding,
                        LifecycleDisposition::AwaitingConfirmation,
                        Vec::new(),
                        ConfirmationRequirement::required(reason, binding),
                        vec![remediation(
                            "confirmation.provide-binding",
                            "confirmation.provide-binding",
                        )?],
                    );
                }
                ConfirmationStatus::Bound => {}
            }
        } else if !projection.confirmations.is_empty() {
            confirmation_status(binding, &projection.confirmations)?;
        }

        let intents = build_intents(input, binding, &authorized)?;
        build_plan(
            input,
            &projection,
            binding,
            LifecycleDisposition::Permitted,
            intents,
            ConfirmationRequirement::not_required(binding),
            Vec::new(),
        )
    }
}

enum ConfirmationStatus {
    Missing,
    Bound,
}

fn confirmation_status(
    binding: DecisionDigest,
    confirmations: &[ConfirmationProjection],
) -> Result<ConfirmationStatus, ControlPlaneError> {
    if confirmations.is_empty() {
        return Ok(ConfirmationStatus::Missing);
    }
    if confirmations.len() != 1 {
        return Err(confirmation_mismatch(binding));
    }
    let confirmation = &confirmations[0];
    let expected = binding.to_string();
    let matches_binding = confirmation.confirmation_id == expected
        || confirmation.confirmation_id == format!("sha256:{expected}");
    if !valid_confirmation_text(&confirmation.confirmation_id, MAX_CONFIRMATION_ID_BYTES)
        || !matches_binding
        || !valid_confirmation_text(
            &confirmation.approved_by,
            MAX_CONFIRMATION_APPROVED_BY_BYTES,
        )
        || !canonical_confirmation_timestamp(&confirmation.approved_at)
    {
        return Err(confirmation_mismatch(binding));
    }
    Ok(ConfirmationStatus::Bound)
}

fn valid_confirmation_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn canonical_confirmation_timestamp(value: &str) -> bool {
    if !valid_confirmation_text(value, MAX_CONFIRMATION_APPROVED_AT_BYTES) {
        return false;
    }
    match value.parse::<jiff::Timestamp>() {
        Ok(timestamp) => timestamp.to_string() == value,
        Err(_) => false,
    }
}

fn build_intents(
    input: &LifecycleInput,
    binding: DecisionDigest,
    authorized: &AuthorizedCommand,
) -> Result<Vec<MutationIntent>, ControlPlaneError> {
    let event_kind = reason(authorized.event_kind)?;
    let event_payload = canonical::event_payload_digest(binding, authorized.event_kind);
    let event = EventIntent::new(event_kind, event_payload);
    let expected_revision = input.snapshot().state_revision;
    let primary = MutationIntent::new(
        SchemaVersion::V1,
        intent_id(binding, 0)?,
        mutation_target(input, &authorized.target)?,
        authorized.operation,
        expected_revision,
        authorized.expected_digest,
        event.clone(),
    );
    let journal = MutationIntent::new(
        SchemaVersion::V1,
        intent_id(binding, 1)?,
        MutationTarget::logical_record(
            namespace("runtime-event")?,
            logical_key(&binding.to_string())?,
        ),
        MutationOperation::AppendEvent,
        expected_revision,
        None,
        event,
    );
    Ok(vec![primary, journal])
}

fn mutation_target(
    input: &LifecycleInput,
    target: &TargetSpec,
) -> Result<MutationTarget, ControlPlaneError> {
    match target {
        TargetSpec::WorkItem => Ok(MutationTarget::logical_record(
            namespace("work-item-state")?,
            logical_key(input.snapshot().work_item_id.as_str())?,
        )),
        TargetSpec::Story(story_id) => Ok(MutationTarget::logical_record(
            namespace("work-item-story")?,
            logical_key(story_id.as_str())?,
        )),
        TargetSpec::Prd(prd_id) => Ok(MutationTarget::logical_record(
            namespace("work-item-prd")?,
            logical_key(prd_id.as_str())?,
        )),
        TargetSpec::File(path) => Ok(MutationTarget::project_file(
            namespace("work-item-file-lock")?,
            path.clone(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_plan(
    input: &LifecycleInput,
    projection: &LifecycleProjection,
    binding: DecisionDigest,
    disposition: LifecycleDisposition,
    intents: Vec<MutationIntent>,
    confirmation: ConfirmationRequirement,
    remediation: Vec<Remediation>,
) -> Result<LifecyclePlan, ControlPlaneError> {
    let digest = canonical::plan_digest(
        binding,
        disposition,
        &intents,
        &confirmation,
        &projection.confirmations,
        &remediation,
    );
    LifecyclePlan::new(
        SchemaVersion::V1,
        disposition,
        intents,
        input.snapshot().state_revision,
        confirmation,
        digest,
        remediation,
    )
    .map_err(|_| contract_error("lifecycle.plan-contract-invalid"))
}

fn intent_id(binding: DecisionDigest, ordinal: u8) -> Result<MutationIntentId, ControlPlaneError> {
    MutationIntentId::new(format!("lifecycle.{binding}.{ordinal}"))
        .map_err(|_| contract_error("lifecycle.intent-id-invalid"))
}

fn namespace(value: &str) -> Result<LogicalNamespace, ControlPlaneError> {
    LogicalNamespace::new(value).map_err(|_| contract_error("lifecycle.namespace-invalid"))
}

fn logical_key(value: &str) -> Result<LogicalKey, ControlPlaneError> {
    LogicalKey::new(value).map_err(|_| contract_error("lifecycle.logical-key-invalid"))
}

fn reason(value: &str) -> Result<ReasonCode, ControlPlaneError> {
    ReasonCode::new(value).map_err(|_| invariant_error(None))
}

fn remediation(code: &str, key: &str) -> Result<Remediation, ControlPlaneError> {
    Ok(Remediation {
        code: reason(code)?,
        message_key: message_key(key)?,
    })
}

fn confirmation_mismatch(binding: DecisionDigest) -> ControlPlaneError {
    let message_key = match message_key("lifecycle.confirmation-mismatch") {
        Ok(message_key) => message_key,
        Err(error) => return error,
    };
    let remediation = match remediation(
        "confirmation.refresh-binding",
        "confirmation.refresh-binding",
    ) {
        Ok(remediation) => remediation,
        Err(error) => return error,
    };
    ControlPlaneError {
        schema_version: SchemaVersion::V1,
        code: ControlPlaneErrorCode::ConfirmationMismatch,
        retry: RetryClass::AfterUserAction,
        message_key,
        remediation: vec![remediation],
        details_digest: Some(ArtifactDigest::from_array(binding.into_array())),
    }
}

fn contract_error(key: &str) -> ControlPlaneError {
    let message_key = match message_key(key) {
        Ok(message_key) => message_key,
        Err(error) => return error,
    };
    let remediation = match remediation("lifecycle.refresh-input", "lifecycle.refresh-input") {
        Ok(remediation) => remediation,
        Err(error) => return error,
    };
    ControlPlaneError {
        schema_version: SchemaVersion::V1,
        code: ControlPlaneErrorCode::ContractValidationFailed,
        retry: RetryClass::AfterInputRepair,
        message_key,
        remediation: vec![remediation],
        details_digest: None,
    }
}

fn message_key(value: &str) -> Result<MessageKey, ControlPlaneError> {
    MessageKey::new(value).map_err(|_| invariant_error(None))
}

pub(crate) fn invariant_error(details_digest: Option<ArtifactDigest>) -> ControlPlaneError {
    ControlPlaneError {
        schema_version: SchemaVersion::V1,
        code: ControlPlaneErrorCode::ContractValidationFailed,
        retry: RetryClass::AfterInputRepair,
        message_key: MessageKey::invariant_fallback(),
        remediation: vec![Remediation {
            code: ReasonCode::invariant_fallback(),
            message_key: MessageKey::invariant_fallback(),
        }],
        details_digest,
    }
}
