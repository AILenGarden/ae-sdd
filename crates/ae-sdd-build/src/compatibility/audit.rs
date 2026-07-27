use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use ae_sdd_gates::GateRegistry;
use ae_sdd_operations::{OperationName, OperationSpec};
use ae_sdd_protocol::{OperationScope, RpcMethod};
use ae_sdd_scanners::ScannerRegistry;
use serde::Deserialize;

use crate::native_entrypoint;

use super::*;

impl CompatibilityManifest {
    pub fn from_path(path: &Path) -> Result<Self, ManifestError> {
        decode_file(path)
    }

    pub fn audit(&self, expected: ExpectedCounts) -> Result<AuditSummary, ManifestError> {
        audit_inventory(self, expected)?;
        Ok(AuditSummary {
            schema_version: self.schema_version.clone(),
            routing_schema_version: None,
            command_count: self.commands.len(),
            operation_count: self.operations.len(),
            gate_count: self.gates.len(),
            scanner_count: self.scanners.len(),
            route_count: 0,
            capability_evidence_count: 0,
            stub_count: 0,
            logical_fallback_count: 0,
        })
    }
}

pub fn audit_compatibility(
    manifest_path: &Path,
    expected: ExpectedCounts,
    excludes: &[PathBuf],
) -> Result<AuditSummary, ManifestError> {
    let manifest = CompatibilityManifest::from_path(manifest_path)?;
    audit_inventory(&manifest, expected)?;
    audit_authoritative_registries(&manifest)?;

    let repository_root = find_repository_root(manifest_path)?;
    let routing_relative = validate_relative_path(&manifest.routing_manifest)?;
    let routing_path = manifest_path
        .parent()
        .ok_or_else(|| ManifestError::EvidencePath(manifest.routing_manifest.clone()))?
        .join(routing_relative);
    let routing: CompatibilityRoutingManifest = decode_file(&routing_path)?;
    if routing.schema_version != ROUTING_SCHEMA {
        return Err(ManifestError::SchemaVersion(routing.schema_version));
    }

    audit_routes(&manifest, &routing, &repository_root, excludes)?;
    audit_capability_evidence(&manifest, &routing, &repository_root, excludes)?;

    Ok(AuditSummary {
        schema_version: manifest.schema_version,
        routing_schema_version: Some(routing.schema_version),
        command_count: manifest.commands.len(),
        operation_count: manifest.operations.len(),
        gate_count: manifest.gates.len(),
        scanner_count: manifest.scanners.len(),
        route_count: routing.commands.len(),
        capability_evidence_count: routing.capabilities.len(),
        stub_count: 0,
        logical_fallback_count: 0,
    })
}

fn audit_inventory(
    manifest: &CompatibilityManifest,
    expected: ExpectedCounts,
) -> Result<(), ManifestError> {
    if manifest.schema_version != INVENTORY_SCHEMA {
        return Err(ManifestError::SchemaVersion(
            manifest.schema_version.clone(),
        ));
    }
    validate_relative_path(&manifest.routing_manifest)?;
    audit_surface("commands", &manifest.commands, expected.commands)?;
    audit_surface("operations", &manifest.operations, expected.operations)?;
    audit_surface("gates", &manifest.gates, expected.gates)?;
    audit_surface("scanners", &manifest.scanners, expected.scanners)
}

fn audit_surface(
    surface: &'static str,
    entries: &[SurfaceEntry],
    expected: usize,
) -> Result<(), ManifestError> {
    if entries.len() != expected {
        return Err(ManifestError::Count {
            surface,
            expected,
            actual: entries.len(),
        });
    }
    let mut ids = BTreeSet::new();
    for entry in entries {
        for (field, value) in [
            ("id", entry.id.as_str()),
            ("source", entry.source.as_str()),
            ("owner", entry.owner.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ManifestError::EmptyField {
                    surface,
                    id: entry.id.clone(),
                    field,
                });
            }
        }
        if !ids.insert(entry.id.as_str()) {
            return Err(ManifestError::DuplicateId {
                surface,
                id: entry.id.clone(),
            });
        }
    }
    Ok(())
}

fn audit_authoritative_registries(manifest: &CompatibilityManifest) -> Result<(), ManifestError> {
    compare_registry(
        "operations",
        ids(&manifest.operations),
        OperationName::ALL
            .into_iter()
            .map(|operation| operation.as_str().to_owned())
            .collect(),
    )?;
    compare_registry(
        "gates",
        ids(&manifest.gates),
        GateRegistry::all()
            .iter()
            .map(|gate| gate.id.to_owned())
            .collect(),
    )?;
    compare_registry(
        "scanners",
        ids(&manifest.scanners),
        ScannerRegistry::all()
            .iter()
            .map(|scanner| legacy_scanner_id(scanner.legacy_source))
            .collect(),
    )
}

