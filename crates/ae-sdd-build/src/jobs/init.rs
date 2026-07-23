use super::*;

pub(super) fn plan(input: &InitInput, roots: &AllowedRoots) -> Result<Vec<Promotion>, JobError> {
    Ok(vec![super::planner::plan_overlay(
        &input.project_root,
        &input.changes,
        roots,
    )?])
}
