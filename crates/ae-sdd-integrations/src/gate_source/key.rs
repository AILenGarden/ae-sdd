use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use ae_sdd_domain::{
    ConfigDigest, EvidenceDigest, EvidenceId, EvidenceRef, FencingToken, GateId,
    GateImplementationDigest, GateKey, InputFingerprint, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, ToolchainDigest, VerificationId, WorkItemId,
    WorkspaceId,
};
use ae_sdd_gates::{GateInputSelector, GateRegistry, GateSpec, NativeGateRule};
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{BusinessWorkspace, PersistencePort, RuntimeError, RuntimeResult};
use ae_sdd_store::UtcTimestamp;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::ra_binding::{AuthoritativeRaPath, authoritative_ra_path};
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

/// Production-only dependencies required to validate authoritative Review
/// authority. A Gate context without them can never satisfy a Review predicate.
#[derive(Clone)]
pub struct ReviewGateAuthority {
    /// Runtime SQLite database holding the Review Batch v2 projections.
    pub database: PathBuf,
    /// Durable runtime metadata port used for identity and job lineage.
    pub persistence: Arc<dyn PersistencePort>,
    /// Current daemon boot identity that must own reviewer attestations.
    pub boot_id: String,
    /// Daemon-authenticated workspace the Review authority is bound to.
    pub workspace: BusinessWorkspace,
}

impl fmt::Debug for ReviewGateAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReviewGateAuthority")
            .field("database", &self.database)
            .field("boot_id", &self.boot_id)
            .field("workspace", &self.workspace)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(super) struct GateContext {
    pub(super) root: PathBuf,
    pub(super) workspace_id: WorkspaceId,
    pub(super) work_item_id: WorkItemId,
    pub(super) policy: PolicyDigest,
    pub(super) inventory: InventoryGeneration,
    pub(super) expected_fencing_token: Option<FencingToken>,
    /// `None` for lightweight/non-production contexts. Review predicates then
    /// fail closed because the durable authority cannot be joined at all.
    pub(super) review: Option<ReviewGateAuthority>,
}

/// Stable reason a Review predicate refused to release its Gate. Absent Review
/// authority dependencies produce a denial exactly like a validator error, so a
/// lightweight context can never read as PASS.
#[derive(Clone, Debug)]
pub(super) struct ReviewAuthorityDenial {
    code: &'static str,
    message: String,
    evidence_id: &'static str,
}

impl ReviewAuthorityDenial {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            evidence_id: "review-authority-denied",
        }
    }

    pub(super) fn context(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            evidence_id: "dr-context-missing",
        }
    }

    /// Projects the denial as Gate finding evidence so a failing Review Gate
    /// reports why instead of only `<GATE>-FAILED`.
    pub(super) fn evidence(&self, located: &LocatedState) -> Option<EvidenceRef> {
        let bytes = self.message.as_bytes();
        Some(EvidenceRef::new(
            EvidenceId::new(self.evidence_id).ok()?,
            VerificationId::new(denial_verification_id(self.code, &self.message)).ok()?,
            ProjectRelativePath::new(located.relative.clone()).ok()?,
            EvidenceDigest::digest(bytes),
            u64::try_from(bytes.len()).ok()?,
        ))
    }
}

