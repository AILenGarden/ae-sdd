//! Filesystem adapters for the typed resource plane.

use std::path::{Path, PathBuf};

use ae_sdd_contracts::resource::ResourceContractError;
use ae_sdd_domain::{ArtifactDigest, ArtifactKind, ArtifactRef, ProjectRelativePath};
use ae_sdd_resources::{
    AssetsCheckResult, AssetsDocument, AssetsMatch, AssetsPath, AssetsPort, AssetsQueryRequest,
    AssetsQueryResult, AssetsReadRequest, BoundedDocument, DeterministicResourceResolver,
    DocumentFinalizeRequest, DocumentPlanError, DocumentPlanner, DocumentPort, DocumentReadRequest,
    DocumentRequestError, DocumentSaveRequest, ResolvedDocument, ResourcePort,
    ResourceResolveError, ResourceResolveRequest,
};
use thiserror::Error;

/// Filesystem-backed implementation of the typed document/resource ports.
#[derive(Clone, Debug)]
pub struct FilesystemDocumentPort {
    root: PathBuf,
    resolver: DeterministicResourceResolver,
}

/// Filesystem-backed implementation of the bounded assets port.
#[derive(Clone, Debug)]
pub struct FilesystemAssetsPort {
    root: PathBuf,
}

impl FilesystemAssetsPort {
    /// Canonicalizes and validates a project root.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FilesystemResourceError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|_| FilesystemResourceError::InvalidRoot)?;
        if !root.is_dir() {
            return Err(FilesystemResourceError::InvalidRoot);
        }
        Ok(Self { root })
    }

    fn path(&self, path: &AssetsPath) -> Result<PathBuf, FilesystemResourceError> {
        let parsed = parse_project_path(path.as_path().as_str())?;
        let target = self
            .root
            .join(parsed.as_str().replace('/', std::path::MAIN_SEPARATOR_STR));
        ensure_contained(&self.root, &target)?;
        Ok(target)
    }

    fn load(&self, path: &AssetsPath) -> Result<(ArtifactRef, String), FilesystemResourceError> {
        let target = self.path(path)?;
        let bytes = std::fs::read(&target).map_err(|_| FilesystemResourceError::NotFound)?;
        if bytes.len() as u64 > ae_sdd_resources::MAX_ASSETS_READ_BYTES {
            return Err(FilesystemResourceError::ReadLimitExceeded);
        }
        let content = String::from_utf8(bytes.clone())
            .map_err(|_| FilesystemResourceError::InvalidAssetsDocument)?;
        let kind = ArtifactKind::new("assets")
            .map_err(|_| FilesystemResourceError::InvalidArtifactKind)?;
        let reference = ArtifactRef::new(
            kind,
            path.as_path().clone(),
            ArtifactDigest::digest(&bytes),
            bytes.len() as u64,
        );
        Ok((reference, content))
    }
}

impl AssetsPort for FilesystemAssetsPort {
    type Error = FilesystemResourceError;

    fn read(&self, request: &AssetsReadRequest) -> Result<AssetsDocument, Self::Error> {
        let (reference, content) = self.load(request.path())?;
        let selected = request
            .section()
            .map(|section| {
                section_content(&content, section).ok_or(FilesystemResourceError::SectionNotFound)
            })
            .transpose()?
            .unwrap_or_else(|| content.clone());
        let (bounded, truncated) = bounded_utf8_prefix(&selected, request.max_bytes() as usize);
        Ok(AssetsDocument::new(
            reference,
            bounded.to_owned(),
            truncated,
        ))
    }

    fn check(&self, path: &AssetsPath) -> Result<AssetsCheckResult, Self::Error> {
        let (reference, content) = self.load(path)?;
        let missing_sections = ["§A", "§B", "§C", "§D", "§E", "§F", "§G"]
            .iter()
            .filter(|section| section_content(&content, section).is_none())
            .map(|section| (*section).into())
            .collect();
        Ok(AssetsCheckResult::new(reference, missing_sections))
    }

