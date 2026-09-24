use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("credential alias is invalid")]
    InvalidAlias,
    #[error("credential vault I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("credential vault data is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("credential vault encoding is invalid")]
    Encoding,
    #[error("credential vault master key is invalid")]
    InvalidKey,
    #[error("credential decryption failed")]
    Decryption,
    #[error("credential text is invalid")]
    Utf8,
}

#[derive(Debug, Clone)]
pub struct CredentialVault {
    root: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct SealedCredential {
    version: u32,
    nonce: String,
    ciphertext: String,
}

impl CredentialVault {
    pub fn open(root: &Path) -> Result<Self, VaultError> {
        fs::create_dir_all(root)?;
        set_directory_permissions(root)?;
        let credentials = root.join("credentials");
        fs::create_dir_all(&credentials)?;
        set_directory_permissions(&credentials)?;
        Ok(Self {
            root: root.to_owned(),
        })
    }

    pub fn store(&self, alias: &str, password: &str) -> Result<(), VaultError> {
        validate_alias(alias)?;
        if password.is_empty() {
            return Err(VaultError::Encoding);
        }
        let key = self.load_or_create_key()?;
        let cipher =
            Aes256Gcm::new_from_slice(key.as_slice()).map_err(|_| VaultError::InvalidKey)?;
        let mut nonce_bytes = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce_bytes).map_err(|_| VaultError::Encoding)?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: password.as_bytes(),
                    aad: alias.as_bytes(),
                },
            )
            .map_err(|_| VaultError::Encoding)?;
        let sealed = SealedCredential {
            version: 1,
            nonce: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
            ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        };
        let mut serialized = serde_json::to_vec(&sealed)?;
        serialized.push(b'\n');
        write_private_file(&self.credential_path(alias), &serialized)
    }

    pub fn read(&self, alias: &str) -> Result<Zeroizing<String>, VaultError> {
        validate_alias(alias)?;
        let key = self.load_key()?;
        let sealed: SealedCredential =
            serde_json::from_slice(&fs::read(self.credential_path(alias))?)?;
        if sealed.version != 1 {
            return Err(VaultError::Encoding);
        }
        let nonce = base64::engine::general_purpose::STANDARD
            .decode(sealed.nonce)
            .map_err(|_| VaultError::Encoding)?;
        if nonce.len() != NONCE_BYTES {
            return Err(VaultError::Encoding);
        }
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(sealed.ciphertext)
            .map_err(|_| VaultError::Encoding)?;
        let cipher =
            Aes256Gcm::new_from_slice(key.as_slice()).map_err(|_| VaultError::InvalidKey)?;
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: &ciphertext,
                        aad: alias.as_bytes(),
                    },
                )
                .map_err(|_| VaultError::Decryption)?,
        );
        String::from_utf8(plaintext.to_vec())
            .map(Zeroizing::new)
            .map_err(|_| VaultError::Utf8)
    }

    pub fn delete(&self, alias: &str) -> Result<(), VaultError> {
        validate_alias(alias)?;
        match fs::remove_file(self.credential_path(alias)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn credential_path(&self, alias: &str) -> PathBuf {
        self.root.join("credentials").join(format!("{alias}.json"))
    }

    fn load_or_create_key(&self) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        match self.load_key() {
            Ok(key) => Ok(key),
            Err(VaultError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut key = Zeroizing::new(vec![0_u8; KEY_BYTES]);
                getrandom::fill(key.as_mut_slice()).map_err(|_| VaultError::InvalidKey)?;
                match create_private_file(&self.root.join("master.key"), key.as_slice()) {
                    Ok(()) => Ok(key),
                    Err(VaultError::Io(error))
                        if error.kind() == std::io::ErrorKind::AlreadyExists =>
                    {
                        self.load_key()
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    fn load_key(&self) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let key = Zeroizing::new(fs::read(self.root.join("master.key"))?);
        if key.len() != KEY_BYTES {
            return Err(VaultError::InvalidKey);
        }
        Ok(key)
    }
}

fn validate_alias(alias: &str) -> Result<(), VaultError> {
    if !alias.is_empty()
        && alias.len() <= 64
        && alias.as_bytes()[0].is_ascii_alphanumeric()
        && alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        Ok(())
    } else {
        Err(VaultError::InvalidAlias)
    }
}

fn create_private_file(path: &Path, content: &[u8]) -> Result<(), VaultError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(content)?;
    file.sync_all()?;
    set_file_permissions(path)?;
    Ok(())
}

fn write_private_file(path: &Path, content: &[u8]) -> Result<(), VaultError> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_file_name(format!(
        ".{}-{}-{suffix}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("credential"),
        std::process::id()
    ));
    create_private_file(&temporary, content)?;
    if let Err(error) = replace_file(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    set_file_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_directory_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_directory_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(unix)]
fn set_file_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> Result<(), std::io::Error> {
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
