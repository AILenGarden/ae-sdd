mod support;

use ae_sdd_domain::{AgentRole, GateOutcome};
use ae_sdd_flow::{
    FlowEventKind, FlowRuntime, SupervisorDegradation, SupervisorFault, SupervisorHealth,
};
use ae_sdd_policy::RequiredGate;

use support::{commit, event, gate_for, input, transition_request};

#[test]
fn persisted_checkpoint_resume_matches_full_replay() {
    // RA-first: the first transition (Initialized -> RequirementAnalyzed)
    // requires G-RA-1..4. Supply all four passes, then commit.
    let request = transition_request(11, AgentRole::Root);
    let passes = [
        gate_for(19, RequiredGate::GRa1, GateOutcome::Pass),
        gate_for(20, RequiredGate::GRa2, GateOutcome::Pass),
        gate_for(21, RequiredGate::GRa3, GateOutcome::Pass),
        gate_for(22, RequiredGate::GRa4, GateOutcome::Pass),
    ];
    let committed = commit(27, 8);

    let checkpoint = FlowRuntime::replay(
        input(),
        [request.clone()]
            .into_iter()
            .chain(passes.iter().cloned())
            .collect::<Vec<_>>(),
    )
    .expect("pre-restart events are valid");
    let resumed = FlowRuntime::apply(&checkpoint, &committed).expect("post-restart event is valid");
    let replayed = FlowRuntime::replay(
        input(),
        [vec![committed], passes.to_vec(), vec![request]].concat(),
    )
    .expect("full replay is valid");

    assert_eq!(resumed, replayed);
    assert_eq!(resumed.decision_digest(), replayed.decision_digest());
}

#[test]
fn background_health_events_do_not_replace_business_action() {
    let pending = FlowRuntime::replay(input(), [transition_request(1, AgentRole::Root)])
        .expect("transition request is valid");
    let action = pending.next_action().clone();
    let degraded = FlowRuntime::apply(
        &pending,
        &event(
            2,
            b"host-fault",
            FlowEventKind::BackgroundFault(SupervisorFault::HostAdapter),
        ),
    )
    .expect("background fault is a valid event");

    assert_eq!(degraded.next_action(), &action);
    assert_eq!(degraded.snapshot().correction_count(), 0);
    assert_eq!(
        degraded.health(),
        SupervisorHealth::Degraded(SupervisorDegradation::Background(
            SupervisorFault::HostAdapter
        ))
    );

    let recovered = FlowRuntime::apply(
        &degraded,
        &event(3, b"host-recovered", FlowEventKind::BackgroundRecovered),
    )
    .expect("recovery is a valid event");
    assert_eq!(recovered.health(), SupervisorHealth::Healthy);
    assert_eq!(recovered.next_action(), &action);
}
