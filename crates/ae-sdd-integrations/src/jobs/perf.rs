use std::collections::BTreeMap;
use std::fs;

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Value, json};

use super::common::{JobContext, MAX_ASSET_BYTES, bounded_u64, read_bounded, schema_error};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let last = bounded_u64(arguments, "last", 50, 10_000)? as usize;
    let limit = bounded_u64(arguments, "limit", 10, 100)? as usize;
    let events = read_events(context, last)?;
    let summary = summarize(&events, limit);
    let mut result = json!({
        "outcome":"PASS",
        "statsDir":".ae-sdd/runtime-stats",
        "last":last,
        "summary":summary,
    });
    if entrypoint == "perf.doctor" {
        result["advice"] = Value::Array(advice(&summary));
    }
    Ok(result)
}

fn read_events(context: &JobContext<'_>, limit: usize) -> RuntimeResult<Vec<Value>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let proposed = context.root.join(".ae-sdd").join("runtime-stats");
    if !proposed.exists() {
        return Ok(Vec::new());
    }
    let directory = context.existing_directory(".ae-sdd/runtime-stats")?;
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(super::common::io_error(error)),
    };
    let mut files = entries
        .take(4_097)
        .map(|entry| entry.map_err(super::common::io_error))
        .collect::<RuntimeResult<Vec<_>>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    if files.len() > 4_096 {
        return Err(schema_error("runtime stats file count exceeds its bound"));
    }
    files.sort();
    let mut reverse_events = Vec::with_capacity(limit);
    for file in files.iter().rev().take(32) {
        let file = context.existing_file(&file.to_string_lossy())?;
        let bytes = read_bounded(&file, MAX_ASSET_BYTES)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| schema_error("runtime stats file must be UTF-8 JSONL"))?;
        for line in text.lines().rev() {
            if line.trim().is_empty() {
                continue;
            }
            if line.len() > 65_536 {
                return Err(schema_error("runtime stats event exceeds its line bound"));
            }
            let event: Value = serde_json::from_str(line)
                .map_err(|_| schema_error("runtime stats JSONL contains an invalid event"))?;
            if !event.is_object() {
                return Err(schema_error("runtime stats event must be an object"));
            }
            reverse_events.push(event);
            if reverse_events.len() == limit {
                break;
            }
        }
        if reverse_events.len() == limit {
            break;
        }
    }
    reverse_events.reverse();
    Ok(reverse_events)
}

