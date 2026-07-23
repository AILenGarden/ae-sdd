use std::collections::BTreeSet;
use std::path::{Component, Path};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum VerificationPlanError {
    #[error("verification.plan requires a non-empty Story identity")]
    StoryRequired,
    #[error(
        "changed path is empty, absolute, parent-relative, missing, or outside the workspace: {0}"
    )]
    UnsafePath(String),
    #[error("changed path is not a regular file: {0}")]
    NotFile(String),
    #[error("changedPaths must contain at least one file")]
    EmptyPaths,
    #[error("verification plan could not be serialized")]
    Serialize,
}

pub(crate) fn build_verification_plan(
    workspace: &Path,
    story_id: &str,
    work_item_id: &str,
    changed_paths: &[Value],
    since_fingerprint: &str,
) -> Result<Value, VerificationPlanError> {
    if story_id.trim().is_empty() {
        return Err(VerificationPlanError::StoryRequired);
    }
    let paths = validate_changed_paths(workspace, changed_paths)?;
    let classes = paths
        .iter()
        .map(|path| classify_path(path))
        .collect::<BTreeSet<_>>();
    let modules = paths
        .iter()
        .filter_map(|path| path.split('/').next())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let (required, deferred, not_required) = verification_requirements(&classes);
    let evidence_input_fingerprint = canonical_fingerprint(&json!({
        "storyId": story_id,
        "workItem": work_item_id,
        "changedPaths": paths,
        "sinceFingerprint": since_fingerprint,
    }))?;
    let plan_fingerprint = canonical_fingerprint(&json!({
        "storyId": story_id,
        "paths": paths,
        "classes": classes,
    }))?;
    Ok(json!({
        "schemaVersion": 1,
        "storyId": story_id,
        "workItem": work_item_id,
        "sinceFingerprint": since_fingerprint,
        "changeClass": classes,
        "affectedModules": modules,
        "required": required,
        "deferredUntilFinal": deferred,
        "notRequired": not_required,
        "planFingerprint": plan_fingerprint,
        "inputFingerprint": evidence_input_fingerprint,
        "evidenceInputFingerprint": evidence_input_fingerprint,
        "changedPaths": paths,
        "nextActions": [{
            "operation": "evidence.record",
            "inputFingerprint": evidence_input_fingerprint,
            "command": "ae-sdd ops execute --request <evidence-record-request.json>",
        }],
    }))
}

fn validate_changed_paths(
    workspace: &Path,
    changed_paths: &[Value],
) -> Result<Vec<String>, VerificationPlanError> {
    let root = workspace
        .canonicalize()
        .map_err(|_| VerificationPlanError::UnsafePath(workspace.display().to_string()))?;
    let mut normalized = BTreeSet::new();
    for item in changed_paths {
        let raw = item
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| VerificationPlanError::UnsafePath(item.to_string()))?;
        let relative = Path::new(raw);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(VerificationPlanError::UnsafePath(raw.to_owned()));
        }
        let resolved = root
            .join(relative)
            .canonicalize()
            .map_err(|_| VerificationPlanError::UnsafePath(raw.to_owned()))?;
        let canonical = resolved
            .strip_prefix(&root)
            .map_err(|_| VerificationPlanError::UnsafePath(raw.to_owned()))?;
        if !resolved.is_file() {
            return Err(VerificationPlanError::NotFile(raw.to_owned()));
        }
        normalized.insert(canonical.to_string_lossy().replace('\\', "/"));
    }
    if normalized.is_empty() {
        return Err(VerificationPlanError::EmptyPaths);
    }
    Ok(normalized.into_iter().collect())
}

