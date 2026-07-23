mod support;

use ae_sdd_context::{CompactCycle, CompactStatus};
use ae_sdd_domain::{CompactId, ContextDigest, ContextGeneration};
use uuid::Uuid;

use support::{artifact, compact_ack, compact_action, session};

#[test]
fn compact_reaches_context_restored_only_after_correlated_ack_and_rehydrate() {
    let compact_id = CompactId::from_uuid(Uuid::from_u128(10));
    let mut cycle = CompactCycle::new(
        compact_id,
        session(1),
        ContextGeneration::new(4),
        ContextGeneration::new(5),
        2_000,
    )
    .expect("valid cycle");
    cycle
        .snapshot_ready(artifact(".ae-sdd/compact/snapshot.json", b"snapshot"))
        .expect("snapshot ready");
    let action = compact_action(compact_id, ContextGeneration::new(4));
    cycle.dispatch(&action).expect("action dispatched");
    cycle.host_began_compacting().expect("host began");
    cycle
        .acknowledge(&compact_ack(&action))
        .expect("ACK accepted");
    assert_eq!(cycle.status(), CompactStatus::HostAcknowledged);

    assert_eq!(
        cycle
            .rehydrate(
                ContextGeneration::new(4),
                ContextDigest::digest(b"restored")
            )
            .expect("rehydrate succeeds"),
        ContextGeneration::new(5)
    );
    assert_eq!(cycle.status(), CompactStatus::ContextRestored);
}
