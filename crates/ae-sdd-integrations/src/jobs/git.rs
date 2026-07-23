use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{RuntimeError, RuntimeResult};
use serde_json::{Value, json};

use super::common::{JobContext, bounded_u64, required_string, schema_error};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    match entrypoint {
        "git.status" => status(context),
        "git.diff" => diff(context, arguments),
        "git.log" => log(context, arguments),
        "git.blame" => blame(context, arguments),
        "git.impact" => impact(context, arguments),
        _ => unreachable!("Git entrypoint was classified by caller"),
    }
}

fn status(context: &JobContext<'_>) -> RuntimeResult<Value> {
    let output = run_git(context, &["status", "--short", "--branch"])?;
    if output.exit_code != Some(0) {
        return Ok(command_error(output));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut branch = Value::Null;
    let mut entries = Vec::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("## ") {
            branch = Value::String(value.to_owned());
            continue;
        }
        if line.len() < 3 {
            continue;
        }
        entries.push(json!({
            "index":line.chars().next().unwrap_or(' '),
            "worktree":line.chars().nth(1).unwrap_or(' '),
            "path":line.get(3..).unwrap_or_default(),
        }));
    }
    Ok(json!({
        "outcome":"PASS",
        "branch":branch,
        "clean":entries.is_empty(),
        "entries":entries,
    }))
}

fn diff(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    let mut args = vec!["diff".to_owned()];
    if arguments.get("stat").and_then(Value::as_bool) == Some(true) {
        args.push("--stat".to_owned());
    }
    append_revision_range(&mut args, arguments)?;
    args.push("--".to_owned());
    let output = run_git_owned(context, &args)?;
    output_value(output, json!({"base":arguments.get("base"),"head":arguments.get("head")}))
}

fn log(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    let limit = bounded_u64(arguments, "limit", 20, 100)?;
    let mut args = vec![
        "log".to_owned(),
        format!("-n{limit}"),
        "--format=%H%x1f%an%x1f%aI%x1f%s".to_owned(),
    ];
    if let Some(path) = arguments
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        validate_project_path(context, path, false)?;
        args.push("--".to_owned());
        args.push(path.replace('\\', "/"));
    }
    let output = run_git_owned(context, &args)?;
    if output.exit_code != Some(0) {
        return Ok(command_error(output));
    }
    let commits = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let fields = line.splitn(4, '\u{1f}').collect::<Vec<_>>();
            (fields.len() == 4).then(|| {
                json!({
                    "commit":fields[0],
                    "author":fields[1],
                    "authoredAt":fields[2],
                    "subject":fields[3],
                })
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"outcome":"PASS","commits":commits,"limit":limit}))
}

fn blame(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    let file = arguments
        .get("file")
        .and_then(Value::as_str)
        .or_else(|| arguments.get("path").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| schema_error("git.blame requires file"))?;
    validate_project_path(context, file, true)?;
    let mut args = vec!["blame".to_owned(), "--line-porcelain".to_owned()];
    match (
        arguments.get("start").and_then(Value::as_u64),
        arguments.get("end").and_then(Value::as_u64),
    ) {
        (Some(start), Some(end)) if start > 0 && end >= start && end - start <= 10_000 => {
            args.push(format!("-L{start},{end}"));
        }
        (None, None) => {}
        _ => return Err(schema_error("git.blame line range is invalid or too large")),
    }
    args.push("--".to_owned());
    args.push(file.replace('\\', "/"));
    let output = run_git_owned(context, &args)?;
    output_value(output, json!({"file":file}))
}

