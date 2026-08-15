use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_methodology::{
    MethodologyAssetSource, compile_catalog, encode_bundle, verify_builtin_coverage,
};
use serde::Serialize;

use super::filesystem::validate_relative;
use super::planner::{collect_source_files, plan_directory_from_inventory};
use super::*;

const BUILD_MANIFEST_PATH: &str = "runtime/build-manifest.json";
const METHODOLOGY_CATALOG_PATH: &str = "standards/runtime/methodology-catalog.v1.json";
const METHODOLOGY_BUNDLE_PATH: &str = "runtime/methodology/catalog.v1.json";

struct InventoryAssets<'a>(BTreeMap<String, &'a [u8]>);

impl MethodologyAssetSource for InventoryAssets<'_> {
    fn read(&self, path: &ProjectRelativePath) -> Option<&[u8]> {
        self.0.get(path.as_str()).copied()
    }
}

struct CompiledMethodologyOutput {
    change: AdminChange,
    manifest: MethodologyManifest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MethodologyManifest {
    schema_version: &'static str,
    bundle_path: &'static str,
    bundle_digest: String,
    catalog_digest: String,
    entry_count: usize,
    entries: Vec<MethodologyManifestEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MethodologyManifestEntry {
    skill_id: String,
    series_kind: String,
    variant: String,
    entry_digest: String,
    compact_path: String,
    compact_digest: String,
    fallback_path: Option<String>,
    fallback_digest: Option<String>,
}

pub(super) fn plan(input: &CompileInput, roots: &AllowedRoots) -> Result<Promotion, JobError> {
    let source = roots.existing(&input.source_directory)?;
    if !source.join("SKILL.md").is_file() {
        return Err(JobError::InvalidSource(
            source.join("SKILL.md").display().to_string(),
        ));
    }
    let inventory = collect_source_files(&source)?;
    let methodology = compile_methodology_bundle(&inventory)?;
    let (methodology_bundle, methodology_manifest) = methodology.map_or_else(
        || (None, None),
        |output| (Some(output.change), Some(output.manifest)),
    );
    let mut package_paths = BTreeSet::new();
    let mut package_manifest = Vec::with_capacity(
        inventory.len() + input.generated_configs.len() + usize::from(methodology_bundle.is_some()),
    );
    for (relative, _, bytes, permission) in &inventory {
        let path = display_path(relative);
        if path == BUILD_MANIFEST_PATH || !package_paths.insert(path.clone()) {
            return Err(JobError::InvalidRelativePath(path));
        }
        package_manifest.push(BTreeMap::from([
            ("path", path),
            ("digest", sha256_hex(bytes)),
            ("permission", format!("{permission:?}")),
        ]));
    }
    let mut generated = Vec::with_capacity(
        input.generated_configs.len() + 1 + usize::from(methodology_bundle.is_some()),
    );
    if let Some(bundle) = methodology_bundle {
        register_generated(
            &mut package_paths,
            &mut package_manifest,
            &mut generated,
            bundle,
        )?;
    }
    for config in &input.generated_configs {
        register_generated(
            &mut package_paths,
            &mut package_manifest,
            &mut generated,
            AdminChange {
                relative_path: PathBuf::from(&config.relative_path),
                contents: config.contents.clone(),
                permission: config.permission,
            },
        )?;
    }
    package_manifest.sort_by(|left, right| left["path"].cmp(&right["path"]));
    let mut build_manifest = serde_json::json!({
        "schemaVersion": "ae-sdd-compiled-runtime/v1",
        "manifestKind": "content-addressed-package",
        "sourceFiles": package_manifest,
    });
    if let Some(methodology) = methodology_manifest {
        build_manifest
            .as_object_mut()
            .ok_or(JobError::InvalidField("buildManifest"))?
            .insert("methodology".to_owned(), serde_json::to_value(methodology)?);
    }
    generated.push(AdminChange {
        relative_path: PathBuf::from(BUILD_MANIFEST_PATH),
        contents: serde_json::to_string_pretty(&build_manifest)? + "\n",
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

fn compile_methodology_bundle(
    inventory: &[(PathBuf, PathBuf, Vec<u8>, PermissionClass)],
) -> Result<Option<CompiledMethodologyOutput>, JobError> {
    let Some((_, _, catalog, _)) = inventory
        .iter()
        .find(|(relative, _, _, _)| display_path(relative) == METHODOLOGY_CATALOG_PATH)
    else {
        return Ok(None);
    };
    let asset_values = inventory
        .iter()
        .map(|(relative, _, bytes, _)| (display_path(relative), bytes.as_slice()))
        .collect::<BTreeMap<_, _>>();
    crate::source_slim::validate_catalog_fallback_bindings(catalog, &asset_values).map_err(
        |error| {
            JobError::InvalidSource(format!(
                "invalid source-slim catalog fallback binding: {error}"
            ))
        },
    )?;
    let assets = InventoryAssets(asset_values);
    let bundle = compile_catalog(catalog, &assets)
        .and_then(|bundle| {
            verify_builtin_coverage(&bundle)?;
            Ok(bundle)
        })
        .map_err(|error| {
            JobError::InvalidSource(format!("invalid Methodology Catalog: {error}"))
        })?;
    let encoded = encode_bundle(&bundle).map_err(|error| {
        JobError::InvalidSource(format!("cannot encode Methodology bundle: {error}"))
    })?;
    let entries = bundle
        .entries()
        .iter()
        .map(|entry| MethodologyManifestEntry {
            skill_id: entry.skill_id().to_string(),
            series_kind: entry.series_kind().to_string(),
            variant: entry.variant().to_string(),
            entry_digest: entry.entry_digest().to_string(),
            compact_path: entry.compact_ref().path().to_string(),
            compact_digest: entry.compact_ref().digest().to_string(),
            fallback_path: entry
                .fallback_ref()
                .map(|reference| reference.path().to_string()),
            fallback_digest: entry
                .fallback_ref()
                .map(|reference| reference.digest().to_string()),
        })
        .collect();
    let manifest = MethodologyManifest {
        schema_version: "ae-sdd-methodology-manifest/v1",
        bundle_path: METHODOLOGY_BUNDLE_PATH,
        bundle_digest: sha256_hex(&encoded),
        catalog_digest: bundle.catalog_digest().to_string(),
        entry_count: bundle.entry_count(),
        entries,
    };
    let contents = String::from_utf8(encoded)
        .map_err(|_| JobError::InvalidSource("Methodology bundle is not UTF-8 JSON".to_owned()))?;
    Ok(Some(CompiledMethodologyOutput {
        change: AdminChange {
            relative_path: PathBuf::from(METHODOLOGY_BUNDLE_PATH),
            contents,
            permission: PermissionClass::PrivateFile,
        },
        manifest,
    }))
}

fn register_generated(
    package_paths: &mut BTreeSet<String>,
    package_manifest: &mut Vec<BTreeMap<&'static str, String>>,
    generated: &mut Vec<AdminChange>,
    change: AdminChange,
) -> Result<(), JobError> {
    validate_relative(&change.relative_path)?;
    let path = display_path(&change.relative_path);
    ProjectRelativePath::new(path.clone())
        .map_err(|_| JobError::InvalidRelativePath(path.clone()))?;
    if path == BUILD_MANIFEST_PATH || !package_paths.insert(path.clone()) {
        return Err(JobError::InvalidRelativePath(path));
    }
    package_manifest.push(BTreeMap::from([
        ("path", path),
        ("digest", sha256_hex(change.contents.as_bytes())),
        ("permission", format!("{:?}", change.permission)),
    ]));
    generated.push(change);
    Ok(())
}
