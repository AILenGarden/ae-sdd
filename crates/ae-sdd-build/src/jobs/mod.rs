use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};

mod admin;
mod compile;
mod distribute;
mod filesystem;
mod harness;
mod init;
mod install;
mod migrate;
mod model;
mod planner;

use filesystem::*;
pub use model::*;

#[cfg(unix)]
use std::fs::File;

const JOB_SCHEMA: &str = "ae-sdd-native-job/v1";
const RECEIPT_SCHEMA: &str = "ae-sdd-native-job-receipt/v1";
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PLAN_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CHANGES: usize = 50_000;

pub fn execute_native_job(request: &NativeJobRequest) -> Result<JobExecution, JobError> {
    validate_request(request)?;
    let roots = AllowedRoots::new(&request.allowed_roots)?;
    let mut transaction = Transaction::plan(request, &roots)?;
    transaction.finalize()?;

    let request_digest = request_digest(request)?;
    let receipt_id = sha256_hex(
        format!(
            "{}\0{}\0{}",
            request.actor, request.entrypoint, request.idempotency_key
        )
        .as_bytes(),
    );
    let changes = transaction.public_changes();
    let plan_digest = digest_json(&changes)?;

    if request.mode == ExecutionMode::DryRun {
        return Ok(JobExecution {
            schema_version: JOB_SCHEMA,
            mode: request.mode,
            entrypoint: request.entrypoint.clone(),
            job_kind: request.job.kind(),
            request_digest,
            plan_digest,
            changes,
            receipt: None,
            replayed: false,
        });
    }

    let receipt_path = transaction.receipt_path(&receipt_id)?;
    if receipt_path.is_file() {
        let receipt: JobReceipt = serde_json::from_slice(&read_bounded(&receipt_path)?)?;
        if receipt.request_digest != request_digest || receipt.plan_digest != plan_digest {
            return Err(JobError::IdempotencyConflict);
        }
        return Ok(JobExecution {
            schema_version: JOB_SCHEMA,
            mode: request.mode,
            entrypoint: request.entrypoint.clone(),
            job_kind: request.job.kind(),
            request_digest,
            plan_digest,
            changes,
            receipt: Some(receipt),
            replayed: true,
        });
    }

    transaction.apply(&receipt_id)?;
    let receipt = JobReceipt {
        schema_version: RECEIPT_SCHEMA.to_owned(),
        receipt_id,
        entrypoint: request.entrypoint.clone(),
        job_kind: request.job.kind(),
        actor: request.actor.clone(),
        reason: request.reason.clone(),
        idempotency_key_digest: sha256_hex(request.idempotency_key.as_bytes()),
        request_digest: request_digest.clone(),
        plan_digest: plan_digest.clone(),
        applied_at_unix_ms: now_unix_ms(),
        promoted_roots: transaction.promoted_roots(),
    };
    let encoded = serde_json::to_vec_pretty(&receipt)?;
    atomic_write(&receipt_path, &encoded, PermissionClass::PrivateFile)?;

    Ok(JobExecution {
        schema_version: JOB_SCHEMA,
        mode: request.mode,
        entrypoint: request.entrypoint.clone(),
        job_kind: request.job.kind(),
        request_digest,
        plan_digest,
        changes,
        receipt: Some(receipt),
        replayed: false,
    })
}

fn validate_request(request: &NativeJobRequest) -> Result<(), JobError> {
    if request.schema_version != JOB_SCHEMA {
        return Err(JobError::Schema(request.schema_version.clone()));
    }
    for (field, value) in [
        ("entrypoint", request.entrypoint.as_str()),
        ("actor", request.actor.as_str()),
        ("reason", request.reason.as_str()),
        ("idempotencyKey", request.idempotency_key.as_str()),
    ] {
        if value.trim().is_empty() || value.len() > 1_024 || value.contains(['\0', '\r', '\n']) {
            return Err(JobError::InvalidField(field));
        }
    }
    if !request.entrypoint.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
    }) {
        return Err(JobError::InvalidField("entrypoint"));
    }
    if request.allowed_roots.is_empty() || request.allowed_roots.len() > 32 {
        return Err(JobError::InvalidField("allowedRoots"));
    }
    let spec = native_entrypoint(&request.entrypoint)
        .ok_or_else(|| JobError::EntrypointNotRegistered(request.entrypoint.clone()))?;
    let actual = request.job.kind();
    if actual != spec.kind {
        return Err(JobError::EntrypointKindMismatch {
            entrypoint: request.entrypoint.clone(),
            expected: spec.kind,
            actual,
        });
    }
    Ok(())
}

