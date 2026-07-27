use ae_sdd_contracts::OverrideDisposition;
use ae_sdd_domain::DecisionDigest;

use crate::{OverrideAuthorization, catalog::layer_priority};

use super::{
    model::{
        RegistryCandidate, RegistryTrace, RegistryTraceReason, RegistryViolation, RegistryWinner,
    },
    selection::authorization_order,
};

pub(super) fn registry_decision_digest(
    winners: &[RegistryWinner],
    trace: &[RegistryTrace],
    violations: &[RegistryViolation],
) -> DecisionDigest {
    let mut bytes = b"ae-sdd-registry-resolution/v1".to_vec();
    append_usize(&mut bytes, winners.len());
    for winner in winners {
        append_candidate(&mut bytes, &winner.candidate);
    }
    append_usize(&mut bytes, trace.len());
    for item in trace {
        bytes.push(layer_priority(item.layer));
        append_string(&mut bytes, item.name.as_str());
        append_string(&mut bytes, item.target.as_str());
        bytes.push(disposition_tag(item.disposition));
        bytes.push(reason_tag(item.reason));
        bytes.extend_from_slice(item.source_digest.as_bytes());
        bytes.extend_from_slice(item.content_digest.as_bytes());
    }
    append_usize(&mut bytes, violations.len());
    for violation in violations {
        append_violation(&mut bytes, violation);
    }
    DecisionDigest::digest(bytes)
}

fn append_candidate(bytes: &mut Vec<u8>, candidate: &RegistryCandidate) {
    bytes.push(layer_priority(candidate.layer));
    append_string(bytes, candidate.name.as_str());
    append_string(bytes, candidate.target.as_str());
    bytes.extend_from_slice(candidate.source_digest.as_bytes());
    bytes.extend_from_slice(candidate.content_digest.as_bytes());
    bytes.push(authorization_order(
        candidate.authorization == OverrideAuthorization::Authorized,
    ));
}

fn append_violation(bytes: &mut Vec<u8>, violation: &RegistryViolation) {
    match violation {
        RegistryViolation::CandidateLimit { limit, actual } => {
            bytes.push(0);
            append_usize(bytes, *limit);
            append_usize(bytes, *actual);
        }
        RegistryViolation::Unauthorized {
            layer,
            name,
            target,
        } => {
            bytes.push(1);
            bytes.push(layer_priority(*layer));
            append_string(bytes, name.as_str());
            append_string(bytes, target.as_str());
        }
        RegistryViolation::SameLayerNameConflict { layer, name } => {
            bytes.push(2);
            bytes.push(layer_priority(*layer));
            append_string(bytes, name.as_str());
        }
        RegistryViolation::SameLayerTargetConflict { layer, target } => {
            bytes.push(3);
            bytes.push(layer_priority(*layer));
            append_string(bytes, target.as_str());
        }
    }
}

fn append_string(bytes: &mut Vec<u8>, value: &str) {
    append_usize(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn append_usize(bytes: &mut Vec<u8>, value: usize) {
    let value = u64::try_from(value).map_or(u64::MAX, |value| value);
    bytes.extend_from_slice(&value.to_le_bytes());
}

const fn disposition_tag(value: OverrideDisposition) -> u8 {
    match value {
        OverrideDisposition::Selected => 0,
        OverrideDisposition::Shadowed => 1,
        OverrideDisposition::Rejected => 2,
    }
}

const fn reason_tag(value: RegistryTraceReason) -> u8 {
    match value {
        RegistryTraceReason::Selected => 0,
        RegistryTraceReason::HigherPrioritySelected => 1,
        RegistryTraceReason::Unauthorized => 2,
        RegistryTraceReason::SameLayerNameConflict => 3,
        RegistryTraceReason::SameLayerTargetConflict => 4,
        RegistryTraceReason::ResolutionBlocked => 5,
    }
}
