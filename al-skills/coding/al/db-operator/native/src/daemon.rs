use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Mutex as AsyncMutex, watch};
use zeroize::Zeroizing;

use crate::audit::{AuditEvent, AuditLogger};
use crate::cache::{CacheError, SchemaCache, SchemaObject};
use crate::config::{
    QUERY_MAX_CONCURRENT, QUERY_MAX_PER_WINDOW, QUERY_RATE_WINDOW_SECONDS,
    QUERY_TIMEOUT_GRACE_SECONDS,
};
use crate::database::{
    DatabaseError, PoolHandle, QueryResult, profile_fingerprint, read_password_from,
};
#[cfg(unix)]
use crate::ipc::UNIX_SOCKET_MODE;
use crate::ipc::{endpoint_name, read_frame};
use crate::lifecycle::{DaemonState, Lifecycle, LifecycleAction};
use crate::protocol::{
    ErrorPayload, Operation, PROTOCOL_VERSION, Request, Response, RuntimeMetadata, decode_request,
    encode_response,
};
use crate::query_guard::{GateError, QueryGate, with_wall_clock_timeout};
use crate::registry::WriteLevel;
use crate::registry::{ConnectionProfile, Registry, config_dir};
use crate::security::{CapabilityVerifier, load_capability_verifier, validate_private_directory};
use crate::sql_policy::validate_for_level;

#[derive(Clone)]
struct PoolEntry {
    fingerprint: String,
    pool: PoolHandle,
    password: Zeroizing<String>,
}

pub struct DaemonRuntime {
    started: Instant,
    private_dir: PathBuf,
    lifecycle: Mutex<Lifecycle>,
    pools: AsyncMutex<HashMap<String, PoolEntry>>,
    cache: SchemaCache,
    shutdown: watch::Sender<bool>,
    capability: CapabilityVerifier,
    persisted_capability: bool,
    query_gate: QueryGate,
    audit: Option<AuditLogger>,
}

impl DaemonRuntime {
    pub fn new(
        cache_path: PathBuf,
        pool_idle_timeout: Duration,
        capability: CapabilityVerifier,
    ) -> Result<Arc<Self>, CacheError> {
        Self::new_with_audit(cache_path, pool_idle_timeout, capability, None)
    }

    pub fn new_with_audit(
        cache_path: PathBuf,
        pool_idle_timeout: Duration,
        capability: CapabilityVerifier,
        audit: Option<AuditLogger>,
    ) -> Result<Arc<Self>, CacheError> {
        let (shutdown, _) = watch::channel(false);
        let persisted_capability = cache_path.parent().is_some_and(|p| p.join("capability.json").exists());
        Ok(Arc::new(Self {
            started: Instant::now(),
            private_dir: cache_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            lifecycle: Mutex::new(Lifecycle::new(pool_idle_timeout)),
            pools: AsyncMutex::new(HashMap::new()),
            cache: SchemaCache::open(&cache_path)?,
            shutdown,
            capability,
            persisted_capability,
            query_gate: QueryGate::new(
                QUERY_MAX_CONCURRENT,
                QUERY_MAX_PER_WINDOW,
                Duration::from_secs(QUERY_RATE_WINDOW_SECONDS),
            ),
            audit,
        }))
    }

    pub async fn process(&self, request: Request) -> Response {
        let started = Instant::now();
        let request_id = request.request_id.clone();
        let (operation, connection) = operation_metadata(&request.operation);
        let persisted_valid = !self.persisted_capability || load_capability_verifier(&self.private_dir.join("capability.json"))
            .is_ok_and(|v| v.verify(&request.capability) && v.connection() == self.capability.connection());
        if !persisted_valid {
            self.close_pools().await;
        }
        if !persisted_valid || !self.capability.verify(&request.capability) {
            let response = error_response(request_id, RuntimeError::Authentication);
            self.record_audit(&response, operation, connection, started.elapsed());
            return response;
        }
        if let Some(requested) = requested_connection(&request.operation)
            && !self.capability.authorizes_connection(requested)
        {
            let response = error_response(request_id, RuntimeError::Authorization);
            self.record_audit(&response, operation, connection, started.elapsed());
            return response;
        }
        self.record_activity();
        let result = match request.operation {
            Operation::Status => self.status().await,
            Operation::Query {
                connection,
                sql,
                write_level,
            } => self
                .query(&connection, &sql, write_level)
                .await
                .and_then(to_value),
            Operation::SchemaGet { connection } => self.schema_get(&connection).and_then(to_value),
        };
        let response = match result {
            Ok(value) => Response {
                version: PROTOCOL_VERSION,
                request_id,
                ok: true,
                result: Some(value),
                error: None,
                runtime: Some(RuntimeMetadata {
                    mode: "daemon".to_owned(),
                    state: self.state_name().to_owned(),
                }),
            },
            Err(error) => error_response(request_id, error),
        };
        self.record_audit(&response, operation, connection, started.elapsed());
        response
    }

