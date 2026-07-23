use std::path::PathBuf;

use ae_sdd_build::{
    ExpectedCounts, HookBenchmarkConfig, NativeJobRequest, audit_compatibility, benchmark_hook,
    execute_native_job, verify_release,
};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "ae-sdd-build",
    version,
    about = "ae-sdd Rust build and migration tooling"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    NativeJob {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        json: bool,
    },
    CompatibilityAudit {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long, default_value_t = 113)]
        expected_commands: usize,
        #[arg(long, default_value_t = 18)]
        expected_operations: usize,
        #[arg(long, default_value_t = 36)]
        expected_gates: usize,
        #[arg(long, default_value_t = 7)]
        expected_scanners: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        exclude: Vec<PathBuf>,
    },
    VerifyRelease {
        #[arg(long)]
        artifact_dir: PathBuf,
        #[arg(long)]
        exclude: Vec<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    BenchmarkHook {
        #[arg(long, default_value_t = 1_000)]
        warmup: u64,
        #[arg(long, default_value_t = 10_000)]
        samples: u64,
        #[arg(long, default_value = "hdr")]
        histogram: String,
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long)]
        workspace_root: Option<PathBuf>,
        #[arg(long)]
        daemon_binary: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::NativeJob { request, json } => {
            let request: NativeJobRequest = serde_json::from_slice(&std::fs::read(request)?)?;
            let execution = execute_native_job(&request)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&execution)?);
            } else {
                println!(
                    "native job {} ({}) {}: changes={}, planDigest={}",
                    execution.entrypoint,
                    execution.job_kind.as_str(),
                    if execution.replayed {
                        "replayed"
                    } else {
                        match execution.mode {
                            ae_sdd_build::ExecutionMode::DryRun => "planned",
                            ae_sdd_build::ExecutionMode::Apply => "applied",
                        }
                    },
                    execution.changes.len(),
                    execution.plan_digest
                );
            }
        }
        Command::CompatibilityAudit {
            manifest,
            expected_commands,
            expected_operations,
            expected_gates,
            expected_scanners,
            json,
            exclude,
        } => {
            let summary = audit_compatibility(
                &manifest,
                ExpectedCounts {
                    commands: expected_commands,
                    operations: expected_operations,
                    gates: expected_gates,
                    scanners: expected_scanners,
                },
                &exclude,
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!(
                    "compatibility inventory valid: commands={}, operations={}, gates={}, scanners={}, routes={}, evidence={}, stubs={}, fallbacks={}",
                    summary.command_count,
                    summary.operation_count,
                    summary.gate_count,
                    summary.scanner_count,
                    summary.route_count,
                    summary.capability_evidence_count,
                    summary.stub_count,
                    summary.logical_fallback_count
                );
            }
        }
        Command::VerifyRelease {
            artifact_dir,
            exclude,
            json,
        } => {
            let verification = verify_release(&artifact_dir, &exclude)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&verification)?);
            } else {
                println!(
                    "release artifact valid: binaries={}, scannedFiles={}, scannedBytes={}",
                    verification.artifacts.len(),
                    verification.scanned_files,
                    verification.scanned_bytes
                );
            }
        }
        Command::BenchmarkHook {
            warmup,
            samples,
            histogram,
            manifest,
            workspace_root,
            daemon_binary,
            json,
        } => {
            let mut config = HookBenchmarkConfig::new(warmup, samples, &histogram)?;
            if let Some(path) = manifest {
                config = config.with_manifest(path);
            }
            if let Some(path) = workspace_root {
                config = config.with_workspace_root(path);
            }
            if let Some(path) = daemon_binary {
                config = config.with_daemon_binary(path);
            }
            let summary = benchmark_hook(config)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!(
                    "hook benchmark: samples={} p50={}us p95={}us p99={}us max={}us errors={} receiptReplays={} cpuMillis={} rssBytes={}",
                    summary.samples,
                    summary.p50_micros,
                    summary.p95_micros,
                    summary.p99_micros,
                    summary.max_micros,
                    summary.error_count,
                    summary.receipt_replay_count,
                    summary.cpu_millis,
                    summary.rss_bytes
                );
            }
        }
    }
    Ok(())
}
