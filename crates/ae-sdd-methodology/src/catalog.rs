use std::{cmp::Ordering, collections::BTreeMap};

use ae_sdd_contracts::{MAX_OVERRIDE_TRACE, MessageKey, OverrideLayer, ReasonCode, SkillId};
use ae_sdd_domain::{ArtifactDigest, ProjectKey};
use serde::Serialize;

use crate::{
    CompiledMethodologyBundle, CompiledMethodologyEntry, MethodologyAssetSource, MethodologyError,
    compiler::digest_json,
    verifier::{verify_artifact, verify_bundle_metadata, verify_entry_metadata},
};

/// Whether a registry contender passed its external authorization policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideAuthorization {
    /// The contender may participate in deterministic selection.
    Authorized,
    /// The contender must make resolution fail closed.
    Denied,
}

/// Project containment attached to one registry contender.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverrideScope {
    /// The contender applies to every project.
    AllProjects,
    /// The contender applies only to the named project registry.
    Project(ProjectKey),
}

/// One validated Methodology contender supplied by an override registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodologyOverride {
    pub(crate) layer: OverrideLayer,
    pub(crate) target_skill_id: SkillId,
    pub(crate) entry: CompiledMethodologyEntry,
    pub(crate) scope: OverrideScope,
    pub(crate) authorization: OverrideAuthorization,
    pub(crate) registry_digest: ArtifactDigest,
}

impl MethodologyOverride {
    /// Constructs an override while enforcing the frozen layer/scope matrix.
    pub fn new(
        layer: OverrideLayer,
        target_skill_id: SkillId,
        entry: CompiledMethodologyEntry,
        scope: OverrideScope,
        authorization: OverrideAuthorization,
        registry_digest: ArtifactDigest,
    ) -> Result<Self, MethodologyError> {
        let valid_scope = matches!(
            (&layer, &scope),
            (OverrideLayer::Project, OverrideScope::Project(_))
                | (
                    OverrideLayer::Global | OverrideLayer::Repository,
                    OverrideScope::AllProjects
                )
        );
        if layer == OverrideLayer::BuiltIn {
            return Err(MethodologyError::InvalidOverrideLayer);
        }
        if !valid_scope {
            return Err(MethodologyError::InvalidOverrideScope);
        }
        Ok(Self {
            layer,
            target_skill_id,
            entry,
            scope,
            authorization,
            registry_digest,
        })
    }

    /// Returns the registry priority layer.
    pub const fn layer(&self) -> OverrideLayer {
        self.layer
    }

    /// Returns the built-in skill identity replaced by this contender.
    pub const fn target_skill_id(&self) -> &SkillId {
        &self.target_skill_id
    }

    /// Returns the content-bound candidate entry.
    pub const fn entry(&self) -> &CompiledMethodologyEntry {
        &self.entry
    }

    /// Returns the contender's project containment.
    pub const fn scope(&self) -> &OverrideScope {
        &self.scope
    }

    /// Returns the external authorization decision.
    pub const fn authorization(&self) -> OverrideAuthorization {
        self.authorization
    }

    /// Returns the immutable registry snapshot digest.
    pub const fn registry_digest(&self) -> ArtifactDigest {
        self.registry_digest
    }

    pub(crate) fn applies_to(&self, project: &ProjectKey) -> bool {
        match &self.scope {
            OverrideScope::AllProjects => true,
            OverrideScope::Project(scope) => scope == project,
        }
    }
}

/// Validated, compact-only in-memory Methodology Catalog.
pub struct MethodologyCatalog {
    pub(crate) entries: Vec<CompiledMethodologyEntry>,
    pub(crate) overrides: Vec<MethodologyOverride>,
    pub(crate) catalog_digest: ArtifactDigest,
    project_registry_digests: BTreeMap<ProjectKey, ArtifactDigest>,
    pub(crate) vocabulary: ResolutionVocabulary,
}