#[derive(Debug)]
struct AllowedRoots(Vec<PathBuf>);

impl AllowedRoots {
    fn new(paths: &[PathBuf]) -> Result<Self, JobError> {
        let mut roots = Vec::with_capacity(paths.len());
        for path in paths {
            let canonical = path
                .canonicalize()
                .map_err(|source| io_error(path, source))?;
            if !canonical.is_dir() {
                return Err(JobError::InvalidAllowedRoot(path.display().to_string()));
            }
            roots.push(canonical);
        }
        roots.sort();
        roots.dedup();
        Ok(Self(roots))
    }

    fn existing(&self, path: &Path) -> Result<PathBuf, JobError> {
        let canonical = path
            .canonicalize()
            .map_err(|source| io_error(path, source))?;
        self.ensure(&canonical)?;
        Ok(canonical)
    }

    fn destination(&self, path: &Path) -> Result<PathBuf, JobError> {
        let absolute = absolute_lexical(path)?;
        let mut ancestor = absolute.as_path();
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| JobError::Containment(absolute.display().to_string()))?;
        }
        let canonical_ancestor = ancestor
            .canonicalize()
            .map_err(|source| io_error(ancestor, source))?;
        self.ensure(&canonical_ancestor)?;
        let suffix = absolute
            .strip_prefix(ancestor)
            .map_err(|_| JobError::Containment(absolute.display().to_string()))?;
        let resolved = if suffix.as_os_str().is_empty() {
            canonical_ancestor
        } else {
            canonical_ancestor.join(suffix)
        };
        self.ensure(&resolved)?;
        Ok(resolved)
    }

    fn ensure(&self, path: &Path) -> Result<(), JobError> {
        if self.0.iter().any(|root| path.starts_with(root)) {
            Ok(())
        } else {
            Err(JobError::Containment(path.display().to_string()))
        }
    }
}

fn absolute_lexical(path: &Path) -> Result<PathBuf, JobError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| io_error(path, source))?
            .join(path)
    };
    if absolute
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(JobError::Containment(path.display().to_string()));
    }
    Ok(absolute)
}

#[derive(Debug)]
struct MaterializedChange {
    destination: PathBuf,
    source: Option<PathBuf>,
    bytes: Vec<u8>,
    digest: String,
    permission: PermissionClass,
}

#[derive(Debug)]
enum Promotion {
    Directory {
        target: PathBuf,
        files: Vec<MaterializedChange>,
    },
    Overlay {
        root: PathBuf,
        files: Vec<MaterializedChange>,
    },
}

#[derive(Debug)]
struct Transaction {
    promotions: Vec<Promotion>,
}

impl Transaction {
    fn plan(request: &NativeJobRequest, roots: &AllowedRoots) -> Result<Self, JobError> {
        let promotions = match &request.job {
            JobInput::Compile(input) => vec![compile::plan(input, roots)?],
            JobInput::Init(input) => init::plan(input, roots)?,
            JobInput::Install(input) => install::plan(input, roots)?,
            JobInput::Distribute(input) => distribute::plan(input, roots)?,
            JobInput::Harness(input) => vec![harness::plan(input, roots)?],
            JobInput::Migrate(input) => migrate::plan(input, roots)?,
            JobInput::Admin(input) => admin::plan(input, roots)?,
        };
        Ok(Self { promotions })
    }

