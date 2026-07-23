use std::fs;
use std::path::{Path, PathBuf};

use ae_sdd_domain::ArtifactDigest;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MANIFEST_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug)]
pub(crate) struct SemanticTarget {
    pub(crate) relative_path: String,
    pub(crate) before_digest: Option<ArtifactDigest>,
    pub(crate) after_bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedEvidence {
    pub(crate) result: Value,
    pub(crate) targets: Vec<SemanticTarget>,
}

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error("evidence operation requires a non-empty Story identity")]
    StoryRequired,
    #[error(
        "evidence artifact is empty, missing, outside the workspace, or not a regular file: {0}"
    )]
    UnsafeArtifact(String),
    #[error("evidence manifest is malformed or has the wrong schema/story: {0}")]
    InvalidManifest(String),
    #[error("evidence manifest integrity check failed")]
    ManifestTampered,
    #[error("active evidence snapshot is missing or does not match its digest: {0}")]
    SnapshotInvalid(String),
    #[error("evidence data could not be serialized")]
    Serialize,
    #[error("evidence filesystem access failed: {0}")]
    Io(#[from] std::io::Error),
}

pub(crate) fn prepare_record(
    workspace: &Path,
    story_id: &str,
    payload: &Value,
    started_at: &str,
) -> Result<PreparedEvidence, EvidenceError> {
    validate_story(story_id)?;
    let root = workspace.canonicalize()?;
    let artifact_value = required_string(payload, "artifactPath")?;
    let source = contained_file(&root, artifact_value)?;
    let source_bytes = fs::read(&source)?;
    let artifact_digest = sha256_prefixed(&source_bytes);
    let source_relative = relative_string(&root, &source)?;
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| EvidenceError::UnsafeArtifact(artifact_value.to_owned()))?;
    let snapshot_relative = format!(
        ".auto-engineering/{story_id}/evidence/artifacts/{}-{file_name}",
        artifact_digest.trim_start_matches("sha256:")
    );
    let snapshot_absolute = root.join(&snapshot_relative);

    let (manifest_relative, mut manifest, manifest_before) = load_manifest(&root, story_id, false)?;
    let entries = manifest
        .get_mut("entries")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| EvidenceError::InvalidManifest("entries must be an array".to_owned()))?;
    let kind = optional_string(payload, "kind").unwrap_or("test");
    let command = payload.get("command").cloned().unwrap_or_else(|| json!(""));
    let command_hash = canonical_fingerprint(&command)?;
    let input_fingerprint = required_string(payload, "inputFingerprint")?;
    let toolchain_fingerprint =
        optional_string(payload, "toolchainFingerprint").unwrap_or("unknown");
    let exit_code = payload.get("exitCode").and_then(Value::as_i64).unwrap_or(0);
    let duration_ms = payload
        .get("durationMs")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let summary = payload.get("summary").cloned().unwrap_or_else(|| json!({}));
    let logical_key = optional_string(payload, "logicalKey")
        .map(str::to_owned)
        .unwrap_or(canonical_fingerprint(&json!({
            "kind": kind,
            "commandHash": command_hash,
            "artifacts": [source_relative],
        }))?);
    let evidence_id = format!(
        "ev-{}",
        &canonical_fingerprint(&json!({
            "kind": kind,
            "command": command,
            "input": input_fingerprint,
        }))?[7..23]
    );

    for previous in entries.iter_mut().filter_map(Value::as_object_mut) {
        let active = previous
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("active")
            == "active";
        if active
            && previous.get("logicalKey").and_then(Value::as_str) == Some(logical_key.as_str())
        {
            previous.insert("status".to_owned(), json!("superseded"));
            previous.insert("supersededAt".to_owned(), json!(started_at));
            previous.insert("supersededBy".to_owned(), json!(evidence_id));
        }
    }
    let entry = json!({
        "evidenceId": evidence_id,
        "kind": kind,
        "commandHash": command_hash,
        "inputFingerprint": input_fingerprint,
        "toolchainFingerprint": toolchain_fingerprint,
        "startedAt": started_at,
        "durationMs": duration_ms,
        "exitCode": exit_code,
        "summary": summary,
        "artifacts": [{
            "path": source_relative,
            "sha256": artifact_digest,
            "snapshotPath": snapshot_relative,
        }],
        "reusable": exit_code == 0,
        "logicalKey": logical_key,
        "status": "active",
    });
    entries.push(entry.clone());
    seal_manifest(&mut manifest)?;

    let mut targets = Vec::new();
    let snapshot_matches = fs::read(&snapshot_absolute)
        .ok()
        .is_some_and(|bytes| bytes == source_bytes);
    if !snapshot_matches {
        targets.push(SemanticTarget {
            relative_path: snapshot_relative,
            before_digest: fs::read(&snapshot_absolute)
                .ok()
                .map(|bytes| ArtifactDigest::digest(&bytes)),
            after_bytes: source_bytes,
        });
    }
    targets.push(SemanticTarget {
        relative_path: manifest_relative,
        before_digest: manifest_before,
        after_bytes: pretty_json(&manifest)?,
    });
    Ok(PreparedEvidence {
        result: entry,
        targets,
    })
}

