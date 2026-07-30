use std::{path::Path, str::FromStr, sync::Mutex, time::Duration};

use ae_sdd_domain::{
    ArtifactDigest, DelegationId, EventSequence, EventStoreId, FencingToken, StateRevision,
    WorkItemId, WorkspaceId,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, types::ValueRef};
use sha2::{Digest, Sha256};

use crate::{
    ChildResultRecord, CompactCycleRecord, ContextPressureSampleRecord, ContextProjectionRecord,
    DelegationRecord, DelegationRequestReceipt, HookEventReceipt, HostAckReceipt, HostActionRecord,
    HostAdapterRecord, IdempotencyKey, MemoryCleanupReceipt, OperationReceipt, RuntimeEventDraft,
    RuntimeEventPayload, RuntimeEventRecord, RuntimeRepository, StoreError,
    SupervisorCheckpointRecord, UtcTimestamp,
};

pub const SQLITE_RUNTIME_BASE_MIGRATION: &str =
    include_str!("../../../migrations/0001_runtime_base.sql");
pub const SQLITE_LEASE_CONTROL_RECEIPT_MIGRATION: &str =
    include_str!("../../../migrations/0002_lease_control_receipts.sql");
pub const SQLITE_ROUTE_SERIES_PLAN_MIGRATION: &str =
    include_str!("../../../migrations/0003_route_series_plan.sql");
pub const SQLITE_WORK_ITEM_LIFECYCLE_MIGRATION: &str =
    include_str!("../../../migrations/0004_work_item_lifecycle.sql");
pub const SQLITE_RESOURCE_CONTEXT_MIGRATION: &str =
    include_str!("../../../migrations/0005_resource_context.sql");
pub const SQLITE_HOST_SESSION_MIGRATION: &str =
    include_str!("../../../migrations/0006_host_session.sql");
pub const SQLITE_REVIEW_RUNTIME_MIGRATION: &str =
    include_str!("../../../migrations/0007_review_runtime.sql");
pub const SQLITE_EXECUTION_RECEIPTS_MIGRATION: &str =
    include_str!("../../../migrations/0008_execution_receipts.sql");
pub const SQLITE_REVIEW_BATCH_V2_MIGRATION: &str =
    include_str!("../../../migrations/0009_review_batch_v2.sql");
pub const SQLITE_RUNTIME_JOB_V1_MIGRATION: &str =
    include_str!("../../../migrations/0010_runtime_job_v1.sql");
pub const SQLITE_EXECUTION_SUPERVISOR_V1_MIGRATION: &str =
    include_str!("../../../migrations/0011_execution_supervisor_v1.sql");
pub const SQLITE_REVIEW_CLEAN_TARGET_UNIFY_MIGRATION: &str =
    include_str!("../../../migrations/0012_review_clean_target_unify.sql");
pub const SQLITE_REVIEW_PARENT_PROJECTION_ANCHOR_MIGRATION: &str =
    include_str!("../../../migrations/0013_review_parent_projection_anchor.sql");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeMigration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// The schema version a fully migrated runtime database must report.
///
/// Single source for the head version so adding a migration does not require
/// editing a literal in every caller and test; the catalog itself is the truth.
#[must_use]
pub fn latest_runtime_schema_version() -> i64 {
    SQLITE_RUNTIME_MIGRATIONS
        .last()
        .expect("migration catalog is non-empty")
        .version
}

pub const SQLITE_RUNTIME_MIGRATIONS: &[RuntimeMigration] = &[
    RuntimeMigration {
        version: 1,
        name: "0001_runtime_base",
        sql: SQLITE_RUNTIME_BASE_MIGRATION,
    },
    RuntimeMigration {
        version: 2,
        name: "0002_lease_control_receipts",
        sql: SQLITE_LEASE_CONTROL_RECEIPT_MIGRATION,
    },
    RuntimeMigration {
        version: 3,
        name: "0003_route_series_plan",
        sql: SQLITE_ROUTE_SERIES_PLAN_MIGRATION,
    },
    RuntimeMigration {
        version: 4,
        name: "0004_work_item_lifecycle",
        sql: SQLITE_WORK_ITEM_LIFECYCLE_MIGRATION,
    },
    RuntimeMigration {
        version: 5,
        name: "0005_resource_context",
        sql: SQLITE_RESOURCE_CONTEXT_MIGRATION,
    },
    RuntimeMigration {
        version: 6,
        name: "0006_host_session",
        sql: SQLITE_HOST_SESSION_MIGRATION,
    },
    RuntimeMigration {
        version: 7,
        name: "0007_review_runtime",
        sql: SQLITE_REVIEW_RUNTIME_MIGRATION,
    },
    RuntimeMigration {
        version: 8,
        name: "0008_execution_receipts",
        sql: SQLITE_EXECUTION_RECEIPTS_MIGRATION,
    },
    RuntimeMigration {
        version: 9,
        name: "0009_review_batch_v2",
        sql: SQLITE_REVIEW_BATCH_V2_MIGRATION,
    },
    RuntimeMigration {
        version: 10,
        name: "0010_runtime_job_v1",
        sql: SQLITE_RUNTIME_JOB_V1_MIGRATION,
    },
    RuntimeMigration {
        version: 11,
        name: "0011_execution_supervisor_v1",
        sql: SQLITE_EXECUTION_SUPERVISOR_V1_MIGRATION,
    },
    RuntimeMigration {
        version: 12,
        name: "0012_review_clean_target_unify",
        sql: SQLITE_REVIEW_CLEAN_TARGET_UNIFY_MIGRATION,
    },
    RuntimeMigration {
        version: 13,
        name: "0013_review_parent_projection_anchor",
        sql: SQLITE_REVIEW_PARENT_PROJECTION_ANCHOR_MIGRATION,
    },
];

