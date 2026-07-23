use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::schema_error;
use super::store::MemorySlices;

const MAX_ITEMS: usize = 128;
const MAX_CELL_BYTES: usize = 2 * 1024;

pub(super) struct CompiledMemory {
    pub(super) boot: String,
    pub(super) context: String,
    pub(super) pending: String,
    pub(super) manifest: Value,
}

pub(super) fn compile_memory(
    entity_type: &str,
    entity_id: &str,
    source_hashes: &BTreeMap<String, String>,
    context: &Value,
) -> RuntimeResult<CompiledMemory> {
    let object = context
        .as_object()
        .ok_or_else(|| schema_error("memory structured context must be an object"))?;
    reject_context_unknown(object)?;
    let boot = render_boot(entity_type, entity_id, object)?;
    let context_text = render_context(object)?;
    let pending = render_pending(object)?;
    let manifest = render_manifest(
        entity_type,
        entity_id,
        source_hashes,
        &boot,
        &context_text,
        &pending,
    )?;
    Ok(CompiledMemory {
        boot,
        context: context_text,
        pending,
        manifest,
    })
}

pub(super) fn refresh_manifest(manifest: &mut Value, slices: &MemorySlices) -> RuntimeResult<()> {
    if manifest.is_null() {
        return Ok(());
    }
    let object = manifest
        .as_object()
        .ok_or_else(|| schema_error("memory manifest is malformed"))?;
    let entity_type = object
        .get("entity_type")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("memory manifest entity_type is missing"))?
        .to_owned();
    let entity_id = object
        .get("entity_id")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("memory manifest entity_id is missing"))?
        .to_owned();
    let source_hashes = object
        .get("source_hashes")
        .and_then(Value::as_object)
        .map(|values| {
            values
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_owned()))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    *manifest = render_manifest(
        &entity_type,
        &entity_id,
        &source_hashes,
        &slices.boot,
        &slices.context,
        &slices.pending,
    )?;
    Ok(())
}

