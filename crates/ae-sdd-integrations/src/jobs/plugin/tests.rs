use std::fs;

use ae_sdd_methodology::OverrideAuthorization;
use ae_sdd_protocol::WorkspaceMode;
use ae_sdd_runtime::BusinessWorkspace;
use serde_json::{Value, json};
use tempfile::TempDir;

use super::super::common::{MAX_FILE_BYTES, digest};
use super::{
    JobContext,
    model::{Layer, Plugin},
    parser::{MAX_PLUGIN_DESCRIPTION_CHARS, parse_registry},
    render::{layer_value, list, source_snapshot_trace, trace, validate},
    resolution::{load_layer, resolve},
};

#[test]
fn strict_yaml_parser_accepts_nested_registry_metadata_and_colons_in_description() {
    let registry = r#"
schema_version: 1
description: "official: repository layer"
plugins:
  - name: java3d-coding-skill
    type: skill-new
    version: 1.4.0
    author: ae-sdd
    description: "Java: bounded adapter"
    provides: coding-adapter-java
    path: ./java3d-coding-skill/SKILL.md
    compatibility:
      ae_sdd_version: ">=3.6.2"
    tags: [java, adapter]
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].description, "Java: bounded adapter");
}

#[test]
fn compatibility_range_rejects_malformed_operator_and_version() {
    let registry = r#"
schema_version: 1
plugins:
  - name: incompatible
    type: skill-new
    version: 1.0.0
    description: malformed compatibility
    provides: incompatible
    path: ./incompatible/SKILL.md
    compatibility:
      ae_sdd_version: "=>3.6"
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(plugins.is_empty());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("compatibility range is invalid")),
        "{errors:?}"
    );
}

#[test]
fn schema_v1_accepts_bounded_dependency_names_and_preserves_them() {
    let registry = r#"
schema_version: 1
plugins:
  - name: dependent
    type: skill-new
    version: 1.0.0
    description: dependency-aware plugin
    provides: dependent
    path: ./dependent/SKILL.md
    dependencies: [base-plugin, policy-plugin]
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        plugins[0].value()["dependencies"],
        json!(["base-plugin", "policy-plugin"])
    );
}

#[test]
fn schema_v1_metadata_is_preserved_in_the_json_projection() {
    let registry = r#"
schema_version: 1
plugins:
  - name: projected
    type: skill-new
    version: 1.0.0
    author: ae-sdd
    description: projected metadata
    provides: projected
    path: ./projected/SKILL.md
    compatibility:
      ae_sdd_version: ">=3.6.2, <4.0.0"
    tags: [rust, adapter]
"#;

    let (plugins, errors) = parse_registry(registry);
    let value = plugins[0].value();

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(value["author"], "ae-sdd");
    assert_eq!(value["compatibility"]["aeSddVersion"], ">=3.6.2, <4.0.0");
    assert_eq!(value["tags"], json!(["rust", "adapter"]));
}

#[test]
fn dependency_names_reject_duplicates_and_self_references() {
    let registry = r#"
schema_version: 1
plugins:
  - name: dependent
    type: skill-new
    version: 1.0.0
    description: invalid dependency graph
    provides: dependent
    path: ./dependent/SKILL.md
    dependencies: [dependent, base-plugin, base-plugin]
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(plugins.is_empty());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("dependencies must be unique and exclude self")),
        "{errors:?}"
    );
}

#[test]
fn plugin_description_is_bounded_by_character_count() {
    let registry = format!(
        "schema_version: 1\nplugins:\n  - name: verbose\n    type: skill-new\n    version: 1.0.0\n    description: \"{}\"\n    provides: verbose\n    path: ./verbose/SKILL.md\n",
        "\u{754c}".repeat(MAX_PLUGIN_DESCRIPTION_CHARS + 1)
    );

    let (plugins, errors) = parse_registry(&registry);

    assert!(plugins.is_empty());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("description exceeds 512 characters")),
        "{errors:?}"
    );
}

#[test]
fn plugin_tags_reject_unbounded_or_nonportable_values() {
    let registry = format!(
        "schema_version: 1\nplugins:\n  - name: tagged\n    type: skill-new\n    version: 1.0.0\n    description: bounded tags\n    provides: tagged\n    path: ./tagged/SKILL.md\n    tags: [valid-tag, {}]\n",
        "x".repeat(129)
    );

    let (plugins, errors) = parse_registry(&registry);

    assert!(plugins.is_empty());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("tags contain an invalid bounded tag")),
        "{errors:?}"
    );
}