impl MethodologyCatalog {
    /// Opens a compiled bundle, validating compact content without reading fallback bodies.
    pub fn open(
        bundle: CompiledMethodologyBundle,
        compact_assets: &impl MethodologyAssetSource,
        mut overrides: Vec<MethodologyOverride>,
    ) -> Result<Self, MethodologyError> {
        let vocabulary = ResolutionVocabulary::new()?;
        verify_bundle_metadata(&bundle)?;
        for entry in &bundle.entries {
            verify_artifact(entry.compact_ref(), compact_assets)?;
        }

        overrides.sort_by(override_order);
        for contender in &overrides {
            verify_entry_metadata(&contender.entry)?;
            verify_artifact(contender.entry.compact_ref(), compact_assets)?;
            let targets = bundle
                .entries
                .iter()
                .filter(|entry| entry.skill_id == contender.target_skill_id)
                .collect::<Vec<_>>();
            if targets.is_empty() {
                return Err(MethodologyError::OverrideTargetMissing(
                    contender.target_skill_id.to_string(),
                ));
            }
            if !targets
                .iter()
                .any(|target| target.series_kind == contender.entry.series_kind)
            {
                return Err(MethodologyError::OverrideSeriesMismatch(
                    contender.target_skill_id.to_string(),
                ));
            }
        }
        verify_trace_budgets(&bundle.entries, &overrides)?;

        let project_registry_digests = project_registry_digests(&overrides)?;
        let catalog_digest = if overrides.is_empty() {
            bundle.catalog_digest
        } else {
            digest_json(&EffectiveCatalogDigestWire::new(
                bundle.catalog_digest,
                &overrides,
            ))?
        };
        Ok(Self {
            entries: bundle.entries,
            overrides,
            catalog_digest,
            project_registry_digests,
            vocabulary,
        })
    }

    /// Returns the number of indexed built-in Methodology entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the effective Catalog digest, including canonical override metadata.
    pub const fn catalog_digest(&self) -> ArtifactDigest {
        self.catalog_digest
    }

    /// Returns the deterministic project registry snapshot expected by queries.
    pub fn project_registry_digest(&self, project: &ProjectKey) -> Option<ArtifactDigest> {
        self.project_registry_digests.get(project).copied()
    }
}

pub(crate) struct ResolutionVocabulary {
    pub(crate) builtin_ambiguous: ReasonCode,
    pub(crate) project_registry_stale: ReasonCode,
    pub(crate) override_denied: ReasonCode,
    pub(crate) resolution_blocked: ReasonCode,
    pub(crate) same_layer_conflict: ReasonCode,
    pub(crate) deprecated: ReasonCode,
    pub(crate) contender_selected: ReasonCode,
    pub(crate) higher_priority_selected: ReasonCode,
    pub(crate) message_stale: MessageKey,
    pub(crate) message_override_denied: MessageKey,
    pub(crate) message_plugin_conflict: MessageKey,
    pub(crate) message_not_found: MessageKey,
    pub(crate) message_invalid: MessageKey,
}

impl ResolutionVocabulary {
    fn new() -> Result<Self, MethodologyError> {
        Ok(Self {
            builtin_ambiguous: reason("methodology.builtin-ambiguous")?,
            project_registry_stale: reason("methodology.project-registry-stale")?,
            override_denied: reason("methodology.override-denied")?,
            resolution_blocked: reason("methodology.resolution-blocked")?,
            same_layer_conflict: reason("methodology.same-layer-conflict")?,
            deprecated: reason("methodology.deprecated")?,
            contender_selected: reason("methodology.contender-selected")?,
            higher_priority_selected: reason("methodology.higher-priority-selected")?,
            message_stale: message("methodology.resolve.stale")?,
            message_override_denied: message("methodology.resolve.override-denied")?,
            message_plugin_conflict: message("methodology.resolve.plugin-conflict")?,
            message_not_found: message("methodology.resolve.not-found")?,
            message_invalid: message("methodology.resolve.invalid")?,
        })
    }
}

fn reason(value: &'static str) -> Result<ReasonCode, MethodologyError> {
    ReasonCode::new(value).map_err(|error| MethodologyError::InvalidField {
        field: "reasonCode",
        value: error.to_string(),
    })
}

fn message(value: &'static str) -> Result<MessageKey, MethodologyError> {
    MessageKey::new(value).map_err(|error| MethodologyError::InvalidField {
        field: "messageKey",
        value: error.to_string(),
    })
}

fn verify_trace_budgets(
    built_ins: &[CompiledMethodologyEntry],
    overrides: &[MethodologyOverride],
) -> Result<(), MethodologyError> {
    let mut common = BTreeMap::<(String, String), usize>::new();
    for entry in built_ins {
        *common
            .entry((entry.skill_id.to_string(), entry.series_kind.to_string()))
            .or_default() += 1;
    }
    let mut project_scoped = BTreeMap::<(String, String, ProjectKey), usize>::new();
    for contender in overrides {
        let target = contender.target_skill_id.to_string();
        let series_kind = contender.entry.series_kind.to_string();
        match &contender.scope {
            OverrideScope::AllProjects => {
                *common.entry((target, series_kind)).or_default() += 1;
            }
            OverrideScope::Project(project) => {
                *project_scoped
                    .entry((target, series_kind, project.clone()))
                    .or_default() += 1;
            }
        }
    }
    if common.values().any(|count| *count > MAX_OVERRIDE_TRACE) {
        return Err(MethodologyError::OverrideTraceLimit);
    }
    for ((target, series_kind, _), project_count) in project_scoped {
        let common_count = common
            .get(&(target, series_kind))
            .copied()
            .map_or(0, |count| count);
        if common_count + project_count > MAX_OVERRIDE_TRACE {
            return Err(MethodologyError::OverrideTraceLimit);
        }
    }
    Ok(())
}