#[derive(Debug)]
pub struct SqliteRuntimeRepository {
    event_store_id: EventStoreId,
    connection: Mutex<Connection>,
}

impl SqliteRuntimeRepository {
    pub fn open(
        path: impl AsRef<Path>,
        proposed_event_store_id: EventStoreId,
        created_at: &UtcTimestamp,
    ) -> Result<Self, StoreError> {
        let mut connection = Connection::open(path)?;
        configure(&connection, true)?;
        let event_store_id = migrate(&mut connection, proposed_event_store_id, created_at)?;
        Ok(Self {
            event_store_id,
            connection: Mutex::new(connection),
        })
    }

    pub fn open_in_memory(
        event_store_id: EventStoreId,
        created_at: &UtcTimestamp,
    ) -> Result<Self, StoreError> {
        let mut connection = Connection::open_in_memory()?;
        configure(&connection, false)?;
        let event_store_id = migrate(&mut connection, event_store_id, created_at)?;
        Ok(Self {
            event_store_id,
            connection: Mutex::new(connection),
        })
    }

    pub fn integrity_check(&self) -> Result<(), StoreError> {
        let connection = self
            .connection
            .lock()
            .expect("SQLite repository lock is not poisoned");
        let result: String =
            connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if result != "ok" {
            return Err(StoreError::DatabaseIncompatible {
                reason: format!("SQLite integrity_check returned {result}").into_boxed_str(),
            });
        }
        Ok(())
    }

    pub fn pragma_value(&self, name: &'static str) -> Result<String, StoreError> {
        let sql = match name {
            "journal_mode" => "PRAGMA journal_mode",
            "synchronous" => "PRAGMA synchronous",
            "foreign_keys" => "PRAGMA foreign_keys",
            "user_version" => "PRAGMA user_version",
            _ => {
                return Err(StoreError::DatabaseIncompatible {
                    reason: "PRAGMA name is not in the compile-time allowlist".into(),
                });
            }
        };
        self.connection
            .lock()
            .expect("SQLite repository lock is not poisoned")
            .query_row(sql, [], |row| {
                Ok(match row.get_ref(0)? {
                    ValueRef::Null => "null".to_owned(),
                    ValueRef::Integer(value) => value.to_string(),
                    ValueRef::Real(value) => value.to_string(),
                    ValueRef::Text(value) => String::from_utf8_lossy(value).into_owned(),
                    ValueRef::Blob(value) => hex::encode(value),
                })
            })
            .map_err(StoreError::from)
    }
}

fn configure(connection: &Connection, file_backed: bool) -> Result<(), StoreError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    if file_backed {
        connection.pragma_update(None, "journal_mode", "WAL")?;
    }
    connection.busy_timeout(Duration::from_millis(1000))?;
    Ok(())
}

fn migrate(
    connection: &mut Connection,
    proposed_event_store_id: EventStoreId,
    created_at: &UtcTimestamp,
) -> Result<EventStoreId, StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current_version: i64 =
        transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let latest_version = latest_runtime_schema_version();
    if !(0..=latest_version).contains(&current_version) {
        return Err(StoreError::DatabaseIncompatible {
            reason: format!("unsupported runtime schema version {current_version}")
                .into_boxed_str(),
        });
    }
    validate_migration_prefix(&transaction, current_version)?;
    for migration in SQLITE_RUNTIME_MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        transaction.execute_batch(migration.sql)?;
        let observed: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if observed != migration.version {
            return Err(StoreError::DatabaseIncompatible {
                reason: format!(
                    "migration {} set user_version {observed}, expected {}",
                    migration.name, migration.version
                )
                .into_boxed_str(),
            });
        }
        transaction.execute(
            "INSERT INTO schema_migration(version,name,checksum,applied_at) VALUES(?1,?2,?3,?4)",
            params![
                migration.version,
                migration.name,
                migration_checksum(migration.sql),
                created_at.to_string()
            ],
        )?;
    }
    validate_migration_prefix(&transaction, latest_version)?;
    let foreign_key_violations: i64 =
        transaction.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if foreign_key_violations != 0 {
        return Err(StoreError::DatabaseIncompatible {
            reason: "runtime migration left foreign-key violations".into(),
        });
    }
    transaction.execute(
        "INSERT OR IGNORE INTO runtime_identity(singleton,event_store_id,created_at) VALUES(1,?1,?2)",
        params![proposed_event_store_id.to_string(), created_at.to_string()],
    )?;
    let persisted: String = transaction.query_row(
        "SELECT event_store_id FROM runtime_identity WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    transaction.commit()?;
    EventStoreId::from_str(&persisted).map_err(StoreError::from)
}

fn migration_checksum(sql: &str) -> String {
    hex::encode(Sha256::digest(sql.as_bytes()))
}