pub(crate) fn prepare_finalize(
    workspace: &Path,
    story_id: &str,
) -> Result<PreparedEvidence, EvidenceError> {
    validate_story(story_id)?;
    let root = workspace.canonicalize()?;
    let (manifest_relative, mut manifest, manifest_before) = load_manifest(&root, story_id, true)?;
    let entries = manifest
        .get_mut("entries")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| EvidenceError::InvalidManifest("entries must be an array".to_owned()))?;
    for entry in entries.iter_mut().filter_map(Value::as_object_mut) {
        if entry.get("status").and_then(Value::as_str) == Some("superseded") {
            continue;
        }
        let artifacts = entry
            .get_mut("artifacts")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                EvidenceError::InvalidManifest("artifacts must be an array".to_owned())
            })?;
        for artifact in artifacts.iter_mut().filter_map(Value::as_object_mut) {
            let relative = artifact
                .get("snapshotPath")
                .or_else(|| artifact.get("path"))
                .and_then(Value::as_str)
                .ok_or_else(|| EvidenceError::SnapshotInvalid("missing path".to_owned()))?;
            let path = contained_relative_file(&root, relative)?;
            let bytes = fs::read(&path)?;
            let actual = sha256_prefixed(&bytes);
            if artifact
                .get("sha256")
                .and_then(Value::as_str)
                .is_some_and(|expected| expected != actual)
            {
                return Err(EvidenceError::SnapshotInvalid(relative.to_owned()));
            }
            artifact.insert("sha256".to_owned(), json!(actual));
        }
    }
    let entry_count = entries.len();
    seal_manifest(&mut manifest)?;
    Ok(PreparedEvidence {
        result: json!({"manifest": manifest_relative, "entryCount": entry_count}),
        targets: vec![SemanticTarget {
            relative_path: manifest_relative,
            before_digest: manifest_before,
            after_bytes: pretty_json(&manifest)?,
        }],
    })
}

