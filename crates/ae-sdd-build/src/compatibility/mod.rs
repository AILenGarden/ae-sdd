use ae_sdd_protocol::RpcMethod;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::NativeJobKind;

mod audit;

pub use audit::audit_compatibility;

pub const LEGACY_COMMAND_COUNT: usize = 113;
pub const LEGACY_OPERATION_COUNT: usize = 18;
pub const LEGACY_GATE_COUNT: usize = 36;
pub const LEGACY_SCANNER_COUNT: usize = 7;

const INVENTORY_SCHEMA: &str = "1";
const ROUTING_SCHEMA: &str = "ae-sdd-compatibility-routing/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityManifest {
    pub schema_version: String,
    pub routing_manifest: String,
    pub sources: InventorySources,
    pub commands: Vec<SurfaceEntry>,
    pub operations: Vec<SurfaceEntry>,
    pub gates: Vec<SurfaceEntry>,
    pub scanners: Vec<SurfaceEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InventorySources {
    pub cli_parser: String,
    pub operation_registry: String,
    pub gate_registry: String,
    pub scanner_registry: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceEntry {
    pub id: String,
    pub source: String,
    pub owner: String,
    pub disposition: Disposition,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    Preserve,
    Alias,
    BreakingFix,
    Replaced,
    RemovedDeprecated,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityRoutingManifest {
    pub schema_version: String,
    pub commands: Vec<CommandRoute>,
    pub capabilities: Vec<CapabilityEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandRoute {
    pub id: String,
    pub route: RouteTarget,
    pub identity: RouteIdentity,
    pub deadline_ms: u64,
    pub fail_closed: bool,
    pub fixture: String,
    pub evidence: String,
    pub status: ImplementationStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RouteTarget {
    Rpc {
        method: RpcMethod,
    },
    TypedOperation {
        operation: String,
    },
    NativeBuildJob {
        job: NativeJobKind,
        entrypoint: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteIdentity {
    pub workspace: bool,
    pub work_item: bool,
    pub session: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityEvidence {
    pub surface: CapabilitySurface,
    pub id: String,
    pub fixture: String,
    pub evidence: String,
    pub fail_closed: bool,
    pub status: ImplementationStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilitySurface {
    Operation,
    Gate,
    Scanner,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImplementationStatus {
    Implemented,
    BreakingFixVerified,
    Pending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedCounts {
    pub commands: usize,
    pub operations: usize,
    pub gates: usize,
    pub scanners: usize,
}

impl ExpectedCounts {
    #[must_use]
    pub const fn legacy() -> Self {
        Self {
            commands: LEGACY_COMMAND_COUNT,
            operations: LEGACY_OPERATION_COUNT,
            gates: LEGACY_GATE_COUNT,
            scanners: LEGACY_SCANNER_COUNT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSummary {
    pub schema_version: String,
    pub routing_schema_version: Option<String>,
    pub command_count: usize,
    pub operation_count: usize,
    pub gate_count: usize,
    pub scanner_count: usize,
    pub route_count: usize,
    pub capability_evidence_count: usize,
    pub stub_count: usize,
    pub logical_fallback_count: usize,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to read compatibility artifact {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid compatibility JSON: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("unsupported compatibility schema version {0}")]
    SchemaVersion(String),
    #[error("{surface} inventory count mismatch: expected {expected}, actual {actual}")]
    Count {
        surface: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{surface} inventory contains duplicate id {id}")]
    DuplicateId { surface: &'static str, id: String },
    #[error("{surface} inventory entry {id} has an empty {field}")]
    EmptyField {
        surface: &'static str,
        id: String,
        field: &'static str,
    },
    #[error(
        "{surface} inventory does not match the Rust registry; missing={missing:?}, extra={extra:?}"
    )]
    RegistryMismatch {
        surface: &'static str,
        missing: Vec<String>,
        extra: Vec<String>,
    },
    #[error(
        "command routing coverage differs from the command inventory; missing={missing:?}, extra={extra:?}"
    )]
    RouteCoverage {
        missing: Vec<String>,
        extra: Vec<String>,
    },
    #[error("command {id} has an invalid route target: {reason}")]
    RouteTarget { id: String, reason: String },
    #[error("legacy command semantics are not implemented: {0:?}")]
    UnimplementedRoutes(Vec<String>),
    #[error("command {id} identity contract does not match its target")]
    RouteIdentity { id: String },
    #[error("command/capability {0} is not fail-closed")]
    NotFailClosed(String),
    #[error("command {id} has an invalid deadline {deadline_ms}ms")]
    Deadline { id: String, deadline_ms: u64 },
    #[error(
        "capability evidence coverage differs from inventory; missing={missing:?}, extra={extra:?}"
    )]
    EvidenceCoverage {
        missing: Vec<String>,
        extra: Vec<String>,
    },
    #[error("compatibility evidence path is invalid, excluded, or missing: {0}")]
    EvidencePath(String),
    #[error("could not locate repository root above {0}")]
    RepositoryRoot(String),
}
