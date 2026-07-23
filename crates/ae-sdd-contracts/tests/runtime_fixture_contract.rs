use std::{collections::BTreeSet, path::Path};

use ae_sdd_contracts::{
    MAX_IMPACT_FACTS, MAX_MUTATION_INTENTS, MAX_REQUIRED_SERIES, MAX_ROUTE_ARTIFACTS,
    MAX_ROUTE_REASON_CODES, MAX_SERIES_GRANT_ITEMS, host::MAX_HOST_MESSAGE_BYTES,
    resource::MAX_CONTEXT_BYTES, session::MAX_CAPABILITY_TOKEN_BYTES,
};
use serde_json::Value;

#[test]
fn methodology_source_catalog_is_complete_explicit_and_contained() {
    let catalog: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../source/standards/runtime/methodology-catalog.v1.json"
    )))
    .expect("catalog JSON");
    let expected: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/runtime/methodology-catalog.expected.json"
    )))
    .expect("expected catalog JSON");
    let entries = catalog["entries"].as_array().expect("entries array");
    assert_eq!(
        entries.len(),
        expected["entryCount"].as_u64().unwrap() as usize
    );

    let mut ids = BTreeSet::new();
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source");
    for entry in entries {
        let id = entry["skillId"].as_str().expect("skillId");
        assert!(ids.insert(id), "duplicate skillId {id}");
        let activation = entry["activation"].as_str().expect("activation");
        let spawn_policy = entry["spawnPolicy"].as_str().expect("spawnPolicy");
        match activation {
            "workflow" => {
                assert_eq!(spawn_policy, "physical_series");
                assert!(!entry["routePredicates"].as_array().unwrap().is_empty());
                assert!(!entry["deliverableKinds"].as_array().unwrap().is_empty());
            }
            "capability" => assert_eq!(spawn_policy, "inline"),
            "deprecated" => assert_eq!(spawn_policy, "forbidden"),
            other => panic!("unknown activation {other}"),
        }
        for field in ["compactRef", "fallbackRef"] {
            if let Some(path) = entry[field].as_str() {
                assert!(!path.contains(".."));
                assert!(!path.contains('\\'));
                assert!(!Path::new(path).is_absolute());
                assert!(source_root.join(path).is_file(), "missing {field} for {id}");
            }
        }
    }

    for (activation, expected_count) in [
        ("workflow", 15_usize),
        ("capability", 14),
        ("deprecated", 2),
    ] {
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry["activation"] == activation)
                .count(),
            expected_count
        );
    }
}

#[test]
fn wire_golden_limits_match_the_frozen_rust_contract() {
    let golden: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/runtime/contracts-wire.v1.json"
    )))
    .expect("wire golden JSON");
    let limits = &golden["limits"];

    assert_eq!(limits["routeReasonCodes"], MAX_ROUTE_REASON_CODES);
    assert_eq!(limits["requiredSeries"], MAX_REQUIRED_SERIES);
    assert_eq!(limits["routeArtifacts"], MAX_ROUTE_ARTIFACTS);
    assert_eq!(limits["impactFacts"], MAX_IMPACT_FACTS);
    assert_eq!(limits["seriesGrantItems"], MAX_SERIES_GRANT_ITEMS);
    assert_eq!(limits["mutationIntents"], MAX_MUTATION_INTENTS);
    assert_eq!(limits["contextBundleBytes"], MAX_CONTEXT_BYTES);
    assert_eq!(limits["hostMessageBytes"], MAX_HOST_MESSAGE_BYTES);
    assert_eq!(limits["capabilityTokenBytes"], MAX_CAPABILITY_TOKEN_BYTES);
}
