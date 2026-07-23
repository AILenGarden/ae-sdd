use std::path::PathBuf;

use ae_sdd_build::{
    ExpectedCounts, HarnessBuildRequest, HookBenchmarkConfig, NativeJobRequest, OfflineRequest,
    PostCommitRequest, ServiceLifecycleRequest, ServiceOperation, audit_compatibility,
    benchmark_hook, execute_harness_build, execute_native_job, execute_offline,
    execute_post_commit, generate_service_lifecycle_plan, inspect_service_descriptor,
    materialize_service_descriptor, verify_release,
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
    Offline {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        json: bool,
    },
    NativeJob {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Harness {
        #[arg(long = "source", required = true)]
        sources: Vec<PathBuf>,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        title: String,
        #[arg(long = "allowed-root", required = true)]
        allowed_roots: Vec<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    PostCommit {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        package: PathBuf,
        #[arg(long = "target", required = true)]
        targets: Vec<PathBuf>,
        #[arg(long = "allowed-root", required = true)]
        allowed_roots: Vec<PathBuf>,
        #[arg(long)]
        commit: String,
        #[arg(long)]
        json: bool,
    },
    Service {
        #[arg(long)]
        request: PathBuf,
        #[arg(long, conflicts_with = "inspect")]
        materialize: bool,
        #[arg(long, conflicts_with = "materialize")]
        inspect: bool,
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
        Command::Offline { request, json } => {
            let request: OfflineRequest = serde_json::from_slice(&std::fs::read(request)?)?;
            let result = execute_offline(&request)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "offline {} {}: changed={}",
                    result.command,
                    match result.mode {
                        ae_sdd_build::ExecutionMode::DryRun => "planned",
                        ae_sdd_build::ExecutionMode::Apply => "applied",
                    },
                    result.changed_paths.len()
                );
            }
        }
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
        Command::Harness {
            sources,
            target,
            title,
            allowed_roots,
            dry_run,
            json,
        } => {
            let execution = execute_harness_build(&HarnessBuildRequest {
                source_files: sources,
                target_file: target,
                title,
                allowed_roots,
                mode: if dry_run {
                    ae_sdd_build::ExecutionMode::DryRun
                } else {
                    ae_sdd_build::ExecutionMode::Apply
                },
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&execution)?);
            } else {
                println!(
                    "harness {}: changes={} planDigest={}",
                    if execution.replayed {
                        "replayed"
                    } else if dry_run {
                        "planned"
                    } else {
                        "applied"
                    },
                    execution.changes.len(),
                    execution.plan_digest
                );
            }
        }
        Command::PostCommit {
            repository_root,
            source,
            package,
            targets,
            allowed_roots,
            commit,
            json,
        } => {
            let execution = execute_post_commit(&PostCommitRequest {
                repository_root,
                source_directory: source,
                package_directory: package,
                target_directories: targets,
                allowed_roots,
                commit_id: commit,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&execution)?);
            } else {
                println!(
                    "post-commit complete: compileReplay={} distributeReplay={} verifiedFiles={}",
                    execution.compile.replayed,
                    execution.distribute.replayed,
                    execution.verification.payload["verifiedFiles"]
                );
            }
        }
        Command::Service {
            request,
            materialize,
            inspect,
            json,
        } => {
            let request: ServiceLifecycleRequest =
                serde_json::from_slice(&std::fs::read(request)?)?;
            let plan = generate_service_lifecycle_plan(&request)?;
            if materialize {
                if request.operation != ServiceOperation::Install {
                    return Err("--materialize is valid only for an install plan".into());
                }
                let result = materialize_service_descriptor(&plan)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!(
                        "service descriptor {}: {} ({})",
                        if result.created {
                            "materialized"
                        } else {
                            "replayed"
                        },
                        result.descriptor_path.display(),
                        result.descriptor_digest
                    );
                }
            } else if inspect {
                let status = inspect_service_descriptor(&plan)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                } else {
                    println!(
                        "service descriptor status: {:?} ({})",
                        status.state,
                        status.descriptor_path.display()
                    );
                }
            } else if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!(
                    "service {} plan for {}: descriptor={} commands={}",
                    request.operation.as_str(),
                    request.platform.as_str(),
                    plan.descriptor_path.display(),
                    plan.manager_commands.len()
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
                    "hook benchmark: samples={} p50={}us p95={}us p99={}us max={}us errors={} receiptReplays={} engagedReplays={} allowDecisions={} cpuMillis={} rssBytes={}",
                    summary.samples,
                    summary.p50_micros,
                    summary.p95_micros,
                    summary.p99_micros,
                    summary.max_micros,
                    summary.error_count,
                    summary.receipt_replay_count,
                    summary.engaged_replay_count,
                    summary.allow_decision_count,
                    summary.cpu_millis,
                    summary.rss_bytes
                );
            }
        }
    }
    Ok(())
}