    fn finalize(&mut self) -> Result<(), JobError> {
        let mut count = 0_usize;
        let mut bytes = 0_u64;
        let mut destinations = BTreeSet::new();
        for promotion in &self.promotions {
            for change in promotion.files() {
                count = count.saturating_add(1);
                bytes = bytes.saturating_add(u64::try_from(change.bytes.len()).unwrap_or(u64::MAX));
                if !destinations.insert(change.destination.clone()) {
                    return Err(JobError::InvalidField("duplicateDestination"));
                }
            }
        }
        if count == 0 || count > MAX_CHANGES || bytes > MAX_PLAN_BYTES {
            return Err(JobError::PlanBudgetExceeded);
        }
        Ok(())
    }

    fn public_changes(&self) -> Vec<PlannedChange> {
        let mut changes: Vec<_> = self
            .promotions
            .iter()
            .flat_map(Promotion::files)
            .map(|change| PlannedChange {
                destination: display_path(&change.destination),
                source: change.source.as_ref().map(|path| display_path(path)),
                digest: change.digest.clone(),
                byte_length: u64::try_from(change.bytes.len()).unwrap_or(u64::MAX),
                permission: change.permission,
            })
            .collect();
        changes.sort_by(|left, right| left.destination.cmp(&right.destination));
        changes
    }

    fn receipt_path(&self, receipt_id: &str) -> Result<PathBuf, JobError> {
        let root = self
            .promotions
            .first()
            .ok_or(JobError::PlanBudgetExceeded)?
            .receipt_root()?;
        Ok(root
            .join(".ae-sdd-job-receipts")
            .join(format!("{receipt_id}.json")))
    }

    fn apply(&self, receipt_id: &str) -> Result<(), JobError> {
        let mut applied: Vec<(&Promotion, String)> = Vec::new();
        for (index, promotion) in self.promotions.iter().enumerate() {
            let suffix = format!("{receipt_id}-{index}");
            if let Err(error) = promotion.apply(&suffix) {
                for (completed, completed_suffix) in applied.into_iter().rev() {
                    let _ = completed.rollback(&completed_suffix);
                }
                return Err(error);
            }
            applied.push((promotion, suffix));
        }
        Ok(())
    }

    fn promoted_roots(&self) -> Vec<String> {
        self.promotions
            .iter()
            .map(|promotion| display_path(promotion.root()))
            .collect()
    }
}

impl Promotion {
    fn files(&self) -> impl Iterator<Item = &MaterializedChange> {
        match self {
            Self::Directory { files, .. } | Self::Overlay { files, .. } => files.iter(),
        }
    }

    const fn root(&self) -> &PathBuf {
        match self {
            Self::Directory { target, .. } => target,
            Self::Overlay { root, .. } => root,
        }
    }

    fn receipt_root(&self) -> Result<PathBuf, JobError> {
        match self {
            Self::Directory { target, .. } => target
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| JobError::Containment(target.display().to_string())),
            Self::Overlay { root, .. } => Ok(root.clone()),
        }
    }

    fn apply(&self, suffix: &str) -> Result<(), JobError> {
        match self {
            Self::Directory { target, files } => promote_directory(target, files, suffix),
            Self::Overlay { root, files } => promote_overlay(root, files, suffix),
        }
    }

    fn rollback(&self, suffix: &str) -> Result<(), JobError> {
        match self {
            Self::Directory { target, .. } => {
                let backup = sibling_path(target, "backup", suffix)?;
                if backup.exists() {
                    if target.exists() {
                        remove_tree_checked(target, target.parent().unwrap_or(target))?;
                    }
                    fs::rename(&backup, target).map_err(|source| io_error(target, source))?;
                }
                Ok(())
            }
            Self::Overlay { root, files } => {
                for change in files.iter().rev() {
                    let target =
                        root.join(change.destination.strip_prefix(root).map_err(|_| {
                            JobError::Containment(display_path(&change.destination))
                        })?);
                    let backup = sibling_path(&target, "backup", suffix)?;
                    if backup.exists() {
                        if target.exists() {
                            fs::remove_file(&target).map_err(|source| io_error(&target, source))?;
                        }
                        fs::rename(&backup, &target).map_err(|source| io_error(&target, source))?;
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests;
