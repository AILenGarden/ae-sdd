use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

use crate::client_config::ClientConfig;
use crate::registry::WriteLevel;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("cannot read capability digest: {0}")]
    Io(#[from] std::io::Error),
    #[error("capability digest must be 64 hexadecimal characters")]
    InvalidDigest,
    #[error("cannot serialize client configuration: {0}")]
    Json(#[from] serde_json::Error),
    #[error("client connection and endpoint must not be empty")]
    InvalidClientConfig,
    #[error("capability record is invalid: {0}")]
    InvalidCapabilityRecord(String),
    #[error("daemon private directory security is invalid: {0}")]
    InvalidPrivateDirectory(String),
}

#[derive(Clone)]
pub struct CapabilityVerifier {
    digest: [u8; 32],
    connection: Option<String>,
    write_level: WriteLevel,
}

#[derive(Debug, Deserialize, Serialize)]
struct CapabilityRecord {
    version: u32,
    digest: String,
    connection: String,
    #[serde(default)]
    write_level: WriteLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_path: Option<PathBuf>,
}

impl CapabilityVerifier {
    pub fn from_token(token: &str) -> Self {
        Self {
            digest: digest_bytes(token),
            connection: None,
            write_level: WriteLevel::None,
        }
    }

    pub fn from_token_for(token: &str, connection: &str) -> Self {
        Self {
            digest: digest_bytes(token),
            connection: Some(connection.to_owned()),
            write_level: WriteLevel::None,
        }
    }

    pub fn from_digest_hex(value: &str) -> Result<Self, SecurityError> {
        let value = value.trim();
        if value.len() != 64 {
            return Err(SecurityError::InvalidDigest);
        }
        let mut digest = [0_u8; 32];
        for (index, byte) in digest.iter_mut().enumerate() {
            let offset = index * 2;
            *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
                .map_err(|_| SecurityError::InvalidDigest)?;
        }
        Ok(Self {
            digest,
            connection: None,
            write_level: WriteLevel::None,
        })
    }

    pub fn verify(&self, candidate: &str) -> bool {
        bool::from(self.digest.ct_eq(&digest_bytes(candidate))) && !candidate.is_empty()
    }

    pub fn authorizes_connection(&self, connection: &str) -> bool {
        self.connection
            .as_deref()
            .is_none_or(|allowed| allowed == connection)
    }

    pub fn connection(&self) -> Option<&str> {
        self.connection.as_deref()
    }

    pub fn write_level(&self) -> WriteLevel {
        self.write_level
    }
}

pub fn capability_digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub fn revoke_connection_capability(private_dir: &Path, alias: &str) -> Result<bool, SecurityError> {
    let path = private_dir.join("capability.json");
    if !path.exists() { return Ok(false); }
    let verifier = load_capability_verifier(&path)?;
    if verifier.connection() != Some(alias) { return Ok(false); }
    let record: CapabilityRecord = serde_json::from_str(&fs::read_to_string(&path)?)?;
    // Remove only the exact client config issued by us, preserving independently changed files.
    if let Some(client_path) = record.client_path {
        if let Ok(bytes) = fs::read(&client_path) {
            if let Ok(client) = serde_json::from_slice::<ClientConfig>(&bytes) {
                if client.connection == alias && capability_digest(&client.capability) == record.digest {
                    fs::remove_file(client_path)?;
                }
            }
        }
    }
    fs::remove_file(path)?;
    Ok(true)
}

pub fn load_capability_verifier(
    path: &std::path::Path,
) -> Result<CapabilityVerifier, SecurityError> {
    let record: CapabilityRecord = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    if record.version != 1 || record.connection.trim().is_empty() {
        return Err(SecurityError::InvalidCapabilityRecord(
            "expected version 1 and a non-empty connection scope".to_owned(),
        ));
    }
    let mut verifier = CapabilityVerifier::from_digest_hex(&record.digest)?;
    verifier.connection = Some(record.connection);
    verifier.write_level = record.write_level;
    Ok(verifier)
}

pub fn issue_client_capability(
    private_dir: &Path,
    client_path: &Path,
    endpoint: &str,
    connection: &str,
    write_level: WriteLevel,
) -> Result<(), SecurityError> {
    if endpoint.trim().is_empty() || connection.trim().is_empty() {
        return Err(SecurityError::InvalidClientConfig);
    }
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let config = ClientConfig {
        endpoint: endpoint.to_owned(),
        capability: token.clone(),
        connection: connection.to_owned(),
        write_level,
    };
    let record = CapabilityRecord {
        version: 1,
        digest: capability_digest(&token),
        connection: connection.to_owned(),
        write_level,
        client_path: Some(client_path.to_owned()),
    };
    let mut record_bytes = serde_json::to_vec_pretty(&record)?;
    record_bytes.push(b'\n');
    write_private_file(&private_dir.join("capability.json"), &record_bytes)?;
    let mut serialized = serde_json::to_vec_pretty(&config)?;
    serialized.push(b'\n');
    write_private_file(client_path, &serialized)?;
    Ok(())
}

pub fn validate_private_directory(path: &Path) -> Result<(), SecurityError> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() {
        return Err(SecurityError::InvalidPrivateDirectory(
            "path is not a directory".to_owned(),
        ));
    }
    validate_private_directory_platform(path, &metadata)
}