fn validate_migration_prefix(
    transaction: &rusqlite::Transaction<'_>,
    current_version: i64,
) -> Result<(), StoreError> {
    let user_table_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    if current_version == 0 {
        if user_table_count != 0 {
            return Err(StoreError::DatabaseIncompatible {
                reason: "version-zero runtime database is not empty".into(),
            });
        }
        return Ok(());
    }
    let has_catalog: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migration'",
        [],
        |row| row.get(0),
    )?;
    if has_catalog != 1 {
        return Err(StoreError::DatabaseIncompatible {
            reason: "non-empty runtime database lacks schema_migration".into(),
        });
    }
    let row_count: i64 =
        transaction.query_row("SELECT COUNT(*) FROM schema_migration", [], |row| {
            row.get(0)
        })?;
    if row_count != current_version {
        return Err(StoreError::DatabaseIncompatible {
            reason: "runtime migration catalog has a gap or extra row".into(),
        });
    }
    for migration in
        SQLITE_RUNTIME_MIGRATIONS
            .iter()
            .take(usize::try_from(current_version).map_err(|_| {
                StoreError::DatabaseIncompatible {
                    reason: "runtime migration version is out of range".into(),
                }
            })?)
    {
        let existing: Option<(String, String)> = transaction
            .query_row(
                "SELECT name,checksum FROM schema_migration WHERE version=?1",
                [migration.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let expected_checksum = migration_checksum(migration.sql);
        if existing
            .as_ref()
            .is_none_or(|(name, checksum)| name != migration.name || checksum != &expected_checksum)
        {
            return Err(StoreError::DatabaseIncompatible {
                reason: format!(
                    "published migration {} name or checksum differs",
                    migration.version
                )
                .into_boxed_str(),
            });
        }
    }
    Ok(())
}

impl RuntimeRepository for SqliteRuntimeRepository {
    fn event_store_id(&self) -> EventStoreId {
        self.event_store_id
    }

    fn operation_receipt(
        &self,
        workspace_id: WorkspaceId,
        idempotency_key: &str,
    ) -> Result<Option<(OperationReceipt, RuntimeEventRecord)>, StoreError> {
        let connection = self
            .connection
            .lock()
            .expect("SQLite repository lock is not poisoned");
        load_operation_receipt(&connection, workspace_id, idempotency_key)
    }

    fn index_committed_mutation(
        &self,
        receipt: &OperationReceipt,
        event: &RuntimeEventDraft,
    ) -> Result<(OperationReceipt, RuntimeEventRecord), StoreError> {
        event.validate()?;
        if receipt.workspace_id != event.workspace_id || receipt.work_item_id != event.work_item_id
        {
            return Err(StoreError::PersistenceConflict {
                entity: "operation_receipt.event_scope",
            });
        }
        let mut connection = self
            .connection
            .lock()
            .expect("SQLite repository lock is not poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((existing, existing_event)) = load_operation_receipt(
            &transaction,
            receipt.workspace_id,
            receipt.idempotency_key.as_str(),
        )? {
            if existing.payload_digest != receipt.payload_digest {
                return Err(StoreError::IdempotencyKeyReused {
                    expected: existing.payload_digest,
                    observed: receipt.payload_digest,
                });
            }
            return Ok((existing, existing_event));
        }
        let (payload_json, payload_ref, payload_digest, byte_length) =
            event_payload_columns(&event.payload)?;
        transaction.execute(
            "INSERT INTO runtime_event(event_store_id,boot_id,workspace_id,session_id,work_item_id,event_type,schema_version,payload_json,payload_ref,payload_digest,byte_len,committed_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                self.event_store_id.to_string(),
                event.boot_id.to_string(),
                event.workspace_id.to_string(),
                event.session_id.map(|value| value.to_string()),
                event.work_item_id.to_string(),
                event.event_type.as_ref(),
                i64::from(event.schema_version),
                payload_json,
                payload_ref,
                payload_digest,
                to_i64(byte_length, "runtime_event.byte_len")?,
                event.committed_at.to_string(),
            ],
        )?;
        let event_sequence = u64::try_from(transaction.last_insert_rowid()).map_err(|_| {
            StoreError::DatabaseIncompatible {
                reason: "SQLite produced a negative event sequence".into(),
            }
        })?;
        transaction.execute(
            "INSERT INTO operation_receipt(workspace_id,work_item_id,idempotency_key,payload_digest,operation,revision_before,revision_after,fencing_token,result_digest,mutation_id,event_seq,committed_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                receipt.workspace_id.to_string(),
                receipt.work_item_id.to_string(),
                receipt.idempotency_key.as_str(),
                receipt.payload_digest.to_string(),
                receipt.operation.to_string(),
                to_i64(receipt.revision_before.get(), "operation_receipt.revision_before")?,
                to_i64(receipt.revision_after.get(), "operation_receipt.revision_after")?,
                to_i64(receipt.fencing_token.get(), "operation_receipt.fencing_token")?,
                receipt.result_digest.to_string(),
                receipt.mutation_id.to_string(),
                to_i64(event_sequence, "operation_receipt.event_seq")?,
                receipt.committed_at.to_string(),
            ],
        )?;
        transaction.commit()?;
        Ok((
            receipt.clone(),
            RuntimeEventRecord {
                event_store_id: self.event_store_id,
                event_sequence: EventSequence::new(event_sequence),
                draft: event.clone(),
            },
        ))
    }

    fn persist_delegation(&self, record: &DelegationRecord) -> Result<(), StoreError> {
        let changed = self.connection
            .lock()
            .expect("SQLite repository lock is not poisoned")
            .execute(
                "INSERT INTO delegation(delegation_id,workspace_id,root_session_id,parent_session_id,child_session_id,parent_delegation_id,role,input_revision,input_fingerprint,status,deadline,receipt_digest,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14) ON CONFLICT(delegation_id) DO UPDATE SET child_session_id=excluded.child_session_id,status=excluded.status,deadline=excluded.deadline,receipt_digest=excluded.receipt_digest,updated_at=excluded.updated_at WHERE delegation.workspace_id=excluded.workspace_id AND delegation.root_session_id=excluded.root_session_id AND delegation.parent_session_id=excluded.parent_session_id AND delegation.role=excluded.role AND delegation.input_revision=excluded.input_revision AND delegation.input_fingerprint=excluded.input_fingerprint",
                params![
                    record.delegation_id.to_string(),
                    record.workspace_id.to_string(),
                    record.root_session_id.to_string(),
                    record.parent_session_id.to_string(),
                    record.child_session_id.map(|value| value.to_string()),
                    record.parent_delegation_id.map(|value| value.to_string()),
                    record.role.as_ref(),
                    to_i64(record.input_revision.get(), "delegation.input_revision")?,
                    record.input_fingerprint.to_string(),
                    record.status.as_ref(),
                    record.deadline.to_string(),
                    record.receipt_digest.to_string(),
                    record.deadline.to_string(),
                    record.deadline.to_string(),
                ],
            )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "delegation",
            });
        }
        Ok(())
    }

    fn delegation(&self, id: DelegationId) -> Result<Option<DelegationRecord>, StoreError> {
        self.connection
            .lock()
            .expect("SQLite repository lock is not poisoned")
            .query_row(
                "SELECT workspace_id,root_session_id,parent_session_id,child_session_id,parent_delegation_id,role,input_revision,input_fingerprint,status,deadline,receipt_digest FROM delegation WHERE delegation_id=?1",
                params![id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?, row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?, row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?, row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()?
            .map(|row| decode_delegation(id, row))
            .transpose()
    }

    fn put_delegation_request_receipt(
        &self,
        receipt: &DelegationRequestReceipt,
    ) -> Result<DelegationRequestReceipt, StoreError> {
        let connection = self
            .connection
            .lock()
            .expect("SQLite repository lock is not poisoned");
        connection.execute(
            "INSERT OR IGNORE INTO delegation_request_receipt(workspace_id,parent_session_id,idempotency_key,request_digest,delegation_id,response_digest,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![receipt.workspace_id.to_string(), receipt.parent_session_id.to_string(), receipt.idempotency_key.as_str(), receipt.request_digest.to_string(), receipt.delegation_id.to_string(), receipt.response_digest.to_string(), receipt.created_at.to_string()],
        )?;
        let existing = connection.query_row(
            "SELECT request_digest,delegation_id,response_digest,created_at FROM delegation_request_receipt WHERE workspace_id=?1 AND parent_session_id=?2 AND idempotency_key=?3",
            params![receipt.workspace_id.to_string(), receipt.parent_session_id.to_string(), receipt.idempotency_key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
        )?;
        let decoded = DelegationRequestReceipt {
            workspace_id: receipt.workspace_id,
            parent_session_id: receipt.parent_session_id,
            idempotency_key: receipt.idempotency_key.clone(),
            request_digest: parse_digest(&existing.0, "delegation_request_receipt.request_digest")?,
            delegation_id: parse_id(&existing.1, "delegation_request_receipt.delegation_id")?,
            response_digest: parse_digest(
                &existing.2,
                "delegation_request_receipt.response_digest",
            )?,
            created_at: UtcTimestamp::from_str(&existing.3)?,
        };
        if decoded.request_digest != receipt.request_digest {
            return Err(StoreError::IdempotencyKeyReused {
                expected: decoded.request_digest,
                observed: receipt.request_digest,
            });
        }
        Ok(decoded)
    }

    fn persist_child_result(&self, record: &ChildResultRecord) -> Result<(), StoreError> {
        let changed = self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO child_result(delegation_id,schema_version,result_digest,byte_len,validation_status,artifact_ref,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(delegation_id) DO UPDATE SET delegation_id=excluded.delegation_id WHERE child_result.schema_version=excluded.schema_version AND child_result.result_digest=excluded.result_digest AND child_result.byte_len=excluded.byte_len AND child_result.validation_status=excluded.validation_status AND child_result.artifact_ref=excluded.artifact_ref AND child_result.created_at=excluded.created_at AND child_result.updated_at=excluded.updated_at",
            params![record.delegation_id.to_string(), i64::from(record.schema_version), record.result_digest.to_string(), to_i64(record.byte_length, "child_result.byte_len")?, record.validation_status.as_ref(), record.artifact_ref.as_ref(), record.created_at.to_string(), record.updated_at.to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "child_result",
            });
        }
        Ok(())
    }

    fn persist_memory_cleanup(&self, receipt: &MemoryCleanupReceipt) -> Result<(), StoreError> {
        let changed = self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO memory_cleanup_receipt(delegation_id,namespace,snapshot_digest,cleanup_digest,cleaned_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(delegation_id) DO UPDATE SET delegation_id=excluded.delegation_id WHERE memory_cleanup_receipt.namespace=excluded.namespace AND memory_cleanup_receipt.snapshot_digest=excluded.snapshot_digest AND memory_cleanup_receipt.cleanup_digest=excluded.cleanup_digest AND memory_cleanup_receipt.cleaned_at=excluded.cleaned_at",
            params![receipt.delegation_id.to_string(), receipt.namespace.as_ref(), receipt.snapshot_digest.to_string(), receipt.cleanup_digest.to_string(), receipt.cleaned_at.to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "memory_cleanup_receipt",
            });
        }
        Ok(())
    }

    fn persist_host_adapter(&self, record: &HostAdapterRecord) -> Result<(), StoreError> {
        let changed = self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO host_adapter_instance(adapter_id,capability_digest,status,last_command_seq,heartbeat_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(adapter_id) DO UPDATE SET capability_digest=excluded.capability_digest,status=excluded.status,last_command_seq=MAX(host_adapter_instance.last_command_seq,excluded.last_command_seq),heartbeat_at=excluded.heartbeat_at,updated_at=excluded.updated_at",
            params![record.adapter_id.as_ref(), record.capability_digest.to_string(), record.status.as_ref(), to_i64(record.last_command_sequence, "host_adapter_instance.last_command_seq")?, record.heartbeat_at.to_string(), record.created_at.to_string(), record.updated_at.to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "host_action",
            });
        }
        Ok(())
    }

    fn persist_host_action(&self, record: &HostActionRecord) -> Result<(), StoreError> {
        self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO host_action(action_id,adapter_id,kind,command_seq,request_digest,session_id,context_generation,ack_status,deadline,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(action_id) DO UPDATE SET ack_status=excluded.ack_status,updated_at=excluded.updated_at WHERE host_action.adapter_id=excluded.adapter_id AND host_action.kind=excluded.kind AND host_action.command_seq=excluded.command_seq AND host_action.request_digest=excluded.request_digest",
            params![
                record.action_id.to_string(), record.adapter_id.as_ref(), record.kind.as_ref(),
                to_i64(record.command_sequence, "host_action.command_seq")?,
                record.request_digest.to_string(), record.session_id.map(|value| value.to_string()),
                record.context_generation.map(|value| to_i64(value.get(), "host_action.context_generation")).transpose()?,
                record.ack_status.as_ref(), record.deadline.to_string(), record.deadline.to_string(), record.deadline.to_string(),
            ],
        )?;
        Ok(())
    }

    fn put_host_ack(&self, receipt: &HostAckReceipt) -> Result<HostAckReceipt, StoreError> {
        let connection = self
            .connection
            .lock()
            .expect("SQLite repository lock is not poisoned");
        let existing: Option<(Option<String>, Option<String>, String)> = connection
            .query_row(
                "SELECT ack_id,response_digest,updated_at FROM host_action WHERE action_id=?1 AND adapter_id=?2",
                params![receipt.action_id.to_string(), receipt.adapter_id.as_ref()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((ack_id, response_digest, acknowledged_at)) = existing else {
            return Err(StoreError::PersistenceConflict {
                entity: "host_ack.action",
            });
        };
        if let Some(ack_id) = ack_id {
            let existing = HostAckReceipt {
                ack_id: parse_id(&ack_id, "host_action.ack_id")?,
                action_id: receipt.action_id,
                adapter_id: receipt.adapter_id.clone(),
                response_digest: parse_digest(
                    response_digest
                        .as_deref()
                        .ok_or_else(|| StoreError::DatabaseIncompatible {
                            reason: "host action ACK is missing response digest".into(),
                        })?,
                    "host_action.response_digest",
                )?,
                acknowledged_at: UtcTimestamp::from_str(&acknowledged_at)?,
            };
            if &existing != receipt {
                return Err(StoreError::PersistenceConflict { entity: "host_ack" });
            }
            return Ok(existing);
        }
        connection.execute(
            "UPDATE host_action SET ack_id=?1,response_digest=?2,ack_status='acknowledged',updated_at=?3 WHERE action_id=?4 AND adapter_id=?5 AND ack_id IS NULL",
            params![receipt.ack_id.to_string(), receipt.response_digest.to_string(), receipt.acknowledged_at.to_string(), receipt.action_id.to_string(), receipt.adapter_id.as_ref()],
        )?;
        Ok(receipt.clone())
    }

    fn persist_pressure_sample(
        &self,
        record: &ContextPressureSampleRecord,
    ) -> Result<(), StoreError> {
        if record.context_window_tokens == 0 || record.used_tokens > record.context_window_tokens {
            return Err(StoreError::PersistenceConflict {
                entity: "context_pressure_sample.tokens",
            });
        }
        let changed = self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO context_pressure_sample(adapter_id,session_id,context_generation,sample_seq,used_tokens,context_window_tokens,source,observed_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(adapter_id,session_id,sample_seq) DO UPDATE SET sample_seq=excluded.sample_seq WHERE context_pressure_sample.context_generation=excluded.context_generation AND context_pressure_sample.used_tokens=excluded.used_tokens AND context_pressure_sample.context_window_tokens=excluded.context_window_tokens AND context_pressure_sample.source=excluded.source AND context_pressure_sample.observed_at=excluded.observed_at",
            params![record.adapter_id.as_ref(), record.session_id.to_string(), to_i64(record.context_generation.get(), "context_pressure_sample.context_generation")?, to_i64(record.sample_sequence, "context_pressure_sample.sample_seq")?, to_i64(record.used_tokens, "context_pressure_sample.used_tokens")?, to_i64(record.context_window_tokens, "context_pressure_sample.context_window_tokens")?, record.source.as_ref(), record.observed_at.to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "context_pressure_sample",
            });
        }
        Ok(())
    }

    fn persist_context_projection(
        &self,
        record: &ContextProjectionRecord,
    ) -> Result<(), StoreError> {
        let changed = self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO context_projection(projection_id,session_id,context_revision,source_revision,policy_digest,inventory_generation,digest,byte_budget,expires_at,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(projection_id) DO UPDATE SET projection_id=excluded.projection_id WHERE context_projection.session_id=excluded.session_id AND context_projection.context_revision=excluded.context_revision AND context_projection.source_revision=excluded.source_revision AND context_projection.policy_digest=excluded.policy_digest AND context_projection.inventory_generation=excluded.inventory_generation AND context_projection.digest=excluded.digest AND context_projection.byte_budget=excluded.byte_budget AND context_projection.expires_at=excluded.expires_at AND context_projection.created_at=excluded.created_at",
            params![record.projection_id.to_string(), record.session_id.to_string(), to_i64(record.context_revision.get(), "context_projection.context_revision")?, to_i64(record.source_revision.get(), "context_projection.source_revision")?, record.policy_digest.to_string(), to_i64(record.inventory_generation.get(), "context_projection.inventory_generation")?, record.digest.to_string(), to_i64(record.byte_budget, "context_projection.byte_budget")?, record.expires_at.to_string(), record.expires_at.to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "context_projection",
            });
        }
        Ok(())
    }

    fn persist_compact_cycle(&self, record: &CompactCycleRecord) -> Result<(), StoreError> {
        if record.next_generation <= record.previous_generation {
            return Err(StoreError::PersistenceConflict {
                entity: "compact_cycle.generation",
            });
        }
        let changed = self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO compact_cycle(compact_id,session_id,snapshot_ref,previous_generation,next_generation,host_action_id,status,deadline,restored_digest,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(compact_id) DO UPDATE SET status=excluded.status,restored_digest=excluded.restored_digest,updated_at=excluded.updated_at WHERE compact_cycle.session_id=excluded.session_id AND compact_cycle.snapshot_ref=excluded.snapshot_ref AND compact_cycle.previous_generation=excluded.previous_generation AND compact_cycle.next_generation=excluded.next_generation AND compact_cycle.host_action_id=excluded.host_action_id",
            params![record.compact_id.to_string(), record.session_id.to_string(), record.snapshot_ref.as_ref(), to_i64(record.previous_generation.get(), "compact_cycle.previous_generation")?, to_i64(record.next_generation.get(), "compact_cycle.next_generation")?, record.host_action_id.to_string(), record.status.as_ref(), record.deadline.to_string(), record.restored_digest.map(|value| value.to_string()), record.deadline.to_string(), record.deadline.to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "compact_cycle",
            });
        }
        Ok(())
    }

    fn persist_supervisor_checkpoint(
        &self,
        record: &SupervisorCheckpointRecord,
    ) -> Result<(), StoreError> {
        let changed = self.connection.lock().expect("SQLite repository lock is not poisoned").execute(
            "INSERT INTO supervisor_checkpoint(workspace_id,work_item_id,last_event_seq,last_event_digest,state_revision,input_fingerprint,policy_digest,last_decision_digest,health,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(workspace_id,work_item_id) DO UPDATE SET last_event_seq=excluded.last_event_seq,last_event_digest=excluded.last_event_digest,state_revision=excluded.state_revision,input_fingerprint=excluded.input_fingerprint,policy_digest=excluded.policy_digest,last_decision_digest=excluded.last_decision_digest,health=excluded.health,updated_at=excluded.updated_at WHERE excluded.last_event_seq > supervisor_checkpoint.last_event_seq OR (excluded.last_event_seq = supervisor_checkpoint.last_event_seq AND excluded.last_event_digest=supervisor_checkpoint.last_event_digest AND excluded.state_revision=supervisor_checkpoint.state_revision AND excluded.input_fingerprint=supervisor_checkpoint.input_fingerprint AND excluded.policy_digest=supervisor_checkpoint.policy_digest AND excluded.last_decision_digest=supervisor_checkpoint.last_decision_digest AND excluded.health=supervisor_checkpoint.health AND excluded.updated_at=supervisor_checkpoint.updated_at)",
            params![record.workspace_id.to_string(), record.work_item_id.to_string(), to_i64(record.last_event_sequence.get(), "supervisor_checkpoint.last_event_seq")?, record.last_event_digest.to_string(), to_i64(record.state_revision.get(), "supervisor_checkpoint.state_revision")?, record.input_fingerprint.to_string(), record.policy_digest.to_string(), record.last_decision_digest.to_string(), record.health.as_ref(), record.updated_at.to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::PersistenceConflict {
                entity: "supervisor_checkpoint.cursor",
            });
        }
        Ok(())
    }

    fn supervisor_checkpoint(
        &self,
        workspace_id: WorkspaceId,
        work_item_id: &WorkItemId,
    ) -> Result<Option<SupervisorCheckpointRecord>, StoreError> {
        self.connection.lock().expect("SQLite repository lock is not poisoned").query_row(
            "SELECT last_event_seq,last_event_digest,state_revision,input_fingerprint,policy_digest,last_decision_digest,health,updated_at FROM supervisor_checkpoint WHERE workspace_id=?1 AND work_item_id=?2",
            params![workspace_id.to_string(), work_item_id.to_string()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?, row.get::<_, String>(7)?)),
        ).optional()?.map(|row| {
            Ok(SupervisorCheckpointRecord {
                workspace_id,
                work_item_id: work_item_id.clone(),
                last_event_sequence: EventSequence::new(from_i64(row.0, "supervisor_checkpoint.last_event_seq")?),
                last_event_digest: parse_digest(&row.1, "supervisor_checkpoint.last_event_digest")?,
                state_revision: StateRevision::new(from_i64(row.2, "supervisor_checkpoint.state_revision")?),
                input_fingerprint: parse_digest(&row.3, "supervisor_checkpoint.input_fingerprint")?,
                policy_digest: parse_digest(&row.4, "supervisor_checkpoint.policy_digest")?,
                last_decision_digest: parse_digest(&row.5, "supervisor_checkpoint.last_decision_digest")?,
                health: row.6.into_boxed_str(),
                updated_at: UtcTimestamp::from_str(&row.7)?,
            })
        }).transpose()
    }

    fn put_hook_event_receipt(
        &self,
        receipt: &HookEventReceipt,
    ) -> Result<HookEventReceipt, StoreError> {
        let connection = self
            .connection
            .lock()
            .expect("SQLite repository lock is not poisoned");
        connection.execute(
            "INSERT OR IGNORE INTO hook_event_receipt(session_id,hook_event_id,request_digest,decision_digest,event_seq,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![receipt.session_id.to_string(), receipt.hook_event_id.as_ref(), receipt.request_digest.to_string(), receipt.decision_digest.to_string(), receipt.event_sequence.map(|value| to_i64(value.get(), "hook_event_receipt.event_seq")).transpose()?, receipt.created_at.to_string()],
        )?;
        let existing = connection.query_row(
            "SELECT request_digest,decision_digest,event_seq,created_at FROM hook_event_receipt WHERE session_id=?1 AND hook_event_id=?2",
            params![receipt.session_id.to_string(), receipt.hook_event_id.as_ref()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<i64>>(2)?, row.get::<_, String>(3)?)),
        )?;
        let decoded = HookEventReceipt {
            session_id: receipt.session_id,
            hook_event_id: receipt.hook_event_id.clone(),
            request_digest: parse_digest(&existing.0, "hook_event_receipt.request_digest")?,
            decision_digest: parse_digest(&existing.1, "hook_event_receipt.decision_digest")?,
            event_sequence: existing
                .2
                .map(|value| {
                    from_i64(value, "hook_event_receipt.event_seq").map(EventSequence::new)
                })
                .transpose()?,
            created_at: UtcTimestamp::from_str(&existing.3)?,
        };
        if decoded.request_digest != receipt.request_digest {
            return Err(StoreError::IdempotencyKeyReused {
                expected: decoded.request_digest,
                observed: receipt.request_digest,
            });
        }
        Ok(decoded)
    }
}

