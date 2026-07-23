mod support;

use ae_sdd_context::{CompactCycle, CompactCycleError, CompactStatus};
use ae_sdd_domain::{CompactId, ContextDigest, ContextGeneration};
use uuid::Uuid;

use support::{artifact, compact_ack, compact_action, session};

#[test]
fn restored_cycle_keeps_generation_cas_across_restart() {
    let compact_id = CompactId::from_uuid(Uuid::from_u128(11));
    let mut before_restart = CompactCycle::new(
        compact_id,
        session(1),
        ContextGeneration::new(8),
        ContextGeneration::new(9),
        2_000,
    )
    .expect("valid cycle");
    before_restart
        .snapshot_ready(artifact(".ae-sdd/compact/snapshot.json", b"snapshot"))
        .expect("snapshot ready");
    let action = compact_action(compact_id, ContextGeneration::new(8));
    before_restart.dispatch(&action).expect("dispatch");
    before_restart.host_began_compacting().expect("host begins");
    before_restart
        .acknowledge(&compact_ack(&action))
        .expect("ACK");

    let mut recovered = before_restart.clone();
    assert_eq!(recovered.status(), CompactStatus::HostAcknowledged);
    assert_eq!(
        recovered.rehydrate(
            ContextGeneration::new(9),
            ContextDigest::digest(b"restored")
        ),
        Err(CompactCycleError::GenerationCasConflict)
    );
    assert_eq!(recovered.status(), CompactStatus::HostAcknowledged);
}
