use super::*;

pub(super) fn plan(input: &InstallInput, roots: &AllowedRoots) -> Result<Vec<Promotion>, JobError> {
    Ok(vec![super::planner::plan_directory(
        &input.package_directory,
        &input.target_directory,
        roots,
        &[],
    )?])
}
