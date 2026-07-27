mod support;

use ae_sdd_context::{
    CompactCoordinator, CompactCoordinatorError, CompactCoordinatorStatus, ContextCapsule,
};
use ae_sdd_contracts::{
    AdapterId, IdempotencyKey, SchemaVersion,
    compact::{CompactAck, CompactRequest, RehydrateReceipt},
    host::{AttestedAck, HostAction, HostActionBody},
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, CompactId, ContextDigest, ContextGeneration,
    HostAckId, HostActionId, InputFingerprint, ProjectRelativePath,
};
use uuid::Uuid;

use support::session;

fn artifact(path: &str, content: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("compact-capsule").expect("valid kind"),
        ProjectRelativePath::new(path).expect("valid path"),
        ArtifactDigest::digest(content),
        u64::try_from(content.len()).expect("fixture length"),
    )
}

struct Fixture {
    request: CompactRequest,
    ack: CompactAck,
    receipt: RehydrateReceipt,
    capsule: ContextCapsule,
}

fn fixture(seed: u128) -> Fixture {
    let adapter = AdapterId::new("codex-instance-a").expect("valid adapter");
    let snapshot = artifact(".ae-sdd/compact/capsule.json", b"bounded capsule");
    let restored_digest = ContextDigest::digest(b"restored projection");
    let request = CompactRequest::new(
        SchemaVersion::V1,
        CompactId::from_uuid(Uuid::from_u128(seed)),
        session(seed + 100),
        adapter.clone(),
        snapshot.clone(),
        ContextGeneration::new(4),
        ContextGeneration::new(5),
        2_000,
        IdempotencyKey::new(format!("compact-{seed}")).expect("valid idempotency key"),
    )
    .expect("valid request");
    let action = HostAction::new(
        SchemaVersion::V1,
        HostActionId::from_uuid(Uuid::from_u128(seed + 200)),
        adapter,
        1,
        InputFingerprint::digest(b"compact request"),
        2_000,
        HostActionBody::compact(request.clone()),
    )
    .expect("valid action");
    let attested = AttestedAck::accepted(
        SchemaVersion::V1,
        HostAckId::from_uuid(Uuid::from_u128(seed + 300)),
        &action,
        None,
        Some(request.session_id()),
        Some(request.previous_generation()),
        1_500,
    )
    .expect("valid attested ACK");
    let ack =
        CompactAck::new(SchemaVersion::V1, &request, &action, attested).expect("valid compact ACK");
    let receipt = RehydrateReceipt::new(
        SchemaVersion::V1,
        &request,
        &ack,
        request.next_generation(),
        restored_digest,
        1_600,
    )
    .expect("valid receipt");
    let capsule = ContextCapsule::new(snapshot, restored_digest, None).expect("bounded capsule");
    Fixture {
        request,
        ack,
        receipt,
        capsule,
    }
}

#[test]
fn only_correlated_ack_and_receipt_reach_rehydrated_and_replay_is_safe() {
    let fixture = fixture(10);
    let mut coordinator =
        CompactCoordinator::new(fixture.request, fixture.capsule).expect("valid coordinator");

    coordinator
        .acknowledge(&fixture.ack, 1_500)
        .expect("matching ACK");
    coordinator
        .acknowledge(&fixture.ack, 1_500)
        .expect("exact ACK replay");
    assert_eq!(
        coordinator.status(),
        CompactCoordinatorStatus::HostAcknowledged
    );

    coordinator
        .rehydrate(&fixture.receipt)
        .expect("matching receipt");
    coordinator
        .rehydrate(&fixture.receipt)
        .expect("exact receipt replay");
    assert_eq!(coordinator.status(), CompactCoordinatorStatus::Rehydrated);
    assert_eq!(
        coordinator.committed_generation(),
        Some(ContextGeneration::new(5))
    );
}

#[test]
fn mismatched_unsupported_and_timed_out_cycles_never_report_success() {
    let current = fixture(20);
    let other = fixture(21);
    let mut mismatched =
        CompactCoordinator::new(current.request, current.capsule).expect("valid coordinator");
    assert_eq!(
        mismatched.acknowledge(&other.ack, 1_500),
        Err(CompactCoordinatorError::CorrelationMismatch)
    );
    assert_ne!(mismatched.status(), CompactCoordinatorStatus::Rehydrated);

    let current = fixture(30);
    let mut unsupported =
        CompactCoordinator::new(current.request, current.capsule).expect("valid coordinator");
    unsupported
        .mark_unsupported()
        .expect("terminal unsupported");
    assert_eq!(unsupported.status(), CompactCoordinatorStatus::Unsupported);
    assert_eq!(unsupported.committed_generation(), None);
    assert_eq!(
        unsupported.acknowledge(&current.ack, 1_500),
        Err(CompactCoordinatorError::TerminalState)
    );

    let current = fixture(40);
    let mut timed_out =
        CompactCoordinator::new(current.request, current.capsule).expect("valid coordinator");
    timed_out.mark_timed_out(2_000).expect("terminal timeout");
    assert_eq!(timed_out.status(), CompactCoordinatorStatus::TimedOut);
    assert_eq!(timed_out.committed_generation(), None);
}

