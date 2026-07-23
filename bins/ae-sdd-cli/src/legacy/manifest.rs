use std::collections::BTreeSet;
use std::sync::OnceLock;

use ae_sdd_protocol::RpcMethod;
use serde::Deserialize;

use super::LEGACY_COMMAND_COUNT;
use super::model::{
    ImplementationStatus, LegacyCommandRoute, LegacyRouteContract, LegacyRouteError,
    LegacyRpcAdapter, LegacyTarget, NativeJobKind,
};

const ROUTING_SCHEMA: &str = "ae-sdd-compatibility-routing/v1";
const ROUTING_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/compatibility/cli-routing.v1.json"
));

static ROUTES: OnceLock<Result<Vec<LegacyCommandRoute>, LegacyRouteError>> = OnceLock::new();

/// Parse and validate the compile-time embedded 113-route fixture once.
pub fn embedded_routes() -> Result<&'static [LegacyCommandRoute], LegacyRouteError> {
    match ROUTES.get_or_init(parse_routes) {
        Ok(routes) => Ok(routes.as_slice()),
        Err(error) => Err(error.clone()),
    }
}

fn parse_routes() -> Result<Vec<LegacyCommandRoute>, LegacyRouteError> {
    let manifest: RoutingManifest = serde_json::from_str(ROUTING_JSON)
        .map_err(|error| LegacyRouteError::InvalidManifest(error.to_string()))?;
    if manifest.schema_version != ROUTING_SCHEMA {
        return Err(invalid(format!(
            "unsupported schema {}",
            manifest.schema_version
        )));
    }
    if manifest.commands.len() != LEGACY_COMMAND_COUNT {
        return Err(invalid(format!(
            "expected {LEGACY_COMMAND_COUNT} commands, found {}",
            manifest.commands.len()
        )));
    }

    let mut ids = BTreeSet::new();
    let mut routes = Vec::with_capacity(manifest.commands.len());
    for command in manifest.commands {
        validate_command(&command, &mut ids)?;
        routes.push(convert(command));
    }
    routes.sort_by(|left, right| left.command_id.cmp(&right.command_id));
    Ok(routes)
}

fn validate_command(
    command: &ManifestCommand,
    ids: &mut BTreeSet<String>,
) -> Result<(), LegacyRouteError> {
    if command.id.is_empty()
        || command.id.trim() != command.id
        || command.id.split(' ').any(str::is_empty)
    {
        return Err(invalid(format!("invalid command id {:?}", command.id)));
    }
    if !ids.insert(command.id.clone()) {
        return Err(invalid(format!("duplicate command id {}", command.id)));
    }
    if !command.fail_closed {
        return Err(invalid(format!("{} is not fail-closed", command.id)));
    }
    if command.deadline_ms == 0 || command.deadline_ms > 600_000 {
        return Err(invalid(format!(
            "{} has invalid deadline {}",
            command.id, command.deadline_ms
        )));
    }
    if command.fixture.is_empty() || command.evidence.is_empty() {
        return Err(invalid(format!(
            "{} is missing fixture or evidence metadata",
            command.id
        )));
    }
    match &command.route {
        ManifestTarget::TypedOperation { operation } if operation.is_empty() => Err(invalid(
            format!("{} has an empty typed operation", command.id),
        )),
        ManifestTarget::NativeBuildJob { entrypoint, .. } if entrypoint.is_empty() => Err(invalid(
            format!("{} has an empty native entrypoint", command.id),
        )),
        _ => Ok(()),
    }
}

fn convert(command: ManifestCommand) -> LegacyCommandRoute {
    let command_id = command.id;
    let target = match command.route {
        ManifestTarget::Rpc {
            method: RpcMethod::JobSubmit,
        } => LegacyTarget::Rpc {
            method: RpcMethod::JobSubmit,
            adapter: LegacyRpcAdapter::JobSubmission {
                job: NativeJobKind::Admin,
                entrypoint: command_id.replace(' ', "."),
            },
        },
        ManifestTarget::Rpc { method } => LegacyTarget::Rpc {
            method,
            adapter: LegacyRpcAdapter::Passthrough,
        },
        ManifestTarget::TypedOperation { operation } => LegacyTarget::Rpc {
            method: RpcMethod::OperationExecute,
            adapter: LegacyRpcAdapter::TypedOperation { operation },
        },
        ManifestTarget::NativeBuildJob {
            job: NativeJobKind::Admin,
            entrypoint,
        } => LegacyTarget::Rpc {
            method: RpcMethod::JobSubmit,
            adapter: LegacyRpcAdapter::JobSubmission {
                job: NativeJobKind::Admin,
                entrypoint,
            },
        },
        ManifestTarget::NativeBuildJob { job, entrypoint } => {
            LegacyTarget::NativeBuildJob { job, entrypoint }
        }
        ManifestTarget::Rejected {
            stable_code,
            remediation,
        } => LegacyTarget::Rejected {
            stable_code,
            remediation,
        },
    };
    LegacyCommandRoute {
        command_id,
        identity_workspace: command.identity.workspace,
        identity_work_item: command.identity.work_item,
        identity_session: command.identity.session,
        target,
        contract: LegacyRouteContract {
            deadline_ms: command.deadline_ms,
            fail_closed: command.fail_closed,
            fixture: command.fixture,
            evidence: command.evidence,
            status: command.status,
        },
    }
}

fn invalid(reason: String) -> LegacyRouteError {
    LegacyRouteError::InvalidManifest(reason)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoutingManifest {
    schema_version: String,
    commands: Vec<ManifestCommand>,
    #[serde(rename = "capabilities")]
    _capabilities: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestCommand {
    id: String,
    route: ManifestTarget,
    identity: ManifestIdentity,
    deadline_ms: u64,
    fail_closed: bool,
    fixture: String,
    evidence: String,
    status: ImplementationStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestIdentity {
    workspace: bool,
    work_item: bool,
    session: bool,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum ManifestTarget {
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
    Rejected {
        #[serde(rename = "stableCode")]
        stable_code: String,
        remediation: String,
    },
}
