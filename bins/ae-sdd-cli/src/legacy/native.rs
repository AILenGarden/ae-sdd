use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;

use serde_json::{Value, json};

use super::LegacyCommandRoute;
use super::tokens::{LegacyArgumentError, ParsedArguments};

/// A resolved B13 invocation, represented either by a caller-owned advanced
/// request file or a typed request synthesized from original legacy argv.
#[derive(Debug)]
pub struct LegacyNativeInvocation {
    pub request: LegacyNativeRequestSource,
    pub output_json: bool,
}

#[derive(Debug)]
pub enum LegacyNativeRequestSource {
    ExplicitFile(PathBuf),
    Generated(Value),
}

/// Parse one of the 13 offline Rust commands without invoking a script.
pub fn parse_native_invocation<F>(
    route: &LegacyCommandRoute,
    entrypoint: &str,
    arguments: &[String],
    environment: F,
) -> Result<LegacyNativeInvocation, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    let boolean_flags = [
        "apply",
        "disable",
        "dry-run",
        "force",
        "global",
        "json",
        "use-python",
    ];
    let mut parsed = ParsedArguments::parse(arguments, &boolean_flags)?;
    let output_json = parsed.take_boolean("json")?;
    if let Some(path) = parsed.take_required_optional("request")? {
        if !parsed.options.is_empty() || !parsed.positionals.is_empty() {
            return Err(error(
                "--request cannot be combined with generated native request arguments",
            ));
        }
        return Ok(LegacyNativeInvocation {
            request: LegacyNativeRequestSource::ExplicitFile(PathBuf::from(path)),
            output_json,
        });
    }

    let actor = take_option_or_env(
        &mut parsed,
        &["actor"],
        &["AE_SDD_ACTOR", "AE_SDD_AGENT_ID"],
        &environment,
    )?
    .unwrap_or_else(|| "legacy-cli".to_owned());
    let reason = take_option_or_env(&mut parsed, &["reason"], &["AE_SDD_REASON"], &environment)?
        .unwrap_or_else(|| format!("legacy compatibility command {}", route.command_id));
    let idempotency_key = take_option_or_env(
        &mut parsed,
        &["idempotency-key"],
        &["AE_SDD_IDEMPOTENCY_KEY"],
        &environment,
    )?;
    let mode = execution_mode(&mut parsed, entrypoint, &environment)?;
    let command = command_payload(entrypoint, &mut parsed, &environment)?;
    reject_remaining(entrypoint, &parsed)?;
    let idempotency_key = idempotency_key.unwrap_or_else(|| {
        derived_idempotency_key(
            entrypoint,
            &json!({"actor":actor,"reason":reason,"mode":mode,"command":command}),
        )
    });

    let request = json!({
        "schemaVersion": "ae-sdd-offline-build/v1",
        "mode": mode,
        "actor": actor,
        "reason": reason,
        "idempotencyKey": idempotency_key,
    });
    let mut object = request
        .as_object()
        .cloned()
        .ok_or_else(|| error("internal native request construction failed"))?;
    let command = command
        .as_object()
        .ok_or_else(|| error("internal native command construction failed"))?;
    object.extend(command.clone());
    let generated = Value::Object(object);
    verify_offline_request(entrypoint, &generated)?;
    Ok(LegacyNativeInvocation {
        request: LegacyNativeRequestSource::Generated(generated),
        output_json,
    })
}

/// Verify that an advanced or synthesized offline request cannot redirect the
/// frozen route to another build kernel.
pub fn verify_offline_request(
    entrypoint: &str,
    request: &Value,
) -> Result<(), LegacyArgumentError> {
    if request.get("schemaVersion").and_then(Value::as_str) != Some("ae-sdd-offline-build/v1") {
        return Err(error(
            "offline request must use schemaVersion ae-sdd-offline-build/v1",
        ));
    }
    if request.get("command").and_then(Value::as_str) != Some(entrypoint) {
        return Err(error(
            "offline request command differs from the frozen legacy route",
        ));
    }
    Ok(())
}

