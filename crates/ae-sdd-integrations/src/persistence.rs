use std::path::Path;
use std::sync::Mutex;

use ae_sdd_domain::{EventStoreId, InputFingerprint};
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{
    DurableEvent, IdempotencyReceipt, PersistencePort, RuntimeError, RuntimeResult,
};
use ae_sdd_store::{RuntimeRepository, SqliteRuntimeRepository, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;
use uuid::Uuid;

/// SQLite/WAL implementation of runtime metadata, event, receipt, and checkpoint ports.
pub struct SqliteRuntimePersistence {
    repository: SqliteRuntimeRepository,
    connection: Mutex<Connection>,
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

    fn store_record(&self, namespace: &str, key: &str, value: &Value) -> RuntimeResult<()> {
        let json = serde_json::to_string(value).map_err(canonical_error)?;
        self.connection()?
            .execute(
                "INSERT INTO runtime_record_v1(namespace,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(namespace,key) DO UPDATE SET value_json=excluded.value_json",
                params![namespace, key, json],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }
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
