use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use ae_sdd_domain::ProjectRelativePath;
use thiserror::Error;

use crate::ScanScopeKind;

const EXCLUDED_DIRECTORIES: &[&str] = &[
    ".git",
    ".ae-sdd",
    ".auto-engineering",
    ".hermes",
    ".pytest_cache",
    ".venv",
    "__pycache__",
    "build",
    "changelog",
    "dist",
    "node_modules",
    "reference",
    "references",
    "reports",
    "target",
    "template",
    "templates",
    "vendor",
];

const GENERATED_RA_SUFFIXES: &[&str] = &[
    "-generateplan",
    "-impact",
    "-reverseissues",
    "-review",
    "-report",
    "-changelog",
];
const MAX_DISCOVERY_DEPTH: usize = 128;
const MAX_DISCOVERED_FILES: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExcludedPath {
    pub path: ProjectRelativePath,
    pub reason: Box<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedScanScope {
    pub files: Vec<(ProjectRelativePath, PathBuf)>,
    pub excluded: Vec<ExcludedPath>,
    pub explicit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaClassification {
    pub accepted: bool,
    pub reason: &'static str,
}

pub fn classify_formal_ra(path: &ProjectRelativePath) -> RaClassification {
    let parts: Vec<_> = path.as_str().split('/').collect();
    let Some(file_name) = parts.last() else {
        return RaClassification {
            accepted: false,
            reason: "not-formal-ra-filename",
        };
    };
    let lower_name = file_name.to_ascii_lowercase();
    if !(lower_name.starts_with("ra-") || lower_name.starts_with("ra_"))
        || !lower_name.ends_with(".md")
    {
        return RaClassification {
            accepted: false,
            reason: "not-formal-ra-filename",
        };
    }
    if parts[..parts.len() - 1]
        .iter()
        .any(|part| EXCLUDED_DIRECTORIES.contains(&part.to_ascii_lowercase().as_str()))
    {
        return RaClassification {
            accepted: false,
            reason: "excluded-directory",
        };
    }
    let stem = lower_name.trim_end_matches(".md");
    if GENERATED_RA_SUFFIXES
        .iter()
        .any(|suffix| stem.contains(suffix))
    {
        return RaClassification {
            accepted: false,
            reason: "generated-ra-event",
        };
    }
    let parents: Vec<_> = parts[..parts.len() - 1]
        .iter()
        .map(|part| part.to_ascii_lowercase())
        .collect();
    let canonical = parents.windows(2).any(|pair| pair == ["ae-sdd-doc", "ra"]);
    let legacy = parents
        .iter()
        .any(|part| matches!(part.as_str(), "design" | "ra"));
    if !canonical && !legacy {
        return RaClassification {
            accepted: false,
            reason: "non-authoritative-location",
        };
    }
    RaClassification {
        accepted: true,
        reason: "formal-ra",
    }
}

pub fn resolve_scan_scope(
    root: &Path,
    kind: ScanScopeKind,
    explicit_paths: &[ProjectRelativePath],
) -> Result<ResolvedScanScope, ScopeError> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(ScopeError::RootNotDirectory(root));
    }
    if !explicit_paths.is_empty() {
        let mut files = Vec::new();
        for relative in explicit_paths {
            let candidate = root.join(relative.as_str());
            let canonical =
                candidate
                    .canonicalize()
                    .map_err(|source| ScopeError::ExplicitPath {
                        path: relative.clone(),
                        source,
                    })?;
            if !canonical.starts_with(&root) {
                return Err(ScopeError::EscapedRoot(relative.clone()));
            }
            if !canonical.is_file() {
                return Err(ScopeError::NotAFile(relative.clone()));
            }
            files.push((relative.clone(), canonical));
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files.dedup_by(|left, right| left.0 == right.0);
        return Ok(ResolvedScanScope {
            files,
            excluded: Vec::new(),
            explicit: true,
        });
    }

    let mut discovered = Vec::new();
    collect_files(&root, &root, &mut discovered, 0)?;
    let mut files = Vec::new();
    let mut excluded = Vec::new();
    for (relative, absolute) in discovered {
        let accepted = matches_scope(kind, &relative);
        if accepted {
            files.push((relative, absolute));
        } else if kind == ScanScopeKind::FormalRa
            && relative.as_str().rsplit('/').next().is_some_and(|name| {
                let lower = name.to_ascii_lowercase();
                lower.starts_with("ra-") || lower.starts_with("ra_")
            })
        {
            let classification = classify_formal_ra(&relative);
            excluded.push(ExcludedPath {
                path: relative,
                reason: classification.reason.into(),
            });
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    excluded.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(ResolvedScanScope {
        files,
        excluded,
        explicit: false,
    })
}

fn collect_files(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(ProjectRelativePath, PathBuf)>,
    depth: usize,
) -> Result<(), ScopeError> {
    if depth > MAX_DISCOVERY_DEPTH {
        return Err(ScopeError::DepthLimit {
            maximum: MAX_DISCOVERY_DEPTH,
        });
    }
    let mut entries: Vec<_> = fs::read_dir(directory)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let relative = portable_relative(root, &path)?;
        if metadata.is_dir() {
            let name = relative
                .as_str()
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if EXCLUDED_DIRECTORIES.contains(&name.as_str()) {
                continue;
            }
            collect_files(root, &path, output, depth + 1)?;
        } else if metadata.is_file() {
            if output.len() >= MAX_DISCOVERED_FILES {
                return Err(ScopeError::FileDiscoveryLimit {
                    maximum: MAX_DISCOVERED_FILES,
                });
            }
            output.push((relative, path));
        }
    }
    Ok(())
}

fn matches_scope(kind: ScanScopeKind, path: &ProjectRelativePath) -> bool {
    let value = path.as_str();
    let lower = value.to_ascii_lowercase();
    let file_name = lower.rsplit('/').next().unwrap_or_default();
    let extension = file_name.rsplit_once('.').map(|(_, extension)| extension);
    match kind {
        ScanScopeKind::FormalRa => classify_formal_ra(path).accepted,
        ScanScopeKind::Production => {
            const EXTENSIONS: &[&str] = &[
                "rs",
                "py",
                "js",
                "jsx",
                "ts",
                "tsx",
                "java",
                "kt",
                "kts",
                "groovy",
                "go",
                "c",
                "cc",
                "cpp",
                "h",
                "hpp",
                "xml",
                "yaml",
                "yml",
                "toml",
                "properties",
            ];
            extension.is_some_and(|extension| EXTENSIONS.contains(&extension))
                && !is_test_path(&lower)
        }
        ScanScopeKind::TestsAndEvidence => {
            is_test_path(&lower)
                || (extension == Some("xml")
                    && (file_name.starts_with("test-") || lower.contains("surefire-reports")))
        }
        ScanScopeKind::Plugin => {
            lower.starts_with("plugins/")
                && extension.is_some_and(|extension| {
                    matches!(
                        extension,
                        "md" | "txt" | "py" | "js" | "ts" | "sh" | "ps1" | "yaml" | "yml"
                    )
                })
        }
    }
}

fn is_test_path(lower: &str) -> bool {
    lower
        .split('/')
        .any(|part| matches!(part, "test" | "tests"))
        || lower
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("test_") || name.contains("_test."))
}

fn portable_relative(root: &Path, path: &Path) -> Result<ProjectRelativePath, ScopeError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ScopeError::OutsideRoot(path.to_path_buf()))?;
    let mut segments = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(ScopeError::NonPortablePath(path.to_path_buf()));
        };
        segments.push(
            segment
                .to_str()
                .ok_or_else(|| ScopeError::NonPortablePath(path.to_path_buf()))?,
        );
    }
    ProjectRelativePath::new(segments.join("/"))
        .map_err(|_| ScopeError::NonPortablePath(path.to_path_buf()))
}

