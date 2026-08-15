//! Deterministic source-SKILL slim entry refresh and validation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::source_slim_frontmatter::{is_true, normalize_text, parse_frontmatter};
use crate::source_slim_renderer::{RenderError, RenderRequest, render_slim_entry};
use crate::source_slim_security::{
    ApprovedSourceRoot, EntryKind, FileSnapshot, path_to_posix, read_bytes,
    validated_fallback_path, validated_skill_path, write_bytes_atomically,
};

pub const SOURCE_SLIM_SCHEMA: &str = "ae-sdd-source-slim/v2";

const SOURCE_SLIM_TEMPLATE: &str = "templates/skill/source-skill-slim-entry-template.md";
const SOURCE_SLIM_CATALOG: &str = "standards/runtime/methodology-catalog.v1.json";
const ROOT_SKILL: &str = "SKILL.md";
const ROOT_SKILL_FALLBACK: &str = "skill-fallbacks/SKILL.full.md";
const REQUIRED_SECTIONS: [&str; 5] = [
    "## Load Contract",
    "## Semantic Inventory",
    "## Source Slimming SOP",
    "## Headings",
    "## Inline References",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceSlimMode {
    Refresh,
    Validate,
}

impl SourceSlimMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::Validate => "validate",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSlimRequest {
    pub source_root: PathBuf,
    pub skills: Vec<PathBuf>,
    pub mode: SourceSlimMode,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSlimEntry {
    pub source: String,
    pub fallback: String,
    pub changed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSlimExecution {
    pub schema_version: &'static str,
    pub mode: SourceSlimMode,
    pub entries: Vec<SourceSlimEntry>,
}

#[derive(Clone, Debug)]
struct PendingWrite {
    source_relative: PathBuf,
    source_snapshot: FileSnapshot,
    fallback_relative: PathBuf,
    fallback_snapshot: FileSnapshot,
    expected: Vec<u8>,
}

#[derive(Clone, Debug)]
struct PresentCatalogBindings {
    snapshot: FileSnapshot,
    values: BTreeMap<String, Option<String>>,
}

#[derive(Clone, Debug)]
enum CatalogBindings {
    Absent,
    Present(PresentCatalogBindings),
}

#[derive(Debug, Error)]
pub enum SourceSlimError {
    #[error("source root is not a directory: {path}")]
    SourceRootNotDirectory { path: String },
    #[error("source root contains a symlink, reparse point, or mount: {path}")]
    SourceRootContainsLink { path: String },
    #[error("source root changed during refresh: {path}")]
    SourceRootChangedDuringRefresh { path: String },
    #[error("source slim requires at least one --skill entry")]
    NoSkills,
    #[error("skill path must be a relative Markdown path without traversal: {path}")]
    InvalidSkillPath { path: String },
    #[error("skill entry must be source/SKILL.md or source/skills/**/*.md: {path}")]
    UnsupportedSkillPath { path: String },
    #[error("source entry escapes the source root: {path}")]
    SourceEntryEscapesRoot { path: String },
    #[error("source entry escapes the skills root: {path}")]
    SourceEntryEscapesSkillsRoot { path: String },
    #[error("source entry contains a symlink or reparse point: {path}")]
    SourceEntryContainsLink { path: String },
    #[error("source entry is not a regular file: {path}")]
    SourceEntryNotFile { path: String },
    #[error("source entry has no supported frontmatter: {path}")]
    MissingFrontmatter { path: String },
    #[error("source entry frontmatter is invalid at {path}: {reason}")]
    InvalidFrontmatter { path: String, reason: String },
    #[error("source entry is not a slim entry: {path}")]
    NotSlimmed { path: String },
    #[error("root SKILL.md must be refreshed from its fixed fallback before validation")]
    RootSkillBootstrapRequired,
    #[error("source entry has no source_fallback metadata: {path}")]
    MissingFallback { path: String },
    #[error("source fallback must be a relative path without traversal: {path}")]
    InvalidFallbackPath { path: String },
    #[error("source fallback must live under skill-fallbacks: {path}")]
    UnsupportedFallbackPath { path: String },
    #[error("source entry cannot use itself as its fallback: {path}")]
    SelfReferentialFallback { path: String },
    #[error("source fallback escapes the source root: {path}")]
    FallbackEscapesRoot { path: String },
    #[error("source fallback escapes the skill-fallbacks root: {path}")]
    FallbackEscapesFallbackRoot { path: String },
    #[error("source fallback contains a symlink or reparse point: {path}")]
    FallbackContainsLink { path: String },
    #[error("source fallback is not a regular file: {path}")]
    FallbackNotFile { path: String },
    #[error("source fallback is already slimmed: {path}")]
    SlimmedFallback { path: String },
    #[error("supporting source-slim path escapes the source root: {path}")]
    SupportingPathEscapesRoot { path: String },
    #[error("supporting source-slim path contains a symlink or reparse point: {path}")]
    SupportingPathContainsLink { path: String },
    #[error("supporting source-slim path is not a regular file: {path}")]
    SupportingPathNotFile { path: String },
    #[error("source entry changed after refresh preflight: {path}")]
    SourceEntryChangedDuringRefresh { path: String },
    #[error("source fallback changed after refresh preflight: {path}")]
    FallbackChangedDuringRefresh { path: String },
    #[error("source-slim template changed after refresh preflight")]
    TemplateChangedDuringRefresh,
    #[error("Methodology Catalog changed after refresh preflight")]
    CatalogChangedDuringRefresh,
    #[error("source-slim template is invalid at {path}: {reason}")]
    TemplateInvalid { path: String, reason: String },
    #[error("source semantic inventory is invalid at {path}: {reason}")]
    SemanticInventoryInvalid { path: String, reason: String },
    #[error("Methodology Catalog binding is invalid: {reason}")]
    CatalogBindingInvalid { reason: String },
    #[error("catalog fallback mismatch for {entry}: expected {expected}, got {actual}")]
    CatalogFallbackMismatch {
        entry: String,
        expected: String,
        actual: String,
    },
    #[error("slim entry does not match the deterministic renderer: {path}")]
    RenderedMismatch { path: String },
    #[error("could not parse mount table: {reason}")]
    MountTableInvalid { reason: String },
    #[error("failed to read or write {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize the semantic inventory: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Refreshes or validates explicitly selected source-SKILL slim entries.
pub fn execute_source_slim(
    request: &SourceSlimRequest,
) -> Result<SourceSlimExecution, SourceSlimError> {
    execute_source_slim_with_pre_commit(request, || {})
}

fn execute_source_slim_with_pre_commit(
    request: &SourceSlimRequest,
    before_commit: impl FnOnce(),
) -> Result<SourceSlimExecution, SourceSlimError> {
    if request.skills.is_empty() {
        return Err(SourceSlimError::NoSkills);
    }

    let root = ApprovedSourceRoot::open(&request.source_root)?;
    let fallback_root =
        root.resolve_existing_directory(Path::new("skill-fallbacks"), EntryKind::Fallback)?;
    let template_relative = Path::new(SOURCE_SLIM_TEMPLATE);
    let template_path = root.resolve_existing_file(template_relative, EntryKind::Supporting)?;
    let template_bytes = read_bytes(&template_path)?;
    let template_snapshot = FileSnapshot::capture(template_path, &template_bytes)?;
    let template = source_text_from_bytes(template_relative, &template_bytes)?;
    let catalog = load_catalog_bindings(&root)?;

    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(request.skills.len());
    let mut pending_writes = Vec::new();
    for requested in &request.skills {
        let relative = validated_skill_path(requested)?;
        let source_key = path_to_posix(&relative);
        if !seen.insert(source_key.clone()) {
            continue;
        }

        let source_path = root.resolve_existing_file(&relative, EntryKind::Source)?;
        if relative != Path::new(ROOT_SKILL) {
            let skills_root =
                root.resolve_existing_directory(Path::new("skills"), EntryKind::Source)?;
            if !source_path.starts_with(&skills_root) {
                return Err(SourceSlimError::SourceEntryEscapesSkillsRoot { path: source_key });
            }
        }
        let source_bytes = read_bytes(&source_path)?;
        let source_snapshot = FileSnapshot::capture(source_path, &source_bytes)?;
        let source_text = source_text_from_bytes(&relative, &source_bytes)?;
        let source_frontmatter = required_frontmatter(&source_text, &relative)?;
        let fallback_relative = fallback_for_source(&relative, &source_frontmatter, request.mode)?;
        let fallback_path = root.resolve_existing_file(&fallback_relative, EntryKind::Fallback)?;
        if !fallback_path.starts_with(&fallback_root) {
            return Err(SourceSlimError::FallbackEscapesFallbackRoot {
                path: path_to_posix(&fallback_relative),
            });
        }
        let fallback_bytes = read_bytes(&fallback_path)?;
        let fallback_snapshot = FileSnapshot::capture(fallback_path, &fallback_bytes)?;
        let fallback_text = source_text_from_bytes(&fallback_relative, &fallback_bytes)?;
        let fallback_frontmatter = required_frontmatter(&fallback_text, &fallback_relative)?;
        if is_true(fallback_frontmatter.metadata.get("source_slimmed")) {
            return Err(SourceSlimError::SlimmedFallback {
                path: path_to_posix(&fallback_relative),
            });
        }
        validate_catalog_binding(&catalog, &relative, &fallback_relative)?;
        let root_name = root
            .canonical()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("source");
        let expected = render_slim_entry(RenderRequest {
            root_name,
            source: &relative,
            fallback: &fallback_relative,
            fallback_text: &fallback_text,
            template: &template,
        })
        .map_err(|error| render_error(template_relative, &fallback_relative, error))?;
        let changed =
            source_text.as_bytes() != expected.as_bytes() || !has_required_sections(&source_text);

        match request.mode {
            SourceSlimMode::Refresh if changed => pending_writes.push(PendingWrite {
                source_relative: relative.clone(),
                source_snapshot,
                fallback_relative: fallback_relative.clone(),
                fallback_snapshot,
                expected: expected.into_bytes(),
            }),
            SourceSlimMode::Validate if changed => {
                return Err(SourceSlimError::RenderedMismatch { path: source_key });
            }
            SourceSlimMode::Refresh | SourceSlimMode::Validate => {}
        }
        entries.push(SourceSlimEntry {
            source: source_key,
            fallback: path_to_posix(&fallback_relative),
            changed,
        });
    }

    before_commit();
    revalidate_shared_inputs(&root, &template_snapshot, &catalog)?;
    for pending in &pending_writes {
        revalidate_shared_inputs(&root, &template_snapshot, &catalog)?;
        revalidate_pending_write(&root, &fallback_root, pending)?;
        write_bytes_atomically(&root, &pending.source_relative, &pending.expected)?;
    }

    Ok(SourceSlimExecution {
        schema_version: SOURCE_SLIM_SCHEMA,
        mode: request.mode,
        entries,
    })
}

fn fallback_for_source(
    relative: &Path,
    frontmatter: &crate::source_slim_frontmatter::Frontmatter,
    mode: SourceSlimMode,
) -> Result<PathBuf, SourceSlimError> {
    if is_true(frontmatter.metadata.get("source_slimmed")) {
        let fallback = frontmatter
            .metadata
            .get("source_fallback")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| SourceSlimError::MissingFallback {
                path: path_to_posix(relative),
            })?;
        let candidate = PathBuf::from(fallback);
        if candidate == relative {
            return Err(SourceSlimError::SelfReferentialFallback {
                path: path_to_posix(relative),
            });
        }
        return validated_fallback_path(fallback);
    }
    if relative == Path::new(ROOT_SKILL) {
        return match mode {
            SourceSlimMode::Refresh => Ok(PathBuf::from(ROOT_SKILL_FALLBACK)),
            SourceSlimMode::Validate => Err(SourceSlimError::RootSkillBootstrapRequired),
        };
    }
    Err(SourceSlimError::NotSlimmed {
        path: path_to_posix(relative),
    })
}

fn required_frontmatter(
    text: &str,
    path: &Path,
) -> Result<crate::source_slim_frontmatter::Frontmatter, SourceSlimError> {
    parse_frontmatter(text)
        .map_err(|reason| SourceSlimError::InvalidFrontmatter {
            path: path_to_posix(path),
            reason,
        })?
        .ok_or_else(|| SourceSlimError::MissingFrontmatter {
            path: path_to_posix(path),
        })
}

fn source_text_from_bytes(path: &Path, bytes: &[u8]) -> Result<String, SourceSlimError> {
    let text = std::str::from_utf8(bytes).map_err(|error| SourceSlimError::Io {
        path: path_to_posix(path),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, error),
    })?;
    Ok(normalize_text(text))
}

fn has_required_sections(text: &str) -> bool {
    REQUIRED_SECTIONS
        .iter()
        .all(|section| text.contains(section))
}

fn revalidate_shared_inputs(
    root: &ApprovedSourceRoot,
    template_snapshot: &FileSnapshot,
    catalog: &CatalogBindings,
) -> Result<(), SourceSlimError> {
    let template =
        root.resolve_existing_file(Path::new(SOURCE_SLIM_TEMPLATE), EntryKind::Supporting)?;
    let template_bytes = read_bytes(&template)?;
    if !template_snapshot.matches(&template, &template_bytes)? {
        return Err(SourceSlimError::TemplateChangedDuringRefresh);
    }
    match catalog {
        CatalogBindings::Absent => {
            if root
                .existing_file_if_present(Path::new(SOURCE_SLIM_CATALOG), EntryKind::Supporting)?
                .is_some()
            {
                return Err(SourceSlimError::CatalogChangedDuringRefresh);
            }
        }
        CatalogBindings::Present(catalog) => {
            let path =
                root.resolve_existing_file(Path::new(SOURCE_SLIM_CATALOG), EntryKind::Supporting)?;
            let bytes = read_bytes(&path)?;
            if !catalog.snapshot.matches(&path, &bytes)? {
                return Err(SourceSlimError::CatalogChangedDuringRefresh);
            }
        }
    }
    Ok(())
}

fn revalidate_pending_write(
    root: &ApprovedSourceRoot,
    fallback_root: &Path,
    pending: &PendingWrite,
) -> Result<(), SourceSlimError> {
    let current_fallback_root =
        root.resolve_existing_directory(Path::new("skill-fallbacks"), EntryKind::Fallback)?;
    if current_fallback_root != fallback_root {
        return Err(SourceSlimError::FallbackChangedDuringRefresh {
            path: "skill-fallbacks".to_owned(),
        });
    }
    revalidate_file_snapshot(
        root,
        &pending.source_relative,
        &pending.source_snapshot,
        EntryKind::Source,
    )?;
    let fallback_path = revalidate_file_snapshot(
        root,
        &pending.fallback_relative,
        &pending.fallback_snapshot,
        EntryKind::Fallback,
    )?;
    if !fallback_path.starts_with(fallback_root) {
        return Err(SourceSlimError::FallbackChangedDuringRefresh {
            path: path_to_posix(&pending.fallback_relative),
        });
    }
    Ok(())
}

fn revalidate_file_snapshot(
    root: &ApprovedSourceRoot,
    relative: &Path,
    snapshot: &FileSnapshot,
    kind: EntryKind,
) -> Result<PathBuf, SourceSlimError> {
    let path = root.resolve_existing_file(relative, kind)?;
    let bytes = read_bytes(&path)?;
    if snapshot.matches(&path, &bytes)? {
        return Ok(path);
    }
    Err(match kind {
        EntryKind::Source => SourceSlimError::SourceEntryChangedDuringRefresh {
            path: path_to_posix(relative),
        },
        EntryKind::Fallback => SourceSlimError::FallbackChangedDuringRefresh {
            path: path_to_posix(relative),
        },
        EntryKind::Supporting => SourceSlimError::TemplateChangedDuringRefresh,
    })
}

fn load_catalog_bindings(root: &ApprovedSourceRoot) -> Result<CatalogBindings, SourceSlimError> {
    let relative = Path::new(SOURCE_SLIM_CATALOG);
    let Some(path) = root.existing_file_if_present(relative, EntryKind::Supporting)? else {
        return Ok(CatalogBindings::Absent);
    };
    let bytes = read_bytes(&path)?;
    let snapshot = FileSnapshot::capture(path, &bytes)?;
    Ok(CatalogBindings::Present(PresentCatalogBindings {
        snapshot,
        values: parse_catalog_bindings(&bytes)?,
    }))
}

fn validate_catalog_binding(
    bindings: &CatalogBindings,
    source: &Path,
    fallback: &Path,
) -> Result<(), SourceSlimError> {
    let CatalogBindings::Present(bindings) = bindings else {
        return Ok(());
    };
    let Some(expected) = bindings.values.get(&path_to_posix(source)) else {
        return Ok(());
    };
    let Some(expected) = expected else {
        return Err(SourceSlimError::CatalogBindingInvalid {
            reason: format!("{} has no fallbackRef", path_to_posix(source)),
        });
    };
    let expected_path = validated_fallback_path(expected)?;
    if expected_path != fallback {
        return Err(SourceSlimError::CatalogFallbackMismatch {
            entry: path_to_posix(source),
            expected: path_to_posix(&expected_path),
            actual: path_to_posix(fallback),
        });
    }
    Ok(())
}

fn parse_catalog_bindings(
    bytes: &[u8],
) -> Result<BTreeMap<String, Option<String>>, SourceSlimError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| SourceSlimError::CatalogBindingInvalid {
            reason: error.to_string(),
        })?;
    let entries = value
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| SourceSlimError::CatalogBindingInvalid {
            reason: "catalog has no entries array".to_owned(),
        })?;
    let mut bindings = BTreeMap::new();
    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| SourceSlimError::CatalogBindingInvalid {
                reason: "catalog entry is not an object".to_owned(),
            })?;
        let compact = object
            .get("compactRef")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SourceSlimError::CatalogBindingInvalid {
                reason: "catalog entry has no compactRef".to_owned(),
            })?;
        let fallback = match object.get("fallbackRef") {
            Some(serde_json::Value::String(value)) => Some(value.to_owned()),
            Some(serde_json::Value::Null) => None,
            _ => {
                return Err(SourceSlimError::CatalogBindingInvalid {
                    reason: format!("catalog entry {compact} has invalid fallbackRef"),
                });
            }
        };
        if bindings.insert(compact.to_owned(), fallback).is_some() {
            return Err(SourceSlimError::CatalogBindingInvalid {
                reason: format!("duplicate compactRef {compact}"),
            });
        }
    }
    Ok(bindings)
}

