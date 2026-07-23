use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use ae_sdd_domain::{
    ConfigDigest, EvidenceDigest, EvidenceId, EvidenceRef, FencingToken, GateId,
    GateImplementationDigest, GateKey, InputFingerprint, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, ToolchainDigest, VerificationId, WorkItemId,
    WorkspaceId,
};
use ae_sdd_gates::{GateRegistry, GateSpec, NativeGateRule};
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{RuntimeError, RuntimeResult};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{external, schema};

const INPUT_FILE_LIMIT: usize = 20_000;
const HASH_CONTENT_LIMIT: u64 = 8 * 1024 * 1024;
const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".hermes",
    ".venv",
    "__pycache__",
    "dist",
    "node_modules",
    "target",
    "vendor",
];

#[derive(Debug)]
pub(super) struct GateContext {
    pub(super) root: PathBuf,
    pub(super) workspace_id: WorkspaceId,
    pub(super) work_item_id: WorkItemId,
    pub(super) policy: PolicyDigest,
    pub(super) inventory: InventoryGeneration,
    pub(super) expected_fencing_token: Option<FencingToken>,
}

impl GateContext {
    pub(super) fn build_key(&self, gate_id: &str, enforce_fencing: bool) -> RuntimeResult<GateKey> {
        let specification = GateRegistry::get(gate_id)
            .ok_or_else(|| RuntimeError::new(StableErrorCode::GateError, "unknown Gate"))?;
        let located = self.load_state()?;
        let fencing = authoritative_fencing(&located);
        if enforce_fencing
            && self
                .expected_fencing_token
                .is_some_and(|expected| expected != fencing)
        {
            return Err(RuntimeError::new(
                StableErrorCode::StaleFencingToken,
                "Gate snapshot fencing token is no longer authoritative",
            ));
        }
        Ok(GateKey::new(
            GateId::new(gate_id.to_owned()).map_err(|_| schema("gateId is invalid"))?,
            implementation_digest(specification),
            self.policy,
            self.workspace_id,
            self.work_item_id.clone(),
            active_story(&located.value, self.work_item_id.as_str()),
            StateRevision::new(
                located
                    .value
                    .get("revision")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| schema("authoritative state revision is missing"))?,
            ),
            fencing,
            self.inventory,
            toolchain_digest(&self.root)?,
            configuration_digest(&self.root)?,
            input_fingerprint(&self.root, &located)?,
        ))
    }

    pub(super) fn load_state(&self) -> RuntimeResult<LocatedState> {
        let directory = self.root.join(".auto-engineering");
        let mut matches = Vec::new();
        for entry in
            fs::read_dir(directory).map_err(|_| external("state directory is unreadable"))?
        {
            let path = entry
                .map_err(|_| external("state directory entry is unreadable"))?
                .path()
                .join("state.json");
            if !path.is_file() {
                continue;
            }
            let bytes = fs::read(&path).map_err(|_| external("state JSON is unreadable"))?;
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|_| external("state JSON is malformed"))?;
            if state_matches(&value, self.work_item_id.as_str()) {
                matches.push(LocatedState {
                    relative: relative_string(&self.root, &path)?,
                    path,
                    value,
                });
            }
        }
        match matches.len() {
            1 => Ok(matches.pop().expect("one state was checked")),
            0 => Err(RuntimeError::new(
                StableErrorCode::ProjectMismatch,
                "Work Item state was not found",
            )),
            _ => Err(RuntimeError::new(
                StableErrorCode::ScopeAmbiguous,
                "Work Item resolves to multiple state files",
            )),
        }
    }
}

pub(super) struct LocatedState {
    pub(super) path: PathBuf,
    pub(super) relative: String,
    pub(super) value: Value,
}

pub(super) fn active_story(state: &Value, work_item: &str) -> Option<StoryId> {
    let candidate = work_item
        .starts_with("STORY-")
        .then_some(work_item)
        .or_else(|| state.get("activeStory").and_then(Value::as_str))
        .or_else(|| state.get("currentStory").and_then(Value::as_str))?;
    StoryId::new(candidate.to_owned()).ok()
}

