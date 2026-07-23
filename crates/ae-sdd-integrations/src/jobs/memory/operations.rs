use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{PersistencePort, RuntimeError, RuntimeResult};
use serde_json::{Value, json};

use super::compiler::{CompiledMemory, compile_memory, extract_common, refresh_manifest};
use super::input::{
    EntityScope, argument_object, assert_project, common_action, common_scope, content_argument,
    reject_unknown, required_text, resolve_scope, source_contexts, structured_context,
};
use super::store::{
    MemoryRecord, MemorySlices, MutationStamp, decode_record, load_record, record_key, store_record,
};
use super::{
    MAX_ENTITY_RECORDS, MAX_SLICE_BYTES, MEMORY_SCHEMA, MemoryContext, MutationContext,
    canonical_digest, schema_error,
};

const MAX_QUERY_BYTES: usize = 4 * 1024;
const MAX_SEARCH_LIMIT: u64 = 100;

pub(super) fn dispatch(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    mutation: Option<&MutationContext<'_>>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    match entrypoint {
        "memory.create" => create(
            context,
            persistence,
            required_mutation(mutation)?,
            arguments,
        ),
        "memory.read" => read(context, persistence, arguments),
        "memory.update" => update(
            context,
            persistence,
            required_mutation(mutation)?,
            arguments,
        ),
        "memory.clean" => clean(
            context,
            persistence,
            required_mutation(mutation)?,
            arguments,
        ),
        "memory.clean-all" => clean_all(
            context,
            persistence,
            required_mutation(mutation)?,
            arguments,
        ),
        "memory.common" => common(context, persistence, mutation, arguments),
        "memory.search" => search(context, persistence, arguments),
        "memory.summarize" => summarize(context, persistence, arguments),
        _ => unreachable!("memory entrypoint registry and dispatcher diverged"),
    }
}

fn create(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    mutation: &MutationContext<'_>,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "context",
            "contextJson",
            "entityId",
            "entityType",
            "phase",
            "project",
            "sources",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    let scope = resolve_scope(object)?;
    let sources = source_contexts(context, object.get("sources"))?;
    let context_value = structured_context(object)?;
    let compiled = compile_memory(
        &scope.entity_type,
        &scope.entity_id,
        &sources.hashes,
        &context_value,
    )?;
    enforce_compiled_bounds(&compiled)?;
    ensure_record_capacity(persistence, context, &scope)?;
    let key = record_key(&scope.entity_type, &scope.entity_id);
    let existing = load_record(persistence, &context.namespace, &key, &context.authority)?;
    if let Some(record) = existing.as_ref()
        && let Some(response) = replay_record_mutation(record, mutation)?
    {
        return Ok(response);
    }
    let revision = existing.map_or(1, |record| record.revision.saturating_add(1));
    let response = json!({
        "outcome":"PASS",
        "created":true,
        "entity_type":scope.entity_type,
        "entity_id":scope.entity_id,
        "revision":revision,
        "slices":["boot.compact.md","context.compact.md","pending.compact.md","manifest.json"],
    });
    let mut record = MemoryRecord {
        schema_version: MEMORY_SCHEMA.to_owned(),
        authority: context.authority.clone(),
        entity_type: scope.entity_type.clone(),
        entity_id: scope.entity_id.clone(),
        active: true,
        revision,
        slices: MemorySlices {
            boot: compiled.boot,
            context: compiled.context,
            pending: compiled.pending,
        },
        manifest: compiled.manifest,
        last_mutation: None,
    };
    stamp_record_mutation(&mut record, mutation, &response);
    store_record(persistence, &context.namespace, &key, &record)?;
    if scope.entity_type != "common" {
        maybe_create_common(context, persistence, &sources.contents)?;
    }
    Ok(response)
}

fn read(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "entityId",
            "entityType",
            "phase",
            "project",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    read_scope(context, persistence, &resolve_scope(object)?)
}