impl GateContext {
    /// Joins project state, the durable SQLite projection, reviewer lineage and
    /// final proof. Any missing dependency or validator error is a denial, never
    /// a silent PASS.
    pub(super) fn review_authority_denial(
        &self,
        located: &LocatedState,
    ) -> Option<ReviewAuthorityDenial> {
        let Some(review) = self.review.as_ref() else {
            return Some(ReviewAuthorityDenial::new(
                "REVIEW_AUTHORITY_UNAVAILABLE",
                "Gate context carries no Review authority dependencies",
            ));
        };
        if review.workspace.workspace_id != self.workspace_id.to_string() {
            return Some(ReviewAuthorityDenial::new(
                "REVIEW_AUTHORITY_UNAVAILABLE",
                "Review authority workspace differs from the evaluated workspace",
            ));
        }
        crate::review_authority::validate_review_gate_authority(
            &review.database,
            &review.workspace,
            &located.path,
            &located.value,
            self.work_item_id.as_str(),
            review.persistence.as_ref(),
            &review.boot_id,
            &UtcTimestamp::now(),
        )
        .err()
        .map(|error| ReviewAuthorityDenial::new(error.code().as_str(), error.message()))
    }

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
            input_fingerprint(&self.root, &located, gate_id, self.work_item_id.as_str())?,
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

/// Builds a bounded `VerificationId`-safe reason slug: the stable error code
/// followed by the validator message with unsupported bytes folded to `-`.
fn denial_verification_id(code: &str, message: &str) -> String {
    let mut slug = String::with_capacity(VerificationId::MAX_BYTES);
    slug.push_str(code);
    slug.push(':');
    let mut previous_separator = true;
    for character in message.chars() {
        if slug.len() >= VerificationId::MAX_BYTES {
            break;
        }
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_') {
            slug.push(character);
            previous_separator = false;
        } else if !previous_separator {
            slug.push('-');
            previous_separator = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
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

/// Selector-scoped Gate input fingerprint.
///
/// The fingerprint binds exactly the authoritative inputs the Gate declares
/// through its `GateDependencySpec` selectors: a source write under
/// `ChangedPaths` no longer rewrites the fingerprint of RA/Story/CodingPlan
/// Gates, so their fresh cached outcomes stay reusable while the affected
/// nodes re-evaluate. A Gate without a selector declaration cannot prove its
/// input scope and fails closed to the legacy whole-state/whole-inventory
/// hash. The state revision, fencing token, inventory generation, toolchain
/// and configuration stay independent `GateKey` dimensions, so any committed
/// state mutation still busts the full key set; scoping only governs reuse
/// while the revision is stable.
fn input_fingerprint(
    root: &Path,
    located: &LocatedState,
    gate_id: &str,
    work_item: &str,
) -> RuntimeResult<InputFingerprint> {
    let selectors = GateRegistry::dependency_spec(gate_id)
        .map(|specification| specification.selectors)
        .unwrap_or(&[]);
    if selectors.is_empty() {
        return legacy_full_input_fingerprint(root, located);
    }
    let mut hasher = Sha256::new();
    hasher.update(b"ae-sdd-authoritative-gate-input/v2\0");
    hash_part(&mut hasher, located.relative.as_bytes());
    hash_state_scope(&mut hasher, &located.value, IDENTITY_STATE_FIELDS)?;
    for selector in selectors {
        hash_part(&mut hasher, selector_label(*selector).as_bytes());
        hash_state_scope(
            &mut hasher,
            &located.value,
            selector_state_fields(*selector),
        )?;
        match selector {
            GateInputSelector::ChangedPaths => {
                hash_changed_paths(&mut hasher, root, &located.value)?;
            }
            GateInputSelector::ExecutionPlan => {
                hash_source_reads(&mut hasher, root, &located.value)?;
            }
            GateInputSelector::EvidenceLedger => {
                hash_evidence_scope(&mut hasher, root, located, work_item)?;
            }
            GateInputSelector::RequirementAnalysis => {
                hash_requirement_analysis(&mut hasher, root, &located.value)?;
            }
            GateInputSelector::ProjectAssets
            | GateInputSelector::Story
            | GateInputSelector::Constraints
            | GateInputSelector::ThinkingEngine
            | GateInputSelector::VerificationPlan
            | GateInputSelector::ReviewBatch
            | GateInputSelector::Toolchain
            | GateInputSelector::Inventory
            | GateInputSelector::RouteBinding => {}
        }
    }
    let mut inputs = workspace_inputs(root)?;
    inputs.retain(|(relative, _)| {
        selectors
            .iter()
            .any(|selector| selector_file_scope(*selector, relative))
    });
    for (relative, path) in inputs {
        hash_part(&mut hasher, relative.as_bytes());
        hash_file_content(&mut hasher, &path)?;
    }
    Ok(InputFingerprint::from_array(hasher.finalize().into()))
}

/// Legacy pre-selector fingerprint: the whole authoritative state plus every
/// workspace input. Retained as the fail-closed fallback for Gates without a
/// selector declaration.
fn legacy_full_input_fingerprint(
    root: &Path,
    located: &LocatedState,
) -> RuntimeResult<InputFingerprint> {
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
        hash_file_content(&mut hasher, &path)?;
    }
    Ok(InputFingerprint::from_array(hasher.finalize().into()))
}

/// State fields every Gate fingerprint binds: process identity and phase.
/// Revision, fencing and last-mutation bookkeeping stay out because the
/// `GateKey` already carries revision and fencing as independent dimensions.
const IDENTITY_STATE_FIELDS: &[&str] = &[
    "stateMachineName",
    "currentWorkItem",
    "phase",
    "currentPhase",
    "pausedFrom",
    "scale",
];

fn selector_label(selector: GateInputSelector) -> &'static str {
    match selector {
        GateInputSelector::ProjectAssets => "project-assets",
        GateInputSelector::Story => "story",
        GateInputSelector::Constraints => "constraints",
        GateInputSelector::ThinkingEngine => "thinking-engine",
        GateInputSelector::ExecutionPlan => "execution-plan",
        GateInputSelector::ChangedPaths => "changed-paths",
        GateInputSelector::VerificationPlan => "verification-plan",
        GateInputSelector::EvidenceLedger => "evidence-ledger",
        GateInputSelector::ReviewBatch => "review-batch",
        GateInputSelector::Toolchain => "toolchain",
        GateInputSelector::Inventory => "inventory",
        GateInputSelector::RequirementAnalysis => "requirement-analysis",
        GateInputSelector::RouteBinding => "route-binding",
    }
}

/// Authoritative state sections one selector contributes to the fingerprint.
fn selector_state_fields(selector: GateInputSelector) -> &'static [&'static str] {
    match selector {
        GateInputSelector::ProjectAssets => &["documentPaths"],
        GateInputSelector::Story => &[
            "storyStates",
            "activeStory",
            "currentStory",
            "documentPaths",
            "storyReview",
            "taskReview",
        ],
        GateInputSelector::Constraints | GateInputSelector::ThinkingEngine => &[],
        GateInputSelector::ExecutionPlan => &[
            "executionPlan",
            "executionRuntime",
            "routeDecision",
            "selectedDesign",
        ],
        GateInputSelector::ChangedPaths => &[],
        GateInputSelector::VerificationPlan => &["verificationPlan"],
        GateInputSelector::EvidenceLedger => {
            &["evidenceAuthority", "evidence", "evidenceFinalized"]
        }
        GateInputSelector::ReviewBatch => &[
            "review",
            "reviewSession",
            "reviewLoop",
            "inputFingerprint",
            "rulesetFingerprint",
            "policyDigest",
            "inventoryGeneration",
        ],
        // Toolchain and inventory are independent `GateKey` dimensions; they
        // contribute nothing extra to the input fingerprint.
        GateInputSelector::Toolchain | GateInputSelector::Inventory => &[],
        // RequirementAnalysis binds the single RA path plus its validated
        // receipt. The file bytes are hashed in `input_fingerprint`; here we
        // expose the state pointers that, when changed, must bust RA gates.
        GateInputSelector::RequirementAnalysis => &[],
        // RouteBinding binds the route candidate, approval, evidence and open
        // conflicts — all state, no file scope.
        GateInputSelector::RouteBinding => &[
            "routeCandidate",
            "routeApprovalReceipt",
            "engineeringRoute",
            "routeBlockingConflicts",
            "scaleEvidenceDigest",
        ],
    }
}