pub(crate) fn validate_catalog_fallback_bindings(
    catalog: &[u8],
    assets: &BTreeMap<String, &[u8]>,
) -> Result<(), SourceSlimError> {
    for (compact, fallback) in parse_catalog_bindings(catalog)? {
        if compact != ROOT_SKILL && !compact.starts_with("skills/") {
            continue;
        }
        let Some(bytes) = assets.get(&compact) else {
            continue;
        };
        let text = source_text_from_bytes(Path::new(&compact), bytes)?;
        let Some(frontmatter) =
            parse_frontmatter(&text).map_err(|reason| SourceSlimError::InvalidFrontmatter {
                path: compact.clone(),
                reason,
            })?
        else {
            continue;
        };
        if !is_true(frontmatter.metadata.get("source_slimmed")) {
            continue;
        }
        let actual = frontmatter.metadata.get("source_fallback").ok_or_else(|| {
            SourceSlimError::MissingFallback {
                path: compact.clone(),
            }
        })?;
        let actual_path = validated_fallback_path(actual)?;
        let expected = fallback.ok_or_else(|| SourceSlimError::CatalogBindingInvalid {
            reason: format!("{compact} is slimmed but catalog fallbackRef is null"),
        })?;
        let expected_path = validated_fallback_path(&expected)?;
        if actual_path != expected_path {
            return Err(SourceSlimError::CatalogFallbackMismatch {
                entry: compact,
                expected: path_to_posix(&expected_path),
                actual: path_to_posix(&actual_path),
            });
        }
    }
    Ok(())
}

