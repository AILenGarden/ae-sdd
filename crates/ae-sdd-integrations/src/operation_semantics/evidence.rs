//! Evidence operation semantics: append-only ledger plus manifest projection.
//!
//! `.auto-engineering/{storyId}/evidence/ledger.jsonl` is the evidence truth.
//! Every `evidence.record`/`evidence.finalize` appends one canonical JSON
//! event per line (hash chain, see `ae_sdd_contracts::evidence`) and rebuilds
//! `manifest.json` as the deterministic active projection sealed with
//! `contentHash`. Historical entries are never modified in place: supersede
//! and finalize are new events. A legacy manifest without a ledger stays
//! read-compatible and gains a ledger only on the next record. All file
//! updates are returned as [`SemanticTarget`]s so the caller commits them
//! through the project mutation journal; nothing here appends directly.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use ae_sdd_contracts::{EvidenceLedgerEventKind, EvidenceLedgerEventV1, MAX_LEDGER_EVENTS};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EvidenceId, InputFingerprint, ProjectRelativePath,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MANIFEST_SCHEMA_VERSION: u64 = 1;
const MAX_EVIDENCE_FILE_BYTES: usize = 1_048_576;
const ENTRY_ARTIFACT_KIND: &str = "evidence-entry";
const SNAPSHOT_ARTIFACT_KIND: &str = "evidence-snapshot";
const MANIFEST_ARTIFACT_KIND: &str = "evidence-manifest";

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
    /// `evidenceAuthority` state projection: ledger/manifest locator + digest.
    pub(crate) authority: Value,
}

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error("evidence operation requires a non-empty Story identity")]
    StoryRequired,
    #[error(
        "evidence artifact is empty, missing, outside the workspace, or not a regular file: {0}"
    )]
    UnsafeArtifact(String),
    #[error("evidence logical key must be non-empty and within its byte limit")]
    InvalidLogicalKey,
    #[error("evidence manifest is malformed or has the wrong schema/story: {0}")]
    InvalidManifest(String),
    #[error("evidence manifest integrity check failed")]
    ManifestTampered,
    #[error("evidence ledger hash chain is missing, malformed, or tampered: {0}")]
    LedgerTampered(String),
    #[error("active evidence snapshot is missing or does not match its digest: {0}")]
    SnapshotInvalid(String),
    #[error("evidence data could not be serialized")]
    Serialize,
    #[error("evidence filesystem access failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Verified ledger state: the typed events plus the exact bytes they were
/// loaded from, so appends can extend the file byte-for-byte. The Review
/// authority keeps a mirrored standalone loader in `review_authority.rs`.
struct LedgerState {
    events: Vec<EvidenceLedgerEventV1>,
    bytes: Vec<u8>,
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

    let ledger = load_ledger(&root, story_id)?;
    let (manifest_relative, manifest, manifest_before) = load_manifest(&root, story_id, false)?;
    let mut entries = project_entries(
        &root,
        ledger.as_ref(),
        residue_entries(&manifest, ledger.as_ref()),
    )?;

