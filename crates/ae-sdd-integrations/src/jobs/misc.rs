use std::collections::BTreeMap;
use std::fs;
use std::str::FromStr;

use ae_sdd_runtime::RuntimeResult;
use ae_sdd_store::UtcTimestamp;
use serde_json::{Value, json};

use super::common::{
    JobContext, MAX_FILE_BYTES, digest, read_bounded, read_json, required_string, safe_segment,
    schema_error,
};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    match entrypoint {
        "automation.status" => automation_status(context),
        "classify" => classify(context, arguments),
        "evidence.lookup" => evidence_lookup(context, arguments),
        _ => unreachable!("miscellaneous entrypoint was classified by caller"),
    }
}

fn automation_status(context: &JobContext<'_>) -> RuntimeResult<Value> {
    let path = context.project_file(".ae-sdd/config.yaml")?;
    let bytes = read_bounded(&path, MAX_FILE_BYTES)?;
    let text =
        String::from_utf8(bytes.clone()).map_err(|_| schema_error("config.yaml must be UTF-8"))?;
    let fields = yaml_section(&text, "automation")?;
    let enabled = yaml_bool(&fields, "enabled", false)?;
    let reviewer_tier = yaml_u64(&fields, "reviewerTier", 3)?;
    let preflight = yaml_bool(&fields, "preflightInfoCollection", true)?;
    let on_stall = fields
        .get("onConsensusStall")
        .cloned()
        .unwrap_or_else(|| "pause".to_owned());
    if !matches!(on_stall.as_str(), "pause" | "fail") {
        return Err(schema_error("automation onConsensusStall is invalid"));
    }
    let review_points = fields
        .get("automatedReviewPoints")
        .map(|value| parse_inline_list(value))
        .transpose()?
        .unwrap_or_default();
    Ok(json!({
        "outcome":"PASS",
        "enabled":enabled,
        "reviewerTier":reviewer_tier,
        "preflightInfoCollection":preflight,
        "onConsensusStall":on_stall,
        "automatedReviewPoints":review_points,
        "enabledAt":fields.get("enabledAt").cloned().unwrap_or_default(),
        "configDigest":digest(&bytes),
    }))
}

