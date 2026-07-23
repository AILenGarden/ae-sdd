use super::filesystem::{display_path, permission_for, read_bounded, sha256_hex};
use super::*;

pub(super) fn plan_directory(
    source: &Path,
    target: &Path,
    roots: &AllowedRoots,
    generated: &[AdminChange],
) -> Result<Promotion, JobError> {
    let canonical_source = roots.existing(source)?;
    let inventory = collect_source_files(&canonical_source)?;
    plan_directory_from_inventory(&canonical_source, target, roots, inventory, generated)
}

pub(super) fn plan_directory_from_inventory(
    source: &Path,
    target: &Path,
    roots: &AllowedRoots,
    inventory: Vec<(PathBuf, PathBuf, Vec<u8>, PermissionClass)>,
    generated: &[AdminChange],
) -> Result<Promotion, JobError> {
    let canonical_target = roots.destination(target)?;
    if canonical_target.starts_with(source) || source.starts_with(&canonical_target) {
        return Err(JobError::OverlappingTrees(
            display_path(source),
            display_path(&canonical_target),
        ));
    }
    let mut files = Vec::with_capacity(inventory.len() + generated.len());
    for (relative, source_path, bytes, permission) in inventory {
        files.push(materialized(
            canonical_target.join(relative),
            Some(source_path),
            bytes,
            permission,
        ));
    }
    for change in generated {
        validate_relative(&change.relative_path)?;
        files.push(materialized(
            canonical_target.join(&change.relative_path),
            None,
            change.contents.as_bytes().to_vec(),
            change.permission,
        ));
    }
    Ok(Promotion::Directory {
        target: canonical_target,
        files,
    })
}

pub(super) fn plan_overlay(
    root: &Path,
    changes: &[AdminChange],
    roots: &AllowedRoots,
) -> Result<Promotion, JobError> {
    let canonical_root = roots.existing(root)?;
    if changes.is_empty() {
        return Err(JobError::PlanBudgetExceeded);
    }
    let mut files = Vec::with_capacity(changes.len());
    for change in changes {
        validate_relative(&change.relative_path)?;
        files.push(materialized(
            canonical_root.join(&change.relative_path),
            None,
            change.contents.as_bytes().to_vec(),
            change.permission,
        ));
    }
    Ok(Promotion::Overlay {
        root: canonical_root,
        files,
    })
}

pub(super) fn collect_source_files(
    root: &Path,
) -> Result<Vec<(PathBuf, PathBuf, Vec<u8>, PermissionClass)>, JobError> {
    if !root.is_dir() {
        return Err(JobError::InvalidSource(display_path(root)));
    }
    let mut output = Vec::new();
    collect_directory(root, root, &mut output)?;
    output.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(output)
}

pub(super) fn collect_directory(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(PathBuf, PathBuf, Vec<u8>, PermissionClass)>,
) -> Result<(), JobError> {
    for entry in fs::read_dir(directory).map_err(|source| io_error(directory, source))? {
        let entry = entry.map_err(|source| io_error(directory, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| io_error(&path, source))?;
        if file_type.is_symlink() {
            return Err(JobError::SymbolicLink(display_path(&path)));
        }
        if file_type.is_dir() {
            collect_directory(root, &path, output)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| JobError::Containment(display_path(&path)))?
                .to_path_buf();
            let bytes = read_bounded(&path)?;
            output.push((relative, path.clone(), bytes, permission_for(&path)?));
        } else {
            return Err(JobError::InvalidSource(display_path(&path)));
        }
    }
    Ok(())
}

pub(super) fn materialized(
    destination: PathBuf,
    source: Option<PathBuf>,
    bytes: Vec<u8>,
    permission: PermissionClass,
) -> MaterializedChange {
    MaterializedChange {
        digest: sha256_hex(&bytes),
        destination,
        source,
        bytes,
        permission,
    }
}
