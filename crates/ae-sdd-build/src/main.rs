use std::path::PathBuf;

use ae_sdd_build::{
    ExpectedCounts, HarnessBuildRequest, HookBenchmarkConfig, InstructionLanguage,
    ManagedInstructionStatus, ManagedInstructionTarget, NativeJobRequest, OfflineRequest,
    PostCommitRequest, RegistryResolution, ServiceLifecycleRequest, ServiceOperation,
    audit_compatibility, benchmark_hook, execute_harness_build, execute_native_job,
    execute_offline, execute_post_commit, execute_service_lifecycle,
    generate_service_lifecycle_plan, inspect_service_descriptor, materialize_service_descriptor,
    resolve_registry, verify_release,
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
        /// Explicit package targets. Mutually exclusive with the registry:
        /// mixing a hardcoded list with a data-driven one is what let the two
        /// drift in the first place.
        #[arg(long = "target", required_unless_present = "distributor_registry")]
        targets: Vec<PathBuf>,
        /// Distributor registry declaring every host, its target and its
        /// optional managed instruction file. Enabled entries that pass their
        /// `detect` check supply both the package and instruction targets.
        #[arg(long, conflicts_with_all = [
            "targets", "codex_instructions", "claude_instructions", "zcode_instructions",
        ])]
        distributor_registry: Option<PathBuf>,
        /// Home directory used to expand `~` in registry paths.
        #[arg(long, requires = "distributor_registry")]
        registry_home: Option<PathBuf>,
        #[arg(long = "allowed-root", required = true)]
        allowed_roots: Vec<PathBuf>,
        #[arg(long)]
        commit: String,
        /// Codex global instruction file; managed with the English L2 slice.
        #[arg(long)]
        codex_instructions: Option<PathBuf>,
        /// Claude global instruction file; managed with the Chinese L2 slice.
        #[arg(long)]
        claude_instructions: Option<PathBuf>,
        /// ZCode global instruction file; managed with the Chinese L2 slice.
        #[arg(long)]
        zcode_instructions: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Service {
        #[arg(long)]
        request: PathBuf,
        #[arg(long, conflicts_with_all = ["materialize", "inspect"])]
        execute: bool,
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
        #[arg(long, default_value_t = 23)]
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

/// Resolves the home directory used to expand `~` in registry paths.
///
/// Failing closed matters here: guessing a home would silently distribute to the
/// wrong tree, and the caller can always pass `--registry-home` explicitly.
fn home_directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
    for key in ["HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            let path = PathBuf::from(value);
            if path.is_dir() {
                return Ok(path);
            }
        }
    }
    Err("home directory could not be resolved; pass --registry-home".into())
}

/// Splits a resolved registry into package targets and instruction targets.
///
/// A host's instruction file is only ever the one the registry declares. A skill
/// directory such as `~/.codex/skills/ae-sdd` carries no reliable relationship
/// to a global instruction file, so it is never used to infer one; a host that
/// declares no `l2GlobalFile` stays package-only.
fn registry_targets(
    resolution: &RegistryResolution,
) -> (Vec<PathBuf>, Vec<ManagedInstructionTarget>) {
    let mut packages = Vec::with_capacity(resolution.hosts.len());
    let mut instructions = Vec::new();
    for host in &resolution.hosts {
        packages.push(host.package_target.clone());
        if let Some((target_file, language)) = &host.instruction_target {
            instructions.push(ManagedInstructionTarget {
                host: host.name.clone(),
                language: *language,
                target_file: target_file.clone(),
            });
        }
    }
    (packages, instructions)
}

/// Maps the explicit CLI instruction flags to managed targets.
///
/// The host-to-language mapping is fixed here on purpose: skill distribution
/// directories such as `~/.codex/skills/ae-sdd` carry no reliable relationship
/// to a global instruction file, so inferring paths from them is forbidden.
/// Harness and Hermes intentionally have no flag; they remain package-only
/// distribution targets.
fn managed_instruction_targets(
    codex: Option<PathBuf>,
    claude: Option<PathBuf>,
    zcode: Option<PathBuf>,
) -> Vec<ManagedInstructionTarget> {
    [
        ("codex", InstructionLanguage::En, codex),
        ("claude", InstructionLanguage::Zh, claude),
        ("zcode", InstructionLanguage::Zh, zcode),
    ]
    .into_iter()
    .filter_map(|(host, language, target_file)| {
        target_file.map(|target_file| ManagedInstructionTarget {
            host: host.to_owned(),
            language,
            target_file,
        })
    })
    .collect()
}

const fn managed_status_label(status: ManagedInstructionStatus) -> &'static str {
    match status {
        ManagedInstructionStatus::Updated => "updated",
        ManagedInstructionStatus::Unchanged => "unchanged",
        ManagedInstructionStatus::MissingTarget => "missing-target",
        ManagedInstructionStatus::MissingAnchor => "missing-anchor",
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
            distributor_registry,
            registry_home,
            allowed_roots,
            commit,
            codex_instructions,
            claude_instructions,
            zcode_instructions,
            json,
        } => {
            let mut skipped_hosts = Vec::new();
            let (target_directories, instruction_targets) = match &distributor_registry {
                Some(path) => {
                    let home = match registry_home {
                        Some(home) => home,
                        None => home_directory()?,
                    };
                    let resolution = resolve_registry(path, &home)?;
                    if resolution.hosts.is_empty() {
                        return Err(format!(
                            "distributor registry {} resolved no host; distributing nothing \
                             would report success while every agent stays stale",
                            path.display()
                        )
                        .into());
                    }
                    skipped_hosts = resolution.skipped.clone();
                    registry_targets(&resolution)
                }
                None => (
                    targets,
                    managed_instruction_targets(
                        codex_instructions,
                        claude_instructions,
                        zcode_instructions,
                    ),
                ),
            };
            let execution = execute_post_commit(&PostCommitRequest {
                repository_root,
                source_directory: source,
                package_directory: package,
                target_directories,
                allowed_roots,
                commit_id: commit,
                managed_instruction_targets: instruction_targets,
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
                if !execution.managed_instructions.is_empty() {
                    let summary = execution
                        .managed_instructions
                        .iter()
                        .map(|outcome| {
                            format!("{}={}", outcome.host, managed_status_label(outcome.status))
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("managed instructions: {summary}");
                }
                // A host that silently stops receiving the package is the defect
                // this registry exists to prevent, so every exclusion is named.
                if !skipped_hosts.is_empty() {
                    let summary = skipped_hosts
                        .iter()
                        .map(|host| format!("{}={}", host.name, host.reason.as_str()))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("registry skipped: {summary}");
                }
            }
        }
        Command::Service {
            request,
            execute,
            materialize,
            inspect,
            json,
        } => {
            let request: ServiceLifecycleRequest =
                serde_json::from_slice(&std::fs::read(request)?)?;
            let plan = generate_service_lifecycle_plan(&request)?;
            if execute {
                let receipt = execute_service_lifecycle(&plan)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&receipt)?);
                } else {
                    println!(
                        "service {} {}: commands={} descriptor={:?}",
                        request.operation.as_str(),
                        if receipt.replayed {
                            "replayed"
                        } else {
                            "executed"
                        },
                        receipt.commands.len(),
                        receipt.descriptor_action
                    );
                }
            } else if materialize {
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
