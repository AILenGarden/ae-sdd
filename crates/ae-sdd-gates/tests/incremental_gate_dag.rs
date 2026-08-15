//! V-EFF-008b: the incremental Gate dependency DAG must compute a stable
//! topological order, fail closed on broken declarations, and invalidate only
//! the Gates that depend on a changed input selector.

use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_gates::{
    GATE_COUNT, GateDag, GateDagError, GateDependencySpec, GateInputSelector, GateRegistry,
};

fn affected_set(dag: &GateDag, changed: &[GateInputSelector]) -> BTreeSet<&'static str> {
    dag.affected(changed).into_iter().collect()
}

#[test]
fn registry_declares_dependency_specs_for_every_gate() {
    let specs = GateRegistry::dependencies();
    assert_eq!(specs.len(), GATE_COUNT);
    let declared: BTreeSet<_> = specs.iter().map(|spec| spec.gate).collect();
    assert_eq!(declared.len(), GATE_COUNT, "declarations must be unique");
    for gate in GateRegistry::all() {
        assert!(
            declared.contains(gate.id),
            "missing dependency declaration for {}",
            gate.id
        );
    }
    for spec in specs {
        assert!(
            GateRegistry::get(spec.gate).is_some(),
            "declaration names unknown Gate {}",
            spec.gate
        );
        assert!(
            !spec.selectors.is_empty(),
            "{} must declare at least one input selector",
            spec.gate
        );
        for prerequisite in spec.prerequisites {
            assert!(
                declared.contains(prerequisite),
                "{} depends on undeclared Gate {}",
                spec.gate,
                prerequisite
            );
        }
    }
}

#[test]
fn registry_dag_builds_and_topological_order_is_stable() {
    let first = GateDag::from_registry().expect("registry DAG must be acyclic");
    let second = GateDag::from_registry().expect("registry DAG must be acyclic");
    assert_eq!(first.topological_order(), second.topological_order());
    assert_eq!(first.topological_order().len(), GATE_COUNT);

    let position: BTreeMap<_, _> = first
        .topological_order()
        .iter()
        .enumerate()
        .map(|(index, gate)| (*gate, index))
        .collect();
    for spec in GateRegistry::dependencies() {
        for prerequisite in spec.prerequisites {
            assert!(
                position[prerequisite] < position[spec.gate],
                "{prerequisite} must sort before {}",
                spec.gate
            );
        }
    }
}

#[test]
fn cyclic_declaration_fails_closed_at_startup() {
    let specs = [
        GateDependencySpec {
            gate: "G-01",
            prerequisites: &["G-02"],
            selectors: &[GateInputSelector::ProjectAssets],
        },
        GateDependencySpec {
            gate: "G-02",
            prerequisites: &["G-01"],
            selectors: &[GateInputSelector::Story],
        },
    ];
    assert!(matches!(
        GateDag::build(&specs),
        Err(GateDagError::Cycle(_))
    ));
}

#[test]
fn unknown_prerequisite_fails_closed_at_startup() {
    let specs = [GateDependencySpec {
        gate: "G-01",
        prerequisites: &["G-99"],
        selectors: &[GateInputSelector::ProjectAssets],
    }];
    assert!(matches!(
        GateDag::build(&specs),
        Err(GateDagError::UnknownPrerequisite { .. })
    ));
}

#[test]
fn duplicate_declaration_fails_closed_at_startup() {
    let spec = GateDependencySpec {
        gate: "G-01",
        prerequisites: &[],
        selectors: &[GateInputSelector::ProjectAssets],
    };
    assert!(matches!(
        GateDag::build(&[spec, spec]),
        Err(GateDagError::Duplicate(_))
    ));
}

#[test]
fn evidence_ledger_change_spares_ra_story_and_coding_plan_gates() {
    let dag = GateDag::from_registry().expect("registry DAG must be acyclic");
    let affected = affected_set(&dag, &[GateInputSelector::EvidenceLedger]);

    const UPSTREAM: &[&str] = &[
        "G-00",
        "G-RA-1",
        "G-RA-2",
        "G-RA-3",
        "G-RA-4",
        "G-RA-5",
        "G-RA-6",
        "G-RA-FLOW-VIOLATION",
        "G-01",
        "G-02",
        "G-03",
        "G-04",
        "G-05",
        "G-06",
        "G-07",
        "G-08",
        "G-14",
        "G-CODEPLAN-SRC",
        "G-HTTP-1",
        "G-DR-CTX",
        "G-STORY-CTX",
        "G-TESTCASE-CTX",
        "G-TASK-CTX",
        "G-DOC-STORAGE",
        "G-PATH",
        "G-DOC-CONSISTENCY",
        "G-09",
    ];
    for gate in UPSTREAM {
        assert!(
            !affected.contains(gate),
            "{gate} must not re-run when only the evidence ledger changes"
        );
    }
    assert!(
        affected.contains("G-10"),
        "the test-evidence Gate depends on the evidence ledger"
    );
}

