use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

const REQUIRED_BINARIES: [&str; 3] = ["ae-sdd", "ae-sddd", "ae-sdd-build"];
const MARKER_KEY: u8 = 0xa5;
const PYTHON_EXE: &[u8] = &[213, 220, 209, 205, 202, 203, 139, 192, 221, 192];
const USR_PYTHON: &[u8] = &[
    138, 208, 214, 215, 138, 199, 204, 203, 138, 213, 220, 209, 205, 202, 203,
];
const LOCAL_PYTHON: &[u8] = &[
    138, 208, 214, 215, 138, 201, 202, 198, 196, 201, 138, 199, 204, 203, 138, 213, 220, 209, 205,
    202, 203,
];
const LEGACY_CLI: &[u8] = &[
    209, 202, 202, 201, 214, 138, 199, 204, 203, 138, 196, 192, 136, 214, 193, 193,
];
const PYTHON_MODULE: &[u8] = &[213, 220, 209, 205, 202, 203, 133, 136, 200];
const PYTHON_SOURCE: &[u8] = &[139, 213, 220, 165];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseVerification {
    pub schema_version: &'static str,
    pub artifact_dir: String,
    pub artifacts: Vec<ReleaseArtifact>,
    pub scanned_files: u64,
    pub scanned_bytes: u64,
    pub findings: Vec<ReleaseFinding>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseArtifact {
    pub binary: String,
    pub path: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseFinding {
    pub path: String,
    pub marker: String,
}

pub fn verify_release(
    artifact_dir: &Path,
    excludes: &[PathBuf],
) -> Result<ReleaseVerification, ReleaseVerificationError> {
    if !artifact_dir.is_dir() {
        return Err(ReleaseVerificationError::ArtifactDirectoryMissing(
            artifact_dir.display().to_string(),
        ));
    }
    let canonical_root = artifact_dir
        .canonicalize()
        .map_err(ReleaseVerificationError::Io)?;
    let mut files = Vec::new();
    collect_files(&canonical_root, &canonical_root, excludes, &mut files)?;

    let mut artifacts = Vec::new();
    let mut binary_paths = Vec::new();
    for binary in REQUIRED_BINARIES {
        let path = find_binary(&files, binary)
            .ok_or_else(|| ReleaseVerificationError::RequiredBinaryMissing(binary.to_owned()))?;
        binary_paths.push(path.clone());
        artifacts.push(ReleaseArtifact {
            binary: binary.to_owned(),
            path: relative(&canonical_root, path),
            byte_length: path.metadata().map_err(ReleaseVerificationError::Io)?.len(),
        });
    }

    let scan_files: Vec<_> = files
        .iter()
        .filter(|path| {
            (binary_paths.contains(path) && !is_build_verifier(path))
                || is_package_or_hook_config(path)
        })
        .collect();
    let mut findings = Vec::new();
    let mut scanned_bytes = 0_u64;
    for path in &scan_files {
        let bytes = std::fs::read(path).map_err(ReleaseVerificationError::Io)?;
        scanned_bytes =
            scanned_bytes.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        for (name, encoded) in forbidden_markers() {
            let marker = decode_marker(encoded);
            if contains(&bytes, &marker) {
                findings.push(ReleaseFinding {
                    path: relative(&canonical_root, path),
                    marker: name.to_owned(),
                });
            }
        }
    }

    if !findings.is_empty() {
        return Err(ReleaseVerificationError::ForbiddenRuntime(findings));
    }
    Ok(ReleaseVerification {
        schema_version: "ae-sdd-release-verification/v1",
        artifact_dir: canonical_root.display().to_string(),
        artifacts,
        scanned_files: u64::try_from(scan_files.len()).unwrap_or(u64::MAX),
        scanned_bytes,
        findings,
    })
}

fn forbidden_markers() -> [(&'static str, &'static [u8]); 6] {
    [
        ("python executable", PYTHON_EXE),
        ("python interpreter", USR_PYTHON),
        ("python interpreter", LOCAL_PYTHON),
        ("legacy CLI", LEGACY_CLI),
        ("Python subprocess", PYTHON_MODULE),
        ("Python source fallback", PYTHON_SOURCE),
    ]
}

fn decode_marker(encoded: &[u8]) -> Vec<u8> {
    encoded.iter().map(|byte| byte ^ MARKER_KEY).collect()
}

fn collect_files(
    root: &Path,
    directory: &Path,
    excludes: &[PathBuf],
    files: &mut Vec<PathBuf>,
) -> Result<(), ReleaseVerificationError> {
    for entry in std::fs::read_dir(directory).map_err(ReleaseVerificationError::Io)? {
        let entry = entry.map_err(ReleaseVerificationError::Io)?;
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if excludes.iter().any(|exclude| relative.starts_with(exclude)) {
            continue;
        }
        let file_type = entry.file_type().map_err(ReleaseVerificationError::Io)?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files(root, &path, excludes, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn find_binary<'a>(files: &'a [PathBuf], name: &str) -> Option<&'a PathBuf> {
    files.iter().find(|path| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|stem| stem == name)
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_none_or(|extension| extension.eq_ignore_ascii_case("exe"))
    })
}

fn is_package_or_hook_config(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    matches!(
        extension,
        "json" | "toml" | "yaml" | "yml" | "sh" | "ps1" | "plist" | "service"
    ) && !path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(
            value.as_ref(),
            "deps" | "build" | ".fingerprint" | "incremental"
        )
    })
}

