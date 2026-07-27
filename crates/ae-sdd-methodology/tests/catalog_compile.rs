use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_methodology::{
    EXPECTED_BUILTIN_ENTRY_COUNT, MethodologyAssetSource, MethodologyCatalog, MethodologyError,
    compile_catalog, encode_bundle, verify_builtin_coverage,
};

fn source_catalog() -> Vec<u8> {
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../source/standards/runtime/methodology-catalog.v1.json"
    ))
    .to_vec()
}

fn source_assets(source: &[u8]) -> BTreeMap<ProjectRelativePath, Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source");
    let value: serde_json::Value = serde_json::from_slice(source).expect("catalog fixture");
    let mut assets = BTreeMap::new();
    for entry in value["entries"].as_array().expect("entries") {
        for field in ["compactRef", "fallbackRef"] {
            let Some(path) = entry[field].as_str() else {
                continue;
            };
            let path = ProjectRelativePath::new(path).expect("project-relative source path");
            let bytes = std::fs::read(root.join(path.as_str())).expect("source asset");
            assets.insert(path, bytes);
        }
    }
    assets
}

struct RecordingAssets {
    assets: BTreeMap<ProjectRelativePath, Vec<u8>>,
    fallback_paths: BTreeSet<ProjectRelativePath>,
    reads: RefCell<Vec<ProjectRelativePath>>,
}

impl MethodologyAssetSource for RecordingAssets {
    fn read(&self, path: &ProjectRelativePath) -> Option<&[u8]> {
        assert!(
            !self.fallback_paths.contains(path),
            "Catalog startup eagerly read fallback body {path}"
        );
        self.reads.borrow_mut().push(path.clone());
        self.assets.get(path).map(Vec::as_slice)
    }
}

#[test]
fn production_catalog_compiles_byte_stably_and_opens_without_fallback_reads() {
    let source = source_catalog();
    let assets = source_assets(&source);
    let first = compile_catalog(&source, &assets).expect("compile production catalog");
    assert_eq!(first.entry_count(), EXPECTED_BUILTIN_ENTRY_COUNT);
    verify_builtin_coverage(&first).expect("31-entry release coverage");

    let mut reordered: serde_json::Value = serde_json::from_slice(&source).unwrap();
    reordered["entries"].as_array_mut().unwrap().reverse();
    let reordered = serde_json::to_vec(&reordered).unwrap();
    let second = compile_catalog(&reordered, &assets).expect("compile reordered catalog");
    assert_eq!(
        encode_bundle(&first).unwrap(),
        encode_bundle(&second).unwrap()
    );

    let fallback_paths = first
        .entries()
        .iter()
        .filter_map(|entry| {
            entry
                .fallback_ref()
                .map(|reference| reference.path().clone())
        })
        .collect();
    let recording = RecordingAssets {
        assets,
        fallback_paths,
        reads: RefCell::new(Vec::new()),
    };
    let catalog = MethodologyCatalog::open(first, &recording, vec![]).expect("open catalog");
    assert_eq!(catalog.entry_count(), EXPECTED_BUILTIN_ENTRY_COUNT);
    assert_eq!(recording.reads.borrow().len(), EXPECTED_BUILTIN_ENTRY_COUNT);

    let mut wrong_deprecated_identity: serde_json::Value = serde_json::from_slice(&source).unwrap();
    wrong_deprecated_identity["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["skillId"] == "cross-cutting.proposal")
        .unwrap()["skillId"] = serde_json::json!("cross-cutting.legacy-proposal");
    let wrong = compile_catalog(
        &serde_json::to_vec(&wrong_deprecated_identity).unwrap(),
        &recording.assets,
    )
    .unwrap();
    assert!(matches!(
        verify_builtin_coverage(&wrong),
        Err(MethodologyError::CoverageMismatch("deprecated identities"))
    ));
}
