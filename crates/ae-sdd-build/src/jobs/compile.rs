use super::planner::{collect_source_files, plan_directory_from_inventory};
use super::*;

pub(super) fn plan(input: &CompileInput, roots: &AllowedRoots) -> Result<Promotion, JobError> {
    let mut generated = Vec::with_capacity(input.generated_configs.len() + 1);
    for config in &input.generated_configs {
        generated.push(AdminChange {
            relative_path: PathBuf::from(&config.relative_path),
            contents: config.contents.clone(),
            permission: config.permission,
        });
    }
    let source = roots.existing(&input.source_directory)?;
    if !source.join("SKILL.md").is_file() {
        return Err(JobError::InvalidSource(
            source.join("SKILL.md").display().to_string(),
        ));
    }
    let inventory = collect_source_files(&source)?;
    let source_manifest: Vec<_> = inventory
        .iter()
        .map(|(relative, _, bytes, permission)| {
            BTreeMap::from([
                ("path", display_path(relative)),
                ("digest", sha256_hex(bytes)),
                ("permission", format!("{permission:?}")),
            ])
        })
        .collect();
    generated.push(AdminChange {
        relative_path: PathBuf::from("runtime/build-manifest.json"),
        contents: serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": "ae-sdd-compiled-runtime/v1",
            "sourceFiles": source_manifest,
        }))? + "\n",
        permission: PermissionClass::PrivateFile,
    });
    plan_directory_from_inventory(
        &source,
        &input.output_directory,
        roots,
        inventory,
        &generated,
    )
}