#[derive(Debug, Error)]
pub enum ScopeError {
    #[error("scan scope I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("scan root is not a directory: {0}")]
    RootNotDirectory(PathBuf),
    #[error("explicit scanner path {path} could not be resolved: {source}")]
    ExplicitPath {
        path: ProjectRelativePath,
        #[source]
        source: io::Error,
    },
    #[error("scanner path escapes root: {0}")]
    EscapedRoot(ProjectRelativePath),
    #[error("scanner path is not a regular file: {0}")]
    NotAFile(ProjectRelativePath),
    #[error("discovered path escaped root: {0}")]
    OutsideRoot(PathBuf),
    #[error("discovered path is not portable UTF-8: {0}")]
    NonPortablePath(PathBuf),
    #[error("scanner discovery nesting exceeds {maximum}")]
    DepthLimit { maximum: usize },
    #[error("scanner discovery exceeds {maximum} files")]
    FileDiscoveryLimit { maximum: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formal_ra_classifier_excludes_generated_and_reference_documents() {
        let formal = ProjectRelativePath::new("ae-sdd-doc/RA/RA-EXAMPLE-001.md").expect("path");
        let report =
            ProjectRelativePath::new("ae-sdd-doc/RA/RA-EXAMPLE-001-Report.md").expect("path");
        let reference = ProjectRelativePath::new("references/RA/RA-EXAMPLE-001.md").expect("path");

        assert!(classify_formal_ra(&formal).accepted);
        assert_eq!(classify_formal_ra(&report).reason, "generated-ra-event");
        assert_eq!(classify_formal_ra(&reference).reason, "excluded-directory");
    }
}