#[test]
fn strict_yaml_parser_rejects_anchors_before_alias_expansion() {
    let registry = r#"
schema_version: 1
plugins:
  - &shared
    name: anchored
    type: skill-new
    version: 1.0.0
    description: anchored plugin
    provides: anchored
    path: ./anchored/SKILL.md
  - *shared
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(plugins.is_empty());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("YAML anchors and aliases are forbidden")),
        "{errors:?}"
    );
}

#[test]
fn alias_bomb_is_rejected_by_byte_budget_before_graph_scan() {
    let mut registry = String::from("schema_version: 1\nbomb: &root [*root]\nplugins: []\n");
    let max_bytes = usize::try_from(MAX_FILE_BYTES).expect("test byte limit fits usize");
    registry.push_str(&"x".repeat(max_bytes + 1));

    let (plugins, errors) = parse_registry(&registry);

    assert!(plugins.is_empty());
    assert!(
        errors
            .first()
            .is_some_and(|error| error.contains("byte limit")),
        "{errors:?}"
    );
}

#[test]
fn repository_registry_remains_valid_under_the_strict_parser() {
    let registry = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plugins/registry.yaml"
    ));

    let (plugins, errors) = parse_registry(registry);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "java3d-coding-skill");
}

#[test]
fn same_layer_target_conflicts_fail_closed_and_do_not_choose_a_winner() {
    let registry = r#"
schema_version: 1
plugins:
  - name: first
    type: skill-new
    version: 1.0.0
    description: first
    provides: shared-target
    path: ./first/SKILL.md
  - name: second
    type: skill-new
    version: 1.0.0
    description: second
    provides: shared-target
    path: ./second/SKILL.md
"#;

    let (mut plugins, errors) = parse_registry(registry);
    for plugin in &mut plugins {
        plugin.content_digest = Some(digest(plugin.name.as_bytes()));
    }
    let layers = vec![Layer {
        label: "project",
        priority: 1,
        relative: ".ae-sdd/plugins/registry.yaml".to_owned(),
        exists: true,
        digest: Some(digest(registry.as_bytes())),
        plugins,
        errors,
    }];
    let resolution = resolve(&layers);

    assert!(resolution.winners.is_empty());
    assert_eq!(
        resolution.conflicts[0]["code"],
        "same_layer_target_conflict"
    );
}

#[test]
fn same_layer_name_conflict_is_rendered_as_a_typed_validation_error() {
    let plugin = |target: &str| Plugin {
        name: "duplicate-name".to_owned(),
        kind: "skill-new".to_owned(),
        version: "1.0.0".to_owned(),
        author: None,
        description: target.to_owned(),
        path: format!("./{target}/SKILL.md"),
        replaces: None,
        provides: Some(target.to_owned()),
        dependencies: Vec::new(),
        compatibility: None,
        tags: Vec::new(),
        resolved_path: Some(format!(".ae-sdd/plugins/{target}/SKILL.md")),
        content_digest: Some(digest(target.as_bytes())),
        authorization: OverrideAuthorization::Authorized,
    };
    let layers = vec![Layer {
        label: "project",
        priority: 1,
        relative: ".ae-sdd/plugins/registry.yaml".to_owned(),
        exists: true,
        digest: Some(digest(b"same-layer names")),
        plugins: vec![plugin("first-target"), plugin("second-target")],
        errors: Vec::new(),
    }];
    let resolution = resolve(&layers);

    let rendered = validate(&layers, &resolution);

    assert_eq!(rendered["outcome"], "FAIL");
    assert!(
        rendered["errors"]
            .as_array()
            .is_some_and(|errors| errors.iter().any(|error| {
                error
                    .as_str()
                    .is_some_and(|error| error.contains("same_layer_name_conflict"))
            })),
        "{rendered}"
    );
}

#[test]
fn invalid_semver_and_escaping_paths_are_rejected_before_resolution() {
    let registry = r#"
schema_version: 1
plugins:
  - name: bad-version
    type: skill-new
    version: latest
    description: invalid
    provides: bad-version
    path: ./bad/SKILL.md
  - name: escape
    type: skill-new
    version: 1.0.0
    description: invalid
    provides: escape
    path: ../outside/SKILL.md
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(plugins.is_empty());
    assert_eq!(errors.len(), 2);
}

