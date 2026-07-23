mod support;

use ae_sdd_domain::{AgentRole, GateOutcome, ProcessPhase};
use ae_sdd_flow::{FlowEventKind, FlowRuntime};
use proptest::prelude::*;

use support::{commit, event, gate, input, transition_request};

#[test]
fn reordered_complete_log_has_identical_decision_and_next_action() {
    let ordered = vec![
        transition_request(4, AgentRole::Root),
        gate(9, GateOutcome::Pass),
        commit(17, 8),
    ];
    let reordered = vec![ordered[2].clone(), ordered[0].clone(), ordered[1].clone()];

    let first = FlowRuntime::replay(input(), ordered).expect("ordered log is valid");
    let second = FlowRuntime::replay(input(), reordered).expect("reordered log is valid");

    assert_eq!(first, second);
    assert_eq!(first.snapshot().phase(), ProcessPhase::RouteSelected);
}

proptest! {
    #[test]
    fn prompt_replay_digest_is_independent_of_delivery_order(
        mut sequences in prop::collection::btree_set(1_u64..10_000, 1..64),
    ) {
        let ordered: Vec<_> = sequences
            .iter()
            .map(|sequence| event(
                *sequence,
                &sequence.to_be_bytes(),
                FlowEventKind::PromptAccepted,
            ))
            .collect();
        let mut reversed = ordered.clone();
        reversed.reverse();
        if let Some(first) = ordered.first() {
            reversed.push(first.clone());
        }

        let expected = FlowRuntime::replay(input(), ordered)
            .expect("unique ordered events are valid");
        let actual = FlowRuntime::replay(input(), reversed)
            .expect("reordered duplicates are valid");

        prop_assert_eq!(actual.decision_digest(), expected.decision_digest());
        sequences.clear();
    }
}
