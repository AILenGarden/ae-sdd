use std::future::Future;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{DaemonClient, LocalIpcTransport};
use ae_sdd_protocol::ClientKind;
use serde::Serialize;
use serde_json::Value;
use sysinfo::{Pid, ProcessesToUpdate, System};
use thiserror::Error;

mod setup;

use setup::{
    BenchmarkWorkspace, cached_context_read, cached_hook_call, hook_context_digest,
    invalidated_hook_call, prepare_cached_hook, validate_controlled_hook, warm_handshake_call,
};

const MAX_HOOK_P95_MICROS: u64 = 50_000;
/// `constraints/testing.md` warm handshake p95 budget.
const MAX_HANDSHAKE_P95_MICROS: u64 = 50_000;
/// `constraints/testing.md` cached read p95 budget.
const MAX_CACHED_READ_P95_MICROS: u64 = 100_000;
/// `constraints/testing.md` invalidated non-external Hook p95 budget.
const MAX_INVALIDATED_HOOK_P95_MICROS: u64 = 250_000;
/// Sample count for the handshake and cached-read probes. Small relative to the
/// Hook loop because each sample opens a fresh connection.
const HANDSHAKE_SAMPLES: u64 = 200;
const CACHED_READ_SAMPLES: u64 = 200;
/// Sample count for the invalidated probe. Each sample rewrites the on-disk
/// state and waits for the daemon context worker to reproject, so samples cost
/// at least one worker period each and cannot be scaled like the cached loop.
const INVALIDATED_SAMPLES: u64 = 40;
/// Bound on context-worker polls per invalidated sample. The daemon worker
/// period is 100 ms; this tolerates a slow reprojection without hanging.
const INVALIDATED_POLL_LIMIT: u32 = 60;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const RPC_TIMEOUT: Duration = Duration::from_secs(2);
const HOOK_DEADLINE_MS: u64 = 250;
const WORK_ITEM_ID: &str = "BENCHMARK-HOOK-001";
const CANARY_INVENTORY_GENERATION: u64 = 2;

