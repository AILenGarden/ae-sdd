use std::sync::Arc;

use ae_sdd_domain::{
    AgentRole, DesignRoute, EventStoreId, InputFingerprint, ProcessPhase, StateRevision, WorkScale,
};
use ae_sdd_flow::{FlowEnvironment, FlowInput, FlowSnapshot, RouteSelection};
use ae_sdd_runtime::{FlowSupervisor, MemoryPersistence};
use uuid::Uuid;

fn input(store_id: EventStoreId) -> FlowInput {
    FlowInput::new(
        FlowSnapshot::new(ProcessPhase::Initialized, StateRevision::new(1), 0),
        FlowEnvironment::new(
            store_id,
            InputFingerprint::digest(b"supervisor-restart-input"),
            RouteSelection::new(WorkScale::Small, DesignRoute::CodingPlan),
        ),
    )
}

#[test]
fn durable_flow_projection_replays_to_the_same_decision_after_restart() {
    let store_id = EventStoreId::from_uuid(Uuid::from_u128(301));
    let persistence = Arc::new(MemoryPersistence::new(store_id));
    let first = FlowSupervisor::new(persistence.clone());
    let input = input(store_id);
    let pending = first
        .request_transition(
            "boot-1",
            "workspace",
            Some("session"),
            "WORK",
            "pause-request",
            input,
            AgentRole::Root,
            ProcessPhase::Paused,
        )
        .expect("transition request");
    assert_eq!(pending.pending_transition(), Some(ProcessPhase::Paused));
    first
        .record_transition_committed(
            "boot-1",
            "workspace",
            Some("session"),
            "WORK",
            "pause-commit",
            input,
            ProcessPhase::Paused,
            StateRevision::new(2),
        )
        .expect("transition commit");
    let before_restart = first
        .project("workspace", "WORK", input)
        .expect("first projection");

    let restarted = FlowSupervisor::new(persistence);
    let after_restart = restarted
        .project("workspace", "WORK", input)
        .expect("replayed projection");
    assert_eq!(after_restart.snapshot().phase(), ProcessPhase::Paused);
    assert_eq!(
        before_restart.decision_digest(),
        after_restart.decision_digest()
    );
    assert!(
        restarted
            .checkpoint("workspace", "WORK")
            .expect("checkpoint reads")
            .is_some()
    );
}
