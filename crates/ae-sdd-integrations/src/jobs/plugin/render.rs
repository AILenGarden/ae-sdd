use ae_sdd_runtime::RuntimeResult;
use serde_json::{Value, json};

use super::super::common::{JobContext, required_string};
use super::model::{Layer, Plugin, Resolution};

pub(super) fn list(layers: &[Layer], resolution: &Resolution) -> Value {
    let invalid = layers.iter().any(|layer| !layer.errors.is_empty())
        || !resolution.conflicts.is_empty()
        || !resolution.adapter_errors.is_empty();
    json!({
        "outcome":if invalid {"FAIL"} else {"PASS"},
        "registryDigest":resolution.registry_digest,
        "layers":layers.iter().map(layer_value).collect::<Vec<_>>(),
        "totalPlugins":layers.iter().map(|layer| layer.plugins.len()).sum::<usize>(),
        "totalConflicts":resolution.conflicts.len(),
        "conflicts":resolution.conflicts,
        "adapterErrors":resolution.adapter_errors,
    })
}

pub(super) fn validate(layers: &[Layer], resolution: &Resolution) -> Value {
    let mut errors = layers
        .iter()
        .flat_map(|layer| {
            layer
                .errors
                .iter()
                .map(move |error| format!("{}: {error}", layer.label))
        })
        .collect::<Vec<_>>();
    errors.extend(resolution.adapter_errors.iter().cloned());
    errors.extend(resolution.conflicts.iter().map(|conflict| {
        let code = conflict["code"].as_str().unwrap_or("registry_violation");
        format!("registry resolution violation: {code}")
    }));
    json!({
        "outcome":if errors.is_empty() {"PASS"} else {"FAIL"},
        "valid":errors.is_empty(),
        "errors":errors,
        "warnings":[],
        "registryDigest":resolution.registry_digest,
        "totalPlugins":layers.iter().map(|layer| layer.plugins.len()).sum::<usize>(),
        "totalConflicts":resolution.conflicts.len(),
    })
}

pub(super) fn trace(
    context: &JobContext<'_>,
    layers: &[Layer],
    resolution: &Resolution,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let target = required_string(arguments, "target")?;
    let mut errors = layers
        .iter()
        .flat_map(|layer| {
            layer
                .errors
                .iter()
                .map(move |error| format!("{}: {error}", layer.label))
        })
        .collect::<Vec<_>>();
    errors.extend(resolution.adapter_errors.iter().cloned());
    if !errors.is_empty() || !resolution.conflicts.is_empty() {
        return Ok(json!({
            "outcome":"FAIL",
            "target":target,
            "hit":false,
            "registryDigest":resolution.registry_digest,
            "sourceSnapshots":source_snapshot_trace(layers),
            "errors":errors,
            "conflicts":resolution.conflicts,
        }));
    }
    if let Some((layer, plugin)) = resolution.winners.get(target).copied() {
        let plugin = &layers[layer].plugins[plugin];
        return Ok(json!({
            "outcome":"PASS",
            "target":target,
            "hit":true,
            "layer":layers[layer].label,
            "priority":layers[layer].priority,
            "registryDigest":resolution.registry_digest,
            "sourceSnapshots":source_snapshot_trace(layers),
            "plugin":plugin.value(),
            "resolvedPath":plugin.resolved_path,
            "overrideTrace":resolution.override_traces.get(target),
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
        "registryDigest":resolution.registry_digest,
        "sourceSnapshots":source_snapshot_trace(layers),
        "plugin":null,
        "resolvedPath":fallback,
    }))
}

pub(super) fn layer_value(layer: &Layer) -> Value {
    let snapshot = source_snapshot_value(layer);
    json!({
        "layer":layer.label,
        "priority":layer.priority,
        "registryPath":layer.relative,
        "exists":layer.exists,
        "availability":snapshot["state"],
        "digest":layer.digest,
        "sourceSnapshot":snapshot,
        "plugins":layer.plugins.iter().map(Plugin::value).collect::<Vec<_>>(),
        "errors":layer.errors,
    })
}

pub(super) fn source_snapshot_trace(layers: &[Layer]) -> Vec<Value> {
    layers.iter().map(source_snapshot_value).collect()
}

fn source_snapshot_value(layer: &Layer) -> Value {
    let availability = if layer.label == "global" && !layer.exists {
        "unavailable"
    } else if layer.exists {
        "available"
    } else {
        "absent"
    };
    json!({
        "layer":layer.label,
        "kind":match layer.label {
            "project" => "workspace_project_registry",
            "global" => "user_home_registry",
            "repository" => "workspace_repository_registry",
            _ => "unknown_registry",
        },
        "state":availability,
        "path":layer.relative,
        "digest":layer.digest,
        "reasonCode":(layer.label == "global" && !layer.exists)
            .then_some("global_home_io_not_wired"),
    })
}
