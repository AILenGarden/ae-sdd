use std::collections::BTreeMap;
use std::path::Path;

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Value, json};

use super::common::{
    JobContext, MAX_FILE_BYTES, digest, read_bounded, required_string, schema_error,
};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let layers = load_layers(context)?;
    let resolution = resolve(&layers);
    match entrypoint {
        "plugin.list" => Ok(list(&layers, &resolution)),
        "plugin.validate" => Ok(validate(&layers, &resolution)),
        "plugin.trace" => trace(context, &layers, &resolution, arguments),
        _ => unreachable!("plugin entrypoint was classified by caller"),
    }
}

#[derive(Clone)]
struct Layer {
    label: &'static str,
    priority: u8,
    relative: String,
    exists: bool,
    digest: Option<String>,
    plugins: Vec<Plugin>,
    errors: Vec<String>,
}

#[derive(Clone)]
struct Plugin {
    name: String,
    kind: String,
    version: String,
    description: String,
    path: String,
    replaces: Option<String>,
    provides: Option<String>,
    resolved_path: Option<String>,
}

impl Plugin {
    fn target(&self) -> Option<&str> {
        self.replaces.as_deref().or(self.provides.as_deref())
    }

    fn value(&self) -> Value {
        json!({
            "name":self.name,
            "type":self.kind,
            "version":self.version,
            "description":self.description,
            "path":self.path,
            "replaces":self.replaces,
            "provides":self.provides,
            "resolvedPath":self.resolved_path,
        })
    }
}

struct Resolution {
    winners: BTreeMap<String, (usize, usize)>,
    conflicts: Vec<Value>,
}

fn load_layers(context: &JobContext<'_>) -> RuntimeResult<Vec<Layer>> {
    Ok(vec![
        load_layer(context, "project", 1, ".ae-sdd/plugins/registry.yaml")?,
        Layer {
            label: "global",
            priority: 2,
            relative: "outside-workspace (not readable by workspace-scoped daemon)".to_owned(),
            exists: false,
            digest: None,
            plugins: Vec::new(),
            errors: Vec::new(),
        },
        load_layer(context, "repository", 3, "plugins/registry.yaml")?,
    ])
}

fn load_layer(
    context: &JobContext<'_>,
    label: &'static str,
    priority: u8,
    relative: &str,
) -> RuntimeResult<Layer> {
    if !context.root.join(relative).exists() {
        return Ok(Layer {
            label,
            priority,
            relative: relative.to_owned(),
            exists: false,
            digest: None,
            plugins: Vec::new(),
            errors: Vec::new(),
        });
    }
    let path = context.project_file(relative)?;
    let bytes = read_bounded(&path, MAX_FILE_BYTES)?;
    let text = String::from_utf8(bytes.clone())
        .map_err(|_| schema_error("plugin registry must be UTF-8 YAML"))?;
    let (mut plugins, mut errors) = parse_registry(&text);
    for plugin in &mut plugins {
        match resolve_plugin_path(context, &path, &plugin.path) {
            Ok(resolved) => plugin.resolved_path = Some(resolved),
            Err(_) => errors.push(format!(
                "plugin {} path is missing or outside the registered workspace",
                plugin.name
            )),
        }
    }
    Ok(Layer {
        label,
        priority,
        relative: relative.to_owned(),
        exists: true,
        digest: Some(digest(&bytes)),
        plugins,
        errors,
    })
}

fn parse_registry(text: &str) -> (Vec<Plugin>, Vec<String>) {
    let mut errors = Vec::new();
    let schema_valid = text.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "schema_version: 1" || trimmed == "schemaVersion: 1"
    });
    if !schema_valid {
        errors.push("registry schema_version must equal 1".to_owned());
    }
    let mut raw_plugins = Vec::<BTreeMap<String, String>>::new();
    let mut current: Option<BTreeMap<String, String>> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("- name:") {
            if let Some(plugin) = current.take() {
                raw_plugins.push(plugin);
            }
            let mut plugin = BTreeMap::new();
            plugin.insert("name".to_owned(), scalar(value));
            current = Some(plugin);
            continue;
        }
        let Some(plugin) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if matches!(
            key,
            "type" | "version" | "description" | "path" | "replaces" | "provides"
        ) {
            plugin.insert(key.to_owned(), scalar(value));
        }
    }
    if let Some(plugin) = current {
        raw_plugins.push(plugin);
    }
    if raw_plugins.len() > 1_024 {
        errors.push("registry plugin count exceeds 1024".to_owned());
        raw_plugins.truncate(1_024);
    }
    let mut plugins = Vec::new();
    for raw in raw_plugins {
        match plugin_from_map(&raw) {
            Ok(plugin) => plugins.push(plugin),
            Err(error) => errors.push(error),
        }
    }
    (plugins, errors)
}