    pub fn subscribe_shutdown(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    pub fn request_shutdown(&self) {
        let _ = self.shutdown.send(true);
    }

    pub async fn maintenance_tick(&self) -> bool {
        if self.persisted_capability && load_capability_verifier(&self.private_dir.join("capability.json")).is_err() {
            self.close_pools().await;
        }
        let action = self
            .lifecycle
            .lock()
            .expect("lifecycle mutex poisoned")
            .tick(self.started.elapsed());
        match action {
            LifecycleAction::None => false,
            LifecycleAction::ClosePools => {
                self.close_pools().await;
                false
            }
        }
    }

    pub async fn close_pools(&self) {
        let pools: Vec<PoolHandle> = {
            let mut entries = self.pools.lock().await;
            entries.drain().map(|(_, entry)| entry.pool).collect()
        };
        for pool in pools {
            pool.close().await;
        }
    }

    async fn status(&self) -> Result<Value, RuntimeError> {
        let pool_count = self.pools.lock().await.len();
        let mut status = json!({
            "state": self.state_name(),
            "idle_seconds": self.idle_seconds(),
            "pool_count": pool_count,
        });
        if let Some(alias) = self.capability.connection() {
            let registry = Registry::load(&self.private_dir.join("connections.json"))?;
            let (stored_alias, profile) = registry.resolve(Some(alias))?;
            status["connection"] = json!(stored_alias);
            status["engine"] = json!(profile.engine.as_str());
        }
        Ok(status)
    }

    async fn query(
        &self,
        alias: &str,
        sql: &str,
        write_level: WriteLevel,
    ) -> Result<QueryResult, RuntimeError> {
        let _permit = self
            .query_gate
            .try_enter(Instant::now())
            .map_err(RuntimeError::Gate)?;
        let registry = Registry::load(&self.private_dir.join("connections.json"))?;
        let (stored_alias, profile) = registry.resolve(Some(alias))?;
        if write_level > profile.write_level || write_level > self.capability.write_level() {
            return Err(RuntimeError::Authorization);
        }
        let safe_sql = validate_for_level(sql, profile.engine, write_level)
            .map_err(|error| RuntimeError::Policy(error.to_string()))?;
        let entry = self.ensure_pool(stored_alias, profile).await?;
        let timeout = Duration::from_secs(
            profile
                .limits
                .statement_timeout_seconds
                .saturating_add(QUERY_TIMEOUT_GRACE_SECONDS),
        );
        with_wall_clock_timeout(
            timeout,
            entry.pool.execute(
                stored_alias,
                profile,
                &safe_sql,
                &entry.password,
                write_level,
            ),
        )
        .await
        .map_err(RuntimeError::Gate)?
        .map_err(Into::into)
    }

    fn schema_get(&self, alias: &str) -> Result<Vec<SchemaObject>, RuntimeError> {
        let registry = Registry::load(&self.private_dir.join("connections.json"))?;
        let (stored_alias, profile) = registry.resolve(Some(alias))?;
        let fingerprint = profile_fingerprint(profile)?;
        self.cache
            .get_snapshot(stored_alias, &fingerprint)
            .map_err(Into::into)
    }

    async fn ensure_pool(
        &self,
        alias: &str,
        profile: &ConnectionProfile,
    ) -> Result<PoolEntry, RuntimeError> {
        let fingerprint = profile_fingerprint(profile)?;
        let existing = { self.pools.lock().await.get(alias).cloned() };
        if let Some(existing) = existing
            && existing.fingerprint == fingerprint
        {
            return Ok(existing);
        }

        let password = read_password_from(&self.private_dir, alias)?;
        let pool = PoolHandle::connect(profile, &password).await?;
        let new_entry = PoolEntry {
            fingerprint,
            pool,
            password,
        };
        let old = self
            .pools
            .lock()
            .await
            .insert(alias.to_owned(), new_entry.clone());
        if let Some(old) = old {
            old.pool.close().await;
        }
        self.lifecycle
            .lock()
            .expect("lifecycle mutex poisoned")
            .record_pool_opened(self.started.elapsed());
        Ok(new_entry)
    }

    fn record_activity(&self) {
        self.lifecycle
            .lock()
            .expect("lifecycle mutex poisoned")
            .record_activity(self.started.elapsed());
    }

    fn state_name(&self) -> &'static str {
        match self
            .lifecycle
            .lock()
            .expect("lifecycle mutex poisoned")
            .state()
        {
            DaemonState::Warm => "warm",
            DaemonState::Hot => "hot",
        }
    }