#[test]
fn override_targets_use_canonical_builtin_paths_and_plugin_names_are_kebab_case() {
    let registry = r#"
schema_version: 1
plugins:
  - name: coding-override
    type: skill-override
    version: 1.0.0
    description: strict override
    replaces: source/skills/phase2-coding/coding-skill.md
    path: ./coding-override/SKILL.md
"#;
    let invalid_name = registry.replace("coding-override", "Coding_Override");

    let (plugins, errors) = parse_registry(registry);
    let (invalid_plugins, invalid_errors) = parse_registry(&invalid_name);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(plugins.len(), 1);
    assert_eq!(
        plugins[0].target(),
        Some("source/skills/phase2-coding/coding-skill.md")
    );
    assert!(invalid_plugins.is_empty());
    assert!(
        invalid_errors
            .iter()
            .any(|error| error.contains("portable name"))
    );
}

#[test]
fn plugin_paths_reject_non_canonical_empty_segments() {
    let registry = r#"
schema_version: 1
plugins:
  - name: empty-segment
    type: skill-new
    version: 1.0.0
    description: invalid
    provides: empty-segment
    path: ./plugin//SKILL.md
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(plugins.is_empty());
    assert!(errors.iter().any(|error| error.contains("canonical")));
}

#[test]
fn plugin_paths_reject_windows_reserved_device_segments_before_file_io() {
    let registry = r#"
schema_version: 1
plugins:
  - name: reserved-path
    type: skill-new
    version: 1.0.0
    description: invalid Windows device path
    provides: reserved-path
    path: ./CON/SKILL.md
"#;

    let (plugins, errors) = parse_registry(registry);

    assert!(plugins.is_empty());
    assert!(
        errors.iter().any(|error| error.contains("canonical")),
        "{errors:?}"
    );
}

#[test]
fn resolver_uses_explicit_layer_priority_and_records_shadowed_contenders() {
    let plugin = |name: &str| Plugin {
        name: name.to_owned(),
        kind: "skill-new".to_owned(),
        version: "1.0.0".to_owned(),
        author: None,
        description: name.to_owned(),
        path: format!("./{name}/SKILL.md"),
        replaces: None,
        provides: Some("shared-target".to_owned()),
        dependencies: Vec::new(),
        compatibility: None,
        tags: Vec::new(),
        resolved_path: Some(format!("plugins/{name}/SKILL.md")),
        content_digest: Some(digest(name.as_bytes())),
        authorization: OverrideAuthorization::Authorized,
    };
    let layers = vec![
        Layer {
            label: "repository",
            priority: 3,
            relative: "plugins/registry.yaml".to_owned(),
            exists: true,
            digest: Some("c".repeat(64)),
            plugins: vec![plugin("repository")],
            errors: Vec::new(),
        },
        Layer {
            label: "project",
            priority: 1,
            relative: ".ae-sdd/plugins/registry.yaml".to_owned(),
            exists: true,
            digest: Some("a".repeat(64)),
            plugins: vec![plugin("project")],
            errors: Vec::new(),
        },
        Layer {
            label: "global",
            priority: 2,
            relative: "global/registry.yaml".to_owned(),
            exists: true,
            digest: Some("b".repeat(64)),
            plugins: vec![plugin("global")],
            errors: Vec::new(),
        },
    ];

    let resolution = resolve(&layers);
    let winner = resolution.winners["shared-target"];

    assert_eq!(layers[winner.0].label, "project");
    assert!(resolution.conflicts.is_empty());
    assert_eq!(resolution.override_traces["shared-target"].len(), 3);
}