pub(super) fn nested_phase<'a>(state: &'a Value, story: Option<&str>) -> Option<&'a str> {
    story
        .and_then(|id| state.pointer(&format!("/storyStates/{id}/currentPhase")))
        .and_then(Value::as_str)
        .or_else(|| state.get("currentPhase").and_then(Value::as_str))
        .or_else(|| state.get("phase").and_then(Value::as_str))
}

pub(super) fn safe_document_path(root: &Path, value: &str) -> bool {
    let path = Path::new(value);
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    candidate
        .canonicalize()
        .is_ok_and(|canonical| canonical.starts_with(root) && canonical.is_file())
}

pub(super) fn workspace_inputs(root: &Path) -> RuntimeResult<Vec<(String, PathBuf)>> {
    let mut output = Vec::new();
    collect_inputs(root, root, &mut output)?;
    output.sort_by(|left, right| left.0.cmp(&right.0));
    output.dedup_by(|left, right| left.0 == right.0);
    Ok(output)
}

pub(super) fn state_evidence(located: &LocatedState) -> Option<EvidenceRef> {
    let bytes = fs::read(&located.path).ok()?;
    Some(EvidenceRef::new(
        EvidenceId::new("authoritative-state").ok()?,
        VerificationId::new("gate-input").ok()?,
        ProjectRelativePath::new(located.relative.clone()).ok()?,
        EvidenceDigest::digest(&bytes),
        u64::try_from(bytes.len()).ok()?,
    ))
}

fn implementation_digest(specification: &GateSpec) -> GateImplementationDigest {
    let rule = match specification.rule {
        NativeGateRule::Predicate(predicate) => format!("predicate:{}", predicate.as_str()),
        NativeGateRule::Scanner(scanner) => format!("scanner:{}", scanner.as_str()),
    };
    GateImplementationDigest::digest(format!(
        "ae-sdd-native-gate/v1\0{}\0{}\0{}\0{}\0{}\0{}",
        specification.id,
        specification.name,
        specification.scope,
        specification.pass_condition,
        specification.failure_action,
        rule
    ))
}

fn input_fingerprint(root: &Path, located: &LocatedState) -> RuntimeResult<InputFingerprint> {
    let mut state = located.value.clone();
    if let Some(object) = state.as_object_mut() {
        object.remove("gateResults");
        object.remove("hookGuard");
    }
    let mut hasher = Sha256::new();
    hasher.update(b"ae-sdd-authoritative-gate-input/v1\0");
    hash_part(&mut hasher, located.relative.as_bytes());
    hash_part(&mut hasher, &canonical_json(&state)?);
    for (label, path) in authority_inputs(located) {
        hash_part(&mut hasher, label.as_bytes());
        hash_part(
            &mut hasher,
            &fs::read(path).map_err(|_| external("authoritative Gate input is unreadable"))?,
        );
    }
    for (relative, path) in workspace_inputs(root)? {
        hash_part(&mut hasher, relative.as_bytes());
        let metadata = fs::metadata(&path).map_err(|_| external("Gate input metadata changed"))?;
        if metadata.len() <= HASH_CONTENT_LIMIT {
            hash_part(
                &mut hasher,
                &fs::read(path).map_err(|_| external("Gate input became unreadable"))?,
            );
        } else {
            hash_part(&mut hasher, &metadata.len().to_be_bytes());
        }
    }
    Ok(InputFingerprint::from_array(hasher.finalize().into()))
}

fn toolchain_digest(root: &Path) -> RuntimeResult<ToolchainDigest> {
    digest_named(root, &["rust-toolchain.toml", "Cargo.lock", "Cargo.toml"])
        .map(ToolchainDigest::from_array)
}

fn configuration_digest(root: &Path) -> RuntimeResult<ConfigDigest> {
    let base = digest_named(
        root,
        &[
            ".codex/hooks.json",
            ".harness/agent.md",
            "AGENTS.md",
            "clippy.toml",
            "deny.toml",
            "rustfmt.toml",
        ],
    )?;
    let mut hasher = Sha256::new();
    hasher.update(base);
    for (relative, path) in workspace_inputs(root)? {
        if relative.starts_with(".ae-sdd/") || relative.starts_with("constraints/") {
            hash_part(&mut hasher, relative.as_bytes());
            hash_part(
                &mut hasher,
                &fs::read(path).map_err(|_| external("configuration input is unreadable"))?,
            );
        }
    }
    Ok(ConfigDigest::from_array(hasher.finalize().into()))
}