    fn query(&self, request: &AssetsQueryRequest) -> Result<AssetsQueryResult, Self::Error> {
        let (reference, content) = self.load(request.path())?;
        let query = request.query().to_ascii_lowercase();
        let mut matches = Vec::new();
        let mut used = 0_usize;
        let mut truncated = false;
        for (index, line) in content.lines().enumerate() {
            if !line.to_ascii_lowercase().contains(&query) {
                continue;
            }
            if matches.len() >= request.max_matches() {
                truncated = true;
                break;
            }
            let snippet = line.chars().take(160).collect::<String>();
            if used.saturating_add(snippet.len()) > request.max_bytes() {
                truncated = true;
                break;
            }
            used = used.saturating_add(snippet.len());
            matches.push(AssetsMatch::new(
                section_for_line(&content, index),
                index + 1,
                snippet,
            ));
        }
        Ok(AssetsQueryResult::new(reference, matches, truncated))
    }
}

impl FilesystemDocumentPort {
    /// Canonicalizes and validates a project root.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FilesystemResourceError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|_| FilesystemResourceError::InvalidRoot)?;
        if !root.is_dir() {
            return Err(FilesystemResourceError::InvalidRoot);
        }
        Ok(Self {
            root,
            resolver: DeterministicResourceResolver,
        })
    }

    /// Returns the canonical root for adapter diagnostics and tests.
    pub const fn root(&self) -> &PathBuf {
        &self.root
    }

    fn target(&self, path: &ProjectRelativePath) -> Result<PathBuf, FilesystemResourceError> {
        let parsed = parse_project_path(path.as_str())?;
        let target = self
            .root
            .join(parsed.as_str().replace('/', std::path::MAIN_SEPARATOR_STR));
        ensure_contained(&self.root, &target)?;
        Ok(target)
    }
}

impl DocumentPort for FilesystemDocumentPort {
    type Error = FilesystemResourceError;

    fn resolve(&self, request: &ResourceResolveRequest) -> Result<ResolvedDocument, Self::Error> {
        for candidate in request.candidates() {
            parse_project_path(candidate.artifact_ref().path().as_str())?;
        }
        let resolved = self.resolver.resolve(request)?;
        let target = self.target(resolved.winner().path())?;
        let metadata = std::fs::metadata(&target).map_err(|_| FilesystemResourceError::NotFound)?;
        if !metadata.is_file() {
            return Err(FilesystemResourceError::NotAFile);
        }
        if metadata.len() != resolved.winner().byte_length() {
            return Err(FilesystemResourceError::DigestMismatch);
        }
        let bytes = std::fs::read(&target).map_err(|_| FilesystemResourceError::NotFound)?;
        if ArtifactDigest::digest(&bytes) != resolved.winner().digest() {
            return Err(FilesystemResourceError::DigestMismatch);
        }
        Ok(ResolvedDocument::new(resolved))
    }

    fn read(&self, request: &DocumentReadRequest) -> Result<BoundedDocument, Self::Error> {
        let target = self.target(request.reference().path())?;
        let bytes = std::fs::read(&target).map_err(|_| FilesystemResourceError::NotFound)?;
        if bytes.len() as u64 > request.max_bytes() {
            return Err(FilesystemResourceError::ReadLimitExceeded);
        }
        let digest = ArtifactDigest::digest(&bytes);
        if digest != request.reference().digest()
            || bytes.len() as u64 != request.reference().byte_length()
            || request
                .expected_digest()
                .is_some_and(|expected| expected != digest)
        {
            return Err(FilesystemResourceError::DigestMismatch);
        }
        Ok(BoundedDocument::new(request.reference().clone(), bytes))
    }

    fn save(
        &self,
        request: &DocumentSaveRequest,
    ) -> Result<ae_sdd_contracts::resource::DocumentTxnPlan, Self::Error> {
        Ok(DocumentPlanner::save(request)?)
    }

    fn finalize(
        &self,
        request: &DocumentFinalizeRequest,
    ) -> Result<ae_sdd_contracts::resource::DocumentTxnPlan, Self::Error> {
        Ok(DocumentPlanner::finalize(request)?)
    }
}

