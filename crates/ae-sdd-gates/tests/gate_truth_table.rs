mod support;

use std::{collections::BTreeSet, sync::Arc};

use ae_sdd_domain::GateOutcome;
use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_gates::{
    GATE_COUNT, GateEvidenceSet, GateRegistry, NativeGateExecutor, NativeGateRule,
    PredicateEvidence,
};
use ae_sdd_policy::GateTruth;
use ae_sdd_scanners::{FindingSeverity, ScanReport, ScannerFinding, ScannerId};

const EXPECTED_IDS: [&str; GATE_COUNT] = [
    "G-00",
    "G-01",
    "G-02",
    "G-03",
    "G-04",
    "G-05",
    "G-06",
    "G-07",
    "G-08",
    "G-HTTP-1",
    "G-09",
    "G-10",
    "G-11",
    "G-12",
    "G-13",
    "G-14",
    "G-CODEPLAN-SRC",
    "G-DOC-STORAGE",
    "G-PATH",
    "G-RA-1",
    "G-RA-2",
    "G-RA-3",
    "G-RA-4",
    "G-RA-FLOW-VIOLATION",
    "G-RA-5",
    "G-RA-6",
    "G-CODE-1",
    "G-DOC-CONSISTENCY",
    "G-REVIEW-LOOP",
    "G-09B",
    "G-REVIEW-DEPTH",
    "G-AUTO-CONSENSUS",
    "G-DR-CTX",
    "G-STORY-CTX",
    "G-TESTCASE-CTX",
    "G-TASK-CTX",
];

#[test]
fn registry_matches_all_36_legacy_gate_ids_without_stub_rules() {
    let actual: Vec<_> = GateRegistry::all().iter().map(|gate| gate.id).collect();
    assert_eq!(actual, EXPECTED_IDS);
    assert_eq!(actual.iter().copied().collect::<BTreeSet<_>>().len(), 36);
}

#[test]
fn native_predicate_passes_only_when_authoritative_evidence_is_true() {
    let spec = GateRegistry::get("G-14").expect("registered Gate");
    let NativeGateRule::Predicate(predicate) = spec.rule else {
        panic!("G-14 is a predicate Gate");
    };
    let key = support::gate_key("G-14", 12);

    for (satisfied, expected_pass, correction_delta) in [(true, true, 0_u64), (false, false, 1_u64)]
    {
        let source = GateEvidenceSet::default()
            .with_predicate(predicate, PredicateEvidence::new(satisfied, Vec::new()));
        let outcome = NativeGateExecutor::new(Arc::new(source)).evaluate_id(&key);
        let judgement = GateTruth::judge(&outcome);
        assert_eq!(judgement.transition_permitted(), expected_pass);
        assert_eq!(judgement.correction_delta(), correction_delta);
        assert_eq!(matches!(outcome, GateOutcome::Pass), expected_pass);
    }
}

#[test]
fn scanner_backed_gate_turns_blockers_into_business_failure() {
    let path = ProjectRelativePath::new("tests/FakeTest.java").expect("valid path");
    let report = ScanReport::new(
        ScannerId::TestAuthenticity,
        vec![path.clone()],
        vec![ScannerFinding::new(
            FindingSeverity::Blocker,
            "literal-assert-true",
            path,
            4,
            "always-pass assertion",
        )],
    );
    let source = GateEvidenceSet::default().with_scanner(report);
    let outcome =
        NativeGateExecutor::new(Arc::new(source)).evaluate_id(&support::gate_key("G-09", 12));

    assert!(matches!(outcome, GateOutcome::Fail(_)));
    assert_eq!(GateTruth::judge(&outcome).correction_delta(), 1);
}
