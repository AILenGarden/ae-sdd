use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum Verdict {
    PreserveImplemented,
    BreakingFixVerified,
    Pending,
}

pub(crate) fn assert_pair(id: &str, verdict: Verdict, reason: &str, python: &Value, rust: &Value) {
    let python_core = stable_projection(id, python);
    let rust_core = stable_projection(id, rust);
    assert_eq!(
        python_core, rust_core,
        "{id} ({verdict:?}) stable semantic projection diverged: {reason}"
    );
    match verdict {
        Verdict::PreserveImplemented => assert_eq!(
            gap_projection(id, python),
            gap_projection(id, rust),
            "{id} preserve projection diverged: {reason}"
        ),
        Verdict::Pending => assert_ne!(
            gap_projection(id, python),
            gap_projection(id, rust),
            "{id} no longer demonstrates its recorded pending gap: {reason}"
        ),
        Verdict::BreakingFixVerified => {}
    }
}

fn stable_projection(id: &str, value: &Value) -> Value {
    match id {
        "assets check" => json!({
            "pass":bool_or_outcome(value,"pass"),
            "projectKey":value["projectKey"],
            "exists":value["exists"],
            "missingSections":value["missingSections"],
        }),
        "assets outline" | "assets stats" => json!({
            "projectKey":value["projectKey"],
            "nSections":asset_stat(value,"n_sections","nSections"),
        }),
        "assets query" => json!({
            "projectKey":value["projectKey"],
            "query":value["query"],
            "top":first(value,&["top_n","topN"]),
            "hitCount":first(value,&["n_hits","nHits"]),
            "snippets":hit_strings(first(value,&["hits"]),"snippet"),
            "matched":hit_arrays(first(value,&["hits"]),"matched_tokens","matchedTokens"),
        }),
        "assets read" => json!({
            "projectKey":first(value,&["project_key","projectKey"]),
            "stage":value["stage"],
            "extraService":read_hit_snippets(value,"service"),
        }),
        "assets section" => json!({
            "projectKey":value["projectKey"],
            "section":value["section"],
            "content":value["content"],
        }),
        "automation status" => json!({
            "enabled":value["enabled"],
            "reviewerTier":value["reviewerTier"],
            "preflightInfoCollection":value["preflightInfoCollection"],
            "onConsensusStall":value["onConsensusStall"],
            "automatedReviewPoints":value["automatedReviewPoints"],
            "enabledAt":value["enabledAt"],
        }),
        "baseline diff" => json!({
            "status":value["status"],
            "baseline":value["baseline"],
            "current":value["current"],
            "new":value["new"],
            "resolved":value["resolved"],
            "touchedDebt":value["touchedDebt"],
        }),
        "baseline inspect" => json!({
            "exists":value["exists"],
            "integrity":value["integrity"],
            "gateId":value["gateId"],
            "findingCount":value["findingCount"],
            "baseline":value["baseline"],
        }),
        "classify" => json!({
            "source":normalize_source(first(value,&["source"])),
            "scale":normalize_scale(value),
            "aiFit":normalize_ai_fit(first(value,&["ai_fit","aiFit"])),
            "multiAgent":first(value,&["multi_agent","multiAgent"]),
            "sourceConfidence":value.pointer("/confidence/source").cloned().unwrap_or(Value::Null),
            "needsReview":first(value,&["needs_review","needsReview"]),
            "analysisRequired":first(value,&["analysis_required","analysisRequired"]),
            "recommendedDesign":first(value,&["recommended_design","recommendedDesign"]),
        }),
        "db audit" => json!({
            "exists":value["exists"],
            "profiles":profiles(value),
        }),
        "db profiles" => json!({"profiles":profiles(value)}),
        "db query" | "db explain" => json!({
            "ok":value["ok"],
            "blocked":value["blocked"],
            "profile":profile(&value["profile"]),
            "sqlClass":json!({
                "readonly":first(&first(value,&["sql_class","sqlClass"]),&["readonly"]),
                "hasWrite":first(&first(value,&["sql_class","sqlClass"]),&["has_write","hasWrite"]),
            }),
            "rowCount":first(value,&["row_count","rowCount"]),
            "limit":value["limit"],
            "rows":value["rows"],
        }),
        "evidence lookup" => json!({
            "reusable":value["reusable"],
            "entry":value["entry"],
        }),
        "git blame" => json!({"file":value["file"],"entries":value["entries"]}),
        "git diff" => json!({
            "base":value["base"],"head":value["head"],"stat":value["stat"],"diff":value["diff"],
        }),
        "git impact" => json!({
            "base":value["base"],"head":value["head"],"files":value["files"],
            "modules":value["modules"],"by_extension":value["by_extension"],
            "risk_hints":value["risk_hints"],
        }),
        "git log" => json!({
            "path":value["path"],"limit":value["limit"],"commits":value["commits"],
        }),
        "git status" => json!({
            "branch":value["branch"],"dirty":value["dirty"],"entries":value["entries"],
        }),
        "perf report" => json!({
            "last":value["last"],"summary":perf_summary(&value["summary"]),
        }),
        "perf doctor" => json!({
            "last":value["last"],"summary":perf_summary(&value["summary"]),
            "adviceCount":value["advice"].as_array().map_or(0,Vec::len),
        }),
        "plugin list" => json!({
            "projectPlugin":named_plugin(value,"fixture"),
            "projectConflictCount":value["totalConflicts"],
        }),
        "plugin trace" => json!({
            "target":value["target"],"plugin":plugin(&value["plugin"]),
        }),
        "plugin validate" => json!({
            "valid":value["valid"],"errors":value["errors"],
        }),
        _ => panic!("unclassified differential command {id}"),
    }
}