    fn idle_seconds(&self) -> u64 {
        self.lifecycle
            .lock()
            .expect("lifecycle mutex poisoned")
            .idle_for(self.started.elapsed())
            .as_secs()
    }

    fn record_audit(
        &self,
        response: &Response,
        operation: String,
        connection: Option<String>,
        elapsed: Duration,
    ) {
        let Some(audit) = &self.audit else {
            return;
        };
        audit.record(AuditEvent {
            request_id: response.request_id.clone(),
            operation,
            connection,
            outcome: response
                .error
                .as_ref()
                .map(|error| error.code.clone())
                .unwrap_or_else(|| "OK".to_owned()),
            elapsed_micros: elapsed.as_micros().min(u64::MAX as u128) as u64,
        });
    }
}

fn operation_metadata(operation: &Operation) -> (String, Option<String>) {
    match operation {
        Operation::Query { connection, .. } => ("query".to_owned(), Some(connection.clone())),
        Operation::SchemaGet { connection } => ("schema_get".to_owned(), Some(connection.clone())),
        Operation::Status => ("status".to_owned(), None),
    }
}

fn requested_connection(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::Query { connection, .. } | Operation::SchemaGet { connection } => {
            Some(connection)
        }
        Operation::Status => None,
    }
}

#[derive(Debug, thiserror::Error)]
enum RuntimeError {
    #[error("query capability is invalid")]
    Authentication,
    #[error("query capability is not authorized for this connection")]
    Authorization,
    #[error("{0}")]
    Registry(#[from] crate::registry::RegistryError),
    #[error("{0}")]
    Database(#[from] DatabaseError),
    #[error("{0}")]
    Cache(#[from] CacheError),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Policy(String),
    #[error("query guard rejected request: {0:?}")]
    Gate(GateError),
}

pub async fn run_daemon() -> anyhow::Result<()> {
    run_daemon_at(
        config_dir(),
        endpoint_name(),
        std::env::var("DB_OPERATOR_PIPE_SDDL").ok(),
    )
    .await
}

pub async fn run_daemon_at(
    private_dir: PathBuf,
    endpoint: String,
    pipe_sddl: Option<String>,
) -> anyhow::Result<()> {
    fs::create_dir_all(&private_dir)?;
    validate_private_directory(&private_dir)?;
    let capability = load_capability_verifier(&private_dir.join("capability.json"))?;
    let audit = AuditLogger::start(private_dir.join("audit.jsonl"))?;
    let runtime = DaemonRuntime::new_with_audit(
        private_dir.join("schema-cache.sqlite3"),
        Duration::from_secs(60),
        capability,
        Some(audit),
    )?;
    let result = run_server(runtime.clone(), &endpoint, pipe_sddl.as_deref()).await;
    runtime.close_pools().await;
    result
}

pub async fn run_daemon_at_with_shutdown(
    private_dir: PathBuf,
    endpoint: String,
    pipe_sddl: Option<String>,
    mut external_shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    fs::create_dir_all(&private_dir)?;
    validate_private_directory(&private_dir)?;
    let capability = load_capability_verifier(&private_dir.join("capability.json"))?;
    let audit = AuditLogger::start(private_dir.join("audit.jsonl"))?;
    let runtime = DaemonRuntime::new_with_audit(
        private_dir.join("schema-cache.sqlite3"),
        Duration::from_secs(60),
        capability,
        Some(audit),
    )?;
    let shutdown_runtime = runtime.clone();
    tokio::spawn(async move {
        if external_shutdown.changed().await.is_ok() && *external_shutdown.borrow() {
            shutdown_runtime.request_shutdown();
        }
    });
    let result = run_server(runtime.clone(), &endpoint, pipe_sddl.as_deref()).await;
    runtime.close_pools().await;
    result
}

#[cfg(unix)]
async fn run_server(
    runtime: Arc<DaemonRuntime>,
    endpoint: &str,
    _pipe_sddl: Option<&str>,
) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::net::UnixListener;

    let endpoint = PathBuf::from(endpoint);
    if endpoint.exists() {
        let _ = fs::remove_file(&endpoint);
    }
    let listener = UnixListener::bind(&endpoint)?;
    fs::set_permissions(&endpoint, fs::Permissions::from_mode(UNIX_SOCKET_MODE))?;
    let mut shutdown = runtime.subscribe_shutdown();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let runtime = runtime.clone();
                tokio::spawn(async move { let _ = handle_stream(stream, runtime).await; });
            }
            _ = interval.tick() => {
                if runtime.maintenance_tick().await { break; }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
        }
    }
    let _ = fs::remove_file(endpoint);
    Ok(())
}

