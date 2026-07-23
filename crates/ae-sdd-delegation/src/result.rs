use std::collections::BTreeSet;

use ae_sdd_domain::{
    ArtifactDigest, ArtifactRef, DeliverableContract, DeliverableId, EvidenceRef, FindingCode,
    OperationId, ResultDigest,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildOutcome {
    Succeeded,
    Blocked,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildFinding {
    code: FindingCode,
    message: Box<str>,
}

impl ChildFinding {
    pub fn new(code: FindingCode, message: impl Into<Box<str>>) -> Result<Self, ChildResultError> {
        let message = message.into();
        if message.is_empty() || message.len() > 2_048 {
            return Err(ChildResultError::InvalidFindingMessage);
        }
        Ok(Self { code, message })
    }

    #[must_use]
    pub const fn code(&self) -> &FindingCode {
        &self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildDeliverable {
    id: DeliverableId,
    artifact: ArtifactRef,
}

impl ChildDeliverable {
    #[must_use]
    pub const fn new(id: DeliverableId, artifact: ArtifactRef) -> Self {
        Self { id, artifact }
    }

    #[must_use]
    pub const fn id(&self) -> &DeliverableId {
        &self.id
    }

    #[must_use]
    pub const fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildResult {
    schema_version: u16,
    outcome: ChildOutcome,
    summary: Box<str>,
    findings: Vec<ChildFinding>,
    deliverables: Vec<ChildDeliverable>,
    evidence: Vec<EvidenceRef>,
    requested_action: Option<OperationId>,
    memory_snapshot_digest: ArtifactDigest,
    canonical_bytes: u32,
    digest: ResultDigest,
}

impl ChildResult {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        outcome: ChildOutcome,
        summary: impl Into<Box<str>>,
        findings: Vec<ChildFinding>,
        deliverables: Vec<ChildDeliverable>,
        evidence: Vec<EvidenceRef>,
        requested_action: Option<OperationId>,
        memory_snapshot_digest: ArtifactDigest,
        contract: &DeliverableContract,
    ) -> Result<Self, ChildResultError> {
        let summary = summary.into();
        if summary.is_empty() {
            return Err(ChildResultError::EmptySummary);
        }
        let summary_bytes = u32::try_from(summary.len()).unwrap_or(u32::MAX);
        if summary_bytes > contract.max_summary_bytes() {
            return Err(ChildResultError::SummaryTooLarge {
                actual: summary_bytes,
                maximum: contract.max_summary_bytes(),
            });
        }
        let mut deliverable_ids = BTreeSet::new();
        for deliverable in &deliverables {
            if !deliverable_ids.insert(deliverable.id().clone()) {
                return Err(ChildResultError::DuplicateDeliverable(
                    deliverable.id().clone(),
                ));
            }
        }

        let canonical = canonical_bytes(
            outcome,
            &summary,
            &findings,
            &deliverables,
            &evidence,
            requested_action.as_ref(),
            memory_snapshot_digest,
        )?;
        let byte_length = u32::try_from(canonical.len()).unwrap_or(u32::MAX);
        if byte_length > contract.max_result_bytes() {
            return Err(ChildResultError::ResultTooLarge {
                actual: byte_length,
                maximum: contract.max_result_bytes(),
            });
        }

        Ok(Self {
            schema_version: 1,
            outcome,
            summary,
            findings,
            deliverables,
            evidence,
            requested_action,
            memory_snapshot_digest,
            canonical_bytes: byte_length,
            digest: ResultDigest::digest(canonical),
        })
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
    pub fn deliverables(&self) -> &[ChildDeliverable] {
        &self.deliverables
    }

    #[must_use]
    pub fn evidence(&self) -> &[EvidenceRef] {
        &self.evidence
    }

    #[must_use]
    pub const fn requested_action(&self) -> Option<&OperationId> {
        self.requested_action.as_ref()
    }

    #[must_use]
    pub const fn memory_snapshot_digest(&self) -> ArtifactDigest {
        self.memory_snapshot_digest
    }

    #[must_use]
    pub const fn canonical_bytes(&self) -> u32 {
        self.canonical_bytes
    }

    #[must_use]
    pub const fn digest(&self) -> ResultDigest {
        self.digest
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
}

#[derive(Debug, Error)]
pub enum ChildResultError {
    #[error("child result summary must not be empty")]
    EmptySummary,
    #[error("child result summary is {actual} bytes; maximum is {maximum}")]
    SummaryTooLarge { actual: u32, maximum: u32 },
    #[error("child result is {actual} bytes; maximum is {maximum}")]
    ResultTooLarge { actual: u32, maximum: u32 },
    #[error("child result contains duplicate deliverable {0}")]
    DuplicateDeliverable(DeliverableId),
    #[error("child finding message must be in 1..=2048 bytes")]
    InvalidFindingMessage,
    #[error("failed to canonicalize child result: {0}")]
    Canonicalize(serde_json::Error),
}

#[allow(clippy::too_many_arguments)]
fn canonical_bytes(
    outcome: ChildOutcome,
    summary: &str,
    findings: &[ChildFinding],
    deliverables: &[ChildDeliverable],
    evidence: &[EvidenceRef],
    requested_action: Option<&OperationId>,
    memory_snapshot_digest: ArtifactDigest,
) -> Result<Vec<u8>, ChildResultError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Finding<'a> {
        code: &'a str,
        message: &'a str,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Deliverable<'a> {
        id: &'a str,
        kind: &'a str,
        path: &'a str,
        digest: String,
        byte_length: u64,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Evidence<'a> {
        evidence_id: &'a str,
        verification_id: &'a str,
        path: &'a str,
        digest: String,
        byte_length: u64,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Canonical<'a> {
        schema_version: u16,
        outcome: &'static str,
        summary: &'a str,
        findings: Vec<Finding<'a>>,
        deliverables: Vec<Deliverable<'a>>,
        evidence: Vec<Evidence<'a>>,
        requested_action: Option<&'a str>,
        memory_snapshot_digest: String,
    }

    let canonical = Canonical {
        schema_version: 1,
        outcome: match outcome {
            ChildOutcome::Succeeded => "succeeded",
            ChildOutcome::Blocked => "blocked",
            ChildOutcome::Failed => "failed",
            ChildOutcome::Cancelled => "cancelled",
        },
        summary,
        findings: findings
            .iter()
            .map(|finding| Finding {
                code: finding.code().as_str(),
                message: finding.message(),
            })
            .collect(),
        deliverables: deliverables
            .iter()
            .map(|deliverable| Deliverable {
                id: deliverable.id().as_str(),
                kind: deliverable.artifact().kind().as_str(),
                path: deliverable.artifact().path().as_str(),
                digest: deliverable.artifact().digest().to_hex(),
                byte_length: deliverable.artifact().byte_length(),
            })
            .collect(),
        evidence: evidence
            .iter()
            .map(|item| Evidence {
                evidence_id: item.evidence_id().as_str(),
                verification_id: item.verification_id().as_str(),
                path: item.path().as_str(),
                digest: item.digest().to_hex(),
                byte_length: item.byte_length(),
            })
            .collect(),
        requested_action: requested_action.map(OperationId::as_str),
        memory_snapshot_digest: memory_snapshot_digest.to_hex(),
    };
    serde_json::to_vec(&canonical).map_err(ChildResultError::Canonicalize)
}

#[cfg(test)]
mod tests {
    use ae_sdd_domain::DeliverableContract;

    use super::*;

    #[test]
    fn summary_and_total_result_budgets_are_enforced() {
        let contract = DeliverableContract::new([], 256, 16).expect("valid contract");
        let too_long = ChildResult::new(
            ChildOutcome::Succeeded,
            "x".repeat(17),
            vec![],
            vec![],
            vec![],
            None,
            ArtifactDigest::digest(b"memory"),
            &contract,
        );
        assert!(matches!(
            too_long,
            Err(ChildResultError::SummaryTooLarge { .. })
        ));

        let tiny_contract = DeliverableContract::new([], 1, 1).expect("valid contract");
        let result = ChildResult::new(
            ChildOutcome::Succeeded,
            "x",
            vec![],
            vec![],
            vec![],
            None,
            ArtifactDigest::digest(b"memory"),
            &tiny_contract,
        );
        assert!(matches!(
            result,
            Err(ChildResultError::ResultTooLarge { .. })
        ));
    }
}