fn classify_path(path: &str) -> &'static str {
    let value = path.to_ascii_lowercase();
    if value.ends_with(".md") || value.ends_with(".rst") || value.ends_with(".adoc") {
        "documentation"
    } else if value.ends_with("test.java")
        || value.ends_with("tests.py")
        || value.ends_with("_test.py")
        || value.ends_with(".test.ts")
        || value.ends_with(".spec.ts")
        || value.ends_with("_test.rs")
        || value.contains("/tests/")
        || value.starts_with("tests/")
    {
        "test-code"
    } else if value.ends_with("pom.xml")
        || value.ends_with("build.gradle")
        || value.ends_with("build.gradle.kts")
        || value.ends_with("cargo.toml")
        || value.ends_with("cargo.lock")
        || value.ends_with(".yml")
        || value.ends_with(".yaml")
        || value.ends_with(".properties")
        || value.ends_with(".toml")
    {
        "build-or-config"
    } else if [".java", ".kt", ".py", ".js", ".ts", ".go", ".cs", ".rs"]
        .iter()
        .any(|suffix| value.ends_with(suffix))
    {
        "production-code"
    } else {
        "other"
    }
}

fn verification_requirements(
    classes: &BTreeSet<&'static str>,
) -> (Vec<&'static str>, Vec<&'static str>, Vec<&'static str>) {
    let mut required = Vec::new();
    let mut deferred = Vec::new();
    let mut not_required = Vec::new();
    if classes.contains("production-code") {
        required.extend(["focused-test", "module-test", "G-09", "G-CODE-1-delta"]);
        deferred.push("full-story-regression");
    }
    if classes.contains("test-code") {
        required.push("affected-tests");
        deferred.push("final-story-test-suite");
    }
    if classes.contains("build-or-config") {
        required.extend(["affected-module-package", "package-after-test"]);
    }
    if classes.contains("documentation")
        && !classes
            .iter()
            .any(|class| matches!(*class, "production-code" | "test-code" | "build-or-config"))
    {
        required.extend(["document-schema", "AC/TC-mapping"]);
        not_required.push("Maven/full-story-regression");
    }
    if required.is_empty() {
        required.push("targeted-validation");
    }
    required.dedup();
    deferred.dedup();
    not_required.dedup();
    (required, deferred, not_required)
}

fn canonical_fingerprint(value: &Value) -> Result<String, VerificationPlanError> {
    let bytes = serde_json::to_vec(value).map_err(|_| VerificationPlanError::Serialize)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn plan_is_sorted_scoped_and_binds_evidence_to_the_work_item() {
        let root = tempdir().expect("workspace");
        fs::create_dir_all(root.path().join("crates/core/tests")).expect("test directory");
        fs::write(root.path().join("crates/core/src.rs"), "fn main() {}\n").expect("source");
        fs::write(
            root.path().join("crates/core/tests/unit_test.rs"),
            "#[test] fn it_works() {}\n",
        )
        .expect("test");
        let plan = build_verification_plan(
            root.path(),
            "STORY-001",
            "STORY-001",
            &[
                Value::String("crates/core/tests/unit_test.rs".to_owned()),
                Value::String("crates/core/src.rs".to_owned()),
            ],
            "before",
        )
        .expect("plan");
        assert_eq!(
            plan["changedPaths"],
            json!(["crates/core/src.rs", "crates/core/tests/unit_test.rs"])
        );
        assert_eq!(plan["changeClass"], json!(["production-code", "test-code"]));
        assert_eq!(plan["affectedModules"], json!(["crates"]));
        assert_eq!(plan["inputFingerprint"], plan["evidenceInputFingerprint"]);
        assert_ne!(plan["inputFingerprint"], plan["planFingerprint"]);
        assert_eq!(plan["nextActions"][0]["operation"], "evidence.record");
    }

    #[test]
    fn plan_rejects_escape_missing_and_empty_paths() {
        let root = tempdir().expect("workspace");
        for paths in [
            vec![Value::String("../outside.rs".to_owned())],
            vec![Value::String("missing.rs".to_owned())],
            Vec::new(),
        ] {
            assert!(
                build_verification_plan(root.path(), "STORY-001", "STORY-001", &paths, "").is_err()
            );
        }
    }
}
