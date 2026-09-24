use std::fs;

#[test]
fn agent_feature_excludes_service_only_dependencies() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    let features = manifest["features"].as_table().expect("features table");
    let client = features["client"].as_array().expect("client feature");
    let client_items: Vec<&str> = client.iter().filter_map(toml::Value::as_str).collect();

    for forbidden in [
        "sqlx",
        "rusqlite",
        "sqlparser",
        "sha2",
        "subtle",
        "rpassword",
        "url",
    ] {
        assert!(
            !client_items.iter().any(|item| item.contains(forbidden)),
            "client feature must not enable {forbidden}"
        );
        assert_eq!(
            manifest["dependencies"][forbidden]["optional"].as_bool(),
            Some(true),
            "{forbidden} must be optional"
        );
    }
    assert!(manifest["dependencies"].get("keyring").is_none());

    let bins = manifest["bin"].as_array().expect("binary list");
    let client_bin = bins
        .iter()
        .find(|bin| bin["name"].as_str() == Some("db-operator"))
        .expect("client binary");
    assert_eq!(
        client_bin["required-features"].as_array().unwrap()[0].as_str(),
        Some("client")
    );
}
