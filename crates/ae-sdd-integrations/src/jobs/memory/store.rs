use ae_sdd_runtime::{PersistencePort, RuntimeError, RuntimeResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::MemoryAuthority;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MemorySlices {
    pub(super) boot: String,
    pub(super) context: String,
    pub(super) pending: String,
}

impl MemorySlices {
    pub(super) fn iter(&self) -> [(&'static str, &str); 3] {
        [
            ("boot", self.boot.as_str()),
            ("context", self.context.as_str()),
            ("pending", self.pending.as_str()),
        ]
    }

    pub(super) fn non_empty_count(&self) -> u64 {
        self.iter()
            .into_iter()
            .filter(|(_, value)| !value.is_empty())
            .count() as u64
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MemoryRecord {
    pub(super) schema_version: String,
    pub(super) authority: MemoryAuthority,
    pub(super) entity_type: String,
    pub(super) entity_id: String,
    pub(super) active: bool,
    pub(super) revision: u64,
    pub(super) slices: MemorySlices,
    pub(super) manifest: Value,
    #[serde(default)]
    pub(super) last_mutation: Option<MutationStamp>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MutationStamp {
    pub(super) idempotency_key: String,
    pub(super) request_digest: String,
    pub(super) response: Value,
}

impl MemoryRecord {
    pub(super) fn common(authority: MemoryAuthority, context: String) -> Self {
        Self {
            schema_version: super::MEMORY_SCHEMA.to_owned(),
            authority,
            entity_type: "common".to_owned(),
            entity_id: "default".to_owned(),
            active: true,
            revision: 1,
            slices: MemorySlices {
                context,
                ..MemorySlices::default()
            },
            manifest: Value::Null,
            last_mutation: None,
        }
    }
}

pub(super) fn record_key(entity_type: &str, entity_id: &str) -> String {
    format!("{entity_type}/{entity_id}")
}

pub(super) fn load_record(
    persistence: &dyn PersistencePort,
    namespace: &str,
    key: &str,
    authority: &MemoryAuthority,
) -> RuntimeResult<Option<MemoryRecord>> {
    persistence
        .load_record(namespace, key)?
        .map(|value| decode_record(&value, authority))
        .transpose()
}

pub(super) fn decode_record(
    value: &Value,
    authority: &MemoryAuthority,
) -> RuntimeResult<MemoryRecord> {
    let record: MemoryRecord = serde_json::from_value(value.clone()).map_err(|_| {
        RuntimeError::new(
            ae_sdd_protocol::StableErrorCode::ExternalStateConflict,
            "durable memory record is malformed",
        )
    })?;
    if record.schema_version != super::MEMORY_SCHEMA || &record.authority != authority {
        return Err(RuntimeError::new(
            ae_sdd_protocol::StableErrorCode::ExternalStateConflict,
            "durable memory authority binding is inconsistent",
        ));
    }
    Ok(record)
}

pub(super) fn store_record(
    persistence: &dyn PersistencePort,
    namespace: &str,
    key: &str,
    record: &MemoryRecord,
) -> RuntimeResult<()> {
    let value = serde_json::to_value(record).map_err(|_| {
        RuntimeError::new(
            ae_sdd_protocol::StableErrorCode::OperationSchemaInvalid,
            "memory record could not be serialized",
        )
    })?;
    persistence.store_record(namespace, key, &value)
}