thread_local! {
    // Windows named-pipe driver teardown can wait forever after the owned
    // daemon has already been terminated. This is a short-lived build tool,
    // so let the OS reclaim the runtime handles at process exit.
    static BENCHMARK_RUNTIME: ManuallyDrop<tokio::runtime::Runtime> = ManuallyDrop::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("benchmark Tokio runtime")
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookBenchmarkConfig {
    warmup: u64,
    samples: u64,
    histogram: Box<str>,
    manifest_path: Option<PathBuf>,
    workspace_root: PathBuf,
    daemon_binary: Option<PathBuf>,
}

impl HookBenchmarkConfig {
    pub fn new(warmup: u64, samples: u64, histogram: &str) -> Result<Self, BenchmarkError> {
        if samples == 0 || samples > 10_000_000 || warmup > 10_000_000 {
            return Err(BenchmarkError::InvalidSampleCount);
        }
        if histogram != "hdr" {
            return Err(BenchmarkError::UnsupportedHistogram(histogram.to_owned()));
        }
        Ok(Self {
            warmup,
            samples,
            histogram: histogram.into(),
            manifest_path: None,
            workspace_root: std::env::current_dir().map_err(BenchmarkError::Io)?,
            daemon_binary: None,
        })
    }

    #[must_use]
    pub fn with_manifest(mut self, manifest_path: PathBuf) -> Self {
        self.manifest_path = Some(manifest_path);
        self
    }

    #[must_use]
    pub fn with_workspace_root(mut self, workspace_root: PathBuf) -> Self {
        self.workspace_root = workspace_root;
        self
    }

    #[must_use]
    pub fn with_daemon_binary(mut self, daemon_binary: PathBuf) -> Self {
        self.daemon_binary = Some(daemon_binary);
        self
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookBenchmarkSummary {
    pub schema_version: &'static str,
    pub benchmark: &'static str,
    pub build_profile: &'static str,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub transport: &'static str,
    pub daemon_pid: u32,
    pub histogram: Box<str>,
    pub warmup: u64,
    pub samples: u64,
    pub p50_micros: u64,
    pub p95_micros: u64,
    pub p99_micros: u64,
    pub max_micros: u64,
    /// Warm `runtime.status` p95, gated by `MAX_HANDSHAKE_P95_MICROS`.
    pub handshake_p95_micros: u64,
    pub handshake_samples: u64,
    /// Warm `context.get` p95, gated by `MAX_CACHED_READ_P95_MICROS`.
    pub cached_read_p95_micros: u64,
    pub cached_read_samples: u64,
    /// Invalidated `hook.user_prompt` p50/p95/max, gated by
    /// `MAX_INVALIDATED_HOOK_P95_MICROS`. Each sample is the round trip that
    /// first observed a reprojected body after an on-disk state move.
    pub invalidated_hook_p50_micros: u64,
    pub invalidated_hook_p95_micros: u64,
    pub invalidated_hook_max_micros: u64,
    pub invalidated_hook_samples: u64,
    pub elapsed_micros: u64,
    pub error_count: u64,
    pub receipt_replay_count: u64,
    pub engaged_replay_count: u64,
    pub allow_decision_count: u64,
    pub cpu_millis: u64,
    pub rss_bytes: u64,
}

pub fn benchmark_hook(config: HookBenchmarkConfig) -> Result<HookBenchmarkSummary, BenchmarkError> {
    if cfg!(debug_assertions) {
        return Err(BenchmarkError::ReleaseProfileRequired);
    }
    let workspace_root = config
        .workspace_root
        .canonicalize()
        .map_err(BenchmarkError::Io)?;
    let benchmark_workspace = BenchmarkWorkspace::create(&workspace_root)?;
    let daemon = BenchmarkDaemon::connect_or_spawn(&config, &workspace_root)?;
    let hook_client = DaemonClient::new(
        daemon.manifest_path.clone(),
        ClientKind::Hook,
        Arc::new(LocalIpcTransport),
        RPC_TIMEOUT,
    );
    let admin_client = DaemonClient::new(
        daemon.manifest_path.clone(),
        ClientKind::Admin,
        Arc::new(LocalIpcTransport),
        RPC_TIMEOUT,
    );
    let endpoint = block_on(hook_client.endpoint_manifest())?;
    let input_fingerprint =
        benchmark_workspace.write_authoritative_state(&endpoint.policy_digest)?;
    let session = prepare_cached_hook(
        &hook_client,
        &admin_client,
        benchmark_workspace.path(),
        &input_fingerprint,
        endpoint.started_at,
    )?;

    for _ in 0..config.warmup {
        let result = cached_hook_call(&hook_client, &session)?;
        validate_controlled_hook(&result, true)?;
    }

    let capacity =
        usize::try_from(config.samples).map_err(|_| BenchmarkError::InvalidSampleCount)?;
    let mut latencies = Vec::with_capacity(capacity);
    let mut error_count = 0_u64;
    let mut receipt_replay_count = 0_u64;
    let mut engaged_replay_count = 0_u64;
    let mut allow_decision_count = 0_u64;
    let mut metrics = ProcessMetrics::new(daemon.pid);
    let cpu_start = metrics.sample()?.cpu_millis;
    let mut peak_rss = metrics.sample()?.rss_bytes;
    let sample_stride = (config.samples / 100).max(1);
    let benchmark_started = Instant::now();
    for index in 0..config.samples {
        let started = Instant::now();
        match cached_hook_call(&hook_client, &session) {
            Ok(result) => {
                let replayed = result
                    .get("replayed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let engaged = result
                    .get("engaged")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let allowed = result.get("decision").and_then(Value::as_str) == Some("allow");
                receipt_replay_count += u64::from(replayed);
                engaged_replay_count += u64::from(engaged);
                allow_decision_count += u64::from(allowed);
                if !(replayed && engaged && allowed) {
                    error_count += 1;
                }
            }
            Err(_) => error_count += 1,
        }
        latencies.push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
        if index % sample_stride == 0 {
            peak_rss = peak_rss.max(metrics.sample()?.rss_bytes);
        }
    }
    let elapsed_micros = u64::try_from(benchmark_started.elapsed().as_micros()).unwrap_or(u64::MAX);

    // Warm handshake probe. Runs after the Hook loop so the daemon is fully
    // warm; a cold first connection would measure startup, not handshake.
    let mut handshake_latencies = Vec::with_capacity(
        usize::try_from(HANDSHAKE_SAMPLES).map_err(|_| BenchmarkError::InvalidSampleCount)?,
    );
    for _ in 0..HANDSHAKE_SAMPLES {
        let started = Instant::now();
        warm_handshake_call(&admin_client)?;
        handshake_latencies.push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
    }
    handshake_latencies.sort_unstable();
    let handshake_p95_micros = percentile(&handshake_latencies, 95);

    // Cached read probe. One priming call so the projection cache is populated,
    // then measure steady-state reads.
    cached_context_read(&hook_client, &session)?;
    let mut cached_read_latencies = Vec::with_capacity(
        usize::try_from(CACHED_READ_SAMPLES).map_err(|_| BenchmarkError::InvalidSampleCount)?,
    );
    for _ in 0..CACHED_READ_SAMPLES {
        let started = Instant::now();
        cached_context_read(&hook_client, &session)?;
        cached_read_latencies
            .push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
    }
    cached_read_latencies.sort_unstable();
    let cached_read_p95_micros = percentile(&cached_read_latencies, 95);

    // Invalidated probe. Runs last because it moves the on-disk state the
    // cached loop depends on.
    let mut invalidated_latencies = invalidated_hook_latencies(
        &hook_client,
        &session,
        &benchmark_workspace,
        &endpoint.policy_digest,
    )?;
    invalidated_latencies.sort_unstable();

    let final_metrics = metrics.sample()?;
    peak_rss = peak_rss.max(final_metrics.rss_bytes);
    latencies.sort_unstable();

    let summary = HookBenchmarkSummary {
        schema_version: "ae-sdd-hook-benchmark/v4",
        benchmark: "engaged-authoritative-cached-hook-rpc-receipt-replay",
        build_profile: "release",
        operating_system: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        transport: if cfg!(windows) {
            "named-pipe"
        } else {
            "unix-domain-socket"
        },
        daemon_pid: daemon.pid,
        histogram: config.histogram,
        warmup: config.warmup,
        samples: config.samples,
        p50_micros: percentile(&latencies, 50),
        p95_micros: percentile(&latencies, 95),
        p99_micros: percentile(&latencies, 99),
        handshake_p95_micros,
        handshake_samples: HANDSHAKE_SAMPLES,
        cached_read_p95_micros,
        cached_read_samples: CACHED_READ_SAMPLES,
        invalidated_hook_p50_micros: percentile(&invalidated_latencies, 50),
        invalidated_hook_p95_micros: percentile(&invalidated_latencies, 95),
        invalidated_hook_max_micros: invalidated_latencies.last().copied().unwrap_or_default(),
        invalidated_hook_samples: INVALIDATED_SAMPLES,
        max_micros: latencies.last().copied().unwrap_or_default(),
        elapsed_micros,
        error_count,
        receipt_replay_count,
        engaged_replay_count,
        allow_decision_count,
        cpu_millis: final_metrics.cpu_millis.saturating_sub(cpu_start),
        rss_bytes: peak_rss,
    };
    if summary.error_count != 0
        || summary.receipt_replay_count != summary.samples
        || summary.engaged_replay_count != summary.samples
        || summary.allow_decision_count != summary.samples
    {
        return Err(BenchmarkError::RoundTripFailures(summary.error_count));
    }
    if summary.p95_micros > MAX_HOOK_P95_MICROS {
        return Err(BenchmarkError::P95BudgetExceeded {
            actual_micros: summary.p95_micros,
            maximum_micros: MAX_HOOK_P95_MICROS,
        });
    }
    if summary.handshake_p95_micros > MAX_HANDSHAKE_P95_MICROS {
        return Err(BenchmarkError::HandshakeP95BudgetExceeded {
            actual_micros: summary.handshake_p95_micros,
            maximum_micros: MAX_HANDSHAKE_P95_MICROS,
        });
    }
    if summary.cached_read_p95_micros > MAX_CACHED_READ_P95_MICROS {
        return Err(BenchmarkError::CachedReadP95BudgetExceeded {
            actual_micros: summary.cached_read_p95_micros,
            maximum_micros: MAX_CACHED_READ_P95_MICROS,
        });
    }
    if summary.invalidated_hook_p95_micros > MAX_INVALIDATED_HOOK_P95_MICROS {
        return Err(BenchmarkError::P95BudgetExceeded {
            actual_micros: summary.invalidated_hook_p95_micros,
            maximum_micros: MAX_INVALIDATED_HOOK_P95_MICROS,
        });
    }
    Ok(summary)
}

/// Invalidated Hook probe. Each sample moves the on-disk authoritative state
/// to a fresh revision, then polls `hook.user_prompt` with unique event ids
/// until the engaged session reports the recomputed projection digest. The
/// recorded latency is the round trip that first observed the reprojected
/// body, per the `invalidated_hook_*` summary contract.
fn invalidated_hook_latencies(
    client: &DaemonClient,
    session: &setup::HookSession,
    workspace: &BenchmarkWorkspace,
    policy_digest: &str,
) -> Result<Vec<u64>, BenchmarkError> {
    let mut latencies = Vec::with_capacity(
        usize::try_from(INVALIDATED_SAMPLES).map_err(|_| BenchmarkError::InvalidSampleCount)?,
    );
    for sample in 0..INVALIDATED_SAMPLES {
        // The prepared state sits at revision 1, so the first move is 2.
        let fingerprint = workspace.write_authoritative_state_at(policy_digest, sample + 2)?;
        let mut observed = None;
        for poll in 0..INVALIDATED_POLL_LIMIT {
            let event_id = format!("benchmark-invalidated-{sample}-{poll}");
            let started = Instant::now();
            let result = invalidated_hook_call(client, session, &event_id)?;
            if hook_context_digest(&result) == Some(fingerprint.as_str()) {
                observed = Some(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        latencies.push(observed.ok_or(BenchmarkError::AuthoritativeContextMismatch)?);
    }
    Ok(latencies)
}

fn block_on<F: Future>(future: F) -> F::Output {
    BENCHMARK_RUNTIME.with(|runtime| runtime.block_on(future))
}

struct BenchmarkDaemon {
    pid: u32,
    manifest_path: PathBuf,
    owned: Option<OwnedDaemon>,
}

impl BenchmarkDaemon {
    fn connect_or_spawn(
        config: &HookBenchmarkConfig,
        workspace_root: &Path,
    ) -> Result<Self, BenchmarkError> {
        if let Some(manifest_path) = &config.manifest_path {
            let client = DaemonClient::new(
                manifest_path,
                ClientKind::Hook,
                Arc::new(LocalIpcTransport),
                RPC_TIMEOUT,
            );
            let manifest = block_on(client.endpoint_manifest())?;
            return Ok(Self {
                pid: manifest.pid,
                manifest_path: manifest_path.clone(),
                owned: None,
            });
        }

        let daemon_binary = config
            .daemon_binary
            .clone()
            .unwrap_or_else(default_daemon_binary);
        if !daemon_binary.is_file() {
            return Err(BenchmarkError::DaemonBinaryMissing(daemon_binary));
        }
        let state_dir = std::env::temp_dir().join(format!(
            "ae-sdd-hook-benchmark-{}-{}",
            std::process::id(),
            now_unix_ms()
        ));
        std::fs::create_dir(&state_dir).map_err(BenchmarkError::Io)?;
        let child = Command::new(&daemon_binary)
            .arg("serve")
            .arg("--state-dir")
            .arg(&state_dir)
            .arg("--allowed-root")
            .arg(workspace_root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(BenchmarkError::Io)?;
        let pid = child.id();
        let manifest_path = state_dir.join("endpoint.v1.json");
        let started = Instant::now();
        while !manifest_path.is_file() {
            if started.elapsed() >= STARTUP_TIMEOUT {
                return Err(BenchmarkError::DaemonStartTimeout);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(Self {
            pid,
            manifest_path,
            owned: Some(OwnedDaemon { child, state_dir }),
        })
    }
}

impl Drop for BenchmarkDaemon {
    fn drop(&mut self) {
        drop(self.owned.take());
    }
}

struct OwnedDaemon {
    child: Child,
    state_dir: PathBuf,
}

impl Drop for OwnedDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        for _ in 0..200 {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        let _ = std::fs::remove_dir_all(&self.state_dir);
    }
}

fn default_daemon_binary() -> PathBuf {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_default()
        .join(format!("ae-sddd{extension}"))
}

struct ProcessMetrics {
    pid: Pid,
    system: System,
}

struct MetricSample {
    cpu_millis: u64,
    rss_bytes: u64,
}

impl ProcessMetrics {
    fn new(pid: u32) -> Self {
        Self {
            pid: Pid::from_u32(pid),
            system: System::new(),
        }
    }

    fn sample(&mut self) -> Result<MetricSample, BenchmarkError> {
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&[self.pid]), true);
        let process = self
            .system
            .process(self.pid)
            .ok_or(BenchmarkError::DaemonExited)?;
        Ok(MetricSample {
            cpu_millis: process.accumulated_cpu_time(),
            rss_bytes: process.memory(),
        })
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let numerator = percentile.saturating_mul(sorted.len().saturating_sub(1));
    sorted[numerator.div_ceil(100)]
}

/// Measured execution-efficiency surface for one supervised resume loop.
///
/// The values are collected by the P0 process E2E (or, later, by release
/// telemetry): the wall time from `execution.resume` to the first admissible
/// patch, the full capsule size, the no-change response size, the authority
/// refresh count per resume, the consecutive no-progress batch peak and the
/// number of broad verifications executed before the focused GREEN.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionEfficiencyMetrics {
    /// Wall time from the first resume to the first patch event, in ms.
    pub resume_to_first_patch_ms: u64,
    /// Serialized bytes of the full `ExecutionCapsuleV1` projection.
    pub full_capsule_bytes: u64,
    /// Serialized bytes of the no-change resume response.
    pub no_change_response_bytes: u64,
    /// Authority refreshes observed for one resume call.
    pub authority_refresh_count: u64,
    /// Peak of consecutive investigation batches without machine progress.
    pub max_no_progress_batches: u64,
    /// Broad verifications executed before the focused GREEN.
    pub broad_before_green_count: u64,
}

/// P0 execution-efficiency gates (implementation plan §5).
///
/// The thresholds are the frozen P0 budget contract: the full capsule never
/// exceeds 16 KiB, a no-change response never exceeds 1 KiB, one resume
/// refreshes the authority exactly once, investigation stops after three
/// consecutive no-progress batches and no broad verification may execute
/// before the focused GREEN.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionEfficiencyThresholds {
    /// Maximum admitted resume-to-first-patch wall time in ms.
    pub max_resume_to_first_patch_ms: u64,
    /// Hard capsule byte ceiling.
    pub max_full_capsule_bytes: u64,
    /// Hard no-change response byte ceiling.
    pub max_no_change_response_bytes: u64,
    /// Maximum authority refreshes admitted per resume.
    pub max_authority_refresh_count: u64,
    /// Maximum consecutive no-progress investigation batches.
    pub max_no_progress_batches: u64,
    /// Admitted broad verifications before the focused GREEN.
    pub max_broad_before_green_count: u64,
}

/// Frozen P0 thresholds for the execution-efficiency benchmark surface.
pub const EXECUTION_EFFICIENCY_P0: ExecutionEfficiencyThresholds = ExecutionEfficiencyThresholds {
    max_resume_to_first_patch_ms: 300_000,
    max_full_capsule_bytes: 16 * 1024,
    max_no_change_response_bytes: 1024,
    max_authority_refresh_count: 1,
    max_no_progress_batches: 3,
    max_broad_before_green_count: 0,
};

/// Benchmark summary for one evaluated execution-efficiency surface.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEfficiencyBenchmarkSummary {
    /// Summary schema identity.
    pub schema_version: &'static str,
    /// Benchmark surface identity.
    pub benchmark: &'static str,
    /// Build profile the metrics were collected under.
    pub build_profile: &'static str,
    /// Operating system the metrics were collected on.
    pub operating_system: &'static str,
    /// CPU architecture the metrics were collected on.
    pub architecture: &'static str,
    /// Measured resume-to-first-patch wall time in ms.
    pub resume_to_first_patch_ms: u64,
    /// Measured full capsule bytes.
    pub full_capsule_bytes: u64,
    /// Measured no-change response bytes.
    pub no_change_response_bytes: u64,
    /// Measured authority refreshes per resume.
    pub authority_refresh_count: u64,
    /// Measured consecutive no-progress batch peak.
    pub max_no_progress_batches: u64,
    /// Measured broad verifications before the focused GREEN.
    pub broad_before_green_count: u64,
}

/// Evaluates one measured execution-efficiency surface against the frozen
/// P0 gates, returning the benchmark summary or the first gate violated.
///
/// The evaluation is a pure function: it performs no I/O and never reads a
/// clock, so debug-profile unit tests and release-profile telemetry share
/// the exact same gate semantics.
pub fn evaluate_execution_efficiency(
    metrics: &ExecutionEfficiencyMetrics,
) -> Result<ExecutionEfficiencyBenchmarkSummary, BenchmarkError> {
    let thresholds = EXECUTION_EFFICIENCY_P0;
    let gates = [
        (
            "resumeToFirstPatchMs",
            metrics.resume_to_first_patch_ms,
            thresholds.max_resume_to_first_patch_ms,
            "ms",
        ),
        (
            "fullCapsuleBytes",
            metrics.full_capsule_bytes,
            thresholds.max_full_capsule_bytes,
            "bytes",
        ),
        (
            "noChangeResponseBytes",
            metrics.no_change_response_bytes,
            thresholds.max_no_change_response_bytes,
            "bytes",
        ),
        (
            "authorityRefreshesPerResume",
            metrics.authority_refresh_count,
            thresholds.max_authority_refresh_count,
            "count",
        ),
        (
            "maxConsecutiveNoProgressBatches",
            metrics.max_no_progress_batches,
            thresholds.max_no_progress_batches,
            "count",
        ),
        (
            "broadTestsBeforeFocusedGreen",
            metrics.broad_before_green_count,
            thresholds.max_broad_before_green_count,
            "count",
        ),
    ];
    for (metric, actual, maximum, unit) in gates {
        if actual > maximum {
            return Err(BenchmarkError::EfficiencyGateExceeded {
                metric,
                actual,
                maximum,
                unit,
            });
        }
    }
    Ok(ExecutionEfficiencyBenchmarkSummary {
        schema_version: "ae-sdd-execution-efficiency-benchmark/v1",
        benchmark: "execution-efficiency-p0-supervised-resume",
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        operating_system: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        resume_to_first_patch_ms: metrics.resume_to_first_patch_ms,
        full_capsule_bytes: metrics.full_capsule_bytes,
        no_change_response_bytes: metrics.no_change_response_bytes,
        authority_refresh_count: metrics.authority_refresh_count,
        max_no_progress_batches: metrics.max_no_progress_batches,
        broad_before_green_count: metrics.broad_before_green_count,
    })
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[derive(Debug, Error)]
pub enum BenchmarkError {
    #[error("warmup/samples must be bounded and samples must be non-zero")]
    InvalidSampleCount,
    #[error("unsupported histogram {0}; expected hdr")]
    UnsupportedHistogram(String),
    #[error("benchmark-hook must run from a release build")]
    ReleaseProfileRequired,
    #[error("release daemon binary does not exist: {0}")]
    DaemonBinaryMissing(PathBuf),
    #[error("daemon endpoint did not become ready before timeout")]
    DaemonStartTimeout,
    #[error("daemon exited while benchmark metrics were sampled")]
    DaemonExited,
    #[error("daemon response is missing field {0}")]
    ResponseField(&'static str),
    #[error("cached Hook seed did not create a new receipt")]
    ReceiptSeedMissing,
    #[error("cached Hook call did not replay a durable receipt")]
    ReceiptReplayMissing,
    #[error("workspace did not follow the required shadow-to-canary transition")]
    WorkspaceModeMismatch,
    #[error("the invalidated session did not reopen as the same engaged session")]
    SessionCutoverMismatch,
    #[error("authoritative project context was not injected into the engaged Hook")]
    AuthoritativeContextMismatch,
    #[error("cached Hook response was not daemon-engaged")]
    HookControlMissing,
    #[error("cached Hook response did not preserve the authoritative allow decision")]
    HookDecisionMismatch,
    #[error("benchmark fixture path is outside the requested allowed root: {0}")]
    UnsafeFixturePath(PathBuf),
    #[error("live Hook RPC failed: {0}")]
    Client(#[from] ae_sdd_client::ClientError),
    #[error("live Hook benchmark JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("live Hook benchmark I/O failed: {0}")]
    Io(std::io::Error),
    #[error("hook benchmark recorded {0} round-trip errors")]
    RoundTripFailures(u64),
    #[error("hook benchmark p95 {actual_micros}us exceeds {maximum_micros}us")]
    P95BudgetExceeded {
        actual_micros: u64,
        maximum_micros: u64,
    },
    #[error("warm handshake p95 {actual_micros}us exceeds {maximum_micros}us")]
    HandshakeP95BudgetExceeded {
        actual_micros: u64,
        maximum_micros: u64,
    },
    #[error("cached read p95 {actual_micros}us exceeds {maximum_micros}us")]
    CachedReadP95BudgetExceeded {
        actual_micros: u64,
        maximum_micros: u64,
    },
    #[error(
        "execution-efficiency gate {metric} recorded {actual} {unit}, exceeding the {maximum} {unit} P0 gate"
    )]
    EfficiencyGateExceeded {
        metric: &'static str,
        actual: u64,
        maximum: u64,
        unit: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use ae_sdd_protocol::{decode_frame, encode_frame};
    use sha2::{Digest, Sha256};

    use super::setup::{BenchmarkParityEvidence, authoritative_state, parity_transition_payload};
    use super::*;

    #[test]
    fn framing_unit_benchmark_is_not_reported_as_hook_rpc_evidence() {
        let payload = br#"{"jsonrpc":"2.0","method":"hook.pre_tool"}"#;
        let frame = encode_frame(payload).expect("frame");
        assert_eq!(decode_frame(&frame).expect("decode"), payload);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 95), 5);
    }

    #[test]
    fn live_benchmark_refuses_debug_profile() {
        if cfg!(debug_assertions) {
            assert!(matches!(
                benchmark_hook(HookBenchmarkConfig::new(1, 1, "hdr").expect("config")),
                Err(BenchmarkError::ReleaseProfileRequired)
            ));
        }
    }

    #[test]
    fn default_manifest_helper_remains_available_for_external_daemon_mode() {
        let _ = ae_sdd_client::default_endpoint_manifest();
    }

    #[test]
    fn authoritative_fixture_fingerprint_excludes_the_guard() {
        let (mut state, fingerprint) = authoritative_state(&"b".repeat(64)).expect("state");
        state.as_object_mut().expect("object").remove("hookGuard");
        assert_eq!(
            fingerprint,
            hex::encode(Sha256::digest(
                serde_json::to_vec(&state).expect("canonical state")
            ))
        );
    }

    #[test]
    fn canary_parity_payload_carries_its_typed_digest() {
        let payload = parity_transition_payload().expect("payload");
        let parity: BenchmarkParityEvidence =
            serde_json::from_value(payload["parity"].clone()).expect("typed parity");
        let expected = hex::encode(Sha256::digest(
            serde_json::to_vec(&parity).expect("canonical parity"),
        ));
        assert_eq!(payload["parityDigest"].as_str(), Some(expected.as_str()));
    }

    fn golden_path_metrics() -> ExecutionEfficiencyMetrics {
        ExecutionEfficiencyMetrics {
            resume_to_first_patch_ms: 2_500,
            full_capsule_bytes: 4_096,
            no_change_response_bytes: 512,
            authority_refresh_count: 1,
            max_no_progress_batches: 3,
            broad_before_green_count: 0,
        }
    }

    #[test]
    fn execution_efficiency_p0_gates_accept_the_golden_path_sample() {
        let summary = evaluate_execution_efficiency(&golden_path_metrics()).expect("P0 gates pass");
        assert_eq!(
            summary.schema_version,
            "ae-sdd-execution-efficiency-benchmark/v1"
        );
        assert_eq!(
            summary.benchmark,
            "execution-efficiency-p0-supervised-resume"
        );
        assert_eq!(summary.full_capsule_bytes, 4_096);
        assert_eq!(summary.no_change_response_bytes, 512);
        assert_eq!(summary.authority_refresh_count, 1);
        assert_eq!(summary.broad_before_green_count, 0);
        let encoded = serde_json::to_value(&summary).expect("summary serializes");
        assert_eq!(encoded["resumeToFirstPatchMs"], 2_500);
        assert_eq!(encoded["maxNoProgressBatches"], 3);
    }

    #[test]
    fn execution_efficiency_p0_gates_reject_each_regression() {
        fn expect_gate_violation(
            metrics: ExecutionEfficiencyMetrics,
            metric: &'static str,
            actual: u64,
            maximum: u64,
            unit: &'static str,
        ) {
            match evaluate_execution_efficiency(&metrics) {
                Err(BenchmarkError::EfficiencyGateExceeded {
                    metric: violated,
                    actual: recorded,
                    maximum: gate,
                    unit: gate_unit,
                }) => {
                    assert_eq!(violated, metric);
                    assert_eq!(recorded, actual);
                    assert_eq!(gate, maximum);
                    assert_eq!(gate_unit, unit);
                }
                other => panic!("gate {metric} must reject {actual}: {other:?}"),
            }
        }

        let base = golden_path_metrics();
        expect_gate_violation(
            ExecutionEfficiencyMetrics {
                resume_to_first_patch_ms: 300_001,
                ..base
            },
            "resumeToFirstPatchMs",
            300_001,
            300_000,
            "ms",
        );
        expect_gate_violation(
            ExecutionEfficiencyMetrics {
                full_capsule_bytes: 16_385,
                ..base
            },
            "fullCapsuleBytes",
            16_385,
            16_384,
            "bytes",
        );
        expect_gate_violation(
            ExecutionEfficiencyMetrics {
                no_change_response_bytes: 1_025,
                ..base
            },
            "noChangeResponseBytes",
            1_025,
            1_024,
            "bytes",
        );
        expect_gate_violation(
            ExecutionEfficiencyMetrics {
                authority_refresh_count: 2,
                ..base
            },
            "authorityRefreshesPerResume",
            2,
            1,
            "count",
        );
        expect_gate_violation(
            ExecutionEfficiencyMetrics {
                max_no_progress_batches: 4,
                ..base
            },
            "maxConsecutiveNoProgressBatches",
            4,
            3,
            "count",
        );
        expect_gate_violation(
            ExecutionEfficiencyMetrics {
                broad_before_green_count: 1,
                ..base
            },
            "broadTestsBeforeFocusedGreen",
            1,
            0,
            "count",
        );
    }
}