fn summarize(events: &[Value], slow_limit: usize) -> Value {
    let durations = metric_values(events, "durationMs");
    let cpus = metric_values(events, "cpuMs");
    let io_waits = events
        .iter()
        .map(|event| {
            let duration = number(event, "durationMs");
            let cpu = number(event, "cpuMs");
            (duration - cpu).max(0.0)
        })
        .collect::<Vec<_>>();
    let bootstraps = events
        .iter()
        .filter_map(|event| event.get("bootstrapMs").and_then(Value::as_f64))
        .collect::<Vec<_>>();
    let mut commands = BTreeMap::<String, (Vec<f64>, Vec<f64>, String)>::new();
    let mut scales = BTreeMap::<String, (Vec<f64>, Vec<f64>)>::new();
    let mut spans = Vec::new();
    for event in events {
        let command = event
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let duration = number(event, "durationMs");
        let cpu = number(event, "cpuMs");
        let started_at = event
            .get("startedAt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let metrics = commands
            .entry(command.clone())
            .or_insert_with(|| (Vec::new(), Vec::new(), started_at));
        metrics.0.push(duration);
        metrics.1.push(cpu);
        let scale = event
            .get("scale")
            .and_then(Value::as_str)
            .or_else(|| event.pointer("/attrs/scale").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_owned();
        let scale_metrics = scales.entry(scale).or_default();
        scale_metrics.0.push(duration);
        scale_metrics.1.push(cpu);
        if let Some(values) = event.get("spans").and_then(Value::as_array) {
            for span in values.iter().take(10_000) {
                spans.push(json!({
                    "name":span.get("name").cloned().unwrap_or(Value::Null),
                    "durationMs":number(span,"durationMs"),
                    "cpuMs":number(span,"cpuMs"),
                    "attrs":span.get("attrs").cloned().unwrap_or_else(|| json!({})),
                    "command":command,
                    "startedAt":event.get("startedAt").cloned().unwrap_or_else(|| Value::String(String::new())),
                }));
            }
        }
    }
    let mut command_values = commands
        .into_iter()
        .map(|(command, (durations, cpus, started_at))| {
            let total = durations.iter().sum::<f64>();
            let total_cpu = cpus.iter().sum::<f64>();
            json!({
                "command":command,
                "count":durations.len(),
                "avgMs":round3(average(&durations)),
                "maxMs":round3(maximum(&durations)),
                "totalMs":round3(total),
                "avgCpuMs":round3(average(&cpus)),
                "maxCpuMs":round3(maximum(&cpus)),
                "totalCpuMs":round3(total_cpu),
                "lastStartedAt":started_at,
            })
        })
        .collect::<Vec<_>>();
    command_values
        .sort_by(|left, right| number(right, "totalMs").total_cmp(&number(left, "totalMs")));
    spans.sort_by(|left, right| number(right, "durationMs").total_cmp(&number(left, "durationMs")));
    spans.truncate(slow_limit);
    let mut slowest_commands = events.iter().collect::<Vec<_>>();
    slowest_commands
        .sort_by(|left, right| number(right, "durationMs").total_cmp(&number(left, "durationMs")));
    let slowest_commands = slowest_commands
        .into_iter()
        .take(slow_limit)
        .map(|event| {
            json!({
                "command":event.get("command").cloned().unwrap_or(Value::Null),
                "durationMs":event.get("durationMs").cloned().unwrap_or(Value::Null),
                "cpuMs":event.get("cpuMs").cloned().unwrap_or(Value::Null),
                "bootstrapMs":event.get("bootstrapMs").cloned().unwrap_or(Value::Null),
                "exitCode":event.get("exitCode").cloned().unwrap_or(Value::Null),
                "startedAt":event.get("startedAt").cloned().unwrap_or(Value::Null),
                "scale":event.get("scale").cloned().or_else(|| event.pointer("/attrs/scale").cloned()).unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let mut by_scale = scales
        .into_iter()
        .map(|(scale, (durations, cpus))| {
            let total = durations.iter().sum::<f64>();
            let total_cpu = cpus.iter().sum::<f64>();
            let avg = average(&durations);
            let avg_cpu = average(&cpus);
            json!({
                "scale":scale,
                "count":durations.len(),
                "totalMs":round3(total),
                "avgMs":round3(avg),
                "maxMs":round3(maximum(&durations)),
                "totalCpuMs":round3(total_cpu),
                "avgCpuMs":round3(avg_cpu),
                "avgIoWaitMs":round3((avg-avg_cpu).max(0.0)),
            })
        })
        .collect::<Vec<_>>();
    by_scale.sort_by(|left, right| number(right, "avgMs").total_cmp(&number(left, "avgMs")));
    let big_average = by_scale
        .iter()
        .find(|bucket| bucket.get("scale").and_then(Value::as_str) == Some("大"))
        .map(|bucket| number(bucket, "avgMs"))
        .filter(|value| *value > 0.0);
    let mut scale_ratios = serde_json::Map::new();
    if let Some(big_average) = big_average {
        for scale in ["微", "小", "中"] {
            if let Some(value) = by_scale
                .iter()
                .find(|bucket| bucket.get("scale").and_then(Value::as_str) == Some(scale))
                .map(|bucket| number(bucket, "avgMs"))
                .filter(|value| *value > 0.0)
            {
                scale_ratios.insert(format!("{scale}/大"), json!(round3(value / big_average)));
            }
        }
    }
    json!({
        "count":events.len(),
        "duration":metric_summary(&durations),
        "cpuMs":metric_summary(&cpus),
        "ioWaitMs":metric_summary(&io_waits),
        "bootstrapMs":metric_summary(&bootstraps),
        "commands":command_values,
        "slowestCommands":slowest_commands,
        "slowestSpans":spans,
        "byScale":by_scale,
        "scaleRatios":scale_ratios,
    })
}

fn advice(summary: &Value) -> Vec<Value> {
    if summary.get("count").and_then(Value::as_u64) == Some(0) {
        return vec![Value::String(
            "No runtime statistics are available for this workspace.".to_owned(),
        )];
    }
    let mut values = Vec::new();
    if let Some(span) = summary
        .get("slowestSpans")
        .and_then(Value::as_array)
        .and_then(|spans| spans.first())
    {
        values.push(Value::String(format!(
            "Slowest span is {} at {:.1} ms; inspect its bounded scan or I/O scope.",
            span.get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            number(span, "durationMs")
        )));
    }
    let duration = summary
        .pointer("/duration/avgMs")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let io_wait = summary
        .pointer("/ioWaitMs/avgMs")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if duration > 0.0 && io_wait > 500.0 && io_wait / duration > 0.7 {
        values.push(Value::String(
            "I/O wait exceeds 70% of average duration; prioritize in-process kernels and narrower scan roots."
                .to_owned(),
        ));
    }
    let bootstrap = summary
        .pointer("/bootstrapMs/p95Ms")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if bootstrap > 150.0 {
        values.push(Value::String(
            "Bootstrap p95 exceeds 150 ms; keep latency-sensitive Hook paths inside the daemon process."
                .to_owned(),
        ));
    }
    if values.is_empty() {
        values.push(Value::String(
            "No obvious bottleneck was detected in the selected sample.".to_owned(),
        ));
    }
    values
}

fn metric_values(events: &[Value], field: &str) -> Vec<f64> {
    events.iter().map(|event| number(event, field)).collect()
}

fn metric_summary(values: &[f64]) -> Value {
    let mut sorted = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    sorted.sort_by(f64::total_cmp);
    json!({
        "count":sorted.len(),
        "avgMs":round3(average(&sorted)),
        "p50Ms":percentile(&sorted,0.50),
        "p95Ms":percentile(&sorted,0.95),
        "maxMs":round3(maximum(&sorted)),
    })
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * quantile).round() as usize;
    round3(sorted[index])
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn maximum(values: &[f64]) -> f64 {
    values.iter().copied().reduce(f64::max).unwrap_or(0.0)
}

fn number(value: &Value, field: &str) -> f64 {
    value.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

fn round3(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}
