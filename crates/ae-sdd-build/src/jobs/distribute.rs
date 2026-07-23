use super::*;

pub(super) fn plan(
    input: &DistributeInput,
    roots: &AllowedRoots,
) -> Result<Vec<Promotion>, JobError> {
    if input.target_directories.is_empty() || input.target_directories.len() > 32 {
        return Err(JobError::InvalidField("targetDirectories"));
    }
    let mut unique = BTreeSet::new();
    let mut planned = Vec::with_capacity(input.target_directories.len());
    for target in &input.target_directories {
        let canonical_target = roots.destination(target)?;
        if !unique.insert(canonical_target.clone()) {
            return Err(JobError::InvalidField("targetDirectories"));
        }
        planned.push(super::planner::plan_directory(
            &input.package_directory,
            &canonical_target,
            roots,
            &[],
        )?);
    }
    Ok(planned)
}
