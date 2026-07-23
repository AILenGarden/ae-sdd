#![allow(dead_code)]

use std::str::FromStr;

use ae_sdd_domain::{
    AgentRole, DesignRoute, EventSequence, EventStoreId, GateOutcome, InputFingerprint,
    ProcessPhase, StateRevision, WorkScale,
};
use ae_sdd_flow::{
    EventCursor, EventProvenance, FlowEnvironment, FlowEvent, FlowEventKind, FlowInput,
    FlowSnapshot, RouteSelection,
};
use ae_sdd_policy::{RequiredGate, TransitionPolicy};

pub fn event_store() -> EventStoreId {
    EventStoreId::from_str("00000000-0000-0000-0000-000000000111")
        .expect("fixed event store ID is valid")
}

pub fn other_event_store() -> EventStoreId {
    EventStoreId::from_str("00000000-0000-0000-0000-000000000222")
        .expect("fixed event store ID is valid")
}

pub fn input_with_corrections(correction_count: u64) -> FlowInput {
    input_at(ProcessPhase::Initialized, correction_count)
}

pub fn input_at(phase: ProcessPhase, correction_count: u64) -> FlowInput {
    let snapshot = FlowSnapshot::new(phase, StateRevision::new(7), correction_count);
    let environment = FlowEnvironment::new(
        event_store(),
        InputFingerprint::digest(b"work-item-input-v1"),
        RouteSelection::new(WorkScale::Large, DesignRoute::Story),
    );
    FlowInput::new(snapshot, environment)
}

pub fn input() -> FlowInput {
    input_with_corrections(0)
}

pub fn event(sequence: u64, label: &[u8], kind: FlowEventKind) -> FlowEvent {
    event_in_store(event_store(), sequence, label, kind)
}

pub fn event_in_store(
    store: EventStoreId,
    sequence: u64,
    label: &[u8],
    kind: FlowEventKind,
) -> FlowEvent {
    let provenance = EventProvenance::new(
        EventCursor::new(store, EventSequence::new(sequence)),
        TransitionPolicy::digest(),
        InputFingerprint::digest(b"work-item-input-v1"),
        InputFingerprint::digest(label),
    );
    FlowEvent::new(provenance, kind)
}

pub fn transition_request(sequence: u64, role: AgentRole) -> FlowEvent {
    transition_request_to(sequence, role, ProcessPhase::RouteSelected)
}

pub fn transition_request_to(sequence: u64, role: AgentRole, target: ProcessPhase) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::TransitionRequested {
            actor_role: role,
            target,
        },
    )
}

pub fn gate(sequence: u64, outcome: GateOutcome) -> FlowEvent {
    gate_for(sequence, RequiredGate::G00, outcome)
}

pub fn gate_for(sequence: u64, gate: RequiredGate, outcome: GateOutcome) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::GateCompleted { gate, outcome },
    )
}

pub fn commit(sequence: u64, revision: u64) -> FlowEvent {
    commit_to(sequence, revision, ProcessPhase::RouteSelected)
}

pub fn commit_to(sequence: u64, revision: u64, phase: ProcessPhase) -> FlowEvent {
    event(
        sequence,
        &sequence.to_be_bytes(),
        FlowEventKind::TransitionCommitted {
            phase,
            state_revision: StateRevision::new(revision),
        },
    )
}