/// Workspace-relative file scope one selector contributes. Selectors without
/// a file scope bind their inputs from state or dedicated hashers instead.
fn selector_file_scope(selector: GateInputSelector, relative: &str) -> bool {
    match selector {
        GateInputSelector::ProjectAssets => {
            relative.starts_with("ae-sdd-doc/")
                || relative.starts_with(".ae-sdd/")
                || matches!(relative, "Cargo.toml" | "pyproject.toml" | "package.json")
        }
        GateInputSelector::Story => {
            relative.starts_with("ae-sdd-doc/Story/")
                || relative.starts_with("ae-sdd-doc/Task/")
                // The canonical TestCase directory is `Test/` (`STORING.md`), not
                // `TestCase/`. Scoping the wrong directory left the TestCase
                // document out of the fingerprint, so deleting it did not change
                // the Gate key and `G-04` reused a stale PASS.
                || relative.starts_with("ae-sdd-doc/Test/")
        }
        GateInputSelector::Constraints => relative.starts_with("constraints/"),
        GateInputSelector::ThinkingEngine => relative.starts_with("source/"),
        GateInputSelector::ExecutionPlan
        | GateInputSelector::ChangedPaths
        | GateInputSelector::VerificationPlan
        | GateInputSelector::EvidenceLedger
        | GateInputSelector::ReviewBatch
        | GateInputSelector::Toolchain
        | GateInputSelector::Inventory
        | GateInputSelector::RequirementAnalysis
        | GateInputSelector::RouteBinding => false,
    }
}