fn audit_routes(
    manifest: &CompatibilityManifest,
    routing: &CompatibilityRoutingManifest,
    repository_root: &Path,
    excludes: &[PathBuf],
) -> Result<(), ManifestError> {
    let inventory_ids = ids(&manifest.commands);
    let mut route_ids = BTreeSet::new();
    let mut pending = Vec::new();
    for route in &routing.commands {
        if !route_ids.insert(route.id.clone()) {
            return Err(ManifestError::DuplicateId {
                surface: "command routes",
                id: route.id.clone(),
            });
        }
        if !route.fail_closed {
            return Err(ManifestError::NotFailClosed(route.id.clone()));
        }
        if route.deadline_ms == 0 || route.deadline_ms > 600_000 {
            return Err(ManifestError::Deadline {
                id: route.id.clone(),
                deadline_ms: route.deadline_ms,
            });
        }
        validate_evidence(repository_root, &route.fixture, excludes)?;
        validate_evidence(repository_root, &route.evidence, excludes)?;
        validate_route_classification(route)?;
        if let Some(inventory) = manifest.commands.iter().find(|entry| entry.id == route.id) {
            validate_route_disposition(route, inventory.disposition)?;
        }
        if route.status == ImplementationStatus::Pending {
            pending.push(route.id.clone());
        } else {
            validate_route_target(route)?;
        }
    }
    let missing: Vec<String> = inventory_ids.difference(&route_ids).cloned().collect();
    let extra: Vec<String> = route_ids.difference(&inventory_ids).cloned().collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(ManifestError::RouteCoverage { missing, extra });
    }
    if !pending.is_empty() {
        return Err(ManifestError::UnimplementedRoutes(pending));
    }
    Ok(())
}

fn validate_route_disposition(
    route: &CommandRoute,
    disposition: Disposition,
) -> Result<(), ManifestError> {
    let rejected = matches!(route.route, RouteTarget::Rejected { .. });
    let breaking_disposition = disposition == Disposition::BreakingFix;
    let verified_breaking_fix = route.status == ImplementationStatus::BreakingFixVerified;
    if breaking_disposition != verified_breaking_fix || (rejected && !verified_breaking_fix) {
        return Err(ManifestError::RouteTarget {
            id: route.id.clone(),
            reason: "breaking-fix inventory and verified route status must match; rejected routes always require both"
                .to_owned(),
        });
    }
    Ok(())
}

fn validate_route_classification(route: &CommandRoute) -> Result<(), ManifestError> {
    let dotted = route.id.replace(' ', ".");
    if crate::B_OFFLINE_ENTRYPOINTS.contains(&dotted.as_str()) {
        let valid = matches!(
            &route.route,
            RouteTarget::NativeBuildJob {
                job: NativeJobKind::Offline,
                entrypoint,
            } if entrypoint == &dotted
        );
        if !valid {
            return Err(ManifestError::RouteTarget {
                id: route.id.clone(),
                reason: "B offline command must use its exact offline kernel entrypoint".to_owned(),
            });
        }
        if route.status != ImplementationStatus::Implemented
            || route.fixture != "crates/ae-sdd-build/tests/offline_kernels.rs"
            || route.evidence != "crates/ae-sdd-build/tests/offline_kernels.rs"
            || route.identity
                != (RouteIdentity {
                    workspace: false,
                    work_item: false,
                    session: false,
                })
        {
            return Err(ManifestError::RouteTarget {
                id: route.id.clone(),
                reason: "B offline command requires native kernel evidence with no daemon identity"
                    .to_owned(),
            });
        }
        return Ok(());
    }
    if crate::C_ADMIN_JOB_COMMANDS.contains(&route.id.as_str()) {
        if !matches!(
            route.route,
            RouteTarget::Rpc {
                method: RpcMethod::JobSubmit
            }
        ) {
            return Err(ManifestError::RouteTarget {
                id: route.id.clone(),
                reason: "C admin command must use job.submit".to_owned(),
            });
        }
        return Ok(());
    }
    if crate::D_REJECTED_COMMANDS.contains(&route.id.as_str()) {
        if !matches!(route.route, RouteTarget::Rejected { .. }) {
            return Err(ManifestError::RouteTarget {
                id: route.id.clone(),
                reason: "D command must use explicit rejected route".to_owned(),
            });
        }
        return Ok(());
    }
    if matches!(
        route.route,
        RouteTarget::NativeBuildJob { .. } | RouteTarget::Rejected { .. }
    ) {
        return Err(ManifestError::RouteTarget {
            id: route.id.clone(),
            reason: "A daemon command cannot use an offline/rejected route".to_owned(),
        });
    }
    Ok(())
}