    let input = typed_input_fingerprint(input_fingerprint);
    let mut new_events = Vec::new();
    let mut sequence = ledger
        .as_ref()
        .map_or(1, |state| state.events.len() as u64 + 1);
    let mut previous = ledger
        .as_ref()
        .and_then(|state| state.events.last().map(EvidenceLedgerEventV1::event_digest));
    let supersedes = entries.iter().rev().any(|entry| {
        entry_is_active(entry) && entry_logical_key(entry) == Some(logical_key.as_str())
    });
    if supersedes {
        let superseded = EvidenceLedgerEventV1::new(
            sequence,
            lifecycle_event_id("ev-superseded", sequence, previous)?,
            EvidenceLedgerEventKind::Superseded,
            logical_key.as_str(),
            input,
            vec![],
            previous,
        )
        .map_err(ledger_event_error)?;
        previous = Some(superseded.event_digest());
        sequence += 1;
        new_events.push(superseded);
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
    let entry_bytes = pretty_json(&entry)?;
    let entry_digest = ArtifactDigest::digest(&entry_bytes);
    let entry_relative =
        format!(".auto-engineering/{story_id}/evidence/entries/{entry_digest}.json");
    let recorded = EvidenceLedgerEventV1::new(
        sequence,
        EvidenceId::new(evidence_id.clone()).map_err(|_| EvidenceError::Serialize)?,
        EvidenceLedgerEventKind::Recorded,
        logical_key.as_str(),
        input,
        vec![
            artifact_ref(
                ENTRY_ARTIFACT_KIND,
                &entry_relative,
                entry_digest,
                entry_bytes.len(),
            )?,
            artifact_ref(
                SNAPSHOT_ARTIFACT_KIND,
                &snapshot_relative,
                ArtifactDigest::digest(&source_bytes),
                source_bytes.len(),
            )?,
        ],
        previous,
    )
    .map_err(ledger_event_error)?;
    new_events.push(recorded);
    if ledger
        .as_ref()
        .is_some_and(|state| state.events.len() + new_events.len() > MAX_LEDGER_EVENTS)
    {
        return Err(EvidenceError::LedgerTampered(
            "evidence ledger event budget is exhausted".to_owned(),
        ));
    }

    if supersedes {
        let position = entries
            .iter()
            .rposition(|entry| {
                entry_is_active(entry) && entry_logical_key(entry) == Some(logical_key.as_str())
            })
            .ok_or(EvidenceError::Serialize)?;
        if let Some(object) = entries[position].as_object_mut() {
            object.insert("status".to_owned(), json!("superseded"));
            object.insert("supersededBy".to_owned(), json!(evidence_id));
        }
    }
    entries.push(entry.clone());

    let ledger_relative = ledger_relative(story_id);
    let mut ledger_bytes = ledger.map(|state| state.bytes).unwrap_or_default();
    let ledger_before = (!ledger_bytes.is_empty()).then(|| ArtifactDigest::digest(&ledger_bytes));
    for event in &new_events {
        ledger_bytes.extend_from_slice(&event.canonical_json());
        ledger_bytes.push(b'\n');
    }

    let sealed = sealed_manifest(story_id, &entries)?;
    let manifest_bytes = pretty_json(&sealed)?;

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
    let entry_absolute = root.join(&entry_relative);
    if !fs::read(&entry_absolute)
        .ok()
        .is_some_and(|bytes| bytes == entry_bytes)
    {
        targets.push(SemanticTarget {
            relative_path: entry_relative,
            before_digest: fs::read(&entry_absolute)
                .ok()
                .map(|bytes| ArtifactDigest::digest(&bytes)),
            after_bytes: entry_bytes,
        });
    }
    targets.push(SemanticTarget {
        relative_path: ledger_relative.clone(),
        before_digest: ledger_before,
        after_bytes: ledger_bytes.clone(),
    });
    targets.push(SemanticTarget {
        relative_path: manifest_relative.clone(),
        before_digest: manifest_before,
        after_bytes: manifest_bytes.clone(),
    });
    Ok(PreparedEvidence {
        result: entry,
        targets,
        authority: authority_projection(
            Some((ledger_relative.as_str(), ledger_bytes.as_slice())),
            &manifest_relative,
            &manifest_bytes,
        ),
    })
}

pub(crate) fn prepare_finalize(
    workspace: &Path,
    story_id: &str,
) -> Result<PreparedEvidence, EvidenceError> {
    validate_story(story_id)?;
    let root = workspace.canonicalize()?;
    let ledger = load_ledger(&root, story_id)?;
    let (manifest_relative, mut manifest, manifest_before) =
        load_manifest(&root, story_id, ledger.is_none())?;
    let mut entries = project_entries(
        &root,
        ledger.as_ref(),
        residue_entries(&manifest, ledger.as_ref()),
    )?;
    for entry in entries.iter_mut().filter_map(Value::as_object_mut) {
        if entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("active")
            != "active"
        {
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

    let Some(ledger) = ledger else {
        // Legacy manifest without a ledger: verify and seal in place, never
        // rewriting entries, exactly as before the ledger existed.
        manifest["entries"] = json!(entries);
        seal_manifest(&mut manifest)?;
        let manifest_bytes = pretty_json(&manifest)?;
        return Ok(PreparedEvidence {
            result: json!({"manifest": manifest_relative, "entryCount": entry_count}),
            authority: authority_projection(None, &manifest_relative, &manifest_bytes),
            targets: vec![SemanticTarget {
                relative_path: manifest_relative,
                before_digest: manifest_before,
                after_bytes: manifest_bytes,
            }],
        });
    };

    let sealed = sealed_manifest(story_id, &entries)?;
    let manifest_bytes = pretty_json(&sealed)?;
    let ledger_relative = ledger_relative(story_id);
    let sequence = ledger.events.len() as u64 + 1;
    let previous = ledger
        .events
        .last()
        .map(EvidenceLedgerEventV1::event_digest);
    if ledger.events.len() + 1 > MAX_LEDGER_EVENTS {
        return Err(EvidenceError::LedgerTampered(
            "evidence ledger event budget is exhausted".to_owned(),
        ));
    }
    let finalized = EvidenceLedgerEventV1::new(
        sequence,
        lifecycle_event_id("ev-finalized", sequence, previous)?,
        EvidenceLedgerEventKind::Finalized,
        "",
        InputFingerprint::digest(&manifest_bytes),
        vec![artifact_ref(
            MANIFEST_ARTIFACT_KIND,
            &manifest_relative,
            ArtifactDigest::digest(&manifest_bytes),
            manifest_bytes.len(),
        )?],
        previous,
    )
    .map_err(ledger_event_error)?;
    let ledger_before = ArtifactDigest::digest(&ledger.bytes);
    let mut ledger_bytes = ledger.bytes;
    ledger_bytes.extend_from_slice(&finalized.canonical_json());
    ledger_bytes.push(b'\n');
    Ok(PreparedEvidence {
        result: json!({
            "manifest": manifest_relative,
            "entryCount": entry_count,
            "ledger": ledger_relative,
            "eventCount": sequence,
        }),
        authority: authority_projection(
            Some((ledger_relative.as_str(), ledger_bytes.as_slice())),
            &manifest_relative,
            &manifest_bytes,
        ),
        targets: vec![
            SemanticTarget {
                relative_path: ledger_relative,
                before_digest: Some(ledger_before),
                after_bytes: ledger_bytes,
            },
            SemanticTarget {
                relative_path: manifest_relative,
                before_digest: manifest_before,
                after_bytes: manifest_bytes,
            },
        ],
    })
}

fn ledger_relative(story_id: &str) -> String {
    format!(".auto-engineering/{story_id}/evidence/ledger.jsonl")
}

fn load_ledger(root: &Path, story_id: &str) -> Result<Option<LedgerState>, EvidenceError> {
    let absolute = root.join(ledger_relative(story_id));
    if !absolute.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&absolute)?;
    if bytes.is_empty() || bytes.len() > MAX_EVIDENCE_FILE_BYTES {
        return Err(EvidenceError::LedgerTampered(
            "evidence ledger exceeds its durable byte bound".to_owned(),
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| EvidenceError::LedgerTampered("evidence ledger is not UTF-8".to_owned()))?;
    if !text.ends_with('\n') {
        return Err(EvidenceError::LedgerTampered(
            "evidence ledger is truncated".to_owned(),
        ));
    }
    let mut events = Vec::new();
    for line in text.lines() {
        let event: EvidenceLedgerEventV1 = serde_json::from_str(line).map_err(|_| {
            EvidenceError::LedgerTampered("evidence ledger event failed closed decode".to_owned())
        })?;
        if line.as_bytes() != event.canonical_json() {
            return Err(EvidenceError::LedgerTampered(
                "evidence ledger event is not canonical JSON".to_owned(),
            ));
        }
        events.push(event);
    }
    EvidenceLedgerEventV1::verify_chain(&events)
        .map_err(|error| EvidenceError::LedgerTampered(error.to_string()))?;
    Ok(Some(LedgerState { events, bytes }))
}

/// Entries of the current manifest that the ledger does not track: legacy
/// entries and toolset receipts. They stay verbatim at the head of the
/// projection and are never rewritten in place.
fn residue_entries(manifest: &Value, ledger: Option<&LedgerState>) -> Vec<Value> {
    let Some(entries) = manifest.get("entries").and_then(Value::as_array) else {
        return Vec::new();
    };
    let recorded: BTreeSet<&str> = ledger
        .map(|state| {
            state
                .events
                .iter()
                .filter(|event| event.kind() == EvidenceLedgerEventKind::Recorded)
                .map(|event| event.event_id().as_str())
                .collect()
        })
        .unwrap_or_default();
    entries
        .iter()
        .filter(|entry| {
            entry
                .get("evidenceId")
                .and_then(Value::as_str)
                .is_none_or(|id| !recorded.contains(id))
        })
        .cloned()
        .collect()
}

/// Deterministically folds ledger events into the active manifest projection
/// on top of the residue entries.
fn project_entries(
    root: &Path,
    ledger: Option<&LedgerState>,
    residue: Vec<Value>,
) -> Result<Vec<Value>, EvidenceError> {
    let mut entries = residue;
    let Some(ledger) = ledger else {
        return Ok(entries);
    };
    for (index, event) in ledger.events.iter().enumerate() {
        match event.kind() {
            EvidenceLedgerEventKind::Recorded => {
                let mut entry = load_entry_payload(root, event)?;
                entry
                    .as_object_mut()
                    .ok_or_else(|| {
                        EvidenceError::LedgerTampered(
                            "evidence entry artifact must be an object".to_owned(),
                        )
                    })?
                    .insert("status".to_owned(), json!("active"));
                entries.push(entry);
            }
            EvidenceLedgerEventKind::Superseded | EvidenceLedgerEventKind::Invalidated => {
                let position = entries
                    .iter()
                    .rposition(|entry| {
                        entry_is_active(entry)
                            && entry_logical_key(entry) == Some(event.logical_key())
                    })
                    .ok_or_else(|| {
                        EvidenceError::LedgerTampered(format!(
                            "{} event has no active entry for its logical key",
                            event.kind().as_str()
                        ))
                    })?;
                let status = if event.kind() == EvidenceLedgerEventKind::Superseded {
                    "superseded"
                } else {
                    "invalidated"
                };
                if let Some(object) = entries[position].as_object_mut() {
                    object.insert("status".to_owned(), json!(status));
                    if event.kind() == EvidenceLedgerEventKind::Superseded
                        && let Some(successor) = ledger.events[index + 1..].iter().find(|later| {
                            later.kind() == EvidenceLedgerEventKind::Recorded
                                && later.logical_key() == event.logical_key()
                        })
                    {
                        object.insert(
                            "supersededBy".to_owned(),
                            json!(successor.event_id().as_str()),
                        );
                    }
                }
            }
            EvidenceLedgerEventKind::Finalized => {}
        }
    }
    Ok(entries)
}

/// Reads the full entry payload a recorded event binds, verifying the
/// content-addressed artifact digest so a tampered payload fails closed.
fn load_entry_payload(root: &Path, event: &EvidenceLedgerEventV1) -> Result<Value, EvidenceError> {
    let reference = event
        .artifact_refs()
        .iter()
        .find(|reference| reference.kind().as_str() == ENTRY_ARTIFACT_KIND)
        .ok_or_else(|| {
            EvidenceError::LedgerTampered(
                "recorded event does not bind an entry artifact".to_owned(),
            )
        })?;
    let bytes = fs::read(root.join(reference.path().as_str())).map_err(|_| {
        EvidenceError::LedgerTampered("recorded event entry artifact is missing".to_owned())
    })?;
    if bytes.is_empty() || bytes.len() > MAX_EVIDENCE_FILE_BYTES {
        return Err(EvidenceError::LedgerTampered(
            "entry artifact exceeds its durable byte bound".to_owned(),
        ));
    }
    if ArtifactDigest::digest(&bytes) != reference.digest() {
        return Err(EvidenceError::LedgerTampered(
            "entry artifact digest does not match its recorded event".to_owned(),
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| EvidenceError::LedgerTampered("entry artifact is not valid JSON".to_owned()))
}

fn entry_is_active(entry: &Value) -> bool {
    entry
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("active")
        == "active"
}

fn entry_logical_key(entry: &Value) -> Option<&str> {
    entry.get("logicalKey").and_then(Value::as_str)
}

/// Binds the caller-supplied fingerprint string to the typed contract. Digest
/// strings (with or without the `sha256:` prefix) are used verbatim; legacy
/// free-form fingerprints are hashed with a domain separator so the typed
/// event still binds exactly the recorded input.
fn typed_input_fingerprint(raw: &str) -> InputFingerprint {
    InputFingerprint::from_str(raw.strip_prefix("sha256:").unwrap_or(raw)).unwrap_or_else(|_| {
        InputFingerprint::digest(format!("evidence-input-fingerprint\x00{raw}"))
    })
}

/// Deterministic identity for supersede/finalize events: unique per chain
/// position because it binds the sequence and the previous event digest.
fn lifecycle_event_id(
    prefix: &str,
    sequence: u64,
    previous: Option<ArtifactDigest>,
) -> Result<EvidenceId, EvidenceError> {
    let digest = ArtifactDigest::digest(format!(
        "{sequence}:{}",
        previous.map_or_else(|| "genesis".to_owned(), |digest| digest.to_string())
    ));
    let hex = digest.to_string();
    EvidenceId::new(format!("{prefix}-{}", &hex[..16])).map_err(|_| EvidenceError::Serialize)
}

fn artifact_ref(
    kind: &str,
    relative: &str,
    digest: ArtifactDigest,
    byte_length: usize,
) -> Result<ArtifactRef, EvidenceError> {
    Ok(ArtifactRef::new(
        ArtifactKind::new(kind).map_err(|_| EvidenceError::Serialize)?,
        ProjectRelativePath::new(relative.to_owned()).map_err(|_| EvidenceError::Serialize)?,
        digest,
        byte_length as u64,
    ))
}

fn sealed_manifest(story_id: &str, entries: &[Value]) -> Result<Value, EvidenceError> {
    let mut manifest = json!({
        "schemaVersion": MANIFEST_SCHEMA_VERSION,
        "storyId": story_id,
        "entries": entries,
    });
    seal_manifest(&mut manifest)?;
    Ok(manifest)
}

fn authority_projection(
    ledger: Option<(&str, &[u8])>,
    manifest_relative: &str,
    manifest_bytes: &[u8],
) -> Value {
    json!({
        "ledgerRef": ledger.map(|(relative, _)| relative),
        "ledgerDigest": ledger.map(|(_, bytes)| sha256_prefixed(bytes)),
        "manifestRef": manifest_relative,
        "manifestDigest": sha256_prefixed(manifest_bytes),
    })
}

fn ledger_event_error(error: ae_sdd_contracts::EvidenceLedgerError) -> EvidenceError {
    match error {
        ae_sdd_contracts::EvidenceLedgerError::InvalidLogicalKey
        | ae_sdd_contracts::EvidenceLedgerError::UnexpectedLogicalKey => {
            EvidenceError::InvalidLogicalKey
        }
        _ => EvidenceError::Serialize,
    }
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

    fn ledger_events(root: &Path, story_id: &str) -> Vec<EvidenceLedgerEventV1> {
        load_ledger(root, story_id)
            .expect("ledger verifies")
            .expect("ledger exists")
            .events
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
        assert_eq!(first.targets.len(), 4);
        assert!(first.targets[0].before_digest.is_none());
        materialize(root.path(), &first);
        let second = prepare_record(root.path(), "STORY-001", &payload, "2026-07-23T00:01:00Z")
            .expect("second record");
        assert_eq!(
            second.targets.len(),
            3,
            "content-addressed snapshot is reused"
        );
        let ledger_before = fs::read(
            root.path()
                .join(".auto-engineering/STORY-001/evidence/ledger.jsonl"),
        )
        .expect("ledger before second record");
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
        let events = ledger_events(root.path(), "STORY-001");
        assert_eq!(events.len(), 3);
        assert_eq!(events[1].kind(), EvidenceLedgerEventKind::Superseded);
        let ledger_after = fs::read(
            root.path()
                .join(".auto-engineering/STORY-001/evidence/ledger.jsonl"),
        )
        .expect("ledger after second record");
        assert!(ledger_after.starts_with(&ledger_before));
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
        let events = ledger_events(root.path(), "STORY-001");
        assert_eq!(
            events.last().map(EvidenceLedgerEventV1::kind),
            Some(EvidenceLedgerEventKind::Finalized)
        );
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

    #[test]
    fn record_rejects_ledger_hash_chain_tampering() {
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
        let ledger_path = root
            .path()
            .join(".auto-engineering/STORY-001/evidence/ledger.jsonl");
        let tampered = fs::read(&ledger_path)
            .expect("ledger")
            .into_iter()
            .map(|byte| if byte == b'1' { b'2' } else { byte })
            .collect::<Vec<_>>();
        fs::write(&ledger_path, tampered).expect("tampered ledger");
        assert!(matches!(
            prepare_finalize(root.path(), "STORY-001"),
            Err(EvidenceError::LedgerTampered(_))
        ));
        assert!(matches!(
            prepare_record(
                root.path(),
                "STORY-001",
                &json!({"artifactPath":"result.json","inputFingerprint":"sha256:input"}),
                "2026-07-23T00:01:00Z"
            ),
            Err(EvidenceError::LedgerTampered(_))
        ));
    }

    #[test]
    fn legacy_manifest_finalizes_without_a_ledger_and_stays_verbatim() {
        let root = tempdir().expect("workspace");
        let snapshot = b"legacy\n";
        let digest = sha256_prefixed(snapshot);
        let snapshot_relative = ".auto-engineering/STORY-001/evidence/artifacts/legacy.txt";
        let absolute = root.path().join(snapshot_relative);
        fs::create_dir_all(absolute.parent().expect("parent")).expect("directory");
        fs::write(&absolute, snapshot).expect("snapshot");
        let legacy_entry = json!({
            "evidenceId": "ev-legacy",
            "kind": "test",
            "inputFingerprint": "i1",
            "exitCode": 0,
            "reusable": true,
            "artifacts": [{"path": "legacy.txt", "sha256": digest, "snapshotPath": snapshot_relative}],
        });
        let mut legacy = json!({
            "schemaVersion": 1,
            "storyId": "STORY-001",
            "entries": [legacy_entry.clone()],
        });
        seal_manifest(&mut legacy).expect("seal");
        let manifest_path = root
            .path()
            .join(".auto-engineering/STORY-001/evidence/manifest.json");
        fs::write(&manifest_path, pretty_json(&legacy).expect("bytes")).expect("legacy manifest");

        let finalized = prepare_finalize(root.path(), "STORY-001").expect("legacy finalize");
        assert_eq!(finalized.targets.len(), 1);
        assert!(finalized.authority["ledgerRef"].is_null());
        materialize(root.path(), &finalized);
        assert!(
            !root
                .path()
                .join(".auto-engineering/STORY-001/evidence/ledger.jsonl")
                .exists()
        );
        let manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("manifest")).expect("JSON");
        assert_eq!(manifest["entries"][0], legacy_entry);
    }
}
