use std::collections::BTreeSet;

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Value, json};

use super::super::common::{JobContext, schema_error};
use super::{assert_registered_project, optional_known_text};

pub(super) fn execute(
    context: &JobContext<'_>,
    work_item_id: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    assert_registered_project(context, arguments)?;
    let mut findings = Vec::new();
    check_obsolete_contracts(context, &mut findings)?;
    check_gate_registry(context, &mut findings)?;
    check_route_inventory(context, &mut findings)?;
    check_hook_implementation(context, &mut findings)?;
    let n_warn = findings
        .iter()
        .filter(|finding| finding["severity"] == "warn")
        .count();
    let n_info = findings.len().saturating_sub(n_warn);
    Ok(json!({
        "outcome":"PASS",
        "workItemId":work_item_id,
        "checks_run":["IC-1","IC-2","IC-3","IC-4"],
        "n_findings":findings.len(),
        "n_warn":n_warn,
        "n_info":n_info,
        "findings":findings,
        "reportOnly":true,
    }))
}

fn check_obsolete_contracts(
    context: &JobContext<'_>,
    findings: &mut Vec<Value>,
) -> RuntimeResult<()> {
    let Some(skill) = optional_known_text(context, "source/SKILL.md")? else {
        findings.push(finding(
            "IC-1",
            "warn",
            "source/SKILL.md is missing",
            "source/SKILL.md",
        ));
        return Ok(());
    };
    let ghosts = [
        "ae-sdd assets update",
        "ae-sdd assets audit",
        "ae-sdd sync-tools",
        "ae-sdd state validate",
        "ae-sdd state show",
        "ae-sdd state diff",
    ];
    let obsolete = [
        "rules.yaml",
        "sync_tools",
        "tools/lib/*.mjs",
        "tools/schemas/*.json",
        "tools/tests/*.test.mjs",
        "Node.js ESM",
    ];
    for (line_index, line) in skill.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        let historical = ["changelog", "history", "deprecated", "removed"]
            .iter()
            .any(|marker| lower.contains(marker));
        for marker in ghosts {
            if line.contains(marker) {
                findings.push(finding(
                    "IC-1",
                    "warn",
                    &format!("ghost command reference: {marker}"),
                    &format!("source/SKILL.md:{}", line_index + 1),
                ));
            }
        }
        if !historical {
            for marker in obsolete {
                if line.contains(marker) {
                    findings.push(finding(
                        "IC-1",
                        "warn",
                        &format!("obsolete implementation reference: {marker}"),
                        &format!("source/SKILL.md:{}", line_index + 1),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn check_gate_registry(context: &JobContext<'_>, findings: &mut Vec<Value>) -> RuntimeResult<()> {
    let path = "crates/ae-sdd-gates/src/registry.rs";
    let count = optional_known_text(context, path)?
        .map(|source| source.matches("id: \"G-").count())
        .unwrap_or_default();
    let severity = if count == 36 { "info" } else { "warn" };
    findings.push(finding(
        "IC-2",
        severity,
        &format!("native Gate registry exposes {count} physical entries (expected 36)"),
        path,
    ));
    Ok(())
}

fn check_route_inventory(context: &JobContext<'_>, findings: &mut Vec<Value>) -> RuntimeResult<()> {
    let path = "tests/fixtures/compatibility/cli-routing.v1.json";
    let Some(source) = optional_known_text(context, path)? else {
        findings.push(finding("IC-3", "warn", "route inventory is missing", path));
        return Ok(());
    };
    let value: Value = serde_json::from_str(&source)
        .map_err(|_| schema_error("compatibility route inventory JSON is invalid"))?;
    let commands = value
        .get("commands")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("compatibility route inventory has no commands"))?;
    let ids = commands
        .iter()
        .filter_map(|command| command.get("id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let complete = commands.len() == 113 && ids.len() == commands.len();
    findings.push(finding(
        "IC-3",
        if complete { "info" } else { "warn" },
        &format!(
            "native legacy route inventory has {} commands and {} unique ids",
            commands.len(),
            ids.len()
        ),
        path,
    ));
    Ok(())
}

fn check_hook_implementation(
    context: &JobContext<'_>,
    findings: &mut Vec<Value>,
) -> RuntimeResult<()> {
    let dispatch =
        optional_known_text(context, "crates/ae-sdd-runtime/src/service.rs")?.unwrap_or_default();
    let implementation =
        optional_known_text(context, "crates/ae-sdd-runtime/src/service_hook_context.rs")?
            .unwrap_or_default();
    let complete = [
        "RpcMethod::HookUserPrompt",
        "RpcMethod::HookPreTool",
        "RpcMethod::HookPostTool",
        "RpcMethod::HookStop",
    ]
    .iter()
    .all(|method| dispatch.contains(method))
        && implementation.contains("pub(super) fn hook(")
        && implementation.contains("commit_receipt_event");
    findings.push(finding(
        "IC-4",
        if complete { "info" } else { "warn" },
        if complete {
            "four Hook methods share a physical fail-closed runtime implementation"
        } else {
            "Hook method dispatch or durable implementation is incomplete"
        },
        "crates/ae-sdd-runtime/src/service_hook_context.rs",
    ));
    Ok(())
}

fn finding(check_id: &str, severity: &str, item: &str, location: &str) -> Value {
    json!({
        "check_id":check_id,
        "severity":severity,
        "item":item,
        "location":location,
        "detail":item,
    })
}