fn classify(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    let (text, filename) = if let Some(path) = arguments
        .get("file")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path = context.existing_file(path)?;
        let bytes = read_bounded(&path, MAX_FILE_BYTES)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| schema_error("classification input file must be UTF-8"))?;
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned();
        (text, Some(filename))
    } else {
        (required_string(arguments, "text")?.to_owned(), None)
    };
    if text.len() > MAX_FILE_BYTES as usize {
        return Err(schema_error("classification text exceeds the 1 MiB bound"));
    }
    let folded = text.to_lowercase();
    let first_line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    let (source, source_confidence, source_reason) =
        classify_source(&folded, &first_line.to_lowercase(), filename.as_deref());
    let nonempty_lines = text.lines().filter(|line| !line.trim().is_empty()).count();
    let mut scale = classify_scale(&folded, nonempty_lines);
    let mut entry_node = match source {
        "PRD" => Some("PRD"),
        "Issue" | "Conversation" => Some("RA"),
        _ => None,
    };
    if contains_any(&folded, &["bug", "defect", "故障", "缺陷", "修复", "fix"]) {
        scale = "micro";
        entry_node = Some("BUG");
    } else if contains_any(&folded, &["config", "配置", "改个常量", "改个枚举"]) {
        scale = "micro";
        entry_node = Some("CONFIG");
    } else if contains_any(
        &folded,
        &["code review", "codereview", "代码审查", "评审代码"],
    ) && !contains_any(&folded, &["ae-sdd", "skill", "runtime", "门禁"])
    {
        scale = "micro";
        entry_node = Some("CODE_REVIEW");
    } else if contains_any(&folded, &["优化", "重构", "改进"])
        && contains_any(
            &folded,
            &["代码", "实现", "函数", "方法", ".rs", ".py", ".java"],
        )
        && !contains_any(&folded, &["ae-sdd", "skill", "runtime", "门禁"])
    {
        scale = "micro";
        entry_node = Some("OPTIMIZE");
    }
    let ai_fit = match scale {
        "micro" | "small" => "high",
        "medium" => "medium",
        _ => "low",
    };
    let multi_agent = matches!(scale, "medium" | "large");
    let needs_review = source_confidence < 0.4;
    let analysis_required = !matches!(entry_node, Some("CODE_REVIEW" | "DOC_FORMAT"));
    let direct_plan = contains_any(
        &folded,
        &[
            "codingplan",
            "coding plan",
            "直接 coding",
            "直接编码",
            "无需设计",
        ],
    );
    let recommended_design = if direct_plan {
        "CODING_PLAN"
    } else {
        match scale {
            "large" => "DR",
            "medium" => "STORY",
            "small" | "micro" => "CODING_PLAN",
            _ => unreachable!(),
        }
    };
    let route_reason = if !analysis_required {
        format!(
            "entry_node={} 为只读轻量链，跳过需求分析",
            entry_node.unwrap_or_default()
        )
    } else if direct_plan {
        "输入明确要求直接形成 CodingPlan".to_owned()
    } else {
        match scale {
            "large" => "大规模任务默认需要架构设计；需求分析后可调整".to_owned(),
            "medium" => "中规模任务默认使用 Story 行为契约；需求分析后可调整".to_owned(),
            _ => format!("规模={}，默认使用紧凑 CodingPlan", scale_display(scale)),
        }
    };
    let next_action = if analysis_required {
        "requirement-analysis"
    } else if entry_node == Some("CODE_REVIEW") {
        "code-review"
    } else {
        "doc-format"
    };
    let spec_strategy = if analysis_required {
        json!({
            "needs":true,
            "entry_spec":"RA",
            "series":"requirement-analysis",
            "auto_create":true,
            "recommended_design":recommended_design,
            "reason":format!(
                "先分析本次任务；当前设计建议={recommended_design}（{route_reason}）"
            ),
        })
    } else {
        json!({
            "needs":false,
            "reason":format!(
                "entry_node={} 为只读轻量链",
                entry_node.unwrap_or_default()
            ),
        })
    };
    Ok(json!({
        "outcome":"PASS",
        "source":source,
        "scale":scale,
        "scaleDisplay":scale_display(scale),
        "aiFit":ai_fit,
        "multiAgent":multi_agent,
        "confidence":{"source":source_confidence,"scale":0.5,"aiFit":0.5},
        "rationale":{"source":source_reason,"scale":"bounded native keyword and line analysis","aiFit":"derived from scale"},
        "needsReview":needs_review,
        "reviewReasons":if needs_review {vec!["source has no strong signal"]} else {Vec::<&str>::new()},
        "entryNode":entry_node,
        "next_action":next_action,
        "nextAction":next_action,
        "spec_strategy":spec_strategy,
        "specStrategy":spec_strategy,
        "analysisRequired":analysis_required,
        "recommendedDesign":recommended_design,
        "route_reason":route_reason,
        "routeReason":route_reason,
        "routeConfidence":source_confidence.max(0.5),
    }))
}

fn evidence_lookup(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    let story = safe_segment(required_string(arguments, "story")?, "story")?;
    let input = required_string(arguments, "inputFingerprint")?;
    let command = required_string(arguments, "command")?;
    let toolchain = required_string(arguments, "toolchainFingerprint")?;
    let Some((relative, manifest)) = load_evidence_manifest(context, &story)? else {
        return Ok(json!({
            "outcome":"PASS",
            "reusable":false,
            "entry":null,
            "manifestPath":format!(".auto-engineering/{story}/evidence/manifest.json"),
            "integrity":"missing",
        }));
    };
    if !manifest_hash_matches(&manifest)? {
        return Ok(json!({
            "outcome":"FAIL",
            "reusable":false,
            "entry":null,
            "manifestPath":relative,
            "integrity":"unverified-or-tampered",
        }));
    }
    let command_hash = digest(
        &serde_json::to_vec(&Value::String(command.to_owned()))
            .map_err(|_| schema_error("evidence command canonicalization failed"))?,
    );
    let entries = manifest
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("evidence manifest entries must be an array"))?;
    if entries.len() > 100_000 {
        return Err(schema_error("evidence manifest exceeds its entry bound"));
    }
    let mut reusable = None;
    for entry in entries.iter().rev() {
        if evidence_entry_matches(context, entry, input, &command_hash, toolchain)? {
            reusable = Some(entry.clone());
            break;
        }
    }
    Ok(json!({
        "outcome":"PASS",
        "reusable":reusable.is_some(),
        "entry":reusable,
        "manifestPath":relative,
        "integrity":"verified",
    }))
}

