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
            "harness" => ".harness/hooks.json",
            "zcode" => ".zcode/hooks.json",
            _ => return Err(OfflineError::InvalidInput("host")),
        };
        changes.push(change(relative, hook_manifest(host, executable)?));
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
    let version_fields = [
        (
            "source/SKILL.md",
            format!("version: {expected_version}"),
            format!("version: {new_version}"),
        ),
        (
            "README.md",
            format!("> **版本：** v{expected_version}"),
            format!("> **版本：** v{new_version}"),
        ),
    ];
    let mut changes = Vec::with_capacity(version_fields.len());
    for (relative, expected_field, new_field) in version_fields {
        let path = root.join(relative);
        let contents = std::fs::read_to_string(&path).map_err(|source| io(&path, source))?;
        if contents.match_indices(&expected_field).count() != 1 {
            return Err(OfflineError::InvalidArtifact(format!(
                "{} does not contain exactly one authoritative version field for {expected_version}",
                display(&path)
            )));
        }
        changes.push(change(
            relative,
            contents.replacen(&expected_field, &new_field, 1),
        ));
    }
    apply_changes(request, &root, changes)
}

fn hook_manifest(host: &str, executable: &str) -> Result<String, OfflineError> {
    let command = |method: &str| {
        format!(
            "{} hook --method hook.{method} --request-json -",
            quote_argument(executable)
        )
    };
    let mut hooks = serde_json::Map::new();
    if matches!(host, "claude" | "codex") {
        hooks.insert(
            "SessionStart".to_owned(),
            serde_json::json!([{"hooks":[{
                "type":"command",
                "command":format!("{} runtime ensure --quiet", quote_argument(executable))
            }]}]),
        );
    }
    // Claude Code has a native `SubagentStart` lifecycle event (ROUTE-702d576a
    // Task 0/1: Create actuation A2, child admission B2); no other host has a
    // live-verified equivalent, so the mapping stays Claude-only until a host
    // passes its own actuation/admission matrix (Plan §0.7). Emitting it for
    // an unverified host would claim support the daemon cannot back.
    if host == "claude" {
        hooks.insert(
            "SubagentStart".to_owned(),
            serde_json::json!([{"hooks":[{"type":"command","command":command("subagent_start")}]}]),
        );
    }
    hooks.insert(
        "PreToolUse".to_owned(),
        serde_json::json!([{"matcher":"Write|Edit|MultiEdit|Bash","hooks":[{"type":"command","command":command("pre_tool")}]}]),
    );
    hooks.insert(
        "PostToolUse".to_owned(),
        serde_json::json!([{"matcher":"Write|Edit|MultiEdit|Bash","hooks":[{"type":"command","command":command("post_tool")}]}]),
    );
    hooks.insert(
        "UserPromptSubmit".to_owned(),
        serde_json::json!([{"hooks":[{"type":"command","command":command("user_prompt")}]}]),
    );
    hooks.insert(
        "Stop".to_owned(),
        serde_json::json!([{"hooks":[{"type":"command","command":command("stop")}]}]),
    );
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "hooks": hooks
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
        let value =
            hook_manifest("codex", "C:\\Program Files\\ae-sdd\\ae-sdd.exe").expect("hook manifest");
        assert!(value.contains("runtime ensure"));
        assert!(value.contains("hook --method hook.pre_tool --request-json -"));
        assert!(value.contains("hook --method hook.post_tool --request-json -"));
        assert!(!value.contains("python"));
    }

    #[test]
    fn session_start_is_emitted_only_for_supported_hosts() {
        let claude = hook_manifest("claude", "ae-sdd").expect("Claude hook manifest");
        let codex = hook_manifest("codex", "ae-sdd").expect("Codex hook manifest");
        let hermes = hook_manifest("hermes", "ae-sdd").expect("Hermes hook manifest");
        assert!(claude.contains("SessionStart"));
        assert!(codex.contains("SessionStart"));
        assert!(claude.contains("runtime ensure --quiet"));
        assert!(codex.contains("runtime ensure --quiet"));
        assert!(!hermes.contains("SessionStart"));
    }

    /// `SubagentStart` binds a physical child claim (ROUTE-702d576a Task 2):
    /// Claude Code has a native `SubagentStart` lifecycle event, but no host
    /// exposes an equivalent live-verified event yet, so the mapping stays
    /// Claude-only until Codex passes its own actuation/admission matrix
    /// (Plan §0.7). Emitting it for every host would claim support the daemon
    /// cannot back with live evidence.
    #[test]
    fn subagent_start_is_emitted_only_for_claude() {
        let claude = hook_manifest("claude", "ae-sdd").expect("Claude hook manifest");
        let codex = hook_manifest("codex", "ae-sdd").expect("Codex hook manifest");
        let hermes = hook_manifest("hermes", "ae-sdd").expect("Hermes hook manifest");
        assert!(claude.contains("SubagentStart"));
        assert!(claude.contains("hook --method hook.subagent_start --request-json -"));
        assert!(!codex.contains("SubagentStart"));
        assert!(!hermes.contains("SubagentStart"));
    }

    #[test]
    fn version_validation_is_strict() {
        assert!(validate_version("1.2.3").is_ok());
        assert!(validate_version("v1.2.3").is_err());
        assert!(validate_version("1.2").is_err());
    }
}
