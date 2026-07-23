//! Methodology Catalog lookup, reference, and override provenance contracts.

use ae_sdd_domain::{ArtifactDigest, ArtifactRef, DecisionDigest, ProjectKey, ProjectRelativePath};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ControlPlaneError, MethodologyVariant, ReasonCode, SchemaVersion, SeriesKind, SkillId,
    serde_domain,
};

/// Maximum number of contenders retained in an override trace.
pub const MAX_OVERRIDE_TRACE: usize = 16;

/// Error returned when a Methodology reference violates its contract.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MethodologyRefError {
    /// Compact and fallback references resolved to the same artifact path.
    #[error("compact and fallback references must identify different artifacts")]
    DuplicateArtifactReference,
}

/// Content-addressed reference to a compiled Methodology entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "MethodologyRefWire", into = "MethodologyRefWire")]
pub struct MethodologyRef {
    schema_version: SchemaVersion,
    skill_id: SkillId,
    series_kind: SeriesKind,
    variant: MethodologyVariant,
    compact_ref: ArtifactRef,
    fallback_ref: Option<ArtifactRef>,
    entry_digest: ArtifactDigest,
    catalog_digest: ArtifactDigest,
}

impl MethodologyRef {
    /// Constructs a validated content-addressed reference.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        skill_id: SkillId,
        series_kind: SeriesKind,
        variant: MethodologyVariant,
        compact_ref: ArtifactRef,
        fallback_ref: Option<ArtifactRef>,
        entry_digest: ArtifactDigest,
        catalog_digest: ArtifactDigest,
    ) -> Result<Self, MethodologyRefError> {
        if fallback_ref
            .as_ref()
            .is_some_and(|fallback| fallback.path() == compact_ref.path())
        {
            return Err(MethodologyRefError::DuplicateArtifactReference);
        }
        Ok(Self {
            schema_version,
            skill_id,
            series_kind,
            variant,
            compact_ref,
            fallback_ref,
            entry_digest,
            catalog_digest,
        })
    }

    /// Returns the referenced skill identifier.
    pub const fn skill_id(&self) -> &SkillId {
        &self.skill_id
    }

    /// Returns the Methodology series kind.
    pub const fn series_kind(&self) -> &SeriesKind {
        &self.series_kind
    }

    /// Returns the compact Methodology artifact reference.
    pub const fn compact_ref(&self) -> &ArtifactRef {
        &self.compact_ref
    }

    /// Returns the optional lazy fallback artifact reference.
    pub const fn fallback_ref(&self) -> Option<&ArtifactRef> {
        self.fallback_ref.as_ref()
    }

    /// Returns the entry content digest.
    pub const fn entry_digest(&self) -> ArtifactDigest {
        self.entry_digest
    }

    /// Returns the Catalog snapshot digest.
    pub const fn catalog_digest(&self) -> ArtifactDigest {
        self.catalog_digest
    }
}

impl<'de> Deserialize<'de> for MethodologyRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        MethodologyRefWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MethodologyRefWire {
    schema_version: SchemaVersion,
    skill_id: SkillId,
    series_kind: SeriesKind,
    variant: MethodologyVariant,
    #[serde(with = "serde_domain::artifact_ref")]
    compact_ref: ArtifactRef,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_domain::optional_artifact_ref"
    )]
    fallback_ref: Option<ArtifactRef>,
    #[serde(with = "serde_domain::artifact_digest")]
    entry_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    catalog_digest: ArtifactDigest,
}

impl TryFrom<MethodologyRefWire> for MethodologyRef {
    type Error = MethodologyRefError;

    fn try_from(value: MethodologyRefWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.skill_id,
            value.series_kind,
            value.variant,
            value.compact_ref,
            value.fallback_ref,
            value.entry_digest,
            value.catalog_digest,
        )
    }
}

impl From<MethodologyRef> for MethodologyRefWire {
    fn from(value: MethodologyRef) -> Self {
        Self {
            schema_version: value.schema_version,
            skill_id: value.skill_id,
            series_kind: value.series_kind,
            variant: value.variant,
            compact_ref: value.compact_ref,
            fallback_ref: value.fallback_ref,
            entry_digest: value.entry_digest,
            catalog_digest: value.catalog_digest,
        }
    }
}

