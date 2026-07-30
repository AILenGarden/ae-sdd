use std::{collections::BTreeSet, sync::Arc};

use ae_sdd_domain::GateKey;
use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_gates::{
    GateFreshnessSource, GateInputError, GateInputSource, PredicateEvidence, PredicateKey,
};
use ae_sdd_scanners::{
    ScanReport, ScanRequest, ScannerEngine, ScannerId, ScannerRegistry, classify_formal_ra,
    resolve_scan_scope,
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
        let verdict = predicate_value(predicate.as_str(), &self.context, &located)
            .map_err(|_| input_error())?;
        let mut evidence: Vec<_> = state_evidence(&located).into_iter().collect();
        if let Some(denial) = verdict.denial.as_ref() {
            evidence.extend(denial.evidence(&located));
        }
        Ok(PredicateEvidence::new(verdict.satisfied, evidence))
    }

    fn scanner_report(
        &self,
        _key: &GateKey,
        scanner: ScannerId,
    ) -> Result<ScanReport, GateInputError> {
        let paths = if is_ra_scanner(scanner) {
            let located = self.context.load_state().map_err(|_| input_error())?;
            let relative = located
                .value
                .pointer("/documentPaths/RA")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| GateInputError::new(code("SCANNER_SCOPE_EMPTY"), false))?;
            let relative = ProjectRelativePath::new(relative.to_owned())
                .map_err(|_| GateInputError::new(code("SCANNER_SCOPE_FAILED"), false))?;
            if !classify_formal_ra(&relative).accepted && !is_route_ra_path(&relative) {
                return Err(GateInputError::new(code("SCANNER_SCOPE_FAILED"), false));
            }
            vec![relative]
        } else {
            let located = self.context.load_state().map_err(|_| input_error())?;
            let changed_paths: BTreeSet<_> = located
                .value
                .pointer("/executionPlan/changedPaths")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .collect();
            resolve_scan_scope(&self.context.root, ScannerRegistry::get(scanner).scope, &[])
                .map_err(|_| GateInputError::new(code("SCANNER_SCOPE_FAILED"), false))?
                .files
                .into_iter()
                .map(|(relative, _)| relative)
                .filter(|relative| {
                    changed_paths.contains(relative.as_str())
                        && !relative.as_str().starts_with("apps/ae-sdd-monitor/")
                })
                .collect()
        };
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

fn is_route_ra_path(path: &ProjectRelativePath) -> bool {
    path.as_str()
        .strip_prefix("ae-sdd-doc/RA/ROUTE-")
        .and_then(|name| name.strip_suffix(".md"))
        .is_some_and(|key| {
            key.len() == 8
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

const fn is_ra_scanner(scanner: ScannerId) -> bool {
    matches!(
        scanner,
        ScannerId::RaAuthenticity
            | ScannerId::RaFlowViolation
            | ScannerId::RaDepth
            | ScannerId::RaImplementation
    )
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
