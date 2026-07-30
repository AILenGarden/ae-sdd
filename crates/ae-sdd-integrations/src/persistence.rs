use std::sync::Mutex;
use std::{collections::BTreeMap, path::Path, str::FromStr};

use ae_sdd_contracts::review::{
    ReviewAttemptV2, ReviewBatchReceiptV2, ReviewBatchV2, ReviewExitReceiptV2, ReviewSessionV2,
};
use ae_sdd_domain::{EventStoreId, InputFingerprint};
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{
    DurableEvent, ExecutionCheckpointRecord, ExecutionCheckpointScope, GrantPathWire,
    IdempotencyReceipt, PersistencePort, RuntimeDelegationAttestationRecord,
    RuntimeDelegationRecord, RuntimeError, RuntimeIdentityKind, RuntimeIdentitySnapshot,
    RuntimeIdentityTransition, RuntimeJobRecord, RuntimeJobStatus, RuntimeJobTransition,
    RuntimeResult, RuntimeSessionRecord, RuntimeWorkspaceRecord, ScopedGrantWire, WireAgentRole,
};
use ae_sdd_store::{RuntimeRepository, SqliteRuntimeRepository, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// SQLite/WAL implementation of runtime metadata, event, receipt, and checkpoint ports.
pub struct SqliteRuntimePersistence {
    repository: SqliteRuntimeRepository,
    connection: Mutex<Connection>,
}

/// One typed Review Batch v2 state transition to persist as a SQLite projection.
///
/// The project state/journal remains authoritative.  This value carries the
/// validated typed records needed to rebuild the user-level SQLite projection
/// after a restart or a partially completed projection transaction.
#[derive(Clone, Debug)]
pub(crate) struct ReviewProjectionWrite {
    workspace_id: String,
    work_item_id: String,
    event_sequence: u64,
    session: ReviewSessionV2,
    batch: ReviewBatchV2,
    attempt: ReviewAttemptV2,
    batch_receipt: ReviewBatchReceiptV2,
    exit_receipt: Option<ReviewExitReceiptV2>,
}

impl ReviewProjectionWrite {
    /// Builds a projection write after checking all cross-record identities.
    pub(crate) fn new(
        workspace_id: impl Into<String>,
        work_item_id: impl Into<String>,
        event_sequence: u64,
        session: ReviewSessionV2,
        batch: ReviewBatchV2,
        attempt: ReviewAttemptV2,
        batch_receipt: ReviewBatchReceiptV2,
        exit_receipt: Option<ReviewExitReceiptV2>,
    ) -> RuntimeResult<Self> {
        let workspace_id = workspace_id.into();
        let work_item_id = work_item_id.into();
        if workspace_id.is_empty() || work_item_id.is_empty() || event_sequence == 0 {
            return Err(review_projection_error(
                "review projection identity or event sequence is invalid",
            ));
        }
        if session.review_id() != batch.review_id()
            || session.review_id() != attempt.review_id()
            || session.review_id() != batch_receipt.review_id()
            || batch.batch_id() != attempt.batch_id()
            || batch.batch_id() != batch_receipt.batch_id()
            || batch_receipt.attempt_id() != attempt.attempt_id()
            || batch.latest_receipt() != &batch_receipt
            || exit_receipt.as_ref() != batch.exit_receipt()
            || exit_receipt
                .as_ref()
                .is_some_and(|receipt| receipt.review_id() != session.review_id())
        {
            return Err(review_projection_error(
                "review projection typed records have inconsistent identities",
            ));
        }
        Ok(Self {
            workspace_id,
            work_item_id,
            event_sequence,
            session,
            batch,
            attempt,
            batch_receipt,
            exit_receipt,
        })
    }

    pub(crate) const fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
}

/// Stable read-only view over one persisted Review Batch v2 projection.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReviewAuthorityProjectionV2 {
    pub(crate) session: Value,
    pub(crate) batch: Value,
    pub(crate) attempts: Vec<Value>,
    pub(crate) contributions: Vec<Value>,
    pub(crate) findings: Vec<Value>,
    pub(crate) remediations: Vec<Value>,
    pub(crate) exit_receipt: Option<Value>,
    pub(crate) first_event_sequence: u64,
    pub(crate) last_event_sequence: u64,
}

fn load_identity_receipt(
    transaction: &Transaction<'_>,
    transition: &RuntimeIdentityTransition,
) -> RuntimeResult<Option<RuntimeIdentitySnapshot>> {
    let existing: Option<(String, String, String, i64)> = transaction
        .query_row(
            "SELECT request_digest,snapshot_json,snapshot_digest,snapshot_byte_len FROM runtime_identity_receipt_v1 WHERE workspace_id=?1 AND scope_digest=?2 AND idempotency_key=?3",
            params![
                transition.snapshot.workspace.workspace_id,
                transition.scope_digest,
                transition.idempotency_key
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    let Some((request_digest, json, digest, byte_len)) = existing else {
        return Ok(None);
    };
    if request_digest != transition.request_digest {
        return Err(RuntimeError::new(
            StableErrorCode::IdempotencyKeyReused,
            "identity idempotency key was reused with a different trusted request",
        ));
    }
    decode_identity_snapshot(&json, &digest, byte_len).map(Some)
}

fn decode_identity_snapshot(
    json: &str,
    digest: &str,
    byte_len: i64,
) -> RuntimeResult<RuntimeIdentitySnapshot> {
    if byte_len != i64::try_from(json.len()).map_err(|_| range_error())?
        || digest != hex::encode(Sha256::digest(json.as_bytes()))
        || json.len() > 65_536
    {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "typed identity receipt digest or byte length is invalid",
        ));
    }
    let snapshot: RuntimeIdentitySnapshot = serde_json::from_str(json).map_err(canonical_error)?;
    if contains_secret(&serde_json::to_value(&snapshot).map_err(canonical_error)?) {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "typed identity receipt contains forbidden secret material",
        ));
    }
    Ok(snapshot)
}

fn validate_identity_transition(transition: &RuntimeIdentityTransition) -> RuntimeResult<()> {
    if transition.operation.is_empty()
        || transition.operation.len() > 128
        || transition.idempotency_key.is_empty()
        || transition.idempotency_key.len() > 128
        || !is_digest(&transition.scope_digest)
        || !is_digest(&transition.request_digest)
        || transition.snapshot.workspace.workspace_id.is_empty()
    {
        return Err(schema_error(
            "typed identity transition is malformed or unbounded",
        ));
    }
    Ok(())
}

