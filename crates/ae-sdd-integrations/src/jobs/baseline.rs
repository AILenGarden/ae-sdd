use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Map, Value, json};

use super::common::{
    JobContext, MAX_FILE_BYTES, digest, read_json, safe_segment, schema_error,
};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let gate = safe_segment(
        arguments
            .get("gate")
            .and_then(Value::as_str)
            .unwrap_or("G-CODE-1"),
        "gate",
    )?;
    let relative = format!(".ae-sdd/baselines/{gate}.json");
    let loaded = load_baseline(context, &relative)?;
    match entrypoint {
        "baseline.inspect" => Ok(inspect(&gate, &relative, loaded)),
        "baseline.diff" => compare(context, &gate, &relative, loaded, arguments),
        _ => unreachable!("baseline entrypoint was classified by caller"),
    }
}

struct LoadedBaseline {
    payload: Option<Value>,
    integrity: &'static str,
}

fn load_baseline(context: &JobContext<'_>, relative: &str) -> RuntimeResult<LoadedBaseline> {
    let path = match context.project_file(relative) {
        Ok(path) => path,
        Err(error) if error.code() == ae_sdd_protocol::StableErrorCode::ExternalStateConflict => {
            return Ok(LoadedBaseline {
                payload: None,
                integrity: "missing",
            });
        }
        Err(error) => return Err(error),
    };
    let payload = match read_json(&path, MAX_FILE_BYTES) {
        Ok(payload) => payload,
        Err(_) => {
            return Ok(LoadedBaseline {
                payload: None,
                integrity: "invalid-json",
            });
        }
    };
    let valid = content_hash_matches(&payload)?;
    Ok(LoadedBaseline {
        payload: Some(payload),
        integrity: if valid { "valid" } else { "tampered" },
    })
}

fn inspect(gate: &str, relative: &str, loaded: LoadedBaseline) -> Value {
    let finding_count = loaded
        .payload
        .as_ref()
        .and_then(|payload| payload.get("findings"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    json!({
        "outcome":if loaded.integrity == "valid" {"PASS"} else {"FAIL"},
        "path":relative,
        "exists":loaded.payload.is_some(),
        "integrity":loaded.integrity,
        "gateId":gate,
        "findingCount":finding_count,
        "baseline":loaded.payload,
    })
}

fn compare(
    context: &JobContext<'_>,
    gate: &str,
    relative: &str,
    loaded: LoadedBaseline,
    arguments: &Value,
) -> RuntimeResult<Value> {
    if loaded.integrity != "valid" {
        return Ok(json!({
            "outcome":"FAIL",
            "status":"BLOCK_BASELINE_REQUIRED",
            "reason":loaded.integrity,
            "gateId":gate,
            "path":relative,
            "baseline":0,
            "current":0,
            "new":[],
            "resolved":[],
            "touchedDebt":[],
        }));
    }
    let baseline = loaded
        .payload
        .ok_or_else(|| schema_error("validated baseline disappeared"))?;
    let ruleset = arguments
        .get("rulesetFingerprint")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !ruleset.is_empty()
        && baseline.get("rulesetFingerprint").and_then(Value::as_str) != Some(ruleset)
    {
        return Ok(json!({
            "outcome":"FAIL",
            "status":"BLOCK_BASELINE_INVALID",
            "reason":"ruleset fingerprint mismatch",
            "gateId":gate,
            "path":relative,
            "baseline":finding_array(&baseline)?.len(),
            "current":0,
            "new":[],
            "resolved":[],
            "touchedDebt":[],
        }));
    }
    let report = report(context, arguments)?;
    let current_values = report
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("scan report findings must be an array"))?;
    if current_values.len() > 100_000 {
        return Err(schema_error("scan report exceeds the finding bound"));
    }
    let baseline_values = finding_array(&baseline)?;
    let baseline_by_key = normalized_map(baseline_values)?;
    let current_by_key = normalized_map(current_values)?;
    let touched = touched_paths(arguments)?;
    let new = current_by_key
        .iter()
        .filter(|(key, finding)| {
            !baseline_by_key.contains_key(*key)
                && finding
                    .get("severity")
                    .and_then(Value::as_str)
                    .is_some_and(|severity| severity.eq_ignore_ascii_case("BLOCKER"))
        })
        .map(|(_, finding)| finding.clone())
        .collect::<Vec<_>>();
    let resolved = baseline_by_key
        .iter()
        .filter(|(key, _)| !current_by_key.contains_key(*key))
        .map(|(_, finding)| finding.clone())
        .collect::<Vec<_>>();
    let touched_debt = baseline_by_key
        .values()
        .filter(|finding| {
            finding
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| touched.contains(&path.replace('\\', "/")))
        })
        .cloned()
        .collect::<Vec<_>>();
    let status = if !new.is_empty() {
        "BLOCK_NEW_FINDINGS"
    } else if !touched_debt.is_empty() {
        "BLOCK_TOUCHED_DEBT"
    } else {
        "PASS_WITH_BASELINE_DEBT"
    };
    Ok(json!({
        "outcome":if status == "PASS_WITH_BASELINE_DEBT" {"PASS"} else {"FAIL"},
        "status":status,
        "gateId":gate,
        "path":relative,
        "baseline":baseline_by_key.len(),
        "current":current_by_key.len(),
        "new":new,
        "resolved":resolved,
        "touchedDebt":touched_debt,
    }))
}