pub(crate) const fn layer_priority(layer: OverrideLayer) -> u8 {
    match layer {
        OverrideLayer::Project => 0,
        OverrideLayer::Global => 1,
        OverrideLayer::Repository => 2,
        OverrideLayer::BuiltIn => 3,
    }
}

fn override_order(left: &MethodologyOverride, right: &MethodologyOverride) -> Ordering {
    layer_priority(left.layer)
        .cmp(&layer_priority(right.layer))
        .then_with(|| {
            left.target_skill_id
                .as_str()
                .cmp(right.target_skill_id.as_str())
        })
        .then_with(|| scope_order(&left.scope).cmp(&scope_order(&right.scope)))
        .then_with(|| {
            left.entry
                .skill_id
                .as_str()
                .cmp(right.entry.skill_id.as_str())
        })
        .then_with(|| {
            left.entry
                .variant
                .as_str()
                .cmp(right.entry.variant.as_str())
        })
        .then_with(|| left.entry.entry_digest.cmp(&right.entry.entry_digest))
        .then_with(|| left.registry_digest.cmp(&right.registry_digest))
        .then_with(|| {
            authorization_order(left.authorization).cmp(&authorization_order(right.authorization))
        })
}

fn scope_order(scope: &OverrideScope) -> (u8, &str) {
    match scope {
        OverrideScope::AllProjects => (0, ""),
        OverrideScope::Project(project) => (1, project.as_str()),
    }
}

const fn authorization_order(authorization: OverrideAuthorization) -> u8 {
    match authorization {
        OverrideAuthorization::Authorized => 0,
        OverrideAuthorization::Denied => 1,
    }
}

fn project_registry_digests(
    overrides: &[MethodologyOverride],
) -> Result<BTreeMap<ProjectKey, ArtifactDigest>, MethodologyError> {
    let mut grouped = BTreeMap::<ProjectKey, Vec<&MethodologyOverride>>::new();
    for contender in overrides {
        if let OverrideScope::Project(project) = &contender.scope {
            grouped.entry(project.clone()).or_default().push(contender);
        }
    }
    grouped
        .into_iter()
        .map(|(project, contenders)| {
            let digest = digest_json(&ProjectRegistryDigestWire::new(&project, &contenders))?;
            Ok((project, digest))
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectiveCatalogDigestWire {
    schema_version: &'static str,
    built_in_catalog_digest: String,
    overrides: Vec<OverrideDigestWire>,
}

impl EffectiveCatalogDigestWire {
    fn new(built_in_catalog_digest: ArtifactDigest, overrides: &[MethodologyOverride]) -> Self {
        Self {
            schema_version: "ae-sdd-effective-methodology-catalog/v1",
            built_in_catalog_digest: built_in_catalog_digest.to_string(),
            overrides: overrides.iter().map(OverrideDigestWire::from).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRegistryDigestWire {
    schema_version: &'static str,
    project_key: String,
    contenders: Vec<OverrideDigestWire>,
}

impl ProjectRegistryDigestWire {
    fn new(project: &ProjectKey, contenders: &[&MethodologyOverride]) -> Self {
        Self {
            schema_version: "ae-sdd-project-methodology-registry/v1",
            project_key: project.to_string(),
            contenders: contenders
                .iter()
                .map(|contender| OverrideDigestWire::from(*contender))
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OverrideDigestWire {
    layer: OverrideLayer,
    target_skill_id: String,
    candidate_skill_id: String,
    candidate_variant: String,
    candidate_entry_digest: String,
    scope: OverrideScopeWire,
    authorization: OverrideAuthorization,
    registry_digest: String,
}

impl From<&MethodologyOverride> for OverrideDigestWire {
    fn from(value: &MethodologyOverride) -> Self {
        Self {
            layer: value.layer,
            target_skill_id: value.target_skill_id.to_string(),
            candidate_skill_id: value.entry.skill_id.to_string(),
            candidate_variant: value.entry.variant.to_string(),
            candidate_entry_digest: value.entry.entry_digest.to_string(),
            scope: OverrideScopeWire::from(&value.scope),
            authorization: value.authorization,
            registry_digest: value.registry_digest.to_string(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "projectKey")]
enum OverrideScopeWire {
    AllProjects,
    Project(String),
}

impl From<&OverrideScope> for OverrideScopeWire {
    fn from(value: &OverrideScope) -> Self {
        match value {
            OverrideScope::AllProjects => Self::AllProjects,
            OverrideScope::Project(project) => Self::Project(project.to_string()),
        }
    }
}
