use std::collections::BTreeMap;

use ae_sdd_contracts::{
    MethodologyCatalogPort, MethodologyQuery, OverrideLayer, ProjectScope, SchemaVersion,
    SeriesKind, SkillId,
};
use ae_sdd_domain::{ProjectKey, ProjectRelativePath};
use ae_sdd_methodology::{MethodologyCatalog, MethodologyResolveErrorKind, compile_catalog};

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

fn query(
    catalog: &MethodologyCatalog,
    series_kind: &str,
    requested_skill: Option<&str>,
) -> MethodologyQuery {
    MethodologyQuery::new(
        SchemaVersion::V1,
        SeriesKind::new(series_kind).unwrap(),
        ProjectScope::new(SchemaVersion::V1, ProjectKey::new("ae-sdd").unwrap(), None),
        requested_skill.map(|value| SkillId::new(value).unwrap()),
        catalog.catalog_digest(),
        None,
    )
}

#[test]
fn activation_controls_resolution_and_physical_spawn() {
    let (source, assets) = production();
    let bundle = compile_catalog(&source, &assets).unwrap();
    let catalog = MethodologyCatalog::open(bundle, &assets, vec![]).unwrap();

    let workflow = catalog
        .resolve_with_trace(&query(&catalog, "coding", Some("phase2-coding.coding")))
        .unwrap();
    assert_eq!(workflow.winner_layer(), OverrideLayer::BuiltIn);
    assert_eq!(workflow.methodology_ref().series_kind().as_str(), "coding");
    assert_eq!(
        catalog
            .resolve_physical(&query(&catalog, "coding", Some("phase2-coding.coding")))
            .unwrap(),
        workflow
    );

    let deprecated = catalog
        .resolve_with_trace(&query(&catalog, "proposal", Some("cross-cutting.proposal")))
        .unwrap_err();
    assert_eq!(deprecated.kind(), MethodologyResolveErrorKind::Deprecated);
    assert_eq!(deprecated.trace().len(), 1);

    let capability = query(
        &catalog,
        "plugin-loader",
        Some("cross-cutting.ae-sdd-plugin-loader"),
    );
    assert!(catalog.resolve_with_trace(&capability).is_ok());
    assert_eq!(
        catalog.resolve_physical(&capability).unwrap_err().kind(),
        MethodologyResolveErrorKind::PhysicalSpawnForbidden
    );

    let via_port = MethodologyCatalogPort::resolve(&catalog, &capability).unwrap();
    assert_eq!(via_port.winner_layer(), OverrideLayer::BuiltIn);
}
