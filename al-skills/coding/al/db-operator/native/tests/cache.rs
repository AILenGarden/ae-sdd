use db_operator_native::cache::{SchemaCache, SchemaObject};

#[test]
fn cache_isolated_snapshots_by_connection_fingerprint() {
    let directory = tempfile::tempdir().unwrap();
    let cache = SchemaCache::open(&directory.path().join("schema-cache.sqlite3")).unwrap();
    let object = SchemaObject {
        database: "analytics".to_owned(),
        schema: "public".to_owned(),
        object_type: "column".to_owned(),
        object_name: "orders.id".to_owned(),
        metadata: serde_json::json!({"data_type": "bigint", "nullable": false}),
        refreshed_at: 1_700_000_000,
    };

    cache
        .replace_snapshot("warehouse", "fingerprint-a", std::slice::from_ref(&object))
        .unwrap();

    assert_eq!(
        cache.get_snapshot("warehouse", "fingerprint-a").unwrap(),
        vec![object]
    );
    assert!(
        cache
            .get_snapshot("warehouse", "fingerprint-b")
            .unwrap()
            .is_empty()
    );
}