/// Catalog layer participating in deterministic Methodology override selection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideLayer {
    /// Project-scoped registry.
    Project,
    /// User/global registry snapshot.
    Global,
    /// Repository-packaged plugin registry.
    Repository,
    /// Built-in compiled fallback.
    BuiltIn,
}

/// Audited outcome for one Methodology contender.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideDisposition {
    /// This contender became the winner.
    Selected,
    /// A higher-priority valid contender shadowed this contender.
    Shadowed,
    /// The contender was invalid or unauthorized and resolution failed closed.
    Rejected,
}

/// One deterministic entry in the ordered override audit trace.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverrideTrace {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Registry layer.
    pub layer: OverrideLayer,
    /// Candidate skill identity.
    pub candidate: SkillId,
    /// Candidate outcome.
    pub disposition: OverrideDisposition,
    /// Stable selection or rejection reason.
    pub reason_code: ReasonCode,
    /// Candidate content digest.
    #[serde(with = "serde_domain::artifact_digest")]
    pub candidate_digest: ArtifactDigest,
}

impl OverrideTrace {
    /// Constructs an override trace item.
    pub const fn new(
        schema_version: SchemaVersion,
        layer: OverrideLayer,
        candidate: SkillId,
        disposition: OverrideDisposition,
        reason_code: ReasonCode,
        candidate_digest: ArtifactDigest,
    ) -> Self {
        Self {
            schema_version,
            layer,
            candidate,
            disposition,
            reason_code,
            candidate_digest,
        }
    }
}

/// Error returned when Methodology resolution trace invariants are violated.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum MethodologyResolutionError {
    /// The audit trace was empty.
    #[error("Methodology resolution requires an override trace")]
    EmptyTrace,
    /// The audit trace exceeded its frozen v1 budget.
    #[error("Methodology override trace exceeds its frozen v1 limit")]
    TraceTooLarge,
    /// The trace did not contain exactly one selected contender.
    #[error("Methodology override trace must contain exactly one selected contender")]
    InvalidWinnerCount,
    /// The selected trace layer did not match the declared winner.
    #[error("Methodology winner layer does not match the selected trace item")]
    WinnerLayerMismatch,
}

/// Deterministic Methodology winner plus complete ordered override provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    try_from = "MethodologyResolutionWire",
    into = "MethodologyResolutionWire"
)]
pub struct MethodologyResolution {
    schema_version: SchemaVersion,
    methodology_ref: MethodologyRef,
    winner_layer: OverrideLayer,
    override_trace: Vec<OverrideTrace>,
    resolution_digest: DecisionDigest,
}

impl MethodologyResolution {
    /// Constructs and validates a deterministic resolution.
    pub fn new(
        schema_version: SchemaVersion,
        methodology_ref: MethodologyRef,
        winner_layer: OverrideLayer,
        override_trace: Vec<OverrideTrace>,
        resolution_digest: DecisionDigest,
    ) -> Result<Self, MethodologyResolutionError> {
        if override_trace.is_empty() {
            return Err(MethodologyResolutionError::EmptyTrace);
        }
        if override_trace.len() > MAX_OVERRIDE_TRACE {
            return Err(MethodologyResolutionError::TraceTooLarge);
        }
        let mut selected = override_trace
            .iter()
            .filter(|item| item.disposition == OverrideDisposition::Selected);
        let winner = selected
            .next()
            .ok_or(MethodologyResolutionError::InvalidWinnerCount)?;
        if selected.next().is_some() {
            return Err(MethodologyResolutionError::InvalidWinnerCount);
        }
        if winner.layer != winner_layer {
            return Err(MethodologyResolutionError::WinnerLayerMismatch);
        }
        Ok(Self {
            schema_version,
            methodology_ref,
            winner_layer,
            override_trace,
            resolution_digest,
        })
    }

    /// Returns the selected Methodology reference.
    pub const fn methodology_ref(&self) -> &MethodologyRef {
        &self.methodology_ref
    }