fn update(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    mutation: &MutationContext<'_>,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "content",
            "contentFile",
            "entityId",
            "entityType",
            "phase",
            "project",
            "slice",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    let scope = resolve_scope(object)?;
    let slice = required_text(object, "slice", 32)?;
    if !matches!(slice, "boot" | "context" | "pending") {
        return Err(schema_error("slice must be boot, context, or pending"));
    }
    let content = content_argument(context, object, false)?;
    let key = record_key(&scope.entity_type, &scope.entity_id);
    let mut record = load_record(persistence, &context.namespace, &key, &context.authority)?
        .filter(|record| record.active)
        .ok_or_else(|| schema_error("memory does not exist in the trusted session namespace"))?;
    if let Some(response) = replay_record_mutation(&record, mutation)? {
        return Ok(response);
    }
    match slice {
        "boot" => record.slices.boot = content,
        "context" => record.slices.context = content,
        "pending" => record.slices.pending = content,
        _ => unreachable!(),
    }
    record.revision = record.revision.saturating_add(1);
    refresh_manifest(&mut record.manifest, &record.slices)?;
    let response = json!({
        "outcome":"PASS",
        "updated":true,
        "slice":slice,
        "entity_type":scope.entity_type,
        "entity_id":scope.entity_id,
        "revision":record.revision,
    });
    stamp_record_mutation(&mut record, mutation, &response);
    store_record(persistence, &context.namespace, &key, &record)?;
    Ok(response)
}

fn clean(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    mutation: &MutationContext<'_>,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "entityId",
            "entityType",
            "phase",
            "project",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    let scope = resolve_scope(object)?;
    if scope.entity_type == "common" {
        return Ok(json!({
            "outcome":"PASS",
            "cleaned":false,
            "reason":"common memory is preserved; use memory common clean",
        }));
    }
    tombstone(context, persistence, mutation, &scope)
}

fn clean_all(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    mutation: &MutationContext<'_>,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "entityId",
            "entityType",
            "phase",
            "project",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    let records = bounded_records(persistence, context)?;
    let mut removed = BTreeSet::new();
    let mut changed_records = Vec::new();
    for (key, value) in records {
        let mut record = decode_record(&value, &context.authority)?;
        if let Some(response) = replay_record_mutation(&record, mutation)? {
            return Ok(response);
        }
        if record.entity_type == "common" {
            continue;
        }
        removed.insert(record.entity_type.clone());
        if record.active {
            record.active = false;
            record.revision = record.revision.saturating_add(1);
            record.slices = MemorySlices::default();
            record.manifest = Value::Null;
            changed_records.push((key, record));
        }
    }
    let removed_types = removed.into_iter().collect::<Vec<_>>();
    let cleanup_digest = canonical_digest(&json!({
        "authority":context.authority,
        "removedTypes":removed_types,
        "preserved":["common"],
    }))?;
    let response = json!({
        "outcome":"PASS",
        "cleaned":true,
        "removed_types":removed_types,
        "preserved":["common"],
        "cleanupReceipt":{
            "schemaVersion":"memory-cleanup/v1",
            "scope":"trusted-session",
            "cleanupDigest":cleanup_digest,
        },
    });
    for (key, mut record) in changed_records {
        stamp_record_mutation(&mut record, mutation, &response);
        store_record(persistence, &context.namespace, &key, &record)?;
    }
    Ok(response)
}

fn common(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    mutation: Option<&MutationContext<'_>>,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "action",
            "commonAction",
            "content",
            "contentFile",
            "entityId",
            "entityType",
            "phase",
            "project",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    let scope = common_scope();
    match common_action(object)? {
        "read" => read_scope(context, persistence, &scope),
        "update" => {
            let mutation = required_mutation(mutation)?;
            let content = content_argument(context, object, true)?;
            let key = record_key("common", "default");
            let mut record =
                load_record(persistence, &context.namespace, &key, &context.authority)?
                    .unwrap_or_else(|| {
                        MemoryRecord::common(context.authority.clone(), String::new())
                    });
            if let Some(response) = replay_record_mutation(&record, mutation)? {
                return Ok(response);
            }
            record.active = true;
            record.revision = record.revision.saturating_add(1);
            record.slices.context = content;
            let response = json!({
                "outcome":"PASS",
                "updated":true,
                "entity_type":"common",
                "entity_id":"default",
                "revision":record.revision,
            });
            stamp_record_mutation(&mut record, mutation, &response);
            store_record(persistence, &context.namespace, &key, &record)?;
            Ok(response)
        }
        "clean" => tombstone(context, persistence, required_mutation(mutation)?, &scope),
        _ => unreachable!(),
    }
}