pub(super) fn extract_common(sources: &BTreeMap<String, String>) -> String {
    const KEYWORDS: [&str; 14] = [
        "must",
        "required",
        "forbidden",
        "idempotent",
        "BigDecimal",
        "SOLID",
        "DRY",
        "KISS",
        "禁止",
        "必须",
        "幂等",
        "敏感数据",
        "硬编码",
        "安全左移",
    ];
    let mut seen = BTreeSet::new();
    let mut lines = Vec::new();
    for content in sources.values() {
        for line in content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if KEYWORDS.iter().any(|keyword| line.contains(keyword)) && seen.insert(line.to_owned())
            {
                lines.push(line.to_owned());
            }
        }
    }
    let mut output = if lines.is_empty() {
        "# Common Context Compact\n\n(no reusable constraints extracted)\n".to_owned()
    } else {
        format!(
            "# Common Context Compact\n\n## Reusable Constraints\n\n{}\n",
            lines
                .into_iter()
                .map(|line| format!("- {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    if output.len() > super::MAX_COMMON_BYTES {
        const WARNING: &str = "\n\ncommon truncated at the 2 KiB daemon bound.\n";
        output.truncate(super::MAX_COMMON_BYTES.saturating_sub(WARNING.len()));
        output.push_str(WARNING);
    }
    output
}

fn render_boot(
    entity_type: &str,
    entity_id: &str,
    context: &Map<String, Value>,
) -> RuntimeResult<String> {
    let series_chain = string_array(context, "series_chain", "seriesChain")?;
    let current_series = string(context, "current_series", "currentSeries")?.unwrap_or_default();
    let next_step = string(context, "next_step", "nextStep")?.unwrap_or_default();
    let deliverables = object_array(context, "deliverables", "deliverables")?;
    let rows = if deliverables.is_empty() {
        vec![vec!["-".to_owned(), "-".to_owned(), "-".to_owned()]]
    } else {
        deliverables
            .iter()
            .map(|item| row(item, &["name", "path", "status"]))
            .collect::<RuntimeResult<Vec<_>>>()?
    };
    Ok(format!(
        "# Memory Boot Compact - {entity_type}/{entity_id}\n\n\
- entity_type: {entity_type}\n\
- entity_id: {entity_id}\n\
- current_series: {current_series}\n\
- next_step: {next_step}\n\
- deterministic: true\n\n\
## Series Chain\n\n{}\n\n\
## Deliverables\n\n{}\n",
        series_chain.join(" -> "),
        table(&["name", "path", "status"], &rows),
    ))
}

fn render_context(context: &Map<String, Value>) -> RuntimeResult<String> {
    let mut sections = vec!["# Memory Context Compact\n".to_owned()];
    let dr_anchors = object_array(context, "dr_anchors", "drAnchors")?;
    if !dr_anchors.is_empty() {
        let rows = dr_anchors
            .iter()
            .map(|item| row(item, &["section", "line", "summary"]))
            .collect::<RuntimeResult<Vec<_>>>()?;
        sections.push(format!(
            "## DR Anchors\n\n{}\n",
            table(&["section", "line", "summary"], &rows)
        ));
    }
    let story_acs = object_array(context, "story_acs", "storyAcs")?;
    if !story_acs.is_empty() {
        let rows = story_acs
            .iter()
            .map(|item| row(item, &["id", "description", "status"]))
            .collect::<RuntimeResult<Vec<_>>>()?;
        sections.push(format!(
            "## Story Acceptance Criteria\n\n{}\n",
            table(&["id", "description", "status"], &rows)
        ));
    }
    let constraints = string_array(context, "constraints", "constraints")?;
    if !constraints.is_empty() {
        sections.push(format!(
            "## Constraints\n\n{}\n",
            constraints
                .into_iter()
                .map(|value| format!("- {value}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    let api_contracts = object_array(context, "api_contracts", "apiContracts")?;
    if !api_contracts.is_empty() {
        let rows = api_contracts
            .iter()
            .map(|item| row(item, &["name", "method", "path", "request", "response"]))
            .collect::<RuntimeResult<Vec<_>>>()?;
        sections.push(format!(
            "## API Contracts\n\n{}\n",
            table(&["name", "method", "path", "request", "response"], &rows)
        ));
    }
    let data_models = object_array(context, "data_models", "dataModels")?;
    if !data_models.is_empty() {
        let rows = data_models
            .iter()
            .map(|item| row(item, &["table", "fields", "notes"]))
            .collect::<RuntimeResult<Vec<_>>>()?;
        sections.push(format!(
            "## Data Models\n\n{}\n",
            table(&["table", "fields", "notes"], &rows)
        ));
    }
    let asset_refs = string_array(context, "asset_refs", "assetRefs")?;
    if !asset_refs.is_empty() {
        sections.push(format!(
            "## Asset References\n\n{}\n",
            asset_refs
                .into_iter()
                .map(|value| format!("- `{value}`"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if sections.len() == 1 {
        sections.push("(no context extracted yet)\n".to_owned());
    }
    Ok(sections.join("\n"))
}

fn render_pending(context: &Map<String, Value>) -> RuntimeResult<String> {
    let review =
        string(context, "review_loop_status", "reviewLoopStatus")?.unwrap_or("(not started)");
    let mut sections = vec![format!(
        "# Memory Pending Compact\n\n## Review Loop Status\n\n{review}\n"
    )];
    let pending = object_array(context, "pending_items", "pendingItems")?;
    if !pending.is_empty() {
        let rows = pending
            .iter()
            .map(|item| row(item, &["id", "description", "owner", "status"]))
            .collect::<RuntimeResult<Vec<_>>>()?;
        sections.push(format!(
            "## Pending Items\n\n{}\n",
            table(&["id", "description", "owner", "status"], &rows)
        ));
    }
    let failures = object_array(context, "failure_history", "failureHistory")?;
    if !failures.is_empty() {
        let rows = failures
            .iter()
            .map(|item| row(item, &["round", "issue", "action"]))
            .collect::<RuntimeResult<Vec<_>>>()?;
        sections.push(format!(
            "## Failure History\n\n{}\n",
            table(&["round", "issue", "action"], &rows)
        ));
    }
    if let Some(corrections) = object(context, "correction_counts", "correctionCounts")?
        && !corrections.is_empty()
    {
        let mut rows = Vec::new();
        for (phase, value) in corrections {
            let count = value
                .as_u64()
                .ok_or_else(|| schema_error("correctionCounts values must be unsigned integers"))?;
            rows.push(vec![cell(phase)?, count.to_string()]);
        }
        sections.push(format!(
            "## Correction Counts\n\n{}\n",
            table(&["phase", "count"], &rows)
        ));
    }
    if sections.len() == 1 {
        sections.push("(no pending items)\n".to_owned());
    }
    Ok(sections.join("\n"))
}

fn render_manifest(
    entity_type: &str,
    entity_id: &str,
    source_hashes: &BTreeMap<String, String>,
    boot: &str,
    context: &str,
    pending: &str,
) -> RuntimeResult<Value> {
    let boot_sha = digest(boot.as_bytes());
    let context_sha = digest(context.as_bytes());
    let pending_sha = digest(pending.as_bytes());
    let fingerprint_payload = json!({
        "boot_sha256":boot_sha,
        "context_sha256":context_sha,
        "entity_id":entity_id,
        "entity_type":entity_type,
        "pending_sha256":pending_sha,
        "source_hashes":source_hashes,
    });
    let fingerprint = digest(
        &serde_json::to_vec(&fingerprint_payload)
            .map_err(|_| schema_error("memory fingerprint could not be serialized"))?,
    );
    Ok(json!({
        "schema":"ae-sdd-memory/v1",
        "entity_type":entity_type,
        "entity_id":entity_id,
        "deterministic":true,
        "fingerprint":fingerprint,
        "source_hashes":source_hashes,
        "slices":{
            "boot":{"path":"boot.compact.md","sha256":boot_sha},
            "context":{"path":"context.compact.md","sha256":context_sha},
            "pending":{"path":"pending.compact.md","sha256":pending_sha},
        },
    }))
}

fn reject_context_unknown(object: &Map<String, Value>) -> RuntimeResult<()> {
    const ALLOWED: [&str; 26] = [
        "apiContracts",
        "api_contracts",
        "assetRefs",
        "asset_refs",
        "constraints",
        "correctionCounts",
        "correction_counts",
        "currentSeries",
        "current_series",
        "dataModels",
        "data_models",
        "deliverables",
        "drAnchors",
        "dr_anchors",
        "failureHistory",
        "failure_history",
        "nextStep",
        "next_step",
        "pendingItems",
        "pending_items",
        "reviewLoopStatus",
        "review_loop_status",
        "seriesChain",
        "series_chain",
        "storyAcs",
        "story_acs",
    ];
    if let Some(field) = object
        .keys()
        .find(|field| !ALLOWED.contains(&field.as_str()))
    {
        return Err(schema_error(&format!(
            "unknown structured memory context field: {field}"
        )));
    }
    Ok(())
}

fn string<'a>(
    object: &'a Map<String, Value>,
    snake: &str,
    camel: &str,
) -> RuntimeResult<Option<&'a str>> {
    let Some(value) = one_alias(object, snake, camel)? else {
        return Ok(None);
    };
    let text = value
        .as_str()
        .ok_or_else(|| schema_error(&format!("{camel} must be text")))?;
    cell(text).map(|_| Some(text))
}

fn string_array(
    object: &Map<String, Value>,
    snake: &str,
    camel: &str,
) -> RuntimeResult<Vec<String>> {
    let Some(value) = one_alias(object, snake, camel)? else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .filter(|values| values.len() <= MAX_ITEMS)
        .ok_or_else(|| schema_error(&format!("{camel} must be a bounded array")))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| schema_error(&format!("{camel} must contain only text")))
                .and_then(cell)
        })
        .collect()
}

fn object_array<'a>(
    object: &'a Map<String, Value>,
    snake: &str,
    camel: &str,
) -> RuntimeResult<Vec<&'a Map<String, Value>>> {
    let Some(value) = one_alias(object, snake, camel)? else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .filter(|values| values.len() <= MAX_ITEMS)
        .ok_or_else(|| schema_error(&format!("{camel} must be a bounded array")))?;
    values
        .iter()
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| schema_error(&format!("{camel} must contain only objects")))
        })
        .collect()
}

fn object<'a>(
    object: &'a Map<String, Value>,
    snake: &str,
    camel: &str,
) -> RuntimeResult<Option<&'a Map<String, Value>>> {
    let Some(value) = one_alias(object, snake, camel)? else {
        return Ok(None);
    };
    value
        .as_object()
        .filter(|values| values.len() <= MAX_ITEMS)
        .map(Some)
        .ok_or_else(|| schema_error(&format!("{camel} must be a bounded object")))
}