fn event_payload_columns(
    payload: &RuntimeEventPayload,
) -> Result<(Option<String>, Option<String>, String, u64), StoreError> {
    payload.validate()?;
    match payload {
        RuntimeEventPayload::InlineJson(bytes) => Ok((
            Some(
                String::from_utf8(bytes.clone()).map_err(|error| StoreError::InvalidJournal {
                    reason: error.to_string().into_boxed_str(),
                })?,
            ),
            None,
            payload.digest().to_string(),
            payload.byte_length(),
        )),
        RuntimeEventPayload::ArtifactRef {
            project_relative_path,
            ..
        } => Ok((
            None,
            Some(project_relative_path.to_string()),
            payload.digest().to_string(),
            payload.byte_length(),
        )),
    }
}

fn load_operation_receipt(
    connection: &Connection,
    workspace_id: WorkspaceId,
    idempotency_key: &str,
) -> Result<Option<(OperationReceipt, RuntimeEventRecord)>, StoreError> {
    type ReceiptRow = (
        String,
        String,
        String,
        i64,
        i64,
        i64,
        String,
        String,
        i64,
        String,
    );
    let receipt: Option<ReceiptRow> = connection.query_row(
        "SELECT work_item_id,payload_digest,operation,revision_before,revision_after,fencing_token,result_digest,mutation_id,event_seq,committed_at FROM operation_receipt WHERE workspace_id=?1 AND idempotency_key=?2",
        params![workspace_id.to_string(), idempotency_key],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?)),
    ).optional()?;
    let Some(row) = receipt else {
        return Ok(None);
    };
    let operation_receipt = OperationReceipt {
        workspace_id,
        work_item_id: parse_id(&row.0, "operation_receipt.work_item_id")?,
        idempotency_key: IdempotencyKey::new(idempotency_key)?,
        payload_digest: parse_digest(&row.1, "operation_receipt.payload_digest")?,
        operation: parse_id(&row.2, "operation_receipt.operation")?,
        revision_before: StateRevision::new(from_i64(row.3, "operation_receipt.revision_before")?),
        revision_after: StateRevision::new(from_i64(row.4, "operation_receipt.revision_after")?),
        fencing_token: FencingToken::new(from_i64(row.5, "operation_receipt.fencing_token")?),
        result_digest: parse_digest(&row.6, "operation_receipt.result_digest")?,
        mutation_id: parse_id(&row.7, "operation_receipt.mutation_id")?,
        committed_at: UtcTimestamp::from_str(&row.9)?,
    };
    let event = load_event(connection, row.8)?;
    Ok(Some((operation_receipt, event)))
}