#[test]
fn cross_layer_same_name_with_different_targets_has_only_the_higher_layer_winner() {
    let plugin = |target: &str, resolved: &str| Plugin {
        name: "same-name".to_owned(),
        kind: "skill-new".to_owned(),
        version: "1.0.0".to_owned(),
        author: None,
        description: target.to_owned(),
        path: format!("./{resolved}/SKILL.md"),
        replaces: None,
        provides: Some(target.to_owned()),
        dependencies: Vec::new(),
        compatibility: None,
        tags: Vec::new(),
        resolved_path: Some(format!("plugins/{resolved}/SKILL.md")),
        content_digest: Some(digest(resolved.as_bytes())),
        authorization: OverrideAuthorization::Authorized,
    };
    let layers = vec![
        Layer {
            label: "repository",
            priority: 3,
            relative: "plugins/registry.yaml".to_owned(),
            exists: true,
            digest: Some("c".repeat(64)),
            plugins: vec![plugin("repository-target", "repository")],
            errors: Vec::new(),
        },
        Layer {
            label: "project",
            priority: 1,
            relative: ".ae-sdd/plugins/registry.yaml".to_owned(),
            exists: true,
            digest: Some("a".repeat(64)),
            plugins: vec![plugin("project-target", "project")],
            errors: Vec::new(),
        },
    ];

    let resolution = resolve(&layers);

    assert!(resolution.winners.contains_key("project-target"));
    assert!(!resolution.winners.contains_key("repository-target"));
}

#[test]
fn rendered_registry_digest_and_trace_are_bound_to_plugin_content() {
    let layer = |body: &[u8]| Layer {
        label: "project",
        priority: 1,
        relative: ".ae-sdd/plugins/registry.yaml".to_owned(),
        exists: true,
        digest: Some(digest(b"same registry source")),
        plugins: vec![Plugin {
            name: "content-bound".to_owned(),
            kind: "skill-new".to_owned(),
            version: "1.0.0".to_owned(),
            author: None,
            description: "content-bound".to_owned(),
            path: "./content-bound/SKILL.md".to_owned(),
            replaces: None,
            provides: Some("content-bound".to_owned()),
            dependencies: Vec::new(),
            compatibility: None,
            tags: Vec::new(),
            resolved_path: Some(".ae-sdd/plugins/content-bound/SKILL.md".to_owned()),
            content_digest: Some(digest(body)),
            authorization: OverrideAuthorization::Authorized,
        }],
        errors: Vec::new(),
    };
    let first_layers = vec![layer(b"first body")];
    let second_layers = vec![layer(b"changed body")];
    let first = resolve(&first_layers);
    let second = resolve(&second_layers);

    assert_ne!(list(&first_layers, &first)["registryDigest"], Value::Null);
    assert_ne!(
        list(&first_layers, &first)["registryDigest"],
        list(&second_layers, &second)["registryDigest"]
    );
    assert_ne!(
        first.override_traces["content-bound"][0]["contentDigest"],
        second.override_traces["content-bound"][0]["contentDigest"]
    );
}

#[test]
fn externally_denied_candidate_fails_closed_through_the_pure_registry_api() {
    let layers = vec![Layer {
        label: "project",
        priority: 1,
        relative: ".ae-sdd/plugins/registry.yaml".to_owned(),
        exists: true,
        digest: Some(digest(b"registry")),
        plugins: vec![Plugin {
            name: "denied".to_owned(),
            kind: "skill-new".to_owned(),
            version: "1.0.0".to_owned(),
            author: None,
            description: "denied".to_owned(),
            path: "./denied/SKILL.md".to_owned(),
            replaces: None,
            provides: Some("denied".to_owned()),
            dependencies: Vec::new(),
            compatibility: None,
            tags: Vec::new(),
            resolved_path: Some(".ae-sdd/plugins/denied/SKILL.md".to_owned()),
            content_digest: Some(digest(b"denied body")),
            authorization: OverrideAuthorization::Denied,
        }],
        errors: Vec::new(),
    }];

    let resolution = resolve(&layers);

    assert!(resolution.winners.is_empty());
    assert_eq!(resolution.conflicts[0]["code"], "unauthorized");
    assert_eq!(
        resolution.override_traces["denied"][0]["disposition"],
        "rejected"
    );
}

#[test]
fn global_layer_is_rendered_as_an_explicit_unavailable_source_snapshot() {
    let global = Layer {
        label: "global",
        priority: 2,
        relative: "~/.ae-sdd/plugins/registry.yaml".to_owned(),
        exists: false,
        digest: None,
        plugins: Vec::new(),
        errors: Vec::new(),
    };

    let rendered = layer_value(&global);

    assert_eq!(rendered["availability"], "unavailable");
    assert_eq!(rendered["sourceSnapshot"]["state"], "unavailable");
    assert_eq!(
        rendered["sourceSnapshot"]["reasonCode"],
        "global_home_io_not_wired"
    );
}