fn one_alias<'a>(
    object: &'a Map<String, Value>,
    snake: &str,
    camel: &str,
) -> RuntimeResult<Option<&'a Value>> {
    if snake != camel && object.contains_key(snake) && object.contains_key(camel) {
        return Err(schema_error(&format!(
            "{snake} and {camel} cannot be combined"
        )));
    }
    Ok(object.get(snake).or_else(|| object.get(camel)))
}

fn row(object: &Map<String, Value>, fields: &[&str]) -> RuntimeResult<Vec<String>> {
    if let Some(field) = object
        .keys()
        .find(|field| !fields.contains(&field.as_str()))
    {
        return Err(schema_error(&format!(
            "unknown memory table field: {field}"
        )));
    }
    fields
        .iter()
        .map(|field| match object.get(*field) {
            None => Ok(String::new()),
            Some(Value::String(value)) => cell(value),
            Some(Value::Number(value)) => cell(&value.to_string()),
            _ => Err(schema_error(&format!(
                "memory table field {field} must be scalar"
            ))),
        })
        .collect()
}

fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut lines = vec![
        format!("| {} |", headers.join(" | ")),
        format!(
            "| {} |",
            std::iter::repeat_n("---", headers.len())
                .collect::<Vec<_>>()
                .join(" | ")
        ),
    ];
    lines.extend(rows.iter().map(|row| format!("| {} |", row.join(" | "))));
    lines.join("\n")
}

fn cell(value: &str) -> RuntimeResult<String> {
    let value = value.trim();
    if value.len() > MAX_CELL_BYTES || value.contains('\0') {
        return Err(schema_error(
            "structured memory value exceeds its cell bound",
        ));
    }
    Ok(value.replace('|', "\\|").replace(['\r', '\n'], " "))
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
