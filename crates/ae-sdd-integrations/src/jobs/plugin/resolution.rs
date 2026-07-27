use std::collections::BTreeMap;
use std::path::Path;

use ae_sdd_contracts::{OverrideDisposition, OverrideLayer, SkillId};
use ae_sdd_domain::{ArtifactDigest, ProjectRelativePath};
use ae_sdd_methodology::{
    RegistryCandidate, RegistryTrace, RegistryTraceReason, RegistryViolation, resolve_registry,
};
use ae_sdd_runtime::RuntimeResult;
use serde_json::{Value, json};

use super::super::common::{JobContext, MAX_FILE_BYTES, digest, read_bounded, schema_error};
use super::model::{Layer, Plugin, Resolution, ResolvedPluginFile};
use super::parser::parse_registry;

pub(super) fn load_layers(context: &JobContext<'_>) -> RuntimeResult<Vec<Layer>> {
    Ok(vec![
        load_layer(context, "project", 1, ".ae-sdd/plugins/registry.yaml")?,
        Layer {
            label: "global",
            priority: 2,
            relative: "~/.ae-sdd/plugins/registry.yaml".to_owned(),
            exists: false,
            digest: None,
            plugins: Vec::new(),
            errors: Vec::new(),
        },
        load_layer(context, "repository", 3, "plugins/registry.yaml")?,
    ])
}

pub(super) fn load_layer(
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
    let (plugins, mut errors) = parse_registry(&text);
    let mut resolved_plugins = Vec::with_capacity(plugins.len());
    for mut plugin in plugins {
        if let Some(target) = plugin.replaces.as_deref()
            && context.existing_file(target).is_err()
        {
            errors.push(format!(
                "PLUGIN_L0_TARGET_NOT_VISIBLE: plugin {} replaces target {target} is not an exact visible L0 inventory file",
                plugin.name
            ));
            continue;
        }
        match resolve_plugin_file(context, &path, &plugin.path) {
            Ok(resolved) => {
                plugin.resolved_path = Some(resolved.relative);
                plugin.content_digest = Some(resolved.content_digest);
                resolved_plugins.push(plugin);
            }
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
        plugins: resolved_plugins,
        errors,
    })
}

fn resolve_plugin_file(
    context: &JobContext<'_>,
    registry_path: &Path,
    value: &str,
) -> RuntimeResult<ResolvedPluginFile> {
    let raw = Path::new(value);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        registry_path
            .parent()
            .ok_or_else(|| schema_error("plugin registry path has no parent"))?
            .join(raw)
    };
    let resolved = context.existing_file(&candidate.to_string_lossy())?;
    let content = read_bounded(&resolved, MAX_FILE_BYTES)?;
    let relative = resolved
        .strip_prefix(&context.root)
        .map_err(|_| schema_error("plugin path escaped the workspace"))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(ResolvedPluginFile {
        relative,
        content_digest: digest(&content),
    })
}

pub(super) fn resolve(layers: &[Layer]) -> Resolution {
    let mut candidates = Vec::new();
    let mut adapter_errors = Vec::new();
    for (layer_index, layer) in layers.iter().enumerate() {
        for (plugin_index, plugin) in layer.plugins.iter().enumerate() {
            match registry_candidate(layer, plugin) {
                Ok(candidate) => candidates.push((candidate, layer_index, plugin_index)),
                Err(error) => adapter_errors.push(error),
            }
        }
    }
    let mut winners = BTreeMap::new();
    let mut override_traces = BTreeMap::new();
    let pure_candidates = candidates
        .iter()
        .map(|(candidate, _, _)| candidate.clone())
        .collect();
    match resolve_registry(pure_candidates) {
        Ok(resolved) => {
            for winner in resolved.winners() {
                let selected = winner.candidate();
                if let Some((_, layer, plugin)) = candidates.iter().find(|(candidate, _, _)| {
                    candidate.layer() == selected.layer()
                        && candidate.name() == selected.name()
                        && candidate.target() == selected.target()
                        && candidate.content_digest() == selected.content_digest()
                }) {
                    winners.insert(selected.target().as_str().to_owned(), (*layer, *plugin));
                }
            }
            append_registry_trace(&mut override_traces, resolved.trace());
            Resolution {
                winners,
                conflicts: Vec::new(),
                override_traces,
                registry_digest: resolved.decision_digest().to_string(),
                adapter_errors,
            }
        }
        Err(error) => {
            append_registry_trace(&mut override_traces, error.trace());
            Resolution {
                winners,
                conflicts: error
                    .violations()
                    .iter()
                    .map(registry_violation_value)
                    .collect(),
                override_traces,
                registry_digest: error.decision_digest().to_string(),
                adapter_errors,
            }
        }
    }
}