fn load_event(
    connection: &Connection,
    event_sequence: i64,
) -> Result<RuntimeEventRecord, StoreError> {
    type EventRow = (
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        i64,
        Option<String>,
        Option<String>,
        String,
        i64,
        String,
    );
    let row: EventRow = connection.query_row(
        "SELECT event_store_id,boot_id,workspace_id,session_id,work_item_id,event_type,schema_version,payload_json,payload_ref,payload_digest,byte_len,committed_at FROM runtime_event WHERE event_seq=?1",
        params![event_sequence],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?)),
    )?;
    let expected_digest: ArtifactDigest = parse_digest(&row.9, "runtime_event.payload_digest")?;
    let byte_length = from_i64(row.10, "runtime_event.byte_len")?;
    let payload = match (row.7, row.8) {
        (Some(json), None) => {
            let bytes = json.into_bytes();
            if ArtifactDigest::digest(&bytes) != expected_digest
                || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != byte_length
            {
                return Err(StoreError::DatabaseIncompatible {
                    reason: "runtime event inline payload failed integrity validation".into(),
                });
            }
            RuntimeEventPayload::InlineJson(bytes)
        }
        (None, Some(reference)) => RuntimeEventPayload::ArtifactRef {
            project_relative_path: reference.into_boxed_str(),
            digest: expected_digest,
            byte_length,
        },
        _ => {
            return Err(StoreError::DatabaseIncompatible {
                reason: "runtime event contains an invalid payload union".into(),
            });
        }
    };
    let draft = RuntimeEventDraft {
        boot_id: parse_id(&row.1, "runtime_event.boot_id")?,
        workspace_id: parse_id(&row.2, "runtime_event.workspace_id")?,
        session_id: row
            .3
            .map(|value| parse_id(&value, "runtime_event.session_id"))
            .transpose()?,
        work_item_id: parse_id(&row.4, "runtime_event.work_item_id")?,
        event_type: row.5.into_boxed_str(),
        schema_version: u32::try_from(row.6).map_err(|_| StoreError::DatabaseIncompatible {
            reason: "runtime event schema version is out of range".into(),
        })?,
        payload,
        committed_at: UtcTimestamp::from_str(&row.11)?,
    };
    draft.validate()?;
    Ok(RuntimeEventRecord {
        event_store_id: parse_id(&row.0, "runtime_event.event_store_id")?,
        event_sequence: EventSequence::new(from_i64(event_sequence, "runtime_event.event_seq")?),
        draft,
    })
}

type DelegationRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    i64,
    String,
    String,
    String,
    String,
);

fn decode_delegation(id: DelegationId, row: DelegationRow) -> Result<DelegationRecord, StoreError> {
    Ok(DelegationRecord {
        delegation_id: id,
        workspace_id: parse_id(&row.0, "delegation.workspace_id")?,
        root_session_id: parse_id(&row.1, "delegation.root_session_id")?,
        parent_session_id: parse_id(&row.2, "delegation.parent_session_id")?,
        child_session_id: row
            .3
            .map(|value| parse_id(&value, "delegation.child_session_id"))
            .transpose()?,
        parent_delegation_id: row
            .4
            .map(|value| parse_id(&value, "delegation.parent_delegation_id"))
            .transpose()?,
        role: row.5.into_boxed_str(),
        input_revision: StateRevision::new(from_i64(row.6, "delegation.input_revision")?),
        input_fingerprint: parse_digest(&row.7, "delegation.input_fingerprint")?,
        status: row.8.into_boxed_str(),
        deadline: UtcTimestamp::from_str(&row.9)?,
        receipt_digest: parse_digest(&row.10, "delegation.receipt_digest")?,
    })
}

fn to_i64(value: u64, field: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::DatabaseIncompatible {
        reason: format!("{field} exceeds SQLite INTEGER range").into_boxed_str(),
    })
}

fn from_i64(value: i64, field: &'static str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::DatabaseIncompatible {
        reason: format!("{field} is negative").into_boxed_str(),
    })
}

fn parse_id<T>(value: &str, field: &'static str) -> Result<T, StoreError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| StoreError::DatabaseIncompatible {
            reason: format!("{field} is invalid: {error}").into_boxed_str(),
        })
}

fn parse_digest<T>(value: &str, field: &'static str) -> Result<T, StoreError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    parse_id(value, field)
}