fn execution_mode<F>(
    parsed: &mut ParsedArguments,
    entrypoint: &str,
    environment: &F,
) -> Result<String, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    let dry_run = parsed.take_boolean("dry-run")?;
    let apply = parsed.take_boolean("apply")?;
    let named = take_option_or_env(parsed, &["mode"], &["AE_SDD_EXECUTION_MODE"], environment)?;
    if usize::from(dry_run) + usize::from(apply) + usize::from(named.is_some()) > 1 {
        return Err(error("choose exactly one of --dry-run, --apply, or --mode"));
    }
    if dry_run {
        return Ok("dry-run".to_owned());
    }
    if apply {
        return Ok("apply".to_owned());
    }
    if let Some(mode) = named {
        return match mode.as_str() {
            "dry-run" | "apply" => Ok(mode),
            _ => Err(error("--mode must be dry-run or apply")),
        };
    }
    let read_only = matches!(
        entrypoint,
        "distributor.list" | "distributor.scan" | "runtime.verify" | "version"
    );
    Ok(if read_only { "dry-run" } else { "apply" }.to_owned())
}

fn command_payload<F>(
    entrypoint: &str,
    parsed: &mut ParsedArguments,
    environment: &F,
) -> Result<Value, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    match entrypoint {
        "assets.generate" => {
            parsed.take_boolean("force")?;
            let project_root =
                option_or_positional(parsed, &["project-root", "project-dir"], "project root")?
                    .unwrap_or(current_directory()?);
            let project_key = option_or_env(
                parsed,
                &["project-key", "project"],
                &["AE_SDD_PROJECT_KEY"],
                environment,
            )?
            .ok_or_else(|| error("assets generate requires --project-key/--project"))?;
            Ok(json!({"command":entrypoint,"projectRoot":project_root,"projectKey":project_key}))
        }
        "bump" => {
            let repository_root = option_or_env(
                parsed,
                &["repository-root", "repo-root"],
                &["AE_SDD_REPOSITORY_ROOT"],
                environment,
            )?
            .unwrap_or(current_directory()?);
            let expected_version = option_or_env(
                parsed,
                &["expected-version"],
                &["AE_SDD_EXPECTED_VERSION"],
                environment,
            )?
            .map_or_else(|| product_version(&repository_root), Ok)?;
            let new_version =
                option_or_positional(parsed, &["new-version", "version"], "new version")?
                    .ok_or_else(|| error("bump requires a new version"))?;
            Ok(
                json!({"command":entrypoint,"repositoryRoot":repository_root,"expectedVersion":expected_version,"newVersion":new_version}),
            )
        }
        "distributor.list" | "distributor.scan" => {
            let registry_file = registry_file(parsed, environment)?;
            Ok(json!({"command":entrypoint,"registryFile":registry_file}))
        }
        "distributor.register" => {
            let registry_file = registry_file(parsed, environment)?;
            let name = option_or_positional(parsed, &["name"], "distributor name")?
                .ok_or_else(|| error("distributor register requires a name"))?;
            let kind = option_or_env(
                parsed,
                &["kind", "protocol"],
                &["AE_SDD_DISTRIBUTOR_KIND"],
                environment,
            )?
            .ok_or_else(|| error("distributor register requires --kind/--protocol"))?
            .replace('_', "-");
            let target_path = option_or_env(
                parsed,
                &["target-path"],
                &["AE_SDD_DISTRIBUTOR_TARGET"],
                environment,
            )?
            .ok_or_else(|| error("distributor register requires --target-path"))?;
            let enabled = !parsed.take_boolean("disable")?;
            Ok(
                json!({"command":entrypoint,"registryFile":registry_file,"entry":{"name":name,"kind":kind,"targetPath":target_path,"enabled":enabled}}),
            )
        }
        "distributor.disable" | "distributor.enable" | "distributor.unregister" => {
            let registry_file = registry_file(parsed, environment)?;
            let name = option_or_positional(parsed, &["name"], "distributor name")?
                .ok_or_else(|| error(format!("{entrypoint} requires a name")))?;
            Ok(json!({"command":entrypoint,"registryFile":registry_file,"name":name}))
        }
        "init" => {
            let project_root =
                option_or_positional(parsed, &["project-root", "project-dir"], "project root")?
                    .ok_or_else(|| error("init requires a project root"))?;
            let project_key = option_or_positional(parsed, &["project-key"], "project key")?
                .or_else(|| environment("AE_SDD_PROJECT_KEY"))
                .ok_or_else(|| error("init requires a project key"))?;
            let force = parsed.take_boolean("force")?;
            Ok(
                json!({"command":entrypoint,"projectRoot":project_root,"projectKey":project_key,"force":force}),
            )
        }
        "init-hooks" => init_hooks_payload(parsed, environment),
        "plugin.init" => plugin_payload(parsed, environment),
        "runtime.verify" => {
            let package_directory =
                option_or_positional(parsed, &["package-directory", "path"], "package directory")?
                    .unwrap_or(current_directory()?);
            Ok(json!({"command":entrypoint,"packageDirectory":package_directory}))
        }
        "version" => Ok(json!({"command":entrypoint})),
        _ => Err(error(format!(
            "native entrypoint {entrypoint} is not one of the frozen B13 kernels"
        ))),
    }
}