fn search(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "entityId",
            "entityType",
            "limit",
            "phase",
            "project",
            "query",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    let query = required_text(object, "query", MAX_QUERY_BYTES)?;
    let limit = object.get("limit").and_then(Value::as_u64).unwrap_or(20);
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(schema_error("limit must be between 1 and 100"));
    }
    let needle = query.to_lowercase();
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let mut entries = Vec::new();
    for (_, value) in bounded_records(persistence, context)? {
        let record = decode_record(&value, &context.authority)?;
        if !record.active {
            continue;
        }
        for (slice, content) in record.slices.iter() {
            if content.to_lowercase().contains(&needle) {
                entries.push(json!({
                    "entity":record.entity_id,
                    "entity_type":record.entity_type,
                    "slice":format!("{slice}.compact"),
                    "snippet":content.chars().take(200).collect::<String>(),
                }));
                if entries.len() >= limit {
                    break;
                }
            }
        }
        if entries.len() >= limit {
            break;
        }
    }
    Ok(json!({"outcome":"PASS","entries":entries,"count":entries.len()}))
}

fn summarize(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let object = argument_object(arguments)?;
    reject_unknown(
        object,
        &[
            "entityId",
            "entityType",
            "phase",
            "project",
            "story",
            "task",
        ],
    )?;
    assert_project(context, object.get("project"))?;
    let mut counts = BTreeMap::<String, u64>::new();
    let mut total = 0_u64;
    for (_, value) in bounded_records(persistence, context)? {
        let record = decode_record(&value, &context.authority)?;
        if !record.active {
            continue;
        }
        let count = record.slices.non_empty_count();
        if count > 0 {
            *counts.entry(record.entity_type).or_default() += count;
            total += count;
        }
    }
    Ok(json!({
        "outcome":"PASS",
        "total_slices":total,
        "by_entity_type":counts,
        "scope":"trusted-session",
    }))
}

fn read_scope(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    scope: &EntityScope,
) -> RuntimeResult<Value> {
    let key = record_key(&scope.entity_type, &scope.entity_id);
    let Some(record) = load_record(persistence, &context.namespace, &key, &context.authority)?
        .filter(|record| record.active)
    else {
        return Ok(json!({
            "outcome":"PASS",
            "found":false,
            "entity_type":scope.entity_type,
            "entity_id":scope.entity_id,
        }));
    };
    let value = json!({
        "outcome":"PASS",
        "found":true,
        "entity_type":record.entity_type,
        "entity_id":record.entity_id,
        "revision":record.revision,
        "boot":record.slices.boot,
        "context":record.slices.context,
        "pending":record.slices.pending,
        "manifest":record.manifest,
    });
    let bytes = serde_json::to_vec(&value)
        .map_err(|_| schema_error("memory projection could not be serialized"))?;
    if bytes.len() > 64 * 1024 {
        return Err(schema_error("memory read projection exceeds 64 KiB"));
    }
    Ok(value)
}

fn tombstone(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    mutation: &MutationContext<'_>,
    scope: &EntityScope,
) -> RuntimeResult<Value> {
    let key = record_key(&scope.entity_type, &scope.entity_id);
    let Some(mut record) = load_record(persistence, &context.namespace, &key, &context.authority)?
    else {
        return Ok(json!({
            "outcome":"PASS",
            "cleaned":false,
            "reason":"memory not found in trusted session namespace",
            "entity_type":scope.entity_type,
            "entity_id":scope.entity_id,
        }));
    };
    if let Some(response) = replay_record_mutation(&record, mutation)? {
        return Ok(response);
    }
    let cleaned = record.active;
    if cleaned {
        record.active = false;
        record.revision = record.revision.saturating_add(1);
        record.slices = MemorySlices::default();
        record.manifest = Value::Null;
    }
    let cleanup_digest = canonical_digest(&json!({
        "authority":context.authority,
        "entityType":scope.entity_type,
        "entityId":scope.entity_id,
        "revision":record.revision,
    }))?;
    let response = json!({
        "outcome":"PASS",
        "cleaned":cleaned,
        "entity_type":scope.entity_type,
        "entity_id":scope.entity_id,
        "cleanupReceipt":{
            "schemaVersion":"memory-cleanup/v1",
            "scope":"trusted-session",
            "cleanupDigest":cleanup_digest,
        },
    });
    stamp_record_mutation(&mut record, mutation, &response);
    store_record(persistence, &context.namespace, &key, &record)?;
    Ok(response)
}