fn validate_identity_cas(
    transaction: &Transaction<'_>,
    transition: &RuntimeIdentityTransition,
) -> RuntimeResult<()> {
    if transition.expected_workspace_mode.is_some()
        || transition.expected_inventory_generation.is_some()
    {
        let current: Option<(String, i64)> = transaction
            .query_row(
                "SELECT mode,inventory_generation FROM workspace WHERE workspace_id=?1",
                [&transition.snapshot.workspace.workspace_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some((mode, generation)) = current else {
            return revision_conflict("expected workspace row does not exist");
        };
        if transition
            .expected_workspace_mode
            .is_some_and(|expected| workspace_mode(expected) != mode)
            || transition
                .expected_inventory_generation
                .is_some_and(|expected| u64::try_from(generation).ok() != Some(expected))
        {
            return revision_conflict("workspace expected value is stale");
        }
    }
    if let Some(expected) = transition.expected_session_status.as_deref() {
        let session_id = transition
            .snapshot
            .session
            .as_ref()
            .ok_or_else(|| schema_error("session CAS lacks a session after-image"))?
            .session_id
            .as_str();
        let current: Option<String> = transaction
            .query_row(
                "SELECT status FROM agent_session WHERE session_id=?1",
                [session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        if current.as_deref() != Some(expected) {
            return revision_conflict("session expected status is stale");
        }
    }
    if let Some(expected) = transition.expected_delegation_status.as_deref() {
        let delegation_id = transition
            .snapshot
            .delegation
            .as_ref()
            .ok_or_else(|| schema_error("delegation CAS lacks an after-image"))?
            .delegation_id
            .as_str();
        let current: Option<String> = transaction
            .query_row(
                "SELECT status FROM delegation WHERE delegation_id=?1",
                [delegation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        if current.as_deref() != Some(expected) {
            return revision_conflict("delegation expected status is stale");
        }
    }
    if let Some(expected) = transition.expected_context_generation {
        let session_id = transition
            .snapshot
            .session
            .as_ref()
            .ok_or_else(|| schema_error("context CAS lacks a session after-image"))?
            .session_id
            .as_str();
        let current: Option<String> = transaction
            .query_row(
                "SELECT snapshot_json FROM runtime_identity_receipt_v1 WHERE identity_kind='session' AND json_extract(snapshot_json,'$.session.sessionId')=?1 ORDER BY rowid DESC LIMIT 1",
                [session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        let observed = current
            .as_deref()
            .map(serde_json::from_str::<RuntimeIdentitySnapshot>)
            .transpose()
            .map_err(canonical_error)?
            .and_then(|snapshot| snapshot.session)
            .map(|session| session.context_generation);
        if observed != Some(expected) {
            return revision_conflict("session context generation is stale");
        }
    }
    Ok(())
}

fn write_workspace(
    transaction: &Transaction<'_>,
    workspace: &RuntimeWorkspaceRecord,
) -> RuntimeResult<()> {
    let existing: Option<(String, String)> = transaction
        .query_row(
            "SELECT canonical_root,project_key FROM workspace WHERE workspace_id=?1",
            [&workspace.workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    if existing.as_ref().is_some_and(|(root, project)| {
        root != &workspace.canonical_root || project != &workspace.project_key
    }) {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "typed workspace immutable identity changed",
        ));
    }
    transaction
        .execute(
            "INSERT INTO workspace(workspace_id,canonical_root,project_key,mode,inventory_generation,dirty,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(workspace_id) DO UPDATE SET mode=excluded.mode,inventory_generation=excluded.inventory_generation,dirty=excluded.dirty,updated_at=excluded.updated_at",
            params![
                workspace.workspace_id,
                workspace.canonical_root,
                workspace.project_key,
                workspace_mode(workspace.mode),
                to_i64(workspace.inventory_generation)?,
                i64::from(workspace.dirty),
                timestamp(workspace.created_at_unix_ms)?,
                timestamp(workspace.updated_at_unix_ms)?,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn write_session(
    transaction: &Transaction<'_>,
    session: &RuntimeSessionRecord,
) -> RuntimeResult<()> {
    let existing_workspace: Option<String> = transaction
        .query_row(
            "SELECT workspace_id FROM agent_session WHERE external_key_hash=?1 LIMIT 1",
            [&session.external_key_hash],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?;
    if existing_workspace
        .as_deref()
        .is_some_and(|workspace_id| workspace_id != session.workspace_id)
    {
        return Err(RuntimeError::new(
            StableErrorCode::ProjectMismatch,
            "external session key is already bound to another workspace",
        ));
    }
    transaction
        .execute(
            "INSERT INTO agent_session(session_id,agent_id,external_key_hash,workspace_id,role,root_session_id,parent_session_id,delegation_id,status,heartbeat_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(session_id) DO UPDATE SET agent_id=excluded.agent_id,external_key_hash=excluded.external_key_hash,status=excluded.status,heartbeat_at=excluded.heartbeat_at,updated_at=excluded.updated_at",
            params![
                session.session_id,
                session.agent_id,
                session.external_key_hash,
                session.workspace_id,
                agent_role(session.role),
                session.root_session_id,
                session.parent_session_id,
                session.delegation_id,
                session.status,
                timestamp(session.updated_at_unix_ms)?,
                timestamp(session.created_at_unix_ms)?,
                timestamp(session.updated_at_unix_ms)?,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn write_delegation(
    transaction: &Transaction<'_>,
    delegation: &RuntimeDelegationRecord,
) -> RuntimeResult<()> {
    transaction
        .execute(
            "INSERT INTO delegation(delegation_id,workspace_id,root_session_id,parent_session_id,child_session_id,parent_delegation_id,role,input_revision,input_fingerprint,status,deadline,receipt_digest,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14) ON CONFLICT(delegation_id) DO UPDATE SET child_session_id=excluded.child_session_id,status=excluded.status,receipt_digest=excluded.receipt_digest,updated_at=excluded.updated_at",
            params![
                delegation.delegation_id,
                delegation.workspace_id,
                delegation.root_session_id,
                delegation.parent_session_id,
                delegation.child_session_id,
                delegation.parent_delegation_id,
                agent_role(delegation.role),
                to_i64(delegation.input_revision)?,
                delegation.input_fingerprint,
                delegation.status,
                timestamp(delegation.deadline_unix_ms)?,
                delegation.receipt_digest,
                timestamp(delegation.created_at_unix_ms)?,
                timestamp(delegation.updated_at_unix_ms)?,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn write_attestation(
    transaction: &Transaction<'_>,
    attestation: &RuntimeDelegationAttestationRecord,
) -> RuntimeResult<()> {
    let grant_json = serde_json::to_string(&attestation.grant).map_err(canonical_error)?;
    if grant_json.len() > 65_536 {
        return Err(schema_error("delegation grant exceeds 65536 bytes"));
    }
    let grant_digest = hex::encode(Sha256::digest(grant_json.as_bytes()));
    transaction
        .execute(
            "DELETE FROM delegation_grant WHERE delegation_id=?1",
            [&attestation.delegation_id],
        )
        .map_err(sqlite_error)?;
    for (kind, selector) in grant_entries(&attestation.grant) {
        transaction
            .execute(
                "INSERT INTO delegation_grant(delegation_id,resource_kind,selector,created_at) VALUES(?1,?2,?3,?4)",
                params![
                    attestation.delegation_id,
                    kind,
                    selector,
                    timestamp(attestation.accepted_at_unix_ms)?
                ],
            )
            .map_err(sqlite_error)?;
    }
    transaction
        .execute(
            "INSERT INTO delegation_attestation_v1(workspace_id,delegation_id,physical_session_id,host_action_id,host_ack_id,claim_id,action_digest,ack_digest,claim_digest,grant_schema_version,grant_json,grant_digest,grant_byte_len,attestation_ref,attestation_digest,accepted_boot_id,accepted_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,1,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                attestation.workspace_id,
                attestation.delegation_id,
                attestation.physical_session_id,
                attestation.host_action_id,
                attestation.host_ack_id,
                attestation.claim_digest,
                attestation.action_digest,
                attestation.ack_digest,
                attestation.claim_digest,
                grant_json,
                grant_digest,
                i64::try_from(grant_json.len()).map_err(|_| range_error())?,
                attestation.attestation_ref,
                attestation.attestation_digest,
                attestation.accepted_boot_id,
                timestamp(attestation.accepted_at_unix_ms)?,
                timestamp(attestation.expires_at_unix_ms)?,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn identity_snapshot_key(snapshot: &RuntimeIdentitySnapshot) -> RuntimeResult<String> {
    match snapshot.identity_kind {
        RuntimeIdentityKind::Workspace => Ok(snapshot.workspace.workspace_id.clone()),
        RuntimeIdentityKind::Session => snapshot
            .session
            .as_ref()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| schema_error("session identity snapshot lacks a session")),
        RuntimeIdentityKind::Delegation => snapshot
            .delegation
            .as_ref()
            .map(|delegation| delegation.delegation_id.clone())
            .ok_or_else(|| schema_error("delegation identity snapshot lacks a delegation")),
    }
}

fn mirror_typed_runtime_record(
    transaction: &Transaction<'_>,
    namespace: &str,
    key: &str,
    value: &Value,
) -> RuntimeResult<()> {
    let now = UtcTimestamp::now().to_string();
    match namespace {
        "host-adapter/v1" => {
            let digest = hex::encode(Sha256::digest(
                serde_json::to_vec(value).map_err(canonical_error)?,
            ));
            transaction
                .execute(
                    "INSERT INTO host_adapter_instance(adapter_id,capability_digest,status,last_command_seq,heartbeat_at,created_at,updated_at) VALUES(?1,?2,'active',0,?3,?3,?3) ON CONFLICT(adapter_id) DO UPDATE SET capability_digest=excluded.capability_digest,status='active',updated_at=excluded.updated_at",
                    params![key, digest, now],
                )
                .map_err(sqlite_error)?;
        }
        "host-action/v1" => {
            let digest = hex::encode(Sha256::digest(
                serde_json::to_vec(value).map_err(canonical_error)?,
            ));
            let adapter_id = value_string(value, "adapterId")?;
            let kind = value_string(value, "kind")?;
            let command_seq = value_u64(value, "commandSeq")?;
            let session_id = value.get("sessionId").and_then(Value::as_str);
            let context_generation = value.get("contextGeneration").and_then(Value::as_u64);
            let deadline = value_u64(value, "deadlineUnixMs")?;
            transaction
                .execute(
                    "INSERT INTO host_action(action_id,adapter_id,kind,command_seq,request_digest,session_id,context_generation,ack_status,ack_id,response_digest,deadline,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'pending',NULL,NULL,?8,?9,?9) ON CONFLICT(action_id) DO NOTHING",
                    params![
                        key,
                        adapter_id,
                        kind,
                        to_i64(command_seq)?,
                        digest,
                        session_id,
                        context_generation.map(to_i64).transpose()?,
                        timestamp(deadline)?,
                        now,
                    ],
                )
                .map_err(sqlite_error)?;
        }
        "host-ack/v1" => {
            let action_id = value_string(value, "actionId")?;
            let ack_digest = hex::encode(Sha256::digest(
                serde_json::to_vec(value).map_err(canonical_error)?,
            ));
            let outcome = value_string(value, "outcome")?;
            let changed = transaction
                .execute(
                    "UPDATE host_action SET ack_status=?1,ack_id=?2,response_digest=?3,updated_at=?4 WHERE action_id=?5",
                    params![outcome, key, ack_digest, now, action_id],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "typed host ACK references a missing action",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn commit_job_transition(
    persistence: &SqliteRuntimePersistence,
    mut transition: RuntimeJobTransition,
) -> RuntimeResult<RuntimeJobRecord> {
    validate_job_record(&transition.record)?;
    let mut connection = persistence.connection()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error)?;
    if transition.expected_status.is_none() {
        let existing_job_id: Option<String> = transaction
            .query_row(
                "SELECT job_id FROM runtime_job_v1 WHERE submission_scope_digest=?1 AND submission_idempotency_key=?2",
                params![
                    transition.record.submission_scope_digest,
                    transition.record.submission_idempotency_key
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some(job_id) = existing_job_id {
            let existing = load_job_connection(&transaction, &job_id)?.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed job submission index references a missing row",
                )
            })?;
            if existing.request_digest != transition.record.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "job submission key was reused with a different trusted request",
                ));
            }
            transaction.commit().map_err(sqlite_error)?;
            return Ok(existing);
        }
    }
    let event = insert_event(
        &transaction,
        persistence.repository.event_store_id(),
        transition.event,
    )?;
    transition.record.last_event_seq = event.event_seq;
    if let (Some(expected_status), Some(expected_row_version)) =
        (transition.expected_status, transition.expected_row_version)
    {
        if transition.record.row_version != expected_row_version.saturating_add(1) {
            return revision_conflict("job row version did not advance by one");
        }
        let (result_json, result_digest, result_byte_len) =
            optional_canonical_json(transition.record.result.as_ref())?;
        let changed = transaction
            .execute(
                "UPDATE runtime_job_v1 SET status=?1,row_version=?2,result_schema_version=?3,result_json=?4,result_digest=?5,result_byte_len=?6,error_code=?7,mutation_id=?8,receipt_locator=?9,project_receipt_digest=?10,source_revision=?11,last_event_seq=?12,started_at=?13,finished_at=?14,updated_at=?15 WHERE job_id=?16 AND status=?17 AND row_version=?18",
                params![
                    job_status(transition.record.status),
                    to_i64(transition.record.row_version)?,
                    result_json.as_ref().map(|_| 1_i64),
                    result_json,
                    result_digest,
                    result_byte_len,
                    transition.record.error_code,
                    transition.record.mutation_id,
                    transition.record.receipt_locator,
                    transition.record.project_receipt_digest,
                    transition.record.source_revision.map(to_i64).transpose()?,
                    to_i64(transition.record.last_event_seq)?,
                    transition.record.started_at_unix_ms.map(timestamp).transpose()?,
                    transition.record.finished_at_unix_ms.map(timestamp).transpose()?,
                    timestamp(transition.record.updated_at_unix_ms)?,
                    transition.record.job_id,
                    job_status(expected_status),
                    to_i64(expected_row_version)?,
                ],
            )
            .map_err(sqlite_error)?;
        if changed != 1 {
            return revision_conflict("typed job expected status or row version is stale");
        }
    } else if transition.expected_status.is_some() || transition.expected_row_version.is_some() {
        return Err(schema_error(
            "job expected status and row version must both be present or absent",
        ));
    } else {
        transition.record.submitted_event_seq = event.event_seq;
        transition.record.last_event_seq = event.event_seq;
        insert_job(&transaction, &transition.record)?;
    }
    let committed =
        load_job_connection(&transaction, &transition.record.job_id)?.ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "typed job commit did not produce a durable row",
            )
        })?;
    transaction.commit().map_err(sqlite_error)?;
    Ok(committed)
}

fn insert_job(transaction: &Transaction<'_>, record: &RuntimeJobRecord) -> RuntimeResult<()> {
    let (arguments_json, arguments_digest, arguments_byte_len) = canonical_json(&record.arguments)?;
    let (grant_json, grant_digest, grant_byte_len) =
        optional_canonical_grant(record.grant.as_ref())?;
    let (result_json, result_digest, result_byte_len) =
        optional_canonical_json(record.result.as_ref())?;
    transaction
        .execute(
            "INSERT INTO runtime_job_v1(job_id,schema_version,workspace_id,work_item_id,session_id,root_session_id,delegation_id,agent_role,context_generation,submission_boot_id,attestation_ref,attestation_digest,grant_schema_version,grant_json,grant_digest,grant_byte_len,identity_digest,workspace_mode,inventory_generation,entrypoint,arguments_schema_version,arguments_json,arguments_digest,arguments_byte_len,submission_scope_digest,submission_idempotency_key,submission_idempotency_key_digest,request_digest,source_revision,input_fingerprint,deadline_at,status,row_version,result_schema_version,result_json,result_digest,result_byte_len,error_code,mutation_id,receipt_locator,project_receipt_digest,submitted_event_seq,last_event_seq,created_at,started_at,finished_at,updated_at) VALUES(?1,1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,1,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,?38,?39,?40,?41,?42,?43,?44,?45)",
            params![
                record.job_id,
                record.workspace_id,
                record.work_item_id,
                record.session_id,
                record.root_session_id,
                record.delegation_id,
                record.agent_role.map(agent_role),
                record.context_generation.map(to_i64).transpose()?,
                record.submission_boot_id,
                record.attestation_ref,
                record.attestation_digest,
                grant_json.as_ref().map(|_| 1_i64),
                grant_json,
                grant_digest,
                grant_byte_len,
                record.identity_digest,
                workspace_mode(record.workspace_mode),
                to_i64(record.inventory_generation)?,
                record.entrypoint,
                arguments_json,
                arguments_digest,
                arguments_byte_len,
                record.submission_scope_digest,
                record.submission_idempotency_key,
                record.submission_idempotency_key_digest,
                record.request_digest,
                record.source_revision.map(to_i64).transpose()?,
                record.input_fingerprint,
                timestamp(record.deadline_unix_ms)?,
                job_status(record.status),
                to_i64(record.row_version)?,
                result_json.as_ref().map(|_| 1_i64),
                result_json,
                result_digest,
                result_byte_len,
                record.error_code,
                record.mutation_id,
                record.receipt_locator,
                record.project_receipt_digest,
                to_i64(record.submitted_event_seq)?,
                to_i64(record.last_event_seq)?,
                timestamp(record.created_at_unix_ms)?,
                record.started_at_unix_ms.map(timestamp).transpose()?,
                record.finished_at_unix_ms.map(timestamp).transpose()?,
                timestamp(record.updated_at_unix_ms)?,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn load_job_connection(
    connection: &Connection,
    job_id: &str,
) -> RuntimeResult<Option<RuntimeJobRecord>> {
    let wire: Option<JobRow> = connection
        .query_row(
            "SELECT job_id,workspace_id,work_item_id,session_id,root_session_id,delegation_id,agent_role,context_generation,submission_boot_id,attestation_ref,attestation_digest,grant_json,grant_digest,grant_byte_len,identity_digest,workspace_mode,inventory_generation,entrypoint,arguments_json,arguments_digest,arguments_byte_len,submission_scope_digest,submission_idempotency_key,submission_idempotency_key_digest,request_digest,source_revision,input_fingerprint,deadline_at,status,row_version,result_json,result_digest,result_byte_len,error_code,mutation_id,receipt_locator,project_receipt_digest,submitted_event_seq,last_event_seq,created_at,started_at,finished_at,updated_at FROM runtime_job_v1 WHERE job_id=?1",
            [job_id],
            |row| {
                Ok(JobRow {
                    job_id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    work_item_id: row.get(2)?,
                    session_id: row.get(3)?,
                    root_session_id: row.get(4)?,
                    delegation_id: row.get(5)?,
                    agent_role: row.get(6)?,
                    context_generation: row.get(7)?,
                    submission_boot_id: row.get(8)?,
                    attestation_ref: row.get(9)?,
                    attestation_digest: row.get(10)?,
                    grant_json: row.get(11)?,
                    grant_digest: row.get(12)?,
                    grant_byte_len: row.get(13)?,
                    identity_digest: row.get(14)?,
                    workspace_mode: row.get(15)?,
                    inventory_generation: row.get(16)?,
                    entrypoint: row.get(17)?,
                    arguments_json: row.get(18)?,
                    arguments_digest: row.get(19)?,
                    arguments_byte_len: row.get(20)?,
                    submission_scope_digest: row.get(21)?,
                    submission_idempotency_key: row.get(22)?,
                    submission_idempotency_key_digest: row.get(23)?,
                    request_digest: row.get(24)?,
                    source_revision: row.get(25)?,
                    input_fingerprint: row.get(26)?,
                    deadline_at: row.get(27)?,
                    status: row.get(28)?,
                    row_version: row.get(29)?,
                    result_json: row.get(30)?,
                    result_digest: row.get(31)?,
                    result_byte_len: row.get(32)?,
                    error_code: row.get(33)?,
                    mutation_id: row.get(34)?,
                    receipt_locator: row.get(35)?,
                    project_receipt_digest: row.get(36)?,
                    submitted_event_seq: row.get(37)?,
                    last_event_seq: row.get(38)?,
                    created_at: row.get(39)?,
                    started_at: row.get(40)?,
                    finished_at: row.get(41)?,
                    updated_at: row.get(42)?,
                })
            },
        )
        .optional()
        .map_err(sqlite_error)?;
    wire.map(JobRow::decode).transpose()
}

struct JobRow {
    job_id: String,
    workspace_id: String,
    work_item_id: Option<String>,
    session_id: Option<String>,
    root_session_id: Option<String>,
    delegation_id: Option<String>,
    agent_role: Option<String>,
    context_generation: Option<i64>,
    submission_boot_id: Option<String>,
    attestation_ref: Option<String>,
    attestation_digest: Option<String>,
    grant_json: Option<String>,
    grant_digest: Option<String>,
    grant_byte_len: Option<i64>,
    identity_digest: Option<String>,
    workspace_mode: String,
    inventory_generation: i64,
    entrypoint: String,
    arguments_json: String,
    arguments_digest: String,
    arguments_byte_len: i64,
    submission_scope_digest: String,
    submission_idempotency_key: String,
    submission_idempotency_key_digest: String,
    request_digest: String,
    source_revision: Option<i64>,
    input_fingerprint: Option<String>,
    deadline_at: String,
    status: String,
    row_version: i64,
    result_json: Option<String>,
    result_digest: Option<String>,
    result_byte_len: Option<i64>,
    error_code: Option<String>,
    mutation_id: Option<String>,
    receipt_locator: Option<String>,
    project_receipt_digest: Option<String>,
    submitted_event_seq: i64,
    last_event_seq: i64,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    updated_at: String,
}

impl JobRow {
    fn decode(self) -> RuntimeResult<RuntimeJobRecord> {
        let arguments = decode_json_body(
            &self.arguments_json,
            &self.arguments_digest,
            self.arguments_byte_len,
        )?;
        let grant = match (
            self.grant_json.as_deref(),
            self.grant_digest.as_deref(),
            self.grant_byte_len,
        ) {
            (None, None, None) => None,
            (Some(json), Some(digest), Some(byte_len)) => {
                let value = decode_json_body(json, digest, byte_len)?;
                Some(serde_json::from_value(value).map_err(canonical_error)?)
            }
            _ => {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed job grant columns are incomplete",
                ));
            }
        };
        let result = match (
            self.result_json.as_deref(),
            self.result_digest.as_deref(),
            self.result_byte_len,
        ) {
            (None, None, None) => None,
            (Some(json), Some(digest), Some(byte_len)) => {
                Some(decode_json_body(json, digest, byte_len)?)
            }
            _ => {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed job result columns are incomplete",
                ));
            }
        };
        if hex::encode(Sha256::digest(self.submission_idempotency_key.as_bytes()))
            != self.submission_idempotency_key_digest
        {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "typed job idempotency key digest is invalid",
            ));
        }
        Ok(RuntimeJobRecord {
            job_id: self.job_id,
            workspace_id: self.workspace_id,
            work_item_id: self.work_item_id,
            session_id: self.session_id,
            root_session_id: self.root_session_id,
            delegation_id: self.delegation_id,
            agent_role: self
                .agent_role
                .as_deref()
                .map(parse_agent_role)
                .transpose()?,
            context_generation: self.context_generation.map(from_i64).transpose()?,
            submission_boot_id: self.submission_boot_id,
            attestation_ref: self.attestation_ref,
            attestation_digest: self.attestation_digest,
            grant,
            identity_digest: self.identity_digest,
            workspace_mode: parse_workspace_mode(&self.workspace_mode)?,
            inventory_generation: from_i64(self.inventory_generation)?,
            entrypoint: self.entrypoint,
            arguments,
            submission_scope_digest: self.submission_scope_digest,
            submission_idempotency_key: self.submission_idempotency_key,
            submission_idempotency_key_digest: self.submission_idempotency_key_digest,
            request_digest: self.request_digest,
            source_revision: self.source_revision.map(from_i64).transpose()?,
            input_fingerprint: self.input_fingerprint,
            deadline_unix_ms: parse_timestamp(&self.deadline_at)?,
            status: parse_job_status(&self.status)?,
            row_version: from_i64(self.row_version)?,
            result,
            error_code: self.error_code,
            mutation_id: self.mutation_id,
            receipt_locator: self.receipt_locator,
            project_receipt_digest: self.project_receipt_digest,
            submitted_event_seq: from_i64(self.submitted_event_seq)?,
            last_event_seq: from_i64(self.last_event_seq)?,
            created_at_unix_ms: parse_timestamp(&self.created_at)?,
            started_at_unix_ms: self
                .started_at
                .as_deref()
                .map(parse_timestamp)
                .transpose()?,
            finished_at_unix_ms: self
                .finished_at
                .as_deref()
                .map(parse_timestamp)
                .transpose()?,
            updated_at_unix_ms: parse_timestamp(&self.updated_at)?,
        })
    }
}

impl SqliteRuntimePersistence {
    /// Opens or migrates a durable runtime database.
    pub fn open(path: impl AsRef<Path>) -> RuntimeResult<Self> {
        let proposed = EventStoreId::from_uuid(Uuid::new_v4());
        let now = UtcTimestamp::now();
        let repository =
            SqliteRuntimeRepository::open(path.as_ref(), proposed, &now).map_err(store_error)?;
        let connection = Connection::open(path).map_err(sqlite_error)?;
        configure(&connection)?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS runtime_generic_receipt_v1(\
                    scope TEXT NOT NULL,key TEXT NOT NULL,request_digest TEXT NOT NULL,\
                    response_json TEXT NOT NULL,event_seq INTEGER NOT NULL REFERENCES runtime_event(event_seq),\
                    PRIMARY KEY(scope,key));\
                 CREATE TABLE IF NOT EXISTS runtime_record_v1(\
                    namespace TEXT NOT NULL,key TEXT NOT NULL,value_json TEXT NOT NULL,\
                    PRIMARY KEY(namespace,key));",
            )
            .map_err(sqlite_error)?;
        Ok(Self {
            repository,
            connection: Mutex::new(connection),
        })
    }

    /// Runs SQLite integrity verification.
    pub fn integrity_check(&self) -> RuntimeResult<()> {
        self.repository.integrity_check().map_err(store_error)
    }

    fn connection(&self) -> RuntimeResult<std::sync::MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "runtime SQLite connection lock is poisoned",
            )
        })
    }
}

/// Persists one committed Review Batch v2 projection transaction.
pub(crate) fn upsert_review_authority_projection(
    database: &Path,
    write: &ReviewProjectionWrite,
) -> RuntimeResult<()> {
    let mut connection = Connection::open(database).map_err(sqlite_error)?;
    configure(&connection)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error)?;
    apply_review_projection(&transaction, write)?;
    transaction.commit().map_err(sqlite_error)
}

/// Replays event-derived Review Batch v2 projection writes in event order.
///
/// Existing exact rows are accepted, missing rows are restored, and stale or
/// conflicting rows fail closed.  No authoritative project state is deleted.
pub(crate) fn rebuild_review_authority_projections(
    database: &Path,
    writes: &[ReviewProjectionWrite],
) -> RuntimeResult<()> {
    let mut ordered = writes.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|write| write.event_sequence());
    let mut connection = Connection::open(database).map_err(sqlite_error)?;
    configure(&connection)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error)?;
    for write in ordered {
        apply_review_projection(&transaction, write)?;
    }
    transaction.commit().map_err(sqlite_error)
}

/// Loads one Review Batch v2 projection using stable row ordering.
pub(crate) fn load_review_authority_projection(
    database: &Path,
    workspace_id: &str,
    work_item_id: &str,
    review_id: &str,
) -> RuntimeResult<Option<ReviewAuthorityProjectionV2>> {
    let connection = Connection::open(database).map_err(sqlite_error)?;
    configure(&connection)?;
    load_review_authority_projection_connection(&connection, workspace_id, work_item_id, review_id)
}

fn load_review_authority_projection_connection(
    connection: &Connection,
    workspace_id: &str,
    work_item_id: &str,
    review_id: &str,
) -> RuntimeResult<Option<ReviewAuthorityProjectionV2>> {
    let session_row: Option<(String, i64, i64)> = connection
        .query_row(
            r#"SELECT json_object(
                'schemaVersion',schema_version,'workspaceId',workspace_id,
                'workItemId',work_item_id,'reviewId',review_id,
                'parentReviewId',parent_review_id,'tier',tier,'status',status,
                'authorSessionId',author_session_id,'rootSessionId',root_session_id,
                'repairClass',repair_class,
                'requiredSpecialties',json(CASE tier
                    WHEN 'tier1' THEN '["general"]'
                    WHEN 'tier2' THEN '["be","ar"]'
                    ELSE '["be","ar","qa"]' END),
                'cleanPolicy',json_object('cleanTarget',clean_target,
                    'finalProofRequirement',final_proof_requirement),
                'budget',json_object('maxAttempts',max_attempts,
                    'maxValidBatches',max_valid_batches,
                    'maxRemediations',max_remediations,
                    'maxWallClockMinutes',max_wall_clock_minutes),
                'counters',json_object('attempts',attempts,'validBatches',valid_batches,
                    'remediations',remediations,'cleanStreak',clean_streak,
                    'infraFailures',infra_failures,'protocolFailures',protocol_failures),
                'inputFingerprint',input_fingerprint,'rulesetFingerprint',ruleset_fingerprint,
                'sourceRevision',source_revision,'inventoryGeneration',inventory_generation,
                'startedAt',started_at,'deadlineAt',deadline_at,'terminalAt',terminal_at,
                'firstEventSequence',first_event_seq,'lastEventSequence',last_event_seq,
                'createdAt',created_at,'updatedAt',updated_at),
                first_event_seq,last_event_seq
             FROM review_session_v2_projection
             WHERE workspace_id=?1 AND work_item_id=?2 AND review_id=?3"#,
            params![workspace_id, work_item_id, review_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    let Some((session_json, first_event_seq, last_event_seq)) = session_row else {
        return Ok(None);
    };
    let session = serde_json::from_str(&session_json).map_err(canonical_error)?;

    let batch_json: Option<String> = connection
        .query_row(
            r#"SELECT json_object(
                'schemaVersion',schema_version,'workspaceId',workspace_id,
                'reviewId',review_id,'batchId',batch_id,
                'inputFingerprint',input_fingerprint,'rulesetFingerprint',ruleset_fingerprint,
                'latestAttemptId',latest_attempt_id,'latestStatus',latest_status,
                'requiredSpecialtyCount',required_specialty_count,
                'effectiveContributionCount',effective_contribution_count,
                'findingCount',finding_count,
                'closed',json(CASE closed WHEN 1 THEN 'true' ELSE 'false' END),
                'validBatchOrdinal',valid_batch_ordinal,
                'latestReceiptDigest',latest_receipt_digest,
                'firstEventSequence',first_event_seq,'lastEventSequence',last_event_seq,
                'createdAt',created_at,'updatedAt',updated_at)
             FROM review_batch_v2_projection
             WHERE workspace_id=?1 AND review_id=?2
             ORDER BY last_event_seq DESC,batch_id DESC LIMIT 1"#,
            params![workspace_id, review_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?;
    let batch = batch_json
        .map(|json| serde_json::from_str(&json).map_err(canonical_error))
        .transpose()?
        .ok_or_else(|| {
            review_projection_error("review session projection is missing its batch projection")
        })?;

    let mut statement = connection
        .prepare(
            r#"SELECT json_object(
                'schemaVersion',schema_version,'workspaceId',workspace_id,
                'reviewId',review_id,'batchId',batch_id,'attemptId',attempt_id,
                'attemptOrdinal',attempt_ordinal,'status',status,
                'inputFingerprint',input_fingerprint,'rulesetFingerprint',ruleset_fingerprint,
                'requiredSpecialtyCount',required_specialty_count,
                'attemptedSpecialtyCount',attempted_specialty_count,
                'effectiveSpecialtyCount',effective_specialty_count,
                'findingCount',finding_count,'idempotencyKeyDigest',idempotency_key_digest,
                'payloadDigest',payload_digest,'receiptDigest',receipt_digest,
                'startedAt',started_at,'completedAt',completed_at,'eventSequence',event_seq)
             FROM review_attempt_v2_projection
             WHERE workspace_id=?1 AND review_id=?2
             ORDER BY attempt_ordinal,batch_id,attempt_id"#,
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![workspace_id, review_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sqlite_error)?;
    let attempts = rows
        .map(|row| {
            let json = row.map_err(sqlite_error)?;
            serde_json::from_str(&json).map_err(canonical_error)
        })
        .collect::<RuntimeResult<Vec<Value>>>()?;
    if attempts.is_empty() {
        return Err(review_projection_error(
            "review batch projection is missing its attempt projection",
        ));
    }

    let mut statement = connection
        .prepare(
            r#"SELECT json_object(
                'schemaVersion',schema_version,'workspaceId',workspace_id,
                'reviewId',review_id,'batchId',batch_id,'specialty',specialty,
                'sourceAttemptId',source_attempt_id,'agentRole',agent_role,'outcome',outcome,
                'physicalSessionId',physical_session_id,'rootSessionId',root_session_id,
                'delegationId',delegation_id,'lineageDepth',lineage_depth,
                'attestationRef',attestation_ref,'attestationDigest',attestation_digest,
                'specialtyGrantDigest',specialty_grant_digest,'reportDigest',report_digest,
                'contributionDigest',contribution_digest,'inputFingerprint',input_fingerprint,
                'rulesetFingerprint',ruleset_fingerprint,'findingCount',finding_count,
                'eventSequence',event_seq,'createdAt',created_at,'updatedAt',updated_at)
             FROM review_effective_contribution_v2_projection
             WHERE workspace_id=?1 AND review_id=?2
             ORDER BY event_seq,batch_id,specialty,contribution_digest"#,
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![workspace_id, review_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sqlite_error)?;
    let contributions = rows
        .map(|row| {
            let json = row.map_err(sqlite_error)?;
            serde_json::from_str(&json).map_err(canonical_error)
        })
        .collect::<RuntimeResult<Vec<Value>>>()?;

    let mut statement = connection
        .prepare(
            r#"SELECT json_object(
                'schemaVersion',schema_version,'workspaceId',workspace_id,
                'reviewId',review_id,'findingFingerprint',finding_fingerprint,
                'firstBatchId',first_batch_id,'lastBatchId',last_batch_id,
                'firstSeenAttemptId',first_seen_attempt_id,
                'lastSeenAttemptId',last_seen_attempt_id,
                'firstReportedSpecialty',first_reported_specialty,
                'code',code,'severity',severity,'summary',summary,'status',status,
                'firstEventSequence',first_event_seq,'lastEventSequence',last_event_seq,
                'createdAt',created_at,'updatedAt',updated_at)
             FROM review_finding_v2_projection
             WHERE workspace_id=?1 AND review_id=?2
             ORDER BY first_event_seq,finding_fingerprint"#,
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![workspace_id, review_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sqlite_error)?;
    let findings = rows
        .map(|row| {
            let json = row.map_err(sqlite_error)?;
            serde_json::from_str(&json).map_err(canonical_error)
        })
        .collect::<RuntimeResult<Vec<Value>>>()?;

    let mut statement = connection
        .prepare(
            r#"SELECT json_object(
                'schemaVersion',schema_version,'workspaceId',workspace_id,
                'reviewId',review_id,'findingBatchId',finding_batch_id,
                'planFingerprint',plan_fingerprint,'newInputFingerprint',new_input_fingerprint,
                'nextReviewId',next_review_id,'remediationDigest',remediation_digest,
                'sourceRevision',source_revision,'targetRevision',target_revision,
                'eventSequence',event_seq,'createdAt',created_at)
             FROM review_remediation_v2_projection
             WHERE workspace_id=?1 AND review_id=?2
             ORDER BY event_seq,finding_batch_id,plan_fingerprint,new_input_fingerprint"#,
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![workspace_id, review_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sqlite_error)?;
    let remediations = rows
        .map(|row| {
            let json = row.map_err(sqlite_error)?;
            serde_json::from_str(&json).map_err(canonical_error)
        })
        .collect::<RuntimeResult<Vec<Value>>>()?;

    let exit_json: Option<String> = connection
        .query_row(
            r#"SELECT json_object(
                'schemaVersion',schema_version,'workspaceId',workspace_id,
                'workItemId',work_item_id,'reviewId',review_id,'tier',tier,
                'sessionStatus',session_status,'disposition',disposition,
                'inputFingerprint',input_fingerprint,
                'observedInputFingerprint',observed_input_fingerprint,
                'rulesetFingerprint',ruleset_fingerprint,'sourceRevision',source_revision,
                'inventoryGeneration',inventory_generation,'policyDigest',policy_digest,
                'requiredSpecialtyCount',required_specialty_count,
                'completedSpecialtyCount',completed_specialty_count,'findingCount',finding_count,
                'counters',json_object('attempts',attempts,'validBatches',valid_batches,
                    'cleanStreak',clean_streak,'remediations',remediations),
                'cleanTarget',clean_target,'lastBatchId',last_batch_id,
                'lastAttemptId',last_attempt_id,'lastAttemptStatus',last_attempt_status,
                'zeroFindingBatchReceiptDigest',zero_finding_batch_receipt_digest,
                'finalProof',json_object('kind',final_proof_kind,'digest',final_proof_digest,
                    'sourceRevision',final_proof_state_revision,
                    'inputFingerprint',final_proof_input_fingerprint,
                    'rulesetFingerprint',final_proof_ruleset_fingerprint,
                    'observedAt',final_proof_observed_at),
                'projectAuthority',json_object('projectReceiptRef',project_receipt_ref,
                    'activeManifestDigest',active_manifest_digest,
                    'stateReceiptRefDigest',state_receipt_ref_digest,
                    'journalMutationId',journal_mutation_id),
                'receiptDigest',receipt_digest,'eventSequence',event_seq,'createdAt',created_at)
             FROM review_exit_receipt_v2_projection
             WHERE workspace_id=?1 AND work_item_id=?2 AND review_id=?3"#,
            params![workspace_id, work_item_id, review_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?;
    let exit_receipt = exit_json
        .map(|json| serde_json::from_str(&json).map_err(canonical_error))
        .transpose()?;

    Ok(Some(ReviewAuthorityProjectionV2 {
        session,
        batch,
        attempts,
        contributions,
        findings,
        remediations,
        exit_receipt,
        first_event_sequence: from_i64(first_event_seq)?,
        last_event_sequence: from_i64(last_event_seq)?,
    }))
}

fn apply_review_projection(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
) -> RuntimeResult<()> {
    let committed_at = review_event_committed_at(transaction, write)?;
    let session = review_json(&write.session, "review session")?;
    let batch = review_json(&write.batch, "review batch")?;
    let attempt = review_json(&write.attempt, "review attempt")?;
    let batch_receipt = review_json(&write.batch_receipt, "review batch receipt")?;
    let exit_receipt = write
        .exit_receipt
        .as_ref()
        .map(|receipt| review_json(receipt, "review exit receipt"))
        .transpose()?;
    for value in [&session, &batch, &attempt, &batch_receipt] {
        require_review_v2(value)?;
    }
    if let Some(value) = exit_receipt.as_ref() {
        require_review_v2(value)?;
    }

    if persist_review_projection_receipt(transaction, write)? {
        return Ok(());
    }
    persist_review_session(transaction, write, &session, &committed_at)?;
    persist_review_batch(
        transaction,
        write,
        &session,
        &batch,
        &batch_receipt,
        &committed_at,
    )?;
    persist_review_attempt(
        transaction,
        write,
        &session,
        &batch,
        &attempt,
        &batch_receipt,
        &committed_at,
    )?;
    persist_review_contributions(transaction, write, &batch)?;
    persist_review_findings(transaction, write, &batch)?;
    persist_review_remediation(transaction, write, &session, &attempt, &committed_at)?;
    if let Some(exit_receipt) = exit_receipt.as_ref() {
        persist_review_exit_receipt(transaction, write, exit_receipt, &committed_at)?;
    }
    Ok(())
}

fn review_event_committed_at(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
) -> RuntimeResult<String> {
    let event: Option<(String, String, String, String)> = transaction
        .query_row(
            "SELECT workspace_id,work_item_id,event_type,committed_at FROM runtime_event WHERE event_seq=?1",
            params![to_i64(write.event_sequence)?],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    let Some((workspace_id, work_item_id, event_type, committed_at)) = event else {
        return Err(review_projection_error(
            "review projection references a missing committed runtime event",
        ));
    };
    if workspace_id != write.workspace_id
        || work_item_id != write.work_item_id
        || !matches!(event_type.as_str(), "review.record" | "review.finalize")
        || parse_timestamp(&committed_at).is_err()
    {
        return Err(review_projection_error(
            "review projection event identity is inconsistent",
        ));
    }
    Ok(committed_at)
}

fn review_json<T: serde::Serialize>(value: &T, label: &'static str) -> RuntimeResult<Value> {
    serde_json::to_value(value).map_err(|_| review_projection_error(label))
}

fn require_review_v2(value: &Value) -> RuntimeResult<()> {
    if projection_string(value, "schemaVersion")? != "v2" {
        return Err(review_projection_error(
            "review projection schemaVersion is not v2",
        ));
    }
    Ok(())
}

fn projection_object<'a>(
    value: &'a Value,
    key: &str,
) -> RuntimeResult<&'a serde_json::Map<String, Value>> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| review_projection_error("review projection object field is missing"))
}

fn projection_array<'a>(value: &'a Value, key: &str) -> RuntimeResult<&'a [Value]> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| review_projection_error("review projection array field is missing"))
}

fn projection_string<'a>(value: &'a Value, key: &str) -> RuntimeResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| review_projection_error("review projection string field is missing"))
}

fn projection_optional_string<'a>(value: &'a Value, key: &str) -> RuntimeResult<Option<&'a str>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| review_projection_error("review projection optional string is invalid")),
    }
}