fn load_evidence_manifest(
    context: &JobContext<'_>,
    story: &str,
) -> RuntimeResult<Option<(String, Value)>> {
    let direct = format!(".auto-engineering/{story}/evidence/manifest.json");
    if let Ok(path) = context.project_file(&direct) {
        return Ok(Some((direct, read_json(&path, MAX_FILE_BYTES)?)));
    }
    let root = context.root.join(".auto-engineering");
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(super::common::io_error(error)),
    };
    let mut candidates = Vec::new();
    for entry in entries.take(4_097) {
        let entry = entry.map_err(super::common::io_error)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if (name == story || name.ends_with(&format!("-{story}")))
            && entry
                .path()
                .join("evidence")
                .join("manifest.json")
                .is_file()
        {
            candidates.push(entry.path().join("evidence").join("manifest.json"));
        }
    }
    if candidates.len() > 1 {
        return Err(ae_sdd_runtime::RuntimeError::new(
            ae_sdd_protocol::StableErrorCode::ScopeAmbiguous,
            "multiple evidence manifests match the requested Story",
        ));
    }
    let Some(path) = candidates.pop() else {
        return Ok(None);
    };
    let path = context.existing_file(&path.to_string_lossy())?;
    let relative = path
        .strip_prefix(&context.root)
        .map_err(|_| schema_error("evidence path escaped the workspace"))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(Some((relative, read_json(&path, MAX_FILE_BYTES)?)))
}

fn manifest_hash_matches(manifest: &Value) -> RuntimeResult<bool> {
    let expected = manifest
        .get("contentHash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if expected.is_empty() {
        return Ok(false);
    }
    let mut payload = manifest
        .as_object()
        .cloned()
        .ok_or_else(|| schema_error("evidence manifest must be an object"))?;
    payload.remove("contentHash");
    payload.retain(|key, _| !key.starts_with('_'));
    let bytes = serde_json::to_vec(&Value::Object(payload))
        .map_err(|_| schema_error("evidence manifest canonicalization failed"))?;
    Ok(digest_matches(expected, &digest(&bytes)))
}

fn evidence_entry_matches(
    context: &JobContext<'_>,
    entry: &Value,
    input: &str,
    command_hash: &str,
    toolchain: &str,
) -> RuntimeResult<bool> {
    let object = entry
        .as_object()
        .ok_or_else(|| schema_error("evidence entry must be an object"))?;
    if object.get("status").and_then(Value::as_str) == Some("superseded")
        || object.get("reusable").and_then(Value::as_bool) != Some(true)
        || object.get("exitCode").and_then(Value::as_i64) != Some(0)
        || object.get("inputFingerprint").and_then(Value::as_str) != Some(input)
        || !object
            .get("commandHash")
            .and_then(Value::as_str)
            .is_some_and(|expected| digest_matches(expected, command_hash))
        || object.get("toolchainFingerprint").and_then(Value::as_str) != Some(toolchain)
    {
        return Ok(false);
    }
    if !fresh(entry)? {
        return Ok(false);
    }
    let artifacts = object
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("evidence artifacts must be an array"))?;
    if artifacts.len() > 1_024 {
        return Err(schema_error("evidence artifact count exceeds its bound"));
    }
    for artifact in artifacts {
        let artifact = artifact
            .as_object()
            .ok_or_else(|| schema_error("evidence artifact must be an object"))?;
        let path = artifact
            .get("snapshotPath")
            .or_else(|| artifact.get("path"))
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("evidence artifact path is required"))?;
        let expected = artifact
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("evidence artifact sha256 is required"))?;
        let bytes = read_bounded(&context.existing_file(path)?, MAX_FILE_BYTES)?;
        if !digest_matches(expected, &digest(&bytes)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn digest_matches(expected: &str, actual_hex: &str) -> bool {
    expected == actual_hex || expected.strip_prefix("sha256:") == Some(actual_hex)
}

fn fresh(entry: &Value) -> RuntimeResult<bool> {
    let Some(window) = entry.get("freshnessWindowSeconds").and_then(Value::as_i64) else {
        return Ok(true);
    };
    if window <= 0 {
        return Ok(false);
    }
    let started = entry
        .get("startedAt")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("fresh evidence requires startedAt"))?;
    let started = UtcTimestamp::from_str(started)
        .map_err(|_| schema_error("evidence startedAt is invalid"))?;
    let expires = started
        .as_timestamp()
        .checked_add(jiff::SignedDuration::from_secs(window))
        .map_err(|_| schema_error("evidence freshness window overflowed"))?;
    Ok(UtcTimestamp::now().as_timestamp() <= &expires)
}

