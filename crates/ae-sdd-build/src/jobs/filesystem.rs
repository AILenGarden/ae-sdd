use super::*;

pub(super) fn promote_directory(
    target: &Path,
    files: &[MaterializedChange],
    suffix: &str,
) -> Result<(), JobError> {
    let parent = target
        .parent()
        .ok_or_else(|| JobError::Containment(display_path(target)))?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let stage = sibling_path(target, "stage", suffix)?;
    let backup = sibling_path(target, "backup", suffix)?;
    if stage.exists() {
        remove_tree_checked(&stage, parent)?;
    }
    if backup.exists() {
        return Err(JobError::IdempotencyConflict);
    }
    fs::create_dir(&stage).map_err(|source| io_error(&stage, source))?;
    set_permission(&stage, PermissionClass::Directory)?;
    for change in files {
        let relative = change
            .destination
            .strip_prefix(target)
            .map_err(|_| JobError::Containment(display_path(&change.destination)))?;
        atomic_write(&stage.join(relative), &change.bytes, change.permission)?;
    }
    sync_directory(&stage)?;
    if target.exists() {
        fs::rename(target, &backup).map_err(|source| io_error(target, source))?;
    }
    if let Err(source) = fs::rename(&stage, target) {
        if backup.exists() {
            let _ = fs::rename(&backup, target);
        }
        return Err(io_error(target, source));
    }
    sync_directory(parent)?;
    if backup.exists() {
        remove_tree_checked(&backup, parent)?;
    }
    Ok(())
}

pub(super) fn promote_overlay(
    root: &Path,
    files: &[MaterializedChange],
    suffix: &str,
) -> Result<(), JobError> {
    let mut staged = Vec::with_capacity(files.len());
    for change in files {
        let target = &change.destination;
        if !target.starts_with(root) {
            return Err(JobError::Containment(display_path(target)));
        }
        let stage = sibling_path(target, "stage", suffix)?;
        atomic_write(&stage, &change.bytes, change.permission)?;
        staged.push((target, stage));
    }

    let mut promoted: Vec<(&PathBuf, PathBuf)> = Vec::new();
    for (target, stage) in &staged {
        let backup = sibling_path(target, "backup", suffix)?;
        if backup.exists() {
            return Err(JobError::IdempotencyConflict);
        }
        if target.exists() {
            fs::rename(target, &backup).map_err(|source| io_error(target, source))?;
        }
        if let Err(source) = fs::rename(stage, target) {
            if backup.exists() {
                let _ = fs::rename(&backup, target);
            }
            for (done, done_backup) in promoted.into_iter().rev() {
                let _ = fs::remove_file(done);
                if done_backup.exists() {
                    let _ = fs::rename(&done_backup, done);
                }
            }
            return Err(io_error(target, source));
        }
        promoted.push((target, backup));
    }
    for (target, backup) in promoted {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|source| io_error(&backup, source))?;
        }
        if let Some(parent) = target.parent() {
            sync_directory(parent)?;
        }
    }
    Ok(())
}

pub(super) fn atomic_write(
    path: &Path,
    bytes: &[u8],
    permission: PermissionClass,
) -> Result<(), JobError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(JobError::PlanBudgetExceeded);
    }
    let parent = path
        .parent()
        .ok_or_else(|| JobError::Containment(display_path(path)))?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| JobError::InvalidRelativePath(display_path(path)))?;
    let temporary = parent.join(format!(".{file_name}.ae-sdd-write-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| io_error(&temporary, source))?;
    if let Err(source) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(io_error(&temporary, source));
    }
    set_permission(&temporary, permission)?;
    if path.exists() {
        fs::remove_file(path).map_err(|source| io_error(path, source))?;
    }
    fs::rename(&temporary, path).map_err(|source| io_error(path, source))?;
    sync_directory(parent)
}

pub(super) fn sibling_path(target: &Path, label: &str, suffix: &str) -> Result<PathBuf, JobError> {
    let parent = target
        .parent()
        .ok_or_else(|| JobError::Containment(display_path(target)))?;
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| JobError::InvalidRelativePath(display_path(target)))?;
    Ok(parent.join(format!(".{name}.ae-sdd-{label}-{suffix}")))
}

pub(super) fn remove_tree_checked(path: &Path, expected_parent: &Path) -> Result<(), JobError> {
    let canonical_parent = expected_parent
        .canonicalize()
        .map_err(|source| io_error(expected_parent, source))?;
    let canonical = path
        .canonicalize()
        .map_err(|source| io_error(path, source))?;
    let name_is_owned = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.contains(".ae-sdd-"));
    if canonical.parent() != Some(canonical_parent.as_path()) || !name_is_owned {
        return Err(JobError::Containment(display_path(path)));
    }
    fs::remove_dir_all(&canonical).map_err(|source| io_error(&canonical, source))
}

pub(super) fn validate_relative(path: &Path) -> Result<(), JobError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(JobError::InvalidRelativePath(display_path(path)));
    }
    Ok(())
}

pub(super) fn read_bounded(path: &Path) -> Result<Vec<u8>, JobError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() {
        return Err(JobError::SymbolicLink(display_path(path)));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(JobError::PlanBudgetExceeded);
    }
    fs::read(path).map_err(|source| io_error(path, source))
}

pub(super) fn request_digest(request: &NativeJobRequest) -> Result<String, JobError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DigestInput<'a> {
        schema_version: &'a str,
        entrypoint: &'a str,
        actor: &'a str,
        reason: &'a str,
        idempotency_key: &'a str,
        allowed_roots: &'a [PathBuf],
        job: &'a JobInput,
    }
    digest_json(&DigestInput {
        schema_version: &request.schema_version,
        entrypoint: &request.entrypoint,
        actor: &request.actor,
        reason: &request.reason,
        idempotency_key: &request.idempotency_key,
        allowed_roots: &request.allowed_roots,
        job: &request.job,
    })
}

pub(super) fn digest_json(value: &impl Serialize) -> Result<String, JobError> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(super) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

pub(super) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn io_error(path: &Path, source: std::io::Error) -> JobError {
    JobError::Io {
        path: display_path(path),
        source,
    }
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> Result<(), JobError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| io_error(path, source))
}

#[cfg(not(unix))]
pub(super) fn sync_directory(_path: &Path) -> Result<(), JobError> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_permission(path: &Path, permission: PermissionClass) -> Result<(), JobError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = match permission {
        PermissionClass::Directory | PermissionClass::Executable => 0o700,
        PermissionClass::PrivateFile => 0o600,
    };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|source| io_error(path, source))
}

#[cfg(not(unix))]
pub(super) fn set_permission(path: &Path, _permission: PermissionClass) -> Result<(), JobError> {
    let mut permissions = fs::metadata(path)
        .map_err(|source| io_error(path, source))?
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|source| io_error(path, source))
}

#[cfg(unix)]
pub(super) fn permission_for(path: &Path) -> Result<PermissionClass, JobError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .map_err(|source| io_error(path, source))?
        .permissions()
        .mode();
    Ok(if mode & 0o111 == 0 {
        PermissionClass::PrivateFile
    } else {
        PermissionClass::Executable
    })
}

#[cfg(not(unix))]
pub(super) fn permission_for(path: &Path) -> Result<PermissionClass, JobError> {
    let executable = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("exe"))
        || path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| matches!(value, "ae-sdd" | "ae-sddd" | "ae-sdd-build"));
    Ok(if executable {
        PermissionClass::Executable
    } else {
        PermissionClass::PrivateFile
    })
}
