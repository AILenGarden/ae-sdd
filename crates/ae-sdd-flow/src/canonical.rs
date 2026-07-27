use std::{error::Error, fmt};

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;
use ae_sdd_contracts::series::{SeriesInput, SeriesPlanDecision};
use ae_sdd_domain::{CompletionDigestSet, CompletionMilestone};
use serde_json::{Map, Value};

use crate::ExecutionCursor;

#[derive(Debug)]
pub(crate) enum CanonicalError {
    Json(serde_json::Error),
    InvalidShape(&'static str),
}

impl From<serde_json::Error> for CanonicalError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl fmt::Display for CanonicalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "JSON encoding failed: {error}"),
            Self::InvalidShape(field) => {
                write!(formatter, "frozen Series wire shape is invalid at {field}")
            }
        }
    }
}

impl Error for CanonicalError {}

/// Encodes the compact execution cursor into the deterministic decision
/// digest: an option tag, then ordinal, queue digest, and a stable explicit
/// slice-status tag. The capsule body is never part of a flow digest.
pub(crate) fn execution_cursor(bytes: &mut Vec<u8>, cursor: Option<ExecutionCursor>) {
    match cursor {
        Some(cursor) => {
            bytes.push(1);
            bytes.extend_from_slice(&cursor.active_ordinal().to_be_bytes());
            bytes.extend_from_slice(cursor.queue_digest().as_bytes());
            bytes.push(execution_slice_status_tag(cursor.active_slice_status()));
        }
        None => bytes.push(0),
    }
}

/// Stable explicit numbering of the frozen slice-machine statuses; these tags
/// are part of the decision digest format and must never be renumbered.
const fn execution_slice_status_tag(status: ExecutionSliceStatus) -> u8 {
    match status {
        ExecutionSliceStatus::Pending => 0,
        ExecutionSliceStatus::Running => 1,
        ExecutionSliceStatus::RedObserved => 2,
        ExecutionSliceStatus::Patched => 3,
        ExecutionSliceStatus::FocusedGreen => 4,
        ExecutionSliceStatus::EvidenceBound => 5,
        ExecutionSliceStatus::Completed => 6,
        ExecutionSliceStatus::Blocked => 7,
    }
}

/// Encodes the orthogonal completion dimension into the deterministic decision
/// digest: milestone tag, the five bound input digests, and the contribution
/// marker. The evidence or review bodies never enter a flow digest.
pub(crate) fn completion(
    bytes: &mut Vec<u8>,
    milestone: CompletionMilestone,
    bound: &CompletionDigestSet,
    review_contributions_ready: bool,
) {
    bytes.push(completion_milestone_tag(milestone));
    bytes.extend_from_slice(bound.code_digest().as_bytes());
    bytes.extend_from_slice(bound.verification_digest().as_bytes());
    bytes.extend_from_slice(bound.evidence_digest().as_bytes());
    bytes.extend_from_slice(bound.review_input_digest().as_bytes());
    bytes.extend_from_slice(bound.gate_digest().as_bytes());
    bytes.push(u8::from(review_contributions_ready));
}

/// Stable explicit numbering of the completion milestones; these tags are part
/// of the decision digest format and must never be renumbered.
pub(crate) const fn completion_milestone_tag(milestone: CompletionMilestone) -> u8 {
    match milestone {
        CompletionMilestone::None => 0,
        CompletionMilestone::ImplementationVerified => 1,
        CompletionMilestone::ReviewReady => 2,
        CompletionMilestone::GovernanceClosed => 3,
    }
}

pub(crate) fn series_input(input: &SeriesInput) -> Result<Vec<u8>, CanonicalError> {
    let mut value = serde_json::to_value(input)?;
    canonicalize_series_input(&mut value)?;
    Ok(serde_json::to_vec(&value)?)
}

pub(crate) fn series_decision(decision: &SeriesPlanDecision) -> Result<Vec<u8>, CanonicalError> {
    let mut value = serde_json::to_value(decision)?;
    if let Some(plan) = value.get_mut("plan") {
        canonicalize_plan(plan)?;
    }
    Ok(serde_json::to_vec(&value)?)
}

