use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Duration;

use ae_sdd_contracts::diagnostics::{BugKind, DIAGNOSTICS_DIR};
use ae_sdd_integrations::{
    DaemonLock, FileWorkspaceResolver, LocalIpcServer, NativeBusinessAdapter, RuntimePaths,
    SqliteRuntimePersistence, SystemClock, publish_endpoint_manifest,
};
use ae_sdd_protocol::{
    ENDPOINT_MANIFEST_SCHEMA_V1, EndpointManifest, PROTOCOL_RANGE_V1, SecretString,
};
use ae_sdd_runtime::{
    BusinessOperationPort, ClockPort, DaemonLifecycle, PersistencePort, RuntimeConfig,
    RuntimeService, WorkspaceResolverPort, diagnostics,
};
use clap::{Parser, Subcommand};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "ae-sddd", version, about = "ae-sdd per-user Rust daemon")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the protected local Named Pipe or Unix Domain Socket.
    Serve {
        /// Per-user daemon state directory.
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Canonical parent roots from which workspaces may be registered.
        #[arg(long, required = true)]
        allowed_root: Vec<PathBuf>,
        /// Current 64-character lowercase policy digest.
        #[arg(long)]
        policy_digest: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let result = match Arguments::parse().command {
        Command::Serve {
            state_dir,
            allowed_root,
            policy_digest,
        } => serve(state_dir, allowed_root, policy_digest).await,
    };
    if let Err(error) = result {
        eprintln!("ae-sddd: {error}");
        std::process::exit(1);
    }
}