fn validate_route_target(route: &CommandRoute) -> Result<(), ManifestError> {
    match &route.route {
        RouteTarget::Rpc { method } => {
            let spec = method.spec();
            let session_required = matches!(
                spec.scope,
                OperationScope::WorkItem | OperationScope::Delegation
            ) || (spec.scope == OperationScope::Session
                && *method != RpcMethod::SessionOpen);
            if (spec.requirements.requires_workspace && !route.identity.workspace)
                || (spec.requirements.requires_work_item && !route.identity.work_item)
                || (session_required && !route.identity.session)
            {
                return Err(ManifestError::RouteIdentity {
                    id: route.id.clone(),
                });
            }
            if matches!(
                method,
                RpcMethod::HookUserPrompt
                    | RpcMethod::HookPreTool
                    | RpcMethod::HookPostTool
                    | RpcMethod::HookStop
            ) && route.deadline_ms > 250
            {
                return Err(ManifestError::Deadline {
                    id: route.id.clone(),
                    deadline_ms: route.deadline_ms,
                });
            }
        }
        RouteTarget::TypedOperation { operation } => {
            let operation =
                OperationName::from_str(operation).map_err(|error| ManifestError::RouteTarget {
                    id: route.id.clone(),
                    reason: error.to_string(),
                })?;
            validate_operation_identity(route, operation.spec())?;
        }
        RouteTarget::NativeBuildJob { job, entrypoint } => {
            let expected = route.id.replace(' ', ".");
            let registered = if *job == NativeJobKind::Offline {
                crate::B_OFFLINE_ENTRYPOINTS.contains(&entrypoint.as_str())
            } else {
                native_entrypoint(entrypoint).is_some_and(|spec| spec.kind == *job)
            };
            if entrypoint != &expected || route.deadline_ms < 1_000 || !registered {
                return Err(ManifestError::RouteTarget {
                    id: route.id.clone(),
                    reason: format!(
                        "{} job entrypoint must be exact ({expected}) with a >=1000ms deadline",
                        job.as_str()
                    ),
                });
            }
        }
        RouteTarget::Rejected {
            stable_code,
            remediation,
        } => {
            if !matches!(
                stable_code.as_str(),
                "LEGACY_COMMAND_REMOVED" | "LEGACY_UNTYPED_MUTATION_REMOVED"
            ) || remediation.trim().is_empty()
            {
                return Err(ManifestError::RouteTarget {
                    id: route.id.clone(),
                    reason: "rejected route needs a registered stable removal code and remediation"
                        .to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn validate_operation_identity(
    route: &CommandRoute,
    operation: &OperationSpec,
) -> Result<(), ManifestError> {
    let session_required = operation.operation != OperationName::LeaseBreak;
    if route.identity.workspace != operation.requires_workspace
        || route.identity.work_item != operation.requires_work_item
        || route.identity.session != session_required
    {
        return Err(ManifestError::RouteIdentity {
            id: route.id.clone(),
        });
    }
    Ok(())
}

fn audit_capability_evidence(
    manifest: &CompatibilityManifest,
    routing: &CompatibilityRoutingManifest,
    repository_root: &Path,
    excludes: &[PathBuf],
) -> Result<(), ManifestError> {
    let expected: BTreeSet<_> = manifest
        .operations
        .iter()
        .map(|entry| (CapabilitySurface::Operation, entry.id.clone()))
        .chain(
            manifest
                .gates
                .iter()
                .map(|entry| (CapabilitySurface::Gate, entry.id.clone())),
        )
        .chain(
            manifest
                .scanners
                .iter()
                .map(|entry| (CapabilitySurface::Scanner, entry.id.clone())),
        )
        .collect();
    let mut actual = BTreeSet::new();
    let mut pending = Vec::new();
    for evidence in &routing.capabilities {
        if !actual.insert((evidence.surface, evidence.id.clone())) {
            return Err(ManifestError::DuplicateId {
                surface: "capability evidence",
                id: format!("{:?}:{}", evidence.surface, evidence.id),
            });
        }
        if !evidence.fail_closed {
            return Err(ManifestError::NotFailClosed(evidence.id.clone()));
        }
        validate_evidence(repository_root, &evidence.fixture, excludes)?;
        validate_evidence(repository_root, &evidence.evidence, excludes)?;
        if let Some(inventory) = capability_inventory_entry(manifest, evidence) {
            validate_capability_disposition(evidence, inventory.disposition)?;
        }
        if evidence.status == ImplementationStatus::Pending {
            pending.push(format!("{:?}:{}", evidence.surface, evidence.id));
        }
    }
    let render = |entries: BTreeSet<(CapabilitySurface, String)>| {
        entries
            .into_iter()
            .map(|(surface, id)| format!("{surface:?}:{id}"))
            .collect()
    };
    let missing: Vec<String> = render(expected.difference(&actual).cloned().collect());
    let extra: Vec<String> = render(actual.difference(&expected).cloned().collect());
    if !missing.is_empty() || !extra.is_empty() {
        return Err(ManifestError::EvidenceCoverage { missing, extra });
    }
    if !pending.is_empty() {
        return Err(ManifestError::UnimplementedCapabilities(pending));
    }
    Ok(())
}

fn capability_inventory_entry<'a>(
    manifest: &'a CompatibilityManifest,
    evidence: &CapabilityEvidence,
) -> Option<&'a SurfaceEntry> {
    let entries = match evidence.surface {
        CapabilitySurface::Operation => &manifest.operations,
        CapabilitySurface::Gate => &manifest.gates,
        CapabilitySurface::Scanner => &manifest.scanners,
    };
    entries.iter().find(|entry| entry.id == evidence.id)
}

fn validate_capability_disposition(
    evidence: &CapabilityEvidence,
    disposition: Disposition,
) -> Result<(), ManifestError> {
    let breaking_disposition = disposition == Disposition::BreakingFix;
    let verified_breaking_fix = evidence.status == ImplementationStatus::BreakingFixVerified;
    if breaking_disposition != verified_breaking_fix {
        return Err(ManifestError::CapabilityStatus {
            id: format!("{:?}:{}", evidence.surface, evidence.id),
            reason: "breaking-fix inventory and verified capability status must match".to_owned(),
        });
    }
    Ok(())
}

fn compare_registry(
    surface: &'static str,
    inventory: BTreeSet<String>,
    registry: BTreeSet<String>,
) -> Result<(), ManifestError> {
    let missing: Vec<String> = registry.difference(&inventory).cloned().collect();
    let extra: Vec<String> = inventory.difference(&registry).cloned().collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(ManifestError::RegistryMismatch {
            surface,
            missing,
            extra,
        });
    }
    Ok(())
}

fn ids(entries: &[SurfaceEntry]) -> BTreeSet<String> {
    entries.iter().map(|entry| entry.id.clone()).collect()
}

fn legacy_scanner_id(source: &str) -> String {
    Path::new(source)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(source)
        .replace('_', "-")
}

fn validate_evidence(
    repository_root: &Path,
    value: &str,
    excludes: &[PathBuf],
) -> Result<(), ManifestError> {
    let relative = validate_relative_path(value)?;
    if excluded(relative, excludes) {
        return Err(ManifestError::EvidencePath(value.to_owned()));
    }
    let path = repository_root.join(relative);
    if !path.is_file() {
        return Err(ManifestError::EvidencePath(value.to_owned()));
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<&Path, ManifestError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ManifestError::EvidencePath(value.to_owned()));
    }
    Ok(path)
}

fn excluded(relative: &Path, excludes: &[PathBuf]) -> bool {
    excludes.iter().any(|exclude| {
        let value = exclude.to_string_lossy().replace('\\', "/");
        let prefix = value.strip_suffix("/**").unwrap_or(&value);
        let prefix = Path::new(prefix);
        relative == prefix || relative.starts_with(prefix)
    })
}

fn find_repository_root(path: &Path) -> Result<PathBuf, ManifestError> {
    let canonical = path.canonicalize().map_err(|source| ManifestError::Read {
        path: path.display().to_string(),
        source,
    })?;
    for ancestor in canonical.ancestors().skip(1) {
        if ancestor.join("Cargo.toml").is_file() {
            return Ok(ancestor.to_path_buf());
        }
    }
    Err(ManifestError::RepositoryRoot(path.display().to_string()))
}

fn decode_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ManifestError> {
    let bytes = std::fs::read(path).map_err(|source| ManifestError::Read {
        path: path.display().to_string(),
        source,
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests;
