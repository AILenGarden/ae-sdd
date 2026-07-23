use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::tokens::LegacyArgumentError;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A private, create-new request file deleted automatically after the child
/// Rust build process exits.
#[derive(Debug)]
pub struct TemporaryJsonRequest {
    path: PathBuf,
}

impl TemporaryJsonRequest {
    pub fn create(request: &Value) -> Result<Self, LegacyArgumentError> {
        let mut last_error = None;
        for _ in 0..32 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            let path = std::env::temp_dir().join(format!(
                "ae-sdd-legacy-{}-{nanos}-{sequence}.json",
                std::process::id()
            ));
            match private_create_new(&path) {
                Ok(mut file) => {
                    if let Err(source) = serde_json::to_writer(&mut file, request) {
                        let _ = std::fs::remove_file(&path);
                        return Err(error(format!(
                            "failed to encode temporary request: {source}"
                        )));
                    }
                    if let Err(source) = file.flush() {
                        let _ = std::fs::remove_file(&path);
                        return Err(io_argument(source));
                    }
                    return Ok(Self { path });
                }
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                    last_error = Some(source);
                }
                Err(source) => return Err(io_argument(source)),
            }
        }
        Err(io_argument(last_error.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::AlreadyExists, "temporary request collision")
        })))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryJsonRequest {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn private_create_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn error(message: impl Into<String>) -> LegacyArgumentError {
    LegacyArgumentError::new(message)
}

fn io_argument(source: io::Error) -> LegacyArgumentError {
    error(format!("legacy temporary request I/O failed: {source}"))
}
