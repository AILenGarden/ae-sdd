use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use super::{
    ServiceDescriptorState, ServiceDescriptorStatus, ServiceError, ServiceLifecyclePlan,
    ServiceMaterialization, ServicePermissionAssertion,
};
use crate::service::render::{descriptor_bytes, digest};

const MAX_DESCRIPTOR_BYTES: u64 = 1024 * 1024;

pub fn materialize_service_descriptor(
    plan: &ServiceLifecyclePlan,
) -> Result<ServiceMaterialization, ServiceError> {
    let home = plan
        .user_home
        .canonicalize()
        .map_err(|source| ServiceError::Io {
            path: plan.user_home.clone(),
            source,
        })?;
    ensure_destination(&home, &plan.state_dir)?;
    ensure_destination(&home, &plan.descriptor_path)?;

    create_private_directory(&plan.state_dir, plan)?;
    let parent = plan
        .descriptor_path
        .parent()
        .ok_or(ServiceError::InvalidPath("descriptorPath"))?;
    create_private_directory(parent, plan)?;

    let bytes = descriptor_bytes(plan.platform, &plan.descriptor_contents);
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_DESCRIPTOR_BYTES {
        return Err(ServiceError::DescriptorTooLarge);
    }
    let created = match read_descriptor(&plan.descriptor_path)? {
        Some(existing) if digest(&existing) == plan.descriptor_digest => false,
        _ => {
            atomic_write(&plan.descriptor_path, &bytes)?;
            protect_private_file(&plan.descriptor_path, plan)?;
            true
        }
    };
    if !created {
        protect_private_file(&plan.descriptor_path, plan)?;
    }

    let status = inspect_service_descriptor(plan)?;
    if status.state != ServiceDescriptorState::Matches
        || status
            .permission_assertions
            .iter()
            .any(|assertion| !assertion.passed)
    {
        return Err(ServiceError::PermissionVerificationFailed);
    }
    Ok(ServiceMaterialization {
        schema_version: "ae-sdd-service-materialization/v1",
        descriptor_path: plan.descriptor_path.clone(),
        descriptor_digest: plan.descriptor_digest.clone(),
        created,
        permission_assertions: status.permission_assertions,
    })
}

pub fn inspect_service_descriptor(
    plan: &ServiceLifecyclePlan,
) -> Result<ServiceDescriptorStatus, ServiceError> {
    let observed = read_descriptor(&plan.descriptor_path)?;
    let observed_digest = observed.as_ref().map(|bytes| digest(bytes));
    let state = match &observed_digest {
        None => ServiceDescriptorState::Absent,
        Some(value) if value == &plan.descriptor_digest => ServiceDescriptorState::Matches,
        Some(_) => ServiceDescriptorState::Drifted,
    };
    let mut assertions = Vec::new();
    if state != ServiceDescriptorState::Absent {
        assertions.extend(permission_assertions(&plan.descriptor_path, false, plan)?);
    }
    if plan.state_dir.exists() {
        assertions.extend(permission_assertions(&plan.state_dir, true, plan)?);
    }
    Ok(ServiceDescriptorStatus {
        schema_version: "ae-sdd-service-status/v1",
        descriptor_path: plan.descriptor_path.clone(),
        state,
        expected_digest: plan.descriptor_digest.clone(),
        observed_digest,
        permission_assertions: assertions,
    })
}

pub(super) fn remove_service_descriptor(plan: &ServiceLifecyclePlan) -> Result<bool, ServiceError> {
    let home = plan
        .user_home
        .canonicalize()
        .map_err(|source| ServiceError::Io {
            path: plan.user_home.clone(),
            source,
        })?;
    ensure_destination(&home, &plan.descriptor_path)?;
    match inspect_service_descriptor(plan)?.state {
        ServiceDescriptorState::Absent => Ok(false),
        ServiceDescriptorState::Drifted => Err(ServiceError::DescriptorDrift),
        ServiceDescriptorState::Matches => {
            fs::remove_file(&plan.descriptor_path).map_err(|source| ServiceError::Io {
                path: plan.descriptor_path.clone(),
                source,
            })?;
            let parent = plan
                .descriptor_path
                .parent()
                .ok_or(ServiceError::InvalidPath("descriptorPath"))?;
            sync_directory(parent)?;
            Ok(true)
        }
    }
}

pub(super) fn ensure_destination(home: &Path, destination: &Path) -> Result<(), ServiceError> {
    let mut ancestor = destination;
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or(ServiceError::DestinationOutsideUserHome)?;
    }
    let canonical = ancestor.canonicalize().map_err(|source| ServiceError::Io {
        path: ancestor.to_path_buf(),
        source,
    })?;
    if !canonical.starts_with(home) {
        return Err(ServiceError::DestinationOutsideUserHome);
    }
    Ok(())
}

