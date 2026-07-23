//! Durable native backend for the legacy `memory.*` command family.
//!
//! The backend never writes `.ae-sdd/memory`. Every record is stored under a
//! daemon-owned namespace bound to workspace, Work Item, root session,
//! physical session, delegation, role, context generation, and scoped grant.

mod compiler;
mod input;
mod operations;
mod store;

use std::path::PathBuf;

use ae_sdd_domain::{AgentRole, ProjectPathScope, ScopedGrant};
use ae_sdd_protocol::{StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{
    BoundJobIdentity, BusinessWorkspace, DurableEvent, IdempotencyReceipt, PersistencePort,
    RuntimeError, RuntimeResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) const MEMORY_SCHEMA: &str = "ae-sdd-memory/v2";
pub(super) const MAX_ENTITY_RECORDS: usize = 256;
pub(super) const MAX_SLICE_BYTES: usize = 16 * 1024;
pub(super) const MAX_COMMON_BYTES: usize = 2 * 1024;

const MEMORY_ENTRYPOINTS: [&str; 8] = [
    "memory.clean",
    "memory.clean-all",
    "memory.common",
    "memory.create",
    "memory.read",
    "memory.search",
    "memory.summarize",
    "memory.update",
];

/// Scheduler-captured lineage fields that are not caller-controlled job
/// arguments. Role and grant are independently taken from `BusinessWorkspace`.
pub(crate) type TrustedMemoryIdentity = BoundJobIdentity;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MemoryAuthority {
    workspace_id: String,
    work_item_id: String,
    root_session_id: String,
    session_id: String,
    delegation_id: Option<String>,
    role: String,
    context_generation: u64,
    grant_fingerprint: String,
}

pub(super) struct MemoryContext<'a> {
    grant: &'a ScopedGrant,
    root: PathBuf,
    authority: MemoryAuthority,
    namespace: String,
}

pub(super) struct MutationContext<'a> {
    idempotency_key: &'a str,
    request_digest: &'a str,
}

/// Returns true only for the frozen eight-command native memory surface.
pub(crate) fn is_entrypoint(entrypoint: &str) -> bool {
    MEMORY_ENTRYPOINTS.contains(&entrypoint)
}

/// Executes one already-admitted memory job using only daemon-attested
/// identity. Callers must never populate `identity` from job payload fields.
pub(crate) fn execute(
    workspace: &BusinessWorkspace,
    work_item_id: Option<&str>,
    persistence: &dyn PersistencePort,
    identity: Option<&TrustedMemoryIdentity>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    if !is_entrypoint(entrypoint) {
        return Err(RuntimeError::new(
            StableErrorCode::OperationNotRegistered,
            "memory job entrypoint is not registered",
        ));
    }
    let identity = identity.ok_or_else(|| {
        RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "memory jobs require a daemon-attested session identity",
        )
    })?;
    let context = MemoryContext::new(workspace, work_item_id, identity)?;
    let mutation = input::is_mutation(entrypoint, arguments)?;
    if mutation
        && !matches!(
            workspace.mode,
            WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter
        )
    {
        return Err(RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "memory mutation requires daemon writer ownership",
        ));
    }
    if !mutation {
        return operations::dispatch(&context, persistence, None, entrypoint, arguments);
    }
    if identity.idempotency_key.trim().is_empty() {
        return Err(schema_error(
            "memory mutation requires the scheduler-captured idempotency key",
        ));
    }
    let request_digest = canonical_digest(&json!({
        "entrypoint":entrypoint,
        "arguments":arguments,
        "authority":context.authority,
    }))?;
    let receipt_scope = format!("memory-mutation/v1/{}", context.namespace);
    if let Some(receipt) = persistence.load_receipt(&receipt_scope, &identity.idempotency_key)? {
        if receipt.request_digest != request_digest {
            return Err(RuntimeError::new(
                StableErrorCode::IdempotencyKeyReused,
                "memory idempotency key was reused with a different payload",
            ));
        }
        let mut response: Value = serde_json::from_str(&receipt.response_json).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable memory mutation receipt is malformed",
            )
        })?;
        set_replayed(&mut response, true);
        return Ok(response);
    }
    let mutation_context = MutationContext {
        idempotency_key: &identity.idempotency_key,
        request_digest: &request_digest,
    };
    let mut response = operations::dispatch(
        &context,
        persistence,
        Some(&mutation_context),
        entrypoint,
        arguments,
    )?;
    set_replayed_default(&mut response);
    let response_json = serde_json::to_string(&response)
        .map_err(|_| schema_error("memory response could not be serialized"))?;
    let event_payload = json!({
        "entrypoint":entrypoint,
        "authorityDigest":canonical_digest(
            &serde_json::to_value(&context.authority)
                .map_err(|_| schema_error("memory authority could not be serialized"))?
        )?,
        "resultDigest":canonical_digest(&response)?,
    });
    let event = DurableEvent {
        event_store_id: persistence.event_store_id()?.to_string(),
        event_seq: 0,
        boot_id: identity.boot_id.clone(),
        kind: "memory.mutated".to_owned(),
        workspace_id: Some(context.authority.workspace_id.clone()),
        session_id: Some(context.authority.session_id.clone()),
        work_item_id: Some(context.authority.work_item_id.clone()),
        payload_digest: canonical_digest(&event_payload)?,
        payload: event_payload,
    };
    persistence.commit_event_and_receipt(
        event,
        IdempotencyReceipt {
            scope: receipt_scope,
            key: identity.idempotency_key.clone(),
            request_digest,
            response_json,
            event_seq: 0,
        },
    )?;
    Ok(response)
}