    /// Returns the winner layer.
    pub const fn winner_layer(&self) -> OverrideLayer {
        self.winner_layer
    }

    /// Returns the ordered override trace.
    pub fn override_trace(&self) -> &[OverrideTrace] {
        &self.override_trace
    }

    /// Returns the canonical resolution digest.
    pub const fn resolution_digest(&self) -> DecisionDigest {
        self.resolution_digest
    }
}

impl<'de> Deserialize<'de> for MethodologyResolution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        MethodologyResolutionWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MethodologyResolutionWire {
    schema_version: SchemaVersion,
    methodology_ref: MethodologyRef,
    winner_layer: OverrideLayer,
    override_trace: Vec<OverrideTrace>,
    #[serde(with = "serde_domain::decision_digest")]
    resolution_digest: DecisionDigest,
}

impl TryFrom<MethodologyResolutionWire> for MethodologyResolution {
    type Error = MethodologyResolutionError;

    fn try_from(value: MethodologyResolutionWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.methodology_ref,
            value.winner_layer,
            value.override_trace,
            value.resolution_digest,
        )
    }
}

impl From<MethodologyResolution> for MethodologyResolutionWire {
    fn from(value: MethodologyResolution) -> Self {
        Self {
            schema_version: value.schema_version,
            methodology_ref: value.methodology_ref,
            winner_layer: value.winner_layer,
            override_trace: value.override_trace,
            resolution_digest: value.resolution_digest,
        }
    }
}

/// Read-only Catalog boundary implemented by `ae-sdd-methodology`.
pub trait MethodologyCatalogPort {
    /// Resolves a Methodology winner from a frozen Catalog snapshot.
    fn resolve(&self, query: &MethodologyQuery)
    -> Result<MethodologyResolution, ControlPlaneError>;
}

/// Minimal Methodology lookup query.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodologyQuery {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Required workflow or capability kind.
    pub series_kind: SeriesKind,
    /// Project-bounded resolution scope.
    pub project_scope: ProjectScope,
    /// Optional exact requested skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_skill: Option<SkillId>,
    /// Catalog digest expected by the caller.
    #[serde(with = "serde_domain::artifact_digest")]
    pub catalog_digest: ArtifactDigest,
    /// Optional project registry snapshot digest.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_artifact_digest"
    )]
    pub project_registry_digest: Option<ArtifactDigest>,
}

impl MethodologyQuery {
    /// Constructs a strict Methodology query.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        schema_version: SchemaVersion,
        series_kind: SeriesKind,
        project_scope: ProjectScope,
        requested_skill: Option<SkillId>,
        catalog_digest: ArtifactDigest,
        project_registry_digest: Option<ArtifactDigest>,
    ) -> Self {
        Self {
            schema_version,
            series_kind,
            project_scope,
            requested_skill,
            catalog_digest,
            project_registry_digest,
        }
    }
}

/// Project-relative Catalog resolution boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectScope {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Stable project key.
    #[serde(with = "serde_domain::project_key")]
    pub project_key: ProjectKey,
    /// Optional project-relative subtree for scoped overrides.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_project_relative_path"
    )]
    pub relative_scope: Option<ProjectRelativePath>,
}

impl ProjectScope {
    /// Constructs a project-bounded scope.
    pub const fn new(
        schema_version: SchemaVersion,
        project_key: ProjectKey,
        relative_scope: Option<ProjectRelativePath>,
    ) -> Self {
        Self {
            schema_version,
            project_key,
            relative_scope,
        }
    }
}

mod optional_artifact_digest {
    use std::str::FromStr;

    use ae_sdd_domain::ArtifactDigest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ArtifactDigest>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|digest| digest.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ArtifactDigest>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ArtifactDigest::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

mod optional_project_relative_path {
    use ae_sdd_domain::ProjectRelativePath;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ProjectRelativePath>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .as_ref()
            .map(ToString::to_string)
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<ProjectRelativePath>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ProjectRelativePath::new(value).map_err(de::Error::custom))
            .transpose()
    }
}