fn report(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    if let Some(report) = arguments.get("report") {
        if !report.is_object() {
            return Err(schema_error("report must be an object"));
        }
        return Ok(report.clone());
    }
    let path = arguments
        .get("reportFile")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("report or reportFile is required"))?;
    read_json(&context.existing_file(path)?, MAX_FILE_BYTES)
}

fn finding_array(payload: &Value) -> RuntimeResult<&Vec<Value>> {
    payload
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("baseline findings must be an array"))
}

fn normalized_map(values: &[Value]) -> RuntimeResult<BTreeMap<String, Value>> {
    values
        .iter()
        .map(normalize_finding)
        .map(|result| result.map(|finding| (finding["findingKey"].as_str().unwrap().to_owned(), finding)))
        .collect()
}

fn normalize_finding(value: &Value) -> RuntimeResult<Value> {
    let mut item = value
        .as_object()
        .cloned()
        .ok_or_else(|| schema_error("finding must be an object"))?;
    let rule = string_field(&item, &["ruleId", "rule"], "UNKNOWN");
    let path = string_field(&item, &["path"], "").replace('\\', "/");
    let symbol = item
        .get("symbol")
        .or_else(|| item.get("line"))
        .cloned()
        .unwrap_or(Value::Null);
    let severity = string_field(&item, &["severity", "category"], "UNKNOWN");
    let key = item
        .get("findingKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let raw = format!("{rule}\n{path}\n{}\n{severity}", scalar(&symbol));
            digest(raw.as_bytes())
        });
    item.insert("findingKey".to_owned(), Value::String(key));
    item.insert("ruleId".to_owned(), Value::String(rule));
    item.insert("path".to_owned(), Value::String(path));
    item.insert("symbol".to_owned(), symbol);
    item.insert("severity".to_owned(), Value::String(severity));
    Ok(Value::Object(item))
}

fn content_hash_matches(payload: &Value) -> RuntimeResult<bool> {
    let expected = payload
        .get("contentHash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if expected.is_empty() {
        return Ok(false);
    }
    let mut unhashed = payload
        .as_object()
        .cloned()
        .ok_or_else(|| schema_error("baseline must be a JSON object"))?;
    unhashed.remove("contentHash");
    let canonical = serde_json::to_vec(&Value::Object(unhashed))
        .map_err(|_| schema_error("baseline canonicalization failed"))?;
    Ok(expected == digest(&canonical))
}

fn touched_paths(arguments: &Value) -> RuntimeResult<BTreeSet<String>> {
    let Some(value) = arguments.get("touched") else {
        return Ok(BTreeSet::new());
    };
    let values = match value {
        Value::String(value) => value.split(',').map(str::trim).map(str::to_owned).collect(),
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .map(str::to_owned)
                    .ok_or_else(|| schema_error("touched must contain strings"))
            })
            .collect::<RuntimeResult<Vec<_>>>()?,
        _ => return Err(schema_error("touched must be a string or string array")),
    };
    if values.len() > 10_000 {
        return Err(schema_error("touched path list exceeds its bound"));
    }
    Ok(values
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
        .collect())
}

fn string_field(object: &Map<String, Value>, names: &[&str], default: &str) -> String {
    names
        .iter()
        .find_map(|name| object.get(*name))
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn scalar(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}