fn init_hooks_payload<F>(
    parsed: &mut ParsedArguments,
    environment: &F,
) -> Result<Value, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    parsed.take_boolean("force")?;
    if parsed.take_boolean("use-python")? {
        return Err(error(
            "--use-python was removed; init-hooks only emits native Rust CLI hooks",
        ));
    }
    let global = parsed.take_boolean("global")?;
    let supplied_root =
        option_or_positional(parsed, &["project-root", "project-dir"], "project root")?;
    let project_root = if global {
        if supplied_root.is_some() {
            return Err(error("--global cannot be combined with a project root"));
        }
        home_directory(environment)?
    } else {
        supplied_root.unwrap_or(current_directory()?)
    };
    let executable = option_or_env(parsed, &["executable"], &["AE_SDD_EXECUTABLE"], environment)?
        .unwrap_or(
            std::env::current_exe()
                .map_err(io_argument)?
                .display()
                .to_string(),
        );
    let hosts = option_or_env(
        parsed,
        &["hosts", "host"],
        &["AE_SDD_HOOK_HOSTS"],
        environment,
    )?
    .map_or_else(|| vec!["claude".to_owned()], |value| comma_list(&value));
    if hosts.is_empty() {
        return Err(error("init-hooks requires at least one host"));
    }
    Ok(
        json!({"command":"init-hooks","projectRoot":project_root,"executable":executable,"hosts":hosts}),
    )
}

fn plugin_payload<F>(
    parsed: &mut ParsedArguments,
    environment: &F,
) -> Result<Value, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    parsed.take_boolean("force")?;
    let layer = parsed.take_required_optional("layer")?;
    let direct_root = parsed.take_aliases(&["plugins-root"])?;
    if layer.is_some() && direct_root.is_some() {
        return Err(error("--layer and --plugins-root cannot be combined"));
    }
    let plugins_root = if let Some(root) = direct_root {
        root
    } else {
        match layer.as_deref().unwrap_or("project") {
            "project" => format!("{}/.ae-sdd/plugins", current_directory()?),
            "global" => format!("{}/.ae-sdd/plugins", home_directory(environment)?),
            _ => return Err(error("--layer must be project or global")),
        }
    };
    let name = option_or_env(parsed, &["name"], &["AE_SDD_PLUGIN_NAME"], environment)?
        .ok_or_else(|| error("plugin init requires --name"))?;
    let description = option_or_env(
        parsed,
        &["description"],
        &["AE_SDD_PLUGIN_DESCRIPTION"],
        environment,
    )?
    .unwrap_or_else(|| format!("Rust-native ae-sdd plugin {name}"));
    Ok(
        json!({"command":"plugin.init","pluginsRoot":plugins_root,"name":name,"description":description}),
    )
}

