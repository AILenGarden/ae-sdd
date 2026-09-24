use db_operator_native::{
    cache::{SchemaCache, SchemaObject},
    client_config::ClientConfig,
    daemon::DaemonRuntime,
    management::{ManagementRequest, execute},
    protocol::{Operation, PROTOCOL_VERSION, Request},
    registry::{Registry, WriteLevel},
    security::{issue_client_capability, load_capability_verifier},
    vault::CredentialVault,
};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

async fn manage(home: &Path, value: Value) -> anyhow::Result<Value> {
    execute(home, serde_json::from_value::<ManagementRequest>(value)?).await
}
fn creation(name: &str) -> Value {
    json!({"operation":"create","name":name,"password":"fixture-password","profile":{"engine":"mysql","host":"invalid.example","port":3306,"database":"fixture","username":"fixture","ssl_mode":"require","environment":"test","tags":[],"write_level":"none","connect_timeout":10,"statement_timeout":30,"max_rows":500}})
}

#[tokio::test]
async fn editing_preserves_password_and_hidden_fields_and_rejects_duplicate_create() {
    let home = tempfile::tempdir().unwrap();
    manage(home.path(), creation("test-a")).await.unwrap();
    assert!(manage(home.path(), creation("test-a")).await.is_err());
    let shown = manage(home.path(), json!({"operation":"show","name":"test-a"}))
        .await
        .unwrap();
    assert!(!shown.to_string().contains("fixture-password"));
    let mut profile = shown["profile"].clone();
    profile["database"] = json!("edited");
    manage(
        home.path(),
        json!({"operation":"update","name":"test-a","profile":profile,"password":""}),
    )
    .await
    .unwrap();
    assert_eq!(
        Registry::load(&home.path().join("connections.json"))
            .unwrap()
            .connections["test-a"]
            .database
            .as_deref(),
        Some("edited")
    );
    assert_eq!(
        CredentialVault::open(home.path())
            .unwrap()
            .read("test-a")
            .unwrap()
            .as_str(),
        "fixture-password"
    );
}

#[tokio::test]
async fn deletion_requires_confirmation_cleans_scope_and_old_token_never_revives() {
    let home = tempfile::tempdir().unwrap();
    let client = tempfile::tempdir().unwrap();
    let client_path = client.path().join("client.json");
    for name in ["test-a", "test-b"] {
        manage(home.path(), creation(name)).await.unwrap();
    }
    issue_client_capability(
        home.path(),
        &client_path,
        "fixture",
        "test-a",
        WriteLevel::None,
    )
    .unwrap();
    let config: ClientConfig =
        serde_json::from_slice(&std::fs::read(&client_path).unwrap()).unwrap();
    let cache = SchemaCache::open(&home.path().join("schema-cache.sqlite3")).unwrap();
    let object = SchemaObject {
        database: "fixture".into(),
        schema: "fixture".into(),
        object_type: "table".into(),
        object_name: "demo".into(),
        metadata: json!({}),
        refreshed_at: 1,
    };
    for name in ["test-a", "test-b"] {
        cache
            .replace_snapshot(name, "f", &[object.clone()])
            .unwrap();
    }
    let runtime = DaemonRuntime::new(
        cache.path().to_owned(),
        Duration::from_secs(30),
        load_capability_verifier(&home.path().join("capability.json")).unwrap(),
    )
    .unwrap();
    assert!(
        manage(
            home.path(),
            json!({"operation":"delete","name":"test-a","confirmed_name":"wrong"})
        )
        .await
        .is_err()
    );
    assert!(client_path.exists());
    manage(
        home.path(),
        json!({"operation":"delete","name":"test-a","confirmed_name":"test-a"}),
    )
    .await
    .unwrap();
    assert!(!home.path().join("capability.json").exists());
    assert!(!client_path.exists());
    assert!(
        CredentialVault::open(home.path())
            .unwrap()
            .read("test-a")
            .is_err()
    );
    assert!(cache.get_snapshot("test-a", "f").unwrap().is_empty());
    assert_eq!(cache.get_snapshot("test-b", "f").unwrap().len(), 1);
    let registry = Registry::load(&home.path().join("connections.json")).unwrap();
    assert_eq!(registry.default.as_deref(), Some("test-b"));
    assert!(registry.connections.contains_key("test-b"));
    manage(home.path(), creation("test-a")).await.unwrap();
    let response = runtime
        .process(Request {
            version: PROTOCOL_VERSION,
            request_id: "revoked".into(),
            capability: config.capability,
            operation: Operation::Status,
        })
        .await;
    assert!(
        !response.ok,
        "deleted authorization must not revive after alias reuse"
    );
}
