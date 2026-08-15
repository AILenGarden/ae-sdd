//! Filesystem containment and atomic-write support for source slimming.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use atomicwrites::{AllowOverwrite, AtomicFile};
use sha2::{Digest, Sha256};

use crate::source_slim::SourceSlimError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileSnapshot {
    canonical_path: PathBuf,
    content_digest: [u8; 32],
    permission_fingerprint: u64,
}

impl FileSnapshot {
    pub(crate) fn capture(path: PathBuf, bytes: &[u8]) -> Result<Self, SourceSlimError> {
        let metadata = fs::metadata(&path).map_err(|source| io_error(&path, source))?;
        Ok(Self {
            canonical_path: path,
            content_digest: Sha256::digest(bytes).into(),
            permission_fingerprint: permission_fingerprint(&metadata),
        })
    }

    pub(crate) fn matches(&self, path: &Path, bytes: &[u8]) -> Result<bool, SourceSlimError> {
        let metadata = fs::metadata(path).map_err(|source| io_error(path, source))?;
        let content_digest: [u8; 32] = Sha256::digest(bytes).into();
        Ok(path == self.canonical_path
            && content_digest == self.content_digest
            && permission_fingerprint(&metadata) == self.permission_fingerprint)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum EntryKind {
    Source,
    Fallback,
    Supporting,
}

/// A source root whose original input path and canonical identity remain
/// checked before every read-to-write promotion.
#[derive(Clone, Debug)]
pub(crate) struct ApprovedSourceRoot {
    requested: PathBuf,
    canonical: PathBuf,
    mount_table: MountTable,
}

impl ApprovedSourceRoot {
    pub(crate) fn open(path: &Path) -> Result<Self, SourceSlimError> {
        let requested = absolute_path(path)?;
        let mount_table = MountTable::load()?;
        reject_input_links(&requested, &mount_table)?;
        let metadata =
            fs::symlink_metadata(&requested).map_err(|source| io_error(&requested, source))?;
        if !metadata.is_dir() {
            return Err(SourceSlimError::SourceRootNotDirectory {
                path: requested.display().to_string(),
            });
        }
        if is_link_or_reparse(&metadata) || is_mount_boundary(&requested, &metadata, &mount_table)?
        {
            return Err(SourceSlimError::SourceRootContainsLink {
                path: requested.display().to_string(),
            });
        }
        let canonical =
            fs::canonicalize(&requested).map_err(|source| io_error(&requested, source))?;
        Ok(Self {
            requested,
            canonical,
            mount_table,
        })
    }

    pub(crate) fn canonical(&self) -> &Path {
        &self.canonical
    }

    pub(crate) fn ensure_current(&self) -> Result<(), SourceSlimError> {
        reject_input_links(&self.requested, &self.mount_table).map_err(|_| {
            SourceSlimError::SourceRootChangedDuringRefresh {
                path: self.requested.display().to_string(),
            }
        })?;
        let metadata = fs::symlink_metadata(&self.requested)
            .map_err(|source| io_error(&self.requested, source))?;
        if !metadata.is_dir()
            || is_link_or_reparse(&metadata)
            || is_mount_boundary(&self.requested, &metadata, &self.mount_table)?
        {
            return Err(SourceSlimError::SourceRootChangedDuringRefresh {
                path: self.requested.display().to_string(),
            });
        }
        let canonical = fs::canonicalize(&self.requested)
            .map_err(|source| io_error(&self.requested, source))?;
        if canonical != self.canonical {
            return Err(SourceSlimError::SourceRootChangedDuringRefresh {
                path: self.requested.display().to_string(),
            });
        }
        Ok(())
    }

    pub(crate) fn resolve_existing_file(
        &self,
        relative: &Path,
        kind: EntryKind,
    ) -> Result<PathBuf, SourceSlimError> {
        self.ensure_current()?;
        self.reject_linked_path(relative, kind)?;
        let requested = self.canonical.join(relative);
        let resolved =
            fs::canonicalize(&requested).map_err(|source| io_error(&requested, source))?;
        if !resolved.starts_with(&self.canonical) {
            return Err(escape_error(kind, relative));
        }
        if !resolved.is_file() {
            return Err(not_file_error(kind, relative));
        }
        Ok(resolved)
    }

    pub(crate) fn resolve_existing_directory(
        &self,
        relative: &Path,
        kind: EntryKind,
    ) -> Result<PathBuf, SourceSlimError> {
        self.ensure_current()?;
        self.reject_linked_path(relative, kind)?;
        let requested = self.canonical.join(relative);
        let resolved =
            fs::canonicalize(&requested).map_err(|source| io_error(&requested, source))?;
        if !resolved.starts_with(&self.canonical) || !resolved.is_dir() {
            return Err(escape_error(kind, relative));
        }
        Ok(resolved)
    }

    pub(crate) fn existing_file_if_present(
        &self,
        relative: &Path,
        kind: EntryKind,
    ) -> Result<Option<PathBuf>, SourceSlimError> {
        self.ensure_current()?;
        let candidate = self.canonical.join(relative);
        match fs::symlink_metadata(&candidate) {
            Ok(_) => self.resolve_existing_file(relative, kind).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_error(&candidate, error)),
        }
    }

    fn reject_linked_path(&self, relative: &Path, kind: EntryKind) -> Result<(), SourceSlimError> {
        let mut current = self.canonical.clone();
        for component in relative.components() {
            let Component::Normal(segment) = component else {
                return Err(contains_link_error(kind, relative));
            };
            current.push(segment);
            let metadata =
                fs::symlink_metadata(&current).map_err(|source| io_error(&current, source))?;
            if is_link_or_reparse(&metadata)
                || is_mount_boundary(&current, &metadata, &self.mount_table)?
            {
                return Err(contains_link_error(kind, relative));
            }
        }
        Ok(())
    }
}

pub(crate) fn read_bytes(path: &Path) -> Result<Vec<u8>, SourceSlimError> {
    fs::read(path).map_err(|source| io_error(path, source))
}

pub(crate) fn write_bytes_atomically(
    root: &ApprovedSourceRoot,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), SourceSlimError> {
    // Resolve the relative target immediately before promotion so a preflight
    // path cannot be reused after a parent link or reparse-point substitution.
    let path = root.resolve_existing_file(relative, EntryKind::Source)?;
    let permissions = fs::metadata(&path)
        .map_err(|source| io_error(&path, source))?
        .permissions();
    AtomicFile::new(&path, AllowOverwrite)
        .write(|file| {
            file.write_all(bytes)?;
            file.set_permissions(permissions.clone())?;
            file.sync_all()
        })
        .map_err(|source| SourceSlimError::Io {
            path: path.display().to_string(),
            source: std::io::Error::from(source),
        })?;
    root.ensure_current()?;
    sync_parent(&path)
}

pub(crate) fn path_to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn validated_skill_path(path: &Path) -> Result<PathBuf, SourceSlimError> {
    if !is_clean_relative_path(path) || path.extension().is_none_or(|extension| extension != "md") {
        return Err(SourceSlimError::InvalidSkillPath {
            path: path.display().to_string(),
        });
    }
    let allowed = path == Path::new("SKILL.md")
        || path
            .strip_prefix("skills")
            .is_ok_and(|relative| !relative.as_os_str().is_empty());
    if !allowed {
        return Err(SourceSlimError::UnsupportedSkillPath {
            path: path.display().to_string(),
        });
    }
    Ok(path.to_path_buf())
}

pub(crate) fn validated_fallback_path(value: &str) -> Result<PathBuf, SourceSlimError> {
    let path = PathBuf::from(value);
    if !is_clean_relative_path(&path) {
        return Err(SourceSlimError::InvalidFallbackPath {
            path: value.to_owned(),
        });
    }
    if !path
        .strip_prefix("skill-fallbacks")
        .is_ok_and(|relative| !relative.as_os_str().is_empty())
    {
        return Err(SourceSlimError::UnsupportedFallbackPath {
            path: value.to_owned(),
        });
    }
    Ok(path)
}

fn is_clean_relative_path(path: &Path) -> bool {
    path.to_str().is_some_and(|raw| {
        !raw.is_empty()
            && raw
                .split(['/', '\\'])
                .all(is_portable_relative_path_component)
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    })
}

fn is_portable_relative_path_component(component: &str) -> bool {
    if component.is_empty()
        || matches!(component, "." | "..")
        || component.ends_with('.')
        || component.ends_with(' ')
        || component
            .chars()
            .any(|character| character.is_control() || character == ':')
    {
        return false;
    }

    let device_stem = component
        .split_once('.')
        .map_or(component, |(stem, _)| stem)
        .to_ascii_uppercase();
    !matches!(
        device_stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "CLOCK$"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn absolute_path(path: &Path) -> Result<PathBuf, SourceSlimError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map_err(|source| io_error(path, source))
            .map(|current| current.join(path))
    }
}

fn reject_input_links(path: &Path, mount_table: &MountTable) -> Result<(), SourceSlimError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if !current.exists() {
            break;
        }
        let metadata =
            fs::symlink_metadata(&current).map_err(|source| io_error(&current, source))?;
        if is_link_or_reparse(&metadata) {
            return Err(SourceSlimError::SourceRootContainsLink {
                path: path.display().to_string(),
            });
        }
        if current == path && is_mount_boundary(&current, &metadata, mount_table)? {
            return Err(SourceSlimError::SourceRootContainsLink {
                path: path.display().to_string(),
            });
        }
    }
    Ok(())
}