fn projection_u64(value: &Value, key: &str) -> RuntimeResult<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| review_projection_error("review projection integer field is missing"))
}

fn projection_optional_u64(value: &Value, key: &str) -> RuntimeResult<Option<u64>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            review_projection_error("review projection optional integer is invalid")
        }),
    }
}

fn projection_bool(value: &Value, key: &str) -> RuntimeResult<bool> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| review_projection_error("review projection boolean field is missing"))
}

fn projection_count(value: &Value, key: &str) -> RuntimeResult<u64> {
    u64::try_from(projection_array(value, key)?.len())
        .map_err(|_| review_projection_error("review projection collection is too large"))
}

/// Maps a frozen v2 batch/attempt status wire value to its migration 0009
/// column domain.
///
/// `ReviewBatchStatusV2` serializes as upper snake case (`VALID_CLEAN`) while
/// the projection columns are constrained to lower snake case. Unknown values
/// fail closed instead of being lowercased blindly, so an unrecognized status
/// can never reach a CHECK constraint as if it were valid.
fn projection_batch_status(value: &Value, key: &str) -> RuntimeResult<&'static str> {
    projected_batch_status_column(projection_string(value, key)?)
}

/// Single source of truth for the wire -> migration 0009 column status mapping.
/// Both the projection writer and the Review Gate validator read it so the two
/// domains can never drift.
pub(crate) fn projected_batch_status_column(wire: &str) -> RuntimeResult<&'static str> {
    match wire {
        "VALID_CLEAN" => Ok("valid_clean"),
        "VALID_FINDINGS" => Ok("valid_findings"),
        "INVALID_INFRA" => Ok("invalid_infra"),
        "INVALID_PROTOCOL" => Ok("invalid_protocol"),
        "INVALID_INPUT_DRIFT" => Ok("invalid_input_drift"),
        "CANCELLED" => Ok("cancelled"),
        _ => Err(review_projection_error(
            "review projection carries an unregistered v2 batch status",
        )),
    }
}

fn projection_digest<'a>(value: &'a Value, key: &str) -> RuntimeResult<&'a str> {
    let digest = projection_string(value, key)?;
    if !is_digest(digest) {
        return Err(review_projection_error(
            "review projection digest field is invalid",
        ));
    }
    Ok(digest)
}

fn bool_i64(value: bool) -> i64 {
    i64::from(value)
}

fn projection_child<'a>(value: &'a Value, key: &str) -> RuntimeResult<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| review_projection_error("review projection nested field is missing"))
}

fn projection_optional_digest<'a>(value: &'a Value, key: &str) -> RuntimeResult<Option<&'a str>> {
    let digest = projection_optional_string(value, key)?;
    if digest.is_some_and(|value| !is_digest(value)) {
        return Err(review_projection_error(
            "review projection optional digest field is invalid",
        ));
    }
    Ok(digest)
}

