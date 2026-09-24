//! Trusted registrar operations. Passwords enter on stdin and are never returned.
use crate::{
    cache::SchemaCache,
    registry::{
        ConnectionProfile, Endpoint, Engine, Limits, Registry, SslProfile, WriteLevel,
        validate_alias,
    },
    security::revoke_connection_capability,
    vault::CredentialVault,
};
use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagementRequest {
    pub operation: String,
    pub name: Option<String>,
    pub confirmed_name: Option<String>,
    pub profile: Option<ProfileInput>,
    pub password: Option<String>,
    #[serde(default)]
    pub test: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileInput {
    engine: Engine,
    host: String,
    port: u16,
    database: Option<String>,
    username: String,
    ssl_mode: String,
    environment: String,
    tags: Vec<String>,
    write_level: WriteLevel,
    connect_timeout: u64,
    statement_timeout: u64,
    max_rows: usize,
}

pub async fn execute(home: &Path, mut input: ManagementRequest) -> Result<Value> {
    let path = home.join("connections.json");
    let mut registry = if path.exists() {
        Registry::load(&path)?
    } else {
        Registry::empty()
    };
    if input.operation == "list" {
        return Ok(
            json!({"connections": registry.connections.iter().map(|(name,p)| json!({"name":name,"engine":p.engine.as_str(),"database":p.database,"environment":p.environment,"tags":p.tags,"write_level":p.write_level,"default":registry.default.as_ref()==Some(name)})).collect::<Vec<_>>() }),
        );
    }
    let name = input
        .name
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("connection name is required"))?;
    validate_alias(name)?;
    let old = registry.connections.get(name).cloned();
    if input.operation == "show" {
        let p = old.ok_or_else(|| anyhow::anyhow!("connection not found"))?;
        return Ok(
            json!({"name":name,"profile":{"engine":p.engine.as_str(),"host":p.endpoint.host,"port":p.endpoint.port,"database":p.database,"username":p.username,"ssl_mode":p.ssl.mode,"environment":p.environment,"tags":p.tags,"write_level":p.write_level,"connect_timeout":p.limits.connect_timeout_seconds,"statement_timeout":p.limits.statement_timeout_seconds,"max_rows":p.limits.max_rows}}),
        );
    }
    if input.operation == "delete" {
        if input.confirmed_name.as_deref() != Some(name) {
            bail!("confirm the exact connection name before deletion");
        }
        // Revoke first. Keep the profile until all cleanup succeeds so failures are retryable.
        let revoked = revoke_connection_capability(home, name)?;
        let cache_path = home.join("schema-cache.sqlite3");
        if cache_path.exists() {
            SchemaCache::open(&cache_path)?.remove_connection(name)?;
        }
        CredentialVault::open(home)?.delete(name)?;
        registry.connections.remove(name);
        if registry.default.as_deref() == Some(name) {
            registry.default = registry.connections.keys().next().cloned();
        }
        registry.save(&path)?;
        return Ok(
            json!({"connection":name,"removed":true,"capability_revoked":revoked,"cache_cleared":true,"credential_removed":true}),
        );
    }
    if input.operation != "create" && input.operation != "update" {
        bail!("unsupported management operation");
    }
    if input.operation == "create" && old.is_some() {
        bail!("connection already exists; use edit");
    }
    if input.operation == "update" && old.is_none() {
        bail!("connection not found");
    }
    let p = input
        .profile
        .ok_or_else(|| anyhow::anyhow!("profile is required"))?;
    if p.port == 0 {
        bail!("invalid port");
    }
    let mut ssl = old
        .as_ref()
        .map(|v| v.ssl.clone())
        .unwrap_or_else(SslProfile::default);
    ssl.mode = p.ssl_mode;
    let profile = ConnectionProfile {
        engine: p.engine,
        endpoint: Endpoint {
            mode: "tcp".into(),
            host: p.host,
            port: p.port,
        },
        database: p.database.filter(|s| !s.is_empty()),
        username: p.username,
        secret_ref: format!("vault://db-operator/{name}"),
        ssl,
        environment: p.environment,
        tags: p.tags,
        limits: Limits {
            connect_timeout_seconds: p.connect_timeout,
            statement_timeout_seconds: p.statement_timeout,
            max_rows: p.max_rows,
        },
        write_level: p.write_level,
    };
    registry
        .connections
        .insert(name.to_owned(), profile.clone());
    if registry.default.is_none() {
        registry.default = Some(name.to_owned());
    }
    // Validate before changing either the registry or vault.
    Registry::from_json_str(&serde_json::to_string(&registry)?)?;
    let vault = CredentialVault::open(home)?;
    let old_password = if old.is_some() {
        Some(vault.read(name)?)
    } else {
        None
    };
    let supplied = Zeroizing::new(input.password.take().unwrap_or_default());
    let password = if supplied.is_empty() {
        old_password
            .as_deref()
            .map(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("password is required for a new connection"))?
    } else {
        supplied.as_str()
    };
    if input.test {
        let pool = crate::database::PoolHandle::connect(&profile, password).await?;
        let result = pool
            .execute(
                name,
                &profile,
                "SELECT 1 AS db_operator_test",
                password,
                WriteLevel::None,
            )
            .await;
        pool.close().await;
        result?;
    }
    vault.store(name, password)?;
    if let Err(error) = registry.save(&path) {
        if let Some(previous) = old_password {
            vault.store(name, previous.as_str())?;
        } else {
            vault.delete(name)?;
        }
        return Err(error.into());
    }
    Ok(json!({"connection":name,"saved":true,"tested":input.test}))
}
