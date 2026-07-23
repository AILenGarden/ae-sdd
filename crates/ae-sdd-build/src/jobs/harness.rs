use super::planner::materialized;
use super::*;

pub(super) fn plan(input: &HarnessInput, roots: &AllowedRoots) -> Result<Promotion, JobError> {
    if input.title.trim().is_empty() || input.title.contains(['\0', '\r', '\n']) {
        return Err(JobError::InvalidField("title"));
    }
    if input.source_files.is_empty() || input.source_files.len() > 256 {
        return Err(JobError::InvalidField("sourceFiles"));
    }
    let target = roots.destination(&input.target_file)?;
    let root = target
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| JobError::Containment(display_path(&target)))?;
    let mut body = format!("# {}\n\n", input.title);
    let mut seen = BTreeSet::new();
    for source in &input.source_files {
        let canonical = roots.existing(source)?;
        if !canonical.is_file() || !seen.insert(canonical.clone()) {
            return Err(JobError::InvalidSource(display_path(source)));
        }
        let bytes = read_bounded(&canonical)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| JobError::InvalidSource(display_path(&canonical)))?;
        body.push_str("<!-- ae-sdd:harness-source path=\"");
        body.push_str(&display_path(&canonical));
        body.push_str("\" sha256=\"");
        body.push_str(&sha256_hex(&bytes));
        body.push_str("\" -->\n");
        body.push_str(text);
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push('\n');
    }
    Ok(Promotion::Overlay {
        root,
        files: vec![materialized(
            target,
            None,
            body.into_bytes(),
            PermissionClass::PrivateFile,
        )],
    })
}