fn review_attempt_source(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    review_id: &str,
    batch_id: &str,
    attempt_id: &str,
) -> RuntimeResult<(i64, String)> {
    transaction
        .query_row(
            "SELECT event_seq,completed_at FROM review_attempt_v2_projection \
             WHERE workspace_id=?1 AND review_id=?2 AND batch_id=?3 AND attempt_id=?4",
            params![workspace_id, review_id, batch_id, attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| {
            review_projection_error(
                "review projection contribution references a missing source attempt",
            )
        })
}

fn persist_review_projection_receipt(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
) -> RuntimeResult<bool> {
    let value = serde_json::json!({
        "schemaVersion": 2,
        "workspaceId": &write.workspace_id,
        "workItemId": &write.work_item_id,
        "eventSequence": write.event_sequence,
        "session": &write.session,
        "batch": &write.batch,
        "attempt": &write.attempt,
        "batchReceipt": &write.batch_receipt,
        "exitReceipt": &write.exit_receipt,
    });
    let bytes = serde_json::to_vec(&value).map_err(|_| {
        review_projection_error("review projection write could not be canonicalized")
    })?;
    let digest = hex::encode(Sha256::digest(bytes));
    let key = format!("{}:{}", write.workspace_id, write.event_sequence);
    let receipt = serde_json::json!({
        "schemaVersion": 2,
        "workspaceId": &write.workspace_id,
        "workItemId": &write.work_item_id,
        "eventSequence": write.event_sequence,
        "digest": digest,
    });
    let receipt_json = serde_json::to_string(&receipt).map_err(|_| {
        review_projection_error("review projection receipt could not be serialized")
    })?;
    let existing: Option<String> = transaction
        .query_row(
            "SELECT value_json FROM runtime_record_v1 WHERE namespace='review-projection-event/v3' AND key=?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?;
    if let Some(existing) = existing {
        if existing != receipt_json {
            return Err(review_projection_error(
                "review projection event was replayed with different typed records",
            ));
        }
        return Ok(true);
    }
    transaction
        .execute(
            "INSERT INTO runtime_record_v1(namespace,key,value_json) VALUES('review-projection-event/v3',?1,?2)",
            params![key, receipt_json],
        )
        .map_err(sqlite_error)?;
    Ok(false)
}

fn persist_review_session(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    session: &Value,
    committed_at: &str,
) -> RuntimeResult<()> {
    let required = projection_array(session, "requiredSpecialties")?;
    let requires = |name: &str| required.iter().any(|value| value.as_str() == Some(name));
    let clean_policy = projection_object(session, "cleanPolicy")?;
    let budget = projection_object(session, "budget")?;
    let counters = projection_object(session, "counters")?;
    let terminal_at = projection_optional_string(session, "terminalAt")?;
    let changed = transaction
        .execute(
            "INSERT INTO review_session_v2_projection(\
                workspace_id,work_item_id,review_id,schema_version,parent_review_id,tier,status,\
                author_session_id,root_session_id,repair_class,final_proof_requirement,\
                requires_general,requires_be,requires_ar,requires_qa,input_fingerprint,\
                ruleset_fingerprint,source_revision,inventory_generation,attempts,valid_batches,\
                remediations,clean_streak,clean_target,infra_failures,protocol_failures,\
                max_attempts,max_valid_batches,max_remediations,max_wall_clock_minutes,\
                started_at,deadline_at,terminal_at,first_event_seq,last_event_seq,created_at,updated_at\
             ) VALUES(\
                ?1,?2,?3,2,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,\
                ?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?33,?30,?34\
             ) ON CONFLICT(workspace_id,review_id) DO UPDATE SET \
                status=excluded.status,attempts=excluded.attempts,valid_batches=excluded.valid_batches,\
                remediations=excluded.remediations,clean_streak=excluded.clean_streak,\
                infra_failures=excluded.infra_failures,protocol_failures=excluded.protocol_failures,\
                terminal_at=excluded.terminal_at,last_event_seq=excluded.last_event_seq,updated_at=excluded.updated_at \
             WHERE excluded.last_event_seq>=review_session_v2_projection.last_event_seq \
               AND review_session_v2_projection.work_item_id=excluded.work_item_id \
               AND review_session_v2_projection.parent_review_id IS excluded.parent_review_id \
               AND review_session_v2_projection.tier=excluded.tier \
               AND review_session_v2_projection.author_session_id=excluded.author_session_id \
               AND review_session_v2_projection.root_session_id=excluded.root_session_id \
               AND review_session_v2_projection.repair_class=excluded.repair_class \
               AND review_session_v2_projection.final_proof_requirement=excluded.final_proof_requirement \
               AND review_session_v2_projection.requires_general=excluded.requires_general \
               AND review_session_v2_projection.requires_be=excluded.requires_be \
               AND review_session_v2_projection.requires_ar=excluded.requires_ar \
               AND review_session_v2_projection.requires_qa=excluded.requires_qa \
               AND review_session_v2_projection.input_fingerprint=excluded.input_fingerprint \
               AND review_session_v2_projection.ruleset_fingerprint=excluded.ruleset_fingerprint \
               AND review_session_v2_projection.source_revision=excluded.source_revision \
               AND review_session_v2_projection.inventory_generation=excluded.inventory_generation \
               AND review_session_v2_projection.clean_target=excluded.clean_target \
               AND review_session_v2_projection.max_attempts=excluded.max_attempts \
               AND review_session_v2_projection.max_valid_batches=excluded.max_valid_batches \
               AND review_session_v2_projection.max_remediations=excluded.max_remediations \
               AND review_session_v2_projection.max_wall_clock_minutes=excluded.max_wall_clock_minutes \
               AND review_session_v2_projection.started_at=excluded.started_at \
               AND review_session_v2_projection.deadline_at=excluded.deadline_at \
               AND excluded.attempts>=review_session_v2_projection.attempts \
               AND excluded.valid_batches>=review_session_v2_projection.valid_batches \
               AND excluded.remediations>=review_session_v2_projection.remediations \
               AND excluded.infra_failures>=review_session_v2_projection.infra_failures \
               AND excluded.protocol_failures>=review_session_v2_projection.protocol_failures \
               AND (review_session_v2_projection.status NOT IN('completed','stalled','invalidated','aborted') \
                    OR review_session_v2_projection.status=excluded.status)",
            params![
                write.workspace_id,
                write.work_item_id,
                projection_string(session, "reviewId")?,
                projection_optional_string(session, "parentReviewId")?,
                projection_string(session, "tier")?,
                projection_string(session, "status")?,
                projection_string(session, "authorSessionId")?,
                projection_string(session, "rootSessionId")?,
                projection_string(session, "repairClass")?,
                projection_string(&Value::Object(clean_policy.clone()), "finalProofRequirement")?,
                bool_i64(requires("general")),
                bool_i64(requires("be")),
                bool_i64(requires("ar")),
                bool_i64(requires("qa")),
                projection_digest(session, "inputFingerprint")?,
                projection_digest(session, "rulesetFingerprint")?,
                to_i64(projection_u64(session, "sourceRevision")?)?,
                to_i64(projection_u64(session, "inventoryGeneration")?)?,
                to_i64(projection_u64(&Value::Object(counters.clone()), "attempts")?)?,
                to_i64(projection_u64(&Value::Object(counters.clone()), "validBatches")?)?,
                to_i64(projection_u64(&Value::Object(counters.clone()), "remediations")?)?,
                to_i64(projection_u64(&Value::Object(counters.clone()), "cleanStreak")?)?,
                to_i64(projection_u64(&Value::Object(clean_policy.clone()), "cleanTarget")?)?,
                to_i64(projection_u64(&Value::Object(counters.clone()), "infraFailures")?)?,
                to_i64(projection_u64(&Value::Object(counters.clone()), "protocolFailures")?)?,
                to_i64(projection_u64(&Value::Object(budget.clone()), "maxAttempts")?)?,
                to_i64(projection_u64(&Value::Object(budget.clone()), "maxValidBatches")?)?,
                to_i64(projection_u64(&Value::Object(budget.clone()), "maxRemediations")?)?,
                to_i64(projection_u64(&Value::Object(budget.clone()), "maxWallClockMinutes")?)?,
                projection_string(session, "startedAt")?,
                projection_string(session, "deadlineAt")?,
                terminal_at,
                to_i64(write.event_sequence)?,
                committed_at,
            ],
        )
        .map_err(sqlite_error)?;
    if changed != 1 {
        return Err(review_projection_error(
            "review session projection rejected a stale or conflicting event",
        ));
    }
    Ok(())
}

fn persist_review_batch(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    session: &Value,
    batch: &Value,
    batch_receipt: &Value,
    committed_at: &str,
) -> RuntimeResult<()> {
    let changed = transaction
        .execute(
            "INSERT INTO review_batch_v2_projection(\
                workspace_id,review_id,batch_id,schema_version,input_fingerprint,ruleset_fingerprint,\
                latest_attempt_id,latest_status,required_specialty_count,effective_contribution_count,\
                finding_count,closed,valid_batch_ordinal,latest_receipt_digest,first_event_seq,\
                last_event_seq,created_at,updated_at\
             ) VALUES(?1,?2,?3,2,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17) \
             ON CONFLICT(workspace_id,review_id,batch_id) DO UPDATE SET \
                latest_attempt_id=excluded.latest_attempt_id,latest_status=excluded.latest_status,\
                effective_contribution_count=excluded.effective_contribution_count,\
                finding_count=excluded.finding_count,closed=excluded.closed,\
                valid_batch_ordinal=excluded.valid_batch_ordinal,\
                latest_receipt_digest=excluded.latest_receipt_digest,\
                last_event_seq=excluded.last_event_seq,updated_at=excluded.updated_at \
             WHERE excluded.last_event_seq>=review_batch_v2_projection.last_event_seq \
               AND review_batch_v2_projection.input_fingerprint=excluded.input_fingerprint \
               AND review_batch_v2_projection.ruleset_fingerprint=excluded.ruleset_fingerprint \
               AND review_batch_v2_projection.required_specialty_count=excluded.required_specialty_count \
               AND (review_batch_v2_projection.closed=0 OR excluded.closed=1) \
               AND (review_batch_v2_projection.valid_batch_ordinal IS NULL \
                    OR review_batch_v2_projection.valid_batch_ordinal=excluded.valid_batch_ordinal)",
            params![
                write.workspace_id,
                projection_string(session, "reviewId")?,
                projection_string(batch, "batchId")?,
                projection_digest(batch, "inputFingerprint")?,
                projection_digest(batch, "rulesetFingerprint")?,
                projection_string(batch, "latestAttemptId")?,
                projection_batch_status(batch, "latestStatus")?,
                to_i64(projection_count(session, "requiredSpecialties")?)?,
                to_i64(projection_count(batch, "retainedContributions")?)?,
                to_i64(projection_count(batch, "findingFingerprints")?)?,
                bool_i64(projection_bool(batch, "closed")?),
                projection_optional_u64(batch, "validBatchOrdinal")?
                    .map(to_i64)
                    .transpose()?,
                projection_digest(batch_receipt, "receiptDigest")?,
                to_i64(write.event_sequence)?,
                to_i64(write.event_sequence)?,
                projection_string(batch_receipt, "observedAt")?,
                committed_at,
            ],
        )
        .map_err(sqlite_error)?;
    if changed != 1 {
        return Err(review_projection_error(
            "review batch projection rejected a stale or conflicting event",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persist_review_attempt(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    session: &Value,
    batch: &Value,
    attempt: &Value,
    batch_receipt: &Value,
    committed_at: &str,
) -> RuntimeResult<()> {
    parse_timestamp(committed_at)?;
    let review_id = projection_string(session, "reviewId")?;
    let batch_id = projection_string(batch, "batchId")?;
    let attempt_id = projection_string(attempt, "attemptId")?;
    let attempt_ordinal = projection_u64(attempt, "attemptOrdinal")?;
    let status = projection_batch_status(batch_receipt, "status")?;
    let required_specialties = projection_array(batch_receipt, "requiredSpecialties")?;
    let session_required = projection_array(session, "requiredSpecialties")?;
    let completed_specialties = projection_array(batch_receipt, "completedSpecialties")?;
    let presented = projection_array(attempt, "contributions")?;
    let retained = projection_array(batch, "retainedContributions")?;
    let finding_count = projection_count(batch_receipt, "findingFingerprints")?;
    let required_count = u64::try_from(required_specialties.len())
        .map_err(|_| review_projection_error("review required specialty set is too large"))?;
    let retained_count = u64::try_from(retained.len())
        .map_err(|_| review_projection_error("review retained specialty set is too large"))?;
    let presented_count = u64::try_from(presented.len())
        .map_err(|_| review_projection_error("review attempted specialty set is too large"))?;
    let valid = matches!(status, "valid_clean" | "valid_findings");
    let (attempted_count, effective_count) = if valid {
        (required_count, required_count)
    } else {
        (presented_count.max(retained_count), retained_count)
    };
    let attempt_key = projection_string(attempt, "idempotencyKey")?;
    let receipt_key = projection_string(batch_receipt, "idempotencyKey")?;
    let attempt_digest = projection_digest(batch_receipt, "attemptDigest")?;
    let canonical_attempt = serde_json::to_vec(&write.attempt)
        .map_err(|_| review_projection_error("review attempt could not be canonicalized"))?;
    if review_id != projection_string(attempt, "reviewId")?
        || review_id != projection_string(batch_receipt, "reviewId")?
        || batch_id != projection_string(attempt, "batchId")?
        || batch_id != projection_string(batch_receipt, "batchId")?
        || attempt_id != projection_string(batch_receipt, "attemptId")?
        || attempt_ordinal != projection_u64(batch_receipt, "attemptOrdinal")?
        || attempt_key != receipt_key
        || projection_string(batch, "latestAttemptId")? != attempt_id
        || projection_batch_status(batch, "latestStatus")? != status
        || required_specialties != session_required
        || (valid && completed_specialties.len() != required_specialties.len())
        || hex::encode(Sha256::digest(canonical_attempt)) != attempt_digest
    {
        return Err(review_projection_error(
            "review attempt projection is inconsistent with its batch receipt",
        ));
    }
    let started_at = projection_string(attempt, "observedAt")?;
    let completed_at = projection_string(batch_receipt, "observedAt")?;
    if parse_timestamp(started_at)? > parse_timestamp(completed_at)? {
        return Err(review_projection_error(
            "review attempt timestamps are inconsistent",
        ));
    }
    let changed = transaction
        .execute(
            "INSERT INTO review_attempt_v2_projection(\
                workspace_id,review_id,batch_id,attempt_id,schema_version,attempt_ordinal,status,\
                input_fingerprint,ruleset_fingerprint,required_specialty_count,\
                attempted_specialty_count,effective_specialty_count,finding_count,\
                idempotency_key_digest,payload_digest,receipt_digest,started_at,completed_at,event_seq\
             ) VALUES(?1,?2,?3,?4,2,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18) \
             ON CONFLICT(workspace_id,review_id,batch_id,attempt_id) DO UPDATE SET \
                attempt_id=excluded.attempt_id \
             WHERE review_attempt_v2_projection.attempt_ordinal=excluded.attempt_ordinal \
               AND review_attempt_v2_projection.status=excluded.status \
               AND review_attempt_v2_projection.input_fingerprint=excluded.input_fingerprint \
               AND review_attempt_v2_projection.ruleset_fingerprint=excluded.ruleset_fingerprint \
               AND review_attempt_v2_projection.required_specialty_count=excluded.required_specialty_count \
               AND review_attempt_v2_projection.attempted_specialty_count=excluded.attempted_specialty_count \
               AND review_attempt_v2_projection.effective_specialty_count=excluded.effective_specialty_count \
               AND review_attempt_v2_projection.finding_count=excluded.finding_count \
               AND review_attempt_v2_projection.idempotency_key_digest=excluded.idempotency_key_digest \
               AND review_attempt_v2_projection.payload_digest=excluded.payload_digest \
               AND review_attempt_v2_projection.receipt_digest=excluded.receipt_digest \
               AND review_attempt_v2_projection.started_at=excluded.started_at \
               AND review_attempt_v2_projection.completed_at=excluded.completed_at \
               AND review_attempt_v2_projection.event_seq=excluded.event_seq",
            params![
                &write.workspace_id,
                review_id,
                batch_id,
                attempt_id,
                to_i64(attempt_ordinal)?,
                status,
                projection_digest(attempt, "inputFingerprint")?,
                projection_digest(attempt, "rulesetFingerprint")?,
                to_i64(required_count)?,
                to_i64(attempted_count)?,
                to_i64(effective_count)?,
                to_i64(finding_count)?,
                hex::encode(Sha256::digest(attempt_key.as_bytes())),
                attempt_digest,
                projection_digest(batch_receipt, "receiptDigest")?,
                started_at,
                completed_at,
                to_i64(write.event_sequence)?,
            ],
        )
        .map_err(sqlite_error)?;
    if changed != 1 {
        return Err(review_projection_error(
            "review attempt projection rejected a conflicting replay",
        ));
    }
    Ok(())
}

fn persist_review_contributions(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    batch: &Value,
) -> RuntimeResult<()> {
    let review_id = projection_string(batch, "reviewId")?;
    let batch_id = projection_string(batch, "batchId")?;
    for contribution in projection_array(batch, "retainedContributions")? {
        let reviewer = projection_child(contribution, "reviewer")?;
        let specialty = projection_string(reviewer, "specialty")?;
        let granted = projection_array(reviewer, "grantedSpecialties")?;
        let outcome = projection_string(contribution, "outcome")?;
        let source_attempt_id = projection_string(contribution, "sourceAttemptId")?;
        if projection_string(reviewer, "agentRole")? != "reviewer"
            || projection_u64(reviewer, "lineageDepth")? != 2
            || granted.len() != 1
            || granted.first().and_then(Value::as_str) != Some(specialty)
            || !matches!(outcome, "clean" | "findings")
            || projection_digest(contribution, "inputFingerprint")?
                != projection_digest(batch, "inputFingerprint")?
            || projection_digest(contribution, "rulesetFingerprint")?
                != projection_digest(batch, "rulesetFingerprint")?
        {
            return Err(review_projection_error(
                "review effective contribution is not strictly attested",
            ));
        }
        let finding_count = projection_count(contribution, "findings")?;
        if (outcome == "clean") != (finding_count == 0) {
            return Err(review_projection_error(
                "review contribution outcome and findings are inconsistent",
            ));
        }
        let (source_event_seq, source_completed_at) = review_attempt_source(
            transaction,
            &write.workspace_id,
            review_id,
            batch_id,
            source_attempt_id,
        )?;
        let changed = transaction
            .execute(
                "INSERT INTO review_effective_contribution_v2_projection(\
                    workspace_id,review_id,batch_id,specialty,schema_version,source_attempt_id,\
                    agent_role,outcome,physical_session_id,root_session_id,delegation_id,lineage_depth,\
                    attestation_ref,attestation_digest,specialty_grant_digest,report_digest,\
                    contribution_digest,input_fingerprint,ruleset_fingerprint,finding_count,\
                    event_seq,created_at,updated_at\
                 ) VALUES(?1,?2,?3,?4,2,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?21) \
                 ON CONFLICT(workspace_id,review_id,batch_id,specialty) DO UPDATE SET \
                    specialty=excluded.specialty \
                 WHERE review_effective_contribution_v2_projection.source_attempt_id=excluded.source_attempt_id \
                   AND review_effective_contribution_v2_projection.agent_role=excluded.agent_role \
                   AND review_effective_contribution_v2_projection.outcome=excluded.outcome \
                   AND review_effective_contribution_v2_projection.physical_session_id=excluded.physical_session_id \
                   AND review_effective_contribution_v2_projection.root_session_id=excluded.root_session_id \
                   AND review_effective_contribution_v2_projection.delegation_id=excluded.delegation_id \
                   AND review_effective_contribution_v2_projection.lineage_depth=excluded.lineage_depth \
                   AND review_effective_contribution_v2_projection.attestation_ref=excluded.attestation_ref \
                   AND review_effective_contribution_v2_projection.attestation_digest=excluded.attestation_digest \
                   AND review_effective_contribution_v2_projection.specialty_grant_digest=excluded.specialty_grant_digest \
                   AND review_effective_contribution_v2_projection.report_digest=excluded.report_digest \
                   AND review_effective_contribution_v2_projection.contribution_digest=excluded.contribution_digest \
                   AND review_effective_contribution_v2_projection.input_fingerprint=excluded.input_fingerprint \
                   AND review_effective_contribution_v2_projection.ruleset_fingerprint=excluded.ruleset_fingerprint \
                   AND review_effective_contribution_v2_projection.finding_count=excluded.finding_count \
                   AND review_effective_contribution_v2_projection.event_seq=excluded.event_seq \
                   AND review_effective_contribution_v2_projection.created_at=excluded.created_at \
                   AND review_effective_contribution_v2_projection.updated_at=excluded.updated_at",
                params![
                    &write.workspace_id,
                    review_id,
                    batch_id,
                    specialty,
                    source_attempt_id,
                    projection_string(reviewer, "agentRole")?,
                    outcome,
                    projection_string(reviewer, "physicalSessionId")?,
                    projection_string(reviewer, "rootSessionId")?,
                    projection_string(reviewer, "delegationId")?,
                    to_i64(projection_u64(reviewer, "lineageDepth")?)?,
                    projection_string(reviewer, "attestationRef")?,
                    projection_digest(reviewer, "attestationDigest")?,
                    projection_digest(reviewer, "specialtyGrantDigest")?,
                    projection_digest(contribution, "reportDigest")?,
                    projection_digest(contribution, "contributionDigest")?,
                    projection_digest(contribution, "inputFingerprint")?,
                    projection_digest(contribution, "rulesetFingerprint")?,
                    to_i64(finding_count)?,
                    source_event_seq,
                    source_completed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if changed != 1 {
            return Err(review_projection_error(
                "review contribution projection rejected a conflicting replay",
            ));
        }
    }
    Ok(())
}

fn persist_review_findings(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    batch: &Value,
) -> RuntimeResult<()> {
    let review_id = projection_string(batch, "reviewId")?;
    let batch_id = projection_string(batch, "batchId")?;
    let mut findings = Vec::<(String, &Value, &str, &str, i64, String)>::new();
    let mut seen = BTreeMap::<String, ()>::new();
    for contribution in projection_array(batch, "retainedContributions")? {
        let reviewer = projection_child(contribution, "reviewer")?;
        let specialty = projection_string(reviewer, "specialty")?;
        let source_attempt_id = projection_string(contribution, "sourceAttemptId")?;
        let (source_event_seq, source_completed_at) = review_attempt_source(
            transaction,
            &write.workspace_id,
            review_id,
            batch_id,
            source_attempt_id,
        )?;
        for finding in projection_array(contribution, "findings")? {
            let canonical = serde_json::to_vec(finding).map_err(|_| {
                review_projection_error("review finding could not be canonicalized")
            })?;
            let fingerprint = hex::encode(Sha256::digest(canonical));
            if seen.insert(fingerprint.clone(), ()).is_none() {
                findings.push((
                    fingerprint,
                    finding,
                    specialty,
                    source_attempt_id,
                    source_event_seq,
                    source_completed_at.clone(),
                ));
            }
        }
    }
    let expected = projection_array(batch, "findingFingerprints")?;
    if expected.len() != findings.len()
        || expected.iter().zip(&findings).any(|(expected, actual)| {
            expected.as_str() != Some(actual.0.as_str()) || !is_digest(actual.0.as_str())
        })
    {
        return Err(review_projection_error(
            "review finding fingerprints do not match canonical findings",
        ));
    }
    for (fingerprint, finding, specialty, attempt_id, event_seq, observed_at) in findings {
        let changed = transaction
            .execute(
                "INSERT INTO review_finding_v2_projection(\
                    workspace_id,review_id,finding_fingerprint,schema_version,first_batch_id,\
                    last_batch_id,first_seen_attempt_id,last_seen_attempt_id,\
                    first_reported_specialty,code,severity,summary,status,first_event_seq,\
                    last_event_seq,created_at,updated_at\
                 ) VALUES(?1,?2,?3,2,?4,?4,?5,?5,?6,?7,?8,?9,'open',?10,?10,?11,?11) \
                 ON CONFLICT(workspace_id,review_id,finding_fingerprint) DO UPDATE SET \
                    last_batch_id=excluded.last_batch_id,\
                    last_seen_attempt_id=excluded.last_seen_attempt_id,\
                    last_event_seq=excluded.last_event_seq,updated_at=excluded.updated_at \
                 WHERE review_finding_v2_projection.code=excluded.code \
                   AND review_finding_v2_projection.severity=excluded.severity \
                   AND review_finding_v2_projection.summary=excluded.summary \
                   AND excluded.last_event_seq>=review_finding_v2_projection.last_event_seq",
                params![
                    &write.workspace_id,
                    review_id,
                    fingerprint,
                    batch_id,
                    attempt_id,
                    specialty,
                    projection_string(finding, "code")?,
                    projection_string(finding, "severity")?,
                    projection_string(finding, "summary")?,
                    event_seq,
                    observed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if changed != 1 {
            return Err(review_projection_error(
                "review finding projection rejected a stale or conflicting replay",
            ));
        }
    }
    Ok(())
}

fn persist_review_remediation(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    session: &Value,
    attempt: &Value,
    committed_at: &str,
) -> RuntimeResult<()> {
    let Some(remediation) = attempt.get("remediation").filter(|value| !value.is_null()) else {
        return Ok(());
    };
    if !remediation.is_object() {
        return Err(review_projection_error(
            "review remediation projection is malformed",
        ));
    }
    // The remediation is carried on the first attempt of the CHILD session, but
    // migration 0009 keys it on the PARENT review and the parent's closed
    // findings batch. Derive the parent identity from the typed
    // `parentReviewId` and never from the attempt's own review.
    let child_review_id = projection_string(session, "reviewId")?;
    let parent_review_id =
        projection_optional_string(session, "parentReviewId")?.ok_or_else(|| {
            review_projection_error("review remediation requires a typed parent review session")
        })?;
    if parent_review_id == child_review_id
        || projection_string(remediation, "nextReviewId")? != child_review_id
    {
        return Err(review_projection_error(
            "review remediation parent and child review identities do not join",
        ));
    }
    let finding_batch_id = projection_string(remediation, "findingBatchId")?;
    require_closed_findings_batch(transaction, write, parent_review_id, finding_batch_id)?;
    let source_revision = parent_review_source_revision(transaction, write, parent_review_id)?;
    let target_revision = projection_u64(session, "sourceRevision")?;
    if target_revision <= source_revision {
        return Err(review_projection_error(
            "review remediation must advance the parent source revision",
        ));
    }
    let changed = transaction
        .execute(
            "INSERT INTO review_remediation_v2_projection(\
                workspace_id,review_id,finding_batch_id,plan_fingerprint,new_input_fingerprint,\
                schema_version,next_review_id,remediation_digest,source_revision,target_revision,\
                event_seq,created_at\
             ) VALUES(?1,?2,?3,?4,?5,2,?6,?7,?8,?9,?10,?11) \
             ON CONFLICT(workspace_id,review_id,finding_batch_id,plan_fingerprint,new_input_fingerprint) \
             DO UPDATE SET finding_batch_id=excluded.finding_batch_id \
             WHERE review_remediation_v2_projection.next_review_id=excluded.next_review_id \
               AND review_remediation_v2_projection.remediation_digest=excluded.remediation_digest \
               AND review_remediation_v2_projection.source_revision=excluded.source_revision \
               AND review_remediation_v2_projection.target_revision=excluded.target_revision \
               AND review_remediation_v2_projection.event_seq=excluded.event_seq \
               AND review_remediation_v2_projection.created_at=excluded.created_at",
            params![
                &write.workspace_id,
                parent_review_id,
                finding_batch_id,
                projection_digest(remediation, "planFingerprint")?,
                projection_digest(remediation, "newInputFingerprint")?,
                child_review_id,
                projection_digest(remediation, "remediationDigest")?,
                to_i64(source_revision)?,
                to_i64(target_revision)?,
                to_i64(write.event_sequence)?,
                committed_at,
            ],
        )
        .map_err(sqlite_error)?;
    if changed != 1 {
        return Err(review_projection_error(
            "review remediation projection rejected a conflicting replay",
        ));
    }
    Ok(())
}

/// Returns the projected `source_revision` of the parent review session.
///
/// The parent row is written by an earlier committed `review.record` event, so a
/// missing parent means the projection history is incomplete and the remediation
/// must fail closed instead of inventing a revision.
fn parent_review_source_revision(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    parent_review_id: &str,
) -> RuntimeResult<u64> {
    let revision: Option<i64> = transaction
        .query_row(
            "SELECT source_revision FROM review_session_v2_projection \
             WHERE workspace_id=?1 AND review_id=?2 AND work_item_id=?3",
            params![&write.workspace_id, parent_review_id, &write.work_item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?;
    let revision = revision.ok_or_else(|| {
        review_projection_error("review remediation parent session projection is missing")
    })?;
    from_i64(revision)
}

/// Fails closed unless the remediated batch is the parent review's own closed
/// findings batch.
fn require_closed_findings_batch(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    parent_review_id: &str,
    finding_batch_id: &str,
) -> RuntimeResult<()> {
    let batch: Option<(String, i64)> = transaction
        .query_row(
            "SELECT latest_status,closed FROM review_batch_v2_projection \
             WHERE workspace_id=?1 AND review_id=?2 AND batch_id=?3",
            params![&write.workspace_id, parent_review_id, finding_batch_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    let Some((latest_status, closed)) = batch else {
        return Err(review_projection_error(
            "review remediation references a missing parent findings batch",
        ));
    };
    if latest_status != "valid_findings" || closed != 1 {
        return Err(review_projection_error(
            "review remediation parent batch is not a closed findings batch",
        ));
    }
    Ok(())
}

fn persist_review_exit_receipt(
    transaction: &Transaction<'_>,
    write: &ReviewProjectionWrite,
    exit_receipt: &Value,
    committed_at: &str,
) -> RuntimeResult<()> {
    parse_timestamp(committed_at)?;
    let counters = projection_child(exit_receipt, "counters")?;
    let final_proof = projection_child(exit_receipt, "finalProof")?;
    let project_authority = projection_child(exit_receipt, "projectAuthority")?;
    let proof_kind = projection_string(final_proof, "kind")?;
    let proof_digest = projection_optional_digest(final_proof, "digest")?;
    let proof_revision = projection_optional_u64(final_proof, "sourceRevision")?;
    let proof_input = projection_optional_digest(final_proof, "inputFingerprint")?;
    let proof_ruleset = projection_optional_digest(final_proof, "rulesetFingerprint")?;
    let proof_observed_at = projection_optional_string(final_proof, "observedAt")?;
    let proof_absent = proof_digest.is_none()
        && proof_revision.is_none()
        && proof_input.is_none()
        && proof_ruleset.is_none()
        && proof_observed_at.is_none();
    let proof_complete = proof_digest.is_some()
        && proof_revision.is_some()
        && proof_input.is_some()
        && proof_ruleset.is_some()
        && proof_observed_at.is_some();
    if (proof_kind == "none" && !proof_absent) || (proof_kind != "none" && !proof_complete) {
        return Err(review_projection_error(
            "review exit final proof fields are inconsistent",
        ));
    }
    if projection_string(exit_receipt, "reviewId")? != write.session.review_id().to_string()
        || projection_string(exit_receipt, "reviewId")? != write.batch.review_id().to_string()
    {
        return Err(review_projection_error(
            "review exit receipt identity is inconsistent",
        ));
    }
    let created_at = projection_string(exit_receipt, "createdAt")?;
    parse_timestamp(created_at)?;
    let changed = transaction
        .execute(
            "INSERT INTO review_exit_receipt_v2_projection(\
                workspace_id,work_item_id,review_id,schema_version,tier,session_status,disposition,\
                input_fingerprint,observed_input_fingerprint,ruleset_fingerprint,source_revision,\
                inventory_generation,policy_digest,required_specialty_count,completed_specialty_count,\
                finding_count,attempts,valid_batches,clean_streak,remediations,clean_target,\
                last_batch_id,last_attempt_id,last_attempt_status,zero_finding_batch_receipt_digest,\
                final_proof_kind,final_proof_digest,final_proof_state_revision,\
                final_proof_input_fingerprint,final_proof_ruleset_fingerprint,final_proof_observed_at,\
                project_receipt_ref,active_manifest_digest,state_receipt_ref_digest,\
                journal_mutation_id,receipt_digest,event_seq,created_at\
             ) VALUES(?1,?2,?3,2,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37) \
             ON CONFLICT(workspace_id,review_id) DO UPDATE SET review_id=excluded.review_id \
             WHERE review_exit_receipt_v2_projection.work_item_id=excluded.work_item_id \
               AND review_exit_receipt_v2_projection.tier=excluded.tier \
               AND review_exit_receipt_v2_projection.session_status=excluded.session_status \
               AND review_exit_receipt_v2_projection.disposition=excluded.disposition \
               AND review_exit_receipt_v2_projection.input_fingerprint=excluded.input_fingerprint \
               AND review_exit_receipt_v2_projection.observed_input_fingerprint=excluded.observed_input_fingerprint \
               AND review_exit_receipt_v2_projection.ruleset_fingerprint=excluded.ruleset_fingerprint \
               AND review_exit_receipt_v2_projection.source_revision=excluded.source_revision \
               AND review_exit_receipt_v2_projection.inventory_generation=excluded.inventory_generation \
               AND review_exit_receipt_v2_projection.policy_digest=excluded.policy_digest \
               AND review_exit_receipt_v2_projection.required_specialty_count=excluded.required_specialty_count \
               AND review_exit_receipt_v2_projection.completed_specialty_count=excluded.completed_specialty_count \
               AND review_exit_receipt_v2_projection.finding_count=excluded.finding_count \
               AND review_exit_receipt_v2_projection.attempts=excluded.attempts \
               AND review_exit_receipt_v2_projection.valid_batches=excluded.valid_batches \
               AND review_exit_receipt_v2_projection.clean_streak=excluded.clean_streak \
               AND review_exit_receipt_v2_projection.remediations=excluded.remediations \
               AND review_exit_receipt_v2_projection.clean_target=excluded.clean_target \
               AND review_exit_receipt_v2_projection.last_batch_id=excluded.last_batch_id \
               AND review_exit_receipt_v2_projection.last_attempt_id=excluded.last_attempt_id \
               AND review_exit_receipt_v2_projection.last_attempt_status=excluded.last_attempt_status \
               AND review_exit_receipt_v2_projection.zero_finding_batch_receipt_digest IS excluded.zero_finding_batch_receipt_digest \
               AND review_exit_receipt_v2_projection.final_proof_kind=excluded.final_proof_kind \
               AND review_exit_receipt_v2_projection.final_proof_digest IS excluded.final_proof_digest \
               AND review_exit_receipt_v2_projection.final_proof_state_revision IS excluded.final_proof_state_revision \
               AND review_exit_receipt_v2_projection.final_proof_input_fingerprint IS excluded.final_proof_input_fingerprint \
               AND review_exit_receipt_v2_projection.final_proof_ruleset_fingerprint IS excluded.final_proof_ruleset_fingerprint \
               AND review_exit_receipt_v2_projection.final_proof_observed_at IS excluded.final_proof_observed_at \
               AND review_exit_receipt_v2_projection.project_receipt_ref=excluded.project_receipt_ref \
               AND review_exit_receipt_v2_projection.active_manifest_digest=excluded.active_manifest_digest \
               AND review_exit_receipt_v2_projection.state_receipt_ref_digest=excluded.state_receipt_ref_digest \
               AND review_exit_receipt_v2_projection.journal_mutation_id=excluded.journal_mutation_id \
               AND review_exit_receipt_v2_projection.receipt_digest=excluded.receipt_digest \
               AND review_exit_receipt_v2_projection.event_seq=excluded.event_seq \
               AND review_exit_receipt_v2_projection.created_at=excluded.created_at",
            params![
                &write.workspace_id,
                &write.work_item_id,
                projection_string(exit_receipt, "reviewId")?,
                projection_string(exit_receipt, "tier")?,
                projection_string(exit_receipt, "sessionStatus")?,
                projection_string(exit_receipt, "disposition")?,
                projection_digest(exit_receipt, "inputFingerprint")?,
                projection_digest(exit_receipt, "observedInputFingerprint")?,
                projection_digest(exit_receipt, "rulesetFingerprint")?,
                to_i64(projection_u64(exit_receipt, "sourceRevision")?)?,
                to_i64(projection_u64(exit_receipt, "inventoryGeneration")?)?,
                projection_digest(exit_receipt, "policyDigest")?,
                to_i64(projection_count(exit_receipt, "requiredSpecialties")?)?,
                to_i64(projection_count(exit_receipt, "completedSpecialties")?)?,
                to_i64(projection_u64(exit_receipt, "findingCount")?)?,
                to_i64(projection_u64(counters, "attempts")?)?,
                to_i64(projection_u64(counters, "validBatches")?)?,
                to_i64(projection_u64(counters, "cleanStreak")?)?,
                to_i64(projection_u64(counters, "remediations")?)?,
                to_i64(projection_u64(exit_receipt, "cleanTarget")?)?,
                projection_string(exit_receipt, "lastBatchId")?,
                projection_string(exit_receipt, "lastAttemptId")?,
                projection_batch_status(exit_receipt, "lastAttemptStatus")?,
                projection_optional_digest(exit_receipt, "zeroFindingBatchReceiptDigest")?,
                proof_kind,
                proof_digest,
                proof_revision.map(to_i64).transpose()?,
                proof_input,
                proof_ruleset,
                proof_observed_at,
                projection_string(project_authority, "projectReceiptRef")?,
                projection_digest(project_authority, "activeManifestDigest")?,
                projection_digest(project_authority, "stateReceiptRefDigest")?,
                projection_string(project_authority, "journalMutationId")?,
                projection_digest(exit_receipt, "receiptDigest")?,
                to_i64(write.event_sequence)?,
                created_at,
            ],
        )
        .map_err(sqlite_error)?;
    if changed != 1 {
        return Err(review_projection_error(
            "review exit receipt projection rejected a conflicting replay",
        ));
    }
    Ok(())
}

impl PersistencePort for SqliteRuntimePersistence {
    fn event_store_id(&self) -> RuntimeResult<EventStoreId> {
        Ok(self.repository.event_store_id())
    }

    fn latest_event_sequence(&self) -> RuntimeResult<u64> {
        let value: i64 = self
            .connection()?
            .query_row(
                "SELECT COALESCE(MAX(event_seq),0) FROM runtime_event",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        from_i64(value)
    }

    fn append_event(&self, event: DurableEvent) -> RuntimeResult<DurableEvent> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let event = insert_event(&transaction, self.repository.event_store_id(), event)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(event)
    }

    fn commit_event_and_receipt(
        &self,
        event: DurableEvent,
        mut receipt: IdempotencyReceipt,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt)> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        if let Some(existing) = load_receipt_tx(&transaction, &receipt.scope, &receipt.key)? {
            if existing.request_digest != receipt.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "idempotency key was reused with a different payload",
                ));
            }
            let event = load_event_tx(&transaction, existing.event_seq)?.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "receipt points to a missing runtime event",
                )
            })?;
            transaction.commit().map_err(sqlite_error)?;
            return Ok((event, existing));
        }
        let event = insert_event(&transaction, self.repository.event_store_id(), event)?;
        receipt.event_seq = event.event_seq;
        transaction
            .execute(
                "INSERT INTO runtime_generic_receipt_v1(scope,key,request_digest,response_json,event_seq) VALUES(?1,?2,?3,?4,?5)",
                params![receipt.scope, receipt.key, receipt.request_digest, receipt.response_json, to_i64(receipt.event_seq)?],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok((event, receipt))
    }

    fn events_after(&self, after: u64, limit: usize) -> RuntimeResult<Vec<DurableEvent>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT event_seq,event_store_id,boot_id,workspace_id,session_id,work_item_id,event_type,payload_json,payload_digest FROM runtime_event WHERE event_seq>?1 ORDER BY event_seq LIMIT ?2")
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map(
                params![
                    to_i64(after)?,
                    i64::try_from(limit).map_err(|_| range_error())?
                ],
                row_to_event,
            )
            .map_err(sqlite_error)?;
        rows.map(|row| row.map_err(sqlite_error)).collect()
    }

    fn oldest_event_sequence(&self) -> RuntimeResult<u64> {
        let value: i64 = self
            .connection()?
            .query_row(
                "SELECT COALESCE(MIN(event_seq),0) FROM runtime_event",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        from_i64(value)
    }

    fn load_receipt(&self, scope: &str, key: &str) -> RuntimeResult<Option<IdempotencyReceipt>> {
        let connection = self.connection()?;
        load_receipt_connection(&connection, scope, key)
    }

    fn store_receipt(&self, receipt: &IdempotencyReceipt) -> RuntimeResult<()> {
        let connection = self.connection()?;
        if let Some(existing) = load_receipt_connection(&connection, &receipt.scope, &receipt.key)?
        {
            if existing.request_digest == receipt.request_digest {
                return Ok(());
            }
            return Err(RuntimeError::new(
                StableErrorCode::IdempotencyKeyReused,
                "idempotency key was reused with a different payload",
            ));
        }
        connection
            .execute(
                "INSERT INTO runtime_generic_receipt_v1(scope,key,request_digest,response_json,event_seq) VALUES(?1,?2,?3,?4,?5)",
                params![receipt.scope, receipt.key, receipt.request_digest, receipt.response_json, to_i64(receipt.event_seq)?],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn load_record(&self, namespace: &str, key: &str) -> RuntimeResult<Option<Value>> {
        let value: Option<String> = self
            .connection()?
            .query_row(
                "SELECT value_json FROM runtime_record_v1 WHERE namespace=?1 AND key=?2",
                params![namespace, key],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        value
            .map(|item| serde_json::from_str(&item).map_err(canonical_error))
            .transpose()
    }

    fn list_records(&self, namespace: &str) -> RuntimeResult<Vec<(String, Value)>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT key,value_json FROM runtime_record_v1 WHERE namespace=?1 ORDER BY key")
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([namespace], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sqlite_error)?;
        rows.map(|row| {
            let (key, json) = row.map_err(sqlite_error)?;
            let value = serde_json::from_str(&json).map_err(canonical_error)?;
            Ok((key, value))
        })
        .collect()
    }

    fn store_record(&self, namespace: &str, key: &str, value: &Value) -> RuntimeResult<()> {
        let json = serde_json::to_string(value).map_err(canonical_error)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO runtime_record_v1(namespace,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(namespace,key) DO UPDATE SET value_json=excluded.value_json",
                params![namespace, key, json],
            )
            .map_err(sqlite_error)?;
        mirror_typed_runtime_record(&transaction, namespace, key, value)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(())
    }

    fn commit_identity_bundle(
        &self,
        transition: RuntimeIdentityTransition,
    ) -> RuntimeResult<RuntimeIdentitySnapshot> {
        validate_identity_transition(&transition)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        if let Some(mut snapshot) = load_identity_receipt(&transaction, &transition)? {
            snapshot.replayed = true;
            transaction.commit().map_err(sqlite_error)?;
            return Ok(snapshot);
        }
        validate_identity_cas(&transaction, &transition)?;
        write_workspace(&transaction, &transition.snapshot.workspace)?;
        if let Some(session) = transition.snapshot.session.as_ref() {
            write_session(&transaction, session)?;
        }
        if let Some(delegation) = transition.snapshot.delegation.as_ref() {
            write_delegation(&transaction, delegation)?;
        }
        if let Some(binding) = transition.snapshot.host_action.as_ref() {
            transaction
                .execute(
                    "INSERT INTO delegation_host_action_v1(workspace_id,delegation_id,host_action_id,parent_session_id,action_digest,created_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(workspace_id,delegation_id) DO UPDATE SET host_action_id=excluded.host_action_id,parent_session_id=excluded.parent_session_id,action_digest=excluded.action_digest",
                    params![
                        binding.workspace_id,
                        binding.delegation_id,
                        binding.host_action_id,
                        binding.parent_session_id,
                        binding.action_digest,
                        timestamp(binding.created_at_unix_ms)?,
                    ],
                )
                .map_err(sqlite_error)?;
        }
        if let Some(attestation) = transition.snapshot.attestation.as_ref() {
            write_attestation(&transaction, attestation)?;
        }
        let mut snapshot = transition.snapshot;
        snapshot.replayed = false;
        let snapshot_json = serde_json::to_string(&snapshot).map_err(canonical_error)?;
        if snapshot_json.len() > 65_536
            || contains_secret(&serde_json::to_value(&snapshot).map_err(canonical_error)?)
        {
            return Err(schema_error(
                "identity snapshot is too large or contains secret material",
            ));
        }
        let snapshot_digest = hex::encode(Sha256::digest(snapshot_json.as_bytes()));
        transaction
            .execute(
                "INSERT INTO runtime_identity_receipt_v1(identity_kind,workspace_id,scope_digest,idempotency_key,operation,request_digest,snapshot_schema_version,snapshot_json,snapshot_digest,snapshot_byte_len,created_at) VALUES(?1,?2,?3,?4,?5,?6,1,?7,?8,?9,?10)",
                params![
                    identity_kind(snapshot.identity_kind),
                    snapshot.workspace.workspace_id,
                    transition.scope_digest,
                    transition.idempotency_key,
                    transition.operation,
                    transition.request_digest,
                    snapshot_json,
                    snapshot_digest,
                    i64::try_from(snapshot_json.len()).map_err(|_| range_error())?,
                    timestamp(transition.committed_at_unix_ms)?,
                ],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(snapshot)
    }

    fn list_identity_snapshots(
        &self,
        kind: RuntimeIdentityKind,
    ) -> RuntimeResult<Vec<RuntimeIdentitySnapshot>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT snapshot_json,snapshot_digest,snapshot_byte_len FROM runtime_identity_receipt_v1 WHERE identity_kind=?1 ORDER BY rowid")
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([identity_kind(kind)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(sqlite_error)?;
        let mut latest = BTreeMap::new();
        for row in rows {
            let (json, digest, byte_len) = row.map_err(sqlite_error)?;
            let snapshot = decode_identity_snapshot(&json, &digest, byte_len)?;
            let key = identity_snapshot_key(&snapshot)?;
            latest.insert(key, snapshot);
        }
        Ok(latest.into_values().collect())
    }

    fn commit_job_transition(
        &self,
        transition: RuntimeJobTransition,
    ) -> RuntimeResult<RuntimeJobRecord> {
        commit_job_transition(self, transition)
    }

    fn load_job(&self, job_id: &str) -> RuntimeResult<Option<RuntimeJobRecord>> {
        let connection = self.connection()?;
        load_job_connection(&connection, job_id)
    }

    fn list_jobs(&self) -> RuntimeResult<Vec<RuntimeJobRecord>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT job_id FROM runtime_job_v1 ORDER BY job_id")
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sqlite_error)?;
        rows.map(|row| {
            let job_id = row.map_err(sqlite_error)?;
            load_job_connection(&connection, &job_id)?.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed runtime job disappeared during ordered scan",
                )
            })
        })
        .collect()
    }

    fn load_execution_checkpoint(
        &self,
        scope: &ExecutionCheckpointScope,
    ) -> RuntimeResult<Option<ExecutionCheckpointRecord>> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT capsule_digest,queue_digest,active_ordinal,no_progress_batches,source_cache_hits,source_cache_misses,updated_event_seq,updated_at FROM execution_supervisor_checkpoint_v1 WHERE workspace_id=?1 AND work_item_id=?2 AND session_id=?3",
                params![scope.workspace_id, scope.work_item_id, scope.session_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .map(
                |(
                    capsule_digest,
                    queue_digest,
                    active_ordinal,
                    no_progress_batches,
                    source_cache_hits,
                    source_cache_misses,
                    updated_event_seq,
                    updated_at,
                )| {
                    if !is_digest(&capsule_digest) || !is_digest(&queue_digest) {
                        return Err(schema_error(
                            "persisted execution checkpoint digest is malformed",
                        ));
                    }
                    Ok(ExecutionCheckpointRecord {
                        workspace_id: scope.workspace_id.clone(),
                        work_item_id: scope.work_item_id.clone(),
                        session_id: scope.session_id.clone(),
                        capsule_digest,
                        queue_digest,
                        active_ordinal: u32::try_from(active_ordinal).map_err(|_| {
                            schema_error("persisted execution checkpoint ordinal is out of range")
                        })?,
                        no_progress_batches: u8::try_from(no_progress_batches).map_err(
                            |_| {
                                schema_error(
                                    "persisted execution checkpoint no-progress counter is out of range",
                                )
                            },
                        )?,
                        source_cache_hits: from_i64(source_cache_hits)?,
                        source_cache_misses: from_i64(source_cache_misses)?,
                        updated_event_seq: from_i64(updated_event_seq)?,
                        updated_at_unix_ms: parse_timestamp(&updated_at)?,
                    })
                },
            )
            .transpose()
    }

    fn store_execution_checkpoint(&self, record: &ExecutionCheckpointRecord) -> RuntimeResult<()> {
        validate_execution_checkpoint_record(record)?;
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO execution_supervisor_checkpoint_v1(workspace_id,work_item_id,session_id,capsule_digest,queue_digest,active_ordinal,no_progress_batches,source_cache_hits,source_cache_misses,updated_event_seq,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(workspace_id,work_item_id,session_id) DO UPDATE SET capsule_digest=excluded.capsule_digest,queue_digest=excluded.queue_digest,active_ordinal=excluded.active_ordinal,no_progress_batches=excluded.no_progress_batches,source_cache_hits=excluded.source_cache_hits,source_cache_misses=excluded.source_cache_misses,updated_event_seq=excluded.updated_event_seq,updated_at=excluded.updated_at",
                params![
                    record.workspace_id,
                    record.work_item_id,
                    record.session_id,
                    record.capsule_digest,
                    record.queue_digest,
                    to_i64(u64::from(record.active_ordinal))?,
                    i64::from(record.no_progress_batches),
                    to_i64(record.source_cache_hits)?,
                    to_i64(record.source_cache_misses)?,
                    to_i64(record.updated_event_seq)?,
                    timestamp(record.updated_at_unix_ms)?,
                ],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn discard_execution_checkpoint(&self, scope: &ExecutionCheckpointScope) -> RuntimeResult<()> {
        let connection = self.connection()?;
        connection
            .execute(
                "DELETE FROM execution_supervisor_checkpoint_v1 WHERE workspace_id=?1 AND work_item_id=?2 AND session_id=?3",
                params![scope.workspace_id, scope.work_item_id, scope.session_id],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }
}

fn validate_execution_checkpoint_record(record: &ExecutionCheckpointRecord) -> RuntimeResult<()> {
    if record.workspace_id.is_empty()
        || record.workspace_id.len() > 128
        || record.work_item_id.is_empty()
        || record.work_item_id.len() > 128
        || record.session_id.is_empty()
        || record.session_id.len() > 128
        || !is_digest(&record.capsule_digest)
        || !is_digest(&record.queue_digest)
    {
        return Err(schema_error(
            "execution checkpoint record is unbounded or malformed",
        ));
    }
    Ok(())
}

fn canonical_json(value: &Value) -> RuntimeResult<(String, String, i64)> {
    if !value.is_object() || contains_secret(value) {
        return Err(schema_error(
            "durable JSON body must be a secret-free object",
        ));
    }
    let json = serde_json::to_string(value).map_err(canonical_error)?;
    if json.len() > 65_536 {
        return Err(schema_error("durable JSON body exceeds 65536 bytes"));
    }
    Ok((
        json.clone(),
        hex::encode(Sha256::digest(json.as_bytes())),
        i64::try_from(json.len()).map_err(|_| range_error())?,
    ))
}

fn optional_canonical_json(
    value: Option<&Value>,
) -> RuntimeResult<(Option<String>, Option<String>, Option<i64>)> {
    value
        .map(canonical_json)
        .transpose()
        .map(|value| match value {
            Some((json, digest, byte_len)) => (Some(json), Some(digest), Some(byte_len)),
            None => (None, None, None),
        })
}

fn optional_canonical_grant(
    grant: Option<&ScopedGrantWire>,
) -> RuntimeResult<(Option<String>, Option<String>, Option<i64>)> {
    grant
        .map(|grant| {
            let value = serde_json::to_value(grant).map_err(canonical_error)?;
            canonical_json(&value)
        })
        .transpose()
        .map(|value| match value {
            Some((json, digest, byte_len)) => (Some(json), Some(digest), Some(byte_len)),
            None => (None, None, None),
        })
}

fn decode_json_body(json: &str, digest: &str, byte_len: i64) -> RuntimeResult<Value> {
    if byte_len != i64::try_from(json.len()).map_err(|_| range_error())?
        || digest != hex::encode(Sha256::digest(json.as_bytes()))
        || json.len() > 65_536
    {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "durable JSON body digest or byte length is invalid",
        ));
    }
    let value: Value = serde_json::from_str(json).map_err(canonical_error)?;
    if !value.is_object() || contains_secret(&value) {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "durable JSON body is malformed or contains secret material",
        ));
    }
    Ok(value)
}

fn validate_job_record(record: &RuntimeJobRecord) -> RuntimeResult<()> {
    if record.job_id.is_empty()
        || record.job_id.len() > 128
        || record.entrypoint.is_empty()
        || record.entrypoint.len() > 128
        || record.submission_idempotency_key.is_empty()
        || record.submission_idempotency_key.len() > 256
        || !is_digest(&record.submission_scope_digest)
        || !is_digest(&record.submission_idempotency_key_digest)
        || !is_digest(&record.request_digest)
        || hex::encode(Sha256::digest(record.submission_idempotency_key.as_bytes()))
            != record.submission_idempotency_key_digest
        || record.source_revision.is_some() != record.input_fingerprint.is_some()
    {
        return Err(schema_error("typed runtime job record is malformed"));
    }
    canonical_json(&record.arguments)?;
    if let Some(result) = record.result.as_ref() {
        canonical_json(result)?;
    }
    if let Some(grant) = record.grant.as_ref() {
        canonical_json(&serde_json::to_value(grant).map_err(canonical_error)?)?;
    }
    let identity_empty = record.session_id.is_none()
        && record.root_session_id.is_none()
        && record.delegation_id.is_none()
        && record.agent_role.is_none()
        && record.context_generation.is_none()
        && record.submission_boot_id.is_none()
        && record.attestation_ref.is_none()
        && record.attestation_digest.is_none()
        && record.grant.is_none()
        && record.identity_digest.is_none();
    let identity_complete = record.session_id.is_some()
        && record.root_session_id.is_some()
        && record.agent_role.is_some()
        && record.context_generation.is_some()
        && record.submission_boot_id.is_some()
        && record.attestation_ref.is_some()
        && record.attestation_digest.as_deref().is_some_and(is_digest)
        && record.grant.is_some()
        && record.identity_digest.as_deref().is_some_and(is_digest);
    if !identity_empty && !identity_complete {
        return Err(schema_error("typed runtime job identity is incomplete"));
    }
    Ok(())
}

fn grant_entries(grant: &ScopedGrantWire) -> Vec<(&'static str, String)> {
    let mut values = Vec::new();
    values.extend(
        grant
            .operations
            .iter()
            .cloned()
            .map(|value| ("operation", value)),
    );
    values.extend(
        grant
            .capabilities
            .iter()
            .cloned()
            .map(|value| ("capability", value)),
    );
    values.extend(grant.paths.iter().map(|path| {
        (
            "path",
            match path {
                GrantPathWire::ProjectRoot => "/".to_owned(),
                GrantPathWire::Subtree { path } => path.clone(),
            },
        )
    }));
    values.sort();
    values
}

fn contains_secret(value: &Value) -> bool {
    match value {
        Value::Object(values) => values.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "capabilityToken"
                    | "claimId"
                    | "credential"
                    | "endpointToken"
                    | "secret"
                    | "token"
                    | "stdout"
                    | "stderr"
            ) || contains_secret(value)
        }),
        Value::Array(values) => values.iter().any(contains_secret),
        _ => false,
    }
}

fn identity_kind(kind: RuntimeIdentityKind) -> &'static str {
    match kind {
        RuntimeIdentityKind::Workspace => "workspace",
        RuntimeIdentityKind::Session => "session",
        RuntimeIdentityKind::Delegation => "delegation",
    }
}

fn agent_role(role: WireAgentRole) -> &'static str {
    match role {
        WireAgentRole::Root => "root",
        WireAgentRole::Series => "series",
        WireAgentRole::Task => "task",
        WireAgentRole::Reviewer => "reviewer",
    }
}

fn parse_agent_role(value: &str) -> RuntimeResult<WireAgentRole> {
    match value {
        "root" => Ok(WireAgentRole::Root),
        "series" => Ok(WireAgentRole::Series),
        "task" => Ok(WireAgentRole::Task),
        "reviewer" => Ok(WireAgentRole::Reviewer),
        _ => Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "typed runtime role is invalid",
        )),
    }
}