#[cfg(unix)]
fn validate_private_directory_platform(
    _path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), SecurityError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(SecurityError::InvalidPrivateDirectory(format!(
            "mode {mode:o} grants group or other access"
        )));
    }
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.uid() != effective_uid {
        return Err(SecurityError::InvalidPrivateDirectory(
            "directory is not owned by the daemon identity".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn validate_private_directory_platform(
    path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), SecurityError> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, PSECURITY_DESCRIPTOR,
        SE_DACL_PROTECTED,
    };

    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let result = unsafe {
        GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(SecurityError::InvalidPrivateDirectory(
            std::io::Error::from_raw_os_error(result as i32).to_string(),
        ));
    }

    let validation = (|| {
        let mut control = 0_u16;
        let mut revision = 0_u32;
        if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
            return Err(SecurityError::InvalidPrivateDirectory(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        if control & SE_DACL_PROTECTED == 0 {
            return Err(SecurityError::InvalidPrivateDirectory(
                "DACL inheritance is not protected".to_owned(),
            ));
        }

        let mut sddl_ptr = ptr::null_mut();
        let mut sddl_len = 0_u32;
        if unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut sddl_ptr,
                &mut sddl_len,
            )
        } == 0
        {
            return Err(SecurityError::InvalidPrivateDirectory(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        let sddl = String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(sddl_ptr, sddl_len as usize)
        });
        unsafe {
            LocalFree(sddl_ptr.cast());
        }
        validate_private_directory_sddl(&sddl)
    })();
    unsafe {
        LocalFree(descriptor.cast());
    }
    validation
}

#[cfg(not(any(unix, windows)))]
fn validate_private_directory_platform(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), SecurityError> {
    Err(SecurityError::InvalidPrivateDirectory(
        "unsupported operating system".to_owned(),
    ))
}

pub fn validate_private_directory_sddl(sddl: &str) -> Result<(), SecurityError> {
    let normalized = sddl.trim().to_ascii_uppercase();
    if !normalized.starts_with("D:P") {
        return Err(SecurityError::InvalidPrivateDirectory(
            "DACL must be protected".to_owned(),
        ));
    }
    for broad in [";;;WD)", ";;;AU)", ";;;BU)", ";;;AN)", ";;;CO)", ";;;CG)"] {
        if normalized.contains(broad) {
            return Err(SecurityError::InvalidPrivateDirectory(
                "DACL grants a broad principal access".to_owned(),
            ));
        }
    }
    if !normalized.contains(";;;SY)") || !normalized.contains(";;;BA)") {
        return Err(SecurityError::InvalidPrivateDirectory(
            "DACL must include SYSTEM and Administrators".to_owned(),
        ));
    }
    Ok(())
}

fn write_private_file(path: &Path, content: &[u8]) -> Result<(), SecurityError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = temporary_path(path, suffix);
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(content)?;
        file.sync_all()?;
        set_private_permissions(&temp_path)?;
        replace_file(&temp_path, path)?;
        set_private_permissions(path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temp_path);
        return Err(error.into());
    }
    Ok(())
}

fn temporary_path(path: &Path, suffix: u128) -> PathBuf {
    path.with_file_name(format!(
        ".{}-{}-{suffix}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("secret"),
        std::process::id()
    ))
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn digest_bytes(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
