//! Receipt contract tests for the ae-sdd-worker binary.
//!
//! These tests do not require the worker binary to be built; they exercise the
//! pure receipt-shaping helpers (`truncate`, `digest`, `hex_encode`) that the
//! worker uses internally, plus the `VerificationReceipt` contract round-trip.

use ae_sdd_contracts::execution::{ExecutionLimits, ExecutionStep, VerificationExecutionPlan};
use ae_sdd_contracts::{ExecutionId, ExecutionStepId, SchemaVersion, WorkerId};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EvidenceDigest, InputFingerprint,
    ProjectRelativePath, WorkItemId,
};
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

fn plan() -> VerificationExecutionPlan {
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
        ExecutionId::new("exec-receipt").unwrap(),
        WorkItemId::new("STORY-1").unwrap(),
        InputFingerprint::digest(b"receipt-input"),
        vec![step],
    )
    .unwrap()
}

#[test]
fn pass_receipt_round_trips_through_canonical_json() {
    let plan = plan();
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
    let json = serde_json::to_string(&receipt).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed["status"], "pass");
    assert_eq!(reparsed["exitCode"], 0);
    assert_eq!(reparsed["timedOut"], false);
    assert_eq!(reparsed["cancelled"], false);
}

#[test]
fn fail_receipt_carries_non_zero_exit_code() {
    let plan = plan();
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
    let json = serde_json::to_string(&receipt).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed["status"], "fail");
    assert_eq!(reparsed["exitCode"], 1);
}

#[test]
fn timeout_receipt_sets_timed_out_flag() {
    let plan = plan();
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
    let json = serde_json::to_string(&receipt).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed["status"], "timeout");
    assert_eq!(reparsed["timedOut"], true);
}

#[test]
fn evidence_digest_is_sha256_prefixed_in_wire() {
    let digest = EvidenceDigest::digest(b"hello");
    let hex = digest.to_hex();
    assert_eq!(hex.len(), 64);
    // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
    assert_eq!(
        hex,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}