fn workspace_mode(mode: ae_sdd_protocol::WorkspaceMode) -> &'static str {
    match mode {
        ae_sdd_protocol::WorkspaceMode::Legacy => "legacy",
        ae_sdd_protocol::WorkspaceMode::Shadow => "shadow",
        ae_sdd_protocol::WorkspaceMode::RustCanary => "rust_canary",
        ae_sdd_protocol::WorkspaceMode::RustSoleWriter => "rust_sole_writer",
    }
}

fn parse_workspace_mode(value: &str) -> RuntimeResult<ae_sdd_protocol::WorkspaceMode> {
    match value {
        "legacy" => Ok(ae_sdd_protocol::WorkspaceMode::Legacy),
        "shadow" => Ok(ae_sdd_protocol::WorkspaceMode::Shadow),
        "rust_canary" => Ok(ae_sdd_protocol::WorkspaceMode::RustCanary),
        "rust_sole_writer" => Ok(ae_sdd_protocol::WorkspaceMode::RustSoleWriter),
        _ => Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "typed runtime workspace mode is invalid",
        )),
    }
}

fn job_status(status: RuntimeJobStatus) -> &'static str {
    match status {
        RuntimeJobStatus::Queued => "queued",
        RuntimeJobStatus::Running => "running",
        RuntimeJobStatus::Pass => "pass",
        RuntimeJobStatus::Fail => "fail",
        RuntimeJobStatus::Error => "error",
        RuntimeJobStatus::Timeout => "timeout",
        RuntimeJobStatus::Cancelled => "cancelled",
        RuntimeJobStatus::Stale => "stale",
    }
}

