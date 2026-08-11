use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use ae_sdd_contracts::{
    ReceiptStatus, RequirementAnalysisEvidence, RouteBindingInput, RouteMappingVersion,
};
use serde_json::Value;

pub(crate) enum AuthoritativeRaPath {
    Missing,
    Invalid,
    Escape,
    Bound { relative: String, absolute: PathBuf },
}

pub(crate) fn authoritative_ra_path(root: &Path, state: &Value) -> AuthoritativeRaPath {
    let Some(relative) = state.pointer("/documentPaths/RA").and_then(Value::as_str) else {
        return AuthoritativeRaPath::Missing;
    };
    let path = Path::new(relative);
    if path.is_absolute()
        || !relative.starts_with("ae-sdd-doc/RA/")
        || !relative.ends_with(".md")
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return AuthoritativeRaPath::Escape;
    }
    let absolute = root.join(path);
    if absolute.exists()
        && !absolute
            .canonicalize()
            .is_ok_and(|canonical| canonical.starts_with(root) && canonical.is_file())
    {
        return AuthoritativeRaPath::Invalid;
    }
    AuthoritativeRaPath::Bound {
        relative: relative.to_owned(),
        absolute,
    }
}

pub(super) fn authoritative_ra_text(root: &Path, state: &Value) -> Option<String> {
    match authoritative_ra_path(root, state) {
        AuthoritativeRaPath::Bound { absolute, .. } => fs::read_to_string(absolute).ok(),
        AuthoritativeRaPath::Missing
        | AuthoritativeRaPath::Invalid
        | AuthoritativeRaPath::Escape => None,
    }
}

pub(super) fn verified_ra_evidence(state: &Value) -> Option<RequirementAnalysisEvidence> {
    let evidence: RequirementAnalysisEvidence =
        serde_json::from_value(state.pointer("/seriesReceipts/RA")?.clone()).ok()?;
    (evidence.ra_receipt_status() == ReceiptStatus::Verified).then_some(evidence)
}

pub(super) fn route_binding_input(state: &Value) -> Option<RouteBindingInput> {
    Some(RouteBindingInput::new(
        verified_ra_evidence(state)?,
        RouteMappingVersion::V1,
    ))
}