fn registry_candidate(layer: &Layer, plugin: &Plugin) -> Result<RegistryCandidate, String> {
    let layer_kind = match layer.label {
        "project" => OverrideLayer::Project,
        "global" => OverrideLayer::Global,
        "repository" => OverrideLayer::Repository,
        other => return Err(format!("unsupported registry layer {other}")),
    };
    let name = SkillId::new(plugin.name.as_str())
        .map_err(|error| format!("plugin {} identity is invalid: {error}", plugin.name))?;
    let target = plugin
        .target()
        .ok_or_else(|| format!("plugin {} has no target", plugin.name))?;
    let target = ProjectRelativePath::new(target)
        .map_err(|error| format!("plugin {} target is invalid: {error}", plugin.name))?;
    let source_digest = layer
        .digest
        .as_deref()
        .ok_or_else(|| format!("plugin {} registry snapshot has no digest", plugin.name))?
        .parse::<ArtifactDigest>()
        .map_err(|error| format!("plugin {} registry digest is invalid: {error}", plugin.name))?;
    let content_digest = plugin
        .content_digest
        .as_deref()
        .ok_or_else(|| format!("plugin {} content has no digest", plugin.name))?
        .parse::<ArtifactDigest>()
        .map_err(|error| format!("plugin {} content digest is invalid: {error}", plugin.name))?;
    RegistryCandidate::new(
        name,
        target,
        layer_kind,
        source_digest,
        content_digest,
        plugin.authorization,
    )
    .map_err(|error| format!("plugin {} candidate is invalid: {error}", plugin.name))
}

fn append_registry_trace(output: &mut BTreeMap<String, Vec<Value>>, trace: &[RegistryTrace]) {
    for item in trace {
        output
            .entry(item.target().as_str().to_owned())
            .or_default()
            .push(registry_trace_value(item));
    }
}

fn registry_trace_value(item: &RegistryTrace) -> Value {
    json!({
        "name":item.name().as_str(),
        "target":item.target().as_str(),
        "layer":layer_label(item.layer()),
        "priority":layer_priority(item.layer()),
        "disposition":disposition_label(item.disposition()),
        "reason":trace_reason_label(item.reason()),
        "sourceDigest":item.source_digest().to_string(),
        "contentDigest":item.content_digest().to_string(),
    })
}

fn registry_violation_value(violation: &RegistryViolation) -> Value {
    match violation {
        RegistryViolation::CandidateLimit { limit, actual } => json!({
            "code":"candidate_limit",
            "limit":limit,
            "actual":actual,
        }),
        RegistryViolation::Unauthorized {
            layer,
            name,
            target,
        } => json!({
            "code":"unauthorized",
            "layer":layer_label(*layer),
            "name":name.as_str(),
            "target":target.as_str(),
        }),
        RegistryViolation::SameLayerNameConflict { layer, name } => json!({
            "code":"same_layer_name_conflict",
            "layer":layer_label(*layer),
            "name":name.as_str(),
        }),
        RegistryViolation::SameLayerTargetConflict { layer, target } => json!({
            "code":"same_layer_target_conflict",
            "layer":layer_label(*layer),
            "target":target.as_str(),
        }),
    }
}

const fn layer_label(layer: OverrideLayer) -> &'static str {
    match layer {
        OverrideLayer::Project => "project",
        OverrideLayer::Global => "global",
        OverrideLayer::Repository => "repository",
        OverrideLayer::BuiltIn => "builtin-fallback",
    }
}

const fn layer_priority(layer: OverrideLayer) -> u8 {
    match layer {
        OverrideLayer::Project => 1,
        OverrideLayer::Global => 2,
        OverrideLayer::Repository => 3,
        OverrideLayer::BuiltIn => 4,
    }
}

const fn disposition_label(value: OverrideDisposition) -> &'static str {
    match value {
        OverrideDisposition::Selected => "selected",
        OverrideDisposition::Shadowed => "shadowed",
        OverrideDisposition::Rejected => "rejected",
    }
}

const fn trace_reason_label(value: RegistryTraceReason) -> &'static str {
    match value {
        RegistryTraceReason::Selected => "selected",
        RegistryTraceReason::HigherPrioritySelected => "higher_priority_selected",
        RegistryTraceReason::Unauthorized => "unauthorized",
        RegistryTraceReason::SameLayerNameConflict => "same_layer_name_conflict",
        RegistryTraceReason::SameLayerTargetConflict => "same_layer_target_conflict",
        RegistryTraceReason::ResolutionBlocked => "resolution_blocked",
    }
}