fn maybe_create_common(
    context: &MemoryContext<'_>,
    persistence: &dyn PersistencePort,
    sources: &BTreeMap<String, String>,
) -> RuntimeResult<()> {
    let key = record_key("common", "default");
    if load_record(persistence, &context.namespace, &key, &context.authority)?
        .is_some_and(|record| record.active)
    {
        return Ok(());
    }
    let record = MemoryRecord::common(context.authority.clone(), extract_common(sources));
    store_record(persistence, &context.namespace, &key, &record)
}

fn ensure_record_capacity(
    persistence: &dyn PersistencePort,
    context: &MemoryContext<'_>,
    scope: &EntityScope,
) -> RuntimeResult<()> {
    let records = bounded_records(persistence, context)?;
    let key = record_key(&scope.entity_type, &scope.entity_id);
    if records.iter().any(|(record_key, _)| record_key == &key) {
        return Ok(());
    }
    if records.len() >= MAX_ENTITY_RECORDS {
        return Err(schema_error(
            "trusted memory namespace record bound exceeded",
        ));
    }
    Ok(())
}

fn bounded_records(
    persistence: &dyn PersistencePort,
    context: &MemoryContext<'_>,
) -> RuntimeResult<Vec<(String, Value)>> {
    let records = persistence.list_records(&context.namespace)?;
    if records.len() > MAX_ENTITY_RECORDS {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "durable memory namespace exceeds its record bound",
        ));
    }
    Ok(records)
}

fn enforce_compiled_bounds(compiled: &CompiledMemory) -> RuntimeResult<()> {
    for content in [&compiled.boot, &compiled.context, &compiled.pending] {
        if content.len() > MAX_SLICE_BYTES {
            return Err(schema_error("compiled memory slice exceeds 16 KiB"));
        }
    }
    let total = compiled.boot.len()
        + compiled.context.len()
        + compiled.pending.len()
        + serde_json::to_vec(&compiled.manifest)
            .map_err(|_| schema_error("memory manifest could not be serialized"))?
            .len();
    if total > 56 * 1024 {
        return Err(schema_error("compiled memory projection exceeds 56 KiB"));
    }
    Ok(())
}

fn required_mutation<'a, 'b>(
    mutation: Option<&'a MutationContext<'b>>,
) -> RuntimeResult<&'a MutationContext<'b>> {
    mutation.ok_or_else(|| schema_error("memory mutation context is required"))
}

fn replay_record_mutation(
    record: &MemoryRecord,
    mutation: &MutationContext<'_>,
) -> RuntimeResult<Option<Value>> {
    let Some(stamp) = record
        .last_mutation
        .as_ref()
        .filter(|stamp| stamp.idempotency_key == mutation.idempotency_key)
    else {
        return Ok(None);
    };
    if stamp.request_digest != mutation.request_digest {
        return Err(RuntimeError::new(
            StableErrorCode::IdempotencyKeyReused,
            "memory idempotency key was reused with a different payload",
        ));
    }
    let mut response = stamp.response.clone();
    if let Some(object) = response.as_object_mut() {
        object.insert("replayed".to_owned(), Value::Bool(true));
    }
    Ok(Some(response))
}

fn stamp_record_mutation(
    record: &mut MemoryRecord,
    mutation: &MutationContext<'_>,
    response: &Value,
) {
    record.last_mutation = Some(MutationStamp {
        idempotency_key: mutation.idempotency_key.to_owned(),
        request_digest: mutation.request_digest.to_owned(),
        response: response.clone(),
    });
}
