use std::{collections::BTreeMap, sync::Arc};

use ae_sdd_domain::{
    ErrorCode, EvidenceRef, FindingCode, GateError, GateFailure, GateFinding, GateKey, GateOutcome,
};
use ae_sdd_scanners::{FindingSeverity, ScanReport, ScannerId};

use crate::{CancellationToken, GateRegistry, GateSpec, NativeGateRule, PredicateKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateEvidence {
    pub satisfied: bool,
    pub evidence: Vec<EvidenceRef>,
}

impl PredicateEvidence {
    pub const fn new(satisfied: bool, evidence: Vec<EvidenceRef>) -> Self {
        Self {
            satisfied,
            evidence,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateInputError {
    code: ErrorCode,
    retryable: bool,
}

impl GateInputError {
    pub const fn new(code: ErrorCode, retryable: bool) -> Self {
        Self { code, retryable }
    }

    pub const fn code(&self) -> &ErrorCode {
        &self.code
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    fn into_outcome(self) -> GateOutcome {
        GateOutcome::Error(GateError::new(self.code, self.retryable))
    }
}

/// Inward port that obtains authoritative facts and scanner reports.
pub trait GateInputSource: Send + Sync + 'static {
    fn predicate(
        &self,
        key: &GateKey,
        predicate: PredicateKey,
    ) -> Result<PredicateEvidence, GateInputError>;

    fn scanner_report(
        &self,
        key: &GateKey,
        scanner: ScannerId,
    ) -> Result<ScanReport, GateInputError>;
}

/// Immutable evidence source useful for composition roots and deterministic tests.
#[derive(Clone, Debug, Default)]
pub struct GateEvidenceSet {
    predicates: BTreeMap<&'static str, PredicateEvidence>,
    scanners: BTreeMap<ScannerId, ScanReport>,
}

impl GateEvidenceSet {
    pub fn with_predicate(mut self, key: PredicateKey, evidence: PredicateEvidence) -> Self {
        self.predicates.insert(key.as_str(), evidence);
        self
    }

    pub fn with_scanner(mut self, report: ScanReport) -> Self {
        self.scanners.insert(report.scanner(), report);
        self
    }
}

impl GateInputSource for GateEvidenceSet {
    fn predicate(
        &self,
        _key: &GateKey,
        predicate: PredicateKey,
    ) -> Result<PredicateEvidence, GateInputError> {
        self.predicates
            .get(predicate.as_str())
            .cloned()
            .ok_or_else(|| {
                GateInputError::new(
                    ErrorCode::new("GATE_INPUT_MISSING").expect("constant error code is valid"),
                    false,
                )
            })
    }

    fn scanner_report(
        &self,
        _key: &GateKey,
        scanner: ScannerId,
    ) -> Result<ScanReport, GateInputError> {
        self.scanners.get(&scanner).cloned().ok_or_else(|| {
            GateInputError::new(
                ErrorCode::new("SCANNER_INPUT_MISSING").expect("constant error code is valid"),
                false,
            )
        })
    }
}

pub trait GateExecutor: Send + Sync + 'static {
    fn evaluate(
        &self,
        specification: &'static GateSpec,
        key: &GateKey,
        cancellation: &CancellationToken,
    ) -> GateOutcome;
}

/// Complete native evaluator. Missing facts fail closed as `ERROR`.
pub struct NativeGateExecutor<S: GateInputSource> {
    source: Arc<S>,
}

impl<S: GateInputSource> NativeGateExecutor<S> {
    pub fn new(source: Arc<S>) -> Self {
        Self { source }
    }

    pub fn evaluate_id(&self, key: &GateKey) -> GateOutcome {
        let Some(specification) = GateRegistry::get(key.gate_id().as_str()) else {
            return GateOutcome::Error(GateError::new(
                ErrorCode::new("UNKNOWN_GATE").expect("constant error code is valid"),
                false,
            ));
        };
        self.evaluate(specification, key, &CancellationToken::caller())
    }
}

impl<S: GateInputSource> GateExecutor for NativeGateExecutor<S> {
    fn evaluate(
        &self,
        specification: &'static GateSpec,
        key: &GateKey,
        cancellation: &CancellationToken,
    ) -> GateOutcome {
        if cancellation.is_cancelled() {
            return cancellation.outcome();
        }
        match specification.rule {
            NativeGateRule::Predicate(predicate) => {
                let evidence = match self.source.predicate(key, predicate) {
                    Ok(evidence) => evidence,
                    Err(error) => return error.into_outcome(),
                };
                if evidence.satisfied {
                    GateOutcome::Pass
                } else {
                    failure(format!("{}-FAILED", specification.id), evidence.evidence)
                }
            }
            NativeGateRule::Scanner(scanner) => {
                let report = match self.source.scanner_report(key, scanner) {
                    Ok(report) => report,
                    Err(error) => return error.into_outcome(),
                };
                if report.scanner() != scanner {
                    return GateOutcome::Error(GateError::new(
                        ErrorCode::new("SCANNER_REPORT_MISMATCH")
                            .expect("constant error code is valid"),
                        false,
                    ));
                }
                if report.permits_gate() {
                    GateOutcome::Pass
                } else {
                    let findings: Vec<_> = report
                        .findings()
                        .iter()
                        .filter(|finding| finding.severity == FindingSeverity::Blocker)
                        .map(|finding| {
                            GateFinding::new(
                                FindingCode::new(finding.rule.clone())
                                    .expect("scanner rule IDs satisfy FindingCode syntax"),
                                [],
                            )
                        })
                        .collect();
                    GateOutcome::Fail(
                        GateFailure::new(findings)
                            .expect("a failing scanner report has blocker findings"),
                    )
                }
            }
        }
    }
}

fn failure(code: String, evidence: Vec<EvidenceRef>) -> GateOutcome {
    GateOutcome::Fail(
        GateFailure::new([GateFinding::new(
            FindingCode::new(code).expect("Gate IDs produce valid finding codes"),
            evidence,
        )])
        .expect("one finding is non-empty"),
    )
}

#[cfg(test)]
mod tests {
    use ae_sdd_policy::GateTruth;

    use super::*;
    use crate::PredicateKey;

    #[test]
    fn missing_predicate_is_error_and_false_predicate_is_business_failure() {
        let key = crate::scheduler::tests_support::gate_key("G-14", 1);
        let missing = NativeGateExecutor::new(Arc::new(GateEvidenceSet::default()));
        assert!(matches!(missing.evaluate_id(&key), GateOutcome::Error(_)));

        let source = GateEvidenceSet::default().with_predicate(
            PredicateKey::new("coding_plan.story.aligned"),
            PredicateEvidence::new(false, Vec::new()),
        );
        let outcome = NativeGateExecutor::new(Arc::new(source)).evaluate_id(&key);
        assert!(matches!(outcome, GateOutcome::Fail(_)));
        assert_eq!(GateTruth::judge(&outcome).correction_delta(), 1);
    }
}