/// Task 7: RA content gates (`G-RA-1..4`) must form a linear chain
/// `1 -> 2 -> 3 -> 4`, and `G-RA-FLOW-VIOLATION` must not be a descendant of
/// `G-RA-3` (no cross-phase prerequisite). A `RequirementAnalysis` input change
/// re-runs every RA gate; a `RouteBinding`-only change re-runs only
/// `G-RA-FLOW-VIOLATION`.
#[test]
fn ra_content_gates_form_linear_chain_and_flow_is_isolated() {
    let dag = GateDag::from_registry().expect("registry DAG must be acyclic");
    let order = dag.topological_order();
    let position = |id: &str| {
        order
            .iter()
            .position(|gate| *gate == id)
            .unwrap_or_else(|| panic!("{id} must be in the topological order"))
    };
    assert!(position("G-RA-1") < position("G-RA-2"));
    assert!(position("G-RA-2") < position("G-RA-3"));
    assert!(position("G-RA-3") < position("G-RA-4"));

    let ra_affected = affected_set(&dag, &[GateInputSelector::RequirementAnalysis]);
    for gate in [
        "G-RA-1",
        "G-RA-2",
        "G-RA-3",
        "G-RA-4",
        "G-RA-5",
        "G-RA-6",
        "G-RA-FLOW-VIOLATION",
    ] {
        assert!(
            ra_affected.contains(gate),
            "{gate} must re-run when RequirementAnalysis inputs change"
        );
    }

    let route_only = affected_set(&dag, &[GateInputSelector::RouteBinding]);
    assert_eq!(
        route_only,
        ["G-RA-FLOW-VIOLATION"].into_iter().collect(),
        "RouteBinding must only bust the RA -> Route binding gate"
    );
}

#[test]
fn review_batch_change_only_affects_review_and_completed_path() {
    let dag = GateDag::from_registry().expect("registry DAG must be acyclic");
    let affected = affected_set(&dag, &[GateInputSelector::ReviewBatch]);
    let expected: BTreeSet<&'static str> = [
        "G-12",
        "G-13",
        "G-REVIEW-LOOP",
        "G-09B",
        "G-REVIEW-DEPTH",
        "G-AUTO-CONSENSUS",
    ]
    .into_iter()
    .collect();
    assert_eq!(affected, expected);
}

#[test]
fn prerequisite_descendants_are_invalidated_transitively() {
    let dag = GateDag::from_registry().expect("registry DAG must be acyclic");
    let affected = affected_set(&dag, &[GateInputSelector::Story]);
    for gate in ["G-02", "G-03", "G-07", "G-08", "G-14", "G-11", "G-12"] {
        assert!(
            affected.contains(gate),
            "{gate} must re-run when the Story changes"
        );
    }
}

#[test]
fn unchanged_selectors_invalidate_nothing() {
    let dag = GateDag::from_registry().expect("registry DAG must be acyclic");
    assert!(
        dag.affected(&[]).is_empty(),
        "without a selector change every fresh Gate result must be reused"
    );
}

#[test]
fn affected_gates_are_returned_in_topological_order() {
    let dag = GateDag::from_registry().expect("registry DAG must be acyclic");
    let affected = dag.affected(&[GateInputSelector::EvidenceLedger]);
    let position: BTreeMap<_, _> = dag
        .topological_order()
        .iter()
        .enumerate()
        .map(|(index, gate)| (*gate, index))
        .collect();
    let mut ordered = affected.clone();
    ordered.sort_by_key(|gate| position[gate]);
    assert_eq!(affected, ordered);
}

#[test]
fn gate_without_selector_declaration_fails_closed_to_re_evaluation() {
    let specs = [
        GateDependencySpec {
            gate: "G-01",
            prerequisites: &[],
            selectors: &[],
        },
        GateDependencySpec {
            gate: "G-02",
            prerequisites: &[],
            selectors: &[GateInputSelector::Story],
        },
        GateDependencySpec {
            gate: "G-03",
            prerequisites: &["G-01"],
            selectors: &[GateInputSelector::Story],
        },
    ];
    let dag = GateDag::build(&specs).expect("declarations build");

    assert!(
        dag.requires_evaluation("G-01", &[]),
        "a Gate without selector declaration always re-evaluates"
    );
    assert!(!dag.requires_evaluation("G-02", &[]));
    assert!(
        dag.requires_evaluation("G-03", &[]),
        "dependents of a fail-closed Gate re-evaluate transitively"
    );
    assert!(
        dag.requires_evaluation("G-99", &[]),
        "an unknown Gate fails closed and re-evaluates"
    );
    let affected = affected_set(&dag, &[]);
    assert!(affected.contains("G-01"));
    assert!(affected.contains("G-03"));
    assert!(!affected.contains("G-02"));
}
