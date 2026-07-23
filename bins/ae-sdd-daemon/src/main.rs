use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ae_sdd_integrations::{
    DaemonLock, FileWorkspaceResolver, LocalIpcServer, RuntimePaths, SqliteRuntimePersistence,
    SystemClock, publish_endpoint_manifest,
};
use ae_sdd_protocol::{
    ENDPOINT_MANIFEST_SCHEMA_V1, EndpointManifest, PROTOCOL_RANGE_V1, SecretString,
};
use ae_sdd_runtime::{
    BusinessOperationPort, ClockPort, DaemonLifecycle, PersistencePort, RejectingBusinessPort,
    RuntimeConfig, RuntimeService, WorkspaceResolverPort,
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

fn main() {
    let result = match Arguments::parse().command {
        Command::Serve {
            state_dir,
            allowed_root,
            policy_digest,
        } => serve(state_dir, allowed_root, policy_digest),
    };
    if let Err(error) = result {
        eprintln!("ae-sddd: {error}");
        std::process::exit(1);
    }
}

fn serve(
    state_dir: Option<PathBuf>,
    allowed_roots: Vec<PathBuf>,
    policy_digest: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths = match state_dir {
        Some(path) => RuntimePaths::from_state_dir(path),
        None => RuntimePaths::per_user_default()?,
    };
    let _lock = DaemonLock::acquire(&paths)?;
    paths.remove_stale_local_endpoint()?;
    append_log(&paths, "daemon.starting");

    let sqlite = Arc::new(SqliteRuntimePersistence::open(&paths.database)?);
    sqlite.integrity_check()?;
    let persistence: Arc<dyn PersistencePort> = sqlite;
    let clock: Arc<dyn ClockPort> = Arc::new(SystemClock);
    let resolver: Arc<dyn WorkspaceResolverPort> =
        Arc::new(FileWorkspaceResolver::new(allowed_roots)?);
    let business: Arc<dyn BusinessOperationPort> = Arc::new(RejectingBusinessPort);
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
    let server = LocalIpcServer::bind(&paths.endpoint, 128, Duration::from_secs(30))?;
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
    publish_endpoint_manifest(&paths, &manifest)?;
    append_log(&paths, "daemon.ready");

    let handler_runtime = Arc::clone(&runtime);
    let handler = Arc::new(
        move |connection: &mut ae_sdd_runtime::ConnectionState, payload: &[u8]| {
            handler_runtime.handle_payload(connection, payload)
        },
    );
    loop {
        server.accept_pending(Arc::clone(&handler))?;
        if runtime.status()?.lifecycle == DaemonLifecycle::Stopping {
            let drain_started = Instant::now();
            while server.active_connections() > 0
                && drain_started.elapsed() < Duration::from_secs(5)
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(server);
    if paths.endpoint_manifest.exists() {
        fs::remove_file(&paths.endpoint_manifest)?;
    }
    append_log(&paths, "daemon.stopped");
    Ok(())
}

fn append_log(paths: &RuntimePaths, event: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
    {
        let _ = writeln!(file, "{} {event}", jiff::Timestamp::now());
    }
}
