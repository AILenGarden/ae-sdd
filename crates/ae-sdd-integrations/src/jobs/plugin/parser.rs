use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use ae_sdd_inventory::{YamlDocument, YamlValue};
use ae_sdd_methodology::OverrideAuthorization;
use yaml_rust2::scanner::{Scanner, TokenType};

use super::super::common::MAX_FILE_BYTES;
use super::model::Plugin;

pub(super) const MAX_PLUGIN_DESCRIPTION_CHARS: usize = 512;

pub(super) fn parse_registry(text: &str) -> (Vec<Plugin>, Vec<String>) {
    let mut errors = Vec::new();
    if u64::try_from(text.len()).map_or(true, |length| length > MAX_FILE_BYTES) {
        return (
            Vec::new(),
            vec![format!(
                "plugin registry byte limit exceeded (maximum {MAX_FILE_BYTES})"
            )],
        );
    }
    if let Err(error) = reject_yaml_graph_features(text) {
        return (Vec::new(), vec![error]);
    }
    let document = match YamlDocument::parse(text.as_bytes()) {
        Ok(document) => document,
        Err(error) => return (Vec::new(), vec![error.to_string()]),
    };
    let Some(root) = document.root().mapping() else {
        return (
            Vec::new(),
            vec!["plugin registry root must be a mapping".to_owned()],
        );
    };
    let allowed_root = ["schema_version", "schemaVersion", "description", "plugins"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    for key in root.keys() {
        if !allowed_root.contains(key.as_ref()) {
            errors.push(format!("registry contains unsupported field {key}"));
        }
    }
    let schema = root
        .get("schema_version")
        .or_else(|| root.get("schemaVersion"));
    if !matches!(schema, Some(YamlValue::Integer(1))) {
        errors.push("registry schema_version must equal integer 1".to_owned());
    }
    let raw_plugins = match root.get("plugins") {
        Some(YamlValue::Sequence(values)) => values,
        _ => {
            errors.push("registry plugins must be a sequence".to_owned());
            return (Vec::new(), errors);
        }
    };
    if raw_plugins.len() > 1_024 {
        errors.push("registry plugin count exceeds 1024".to_owned());
    }
    let mut plugins = Vec::new();
    for raw in raw_plugins.iter().take(1_024) {
        match plugin_from_yaml(raw) {
            Ok(plugin) => plugins.push(plugin),
            Err(error) => errors.push(error),
        }
    }
    (plugins, errors)
}

fn reject_yaml_graph_features(text: &str) -> Result<(), String> {
    let mut scanner = Scanner::new(text.chars());
    if scanner.any(|token| matches!(token.1, TokenType::Alias(_) | TokenType::Anchor(_))) {
        return Err("YAML anchors and aliases are forbidden in plugin registries".to_owned());
    }
    if let Some(error) = scanner.get_error() {
        return Err(format!("plugin registry YAML scan failed: {error}"));
    }
    Ok(())
}

fn plugin_from_yaml(raw: &YamlValue) -> Result<Plugin, String> {
    let mapping = raw
        .mapping()
        .ok_or_else(|| "plugin entry must be a mapping".to_owned())?;
    let allowed = [
        "name",
        "type",
        "version",
        "author",
        "description",
        "path",
        "replaces",
        "provides",
        "compatibility",
        "dependencies",
        "tags",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for key in mapping.keys() {
        if !allowed.contains(key.as_ref()) {
            return Err(format!("plugin contains unsupported field {key}"));
        }
    }
    let required = |key: &str| yaml_required_string(mapping, key);
    let name = required("name")?;
    let kind = required("type")?;
    let version = required("version")?;
    let description = required("description")?;
    let path = required("path")?;
    if description.chars().count() > MAX_PLUGIN_DESCRIPTION_CHARS {
        return Err(format!(
            "plugin {name} description exceeds {MAX_PLUGIN_DESCRIPTION_CHARS} characters"
        ));
    }
    if !matches!(
        kind.as_str(),
        "skill-override" | "template-override" | "skill-new" | "template-new"
    ) {
        return Err(format!("plugin {name} has unsupported type"));
    }
    if !portable_plugin_name(&name) {
        return Err(format!("plugin {name} has an invalid portable name"));
    }
    if !canonical_semver(&version) {
        return Err(format!("plugin {name} has invalid semantic version"));
    }
    if !portable_plugin_path(&path) {
        return Err(format!("plugin {name} path is not canonical and contained"));
    }
    let replaces = mapping
        .get("replaces")
        .and_then(YamlValue::string)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let provides = mapping
        .get("provides")
        .and_then(YamlValue::string)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if replaces.is_some() == provides.is_some() {
        return Err(format!(
            "plugin {name} must declare exactly one of replaces or provides"
        ));
    }
    let target_kind_matches = match kind.as_str() {
        "skill-override" | "template-override" => {
            replaces.as_deref().is_some_and(portable_builtin_target)
        }
        "skill-new" | "template-new" => provides.as_deref().is_some_and(portable_plugin_name),
        _ => false,
    };
    if !target_kind_matches {
        return Err(format!(
            "plugin {name} type does not match a canonical target field"
        ));
    }
    validate_optional_metadata(mapping, &name)?;
    let author = mapping
        .get("author")
        .and_then(YamlValue::string)
        .map(str::to_owned);
    let compatibility = mapping
        .get("compatibility")
        .and_then(YamlValue::mapping)
        .and_then(|value| value.get("ae_sdd_version"))
        .and_then(YamlValue::string)
        .map(str::to_owned);
    let tags = mapping
        .get("tags")
        .and_then(|value| match value {
            YamlValue::Sequence(values) => Some(values),
            _ => None,
        })
        .map(|values| {
            values
                .iter()
                .filter_map(YamlValue::string)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let dependencies = yaml_identifier_sequence(mapping, "dependencies", &name, 64)?;
    let unique_dependencies = dependencies.iter().collect::<BTreeSet<_>>();
    if unique_dependencies.len() != dependencies.len()
        || dependencies.iter().any(|dependency| dependency == &name)
    {
        return Err(format!(
            "plugin {name} dependencies must be unique and exclude self"
        ));
    }
    Ok(Plugin {
        name,
        kind,
        version,
        author,
        description,
        path,
        replaces,
        provides,
        dependencies,
        compatibility,
        tags,
        resolved_path: None,
        content_digest: None,
        authorization: OverrideAuthorization::Authorized,
    })
}

fn yaml_required_string(
    mapping: &BTreeMap<Box<str>, YamlValue>,
    key: &str,
) -> Result<String, String> {
    mapping
        .get(key)
        .and_then(YamlValue::string)
        .filter(|value| !value.is_empty() && value.len() <= 4_096)
        .map(str::to_owned)
        .ok_or_else(|| format!("plugin is missing valid {key}"))
}

fn validate_optional_metadata(
    mapping: &BTreeMap<Box<str>, YamlValue>,
    name: &str,
) -> Result<(), String> {
    if mapping
        .get("author")
        .is_some_and(|value| value.string().is_none())
    {
        return Err(format!("plugin {name} author must be a string"));
    }
    if let Some(value) = mapping.get("compatibility") {
        let compatibility = value
            .mapping()
            .ok_or_else(|| format!("plugin {name} compatibility must be a mapping"))?;
        let range = compatibility
            .get("ae_sdd_version")
            .and_then(YamlValue::string);
        if compatibility.len() != 1 || range.is_none() {
            return Err(format!(
                "plugin {name} compatibility must contain only ae_sdd_version"
            ));
        }
        if !range.is_some_and(basic_semver_range) {
            return Err(format!("plugin {name} compatibility range is invalid"));
        }
    }
    if let Some(value) = mapping.get("tags") {
        let YamlValue::Sequence(tags) = value else {
            return Err(format!("plugin {name} tags must be a sequence"));
        };
        if tags.len() > 64
            || tags
                .iter()
                .any(|tag| !tag.string().is_some_and(portable_tag))
        {
            return Err(format!("plugin {name} tags contain an invalid bounded tag"));
        }
    }
    Ok(())
}

fn yaml_identifier_sequence(
    mapping: &BTreeMap<Box<str>, YamlValue>,
    key: &str,
    plugin_name: &str,
    maximum: usize,
) -> Result<Vec<String>, String> {
    let Some(value) = mapping.get(key) else {
        return Ok(Vec::new());
    };
    let YamlValue::Sequence(values) = value else {
        return Err(format!("plugin {plugin_name} {key} must be a sequence"));
    };
    if values.len() > maximum {
        return Err(format!(
            "plugin {plugin_name} {key} exceeds the {maximum}-item limit"
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .string()
                .filter(|value| portable_plugin_name(value))
                .map(str::to_owned)
                .ok_or_else(|| {
                    format!("plugin {plugin_name} {key} contains an invalid plugin name")
                })
        })
        .collect()
}

fn portable_plugin_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn portable_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn canonical_semver(value: &str) -> bool {
    let components: Vec<&str> = value.split('.').collect();
    components.len() == 3
        && components.iter().all(|component| {
            !component.is_empty()
                && component.bytes().all(|byte| byte.is_ascii_digit())
                && (*component == "0" || !component.starts_with('0'))
        })
}

fn basic_semver_range(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split(',').all(|clause| {
            let clause = clause.trim();
            let version = [">=", "<=", ">", "<", "=", "^", "~"]
                .into_iter()
                .find_map(|operator| clause.strip_prefix(operator))
                .unwrap_or(clause)
                .trim();
            canonical_semver(version)
        })
}

fn portable_plugin_path(value: &str) -> bool {
    let path = Path::new(value);
    let relative = value.strip_prefix("./").unwrap_or(value);
    !relative.is_empty()
        && value.len() <= 4_096
        && !value.contains('\\')
        && !value.contains("//")
        && !value.bytes().any(|byte| byte.is_ascii_control())
        && !path.is_absolute()
        && relative.split('/').all(|segment| {
            !segment.is_empty()
                && !matches!(segment, "." | "..")
                && !is_windows_reserved_device_segment(segment)
        })
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Windows 将这些设备名(不区分大小写，忽略扩展名)作为保留字，即使在非 Windows
/// 宿主上编译也必须一致拒绝，因为插件路径需要在 Windows release 上同样可写。
fn is_windows_reserved_device_segment(segment: &str) -> bool {
    const RESERVED_NAMES: [&str; 24] = [
        "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
        "LPT9",
    ];
    let stem = segment.split('.').next().unwrap_or(segment);
    RESERVED_NAMES
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
}

fn portable_builtin_target(value: &str) -> bool {
    !value.starts_with("./")
        && (value.starts_with("source/skills/") || value.starts_with("source/templates/"))
        && portable_plugin_path(value)
}
