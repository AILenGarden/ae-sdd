use db_operator_native::registry::{Registry, WriteLevel};

#[test]
fn registry_loads_schema_v3_vault_profiles() {
    let registry = Registry::from_json_str(
        r#"{
          "version": 3,
          "default": "warehouse",
          "connections": {
            "warehouse": {
              "engine": "mysql",
              "endpoint": {"mode": "tcp", "host": "db.internal", "port": 3306},
              "database": null,
              "username": "reader",
              "secret_ref": "vault://db-operator/warehouse",
              "ssl": {"mode": "require"},
              "environment": "prod",
              "tags": ["reporting"],
              "limits": {
                "connect_timeout_seconds": 10,
                "statement_timeout_seconds": 30,
                "max_rows": 500
              },
              "read_only": true
            }
          }
        }"#,
    )
    .expect("schema v3 profile should load");

    let (alias, connection) = registry.resolve(None).expect("default should resolve");
    assert_eq!(alias, "warehouse");
    assert_eq!(connection.endpoint.host, "db.internal");
    assert_eq!(connection.database, None);
    assert_eq!(connection.limits.max_rows, 500);
    assert_eq!(
        connection.write_level,
        db_operator_native::registry::WriteLevel::None
    );
}

#[test]
fn registry_saves_metadata_without_secret_material() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connections.json");
    let registry = Registry::from_json_str(
        r#"{
          "version": 3,
          "default": null,
          "connections": {}
        }"#,
    )
    .unwrap();

    registry.save(&path).unwrap();

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.to_ascii_lowercase().contains("password"));
    assert_eq!(Registry::load(&path).unwrap().version, 3);
}

#[test]
fn registry_defaults_and_parses_write_levels() {
    let input = r#"{
      "version": 3,
      "default": "writer",
      "connections": {
        "writer": {
          "engine": "mysql",
          "endpoint": {"mode": "tcp", "host": "db.internal", "port": 3306},
          "database": null,
          "username": "writer",
          "secret_ref": "vault://db-operator/writer",
          "write_level": "ddl"
        },
        "reader": {
          "engine": "mysql",
          "endpoint": {"mode": "tcp", "host": "db.internal", "port": 3306},
          "database": null,
          "username": "reader",
          "secret_ref": "vault://db-operator/reader"
        }
      }
    }"#;
    let registry = Registry::from_json_str(input).unwrap();
    assert_eq!(registry.connections["writer"].write_level, WriteLevel::Ddl);
    assert_eq!(registry.connections["reader"].write_level, WriteLevel::None);
}
