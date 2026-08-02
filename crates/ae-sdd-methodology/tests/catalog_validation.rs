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

/// D-02 freezes `seriesKind` as a *main node* for entries that actually are a
/// Series — that is, `activation: workflow` / `spawnPolicy: physical_series`.
///
/// The catalog already carries the discriminator: `activation: capability`
/// entries (tooling like `git-insight`, `memory-management`) spawn inline, hold
/// no `routePredicates`, and are not business Series at all. Requiring *them* to
/// name a main node would be wrong, so this test scopes to workflow entries.
///
/// The resolver selects slices by exact `series_kind` equality
/// (`resolver.rs`: `entry.series_kind == query.series_kind`). A catalog that
/// spells the field `story-generate` therefore leaves the `story` main node with
/// zero resolvable slices: `FlowRuntime` asks for the frozen main node and the
/// filter matches nothing. `{kind}-generate`/`-review`/`-update` names a
/// *sub-node activity*, which belongs in its own field — the two axes are
/// disjoint per `ae-sdd-daemon-design.md` §11.1.
///
/// The compiler only checks that `seriesKind` is a well-formed portable id, so
/// nothing caught this. This test is the missing constraint.
#[test]
fn every_catalog_entry_series_kind_is_a_frozen_main_node() {
    let source = production_source();
    let value: serde_json::Value = serde_json::from_slice(&source).expect("catalog JSON");
    let entries = value["entries"].as_array().expect("entries array");

    let offenders = entries
        .iter()
        .filter(|entry| entry["activation"].as_str() == Some("workflow"))
        .filter_map(|entry| {
            let kind = entry["seriesKind"].as_str()?;
            (!ae_sdd_contracts::MAIN_NODE_SERIES_KINDS.contains(&kind))
                .then(|| (entry["skillId"].as_str().unwrap_or("<missing>"), kind))
        })
        .collect::<Vec<_>>();

    assert!(
        offenders.is_empty(),
        "a workflow entry's seriesKind must be a frozen main node; {} violate it: {offenders:?}",
        offenders.len()
    );
}

/// The discriminator above is only trustworthy if it is actually two disjoint
/// populations, so this pins the split itself.
///
/// A `capability` entry that acquired `routePredicates`, or a `workflow` entry
/// that lost them, would silently move between the two rules above.
#[test]
fn activation_partitions_the_catalog_into_series_and_capabilities() {
    let source = production_source();
    let value: serde_json::Value = serde_json::from_slice(&source).expect("catalog JSON");
    let entries = value["entries"].as_array().expect("entries array");

    for entry in entries {
        let skill = entry["skillId"].as_str().unwrap_or("<missing>");
        let predicates = entry["routePredicates"]
            .as_array()
            .map_or(0, |values| values.len());
        match entry["activation"].as_str() {
            Some("workflow") => assert!(
                predicates > 0,
                "{skill} is a Series, so it must carry routePredicates"
            ),
            Some("capability" | "deprecated") => assert_eq!(
                predicates, 0,
                "{skill} is not a Series, so it must not carry routePredicates"
            ),
            other => panic!("{skill} has an unrecognised activation: {other:?}"),
        }
    }
}

/// The contract above is only useful if it also holds in the other direction:
/// every frozen main node must resolve to at least one slice.
///
/// Without this, a catalog could satisfy the per-entry check while still
/// stranding a main node — and `FlowRuntime` would fail closed at the moment it
/// tried to run that Series, not at catalog compile time.
#[test]
fn every_frozen_main_node_resolves_to_at_least_one_slice() {
    let source = production_source();
    let value: serde_json::Value = serde_json::from_slice(&source).expect("catalog JSON");
    let entries = value["entries"].as_array().expect("entries array");

    let stranded = ae_sdd_contracts::MAIN_NODE_SERIES_KINDS
        .iter()
        .filter(|main_node| {
            !entries
                .iter()
                .any(|entry| entry["seriesKind"].as_str() == Some(**main_node))
        })
        .collect::<Vec<_>>();

    assert!(
        stranded.is_empty(),
        "every frozen main node needs a resolvable slice; stranded: {stranded:?}"
    );
}
