use std::sync::Arc;

use ae_sdd_domain::{
    AgentRole, DesignRoute, EventStoreId, GateOutcome, InputFingerprint, ProcessPhase,
    StateRevision, WorkScale,
};
use ae_sdd_flow::{FlowEnvironment, FlowInput, FlowSnapshot, RouteLifecycle, RouteSelection};
use ae_sdd_policy::RequiredGate;
use ae_sdd_runtime::{FlowSupervisor, MemoryPersistence, PersistencePort};
use uuid::Uuid;

#[test]
fn same_flow_event_is_deduplicated_and_conflicting_reuse_fails_closed() {
    let store_id = EventStoreId::from_uuid(Uuid::from_u128(311));
    let persistence = Arc::new(MemoryPersistence::new(store_id));
    let supervisor = FlowSupervisor::new(persistence.clone());
    let input = FlowInput::new(
        FlowSnapshot::new(ProcessPhase::Initialized, StateRevision::new(1), 0),
        FlowEnvironment::new(
            store_id,
            InputFingerprint::digest(b"event-dedup-input"),
            RouteLifecycle::Frozen(RouteSelection::new(
                WorkScale::Small,
                DesignRoute::CodingPlan,
            )),
        ),
    );
    let first = supervisor
        .request_transition(
            "boot",
            "workspace",
            Some("session"),
            "WORK",
            "same-key",
            input,
            AgentRole::Root,
            ProcessPhase::Paused,
        )
        .expect("first event");
    let sequence = persistence
        .latest_event_sequence()
        .expect("latest sequence");
    let replay = supervisor
        .request_transition(
            "boot",
            "workspace",
            Some("session"),
            "WORK",
            "same-key",
            input,
            AgentRole::Root,
            ProcessPhase::Paused,
        )
        .expect("same payload replay");
    assert_eq!(sequence, persistence.latest_event_sequence().unwrap());
    assert_eq!(first.decision_digest(), replay.decision_digest());

    let error = supervisor
        .request_transition(
            "boot",
            "workspace",
            Some("session"),
            "WORK",
            "same-key",
            input,
            AgentRole::Root,
            ProcessPhase::RouteSelected,
        )
        .expect_err("conflicting idempotency reuse must fail");
    assert_eq!(
        error.code(),
        ae_sdd_protocol::StableErrorCode::IdempotencyKeyReused
    );

    let cross_kind = supervisor
        .record_gate(
            "boot",
            "workspace",
            Some("session"),
            "WORK",
            "same-key",
            input,
            RequiredGate::G00,
            &GateOutcome::Pass,
        )
        .expect_err("one semantic key cannot be reused across flow event kinds");
    assert_eq!(
        cross_kind.code(),
        ae_sdd_protocol::StableErrorCode::IdempotencyKeyReused
    );
}
