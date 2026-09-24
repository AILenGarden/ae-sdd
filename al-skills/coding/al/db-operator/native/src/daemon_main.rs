use std::path::PathBuf;

use clap::Parser;
use db_operator_native::daemon::run_daemon_at;
#[cfg(windows)]
use db_operator_native::daemon::run_daemon_at_with_shutdown;
use db_operator_native::ipc::endpoint_name;
use db_operator_native::registry::config_dir;

#[derive(Debug, Parser)]
#[command(
    name = "db-operator-daemon",
    version,
    about = "Credential-isolated DB Operator security daemon"
)]
struct Cli {
    /// Run under the Windows Service Control Manager.
    #[arg(long, hide = true)]
    service: bool,
    /// Daemon-private registry, vault, capability digest, and cache directory.
    #[arg(long, value_name = "DIR")]
    home: Option<PathBuf>,
    /// Service-scoped Named Pipe or Unix Domain Socket endpoint.
    #[arg(long)]
    endpoint: Option<String>,
    /// Protected Windows Named Pipe DACL in SDDL form.
    #[arg(long)]
    pipe_sddl: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    #[cfg(windows)]
    if cli.service {
        if let Err(error) =
            windows_service::service_dispatcher::start(WINDOWS_SERVICE_NAME, ffi_service_main)
        {
            eprintln!("ERROR: {error:#}");
            std::process::exit(1);
        }
        return;
    }

    let runtime = tokio::runtime::Runtime::new().expect("failed to create Tokio runtime");
    if let Err(error) = runtime.block_on(run_daemon_at(
        cli.home.unwrap_or_else(config_dir),
        cli.endpoint.unwrap_or_else(endpoint_name),
        cli.pipe_sddl,
    )) {
        eprintln!("ERROR: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
const WINDOWS_SERVICE_NAME: &str = "db-operator";

#[cfg(windows)]
windows_service::define_windows_service!(ffi_service_main, windows_service_main);

#[cfg(windows)]
fn windows_service_main(_arguments: Vec<std::ffi::OsString>) {
    write_windows_service_trace("service_main entered");
    if let Err(error) = run_windows_service() {
        write_windows_service_error(&error);
        eprintln!("ERROR: {error:#}");
    } else {
        write_windows_service_trace("service_main completed successfully");
    }
}

#[cfg(windows)]
fn write_windows_service_error(error: &anyhow::Error) {
    write_windows_service_trace(&format!("service error: {error:#}"));
}

#[cfg(windows)]
fn write_windows_service_trace(message: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;

    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let home = args
        .windows(2)
        .find(|pair| pair[0] == "--home")
        .map(|pair| PathBuf::from(&pair[1]))
        .unwrap_or_else(config_dir);
    if let Ok(mut log) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("service-trace.log"))
    {
        let _ = writeln!(log, "{message}");
    }
}

#[cfg(windows)]
fn run_windows_service() -> anyhow::Result<()> {
    use std::time::Duration;
    use tokio::sync::watch;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let event_handler = move |control_event| {
        write_windows_service_trace(&format!("control event: {control_event:?}"));
        match control_event {
            ServiceControl::Stop => {
                let _ = shutdown_tx.send(true);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    let status_handle = service_control_handler::register(WINDOWS_SERVICE_NAME, event_handler)?;
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 1,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    })?;

    let cli = Cli::parse();
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    })?;

    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(run_daemon_at_with_shutdown(
        cli.home.unwrap_or_else(config_dir),
        cli.endpoint.unwrap_or_else(endpoint_name),
        cli.pipe_sddl,
        shutdown_rx,
    ));
    write_windows_service_trace(&format!("daemon returned: {result:?}"));
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: if result.is_ok() {
            ServiceExitCode::Win32(0)
        } else {
            ServiceExitCode::ServiceSpecific(1)
        },
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    })?;
    result
}