fn load_manifest(
    root: &Path,
    story_id: &str,
    required: bool,
) -> Result<(String, Value, Option<ArtifactDigest>), EvidenceError> {
    let relative = format!(".auto-engineering/{story_id}/evidence/manifest.json");
    let absolute = root.join(&relative);
    if !absolute.is_file() {
        if required {
            return Err(EvidenceError::InvalidManifest(
                "manifest does not exist".to_owned(),
            ));
        }
        return Ok((
            relative,
            json!({"schemaVersion": MANIFEST_SCHEMA_VERSION, "storyId": story_id, "entries": []}),
            None,
        ));
    }
    let bytes = fs::read(&absolute)?;
    let before = Some(ArtifactDigest::digest(&bytes));
    let manifest: Value = serde_json::from_slice(&bytes)
        .map_err(|_| EvidenceError::InvalidManifest("invalid JSON".to_owned()))?;
    let object = manifest
        .as_object()
        .ok_or_else(|| EvidenceError::InvalidManifest("root must be an object".to_owned()))?;
    if object.get("schemaVersion").and_then(Value::as_u64) != Some(MANIFEST_SCHEMA_VERSION)
        || object.get("storyId").and_then(Value::as_str) != Some(story_id)
        || !object.get("entries").is_some_and(Value::is_array)
    {
        return Err(EvidenceError::InvalidManifest(
            "schemaVersion, storyId, or entries mismatch".to_owned(),
        ));
    }
    if let Some(expected) = object.get("contentHash").and_then(Value::as_str)
        && expected != manifest_content_hash(&manifest)?
    {
        return Err(EvidenceError::ManifestTampered);
    }
    Ok((relative, manifest, before))
}

fn seal_manifest(manifest: &mut Value) -> Result<(), EvidenceError> {
    let content_hash = manifest_content_hash(manifest)?;
    manifest
        .as_object_mut()
        .ok_or_else(|| EvidenceError::InvalidManifest("root must be an object".to_owned()))?
        .insert("contentHash".to_owned(), json!(content_hash));
    Ok(())
}

fn manifest_content_hash(manifest: &Value) -> Result<String, EvidenceError> {
    let mut payload = manifest.clone();
    let object = payload
        .as_object_mut()
        .ok_or_else(|| EvidenceError::InvalidManifest("root must be an object".to_owned()))?;
    object.retain(|key, _| key != "contentHash" && !key.starts_with('_'));
    canonical_fingerprint(&payload)
}

fn contained_file(root: &Path, raw: &str) -> Result<PathBuf, EvidenceError> {
    let requested = Path::new(raw);
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    let resolved = candidate
        .canonicalize()
        .map_err(|_| EvidenceError::UnsafeArtifact(raw.to_owned()))?;
    if !resolved.is_file() || resolved.strip_prefix(root).is_err() {
        return Err(EvidenceError::UnsafeArtifact(raw.to_owned()));
    }
    Ok(resolved)
}

fn contained_relative_file(root: &Path, raw: &str) -> Result<PathBuf, EvidenceError> {
    let relative = Path::new(raw);
    if raw.trim().is_empty() || relative.is_absolute() || raw.replace('\\', "/").contains("../") {
        return Err(EvidenceError::SnapshotInvalid(raw.to_owned()));
    }
    contained_file(root, raw).map_err(|_| EvidenceError::SnapshotInvalid(raw.to_owned()))
}

fn relative_string(root: &Path, path: &Path) -> Result<String, EvidenceError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| EvidenceError::UnsafeArtifact(path.display().to_string()))
}

fn validate_story(story_id: &str) -> Result<(), EvidenceError> {
    if story_id.trim().is_empty()
        || Path::new(story_id).is_absolute()
        || story_id.contains('/')
        || story_id.contains('\\')
        || story_id == "."
        || story_id == ".."
    {
        Err(EvidenceError::StoryRequired)
    } else {
        Ok(())
    }
}

fn required_string<'a>(payload: &'a Value, field: &str) -> Result<&'a str, EvidenceError> {
    optional_string(payload, field)
        .ok_or_else(|| EvidenceError::UnsafeArtifact(format!("missing {field}")))
}