fn parse_job_status(value: &str) -> RuntimeResult<RuntimeJobStatus> {
    match value {
        "queued" => Ok(RuntimeJobStatus::Queued),
        "running" => Ok(RuntimeJobStatus::Running),
        "pass" => Ok(RuntimeJobStatus::Pass),
        "fail" => Ok(RuntimeJobStatus::Fail),
        "error" => Ok(RuntimeJobStatus::Error),
        "timeout" => Ok(RuntimeJobStatus::Timeout),
        "cancelled" => Ok(RuntimeJobStatus::Cancelled),
        "stale" => Ok(RuntimeJobStatus::Stale),
        _ => Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "typed runtime job status is invalid",
        )),
    }
}

fn timestamp(unix_ms: u64) -> RuntimeResult<String> {
    let value = i64::try_from(unix_ms).map_err(|_| range_error())?;
    jiff::Timestamp::from_millisecond(value)
        .map(|timestamp| timestamp.to_string())
        .map_err(|_| range_error())
}

fn parse_timestamp(value: &str) -> RuntimeResult<u64> {
    let timestamp = jiff::Timestamp::from_str(value).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "typed runtime timestamp is invalid",
        )
    })?;
    u64::try_from(timestamp.as_millisecond()).map_err(|_| range_error())
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn value_string<'a>(value: &'a Value, key: &str) -> RuntimeResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("typed legacy import string field is missing"))
}

