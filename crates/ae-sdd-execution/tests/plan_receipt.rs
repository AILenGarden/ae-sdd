//! Plan↔receipt identity and PASS-consistency truth-table tests.

use ae_sdd_contracts::execution::{ExecutionLimits, ExecutionStep, VerificationExecutionPlan};
use ae_sdd_contracts::{ExecutionId, ExecutionStepId, SchemaVersion, WorkerId};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EvidenceDigest, InputFingerprint,
    ProjectRelativePath, WorkItemId,
};
use ae_sdd_execution::{ExecutionPolicyError, ExecutionPolicyFault, validate_against_plan};
use ae_sdd_protocol::JobStatus;

fn artifact(kind: &str, path: &str) -> ArtifactRef {
    let bytes = path.as_bytes();
    ArtifactRef::new(
        ArtifactKind::new(kind).unwrap(),
        ProjectRelativePath::new(path).unwrap(),
        ArtifactDigest::digest(bytes),
        bytes.len() as u64,
    )
}

fn plan_with_id(execution_id: &str, work_item_id: &str, input: &[u8]) -> VerificationExecutionPlan {
    let step = ExecutionStep::with_limits(
        SchemaVersion::V1,
        ExecutionStepId::new("step-1").unwrap(),
        artifact("program", "tools/cargo.exe"),
        vec![],
        None,
        vec![],
        ExecutionLimits::new(60_000, 1024, 1024).unwrap(),
    )
    .unwrap();
    VerificationExecutionPlan::new(
        SchemaVersion::V1,
        ExecutionId::new(execution_id).unwrap(),
        WorkItemId::new(work_item_id).unwrap(),
        InputFingerprint::digest(input),
        vec![step],
    )
    .unwrap()
}

#[test]
fn matching_receipt_passes_validation() {
    let plan = plan_with_id("exec-1", "STORY-1", b"input");
    let receipt = plan
        .receipt(
            WorkerId::new("worker-1").unwrap(),
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(b"stdout"),
            EvidenceDigest::digest(b"stderr"),
            1_000,
            2_000,
            false,
            false,
        )
        .unwrap();
    validate_against_plan(&plan, &receipt).expect("matching receipt validates");
}

#[test]
fn mismatched_execution_id_is_rejected() {
    let plan_a = plan_with_id("exec-1", "STORY-1", b"input");
    let plan_b = plan_with_id("exec-2", "STORY-1", b"input");
    let receipt = plan_b
        .receipt(
            WorkerId::new("worker-1").unwrap(),
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(b"out"),
            EvidenceDigest::digest(b"err"),
            1,
            2,
            false,
            false,
        )
        .unwrap();
    let err = validate_against_plan(&plan_a, &receipt).unwrap_err();
    assert_eq!(
        err,
        ExecutionPolicyError::ReceiptRejected(ExecutionPolicyFault::IdentityMismatch)
    );
}

#[test]
fn mismatched_input_fingerprint_is_rejected() {
    let plan_a = plan_with_id("exec-1", "STORY-1", b"input-a");
    let plan_b = plan_with_id("exec-1", "STORY-1", b"input-b");
    let receipt = plan_b
        .receipt(
            WorkerId::new("worker-1").unwrap(),
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(b"out"),
            EvidenceDigest::digest(b"err"),
            1,
            2,
            false,
            false,
        )
        .unwrap();
    let err = validate_against_plan(&plan_a, &receipt).unwrap_err();
    assert_eq!(
        err,
        ExecutionPolicyError::ReceiptRejected(ExecutionPolicyFault::IdentityMismatch)
    );
}

#[test]
fn fail_status_with_finding_exit_code_is_accepted_as_findings() {
    let plan = plan_with_id("exec-1", "STORY-1", b"input");
    let receipt = plan
        .receipt(
            WorkerId::new("worker-1").unwrap(),
            JobStatus::Fail,
            Some(1),
            EvidenceDigest::digest(b"out"),
            EvidenceDigest::digest(b"err"),
            1,
            2,
            false,
            false,
        )
        .unwrap();
    validate_against_plan(&plan, &receipt).expect("fail receipt validates");
}

#[test]
fn timeout_receipt_is_accepted_when_timed_out_flag_matches() {
    let plan = plan_with_id("exec-1", "STORY-1", b"input");
    let receipt = plan
        .receipt(
            WorkerId::new("worker-1").unwrap(),
            JobStatus::Timeout,
            None,
            EvidenceDigest::digest(b"out"),
            EvidenceDigest::digest(b"err"),
            1,
            2,
            true,
            false,
        )
        .unwrap();
    validate_against_plan(&plan, &receipt).expect("timeout receipt validates");
}

#[test]
fn cancelled_receipt_is_accepted_when_cancelled_flag_matches() {
    let plan = plan_with_id("exec-1", "STORY-1", b"input");
    let receipt = plan
        .receipt(
            WorkerId::new("worker-1").unwrap(),
            JobStatus::Cancelled,
            None,
            EvidenceDigest::digest(b"out"),
            EvidenceDigest::digest(b"err"),
            1,
            2,
            false,
            true,
        )
        .unwrap();
    validate_against_plan(&plan, &receipt).expect("cancelled receipt validates");
}