#[cfg(windows)]
async fn run_server(
    runtime: Arc<DaemonRuntime>,
    endpoint: &str,
    pipe_sddl: Option<&str>,
) -> anyhow::Result<()> {
    use crate::ipc::create_secure_pipe_server;

    let sddl = pipe_sddl.ok_or_else(|| anyhow::anyhow!("--pipe-sddl is required on Windows"))?;
    let mut server = create_secure_pipe_server(endpoint, true, sddl)?;
    let mut shutdown = runtime.subscribe_shutdown();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            connected = server.connect() => {
                connected?;
                let connected_server = server;
                server = create_secure_pipe_server(endpoint, false, sddl)?;
                let runtime = runtime.clone();
                tokio::spawn(async move { let _ = handle_stream(connected_server, runtime).await; });
            }
            _ = interval.tick() => {
                if runtime.maintenance_tick().await { break; }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
        }
    }
    Ok(())
}

async fn handle_stream<S>(stream: S, runtime: Arc<DaemonRuntime>) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let frame = read_frame(reader).await?;
    let response = match decode_request(&frame) {
        Ok(request) => runtime.process(request).await,
        Err(error) => error_response(
            "unknown".to_owned(),
            RuntimeError::Policy(error.to_string()),
        ),
    };
    writer.write_all(&encode_response(&response)?).await?;
    writer.flush().await?;
    Ok(())
}

fn error_response(request_id: String, error: RuntimeError) -> Response {
    let code = match &error {
        RuntimeError::Authentication => "AUTHENTICATION_FAILED",
        RuntimeError::Authorization => "AUTHORIZATION_FAILED",
        RuntimeError::Registry(_) => "REGISTRY_ERROR",
        RuntimeError::Database(DatabaseError::Credential(_)) => "CREDENTIAL_ERROR",
        RuntimeError::Database(DatabaseError::Connection(_)) => "CONNECTION_ERROR",
        RuntimeError::Database(DatabaseError::Session(_)) => "SESSION_ERROR",
        RuntimeError::Database(DatabaseError::Query(_)) => "QUERY_ERROR",
        RuntimeError::Database(DatabaseError::Policy(_)) | RuntimeError::Policy(_) => {
            "SQL_POLICY_REJECTED"
        }
        RuntimeError::Database(DatabaseError::Fingerprint(_)) => "PROFILE_ERROR",
        RuntimeError::Gate(GateError::ConcurrencyLimited) => "CONCURRENCY_LIMITED",
        RuntimeError::Gate(GateError::RateLimited) => "RATE_LIMITED",
        RuntimeError::Gate(GateError::TimedOut) => "QUERY_TIMEOUT",
        RuntimeError::Cache(_) => "SCHEMA_CACHE_ERROR",
        RuntimeError::Json(_) => "SERIALIZATION_ERROR",
    };
    Response {
        version: PROTOCOL_VERSION,
        request_id,
        ok: false,
        result: None,
        error: Some(ErrorPayload {
            code: code.to_owned(),
            message: error.to_string(),
        }),
        runtime: None,
    }
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, RuntimeError> {
    serde_json::to_value(value).map_err(Into::into)
}