#[test]
fn trace_snapshot_list_keeps_the_unwired_global_layer_visible() {
    let layers = vec![Layer {
        label: "global",
        priority: 2,
        relative: "~/.ae-sdd/plugins/registry.yaml".to_owned(),
        exists: false,
        digest: None,
        plugins: Vec::new(),
        errors: Vec::new(),
    }];

    let snapshots = source_snapshot_trace(&layers);

    assert_eq!(snapshots[0]["layer"], "global");
    assert_eq!(snapshots[0]["state"], "unavailable");
    assert_eq!(snapshots[0]["reasonCode"], "global_home_io_not_wired");
}

#[test]
fn override_replaces_must_exactly_match_a_visible_l0_inventory_file() {
    let root = TempDir::new().expect("plugin workspace");
    let registry_directory = root.path().join(".ae-sdd/plugins");
    let plugin_directory = registry_directory.join("override");
    fs::create_dir_all(&plugin_directory).expect("plugin directory");
    fs::write(plugin_directory.join("SKILL.md"), "# override\n").expect("plugin body");
    fs::write(
            registry_directory.join("registry.yaml"),
            "schema_version: 1\nplugins:\n  - name: override\n    type: skill-override\n    version: 1.0.0\n    description: exact replacement\n    replaces: source/skills/missing-skill.md\n    path: ./override/SKILL.md\n",
        )
        .expect("registry");
    let workspace = BusinessWorkspace {
        workspace_id: "workspace".to_owned(),
        canonical_root: root.path().to_string_lossy().into_owned(),
        project_key: "ae-sdd".to_owned(),
        mode: WorkspaceMode::Shadow,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 0,
    };
    let context = JobContext::new(&workspace, None).expect("job context");

    let missing = load_layer(&context, "project", 1, ".ae-sdd/plugins/registry.yaml")
        .expect("missing-target layer");

    assert!(missing.plugins.is_empty());
    assert!(
        missing
            .errors
            .iter()
            .any(|error| error.contains("PLUGIN_L0_TARGET_NOT_VISIBLE")),
        "{:?}",
        missing.errors
    );

    let builtin = root.path().join("source/skills/missing-skill.md");
    fs::create_dir_all(builtin.parent().expect("builtin parent")).expect("builtin directory");
    fs::write(&builtin, "# builtin\n").expect("builtin body");
    let visible = load_layer(&context, "project", 1, ".ae-sdd/plugins/registry.yaml")
        .expect("visible-target layer");

    assert!(visible.errors.is_empty(), "{:?}", visible.errors);
    assert_eq!(visible.plugins.len(), 1);
}

#[test]
fn trace_fails_closed_when_adapter_cannot_build_a_typed_candidate() {
    let root = TempDir::new().expect("plugin workspace");
    let workspace = BusinessWorkspace {
        workspace_id: "workspace".to_owned(),
        canonical_root: root.path().to_string_lossy().into_owned(),
        project_key: "ae-sdd".to_owned(),
        mode: WorkspaceMode::Shadow,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 0,
    };
    let context = JobContext::new(&workspace, None).expect("job context");
    let layers = vec![Layer {
        label: "project",
        priority: 1,
        relative: ".ae-sdd/plugins/registry.yaml".to_owned(),
        exists: true,
        digest: Some(digest(b"registry")),
        plugins: vec![Plugin {
            name: "missing-digest".to_owned(),
            kind: "skill-new".to_owned(),
            version: "1.0.0".to_owned(),
            author: None,
            description: "missing digest".to_owned(),
            path: "./missing/SKILL.md".to_owned(),
            replaces: None,
            provides: Some("missing-digest".to_owned()),
            dependencies: Vec::new(),
            compatibility: None,
            tags: Vec::new(),
            resolved_path: Some(".ae-sdd/plugins/missing/SKILL.md".to_owned()),
            content_digest: None,
            authorization: OverrideAuthorization::Authorized,
        }],
        errors: Vec::new(),
    }];
    let resolution = resolve(&layers);

    let rendered = trace(
        &context,
        &layers,
        &resolution,
        &json!({"target":"missing-digest"}),
    )
    .expect("trace result");

    assert_eq!(rendered["outcome"], "FAIL");
    assert!(
        rendered["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty())
    );
}