fn value_u64(value: &Value, key: &str) -> RuntimeResult<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error("typed legacy import integer field is missing"))
}

fn schema_error(message: &'static str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

fn review_projection_error(message: &'static str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

fn revision_conflict<T>(message: &'static str) -> RuntimeResult<T> {
    Err(RuntimeError::new(
        StableErrorCode::RevisionConflict,
        message,
    ))
}

fn configure(connection: &Connection) -> RuntimeResult<()> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(sqlite_error)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(sqlite_error)?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(sqlite_error)?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    event_store_id: EventStoreId,
    mut event: DurableEvent,
) -> RuntimeResult<DurableEvent> {
    let payload_json = serde_json::to_string(&event.payload).map_err(canonical_error)?;
    if payload_json.len() > 65_536 {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "runtime event payload exceeds 65536 bytes",
        ));
    }
    let now = UtcTimestamp::now().to_string();
    transaction
        .execute(
            "INSERT INTO runtime_event(event_store_id,boot_id,workspace_id,session_id,work_item_id,event_type,schema_version,payload_json,payload_ref,payload_digest,byte_len,committed_at) VALUES(?1,?2,?3,?4,?5,?6,1,?7,NULL,?8,?9,?10)",
            params![
                event_store_id.to_string(),
                event.boot_id,
                event.workspace_id.as_deref().unwrap_or(""),
                event.session_id,
                event.work_item_id.as_deref().unwrap_or(""),
                event.kind,
                payload_json,
                event.payload_digest,
                i64::try_from(payload_json.len()).map_err(|_| range_error())?,
                now,
            ],
        )
        .map_err(sqlite_error)?;
    event.event_store_id = event_store_id.to_string();
    event.event_seq = from_i64(transaction.last_insert_rowid())?;
    Ok(event)
}

fn load_receipt_tx(
    transaction: &Transaction<'_>,
    scope: &str,
    key: &str,
) -> RuntimeResult<Option<IdempotencyReceipt>> {
    transaction
        .query_row(
            "SELECT request_digest,response_json,event_seq FROM runtime_generic_receipt_v1 WHERE scope=?1 AND key=?2",
            params![scope, key],
            |row| {
                Ok(IdempotencyReceipt {
                    scope: scope.to_owned(),
                    key: key.to_owned(),
                    request_digest: row.get(0)?,
                    response_json: row.get(1)?,
                    event_seq: u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
                })
            },
        )
        .optional()
        .map_err(sqlite_error)
}

fn load_receipt_connection(
    connection: &Connection,
    scope: &str,
    key: &str,
) -> RuntimeResult<Option<IdempotencyReceipt>> {
    connection
        .query_row(
            "SELECT request_digest,response_json,event_seq FROM runtime_generic_receipt_v1 WHERE scope=?1 AND key=?2",
            params![scope, key],
            |row| {
                Ok(IdempotencyReceipt {
                    scope: scope.to_owned(),
                    key: key.to_owned(),
                    request_digest: row.get(0)?,
                    response_json: row.get(1)?,
                    event_seq: u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
                })
            },
        )
        .optional()
        .map_err(sqlite_error)
}

fn load_event_tx(
    transaction: &Transaction<'_>,
    sequence: u64,
) -> RuntimeResult<Option<DurableEvent>> {
    transaction
        .query_row(
            "SELECT event_seq,event_store_id,boot_id,workspace_id,session_id,work_item_id,event_type,payload_json,payload_digest FROM runtime_event WHERE event_seq=?1",
            [to_i64(sequence)?],
            row_to_event,
        )
        .optional()
        .map_err(sqlite_error)
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<DurableEvent> {
    let workspace_id: String = row.get(3)?;
    let work_item_id: String = row.get(5)?;
    let payload_json: String = row.get(7)?;
    let payload = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(DurableEvent {
        event_seq: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
        event_store_id: row.get(1)?,
        boot_id: row.get(2)?,
        workspace_id: (!workspace_id.is_empty()).then_some(workspace_id),
        session_id: row.get(4)?,
        work_item_id: (!work_item_id.is_empty()).then_some(work_item_id),
        kind: row.get(6)?,
        payload,
        payload_digest: row.get(8)?,
    })
}

fn to_i64(value: u64) -> RuntimeResult<i64> {
    i64::try_from(value).map_err(|_| range_error())
}

fn from_i64(value: i64) -> RuntimeResult<u64> {
    u64::try_from(value).map_err(|_| range_error())
}

fn range_error() -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime SQLite integer is outside the supported range",
    )
}

fn sqlite_error(_error: rusqlite::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime SQLite operation failed",
    )
}

fn store_error(_error: ae_sdd_store::StoreError) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime store operation failed",
    )
}

fn canonical_error(_error: serde_json::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "durable runtime JSON is malformed",
    )
}

