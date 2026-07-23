use std::path::{Component, Path};

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Value, json};

use super::super::common::{JobContext, required_string};
use super::{assert_registered_project, optional_string};

const COMPLIANT_ROOTS: [&str; 6] = [
    "ae-sdd-doc",
    "design",
    ".ae-task",
    ".ae-plan",
    ".ae-sdd",
    ".auto-engineering",
];
const STRAY_COMPONENTS: [&str; 6] = ["tmp", "temp", "$temp", "desktop", "下载", "downloads"];

pub(super) fn execute(
    context: &JobContext<'_>,
    work_item_id: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let target = required_string(arguments, "path")?;
    let project_supplied = assert_registered_project(context, arguments)?;
    let intent = optional_string(arguments, "intent")?.unwrap_or_default();
    let normalized = target.replace('\\', "/");
    let folded = normalized.to_lowercase();
    let raw = Path::new(target);
    let mut issues = Vec::new();

    if raw
        .components()
        .any(|component| component == Component::ParentDir)
    {
        issues.push("path contains parent traversal".to_owned());
    }
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        context.root.join(raw)
    };
    if !candidate.starts_with(&context.root) {
        issues.push("path is outside the registered workspace".to_owned());
    }
    let components = folded
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| STRAY_COMPONENTS.contains(component))
    {
        issues.push("path contains a stray temporary or user-download location".to_owned());
    }
    let in_compliant_root = components
        .iter()
        .any(|component| COMPLIANT_ROOTS.contains(component));
    if !in_compliant_root && !project_supplied {
        issues.push(format!(
            "path is not below a compliant root ({}) and no trusted project root was supplied",
            COMPLIANT_ROOTS.join(", ")
        ));
    }
    let compliant = issues.is_empty();
    Ok(json!({
        "outcome":if compliant {"PASS"} else {"FAIL"},
        "workItemId":work_item_id,
        "path":target,
        "intent":intent,
        "compliant":compliant,
        "issues":issues,
    }))
}