fn optional_string<'a>(payload: &'a Value, field: &str) -> Option<&'a str> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn canonical_fingerprint(value: &Value) -> Result<String, EvidenceError> {
    let bytes = serde_json::to_vec(value).map_err(|_| EvidenceError::Serialize)?;
    Ok(sha256_prefixed(&bytes))
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn pretty_json(value: &Value) -> Result<Vec<u8>, EvidenceError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|_| EvidenceError::Serialize)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn materialize(root: &Path, prepared: &PreparedEvidence) {
        for target in &prepared.targets {
            let path = root.join(&target.relative_path);
            fs::create_dir_all(path.parent().expect("target parent")).expect("target directory");
            fs::write(path, &target.after_bytes).expect("target write");
        }
    }

    #[test]
    fn record_snapshots_and_supersedes_the_same_logical_key() {
        let root = tempdir().expect("workspace");
        fs::create_dir_all(root.path().join("results")).expect("results");
        fs::write(root.path().join("results/test.json"), b"{\"pass\":true}\n").expect("artifact");
        let payload = json!({
            "artifactPath":"results/test.json",
            "inputFingerprint":"sha256:input",
            "kind":"test",
            "command":["cargo","test"],
            "toolchainFingerprint":"rust-1",
            "logicalKey":"tests/core",
        });
        let first = prepare_record(root.path(), "STORY-001", &payload, "2026-07-23T00:00:00Z")
            .expect("first record");
        assert_eq!(first.targets.len(), 2);
        assert!(first.targets[0].before_digest.is_none());
        materialize(root.path(), &first);
        let second = prepare_record(root.path(), "STORY-001", &payload, "2026-07-23T00:01:00Z")
            .expect("second record");
        assert_eq!(
            second.targets.len(),
            1,
            "content-addressed snapshot is reused"
        );
        materialize(root.path(), &second);
        let manifest: Value = serde_json::from_slice(
            &fs::read(
                root.path()
                    .join(".auto-engineering/STORY-001/evidence/manifest.json"),
            )
            .expect("manifest"),
        )
        .expect("manifest JSON");
        assert_eq!(manifest["entries"][0]["status"], "superseded");
        assert_eq!(manifest["entries"][1]["status"], "active");
        assert_eq!(manifest["entries"][1]["logicalKey"], "tests/core");
    }

    #[test]
    fn finalize_seals_active_snapshots_and_rejects_tampering() {
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("result.json"), b"{}\n").expect("artifact");
        let prepared = prepare_record(
            root.path(),
            "STORY-001",
            &json!({"artifactPath":"result.json","inputFingerprint":"sha256:input"}),
            "2026-07-23T00:00:00Z",
        )
        .expect("record");
        materialize(root.path(), &prepared);
        let finalized = prepare_finalize(root.path(), "STORY-001").expect("finalize");
        assert_eq!(finalized.result["entryCount"], 1);
        materialize(root.path(), &finalized);
        let snapshot = prepared.result["artifacts"][0]["snapshotPath"]
            .as_str()
            .expect("snapshot");
        fs::write(root.path().join(snapshot), b"tampered\n").expect("tamper");
        assert!(matches!(
            prepare_finalize(root.path(), "STORY-001"),
            Err(EvidenceError::SnapshotInvalid(_))
        ));
    }

    #[test]
    fn record_rejects_manifest_content_hash_tampering() {
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("result.json"), b"{}\n").expect("artifact");
        let prepared = prepare_record(
            root.path(),
            "STORY-001",
            &json!({"artifactPath":"result.json","inputFingerprint":"sha256:input"}),
            "2026-07-23T00:00:00Z",
        )
        .expect("record");
        materialize(root.path(), &prepared);
        let manifest_path = root
            .path()
            .join(".auto-engineering/STORY-001/evidence/manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("manifest")).expect("JSON");
        manifest["storyId"] = json!("STORY-TAMPERED");
        fs::write(&manifest_path, pretty_json(&manifest).expect("JSON bytes")).expect("tamper");
        assert!(
            prepare_record(
                root.path(),
                "STORY-001",
                &json!({
                    "artifactPath":"result.json","inputFingerprint":"sha256:input"
                }),
                "2026-07-23T00:01:00Z"
            )
            .is_err()
        );
    }
}