#[test]
fn capsule_budget_is_enforced_before_dispatch() {
    let oversized = ArtifactRef::new(
        ArtifactKind::new("compact-capsule").expect("valid kind"),
        ProjectRelativePath::new(".ae-sdd/compact/oversized.json").expect("valid path"),
        ArtifactDigest::digest(b"oversized"),
        ContextCapsule::MAX_BYTES + 1,
    );

    assert!(matches!(
        ContextCapsule::new(oversized, ContextDigest::digest(b"restored"), None),
        Err(CompactCoordinatorError::CapsuleBudgetExceeded { .. })
    ));
}

#[test]
fn capsule_getters_and_request_binding_are_exact() {
    let snapshot = artifact(".ae-sdd/compact/snapshot.json", b"snapshot");
    let delta = artifact(".ae-sdd/compact/delta.json", b"delta");
    let restored = ContextDigest::digest(b"restored");
    let capsule = ContextCapsule::new(snapshot.clone(), restored, Some(delta.clone()))
        .expect("bounded capsule");

    assert_eq!(capsule.snapshot_ref(), &snapshot);
    assert_eq!(capsule.restored_projection_digest(), restored);
    assert_eq!(capsule.delta_ref(), Some(&delta));
    assert_eq!(
        capsule.byte_length(),
        snapshot.byte_length() + delta.byte_length()
    );

    let current = fixture(50);
    assert!(matches!(
        CompactCoordinator::new(current.request, capsule),
        Err(CompactCoordinatorError::CapsuleRequestMismatch)
    ));

    let empty = ArtifactRef::new(
        ArtifactKind::new("compact-capsule").expect("valid kind"),
        ProjectRelativePath::new(".ae-sdd/compact/empty.json").expect("valid path"),
        ArtifactDigest::digest([]),
        0,
    );
    assert!(matches!(
        ContextCapsule::new(empty, restored, None),
        Err(CompactCoordinatorError::CapsuleBudgetExceeded { actual: 0 })
    ));
    let maximum = ArtifactRef::new(
        ArtifactKind::new("compact-capsule").expect("valid kind"),
        ProjectRelativePath::new(".ae-sdd/compact/maximum.json").expect("valid path"),
        ArtifactDigest::digest(b"maximum"),
        u64::MAX,
    );
    let one = ArtifactRef::new(
        ArtifactKind::new("compact-delta").expect("valid kind"),
        ProjectRelativePath::new(".ae-sdd/compact/one.json").expect("valid path"),
        ArtifactDigest::digest(b"one"),
        1,
    );
    assert!(matches!(
        ContextCapsule::new(maximum, restored, Some(one)),
        Err(CompactCoordinatorError::CapsuleBudgetExceeded { actual: u64::MAX })
    ));
}

#[test]
fn coordinator_rejects_late_out_of_order_and_mismatched_facts() {
    let current = fixture(60);
    let other = fixture(61);
    let mut coordinator =
        CompactCoordinator::new(current.request, current.capsule).expect("valid coordinator");

    assert_eq!(
        coordinator.rehydrate(&current.receipt),
        Err(CompactCoordinatorError::InvalidTransition)
    );
    assert_eq!(
        coordinator.acknowledge(&current.ack, 2_001),
        Err(CompactCoordinatorError::DeadlineExpired)
    );
    assert_eq!(
        coordinator.mark_timed_out(1_999),
        Err(CompactCoordinatorError::TerminalState)
    );
    coordinator
        .acknowledge(&current.ack, 1_500)
        .expect("timely correlated ACK");
    assert_eq!(
        coordinator.rehydrate(&other.receipt),
        Err(CompactCoordinatorError::CorrelationMismatch)
    );
    assert_eq!(
        coordinator.mark_unsupported(),
        Err(CompactCoordinatorError::TerminalState)
    );
    coordinator
        .rehydrate(&current.receipt)
        .expect("matching receipt");
    assert_eq!(
        coordinator.acknowledge(&current.ack, 1_500),
        Err(CompactCoordinatorError::TerminalState)
    );

    let current = fixture(70);
    let mut unsupported =
        CompactCoordinator::new(current.request, current.capsule).expect("valid coordinator");
    unsupported
        .mark_unsupported()
        .expect("unsupported terminal");
    assert_eq!(
        unsupported.mark_unsupported(),
        Err(CompactCoordinatorError::TerminalState)
    );
}
