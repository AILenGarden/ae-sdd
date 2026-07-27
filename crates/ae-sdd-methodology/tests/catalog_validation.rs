use std::collections::BTreeMap;

use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_methodology::{
    MethodologyError, compile_catalog, decode_bundle, encode_bundle, verify_bundle,
};

fn fixture(relative: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/runtime")
            .join(relative),
    )
    .expect("fixture")
}

fn production_source() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../source/standards/runtime/methodology-catalog.v1.json"),
    )
    .expect("production catalog")
}

fn source_assets(source: &[u8]) -> BTreeMap<ProjectRelativePath, Vec<u8>> {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source");
    let value: serde_json::Value = serde_json::from_slice(source).expect("catalog JSON");
    let mut assets = BTreeMap::new();
    let entries = value["entries"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for entry in entries {
        for field in ["compactRef", "fallbackRef"] {
            let Some(path) = entry[field].as_str() else {
                continue;
            };
            let path = ProjectRelativePath::new(path).expect("valid fixture path");
            let bytes = std::fs::read(source_root.join(path.as_str())).expect("source asset");
            assets.insert(path, bytes);
        }
    }
    assets
}

#[test]
fn source_schema_identity_path_and_activation_fail_closed() {
    let empty = BTreeMap::new();
    assert!(matches!(
        compile_catalog(
            &fixture("methodology-invalid-unknown-schema.v1.json"),
            &empty
        ),
        Err(MethodologyError::UnsupportedSchema(_))
    ));

    let duplicate = fixture("methodology-invalid-duplicate.v1.json");
    assert!(matches!(
        compile_catalog(&duplicate, &source_assets(&duplicate)),
        Err(MethodologyError::DuplicateEntry { .. })
    ));

    assert!(matches!(
        compile_catalog(&fixture("methodology-invalid-escape.v1.json"), &empty),
        Err(MethodologyError::InvalidPath(_))
    ));

    let source = production_source();
    let mut invalid: serde_json::Value = serde_json::from_slice(&source).unwrap();
    invalid["entries"][0]["activation"] = serde_json::json!("workflow");
    invalid["entries"][0]["spawnPolicy"] = serde_json::json!("inline");
    assert!(matches!(
        compile_catalog(
            &serde_json::to_vec(&invalid).unwrap(),
            &source_assets(&source)
        ),
        Err(MethodologyError::InvalidActivationPolicy)
    ));

    let mut missing_required: serde_json::Value = serde_json::from_slice(&source).unwrap();
    missing_required["entries"][0]
        .as_object_mut()
        .unwrap()
        .remove("fallbackRef");
    assert!(matches!(
        compile_catalog(
            &serde_json::to_vec(&missing_required).unwrap(),
            &source_assets(&source)
        ),
        Err(MethodologyError::InvalidJson(_))
    ));
}

#[test]
fn missing_or_tampered_artifacts_and_bundle_digests_fail_closed() {
    let source = production_source();
    let mut assets = source_assets(&source);
    let first_path = assets.keys().next().unwrap().clone();
    let removed = assets.remove(&first_path).unwrap();
    assert!(matches!(
        compile_catalog(&source, &assets),
        Err(MethodologyError::CompactMissing(_)) | Err(MethodologyError::FallbackMissing(_))
    ));
    assets.insert(first_path, removed);

    let bundle = compile_catalog(&source, &assets).unwrap();
    let compact_path = bundle.entries()[0].compact_ref().path().clone();
    assets.get_mut(&compact_path).unwrap().push(b'!');
    assert!(matches!(
        verify_bundle(&bundle, &assets),
        Err(MethodologyError::ArtifactTampered(_))
    ));

    let mut encoded: serde_json::Value =
        serde_json::from_slice(&encode_bundle(&bundle).unwrap()).unwrap();
    encoded["entries"][0]["entryDigest"] = serde_json::json!("0".repeat(64));
    assert!(matches!(
        decode_bundle(&serde_json::to_vec(&encoded).unwrap()),
        Err(MethodologyError::EntryDigestMismatch(_))
    ));

    let mut missing_required: serde_json::Value =
        serde_json::from_slice(&encode_bundle(&bundle).unwrap()).unwrap();
    let nullable_entry = missing_required["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["fallbackRef"].is_null())
        .unwrap();
    nullable_entry
        .as_object_mut()
        .unwrap()
        .remove("fallbackRef");
    assert!(matches!(
        decode_bundle(&serde_json::to_vec(&missing_required).unwrap()),
        Err(MethodologyError::InvalidJson(_))
    ));

    let mut invalid_version: serde_json::Value =
        serde_json::from_slice(&encode_bundle(&bundle).unwrap()).unwrap();
    invalid_version["catalogVersion"] = serde_json::json!("01.0.0");
    assert!(matches!(
        decode_bundle(&serde_json::to_vec(&invalid_version).unwrap()),
        Err(MethodologyError::InvalidField {
            field: "catalogVersion",
            ..
        })
    ));

    let mut invalid_metadata: serde_json::Value =
        serde_json::from_slice(&encode_bundle(&bundle).unwrap()).unwrap();
    invalid_metadata["entries"][0]["compactRef"]["byteLength"] = serde_json::json!(0);
    assert!(matches!(
        decode_bundle(&serde_json::to_vec(&invalid_metadata).unwrap()),
        Err(MethodologyError::InvalidArtifactSize { actual: 0, .. })
    ));

    let mut fallback_missing_assets = source_assets(&source);
    let fallback_path = bundle
        .entries()
        .iter()
        .find_map(|entry| {
            entry
                .fallback_ref()
                .map(|reference| reference.path().clone())
        })
        .unwrap();
    fallback_missing_assets.remove(&fallback_path);
    assert!(matches!(
        verify_bundle(&bundle, &fallback_missing_assets),
        Err(MethodologyError::FallbackMissing(_))
    ));
}
