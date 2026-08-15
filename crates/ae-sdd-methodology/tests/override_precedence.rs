use std::collections::BTreeMap;

use ae_sdd_contracts::{
    MethodologyQuery, OverrideDisposition, OverrideLayer, ProjectScope, SchemaVersion, SeriesKind,
    SkillId,
};
use ae_sdd_domain::{ArtifactDigest, ProjectKey, ProjectRelativePath};
use ae_sdd_methodology::{
    CompiledMethodologyEntry, MethodologyCatalog, MethodologyOverride, MethodologyResolveErrorKind,
    OverrideAuthorization, OverrideScope, compile_catalog,
};

fn production() -> (Vec<u8>, BTreeMap<ProjectRelativePath, Vec<u8>>) {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source");
    let source =
        std::fs::read(source_root.join("standards/runtime/methodology-catalog.v1.json")).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&source).unwrap();
    let mut assets = BTreeMap::new();
    for entry in value["entries"].as_array().unwrap() {
        for field in ["compactRef", "fallbackRef"] {
            if let Some(path) = entry[field].as_str() {
                let path = ProjectRelativePath::new(path).unwrap();
                assets.insert(
                    path.clone(),
                    std::fs::read(source_root.join(path.as_str())).unwrap(),
                );
            }
        }
    }
    (source, assets)
}

fn override_entry(
    skill_id: &str,
    variant: &str,
    compact_path: &str,
    contents: &[u8],
    assets: &mut BTreeMap<ProjectRelativePath, Vec<u8>>,
) -> CompiledMethodologyEntry {
    let source = serde_json::json!({
        "schemaVersion": "ae-sdd-methodology-catalog/v1",
        "catalogVersion": "1.0.0",
        "entries": [{
            "skillId": skill_id,
            "seriesKind": "coding",
            "activity": "execute",
            "variant": variant,
            "version": "1.0.0",
            "activation": "workflow",
            "spawnPolicy": "physical_series",
            "compactRef": compact_path,
            "fallbackRef": null,
            "routePredicates": [{
                "fact": "required-series",
                "operator": "contains",
                "value": "coding"
            }],
            "requiredInputs": ["coding-plan"],
            "deliverableKinds": ["code-change"],
            "requiredGates": ["G-08"],
            "toolDependencies": []
        }]
    });
    assets.insert(
        ProjectRelativePath::new(compact_path).unwrap(),
        contents.to_vec(),
    );
    compile_catalog(&serde_json::to_vec(&source).unwrap(), assets)
        .unwrap()
        .entries()[0]
        .clone()
}

fn query(catalog: &MethodologyCatalog, project: &ProjectKey) -> MethodologyQuery {
    MethodologyQuery::new(
        SchemaVersion::V1,
        SeriesKind::new("coding").unwrap(),
        ProjectScope::new(SchemaVersion::V1, project.clone(), None),
        Some(SkillId::new("phase2-coding.coding").unwrap()),
        catalog.catalog_digest(),
        catalog.project_registry_digest(project),
    )
}

#[test]
fn highest_layer_wins_with_byte_stable_complete_trace() {
    let (source, mut assets) = production();
    let bundle = compile_catalog(&source, &assets).unwrap();
    let project = ProjectKey::new("ae-sdd").unwrap();
    let target = SkillId::new("phase2-coding.coding").unwrap();

    let project_entry = override_entry(
        "project.coding",
        "project-v1",
        "plugins/project/coding.md",
        b"project coding",
        &mut assets,
    );
    let global_entry = override_entry(
        "global.coding",
        "global-v1",
        "plugins/global/coding.md",
        b"global coding",
        &mut assets,
    );
    let repository_entry = override_entry(
        "repository.coding",
        "repository-v1",
        "plugins/repository/coding.md",
        b"repository coding",
        &mut assets,
    );
    let overrides = vec![
        MethodologyOverride::new(
            OverrideLayer::Repository,
            target.clone(),
            repository_entry,
            OverrideScope::AllProjects,
            OverrideAuthorization::Authorized,
            ArtifactDigest::digest(b"repository registry"),
        )
        .unwrap(),
        MethodologyOverride::new(
            OverrideLayer::Project,
            target.clone(),
            project_entry.clone(),
            OverrideScope::Project(project.clone()),
            OverrideAuthorization::Authorized,
            ArtifactDigest::digest(b"project registry"),
        )
        .unwrap(),
        MethodologyOverride::new(
            OverrideLayer::Global,
            target,
            global_entry,
            OverrideScope::AllProjects,
            OverrideAuthorization::Authorized,
            ArtifactDigest::digest(b"global registry"),
        )
        .unwrap(),
    ];
    let mut reversed_overrides = overrides.clone();
    reversed_overrides.reverse();
    let reversed_catalog =
        MethodologyCatalog::open(bundle.clone(), &assets, reversed_overrides).unwrap();
    let catalog = MethodologyCatalog::open(bundle, &assets, overrides).unwrap();
    assert_eq!(catalog.catalog_digest(), reversed_catalog.catalog_digest());
    let first = catalog
        .resolve_with_trace(&query(&catalog, &project))
        .unwrap();
    let second = catalog
        .resolve_with_trace(&query(&catalog, &project))
        .unwrap();
    let reversed = reversed_catalog
        .resolve_with_trace(&query(&reversed_catalog, &project))
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first, reversed);
    assert_eq!(first.winner_layer(), OverrideLayer::Project);
    assert_eq!(
        first.methodology_ref().skill_id().as_str(),
        "project.coding"
    );
    assert_eq!(
        first.methodology_ref().entry_digest(),
        project_entry.entry_digest()
    );
    assert_eq!(
        first
            .override_trace()
            .iter()
            .map(|item| (item.layer, item.disposition))
            .collect::<Vec<_>>(),
        vec![
            (OverrideLayer::Project, OverrideDisposition::Selected),
            (OverrideLayer::Global, OverrideDisposition::Shadowed),
            (OverrideLayer::Repository, OverrideDisposition::Shadowed),
            (OverrideLayer::BuiltIn, OverrideDisposition::Shadowed),
        ]
    );

    let mut stale_query = query(&catalog, &project);
    stale_query.project_registry_digest = Some(ArtifactDigest::digest(b"stale registry"));
    let stale = catalog.resolve_with_trace(&stale_query).unwrap_err();
    assert_eq!(
        stale.kind(),
        MethodologyResolveErrorKind::ProjectRegistryStale
    );
    assert_eq!(stale.trace().len(), 4);
}