fn canonicalize_series_input(value: &mut Value) -> Result<(), CanonicalError> {
    let object = value.as_object_mut().ok_or(CanonicalError::InvalidShape(
        "SeriesInput must encode as an object",
    ))?;
    {
        let plans = required_array(object, "candidatePlans")?;
        for plan in plans.iter_mut() {
            canonicalize_plan(plan)?;
        }
        sort_and_dedup(plans, |plan| required_string(plan, "seriesId"))?;
    }
    {
        let resolutions = required_array(object, "methodologyResolutions")?;
        sort_and_dedup(resolutions, |resolution| {
            let methodology =
                resolution
                    .get("methodologyRef")
                    .ok_or(CanonicalError::InvalidShape(
                        "MethodologyResolution lacks methodologyRef",
                    ))?;
            Ok(format!(
                "{}\0{}\0{}",
                required_string(methodology, "seriesKind")?,
                required_string(methodology, "skillId")?,
                required_string(resolution, "resolutionDigest")?
            ))
        })?;
    }
    {
        let receipts = required_array(object, "existingReceipts")?;
        sort_and_dedup(receipts, |receipt| {
            Ok(format!(
                "{}\0{}",
                required_string(receipt, "seriesId")?,
                required_string(receipt, "receiptDigest")?
            ))
        })?;
    }
    let route = required_object(object, "route")?;
    sort_string_array(route, "reasonCodes")?;
    Ok(())
}

fn canonicalize_plan(value: &mut Value) -> Result<(), CanonicalError> {
    let object = value.as_object_mut().ok_or(CanonicalError::InvalidShape(
        "SeriesPlan must encode as an object",
    ))?;
    sort_string_array(object, "allowedOperations")?;
    sort_string_array(object, "dependencyIds")?;
    sort_and_dedup(required_array(object, "allowedPaths")?, |path| {
        Ok(path.to_string())
    })?;
    let context = required_object(object, "contextRef")?;
    let artifacts = required_array(context, "artifactRefs")?;
    sort_and_dedup(artifacts, |artifact| {
        Ok(format!(
            "{}\0{}",
            required_string(artifact, "path")?,
            required_string(artifact, "digest")?
        ))
    })?;
    Ok(())
}

fn sort_string_array(
    object: &mut Map<String, Value>,
    name: &'static str,
) -> Result<(), CanonicalError> {
    sort_and_dedup(required_array(object, name)?, |value| {
        value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or(CanonicalError::InvalidShape(
                "canonical string collection contains a non-string value",
            ))
    })
}

fn sort_and_dedup(
    values: &mut Vec<Value>,
    key: impl Fn(&Value) -> Result<String, CanonicalError>,
) -> Result<(), CanonicalError> {
    let mut keyed = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        let primary = key(&value)?;
        let encoded = value.to_string();
        keyed.push((primary, encoded, value));
    }
    keyed.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    keyed.dedup_by(|left, right| left.2 == right.2);
    values.extend(keyed.into_iter().map(|(_, _, value)| value));
    Ok(())
}

fn required_string(value: &Value, name: &'static str) -> Result<String, CanonicalError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or(CanonicalError::InvalidShape(name))
}

fn required_array<'a>(
    object: &'a mut Map<String, Value>,
    name: &'static str,
) -> Result<&'a mut Vec<Value>, CanonicalError> {
    object
        .get_mut(name)
        .and_then(Value::as_array_mut)
        .ok_or(CanonicalError::InvalidShape(name))
}

fn required_object<'a>(
    object: &'a mut Map<String, Value>,
    name: &'static str,
) -> Result<&'a mut Map<String, Value>, CanonicalError> {
    object
        .get_mut(name)
        .and_then(Value::as_object_mut)
        .ok_or(CanonicalError::InvalidShape(name))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::canonicalize_series_input;

    #[test]
    fn canonicalization_rejects_a_candidate_without_a_frozen_identity() {
        let mut value = json!({
            "candidatePlans": [{"allowedOperations": []}],
            "methodologyResolutions": [],
            "existingReceipts": []
        });

        assert!(canonicalize_series_input(&mut value).is_err());
    }

    #[test]
    fn canonicalization_rejects_a_non_string_reason_code() {
        let mut value = json!({
            "candidatePlans": [],
            "methodologyResolutions": [],
            "existingReceipts": [],
            "route": {"reasonCodes": [1]}
        });

        assert!(canonicalize_series_input(&mut value).is_err());
    }
}