fn render_error(template: &Path, fallback: &Path, error: RenderError) -> SourceSlimError {
    match error {
        RenderError::Frontmatter(reason) => SourceSlimError::InvalidFrontmatter {
            path: path_to_posix(fallback),
            reason,
        },
        RenderError::Template(reason) => SourceSlimError::TemplateInvalid {
            path: path_to_posix(template),
            reason,
        },
        RenderError::Semantic(reason) => SourceSlimError::SemanticInventoryInvalid {
            path: path_to_posix(fallback),
            reason,
        },
        RenderError::Serialize(error) => SourceSlimError::Serialize(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::TempDir;

    struct RefreshFixture {
        _temp: TempDir,
        source_root: PathBuf,
        skill_relative: PathBuf,
        skill: PathBuf,
        fallback: PathBuf,
        template: PathBuf,
        stale_entry: String,
    }

    fn refresh_fixture() -> RefreshFixture {
        let temp = TempDir::new().expect("temporary source root");
        let source_root = temp.path().join("source");
        let skill_relative = PathBuf::from("skills/phase1-design/example-skill.md");
        let fallback_relative =
            PathBuf::from("skill-fallbacks/skills/phase1-design/example-skill.full.md");
        let skill = source_root.join(&skill_relative);
        let fallback = source_root.join(&fallback_relative);
        let template = source_root.join(SOURCE_SLIM_TEMPLATE);
        fs::create_dir_all(skill.parent().expect("skill parent")).expect("skill directory");
        fs::create_dir_all(fallback.parent().expect("fallback parent"))
            .expect("fallback directory");
        fs::create_dir_all(template.parent().expect("template parent"))
            .expect("template directory");
        fs::write(
            &template,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../source/templates/skill/source-skill-slim-entry-template.md"
            )),
        )
        .expect("template");
        fs::write(
            &fallback,
            "---\nname: example\n---\n\n# Example\n\nOriginal fallback text.\n",
        )
        .expect("fallback");
        let stale_entry = "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n".to_owned();
        fs::write(&skill, &stale_entry).expect("stale slim entry");

        RefreshFixture {
            _temp: temp,
            source_root,
            skill_relative,
            skill,
            fallback,
            template,
            stale_entry,
        }
    }

    #[test]
    fn refresh_rejects_fallback_drift_after_preflight() {
        let fixture = refresh_fixture();

        let error = execute_source_slim_with_pre_commit(
            &SourceSlimRequest {
                source_root: fixture.source_root.clone(),
                skills: vec![fixture.skill_relative.clone()],
                mode: SourceSlimMode::Refresh,
            },
            || {
                fs::write(
                    &fixture.fallback,
                    "---\nname: example\n---\n\n# Example\n\nChanged fallback text.\n",
                )
                .expect("concurrent fallback update");
            },
        )
        .expect_err("refresh must reject fallback drift");

        assert!(matches!(
            error,
            SourceSlimError::FallbackChangedDuringRefresh { .. }
        ));
        assert_eq!(
            fs::read_to_string(fixture.skill).expect("stale entry"),
            fixture.stale_entry
        );
    }

    #[test]
    fn refresh_rejects_source_drift_after_preflight() {
        let fixture = refresh_fixture();

        let error = execute_source_slim_with_pre_commit(
            &SourceSlimRequest {
                source_root: fixture.source_root.clone(),
                skills: vec![fixture.skill_relative.clone()],
                mode: SourceSlimMode::Refresh,
            },
            || {
                fs::write(&fixture.skill, format!("{}\n", fixture.stale_entry))
                    .expect("concurrent source update");
            },
        )
        .expect_err("refresh must reject source drift");

        assert!(matches!(
            error,
            SourceSlimError::SourceEntryChangedDuringRefresh { .. }
        ));
    }

    #[test]
    fn refresh_rejects_template_drift_after_preflight() {
        let fixture = refresh_fixture();

        let error = execute_source_slim_with_pre_commit(
            &SourceSlimRequest {
                source_root: fixture.source_root.clone(),
                skills: vec![fixture.skill_relative.clone()],
                mode: SourceSlimMode::Refresh,
            },
            || {
                fs::write(&fixture.template, "# Concurrent template update\n")
                    .expect("concurrent template update");
            },
        )
        .expect_err("refresh must reject template drift");

        assert!(matches!(
            error,
            SourceSlimError::TemplateChangedDuringRefresh
        ));
    }

    #[test]
    fn refresh_rejects_a_catalog_created_after_preflight() {
        let fixture = refresh_fixture();
        let catalog = fixture.source_root.join(SOURCE_SLIM_CATALOG);

        let error = execute_source_slim_with_pre_commit(
            &SourceSlimRequest {
                source_root: fixture.source_root.clone(),
                skills: vec![fixture.skill_relative.clone()],
                mode: SourceSlimMode::Refresh,
            },
            || {
                fs::create_dir_all(catalog.parent().expect("catalog parent"))
                    .expect("catalog directory");
                fs::write(&catalog, "{\"entries\": []}\n").expect("concurrent catalog");
            },
        )
        .expect_err("refresh must reject a catalog created after preflight");

        assert!(matches!(
            error,
            SourceSlimError::CatalogChangedDuringRefresh
        ));
        assert_eq!(
            fs::read_to_string(fixture.skill).expect("stale entry"),
            fixture.stale_entry
        );
    }
}