impl<'a> MemoryContext<'a> {
    fn new(
        workspace: &'a BusinessWorkspace,
        work_item_id: Option<&'a str>,
        identity: &'a TrustedMemoryIdentity,
    ) -> RuntimeResult<Self> {
        let work_item_id = work_item_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| schema_error("memory job requires trusted workItemId identity"))?;
        let role = workspace.agent_role.ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "memory job requires a daemon-verified Agent role",
            )
        })?;
        let grant = workspace.agent_grant.as_ref().ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "memory job requires a daemon-verified scoped grant",
            )
        })?;
        for (value, field, max) in [
            (identity.boot_id.as_str(), "boot", 128),
            (workspace.workspace_id.as_str(), "workspace", 128),
            (work_item_id, "workItem", 256),
            (identity.root_session_id.as_str(), "rootSession", 128),
            (identity.session_id.as_str(), "session", 128),
        ] {
            validate_identity(value, field, max)?;
        }
        match role {
            AgentRole::Root
                if identity.delegation_id.is_none()
                    && identity.root_session_id == identity.session_id => {}
            AgentRole::Root => {
                return Err(RuntimeError::new(
                    StableErrorCode::RoleOperationForbidden,
                    "trusted root memory identity has an invalid lineage binding",
                ));
            }
            AgentRole::Series | AgentRole::Task | AgentRole::Reviewer => {
                let delegation = identity.delegation_id.as_deref().ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "trusted child memory identity lacks a delegation binding",
                    )
                })?;
                validate_identity(delegation, "delegation", 128)?;
            }
        }
        let root = std::fs::canonicalize(&workspace.canonical_root).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "registered workspace root cannot be canonicalized",
            )
        })?;
        if !root.is_dir() {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "registered workspace root is not a directory",
            ));
        }
        let authority = MemoryAuthority {
            workspace_id: workspace.workspace_id.clone(),
            work_item_id: work_item_id.to_owned(),
            root_session_id: identity.root_session_id.clone(),
            session_id: identity.session_id.clone(),
            delegation_id: identity.delegation_id.clone(),
            role: role_name(role).to_owned(),
            context_generation: identity.context_generation,
            grant_fingerprint: grant_fingerprint(grant)?,
        };
        let namespace = authority.namespace()?;
        Ok(Self {
            grant,
            root,
            authority,
            namespace,
        })
    }
}

impl MemoryAuthority {
    fn namespace(&self) -> RuntimeResult<String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| schema_error("memory authority could not be canonicalized"))?;
        Ok(format!("memory/v2/{}", hex::encode(Sha256::digest(bytes))))
    }
}

fn grant_fingerprint(grant: &ScopedGrant) -> RuntimeResult<String> {
    let operations = grant
        .operations()
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    let capabilities = grant
        .capabilities()
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    let paths = grant
        .paths()
        .iter()
        .map(|value| match value {
            ProjectPathScope::ProjectRoot => json!({"kind":"project_root"}),
            ProjectPathScope::Subtree(path) => {
                json!({"kind":"subtree","path":path.as_str()})
            }
        })
        .collect::<Vec<_>>();
    canonical_digest(&json!({
        "operations":operations,
        "capabilities":capabilities,
        "paths":paths,
    }))
}

pub(super) fn canonical_digest(value: &Value) -> RuntimeResult<String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| schema_error("memory value could not be canonicalized"))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub(super) fn schema_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

fn validate_identity(value: &str, field: &str, max: usize) -> RuntimeResult<()> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(schema_error(&format!(
            "trusted {field} identity is invalid"
        )));
    }
    Ok(())
}

fn set_replayed(value: &mut Value, replayed: bool) {
    if let Some(object) = value.as_object_mut() {
        object.insert("replayed".to_owned(), Value::Bool(replayed));
    }
}

fn set_replayed_default(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object
            .entry("replayed".to_owned())
            .or_insert(Value::Bool(false));
    }
}

fn role_name(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Root => "root",
        AgentRole::Series => "series",
        AgentRole::Task => "task",
        AgentRole::Reviewer => "reviewer",
    }
}