fn registry_file<F>(
    parsed: &mut ParsedArguments,
    environment: &F,
) -> Result<String, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    option_or_env(
        parsed,
        &["registry-file"],
        &["AE_SDD_DISTRIBUTOR_REGISTRY"],
        environment,
    )?
    .map_or_else(
        || home_directory(environment).map(|home| format!("{home}/.ae-sdd/distributors.json")),
        Ok,
    )
}

fn option_or_positional(
    parsed: &mut ParsedArguments,
    aliases: &[&str],
    description: &str,
) -> Result<Option<String>, LegacyArgumentError> {
    let named = parsed.take_aliases(aliases)?;
    let positional = if parsed.positionals.is_empty() {
        None
    } else {
        Some(parsed.positionals.remove(0))
    };
    if named.is_some() && positional.is_some() {
        Err(error(format!(
            "{description} was supplied as both a positional and named argument"
        )))
    } else {
        Ok(named.or(positional))
    }
}

fn option_or_env<F>(
    parsed: &mut ParsedArguments,
    aliases: &[&str],
    environment_names: &[&str],
    environment: &F,
) -> Result<Option<String>, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    take_option_or_env(parsed, aliases, environment_names, environment)
}

fn take_option_or_env<F>(
    parsed: &mut ParsedArguments,
    aliases: &[&str],
    environment_names: &[&str],
    environment: &F,
) -> Result<Option<String>, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(value) = parsed.take_aliases(aliases)? {
        return Ok(Some(value));
    }
    Ok(environment_names.iter().find_map(|name| {
        environment(name).and_then(|value| (!value.trim().is_empty()).then_some(value))
    }))
}

fn reject_remaining(entrypoint: &str, parsed: &ParsedArguments) -> Result<(), LegacyArgumentError> {
    if let Some(name) = parsed.options.keys().next() {
        return Err(error(format!(
            "{entrypoint} does not accept unknown flag --{name}"
        )));
    }
    if let Some(value) = parsed.positionals.first() {
        return Err(error(format!(
            "{entrypoint} does not accept extra positional argument {value:?}"
        )));
    }
    Ok(())
}

fn comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

fn current_directory() -> Result<String, LegacyArgumentError> {
    std::env::current_dir()
        .map(|path| path.display().to_string())
        .map_err(io_argument)
}

fn product_version(repository_root: &str) -> Result<String, LegacyArgumentError> {
    let path = PathBuf::from(repository_root).join("source/SKILL.md");
    let file = std::fs::File::open(&path).map_err(io_argument)?;
    let mut frontmatter_started = false;
    for line in BufReader::new(file).lines().take(64) {
        let line = line.map_err(io_argument)?;
        let trimmed = line.trim();
        if trimmed == "---" {
            if frontmatter_started {
                break;
            }
            frontmatter_started = true;
            continue;
        }
        if frontmatter_started && let Some(version) = trimmed.strip_prefix("version:") {
            let version = version.trim().trim_matches(['\'', '"']);
            if !version.is_empty() {
                return Ok(version.to_owned());
            }
        }
    }
    Err(error(format!(
        "could not read product version from {}",
        path.display()
    )))
}

fn home_directory<F>(environment: &F) -> Result<String, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    ["AE_SDD_HOME", "USERPROFILE", "HOME"]
        .iter()
        .find_map(|name| environment(name))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| error("home directory is unavailable; set AE_SDD_HOME"))
}

fn derived_idempotency_key(entrypoint: &str, request: &Value) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let canonical = serde_json::to_vec(request).unwrap_or_default();
    for byte in entrypoint
        .bytes()
        .chain(std::iter::once(0))
        .chain(canonical)
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("legacy-{entrypoint}-{hash:016x}")
}

fn error(message: impl Into<String>) -> LegacyArgumentError {
    LegacyArgumentError::new(message)
}

fn io_argument(source: io::Error) -> LegacyArgumentError {
    error(format!("legacy native request I/O failed: {source}"))
}