fn classify_source(
    folded: &str,
    first: &str,
    filename: Option<&str>,
) -> (&'static str, f64, &'static str) {
    if first.starts_with("# prd") {
        return ("PRD", 1.0, "title signal");
    }
    if first.starts_with("# dr") || first.starts_with("# story") || first.starts_with("# task") {
        return ("DR", 1.0, "title signal");
    }
    if first.starts_with("# issue") || first.starts_with("# bug") {
        return ("Issue", 1.0, "title signal");
    }
    if let Some(filename) = filename.map(str::to_lowercase) {
        if filename.contains("prd") {
            return ("PRD", 0.9, "filename signal");
        }
        if ["dr-", "story-", "task-"]
            .iter()
            .any(|value| filename.contains(value))
        {
            return ("DR", 0.9, "filename signal");
        }
        if filename.contains("bug") || filename.contains("issue") {
            return ("Issue", 0.9, "filename signal");
        }
    }
    if contains_any(folded, &["product requirement", "产品需求", "prd"]) {
        ("PRD", 0.5, "keyword signal")
    } else if contains_any(folded, &["issue", "bug", "defect", "缺陷", "问题", "工单"]) {
        ("Issue", 0.5, "keyword signal")
    } else if contains_any(
        folded,
        &["design requirement", "设计需求", "story", "task", "dr-"],
    ) {
        ("DR", 0.5, "keyword signal")
    } else {
        (
            "Conversation",
            0.2,
            "fallback without a strong source signal",
        )
    }
}

fn classify_scale(folded: &str, lines: usize) -> &'static str {
    if contains_any(
        folded,
        &["large", "massive", "cross-module", "跨模块", "架构", "全量"],
    ) {
        "large"
    } else if contains_any(
        folded,
        &["medium", "中型", "多个模块", "10 files", "10 个文件"],
    ) {
        "medium"
    } else if contains_any(
        folded,
        &["small", "小任务", "小改", "几个文件", "1-3 files"],
    ) {
        "small"
    } else if contains_any(
        folded,
        &["micro", "微任务", "微改", "单文件", "typo", "trivial"],
    ) || lines < 10
    {
        "micro"
    } else if lines < 50 {
        "small"
    } else if lines < 200 {
        "medium"
    } else {
        "large"
    }
}

fn scale_display(scale: &str) -> &str {
    match scale {
        "large" => "大",
        "medium" => "中",
        "small" => "小",
        "micro" => "微",
        _ => "未知",
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn yaml_section(text: &str, section: &str) -> RuntimeResult<BTreeMap<String, String>> {
    let marker = format!("{section}:");
    let mut inside = false;
    let mut values = BTreeMap::new();
    for line in text.lines() {
        if !inside {
            inside = line.trim_end() == marker && !line.starts_with(char::is_whitespace);
            continue;
        }
        if !line.trim().is_empty() && !line.starts_with(char::is_whitespace) {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (key, raw) = trimmed
            .split_once(':')
            .ok_or_else(|| schema_error("automation YAML contains an unsupported line"))?;
        let value = raw
            .split_once(" #")
            .map_or(raw, |(value, _)| value)
            .trim()
            .trim_matches(['\'', '"'])
            .to_owned();
        values.insert(key.trim().to_owned(), value);
    }
    Ok(values)
}

fn yaml_bool(values: &BTreeMap<String, String>, key: &str, default: bool) -> RuntimeResult<bool> {
    match values.get(key).map(String::as_str) {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(schema_error("automation boolean is invalid")),
    }
}

fn yaml_u64(values: &BTreeMap<String, String>, key: &str, default: u64) -> RuntimeResult<u64> {
    values.get(key).map_or(Ok(default), |value| {
        value
            .parse::<u64>()
            .ok()
            .filter(|value| *value <= 10)
            .ok_or_else(|| schema_error("automation integer is invalid"))
    })
}

fn parse_inline_list(value: &str) -> RuntimeResult<Vec<Value>> {
    let trimmed = value.trim().trim_start_matches('[').trim_end_matches(']');
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    trimmed
        .split(',')
        .map(str::trim)
        .map(|item| {
            item.parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map(|number| json!(number))
                .ok_or_else(|| schema_error("automation review point list is invalid"))
        })
        .collect()
}