pub(super) fn create_private_directory(
    path: &Path,
    plan: &ServiceLifecyclePlan,
) -> Result<(), ServiceError> {
    fs::create_dir_all(path).map_err(|source| ServiceError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if fs::symlink_metadata(path)
        .map_err(|source| ServiceError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .file_type()
        .is_symlink()
    {
        return Err(ServiceError::SymbolicLink(path.to_path_buf()));
    }
    protect_private_directory(path, plan)
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), ServiceError> {
    let parent = path
        .parent()
        .ok_or(ServiceError::InvalidPath("descriptorPath"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(ServiceError::InvalidPath("descriptorPath"))?;
    let suffix = std::process::id();
    let stage = parent.join(format!(".{name}.ae-sdd-stage-{suffix}"));
    let backup = parent.join(format!(".{name}.ae-sdd-backup-{suffix}"));
    if stage.exists() || backup.exists() {
        return Err(ServiceError::StagingConflict);
    }
    let mut handle = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stage)
        .map_err(|source| ServiceError::Io {
            path: stage.clone(),
            source,
        })?;
    if let Err(source) = handle.write_all(bytes).and_then(|()| handle.sync_all()) {
        let _ = fs::remove_file(&stage);
        return Err(ServiceError::Io {
            path: stage,
            source,
        });
    }
    if path.exists() {
        fs::rename(path, &backup).map_err(|source| ServiceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    if let Err(source) = fs::rename(&stage, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(ServiceError::Io {
            path: path.to_path_buf(),
            source,
        });
    }
    if backup.exists() {
        fs::remove_file(&backup).map_err(|source| ServiceError::Io {
            path: backup,
            source,
        })?;
    }
    sync_directory(parent)
}

pub(super) fn read_descriptor(path: &Path) -> Result<Option<Vec<u8>>, ServiceError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ServiceError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(ServiceError::SymbolicLink(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(ServiceError::InvalidPath("descriptorPath"));
    }
    if metadata.len() > MAX_DESCRIPTOR_BYTES {
        return Err(ServiceError::DescriptorTooLarge);
    }
    fs::read(path).map(Some).map_err(|source| ServiceError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn protect_private_directory(
    path: &Path,
    _plan: &ServiceLifecyclePlan,
) -> Result<(), ServiceError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
        ServiceError::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(unix)]
pub(super) fn protect_private_file(
    path: &Path,
    _plan: &ServiceLifecyclePlan,
) -> Result<(), ServiceError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
        ServiceError::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(windows)]
fn protect_private_directory(path: &Path, plan: &ServiceLifecyclePlan) -> Result<(), ServiceError> {
    protect_windows(path, plan, true)
}

#[cfg(windows)]
pub(super) fn protect_private_file(
    path: &Path,
    plan: &ServiceLifecyclePlan,
) -> Result<(), ServiceError> {
    protect_windows(path, plan, false)
}

#[cfg(windows)]
fn protect_windows(
    path: &Path,
    plan: &ServiceLifecyclePlan,
    directory: bool,
) -> Result<(), ServiceError> {
    use std::process::{Command, Stdio};

    let principal = plan
        .permission_policy
        .windows_dacl_principal
        .as_deref()
        .ok_or(ServiceError::PermissionVerificationFailed)?;
    let suffix = if directory { ":(OI)(CI)F" } else { ":F" };
    let grant = format!("{principal}{suffix}");
    let status = Command::new("icacls.exe")
        .arg(path)
        .args(["/inheritance:r", "/grant:r", &grant])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| ServiceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if !status.success() {
        return Err(ServiceError::PermissionVerificationFailed);
    }
    Ok(())
}

#[cfg(unix)]
fn permission_assertions(
    path: &Path,
    directory: bool,
    _plan: &ServiceLifecyclePlan,
) -> Result<Vec<ServicePermissionAssertion>, ServiceError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .map_err(|source| ServiceError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .permissions()
        .mode()
        & 0o777;
    let expected = if directory { 0o700 } else { 0o600 };
    Ok(vec![ServicePermissionAssertion {
        target: path.to_path_buf(),
        expected: format!("{expected:04o}"),
        observed: format!("{mode:04o}"),
        passed: mode == expected,
    }])
}

#[cfg(windows)]
fn permission_assertions(
    path: &Path,
    _directory: bool,
    plan: &ServiceLifecyclePlan,
) -> Result<Vec<ServicePermissionAssertion>, ServiceError> {
    use std::process::Command;

    let principal = plan
        .permission_policy
        .windows_dacl_principal
        .as_deref()
        .ok_or(ServiceError::PermissionVerificationFailed)?;
    let output = Command::new("icacls.exe")
        .arg(path)
        .output()
        .map_err(|source| ServiceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let listing = String::from_utf8_lossy(&output.stdout);
    let passed = output.status.success() && listing.contains(principal);
    Ok(vec![ServicePermissionAssertion {
        target: path.to_path_buf(),
        expected: format!("DACL current-user-only:{principal}"),
        observed: if passed {
            format!("DACL contains:{principal}")
        } else {
            "DACL verification failed".to_owned()
        },
        passed,
    }])
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), ServiceError> {
    use std::fs::File;

    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| ServiceError::Io {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), ServiceError> {
    Ok(())
}