/// Stable, sanitized filesystem-boundary failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FilesystemResourceError {
    /// The project root was not a canonical directory.
    #[error("invalid project root")]
    InvalidRoot,
    /// A project-relative path used an unsafe lexical form.
    #[error("invalid project-relative path")]
    InvalidProjectPath,
    /// Canonicalization or a reparse/symlink target escaped the root.
    #[error("resource path escapes the canonical project root")]
    ContainmentDenied,
    /// The target does not exist.
    #[error("resource target was not found")]
    NotFound,
    /// The target exists but is not a regular file.
    #[error("resource target is not a regular file")]
    NotAFile,
    /// The read exceeded the caller's bounded byte budget.
    #[error("resource read exceeded its byte budget")]
    ReadLimitExceeded,
    /// The observed content did not match its content-addressed reference.
    #[error("resource content digest or length mismatch")]
    DigestMismatch,
    /// The assets document was not valid UTF-8 Markdown.
    #[error("assets document is not valid UTF-8 Markdown")]
    InvalidAssetsDocument,
    /// A named assets section was not found.
    #[error("requested assets section was not found")]
    SectionNotFound,
    /// A fixed artifact kind could not be constructed.
    #[error("fixed resource artifact kind is invalid")]
    InvalidArtifactKind,
    /// Typed resolver validation failed.
    #[error(transparent)]
    Resolve(#[from] ResourceResolveError),
    /// Typed document request validation failed.
    #[error(transparent)]
    Request(#[from] DocumentRequestError),
    /// Typed document plan validation failed.
    #[error(transparent)]
    Plan(#[from] DocumentPlanError),
    /// Shared resource contract validation failed.
    #[error(transparent)]
    Contract(#[from] ResourceContractError),
}

/// Parses a portable project-relative path and rejects Windows escape forms.
pub(crate) fn parse_project_path(
    value: &str,
) -> Result<ProjectRelativePath, FilesystemResourceError> {
    if value.is_empty()
        || value.contains('\\')
        || value.starts_with('/')
        || value.contains(':')
        || value.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
    {
        return Err(FilesystemResourceError::InvalidProjectPath);
    }
    for component in value.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(FilesystemResourceError::InvalidProjectPath);
        }
        if component.ends_with('.') || component.ends_with(' ') {
            return Err(FilesystemResourceError::InvalidProjectPath);
        }
        let stem = component
            .split_once('.')
            .map_or(component, |(stem, _)| stem)
            .to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit()
                && stem.as_bytes()[3] != b'0')
        {
            return Err(FilesystemResourceError::InvalidProjectPath);
        }
    }
    ProjectRelativePath::new(value.to_owned())
        .map_err(|_| FilesystemResourceError::InvalidProjectPath)
}

fn ensure_contained(root: &Path, target: &Path) -> Result<(), FilesystemResourceError> {
    let mut current = root.to_path_buf();
    let relative = target
        .strip_prefix(root)
        .map_err(|_| FilesystemResourceError::ContainmentDenied)?;
    for component in relative.components() {
        current.push(component);
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|_| FilesystemResourceError::NotFound)?;
        if metadata_flags_are_reparse_or_symlink(
            file_attributes(&metadata),
            metadata.file_type().is_symlink(),
        ) {
            return Err(FilesystemResourceError::ContainmentDenied);
        }
    }
    let canonical = target
        .canonicalize()
        .map_err(|_| FilesystemResourceError::NotFound)?;
    if !canonical.starts_with(root) {
        return Err(FilesystemResourceError::ContainmentDenied);
    }
    Ok(())
}

fn file_attributes(metadata: &std::fs::Metadata) -> u32 {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes()
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        0
    }
}

pub(crate) fn metadata_flags_are_reparse_or_symlink(
    file_attributes: u32,
    is_symlink: bool,
) -> bool {
    is_symlink || (file_attributes & 0x400) != 0
}

fn section_content(content: &str, requested: &str) -> Option<String> {
    let normalized = requested
        .trim()
        .trim_start_matches('§')
        .to_ascii_lowercase();
    let lines = content.lines().collect::<Vec<_>>();
    let mut start = None;
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with("## ") {
            continue;
        }
        let heading = line.trim_start_matches("## ").trim();
        let key = heading
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_start_matches('§')
            .to_ascii_lowercase();
        if key == normalized {
            start = Some(index);
            break;
        }
    }
    let start = start?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.starts_with("## "))
        .map_or(lines.len(), |(index, _)| index);
    Some(lines[start..end].join("\n"))
}

fn bounded_utf8_prefix(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }
    let mut end = 0;
    for (index, character) in value.char_indices() {
        let next = index + character.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    (&value[..end], true)
}

fn section_for_line(content: &str, line: usize) -> String {
    content
        .lines()
        .take(line + 1)
        .filter(|value| value.starts_with("## "))
        .last()
        .map(|value| value.trim_start_matches("## ").to_owned())
        .unwrap_or_else(|| "§0".to_owned())
}

#[cfg(test)]
mod tests;
