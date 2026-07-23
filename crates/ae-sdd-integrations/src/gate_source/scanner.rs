use std::sync::Arc;

use ae_sdd_domain::GateKey;
use ae_sdd_gates::{
    GateFreshnessSource, GateInputError, GateInputSource, PredicateEvidence, PredicateKey,
};
use ae_sdd_scanners::{
    ScanReport, ScanRequest, ScannerEngine, ScannerId, ScannerRegistry, resolve_scan_scope,
};

use super::{
    code, input_error,
    key::{GateContext, state_evidence},
    predicate::predicate_value,
};

pub(super) struct ProjectGateSource {
    pub(super) context: Arc<GateContext>,
}

impl GateInputSource for ProjectGateSource {
    fn predicate(
        &self,
        _key: &GateKey,
        predicate: PredicateKey,
    ) -> Result<PredicateEvidence, GateInputError> {
        let located = self.context.load_state().map_err(|_| input_error())?;
        let satisfied = predicate_value(
            predicate.as_str(),
            &self.context.root,
            &located.value,
            self.context.work_item_id.as_str(),
        )
        .map_err(|_| input_error())?;
        Ok(PredicateEvidence::new(
            satisfied,
            state_evidence(&located).into_iter().collect(),
        ))
    }

    fn scanner_report(
        &self,
        _key: &GateKey,
        scanner: ScannerId,
    ) -> Result<ScanReport, GateInputError> {
        let scope =
            resolve_scan_scope(&self.context.root, ScannerRegistry::get(scanner).scope, &[])
                .map_err(|_| GateInputError::new(code("SCANNER_SCOPE_FAILED"), false))?;
        let paths: Vec<_> = scope
            .files
            .into_iter()
            .map(|(relative, _)| relative)
            .filter(|relative| !relative.as_str().starts_with("apps/ae-sdd-monitor/"))
            .collect();
        if paths.is_empty() {
            return Err(GateInputError::new(code("SCANNER_SCOPE_EMPTY"), false));
        }
        ScannerEngine::scan(
            scanner,
            &ScanRequest::new(&self.context.root).explicit(paths),
        )
        .map_err(|_| GateInputError::new(code("SCANNER_EXECUTION_FAILED"), false))
    }
}

pub(super) struct ProjectGateFreshness {
    pub(super) context: Arc<GateContext>,
}

impl GateFreshnessSource for ProjectGateFreshness {
    fn current_key(&self, snapshot: &GateKey) -> Result<GateKey, GateInputError> {
        self.context
            .build_key(snapshot.gate_id().as_str(), false)
            .map_err(|_| input_error())
    }
}