fn escape_error(kind: EntryKind, relative: &Path) -> SourceSlimError {
    let path = path_to_posix(relative);
    match kind {
        EntryKind::Source => SourceSlimError::SourceEntryEscapesRoot { path },
        EntryKind::Fallback => SourceSlimError::FallbackEscapesRoot { path },
        EntryKind::Supporting => SourceSlimError::SupportingPathEscapesRoot { path },
    }
}

fn contains_link_error(kind: EntryKind, relative: &Path) -> SourceSlimError {
    let path = path_to_posix(relative);
    match kind {
        EntryKind::Source => SourceSlimError::SourceEntryContainsLink { path },
        EntryKind::Fallback => SourceSlimError::FallbackContainsLink { path },
        EntryKind::Supporting => SourceSlimError::SupportingPathContainsLink { path },
    }
}

fn not_file_error(kind: EntryKind, relative: &Path) -> SourceSlimError {
    let path = path_to_posix(relative);
    match kind {
        EntryKind::Source => SourceSlimError::SourceEntryNotFile { path },
        EntryKind::Fallback => SourceSlimError::FallbackNotFile { path },
        EntryKind::Supporting => SourceSlimError::SupportingPathNotFile { path },
    }
}

fn io_error(path: &Path, source: std::io::Error) -> SourceSlimError {
    SourceSlimError::Io {
        path: path.display().to_string(),
        source,
    }
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn is_mount_boundary(
    path: &Path,
    metadata: &fs::Metadata,
    mount_table: &MountTable,
) -> Result<bool, SourceSlimError> {
    if !metadata.is_dir() {
        return Ok(false);
    }
    if mount_table.contains(path) {
        return Ok(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let Some(parent) = path.parent() else {
            return Ok(false);
        };
        let parent_metadata = fs::metadata(parent).map_err(|source| io_error(parent, source))?;
        return Ok(metadata.dev() != parent_metadata.dev());
    }
    #[cfg(not(unix))]
    {
        Ok(false)
    }
}

#[cfg(unix)]
fn permission_fingerprint(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::PermissionsExt;

    u64::from(metadata.permissions().mode())
}

#[cfg(not(unix))]
fn permission_fingerprint(metadata: &fs::Metadata) -> u64 {
    u64::from(metadata.permissions().readonly())
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), SourceSlimError> {
    use std::fs::File;

    let parent = path.parent().ok_or_else(|| SourceSlimError::Io {
        path: path.display().to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "file has no parent"),
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error(parent, source))
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), SourceSlimError> {
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct MountTable {
    points: BTreeSet<PathBuf>,
}

impl MountTable {
    fn load() -> Result<Self, SourceSlimError> {
        #[cfg(target_os = "linux")]
        {
            let source = Path::new("/proc/self/mountinfo");
            let text = fs::read_to_string(source).map_err(|error| io_error(source, error))?;
            return Self::parse(&text);
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self::default())
        }
    }

    fn contains(&self, path: &Path) -> bool {
        self.points.contains(path)
    }

    #[cfg(target_os = "linux")]
    fn parse(text: &str) -> Result<Self, SourceSlimError> {
        let mut points = BTreeSet::new();
        for line in text.lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let mount_point = fields
                .get(4)
                .ok_or_else(|| SourceSlimError::MountTableInvalid {
                    reason: "mountinfo entry has no mount point".to_owned(),
                })?;
            points.insert(unescape_mountinfo(mount_point)?);
        }
        Ok(Self { points })
    }
}

#[cfg(target_os = "linux")]
fn unescape_mountinfo(value: &str) -> Result<PathBuf, SourceSlimError> {
    use std::os::unix::ffi::OsStringExt;

    let mut output = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            output.push(bytes[index]);
            index += 1;
            continue;
        }
        if index + 3 >= bytes.len() {
            return Err(SourceSlimError::MountTableInvalid {
                reason: "truncated mountinfo escape".to_owned(),
            });
        }
        let digits = std::str::from_utf8(&bytes[index + 1..index + 4]).map_err(|_| {
            SourceSlimError::MountTableInvalid {
                reason: "invalid mountinfo escape".to_owned(),
            }
        })?;
        let decoded =
            u8::from_str_radix(digits, 8).map_err(|_| SourceSlimError::MountTableInvalid {
                reason: "invalid mountinfo escape".to_owned(),
            })?;
        output.push(decoded);
        index += 4;
    }
    Ok(PathBuf::from(std::ffi::OsString::from_vec(output)))
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn mount_table_detects_a_same_device_bind_mount() {
        let table = MountTable::parse(
            "42 31 8:1 /outside /workspace/source/skills rw,relatime - ext4 /dev/sda rw\n",
        )
        .expect("mount table");
        assert!(table.contains(Path::new("/workspace/source/skills")));
    }
}