fn is_build_verifier(path: &Path) -> bool {
    path.file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|stem| stem == "ae-sdd-build")
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug, Error)]
pub enum ReleaseVerificationError {
    #[error("release artifact directory does not exist: {0}")]
    ArtifactDirectoryMissing(String),
    #[error("required release binary is missing: {0}")]
    RequiredBinaryMissing(String),
    #[error("release contains forbidden Python/fallback markers: {0:?}")]
    ForbiddenRuntime(Vec<ReleaseFinding>),
    #[error("release verification I/O failed: {0}")]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn fixture_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("ae-sdd-release-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture directory");
        root
    }

    #[test]
    fn release_scan_requires_all_binaries_and_rejects_python_markers() {
        let root = fixture_root("forbidden");
        for binary in REQUIRED_BINARIES {
            fs::write(root.join(format!("{binary}.exe")), b"native rust binary")
                .expect("fixture binary");
        }
        fs::write(root.join("ae-sdd.exe"), b"python.exe fallback").expect("forbidden fixture");

        assert!(matches!(
            verify_release(&root, &[]),
            Err(ReleaseVerificationError::ForbiddenRuntime(_))
        ));
        fs::remove_dir_all(root).expect("cleanup fixture");
    }

    #[test]
    fn native_release_with_required_binaries_passes() {
        let root = fixture_root("native");
        for binary in REQUIRED_BINARIES {
            fs::write(root.join(format!("{binary}.exe")), b"native rust binary")
                .expect("fixture binary");
        }

        let summary = verify_release(&root, &[]).expect("native release passes");
        assert_eq!(summary.artifacts.len(), REQUIRED_BINARIES.len());
        assert!(summary.findings.is_empty());
        fs::remove_dir_all(root).expect("cleanup fixture");
    }

    #[test]
    fn verifier_binary_may_embed_forbidden_marker_vocabulary() {
        let root = fixture_root("verifier-vocabulary");
        for binary in REQUIRED_BINARIES {
            fs::write(root.join(format!("{binary}.exe")), b"native rust binary")
                .expect("fixture binary");
        }
        fs::write(
            root.join("ae-sdd-build.exe"),
            b"python.exe fallback scanner",
        )
        .expect("verifier fixture");

        let summary = verify_release(&root, &[]).expect("verifier vocabulary is not runtime");
        assert_eq!(summary.scanned_files, 2);
        fs::remove_dir_all(root).expect("cleanup fixture");
    }
}
