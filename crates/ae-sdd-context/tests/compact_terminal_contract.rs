mod support;

use ae_sdd_context::{CompactCycle, CompactCycleError, CompactStatus};
use ae_sdd_domain::{CompactId, ContextDigest, ContextGeneration, HostAckId, HostActionId};
use ae_sdd_host::{HostAck, HostAckOutcome, HostAdapterId};
use uuid::Uuid;

use support::{artifact, compact_ack, compact_action, session};

fn compact_id(seed: u128) -> CompactId {
    CompactId::from_uuid(Uuid::from_u128(seed))
}

fn cycle(seed: u128) -> CompactCycle {
    CompactCycle::new(
        compact_id(seed),
        session(1),
        ContextGeneration::new(4),
        ContextGeneration::new(5),
        2_000,
    )
    .expect("valid compact cycle")
}

#[test]
fn cycle_constructor_and_identity_getters_enforce_generation_and_deadline() {
    assert_eq!(
        CompactCycle::new(
            compact_id(1),
            session(1),
            ContextGeneration::new(4),
            ContextGeneration::new(4),
            2_000,
        )
        .map(|_| ()),
        Err(CompactCycleError::InvalidGenerationStep)
    );
    assert_eq!(
        CompactCycle::new(
            compact_id(1),
            session(1),
            ContextGeneration::new(4),
            ContextGeneration::new(6),
            2_000,
        )
        .map(|_| ()),
        Err(CompactCycleError::InvalidGenerationStep)
    );
    assert_eq!(
        CompactCycle::new(
            compact_id(1),
            session(1),
            ContextGeneration::new(4),
            ContextGeneration::new(5),
            0,
        )
        .map(|_| ()),
        Err(CompactCycleError::InvalidDeadline)
    );

    let value = cycle(1);
    assert_eq!(value.status(), CompactStatus::PressureDetected);
    assert_eq!(value.compact_id(), compact_id(1));
    assert_eq!(value.session_id(), session(1));
    assert_eq!(value.previous_generation(), ContextGeneration::new(4));
    assert_eq!(value.next_generation(), ContextGeneration::new(5));
    assert_eq!(value.restored_projection_digest(), None);
}

#[test]
fn dispatch_and_acknowledgement_require_exact_state_and_correlation() {
    let mut value = cycle(2);
    let action = compact_action(compact_id(2), ContextGeneration::new(4));
    assert!(matches!(
        value.dispatch(&action),
        Err(CompactCycleError::InvalidTransition {
            from: CompactStatus::PressureDetected,
            expected: CompactStatus::SnapshotReady,
        })
    ));
    value
        .snapshot_ready(artifact(".ae-sdd/compact/snapshot.json", b"snapshot"))
        .expect("snapshot ready");

    let wrong_action = compact_action(compact_id(99), ContextGeneration::new(4));
    assert_eq!(
        value.dispatch(&wrong_action),
        Err(CompactCycleError::ActionCorrelationMismatch)
    );
    value.dispatch(&action).expect("matching compact action");
    value
        .host_began_compacting()
        .expect("host began compacting");

    let mismatched_ack = HostAck::new(
        HostAckId::from_uuid(Uuid::from_u128(30)),
        HostActionId::from_uuid(Uuid::from_u128(999)),
        HostAdapterId::new("codex").expect("valid adapter"),
        1,
        HostAckOutcome::Accepted,
        None,
        Some(session(1)),
    )
    .expect("well-formed mismatched ACK");
    assert_eq!(
        value.acknowledge(&mismatched_ack),
        Err(CompactCycleError::AckCorrelationMismatch)
    );
    let rejected = HostAck::new(
        HostAckId::from_uuid(Uuid::from_u128(31)),
        action.action_id(),
        action.adapter_id().clone(),
        action.command_seq(),
        HostAckOutcome::Rejected {
            error_code: "host-refused".into(),
        },
        None,
        Some(session(1)),
    )
    .expect("well-formed rejected ACK");
    assert_eq!(
        value.acknowledge(&rejected),
        Err(CompactCycleError::AckRejected)
    );
    value
        .acknowledge(&compact_ack(&action))
        .expect("matching accepted ACK");
    assert_eq!(
        value.rehydrate(
            ContextGeneration::new(5),
            ContextDigest::digest(b"restored")
        ),
        Err(CompactCycleError::GenerationCasConflict)
    );
    let restored = ContextDigest::digest(b"restored");
    assert_eq!(
        value
            .rehydrate(ContextGeneration::new(4), restored)
            .expect("generation CAS succeeds"),
        ContextGeneration::new(5)
    );
    assert_eq!(value.status(), CompactStatus::ContextRestored);
    assert_eq!(value.restored_projection_digest(), Some(restored));
    assert_eq!(
        value.mark_failed(),
        Err(CompactCycleError::InvalidTerminalTransition)
    );
}

#[test]
fn unsupported_timeout_and_failed_are_terminal_without_false_success() {
    let mut unsupported = cycle(3);
    unsupported
        .snapshot_ready(artifact(".ae-sdd/compact/snapshot.json", b"snapshot"))
        .expect("snapshot ready");
    unsupported
        .mark_unsupported()
        .expect("snapshot-ready cycle may be unsupported");
    assert_eq!(unsupported.status(), CompactStatus::Unsupported);
    assert_eq!(
        unsupported.mark_failed(),
        Err(CompactCycleError::InvalidTerminalTransition)
    );

    let mut timed_out = cycle(4);
    assert_eq!(
        timed_out.mark_timed_out(1_999),
        Err(CompactCycleError::InvalidTerminalTransition)
    );
    timed_out
        .mark_timed_out(2_000)
        .expect("deadline is inclusive");
    assert_eq!(timed_out.status(), CompactStatus::TimedOut);
    assert_eq!(
        timed_out.mark_timed_out(2_001),
        Err(CompactCycleError::InvalidTerminalTransition)
    );

    let mut failed = cycle(5);
    failed.mark_failed().expect("non-terminal cycle may fail");
    assert_eq!(failed.status(), CompactStatus::Failed);
    assert_eq!(
        failed.mark_failed(),
        Err(CompactCycleError::InvalidTerminalTransition)
    );

    let mut requested = cycle(6);
    requested
        .snapshot_ready(artifact(".ae-sdd/compact/snapshot.json", b"snapshot"))
        .expect("snapshot ready");
    let action = compact_action(compact_id(6), ContextGeneration::new(4));
    requested.dispatch(&action).expect("compact requested");
    assert_eq!(
        requested.mark_unsupported(),
        Err(CompactCycleError::InvalidTerminalTransition)
    );
}