#[test]
fn same_layer_conflict_and_unauthorized_override_return_full_trace() {
    let (source, mut assets) = production();
    let project = ProjectKey::new("ae-sdd").unwrap();
    let target = SkillId::new("phase2-coding.coding").unwrap();
    let first = override_entry(
        "project.coding-one",
        "project-one",
        "plugins/project/coding-one.md",
        b"one",
        &mut assets,
    );
    let second = override_entry(
        "project.coding-two",
        "project-two",
        "plugins/project/coding-two.md",
        b"two",
        &mut assets,
    );
    let conflict = MethodologyCatalog::open(
        compile_catalog(&source, &assets).unwrap(),
        &assets,
        vec![
            MethodologyOverride::new(
                OverrideLayer::Project,
                target.clone(),
                first.clone(),
                OverrideScope::Project(project.clone()),
                OverrideAuthorization::Authorized,
                ArtifactDigest::digest(b"project registry"),
            )
            .unwrap(),
            MethodologyOverride::new(
                OverrideLayer::Project,
                target.clone(),
                second,
                OverrideScope::Project(project.clone()),
                OverrideAuthorization::Authorized,
                ArtifactDigest::digest(b"project registry"),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let error = conflict
        .resolve_with_trace(&query(&conflict, &project))
        .unwrap_err();
    assert_eq!(error.kind(), MethodologyResolveErrorKind::SameLayerConflict);
    assert_eq!(error.trace().len(), 3);

    let denied = MethodologyCatalog::open(
        compile_catalog(&source, &assets).unwrap(),
        &assets,
        vec![
            MethodologyOverride::new(
                OverrideLayer::Project,
                target,
                first,
                OverrideScope::Project(project.clone()),
                OverrideAuthorization::Denied,
                ArtifactDigest::digest(b"project registry"),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let error = denied
        .resolve_with_trace(&query(&denied, &project))
        .unwrap_err();
    assert_eq!(
        error.kind(),
        MethodologyResolveErrorKind::UnauthorizedOverride
    );
    assert_eq!(error.trace().len(), 2);
}

#[test]
fn trace_budget_is_bounded_per_project_resolution_not_global_catalog_size() {
    let (source, mut assets) = production();
    let bundle = compile_catalog(&source, &assets).unwrap();
    let target = SkillId::new("phase2-coding.coding").unwrap();
    let mut overrides = Vec::new();
    for index in 0..20 {
        let project = ProjectKey::new(format!("project-{index}")).unwrap();
        let entry = override_entry(
            &format!("project-{index}.coding"),
            &format!("project-{index}"),
            &format!("plugins/project-{index}/coding.md"),
            format!("project {index} coding").as_bytes(),
            &mut assets,
        );
        overrides.push(
            MethodologyOverride::new(
                OverrideLayer::Project,
                target.clone(),
                entry,
                OverrideScope::Project(project),
                OverrideAuthorization::Authorized,
                ArtifactDigest::digest(format!("registry {index}")),
            )
            .unwrap(),
        );
    }

    let catalog = MethodologyCatalog::open(bundle, &assets, overrides).unwrap();
    let project = ProjectKey::new("project-0").unwrap();
    let resolution = catalog
        .resolve_with_trace(&query(&catalog, &project))
        .unwrap();
    assert_eq!(resolution.override_trace().len(), 2);
}
