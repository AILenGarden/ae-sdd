use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ae_sdd_runtime::RuntimeResult;
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::common::{JobContext, MAX_FILE_BYTES, read_bounded, schema_error};
use super::{optional_string, required_known_text};

const MAX_GRAPH_RULES: usize = 256;
const MAX_CHANGED_FILES: usize = 1_024;

pub(super) fn execute(
    context: &JobContext<'_>,
    work_item_id: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let graph = load_update_graph(context)?;
    let checks = validate_update_graph(&graph)?;
    let affected = optional_string(arguments, "affected")?;
    let only = optional_string(arguments, "only")?;
    if affected.is_some() && only.is_some() {
        return Err(schema_error("affected and only cannot be combined"));
    }
    if let Some(affected) = affected {
        return query_affected(work_item_id, &graph, affected);
    }
    let selected = only.as_ref().map_or_else(
        || checks.keys().cloned().collect::<Vec<_>>(),
        |only| vec![only.clone()],
    );
    let mut results = Vec::with_capacity(selected.len());
    for check_id in selected {
        if !checks.contains_key(&check_id) {
            results.push(json!({
                "check_id":check_id,
                "name":"unknown check",
                "severity":"error",
                "pass":false,
                "message":"unknown check id",
                "fix":Value::Null,
            }));
            continue;
        }
        let (passed, message) = if check_id == "UC-01" {
            check_versions(context)?
        } else {
            let rule_count = checks.get(&check_id).map_or(0, Vec::len);
            (
                false,
                format!(
                    "native graph structure covers {rule_count} dependency rule(s), but this semantic checker is not yet registered"
                ),
            )
        };
        results.push(json!({
            "check_id":check_id,
            "name":"native update dependency check",
            "severity":"error",
            "pass":passed,
            "message":message,
            "fix":Value::Null,
        }));
    }
    let passed = results
        .iter()
        .filter(|result| result["pass"] == true)
        .count();
    let failed = results.len().saturating_sub(passed);
    Ok(json!({
        "outcome":if failed == 0 {"PASS"} else {"FAIL"},
        "workItemId":work_item_id,
        "total":results.len(),
        "passed":passed,
        "failed":failed,
        "warnings":0,
        "all_pass":failed == 0,
        "checks":results,
    }))
}

fn query_affected(
    work_item_id: &str,
    graph: &UpdateGraph,
    affected: String,
) -> RuntimeResult<Value> {
    let changed_files = affected
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
        .collect::<Vec<_>>();
    if changed_files.is_empty() || changed_files.len() > MAX_CHANGED_FILES {
        return Err(schema_error("affected must contain 1..1024 changed paths"));
    }
    if changed_files
        .iter()
        .any(|path| path.len() > 1_024 || Path::new(path).is_absolute())
        || changed_files.iter().map(String::len).sum::<usize>() > 256 * 1_024
    {
        return Err(schema_error(
            "affected paths must be bounded repository-relative paths",
        ));
    }
    let mut matched_rules = Vec::new();
    let mut affected_seen = BTreeSet::new();
    let mut affected_items = Vec::new();
    let mut checks_seen = BTreeSet::new();
    let mut checks_to_run = Vec::new();
    for rule in &graph.rules {
        if !changed_files.iter().any(|changed| {
            rule.trigger
                .iter()
                .any(|pattern| glob_matches(pattern, changed))
        }) {
            continue;
        }
        matched_rules.push(json!({
            "id":rule.id,
            "name":rule.name,
            "trigger_condition":rule.trigger_condition,
        }));
        for item in &rule.affected {
            if item.path.replace('\\', "/").starts_with("source/CHANGELOG") {
                continue;
            }
            if affected_seen.insert((item.path.clone(), item.action.clone())) {
                affected_items.push(json!({
                    "path":item.path,
                    "action":item.action,
                    "auto_checkable":item.auto_checkable,
                    "from_rule":rule.id,
                }));
            }
        }
        for check in &rule.checks {
            if checks_seen.insert(check.clone()) {
                checks_to_run.push(check.clone());
            }
        }
    }
    Ok(json!({
        "outcome":"PASS",
        "workItemId":work_item_id,
        "changed_files":changed_files,
        "matched_rules":matched_rules,
        "affected_items":affected_items,
        "checks_to_run":checks_to_run,
    }))
}

fn load_update_graph(context: &JobContext<'_>) -> RuntimeResult<UpdateGraph> {
    let path = context.project_file("source/standards/update-graph.json")?;
    let graph: UpdateGraph = serde_json::from_slice(&read_bounded(&path, MAX_FILE_BYTES)?)
        .map_err(|_| schema_error("update-graph.json violates its strict schema"))?;
    if graph.schema != "ae-sdd-update-graph/v1" {
        return Err(schema_error("update graph schema is unsupported"));
    }
    Ok(graph)
}