fn impact(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    let mut files = argument_files(arguments)?;
    if files.is_empty() {
        let mut args = vec!["diff".to_owned(), "--name-only".to_owned()];
        append_revision_range(&mut args, arguments)?;
        args.push("--".to_owned());
        let output = run_git_owned(context, &args)?;
        if output.exit_code != Some(0) {
            return Ok(command_error(output));
        }
        files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect();
    }
    if files.len() > 10_000 {
        return Err(schema_error("git.impact file list exceeds its bound"));
    }
    let mut areas = BTreeMap::<String, usize>::new();
    let mut extensions = BTreeMap::<String, usize>::new();
    for file in &files {
        validate_project_path(context, file, false)?;
        let normalized = file.replace('\\', "/");
        let area = normalized.split('/').next().unwrap_or("root");
        *areas.entry(area.to_owned()).or_default() += 1;
        let extension = Path::new(&normalized)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("none");
        *extensions.entry(extension.to_owned()).or_default() += 1;
    }
    Ok(json!({
        "outcome":"PASS",
        "files":files,
        "fileCount":files.len(),
        "areas":areas,
        "extensions":extensions,
    }))
}

fn append_revision_range(args: &mut Vec<String>, arguments: &Value) -> RuntimeResult<()> {
    let base = arguments
        .get("base")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let head = arguments
        .get("head")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (base, head) {
        (Some(base), Some(head)) => {
            validate_ref(base)?;
            validate_ref(head)?;
            args.push(format!("{base}..{head}"));
        }
        (Some(value), None) | (None, Some(value)) => {
            validate_ref(value)?;
            args.push(value.to_owned());
        }
        (None, None) => {}
    }
    Ok(())
}

fn validate_ref(value: &str) -> RuntimeResult<()> {
    if value.is_empty()
        || value.len() > 256
        || value.starts_with('-')
        || value.contains("..")
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.' | '/' | '~' | '^')
        })
    {
        return Err(schema_error("Git revision is invalid"));
    }
    Ok(())
}

fn validate_project_path(
    context: &JobContext<'_>,
    value: &str,
    must_exist: bool,
) -> RuntimeResult<()> {
    ProjectRelativePath::new(value.replace('\\', "/"))
        .map_err(|_| schema_error("Git path must be project-relative and traversal-free"))?;
    if value.starts_with('-') || value.len() > 4_096 {
        return Err(schema_error("Git path is invalid"));
    }
    if must_exist {
        context.existing_file(value)?;
    }
    Ok(())
}

fn argument_files(arguments: &Value) -> RuntimeResult<Vec<String>> {
    let value = arguments.get("files").or_else(|| arguments.get("file"));
    match value {
        None => Ok(Vec::new()),
        Some(Value::String(value)) => Ok(vec![value.trim().to_owned()]),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .map(str::to_owned)
                    .ok_or_else(|| schema_error("git.impact files must contain strings"))
            })
            .collect(),
        Some(_) => Err(schema_error("git.impact files must be a string or array")),
    }
}

fn run_git(
    context: &JobContext<'_>,
    arguments: &[&str],
) -> RuntimeResult<crate::BoundedCommandOutput> {
    let owned = arguments
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    run_git_owned(context, &owned)
}

fn run_git_owned(
    context: &JobContext<'_>,
    arguments: &[String],
) -> RuntimeResult<crate::BoundedCommandOutput> {
    let mut command = Vec::with_capacity(arguments.len() + 1);
    command.push("--no-pager".to_owned());
    command.extend_from_slice(arguments);
    crate::BoundedCommandRunner::new(1_048_576)
        .run(
            Path::new("git"),
            &command,
            Some(&context.root),
            Duration::from_secs(30),
        )
        .map_err(|_| RuntimeError::new(StableErrorCode::GateError, "bounded Git process failed"))
}

fn output_value(
    output: crate::BoundedCommandOutput,
    metadata: Value,
) -> RuntimeResult<Value> {
    if output.exit_code != Some(0) {
        return Ok(command_error(output));
    }
    Ok(json!({
        "outcome":"PASS",
        "exitCode":output.exit_code,
        "stdout":String::from_utf8_lossy(&output.stdout),
        "stderr":String::from_utf8_lossy(&output.stderr),
        "metadata":metadata,
    }))
}

fn command_error(output: crate::BoundedCommandOutput) -> Value {
    json!({
        "outcome":"ERROR",
        "exitCode":output.exit_code,
        "stdout":String::from_utf8_lossy(&output.stdout),
        "stderr":String::from_utf8_lossy(&output.stderr),
    })
}