async fn serve(
    state_dir: Option<PathBuf>,
    allowed_roots: Vec<PathBuf>,
    policy_digest: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths = match state_dir {
        Some(path) => RuntimePaths::from_state_dir(path),
        None => RuntimePaths::per_user_default()?,
    };
    init_tracing(paths.log_file.clone());
    diagnostics::init(paths.state_dir.join(DIAGNOSTICS_DIR));
    install_panic_hook();
    tracing::info!(state_dir = %paths.state_dir.display(), "daemon starting");
    let startup_paths = paths.clone();
    let startup_roots = allowed_roots;
    let (_lock, sqlite, resolver, server) = tokio::task::spawn_blocking(move || {
        let lock = DaemonLock::acquire(&startup_paths).map_err(|error| error.to_string())?;
        startup_paths
            .remove_stale_local_endpoint()
            .map_err(|error| error.to_string())?;
        let sqlite = Arc::new(
            SqliteRuntimePersistence::open(&startup_paths.database)
                .map_err(|error| error.to_string())?,
        );
        sqlite.integrity_check().map_err(|error| {
            // Recorded before the startup error propagates: this failure aborts
            // the daemon, so without its own line the ops track would show only
            // an unexplained absence of a daemon.
            diagnostics::emit_bug(
                BugKind::Store,
                "bins/ae-sdd-daemon/src/main.rs",
                &error.to_string(),
                Vec::new(),
                diagnostics::BugIds::default(),
            );
            diagnostics::flush(Duration::from_millis(500));
            error.to_string()
        })?;
        let resolver =
            Arc::new(FileWorkspaceResolver::new(startup_roots).map_err(|error| error.to_string())?);
        let server = LocalIpcServer::bind(&startup_paths.endpoint, 128, Duration::from_secs(30))
            .map_err(|error| error.to_string())?;
        Ok::<_, String>((lock, sqlite, resolver, server))
    })
    .await
    .map_err(|_| "daemon startup blocking task failed")??;
    let event_store_id = sqlite.event_store_id()?;
    let clock: Arc<dyn ClockPort> = Arc::new(SystemClock);
    let resolver: Arc<dyn WorkspaceResolverPort> = resolver;
    let mut config = RuntimeConfig::default();
    if let Some(digest) = policy_digest {
        if digest.len() != 64
            || digest
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err("policy digest must be 64 lowercase hexadecimal characters".into());
        }
        config.policy_digest = digest;
    }
    let boot_id = ae_sdd_domain::BootId::from_uuid(Uuid::new_v4());
    let persistence: Arc<dyn PersistencePort> = sqlite.clone();
    let business: Arc<dyn BusinessOperationPort> = Arc::new(NativeBusinessAdapter::new(
        paths.database.clone(),
        event_store_id,
        boot_id,
        config.policy_digest.clone(),
        Arc::clone(&persistence),
    ));
    let endpoint_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let runtime = Arc::new(RuntimeService::new(
        config,
        boot_id,
        endpoint_token.clone(),
        persistence,
        clock,
        resolver,
        business,
    ));
    let recover_runtime = Arc::clone(&runtime);
    tokio::task::spawn_blocking(move || recover_runtime.recover())
        .await
        .map_err(|_| "runtime recovery task failed")??;
    let (capability_key_id, capability_public_key) = runtime.capability_key();
    let manifest = EndpointManifest {
        schema_version: ENDPOINT_MANIFEST_SCHEMA_V1.to_owned(),
        pid: std::process::id(),
        boot_id: boot_id.to_string(),
        event_store_id: runtime.event_store_id()?.to_string(),
        endpoint: paths.endpoint.clone(),
        endpoint_token: SecretString::new(endpoint_token),
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        daemon_version: ae_sdd_runtime::RUNTIME_BUILD.to_owned(),
        policy_digest: runtime.policy_digest().to_owned(),
        capability_key_id,
        capability_public_key,
        started_at: jiff::Timestamp::now().to_string(),
    };
    let publish_paths = paths.clone();
    tokio::task::spawn_blocking(move || publish_endpoint_manifest(&publish_paths, &manifest))
        .await
        .map_err(|_| "endpoint manifest publish task failed")??;
    tracing::info!(
        boot_id = %boot_id,
        event_store_id = %runtime.event_store_id()?,
        "daemon ready"
    );

    let handler_runtime = Arc::clone(&runtime);
    let handler = Arc::new(
        move |connection: &mut ae_sdd_runtime::ConnectionState, payload: &[u8]| {
            handler_runtime.handle_payload(connection, payload)
        },
    );
    let job_runtime = Arc::clone(&runtime);
    let job_worker = tokio::spawn(async move {
        loop {
            let status_runtime = Arc::clone(&job_runtime);
            let lifecycle = tokio::task::spawn_blocking(move || status_runtime.status())
                .await
                .map_err(|_| "job worker status task failed".to_owned())?
                .map_err(|error| error.to_string())?
                .lifecycle;
            match lifecycle {
                DaemonLifecycle::Stopping => return Ok::<(), String>(()),
                DaemonLifecycle::Draining => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                }
                DaemonLifecycle::Running => {}
            }
            let worker_runtime = Arc::clone(&job_runtime);
            let ran = tokio::task::spawn_blocking(move || worker_runtime.run_one_pending_job())
                .await
                .map_err(|_| "job worker blocking task failed".to_owned())?
                .map_err(|error| error.to_string())?;
            if !ran {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    });
    let context_runtime = Arc::clone(&runtime);
    let context_worker = tokio::spawn(async move {
        loop {
            let status_runtime = Arc::clone(&context_runtime);
            let lifecycle = tokio::task::spawn_blocking(move || status_runtime.status())
                .await
                .map_err(|_| "context worker status task failed".to_owned())?
                .map_err(|error| error.to_string())?
                .lifecycle;
            match lifecycle {
                DaemonLifecycle::Stopping => return Ok::<(), String>(()),
                DaemonLifecycle::Draining => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
                DaemonLifecycle::Running => {}
            }
            let refresh_runtime = Arc::clone(&context_runtime);
            let refreshed =
                tokio::task::spawn_blocking(move || refresh_runtime.refresh_active_contexts())
                    .await
                    .map_err(|_| "context refresh blocking task failed".to_owned())?
                    .map_err(|error| error.to_string())?;
            tracing::trace!(refreshed, "active context projections refreshed");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    let mut lifecycle_tick = tokio::time::interval(Duration::from_millis(25));
    loop {
        tokio::select! {
            accepted = server.accept_and_spawn(Arc::clone(&handler)) => accepted?,
            _ = lifecycle_tick.tick() => {
                let status_runtime = Arc::clone(&runtime);
                let lifecycle = tokio::task::spawn_blocking(move || {
                    status_runtime.status().map(|status| status.lifecycle)
                })
                .await
                .map_err(|_| "runtime status task failed")??;
                if lifecycle == DaemonLifecycle::Stopping {
                    break;
                }
            }
        }
    }
    if !server.wait_for_idle(Duration::from_secs(5)).await {
        tracing::warn!(
            active_connections = server.active_connections(),
            "daemon drain deadline expired"
        );
    }
    match tokio::time::timeout(Duration::from_secs(5), job_worker).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => {
            tracing::warn!(error = %error, "job worker stopped with an error");
            record_worker_bug("job worker stopped with an error", &error);
        }
        Ok(Err(_)) => {
            tracing::warn!("job worker task failed");
            record_worker_bug("job worker task failed", "join error");
        }
        Err(_) => {
            tracing::warn!("job worker drain deadline expired");
            record_worker_bug("job worker drain deadline expired", "timeout");
        }
    }
    match tokio::time::timeout(Duration::from_secs(5), context_worker).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => {
            tracing::warn!(error = %error, "context worker stopped with an error");
            record_worker_bug("context worker stopped with an error", &error);
        }
        Ok(Err(_)) => {
            tracing::warn!("context worker task failed");
            record_worker_bug("context worker task failed", "join error");
        }
        Err(_) => {
            tracing::warn!("context worker drain deadline expired");
            record_worker_bug("context worker drain deadline expired", "timeout");
        }
    }
    drop(server);
    let manifest_path = paths.endpoint_manifest.clone();
    tokio::task::spawn_blocking(move || match std::fs::remove_file(&manifest_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    })
    .await
    .map_err(|_| "endpoint cleanup task failed")??;
    tracing::info!(boot_id = %boot_id, "daemon stopped");
    // Drain before returning: the queued tail is exactly the run-up to shutdown,
    // which is the part worth having when the shutdown itself is the problem.
    diagnostics::flush(Duration::from_secs(2));
    Ok(())
}

/// Records a background worker failure on the diagnostic ops track.
///
/// Worker failures are genuine defects rather than policy outcomes, so unlike a
/// denied operation they belong in the defect stream.
fn record_worker_bug(message: &str, detail: &str) {
    diagnostics::emit_bug(
        BugKind::Worker,
        "bins/ae-sdd-daemon/src/main.rs",
        message,
        vec![detail.to_owned()],
        diagnostics::BugIds::default(),
    );
}

/// Records a panic to the diagnostic ops track before the process unwinds.
///
/// The release profile aborts on panic, so an asynchronous write would usually
/// lose the record: the process is gone before the writer thread runs.  This
/// hook therefore flushes synchronously under a short deadline — a panic is the
/// single most valuable defect record, and it would otherwise be the one most
/// reliably lost.  The default hook still runs, so stderr output is unchanged.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let site = info.location().map_or_else(
            || "unknown".to_owned(),
            |location| format!("{}:{}", location.file(), location.line()),
        );
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic with a non-string payload".to_owned());
        diagnostics::emit_bug(
            BugKind::Panic,
            &site,
            &message,
            Vec::new(),
            diagnostics::BugIds::default(),
        );
        diagnostics::flush(Duration::from_millis(500));
        default_hook(info);
    }));
}

