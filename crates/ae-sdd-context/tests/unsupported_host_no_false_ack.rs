mod support;

use ae_sdd_context::{CompactCycle, CompactStatus};
use ae_sdd_domain::{CompactId, ContextGeneration};
use uuid::Uuid;

use support::session;

#[test]
fn unsupported_host_never_reports_ack_or_context_restored() {
    let mut cycle = CompactCycle::new(
        CompactId::from_uuid(Uuid::from_u128(12)),
        session(1),
        ContextGeneration::new(1),
        ContextGeneration::new(2),
        2_000,
    )
    .expect("valid cycle");
    cycle
        .mark_unsupported()
        .expect("unsupported terminal state");

    assert_eq!(cycle.status(), CompactStatus::Unsupported);
    assert_eq!(cycle.restored_projection_digest(), None);
}