#[allow(dead_code)]
fn _type_anchor(_fingerprint: InputFingerprint) {}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use ae_sdd_review::ReviewSupervisor;
    use serde_json::json;
    use tempfile::TempDir;

    const REPLAY_WORKSPACE: &str = "00000000-0000-0000-0000-0000000000a1";
    const REPLAY_WORK_ITEM: &str = "STORY-PROJECTION-REPLAY-001";
    const REPLAY_ROOT_SESSION: &str = "00000000-0000-0000-0000-0000000000b1";
    const REPLAY_AUTHOR_SESSION: &str = "00000000-0000-0000-0000-0000000000b2";
    const REPLAY_REVIEWER_SESSION: &str = "00000000-0000-0000-0000-0000000000b3";
    const REPLAY_DELEGATION: &str = "00000000-0000-0000-0000-0000000000c1";
    const REPLAY_ACTION: &str = "00000000-0000-0000-0000-0000000000d1";
    const REPLAY_ACK: &str = "00000000-0000-0000-0000-0000000000d2";
    const REPLAY_ADAPTER: &str = "projection-replay-adapter";
    const REPLAY_INPUT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const REPLAY_RULESET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const REPLAY_POLICY: &str = "3333333333333333333333333333333333333333333333333333333333333333";
    const REPLAY_BOOT: &str = "00000000-0000-0000-0000-0000000000e1";

    /// Table/row census used to prove an exact replay changed nothing.
    ///
    /// Every projection table plus the projection receipt namespace is captured
    /// as its full ordered row text, so both the row count and the row bytes are
    /// compared, not just a summary count.
    fn projection_census(database: &Path) -> Vec<(String, Vec<String>)> {
        let connection = Connection::open(database).expect("census database opens");
        configure(&connection).expect("census pragmas apply");
        let tables = [
            "review_session_v2_projection",
            "review_batch_v2_projection",
            "review_attempt_v2_projection",
            "review_effective_contribution_v2_projection",
            "review_finding_v2_projection",
            "review_remediation_v2_projection",
            "review_exit_receipt_v2_projection",
        ];
        let mut census = Vec::new();
        for table in tables {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table}"))
                .expect("census select prepares");
            let column_count = statement.column_count();
            let rows = statement
                .query_map([], move |row| {
                    let mut cells = Vec::with_capacity(column_count);
                    for index in 0..column_count {
                        cells.push(format!(
                            "{:?}",
                            row.get::<_, rusqlite::types::Value>(index)?
                        ));
                    }
                    Ok(cells.join("|"))
                })
                .expect("census rows query");
            let mut values = rows
                .map(|row| row.expect("census row decodes"))
                .collect::<Vec<_>>();
            values.sort();
            census.push((table.to_owned(), values));
        }
        let mut statement = connection
            .prepare(
                "SELECT key||'='||value_json FROM runtime_record_v1 \
                 WHERE namespace='review-projection-event/v3' ORDER BY key",
            )
            .expect("receipt select prepares");
        let receipts = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("receipt rows query")
            .map(|row| row.expect("receipt row decodes"))
            .collect::<Vec<_>>();
        census.push(("review-projection-event/v3".to_owned(), receipts));
        census
    }

    /// Opens a migrated temp runtime database with the full FK authority chain
    /// that migration 0009 requires on the Review Batch v2 projection write path.
    ///
    /// Foreign keys stay ENABLED: every referenced workspace, agent session,
    /// delegation, host action, attestation and runtime event row is seeded for
    /// real so the projection write exercises the production constraints.
    fn seeded_projection_database(directory: &TempDir, event_sequence: i64) -> PathBuf {
        let database = directory.path().join("runtime.db");
        SqliteRuntimePersistence::open(&database).expect("runtime database opens");
        let connection = Connection::open(&database).expect("authority database opens");
        configure(&connection).expect("authority pragmas apply");
        let foreign_keys_enabled: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign key pragma reads");
        assert_eq!(foreign_keys_enabled, 1, "seeding must run with FKs enabled");
        let now = "2026-07-26T00:00:00Z";

        connection
            .execute(
                "INSERT INTO workspace(workspace_id,canonical_root,project_key,mode,\
                    inventory_generation,dirty,created_at,updated_at) \
                 VALUES(?1,'/tmp/projection-replay','projection-replay','rust_canary',3,0,?2,?2)",
                params![REPLAY_WORKSPACE, now],
            )
            .expect("workspace authority row inserts");
        connection
            .execute(
                "INSERT INTO host_adapter_instance(adapter_id,capability_digest,status,\
                    last_command_seq,heartbeat_at,created_at,updated_at) \
                 VALUES(?1,?2,'active',1,?3,?3,?3)",
                params![REPLAY_ADAPTER, REPLAY_POLICY, now],
            )
            .expect("host adapter authority row inserts");

        // `external_key_hash` is unique per workspace, so every seeded session
        // carries its own distinct 64-hex external key digest.
        for (session_id, role, parent, delegation, external_key) in [
            (REPLAY_ROOT_SESSION, "root", None, None, "a".repeat(64)),
            (
                REPLAY_AUTHOR_SESSION,
                "task",
                Some(REPLAY_ROOT_SESSION),
                None,
                "b".repeat(64),
            ),
            (
                REPLAY_REVIEWER_SESSION,
                "reviewer",
                Some(REPLAY_ROOT_SESSION),
                Some(REPLAY_DELEGATION),
                "c".repeat(64),
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO agent_session(session_id,agent_id,external_key_hash,workspace_id,\
                        role,root_session_id,parent_session_id,delegation_id,status,heartbeat_at,\
                        created_at,updated_at) \
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'active',?9,?9,?9)",
                    params![
                        session_id,
                        format!("agent-{role}"),
                        external_key,
                        REPLAY_WORKSPACE,
                        role,
                        REPLAY_ROOT_SESSION,
                        parent,
                        delegation,
                        now
                    ],
                )
                .expect("agent session authority row inserts");
        }

        connection
            .execute(
                "INSERT INTO delegation(delegation_id,workspace_id,root_session_id,\
                    parent_session_id,child_session_id,parent_delegation_id,role,input_revision,\
                    input_fingerprint,status,deadline,receipt_digest,created_at,updated_at) \
                 VALUES(?1,?2,?3,?3,?4,NULL,'reviewer',7,?5,'accepted',?6,?7,?6,?6)",
                params![
                    REPLAY_DELEGATION,
                    REPLAY_WORKSPACE,
                    REPLAY_ROOT_SESSION,
                    REPLAY_REVIEWER_SESSION,
                    REPLAY_INPUT,
                    now,
                    REPLAY_RULESET
                ],
            )
            .expect("delegation authority row inserts");
        connection
            .execute(
                "INSERT INTO host_action(action_id,adapter_id,kind,command_seq,request_digest,\
                    session_id,context_generation,ack_status,ack_id,response_digest,deadline,\
                    created_at,updated_at) \
                 VALUES(?1,?2,'delegation.spawn',1,?3,?4,1,'acked',?5,?6,?7,?7,?7)",
                params![
                    REPLAY_ACTION,
                    REPLAY_ADAPTER,
                    REPLAY_INPUT,
                    REPLAY_REVIEWER_SESSION,
                    REPLAY_ACK,
                    REPLAY_RULESET,
                    now
                ],
            )
            .expect("host action authority row inserts");
        connection
            .execute(
                "INSERT INTO delegation_host_action_v1(workspace_id,delegation_id,host_action_id,\
                    parent_session_id,action_digest,created_at) \
                 VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    REPLAY_WORKSPACE,
                    REPLAY_DELEGATION,
                    REPLAY_ACTION,
                    REPLAY_ROOT_SESSION,
                    REPLAY_INPUT,
                    now
                ],
            )
            .expect("delegation host action authority row inserts");

        let grant_json =
            json!({"specialty":"general","grantedSpecialties":["general"]}).to_string();
        let grant_byte_len = i64::try_from(grant_json.len()).expect("grant length fits i64");
        connection
            .execute(
                "INSERT INTO delegation_attestation_v1(workspace_id,delegation_id,\
                    physical_session_id,host_action_id,host_ack_id,claim_id,action_digest,\
                    ack_digest,claim_digest,grant_schema_version,grant_json,grant_digest,\
                    grant_byte_len,attestation_ref,attestation_digest,accepted_boot_id,\
                    accepted_at,expires_at) \
                 VALUES(?1,?2,?3,?4,?5,'claim-replay',?6,?7,?7,1,?8,?9,?10,'delegation:reviewer',\
                    ?9,?11,?12,'2026-07-26T01:00:00Z')",
                params![
                    REPLAY_WORKSPACE,
                    REPLAY_DELEGATION,
                    REPLAY_REVIEWER_SESSION,
                    REPLAY_ACTION,
                    REPLAY_ACK,
                    REPLAY_INPUT,
                    REPLAY_RULESET,
                    grant_json,
                    REPLAY_POLICY,
                    grant_byte_len,
                    REPLAY_BOOT,
                    now
                ],
            )
            .expect("delegation attestation authority row inserts");

        connection
            .execute(
                "INSERT INTO runtime_event(event_seq,event_store_id,boot_id,workspace_id,\
                    session_id,work_item_id,event_type,schema_version,payload_json,payload_ref,\
                    payload_digest,byte_len,committed_at) \
                 VALUES(?1,'projection-replay-store',?2,?3,?4,?5,'review.record',1,'{}',NULL,?6,2,?7)",
                params![
                    event_sequence,
                    REPLAY_BOOT,
                    REPLAY_WORKSPACE,
                    REPLAY_REVIEWER_SESSION,
                    REPLAY_WORK_ITEM,
                    REPLAY_INPUT,
                    now
                ],
            )
            .expect("runtime event authority row inserts");
        database
    }

    #[test]
    fn review_projection_loader_uses_stable_ordered_sql() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("runtime.db");
        SqliteRuntimePersistence::open(&database).expect("runtime database opens");
        let connection = Connection::open(&database).expect("projection database opens");
        connection
            .pragma_update(None, "foreign_keys", false)
            .expect("fixture can bypass unrelated authority rows");
        let digest = "0".repeat(64);
        connection
            .execute(
                "INSERT INTO review_session_v2_projection(\
                    workspace_id,work_item_id,review_id,schema_version,parent_review_id,tier,status,\
                    author_session_id,root_session_id,repair_class,final_proof_requirement,\
                    requires_general,requires_be,requires_ar,requires_qa,input_fingerprint,\
                    ruleset_fingerprint,source_revision,inventory_generation,attempts,valid_batches,\
                    remediations,clean_streak,clean_target,infra_failures,protocol_failures,\
                    max_attempts,max_valid_batches,max_remediations,max_wall_clock_minutes,\
                    started_at,deadline_at,terminal_at,first_event_seq,last_event_seq,created_at,updated_at\
                 ) VALUES('workspace-1','work-item-1','review-1',2,NULL,'tier2','running',\
                    'author-1','root-1','none','deterministic_gates',0,1,1,0,?1,?1,1,1,\
                    1,0,0,0,1,1,0,4,4,4,60,'2026-07-26T00:00:00Z',\
                    '2026-07-26T01:00:00Z',NULL,1,1,'2026-07-26T00:00:00Z',\
                    '2026-07-26T00:00:01Z')",
                [&digest],
            )
            .expect("session fixture inserts");
        connection
            .execute(
                "INSERT INTO review_batch_v2_projection(\
                    workspace_id,review_id,batch_id,schema_version,input_fingerprint,\
                    ruleset_fingerprint,latest_attempt_id,latest_status,required_specialty_count,\
                    effective_contribution_count,finding_count,closed,valid_batch_ordinal,\
                    latest_receipt_digest,first_event_seq,last_event_seq,created_at,updated_at\
                 ) VALUES('workspace-1','review-1','batch-1',2,?1,?1,'attempt-1',\
                    'invalid_infra',2,0,0,0,NULL,?1,1,1,'2026-07-26T00:00:00Z',\
                    '2026-07-26T00:00:01Z')",
                [&digest],
            )
            .expect("batch fixture inserts");
        connection
            .execute(
                "INSERT INTO review_attempt_v2_projection(\
                    workspace_id,review_id,batch_id,attempt_id,schema_version,attempt_ordinal,\
                    status,input_fingerprint,ruleset_fingerprint,required_specialty_count,\
                    attempted_specialty_count,effective_specialty_count,finding_count,\
                    idempotency_key_digest,payload_digest,receipt_digest,started_at,completed_at,event_seq\
                 ) VALUES('workspace-1','review-1','batch-1','attempt-1',2,1,'invalid_infra',\
                    ?1,?1,2,1,0,0,?1,?1,?1,'2026-07-26T00:00:00Z',\
                    '2026-07-26T00:00:01Z',1)",
                [&digest],
            )
            .expect("attempt fixture inserts");

        let projection = load_review_authority_projection_connection(
            &connection,
            "workspace-1",
            "work-item-1",
            "review-1",
        )
        .expect("projection loads")
        .expect("projection exists");
        assert_eq!(projection.attempts.len(), 1);
        assert_eq!(projection.first_event_sequence, 1);
        assert_eq!(projection.last_event_sequence, 1);
        assert!(projection.contributions.is_empty());
        assert!(projection.findings.is_empty());
        assert!(projection.remediations.is_empty());
        assert!(projection.exit_receipt.is_none());
    }

    /// Terminal Tier 1 review state whose reviewer identity matches the rows
    /// seeded by [`seeded_projection_database`].
    ///
    /// The typed session and attempt are evaluated by the real supervisor, so
    /// the batch, counters and exit receipt are authority-consistent instead of
    /// hand-built.
    fn terminal_review_state() -> Value {
        terminal_review_state_observed_at("2026-07-26T00:01:00Z")
    }

    /// Same terminal Tier 1 state as [`terminal_review_state`] with a caller
    /// supplied attempt observation instant, which yields different typed
    /// records for the same workspace and event sequence.
    fn terminal_review_state_observed_at(observed_at: &str) -> Value {
        let session: ReviewSessionV2 = serde_json::from_value(json!({
            "schemaVersion":"v2",
            "reviewId":"review-replay",
            "parentReviewId":null,
            "tier":"tier1",
            "requiredSpecialties":["general"],
            "authorSessionId":REPLAY_AUTHOR_SESSION,
            "rootSessionId":REPLAY_ROOT_SESSION,
            "inputFingerprint":REPLAY_INPUT,
            "rulesetFingerprint":REPLAY_RULESET,
            "policyDigest":REPLAY_POLICY,
            "sourceRevision":7,
            "inventoryGeneration":3,
            "repairClass":"none",
            "cleanPolicy":{"cleanTarget":1,"finalProofRequirement":"none"},
            "budget":{
                "maxAttempts":3,"maxValidBatches":2,"maxRemediations":2,"maxWallClockMinutes":30
            },
            "counters":{
                "attempts":0,"validBatches":0,"cleanStreak":0,"remediations":0,
                "infraFailures":0,"protocolFailures":0
            },
            "status":"running",
            "startedAt":"2026-07-26T00:00:00Z",
            "deadlineAt":"2026-07-26T01:00:00Z",
            "terminalAt":null
        }))
        .expect("replay session decodes");
        let attempt: ReviewAttemptV2 = serde_json::from_value(json!({
            "schemaVersion":"v2",
            "reviewId":"review-replay",
            "batchId":"batch-replay",
            "attemptId":"attempt-replay",
            "attemptOrdinal":1,
            "idempotencyKey":"attempt-replay-key",
            "inputFingerprint":REPLAY_INPUT,
            "rulesetFingerprint":REPLAY_RULESET,
            "contributions":[{
                "sourceAttemptId":"attempt-replay",
                "reviewer":{
                    "agentRole":"reviewer",
                    "specialty":"general",
                    "grantedSpecialties":["general"],
                    "physicalSessionId":REPLAY_REVIEWER_SESSION,
                    "rootSessionId":REPLAY_ROOT_SESSION,
                    "delegationId":REPLAY_DELEGATION,
                    "lineageDepth":2,
                    "attestationRef":"delegation:reviewer",
                    "attestationDigest":REPLAY_POLICY,
                    "specialtyGrantDigest":REPLAY_POLICY
                },
                "outcome":"clean",
                "findings":[],
                "reportDigest":REPLAY_POLICY,
                "contributionDigest":REPLAY_RULESET,
                "inputFingerprint":REPLAY_INPUT,
                "rulesetFingerprint":REPLAY_RULESET
            }],
            "observedAt":observed_at,
            "finalProof":{
                "kind":"none","digest":null,"sourceRevision":null,
                "inputFingerprint":null,"rulesetFingerprint":null,"observedAt":null
            },
            "projectAuthority":{
                "projectReceiptRef":"state:review",
                "activeManifestDigest":REPLAY_POLICY,
                "stateReceiptRefDigest":REPLAY_POLICY,
                "journalMutationId":"mutation-replay"
            },
            "remediation":null
        }))
        .expect("replay attempt decodes");
        let attempt_value = serde_json::to_value(&attempt).expect("attempt serializes");
        let evaluated =
            ReviewSupervisor::evaluate(&session, None, attempt).expect("clean tier1 evaluates");
        json!({
            "reviewSession": evaluated.next_session(),
            "review": {
                "status":"passed",
                "findings":[],
                "batch":evaluated.next_batch(),
                "attempt":attempt_value,
                "receipt":evaluated.exit_receipt()
            }
        })
    }

    /// Builds the v2 projection write for `state` at `event_sequence`.
    fn replay_projection_write(state: &Value, event_sequence: i64) -> ReviewProjectionWrite {
        crate::review_authority::review_projection_write_from_state(
            state,
            REPLAY_WORKSPACE,
            REPLAY_WORK_ITEM,
            u64::try_from(event_sequence).expect("event sequence is non-negative"),
        )
        .expect("terminal review state projects")
        .expect("v2 projection write")
    }

    #[test]
    fn review_projection_replay_of_identical_write_is_a_no_op() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = seeded_projection_database(&directory, 41);
        let write = replay_projection_write(&terminal_review_state(), 41);

        upsert_review_authority_projection(&database, &write).expect("first projection apply");
        let after_first = projection_census(&database);

        // Observer connection stays open across the replay: SQLite only bumps
        // `data_version` for this connection when another connection commits an
        // actual page change, so an unchanged counter proves the replay wrote
        // nothing rather than merely rewriting identical values.
        let observer = Connection::open(&database).expect("observer database opens");
        configure(&observer).expect("observer pragmas apply");
        let data_version_before = data_version(&observer);

        upsert_review_authority_projection(&database, &write)
            .expect("identical replay applies as a no-op");
        let after_replay = projection_census(&database);

        assert_eq!(
            data_version(&observer),
            data_version_before,
            "exact replay must not write to the database at all"
        );

        assert_eq!(
            after_replay, after_first,
            "exact replay must leave every projection row byte-identical"
        );
        // A clean Tier 1 attempt raises no findings and needs no remediation, so
        // only the singleton tables are asserted; the census equality above
        // already covers the empty ones.
        for table in [
            "review_session_v2_projection",
            "review_batch_v2_projection",
            "review_attempt_v2_projection",
            "review_effective_contribution_v2_projection",
            "review_exit_receipt_v2_projection",
            "review-projection-event/v3",
        ] {
            let rows = census_rows(&after_replay, table);
            assert_eq!(
                rows.len(),
                1,
                "{table} must hold exactly one row after an exact replay"
            );
        }
        for table in [
            "review_finding_v2_projection",
            "review_remediation_v2_projection",
        ] {
            assert!(
                census_rows(&after_replay, table).is_empty(),
                "{table} must stay empty for a clean Tier 1 attempt"
            );
        }
    }

    #[test]
    fn terminal_child_projection_preserves_an_unavailable_parent_anchor() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = seeded_projection_database(&directory, 41);
        let mut state = terminal_review_state();
        state["reviewSession"]["parentReviewId"] = json!("review-parent-not-replayed");
        let write = replay_projection_write(&state, 41);

        upsert_review_authority_projection(&database, &write)
            .expect("rebuildable projection does not require the parent event to be retained");

        let projection = load_review_authority_projection(
            &database,
            REPLAY_WORKSPACE,
            REPLAY_WORK_ITEM,
            "review-replay",
        )
        .expect("projection loads")
        .expect("projection exists");
        assert_eq!(
            projection.session["parentReviewId"],
            json!("review-parent-not-replayed")
        );
    }

    /// Reads SQLite's change counter as seen by an observer connection.
    fn data_version(observer: &Connection) -> i64 {
        observer
            .query_row("PRAGMA data_version", [], |row| row.get(0))
            .expect("data version pragma reads")
    }

    /// Looks up one census entry, failing loudly if the census stopped covering
    /// that table.
    fn census_rows<'a>(census: &'a [(String, Vec<String>)], table: &str) -> &'a [String] {
        census
            .iter()
            .find(|(name, _)| name == table)
            .map(|(_, rows)| rows.as_slice())
            .unwrap_or_else(|| panic!("census covers {table}"))
    }

    #[test]
    fn review_projection_replay_with_different_records_fails_closed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = seeded_projection_database(&directory, 41);
        let write = replay_projection_write(&terminal_review_state(), 41);
        upsert_review_authority_projection(&database, &write).expect("first projection apply");
        let before_conflict = projection_census(&database);

        let conflicting = replay_projection_write(
            &terminal_review_state_observed_at("2026-07-26T00:02:30Z"),
            41,
        );
        assert_ne!(
            conflicting.attempt, write.attempt,
            "the conflicting replay must carry different typed records"
        );
        let error = upsert_review_authority_projection(&database, &conflicting)
            .expect_err("conflicting replay must fail closed");

        assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
        assert_eq!(
            projection_census(&database),
            before_conflict,
            "a rejected replay must not mutate the committed projection"
        );
    }
}