fn gap_projection(id: &str, value: &Value) -> Value {
    match id {
        "assets outline" | "assets stats" => json!({
            "nDocs":asset_stat(value,"n_docs","nDocs"),
            "nTokens":asset_stat(value,"n_tokens","nTokens"),
            "avgDocLen":asset_stat(value,"avg_doc_len","avgDocLen"),
            "sections":asset_stat(value,"sections","sections"),
        }),
        "assets query" => json!({
            "scores":hit_strings(first(value,&["hits"]),"score"),
            "sections":hit_strings(first(value,&["hits"]),"section"),
        }),
        "assets read" => json!({
            "baselineKeys":object_keys(first(value,&["baseline_hits","baselineHits"])),
            "stats":{
                "nDocs":asset_stat(&first(value,&["stats"]),"n_docs","nDocs"),
                "nTokens":asset_stat(&first(value,&["stats"]),"n_tokens","nTokens"),
                "nSections":asset_stat(&first(value,&["stats"]),"n_sections","nSections"),
                "avgDocLen":asset_stat(&first(value,&["stats"]),"avg_doc_len","avgDocLen"),
                "sections":asset_stat(&first(value,&["stats"]),"sections","sections"),
            },
        }),
        "classify" => json!({
            "nextAction":first(value,&["next_action"]),
            "specStrategy":first(value,&["spec_strategy"]),
            "routeReason":first(value,&["route_reason","routeReason"]),
        }),
        _ => Value::Null,
    }
}

fn bool_or_outcome(value: &Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| value.get("outcome").and_then(Value::as_str) == Some("PASS"))
}

fn first(value: &Value, names: &[&str]) -> Value {
    names
        .iter()
        .find_map(|name| value.get(*name).cloned())
        .unwrap_or(Value::Null)
}

fn asset_stat(value: &Value, python: &str, rust: &str) -> Value {
    let source = value.get("stats").unwrap_or(value);
    first(source, &[python, rust])
}

fn hit_strings(value: Value, field: &str) -> Value {
    Value::Array(
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|hit| hit.get(field).cloned())
            .collect(),
    )
}

fn hit_arrays(value: Value, python: &str, rust: &str) -> Value {
    Value::Array(
        value
            .as_array()
            .into_iter()
            .flatten()
            .map(|hit| first(hit, &[python, rust]))
            .collect(),
    )
}

fn read_hit_snippets(value: &Value, key: &str) -> Value {
    let hits = first(value, &["extra_hits", "extraHits"]);
    hit_strings(hits.get(key).cloned().unwrap_or(Value::Null), "snippet")
}

fn normalize_source(value: Value) -> Value {
    match value.as_str() {
        Some("对话" | "Conversation") => json!("Conversation"),
        Some(other) => json!(other),
        None => Value::Null,
    }
}

fn normalize_scale(value: &Value) -> Value {
    let raw = first(value, &["scaleDisplay", "scale"]);
    match raw.as_str() {
        Some("大" | "large") => json!("large"),
        Some("中" | "medium") => json!("medium"),
        Some("小" | "small") => json!("small"),
        Some("微" | "micro") => json!("micro"),
        Some(other) => json!(other),
        None => Value::Null,
    }
}

fn normalize_ai_fit(value: Value) -> Value {
    match value.as_str() {
        Some("低" | "low") => json!("low"),
        Some("中" | "medium") => json!("medium"),
        Some("高" | "high") => json!("high"),
        Some(other) => json!(other),
        None => Value::Null,
    }
}

fn profiles(value: &Value) -> Value {
    Value::Array(
        value["profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .map(profile)
            .collect(),
    )
}

fn profile(value: &Value) -> Value {
    json!({
        "name":value["name"],"driver":value["driver"],"host":value["host"],
        "port":value["port"],"schema":value["schema"],"secrets":value["secrets"],
    })
}

fn perf_summary(value: &Value) -> Value {
    json!({
        "count":value["count"],
        "duration":metric(&value["duration"]),
        "cpuMs":metric(&value["cpuMs"]),
        "ioWaitMs":metric(&value["ioWaitMs"]),
        "bootstrapMs":value["bootstrapMs"],
        "commands":value["commands"],
        "slowestCommands":value["slowestCommands"],
        "slowestSpans":value["slowestSpans"],
        "byScale":value["byScale"],
        "scaleRatios":value["scaleRatios"],
    })
}

fn metric(value: &Value) -> Value {
    json!({
        "avgMs":value["avgMs"],"p50Ms":value["p50Ms"],
        "p95Ms":value["p95Ms"],"maxMs":value["maxMs"],
    })
}

fn named_plugin(value: &Value, name: &str) -> Value {
    if let Some(found) = value
        .get("allPlugins")
        .and_then(Value::as_array)
        .and_then(|plugins| plugins.iter().find(|plugin| plugin["name"] == name))
    {
        return plugin(found);
    }
    value["layers"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|layer| layer["plugins"].as_array().into_iter().flatten())
        .find(|candidate| candidate["name"] == name)
        .map(plugin)
        .unwrap_or(Value::Null)
}

fn plugin(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    json!({
        "name":value["name"],"type":value["type"],"version":value["version"],
        "description":value["description"],"path":value["path"],
        "provides":value["provides"],"replaces":value["replaces"],
    })
}

fn object_keys(value: Value) -> Value {
    let mut keys = value
        .as_object()
        .map(Map::keys)
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    json!(keys)
}