fn validate_update_graph(graph: &UpdateGraph) -> RuntimeResult<BTreeMap<String, Vec<String>>> {
    if graph.rules.is_empty() || graph.rules.len() > MAX_GRAPH_RULES {
        return Err(schema_error("update graph rule count is invalid"));
    }
    let mut rule_ids = BTreeSet::new();
    let mut checks: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rule in &graph.rules {
        if rule.id.trim().is_empty()
            || !rule_ids.insert(rule.id.clone())
            || rule.trigger.is_empty()
            || rule.trigger.len() > 128
            || rule.affected.is_empty()
            || rule.affected.len() > 256
            || rule.checks.is_empty()
            || rule.checks.len() > 64
            || rule.trigger.iter().any(|pattern| pattern.len() > 1_024)
        {
            return Err(schema_error(
                "update graph contains an incomplete or duplicate rule",
            ));
        }
        for check in &rule.checks {
            if !check.starts_with("UC-") || check.len() > 16 {
                return Err(schema_error("update graph contains an invalid check id"));
            }
            checks
                .entry(check.clone())
                .or_default()
                .push(rule.id.clone());
        }
    }
    Ok(checks)
}

fn check_versions(context: &JobContext<'_>) -> RuntimeResult<(bool, String)> {
    let skill = required_known_text(context, "source/SKILL.md")?;
    let paths = required_known_text(context, "tools/lib/paths.py")?;
    let readme = required_known_text(context, "README.md")?;
    let skill_version = line_value(&skill, "version:");
    let paths_version = quoted_assignment(&paths, "MASTER_VERSION");
    let readme_version = skill_version
        .as_ref()
        .filter(|version| readme.contains(&format!("v{version}")))
        .cloned();
    let passed = skill_version.is_some()
        && skill_version == paths_version
        && skill_version == readme_version;
    Ok((
        passed,
        format!(
            "source/SKILL.md={}; tools/lib/paths.py={}; README.md={}",
            skill_version.as_deref().unwrap_or("missing"),
            paths_version.as_deref().unwrap_or("missing"),
            readme_version.as_deref().unwrap_or("missing")
        ),
    ))
}

fn line_value(source: &str, prefix: &str) -> Option<String> {
    source.lines().find_map(|line| {
        line.trim()
            .strip_prefix(prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.trim_matches(['\'', '"']).to_owned())
    })
}

fn quoted_assignment(source: &str, name: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let line = line.trim();
        if !line.starts_with(name) {
            return None;
        }
        let value = line.split_once('=')?.1.trim();
        let value = value.strip_prefix('"')?.split_once('"')?.0;
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let value = value.replace('\\', "/");
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;
    let mut pattern_index = 0;
    while pattern_index < pattern.len() {
        let mut current = vec![false; value.len() + 1];
        if pattern[pattern_index] == b'*' {
            let recursive = pattern.get(pattern_index + 1) == Some(&b'*');
            pattern_index += if recursive { 2 } else { 1 };
            current[0] = previous[0];
            for value_index in 1..=value.len() {
                current[value_index] = previous[value_index]
                    || (current[value_index - 1] && (recursive || value[value_index - 1] != b'/'));
            }
        } else {
            for value_index in 1..=value.len() {
                current[value_index] =
                    previous[value_index - 1] && pattern[pattern_index] == value[value_index - 1];
            }
            pattern_index += 1;
        }
        previous = current;
    }
    previous[value.len()]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateGraph {
    #[serde(rename = "$schema")]
    schema: String,
    #[serde(rename = "description")]
    _description: String,
    #[serde(rename = "version")]
    _version: String,
    rules: Vec<UpdateRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateRule {
    id: String,
    name: String,
    trigger: Vec<String>,
    trigger_condition: String,
    affected: Vec<UpdateAffected>,
    checks: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateAffected {
    path: String,
    action: String,
    auto_checkable: bool,
}

#[cfg(test)]
mod tests {
    use super::glob_matches;

    #[test]
    fn update_graph_globs_distinguish_single_and_recursive_segments() {
        assert!(glob_matches(
            "source/skills/**/*.md",
            "source/skills/phase2-coding/coding-skill.md"
        ));
        assert!(glob_matches("scripts/*_scan.py", "scripts/flow_scan.py"));
        assert!(!glob_matches(
            "scripts/*_scan.py",
            "scripts/nested/flow_scan.py"
        ));
    }
}