fn digest_named(root: &Path, names: &[&str]) -> RuntimeResult<[u8; 32]> {
    let mut hasher = Sha256::new();
    for name in names {
        let path = root.join(name);
        hash_part(&mut hasher, name.as_bytes());
        if path.is_file() {
            hash_part(
                &mut hasher,
                &fs::read(path).map_err(|_| external("digest input is unreadable"))?,
            );
        } else {
            hash_part(&mut hasher, b"<missing>");
        }
    }
    Ok(hasher.finalize().into())
}

fn collect_inputs(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(String, PathBuf)>,
) -> RuntimeResult<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|_| external("Gate input directory is unreadable"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| external("Gate input directory entry is unreadable"))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| external("Gate input metadata is unreadable"))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let relative = relative_string(root, &path)?;
        if metadata.is_dir() {
            let name = relative
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if EXCLUDED_DIRS.contains(&name.as_str())
                || name == ".auto-engineering"
                || relative.eq_ignore_ascii_case("apps/ae-sdd-monitor")
            {
                continue;
            }
            collect_inputs(root, &path, output)?;
        } else if metadata.is_file() && relevant_input(&relative) {
            if output.len() >= INPUT_FILE_LIMIT {
                return Err(external("Gate input inventory exceeds the file limit"));
            }
            output.push((relative, path));
        }
    }
    Ok(())
}

fn relevant_input(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "c" | "cc"
                | "cpp"
                | "go"
                | "h"
                | "hpp"
                | "java"
                | "js"
                | "json"
                | "jsx"
                | "kt"
                | "kts"
                | "md"
                | "properties"
                | "ps1"
                | "py"
                | "rs"
                | "sh"
                | "toml"
                | "ts"
                | "tsx"
                | "xml"
                | "yaml"
                | "yml"
        )
    )
}

fn authority_inputs(located: &LocatedState) -> Vec<(String, PathBuf)> {
    let Some(directory) = located.path.parent() else {
        return Vec::new();
    };
    let mut inputs = Vec::new();
    let lease = directory.join("state.lease.json");
    if lease.is_file() {
        inputs.push(("state.lease.json".to_owned(), lease));
    }
    let evidence = directory.join("evidence");
    if let Ok(entries) = fs::read_dir(evidence) {
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        files.sort();
        inputs.extend(
            files
                .into_iter()
                .filter(|path| path.is_file())
                .filter_map(|path| {
                    path.file_name()
                        .map(|name| (format!("evidence/{}", name.to_string_lossy()), path.clone()))
                }),
        );
    }
    inputs
}

fn authoritative_fencing(located: &LocatedState) -> FencingToken {
    let lease = located.path.with_file_name("state.lease.json");
    let lease_token = fs::read(lease)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| {
            value
                .get("active")
                .and_then(|active| active.get("fencingToken"))
                .or_else(|| value.get("fencingToken"))
                .and_then(Value::as_u64)
        });
    FencingToken::new(
        lease_token
            .or_else(|| {
                located
                    .value
                    .get("lastFencingToken")
                    .and_then(Value::as_u64)
            })
            .unwrap_or(0),
    )
}

fn state_matches(state: &Value, work_item: &str) -> bool {
    [
        "stateMachineName",
        "stateMachineId",
        "currentWorkItem",
        "activeStory",
        "activeTask",
    ]
    .iter()
    .any(|field| state.get(*field).and_then(Value::as_str) == Some(work_item))
        || state
            .get("storyStates")
            .and_then(Value::as_object)
            .is_some_and(|stories| stories.contains_key(work_item))
}

fn canonical_json(value: &Value) -> RuntimeResult<Vec<u8>> {
    serde_json::to_vec(&canonical_value(value))
        .map_err(|_| external("Gate input JSON cannot be canonicalized"))
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect::<Map<_, _>>(),
        ),
        Value::Array(array) => Value::Array(array.iter().map(canonical_value).collect()),
        _ => value.clone(),
    }
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn relative_string(root: &Path, path: &Path) -> RuntimeResult<String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| external("Gate input escaped the workspace root"))
}