/// Upper bound for approved changed paths folded into one fingerprint.
const CHANGED_PATH_LIMIT: usize = 1_024;

/// Hashes the approved changed-path list plus each path's current content.
/// A missing path hashes an explicit marker so a deleted source still busts
/// every `ChangedPaths` Gate instead of silently reusing its cached outcome.
fn hash_changed_paths(hasher: &mut Sha256, root: &Path, state: &Value) -> RuntimeResult<()> {
    let mut paths: Vec<String> = state
        .pointer("/executionPlan/changedPaths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths.dedup();
    if paths.len() > CHANGED_PATH_LIMIT {
        return Err(external(
            "approved changed paths exceed the fingerprint limit",
        ));
    }
    for relative in paths {
        hash_part(hasher, relative.as_bytes());
        let path = root.join(&relative);
        if path.is_file() {
            hash_file_content(hasher, &path)?;
        } else {
            hash_part(hasher, b"<missing>");
        }
    }
    Ok(())
}

/// Folds the plan's traced sources into the fingerprint.
///
/// `G-CODEPLAN-SRC` passes only while a `sourceReads` entry names a file that
/// exists, so that file's presence is a Gate input. `sourceReads` may name any
/// path, and the declared file scopes cover only fixed directories, so the
/// paths are read from the plan itself — the same approach `hash_changed_paths`
/// takes. Without this a traced source could be deleted and the Gate would
/// reuse its stale PASS.
fn hash_source_reads(hasher: &mut Sha256, root: &Path, state: &Value) -> RuntimeResult<()> {
    let mut paths: Vec<String> = state
        .pointer("/executionPlan/sourceReads")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths.dedup();
    if paths.len() > CHANGED_PATH_LIMIT {
        return Err(external(
            "approved source reads exceed the fingerprint limit",
        ));
    }
    for relative in paths {
        hash_part(hasher, relative.as_bytes());
        // `source_trace_complete` accepts an absolute path or one relative to
        // the workspace root, so the fingerprint must resolve it the same way.
        let candidate = Path::new(&relative);
        let path = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.join(candidate)
        };
        if path.is_file() {
            hash_file_content(hasher, &path)?;
        } else {
            hash_part(hasher, b"<missing>");
        }
    }
    Ok(())
}

