use super::*;

pub(super) fn plan(input: &MigrateInput, roots: &AllowedRoots) -> Result<Vec<Promotion>, JobError> {
    Ok(vec![super::planner::plan_directory(
        &input.source_directory,
        &input.target_directory,
        roots,
        &[],
    )?])
}