#[derive(Clone)]
struct NonBlockingLogWriter {
    sender: SyncSender<Vec<u8>>,
}

impl Write for NonBlockingLogWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let _ = self.sender.try_send(buffer.to_vec());
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn init_tracing(log_file: PathBuf) {
    const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
    let (sender, receiver) = sync_channel::<Vec<u8>>(1_024);
    let _ = std::thread::Builder::new()
        .name("ae-sdd-log-writer".to_owned())
        .spawn(move || {
            if let Some(parent) = log_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut file = open_log(&log_file, false).ok();
            let mut bytes = file
                .as_ref()
                .and_then(|handle| handle.metadata().ok())
                .map_or(0, |metadata| metadata.len());
            while let Ok(message) = receiver.recv() {
                if bytes.saturating_add(message.len() as u64) > MAX_LOG_BYTES {
                    // Rotate rather than truncate: truncating discarded the whole
                    // lifecycle history every time the file filled, so the log was
                    // reliably empty of anything that happened before the last
                    // couple of megabytes. The handle is closed first because
                    // Windows refuses to rename a file that is still open.
                    drop(file.take());
                    let previous = log_file.with_extension("log.1");
                    let _ = std::fs::rename(&log_file, &previous);
                    file = open_log(&log_file, false).ok();
                    bytes = 0;
                }
                if let Some(handle) = file.as_mut()
                    && handle.write_all(&message).is_ok()
                {
                    bytes = bytes.saturating_add(message.len() as u64);
                    let _ = handle.flush();
                }
            }
        });
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .with_writer(move || NonBlockingLogWriter {
            sender: sender.clone(),
        })
        .try_init();
}

fn open_log(path: &std::path::Path, truncate: bool) -> std::io::Result<std::fs::File> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .append(!truncate)
        .truncate(truncate)
        .open(path)
}