/// Hashes the active Story's evidence directory (ledger, manifest, entries
/// and snapshots) so an evidence mutation only busts EvidenceLedger Gates.
fn hash_evidence_scope(
    hasher: &mut Sha256,
    root: &Path,
    located: &LocatedState,
    work_item: &str,
) -> RuntimeResult<()> {
    let Some(story) = active_story(&located.value, work_item) else {
        hash_part(hasher, b"<no-active-story>");
        return Ok(());
    };
    let directory = root
        .join(".auto-engineering")
        .join(story.as_str())
        .join("evidence");
    let mut files = Vec::new();
    if directory.is_dir() {
        collect_evidence_files(root, &directory, &mut files)?;
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files.dedup_by(|left, right| left.0 == right.0);
    }
    for (relative, path) in files {
        hash_part(hasher, relative.as_bytes());
        hash_file_content(hasher, &path)?;
    }
    Ok(())
}

/// Recursively collects every regular file under the Story evidence
/// directory, including the `ledger.jsonl` truth that the generic source
/// inventory extension filter does not cover.
fn collect_evidence_files(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(String, PathBuf)>,
) -> RuntimeResult<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|_| external("evidence directory is unreadable"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| external("evidence directory entry is unreadable"))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| external("evidence entry metadata is unreadable"))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_evidence_files(root, &path, output)?;
        } else if metadata.is_file() {
            if output.len() >= INPUT_FILE_LIMIT {
                return Err(external("evidence inputs exceed the file limit"));
            }
            output.push((relative_string(root, &path)?, path));
        }
    }
    Ok(())
}

/// Hashes one file the same bounded way the legacy inventory hash did.
fn hash_file_content(hasher: &mut Sha256, path: &Path) -> RuntimeResult<()> {
    let metadata = fs::metadata(path).map_err(|_| external("Gate input metadata changed"))?;
    if metadata.len() <= HASH_CONTENT_LIMIT {
        hash_part(
            hasher,
            &fs::read(path).map_err(|_| external("Gate input became unreadable"))?,
        );
    } else {
        hash_part(hasher, &metadata.len().to_be_bytes());
    }
    Ok(())
}

/// Hashes the named authoritative state sections in canonical JSON form.
fn hash_state_scope(hasher: &mut Sha256, state: &Value, fields: &[&str]) -> RuntimeResult<()> {
    for field in fields {
        hash_part(hasher, field.as_bytes());
        match state.get(*field) {
            Some(value) => hash_part(hasher, &canonical_json(value)?),
            None => hash_part(hasher, b"<absent>"),
        }
    }
    Ok(())
}

/// Hashes the single bound RA document plus its validated receipt.
///
/// The path authority is `/documentPaths/RA` only — no directory scan, no story
/// fallback, no `route_exempt`. A missing/escaped/invalid path hashes an
/// explicit marker so the gate fails closed instead of silently reusing a
/// cached PASS. Foreign project assets and other Work Items' RA documents do
/// not contribute to this fingerprint. Task 11 owns the authoritative resolver
/// shared with predicates/scanners; this hasher is the fingerprint-side twin.
fn hash_requirement_analysis(hasher: &mut Sha256, root: &Path, state: &Value) -> RuntimeResult<()> {
    hash_part(hasher, b"ra-path");
    match authoritative_ra_path(root, state) {
        AuthoritativeRaPath::Bound { relative, absolute } => {
            hash_part(hasher, relative.as_bytes());
            if absolute.is_file() {
                hash_file_content(hasher, &absolute)?;
            } else {
                hash_part(hasher, b"<missing>");
            }
        }
        AuthoritativeRaPath::Escape => hash_part(hasher, b"<escape>"),
        AuthoritativeRaPath::Invalid => hash_part(hasher, b"<invalid>"),
        AuthoritativeRaPath::Missing => hash_part(hasher, b"<missing>"),
    }
    hash_part(hasher, b"ra-receipt");
    match state.pointer("/seriesReceipts/RA") {
        Some(receipt) => hash_part(hasher, &canonical_json(receipt)?),
        None => hash_part(hasher, b"<absent>"),
    }
    Ok(())
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
                || (directory == root && name.starts_with("target-"))
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
