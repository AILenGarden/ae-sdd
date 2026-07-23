use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{DaemonClient, LocalIpcTransport};
use ae_sdd_protocol::{ClientKind, RequestParams, RpcMethod};
use serde::Serialize;
use serde_json::{Value, json};
use sysinfo::{Pid, ProcessesToUpdate, System};
use thiserror::Error;

const MAX_HOOK_P95_MICROS: u64 = 50_000;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const RPC_TIMEOUT: Duration = Duration::from_secs(2);

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
    pub elapsed_micros: u64,
    pub error_count: u64,
    pub receipt_replay_count: u64,
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
    let daemon = BenchmarkDaemon::connect_or_spawn(&config, &workspace_root)?;
    let client = DaemonClient::new(
        daemon.manifest_path.clone(),
        ClientKind::Hook,
        Arc::new(LocalIpcTransport),
        RPC_TIMEOUT,
    );
    let session = prepare_cached_hook(&client, &workspace_root)?;

    for _ in 0..config.warmup {
        let result = cached_hook_call(&client, &session)?;
        if !result
            .get("replayed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(BenchmarkError::ReceiptReplayMissing);
        }
    }

    let capacity =
        usize::try_from(config.samples).map_err(|_| BenchmarkError::InvalidSampleCount)?;
    let mut latencies = Vec::with_capacity(capacity);
    let mut error_count = 0_u64;
    let mut receipt_replay_count = 0_u64;
    let mut metrics = ProcessMetrics::new(daemon.pid);
    let cpu_start = metrics.sample()?.cpu_millis;
    let mut peak_rss = metrics.sample()?.rss_bytes;
    let sample_stride = (config.samples / 100).max(1);
    let benchmark_started = Instant::now();
    for index in 0..config.samples {
        let started = Instant::now();
        match cached_hook_call(&client, &session) {
            Ok(result)
                if result
                    .get("replayed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false) =>
            {
                receipt_replay_count += 1;
            }
            Ok(_) => error_count += 1,
            Err(_) => error_count += 1,
        }
        latencies.push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
        if index % sample_stride == 0 {
            peak_rss = peak_rss.max(metrics.sample()?.rss_bytes);
        }
    }
    let elapsed_micros = u64::try_from(benchmark_started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let final_metrics = metrics.sample()?;
    peak_rss = peak_rss.max(final_metrics.rss_bytes);
    latencies.sort_unstable();

    let summary = HookBenchmarkSummary {
        schema_version: "ae-sdd-hook-benchmark/v2",
        benchmark: "cached-hook-rpc-receipt-replay",
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
        max_micros: latencies.last().copied().unwrap_or_default(),
        elapsed_micros,
        error_count,
        receipt_replay_count,
        cpu_millis: final_metrics.cpu_millis.saturating_sub(cpu_start),
        rss_bytes: peak_rss,
    };
    if summary.error_count != 0 || summary.receipt_replay_count != summary.samples {
        return Err(BenchmarkError::RoundTripFailures(summary.error_count));
    }
    if summary.p95_micros > MAX_HOOK_P95_MICROS {
        return Err(BenchmarkError::P95BudgetExceeded {
            actual_micros: summary.p95_micros,
            maximum_micros: MAX_HOOK_P95_MICROS,
        });
    }
    Ok(summary)
}

fn prepare_cached_hook(
    client: &DaemonClient,
    workspace_root: &Path,
) -> Result<HookSession, BenchmarkError> {
    let workspace: Value = client.call(
        RpcMethod::WorkspaceRegister,
        request_params(
            json!({
                "projectRoot": workspace_root.to_string_lossy(),
                "projectKey": "ae-sdd-hook-benchmark",
                "mode": "shadow"
            }),
            None,
            None,
            None,
            Some("benchmark-workspace-register"),
        ),
    )?;
    let workspace_id = required_string(&workspace, "workspaceId")?;
    let session: Value = client.call(
        RpcMethod::SessionOpen,
        request_params(
            json!({
                "externalKey": "ae-sdd-hook-benchmark-session",
                "role": "root",
                "engaged": true
            }),
            Some(&workspace_id),
            None,
            None,
            Some("benchmark-session-open"),
        ),
    )?;
    let session_id = required_string(&session, "sessionId")?;
    let capability_token = required_string(&session, "capabilityToken")?;
    let prepared = HookSession {
        workspace_id,
        session_id,
        capability_token,
        turn_id: "00000000-0000-0000-0000-000000000101".to_owned(),
        work_item_id: "BENCHMARK-HOOK-001".to_owned(),
    };
    let _: Value = client.call(
        RpcMethod::HookUserPrompt,
        prepared.params(
            "benchmark-user-prompt",
            json!({"hookEventId":"benchmark-user-prompt","turnSeq":1,"hostPayload":{"prompt":"benchmark"}}),
        ),
    )?;
    let first = cached_hook_call(client, &prepared)?;
    if first
        .get("replayed")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Err(BenchmarkError::ReceiptSeedMissing);
    }
    Ok(prepared)
}

fn cached_hook_call(client: &DaemonClient, session: &HookSession) -> Result<Value, BenchmarkError> {
    client
        .call(
            RpcMethod::HookPreTool,
            session.params(
                "benchmark-pre-tool-replay",
                json!({
                    "hookEventId": "benchmark-pre-tool-replay",
                    "turnSeq": 1,
                    "hostPayload": {"tool": "apply_patch", "path": "benchmark"}
                }),
            ),
        )
        .map_err(BenchmarkError::Client)
}

struct HookSession {
    workspace_id: String,
    session_id: String,
    capability_token: String,
    turn_id: String,
    work_item_id: String,
}

impl HookSession {
    fn params(&self, idempotency_key: &str, payload: Value) -> RequestParams<Value> {
        let mut params = request_params(
            payload,
            Some(&self.workspace_id),
            Some(&self.session_id),
            Some(&self.capability_token),
            Some(idempotency_key),
        );
        params.agent_id = Some("benchmark-agent".to_owned());
        params.turn_id = Some(self.turn_id.clone());
        params.work_item_id = Some(self.work_item_id.clone());
        params
    }
}

fn request_params(
    payload: Value,
    workspace_id: Option<&str>,
    session_id: Option<&str>,
    capability_token: Option<&str>,
    idempotency_key: Option<&str>,
) -> RequestParams<Value> {
    RequestParams {
        protocol_version: "1.0".to_owned(),
        workspace_id: workspace_id.map(str::to_owned),
        agent_id: Some("benchmark-agent".to_owned()),
        session_id: session_id.map(str::to_owned),
        capability_token: capability_token.map(str::to_owned),
        turn_id: None,
        work_item_id: None,
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: idempotency_key.map(str::to_owned),
        confirmation: None,
        deadline_ms: 1_000,
        payload,
    }
}

fn required_string(value: &Value, field: &'static str) -> Result<String, BenchmarkError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(BenchmarkError::ResponseField(field))
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
            let manifest = client.endpoint_manifest()?;
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
        let _ = self.child.wait();
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
    #[error("live Hook RPC failed: {0}")]
    Client(#[from] ae_sdd_client::ClientError),
    #[error("live Hook benchmark I/O failed: {0}")]
    Io(std::io::Error),
    #[error("hook benchmark recorded {0} round-trip errors")]
    RoundTripFailures(u64),
    #[error("hook benchmark p95 {actual_micros}us exceeds {maximum_micros}us")]
    P95BudgetExceeded {
        actual_micros: u64,
        maximum_micros: u64,
    },
}

#[cfg(test)]
mod tests {
    use ae_sdd_protocol::{decode_frame, encode_frame};

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
}
