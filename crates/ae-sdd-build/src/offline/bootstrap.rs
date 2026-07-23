use std::path::{Path, PathBuf};

use super::*;
use crate::{AdminChange, PermissionClass};

pub(super) fn init(
    request: &OfflineRequest,
    project_root: &Path,
    project_key: &str,
    force: bool,
) -> Result<OfflineResult, OfflineError> {
    validate_name(project_key)?;
    let root = project_root
        .canonicalize()
        .map_err(|source| io(project_root, source))?;
    let target = root.join(".ae-sdd");
    if target.exists() && !force {
        return Err(OfflineError::AlreadyExists(display(&target)));
    }
    let config = format!(
        "schemaVersion: ae-sdd-project/v1\nprojectKey: {project_key}\ngitPath: {}\nruntime: daemon\n",
        display(&root)
    );
    apply_changes(
        request,
        &root,
        vec![
            change(".ae-sdd/config.yaml", config),
            change(
                ".ae-sdd/overrides/README.md",
                "# Project overrides\n\nKeep project-specific constraints here; do not fork the runtime.\n"
                    .to_owned(),
            ),
            change(".ae-sdd/reports/.gitkeep", String::new()),
            change(".ae-sdd/assets/.gitkeep", String::new()),
        ],
    )
}

pub(super) fn init_hooks(
    request: &OfflineRequest,
    project_root: &Path,
    executable: &str,
    hosts: &[String],
) -> Result<OfflineResult, OfflineError> {
    if executable.trim().is_empty() || executable.contains(['\0', '\r', '\n']) {
        return Err(OfflineError::InvalidInput("executable"));
    }
    if hosts.is_empty() || hosts.len() > 5 {
        return Err(OfflineError::InvalidInput("hosts"));
    }
    let root = project_root
        .canonicalize()
        .map_err(|source| io(project_root, source))?;
    let mut changes = Vec::with_capacity(hosts.len());
    for host in hosts {
        let relative = match host.as_str() {
            "claude" => ".claude/settings.json",
            "codex" => ".codex/hooks.json",
            "hermes" => ".hermes/hooks.json",
            "mavis" => ".mavis/hooks.json",
            "zcode" => ".zcode/hooks.json",
            _ => return Err(OfflineError::InvalidInput("host")),
        };
        changes.push(change(relative, hook_manifest(executable)?));
    }
    apply_changes(request, &root, changes)
}

pub(super) fn plugin_init(
    request: &OfflineRequest,
    plugins_root: &Path,
    name: &str,
    description: &str,
) -> Result<OfflineResult, OfflineError> {
    validate_name(name)?;
    if description.trim().is_empty()
        || description.len() > 1_024
        || description.contains(['\0', '\r', '\n'])
    {
        return Err(OfflineError::InvalidInput("description"));
    }
    let root = plugins_root
        .canonicalize()
        .map_err(|source| io(plugins_root, source))?;
    if root.join(name).exists() {
        return Err(OfflineError::AlreadyExists(display(&root.join(name))));
    }
    let registry = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": 1,
        "description": "Rust-native ae-sdd plugin registry",
        "plugins": [{
            "name": name,
            "type": "skill-new",
            "version": "0.1.0",
            "description": description,
            "provides": name,
            "path": format!("./{name}/SKILL.md")
        }]
    }))? + "\n";
    apply_changes(
        request,
        &root,
        vec![
            change("registry.yaml", registry),
            change(
                &format!("{name}/plugin.json"),
                serde_json::to_string_pretty(&serde_json::json!({
                    "schemaVersion": "ae-sdd-plugin/v1",
                    "name": name,
                    "description": description,
                    "enabled": true
                }))? + "\n",
            ),
            change(
                &format!("{name}/SKILL.md"),
                format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n"),
            ),
        ],
    )
}

pub(super) fn bump(
    request: &OfflineRequest,
    repository_root: &Path,
    expected_version: &str,
    new_version: &str,
) -> Result<OfflineResult, OfflineError> {
    validate_version(expected_version)?;
    validate_version(new_version)?;
    if expected_version == new_version {
        return Err(OfflineError::InvalidInput("newVersion"));
    }
    let root = repository_root
        .canonicalize()
        .map_err(|source| io(repository_root, source))?;
    let files = ["Cargo.toml", "source/SKILL.md", "README.md"];
    let mut changes = Vec::with_capacity(files.len());
    for relative in files {
        let path = root.join(relative);
        let contents = std::fs::read_to_string(&path).map_err(|source| io(&path, source))?;
        if !contents.contains(expected_version) {
            return Err(OfflineError::InvalidArtifact(format!(
                "{} does not contain expected version {expected_version}",
                display(&path)
            )));
        }
        changes.push(change(
            relative,
            contents.replace(expected_version, new_version),
        ));
    }
    apply_changes(request, &root, changes)
}

fn hook_manifest(executable: &str) -> Result<String, OfflineError> {
    let command = |method: &str| {
        format!(
            "{} hook --method hook.{method} --request-json -",
            quote_argument(executable)
        )
    };
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {
            "PreToolUse": [{"matcher":"Write|Edit|MultiEdit|Bash","hooks":[{"type":"command","command":command("pre_tool")}]}],
            "PostToolUse": [{"matcher":"Write|Edit|MultiEdit|Bash","hooks":[{"type":"command","command":command("post_tool")}]}],
            "UserPromptSubmit": [{"hooks":[{"type":"command","command":command("user_prompt")}]}],
            "Stop": [{"hooks":[{"type":"command","command":command("stop")}]}]
        }
    }))? + "\n")
}

fn quote_argument(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn change(relative: &str, contents: String) -> AdminChange {
    AdminChange {
        relative_path: PathBuf::from(relative),
        contents,
        permission: PermissionClass::PrivateFile,
    }
}

fn validate_version(value: &str) -> Result<(), OfflineError> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(OfflineError::InvalidInput("version"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_manifest_uses_frozen_native_argv() {
        let value = hook_manifest("C:\\Program Files\\ae-sdd\\ae-sdd.exe").expect("hook manifest");
        assert!(value.contains("hook --method hook.pre_tool --request-json -"));
        assert!(value.contains("hook --method hook.post_tool --request-json -"));
        assert!(!value.contains("python"));
    }

    #[test]
    fn version_validation_is_strict() {
        assert!(validate_version("1.2.3").is_ok());
        assert!(validate_version("v1.2.3").is_err());
        assert!(validate_version("1.2").is_err());
    }
}