fn plugin_from_map(raw: &BTreeMap<String, String>) -> Result<Plugin, String> {
    let required = |key: &str| {
        raw.get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 4_096)
            .map(str::to_owned)
            .ok_or_else(|| format!("plugin is missing valid {key}"))
    };
    let name = required("name")?;
    let kind = required("type")?;
    let version = required("version")?;
    let description = required("description")?;
    let path = required("path")?;
    if !matches!(
        kind.as_str(),
        "skill-override" | "template-override" | "skill-new" | "template-new"
    ) {
        return Err(format!("plugin {name} has unsupported type"));
    }
    let replaces = raw
        .get("replaces")
        .filter(|value| !value.is_empty())
        .cloned();
    let provides = raw
        .get("provides")
        .filter(|value| !value.is_empty())
        .cloned();
    if replaces.is_some() == provides.is_some() {
        return Err(format!(
            "plugin {name} must declare exactly one of replaces or provides"
        ));
    }
    Ok(Plugin {
        name,
        kind,
        version,
        description,
        path,
        replaces,
        provides,
        resolved_path: None,
    })
}

fn resolve_plugin_path(
    context: &JobContext<'_>,
    registry_path: &Path,
    value: &str,
) -> RuntimeResult<String> {
    let raw = Path::new(value);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        registry_path.parent().unwrap_or(registry_path).join(raw)
    };
    let resolved = context.existing_file(&candidate.to_string_lossy())?;
    Ok(resolved
        .strip_prefix(&context.root)
        .map_err(|_| schema_error("plugin path escaped the workspace"))?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn resolve(layers: &[Layer]) -> Resolution {
    let mut winners = BTreeMap::new();
    let mut contenders = BTreeMap::<String, Vec<(usize, usize)>>::new();
    for (layer_index, layer) in layers.iter().enumerate() {
        for (plugin_index, plugin) in layer.plugins.iter().enumerate() {
            if let Some(target) = plugin.target() {
                contenders
                    .entry(target.to_owned())
                    .or_default()
                    .push((layer_index, plugin_index));
                winners
                    .entry(target.to_owned())
                    .or_insert((layer_index, plugin_index));
            }
        }
    }
    let conflicts = contenders
        .into_iter()
        .filter(|(_, values)| values.len() > 1)
        .map(|(target, values)| {
            let winner = values[0];
            json!({
                "target":target,
                "winner":layers[winner.0].plugins[winner.1].name,
                "winnerLayer":layers[winner.0].label,
                "losers":values.iter().skip(1).map(|(layer,plugin)| json!({
                    "name":layers[*layer].plugins[*plugin].name,
                    "layer":layers[*layer].label,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Resolution { winners, conflicts }
}

fn list(layers: &[Layer], resolution: &Resolution) -> Value {
    json!({
        "outcome":"PASS",
        "layers":layers.iter().map(layer_value).collect::<Vec<_>>(),
        "totalPlugins":layers.iter().map(|layer| layer.plugins.len()).sum::<usize>(),
        "totalConflicts":resolution.conflicts.len(),
        "conflicts":resolution.conflicts,
    })
}

fn validate(layers: &[Layer], resolution: &Resolution) -> Value {
    let errors = layers
        .iter()
        .flat_map(|layer| {
            layer
                .errors
                .iter()
                .map(move |error| format!("{}: {error}", layer.label))
        })
        .collect::<Vec<_>>();
    let warnings = resolution
        .conflicts
        .iter()
        .map(|conflict| format!("multiple plugins target {}", conflict["target"]))
        .collect::<Vec<_>>();
    json!({
        "outcome":if errors.is_empty() {"PASS"} else {"FAIL"},
        "valid":errors.is_empty(),
        "errors":errors,
        "warnings":warnings,
        "totalPlugins":layers.iter().map(|layer| layer.plugins.len()).sum::<usize>(),
        "totalConflicts":resolution.conflicts.len(),
    })
}

fn trace(
    context: &JobContext<'_>,
    layers: &[Layer],
    resolution: &Resolution,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let target = required_string(arguments, "target")?;
    if let Some((layer, plugin)) = resolution.winners.get(target).copied() {
        let plugin = &layers[layer].plugins[plugin];
        return Ok(json!({
            "outcome":"PASS",
            "target":target,
            "hit":true,
            "layer":layers[layer].label,
            "priority":layers[layer].priority,
            "plugin":plugin.value(),
            "resolvedPath":plugin.resolved_path,
            "conflict":resolution.conflicts.iter().find(|value| value["target"] == target),
        }));
    }
    let fallback = context.existing_file(target).ok().and_then(|path| {
        path.strip_prefix(&context.root)
            .ok()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
    });
    Ok(json!({
        "outcome":"PASS",
        "target":target,
        "hit":false,
        "layer":"builtin-fallback",
        "plugin":null,
        "resolvedPath":fallback,
    }))
}

fn layer_value(layer: &Layer) -> Value {
    json!({
        "layer":layer.label,
        "priority":layer.priority,
        "registryPath":layer.relative,
        "exists":layer.exists,
        "digest":layer.digest,
        "plugins":layer.plugins.iter().map(Plugin::value).collect::<Vec<_>>(),
        "errors":layer.errors,
    })
}

fn scalar(value: &str) -> String {
    value
        .split_once(" #")
        .map_or(value, |(value, _)| value)
        .trim()
        .trim_matches(['\'', '"'])
        .to_owned()
}
