use std::fmt;

use ae_sdd_contracts::{
    ControlPlaneError, ControlPlaneErrorCode, MethodologyCatalogPort, MethodologyQuery,
    MethodologyRef, MethodologyResolution, OverrideDisposition, OverrideLayer, OverrideTrace,
    ReasonCode, RetryClass, SchemaVersion,
};
use ae_sdd_domain::{ArtifactDigest, DecisionDigest};
use serde::Serialize;

use crate::{
    Activation, CompiledMethodologyEntry, MethodologyCatalog, OverrideAuthorization, SpawnPolicy,
    catalog::ResolutionVocabulary,
    registry::{SelectionCandidateView, analyze_selection},
};

/// Stable reason a Methodology query could not produce a usable winner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MethodologyResolveErrorKind {
    /// The caller queried a stale Catalog snapshot.
    CatalogStale,
    /// No entry matched the requested skill and Series kind.
    NotFound,
    /// More than one built-in entry matched an under-specified query.
    Ambiguous,
    /// A compatibility-only entry was requested.
    Deprecated,
    /// An inline capability was requested as a physical Series.
    PhysicalSpawnForbidden,
    /// Frozen cross-Part resolution invariants could not be constructed.
    ResolutionInvalid,
    /// A matching override was explicitly unauthorized.
    UnauthorizedOverride,
    /// More than one contender remained at the same priority layer.
    SameLayerConflict,
    /// The project registry snapshot did not match the query binding.
    ProjectRegistryStale,
}

/// Fail-closed resolution error retaining the complete ordered contender trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodologyResolveError {
    kind: MethodologyResolveErrorKind,
    trace: Vec<OverrideTrace>,
}

impl MethodologyResolveError {
    pub(crate) fn new(kind: MethodologyResolveErrorKind, trace: Vec<OverrideTrace>) -> Self {
        Self { kind, trace }
    }

    /// Returns the stable failure classification.
    pub const fn kind(&self) -> MethodologyResolveErrorKind {
        self.kind
    }

    /// Returns every contender considered before the fail-closed result.
    pub fn trace(&self) -> &[OverrideTrace] {
        &self.trace
    }
}

impl fmt::Display for MethodologyResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Methodology resolution failed: {:?}", self.kind)
    }
}

impl std::error::Error for MethodologyResolveError {}

impl MethodologyCatalog {
    /// Resolves a deterministic winner and retains a complete contender trace on failure.
    pub fn resolve_with_trace(
        &self,
        query: &MethodologyQuery,
    ) -> Result<MethodologyResolution, MethodologyResolveError> {
        if query.catalog_digest != self.catalog_digest {
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::CatalogStale,
                Vec::new(),
            ));
        }

        let built_ins = self
            .entries
            .iter()
            .filter(|entry| entry.series_kind == query.series_kind)
            .filter(|entry| {
                query
                    .requested_skill
                    .as_ref()
                    .is_none_or(|requested| requested == &entry.skill_id)
            })
            .collect::<Vec<_>>();
        let Some(built_in) = built_ins.first().copied() else {
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::NotFound,
                Vec::new(),
            ));
        };
        if built_ins.len() > 1 {
            let trace = built_ins
                .into_iter()
                .map(|entry| {
                    trace_item(
                        OverrideLayer::BuiltIn,
                        entry,
                        OverrideDisposition::Rejected,
                        &self.vocabulary.builtin_ambiguous,
                    )
                })
                .collect();
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::Ambiguous,
                trace,
            ));
        }

        let mut contenders = self
            .overrides
            .iter()
            .filter(|contender| contender.target_skill_id == built_in.skill_id)
            .filter(|contender| contender.applies_to(&query.project_scope.project_key))
            .map(|contender| Contender {
                layer: contender.layer,
                entry: &contender.entry,
                target: built_in.skill_id.as_str(),
                source_digest: contender.registry_digest,
                authorization: contender.authorization,
            })
            .collect::<Vec<_>>();
        contenders.push(Contender {
            layer: OverrideLayer::BuiltIn,
            entry: built_in,
            target: built_in.skill_id.as_str(),
            source_digest: self.catalog_digest,
            authorization: OverrideAuthorization::Authorized,
        });
        let analysis = analyze_selection(&contenders);

        if query.project_registry_digest
            != self.project_registry_digest(&query.project_scope.project_key)
        {
            let trace = analysis
                .order
                .iter()
                .map(|index| {
                    let contender = &contenders[*index];
                    trace_item(
                        contender.layer,
                        contender.entry,
                        OverrideDisposition::Rejected,
                        &self.vocabulary.project_registry_stale,
                    )
                })
                .collect();
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::ProjectRegistryStale,
                trace,
            ));
        }

        if !analysis.unauthorized.is_empty() {
            let trace = analysis
                .order
                .iter()
                .map(|index| {
                    let contender = &contenders[*index];
                    let denied = analysis.unauthorized.contains(index);
                    trace_item(
                        contender.layer,
                        contender.entry,
                        if denied {
                            OverrideDisposition::Rejected
                        } else {
                            OverrideDisposition::Shadowed
                        },
                        if denied {
                            &self.vocabulary.override_denied
                        } else {
                            &self.vocabulary.resolution_blocked
                        },
                    )
                })
                .collect();
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::UnauthorizedOverride,
                trace,
            ));
        }

        if !analysis.target_conflicts.is_empty() {
            let trace = analysis
                .order
                .iter()
                .map(|index| {
                    let contender = &contenders[*index];
                    let conflict = analysis.target_conflict_indices.contains(index);
                    trace_item(
                        contender.layer,
                        contender.entry,
                        if conflict {
                            OverrideDisposition::Rejected
                        } else {
                            OverrideDisposition::Shadowed
                        },
                        if conflict {
                            &self.vocabulary.same_layer_conflict
                        } else {
                            &self.vocabulary.resolution_blocked
                        },
                    )
                })
                .collect();
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::SameLayerConflict,
                trace,
            ));
        }

        let winner_index = analysis
            .winners
            .get(built_in.skill_id.as_str())
            .copied()
            .ok_or_else(|| {
                MethodologyResolveError::new(
                    MethodologyResolveErrorKind::ResolutionInvalid,
                    Vec::new(),
                )
            })?;
        let winner = contenders[winner_index];
        let deprecated = winner.entry.activation == Activation::Deprecated;
        let trace = analysis
            .order
            .iter()
            .map(|index| {
                let contender = &contenders[*index];
                let selected = *index == winner_index;
                trace_item(
                    contender.layer,
                    contender.entry,
                    if selected && deprecated {
                        OverrideDisposition::Rejected
                    } else if selected {
                        OverrideDisposition::Selected
                    } else {
                        OverrideDisposition::Shadowed
                    },
                    if selected && deprecated {
                        &self.vocabulary.deprecated
                    } else if selected {
                        &self.vocabulary.contender_selected
                    } else {
                        &self.vocabulary.higher_priority_selected
                    },
                )
            })
            .collect::<Vec<_>>();
        if deprecated {
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::Deprecated,
                trace,
            ));
        }
        resolution(winner.entry, winner.layer, self.catalog_digest, trace)
    }

    /// Resolves only Methodologies authorized to create a physical Series session.
    pub fn resolve_physical(
        &self,
        query: &MethodologyQuery,
    ) -> Result<MethodologyResolution, MethodologyResolveError> {
        let resolution = self.resolve_with_trace(query)?;
        let selected = self
            .entries
            .iter()
            .chain(self.overrides.iter().map(|contender| &contender.entry))
            .find(|entry| {
                entry.entry_digest() == resolution.methodology_ref().entry_digest()
                    && entry.skill_id() == resolution.methodology_ref().skill_id()
            })
            .ok_or_else(|| {
                MethodologyResolveError::new(
                    MethodologyResolveErrorKind::ResolutionInvalid,
                    resolution.override_trace().to_vec(),
                )
            })?;
        if selected.spawn_policy() != SpawnPolicy::PhysicalSeries {
            return Err(MethodologyResolveError::new(
                MethodologyResolveErrorKind::PhysicalSpawnForbidden,
                resolution.override_trace().to_vec(),
            ));
        }
        Ok(resolution)
    }
}

impl MethodologyCatalogPort for MethodologyCatalog {
    fn resolve(
        &self,
        query: &MethodologyQuery,
    ) -> Result<MethodologyResolution, ControlPlaneError> {
        self.resolve_with_trace(query)
            .map_err(|error| control_plane_error(&self.vocabulary, error))
    }
}

#[derive(Clone, Copy)]
struct Contender<'a> {
    layer: OverrideLayer,
    entry: &'a CompiledMethodologyEntry,
    target: &'a str,
    source_digest: ArtifactDigest,
    authorization: OverrideAuthorization,
}

impl SelectionCandidateView for Contender<'_> {
    fn selection_name(&self) -> &str {
        self.entry.skill_id.as_str()
    }

    fn selection_target(&self) -> &str {
        self.target
    }

    fn selection_layer(&self) -> OverrideLayer {
        self.layer
    }

    fn selection_source_digest(&self) -> ArtifactDigest {
        self.source_digest
    }

    fn selection_content_digest(&self) -> ArtifactDigest {
        self.entry.entry_digest
    }

    fn selection_authorized(&self) -> bool {
        self.authorization == OverrideAuthorization::Authorized
    }
}

fn resolution(
    entry: &CompiledMethodologyEntry,
    winner_layer: OverrideLayer,
    catalog_digest: ArtifactDigest,
    trace: Vec<OverrideTrace>,
) -> Result<MethodologyResolution, MethodologyResolveError> {
    let methodology_ref = MethodologyRef::new(
        SchemaVersion::V1,
        entry.skill_id.clone(),
        entry.series_kind.clone(),
        entry.variant.clone(),
        entry.compact_ref.clone(),
        entry.fallback_ref.clone(),
        entry.entry_digest,
        catalog_digest,
    )
    .map_err(|_| {
        MethodologyResolveError::new(
            MethodologyResolveErrorKind::ResolutionInvalid,
            trace.clone(),
        )
    })?;
    let digest = DecisionDigest::digest(
        serde_json::to_vec(&ResolutionDigestMaterial {
            catalog_digest: catalog_digest.to_string(),
            winner_digest: entry.entry_digest.to_string(),
            winner_layer,
            trace: &trace,
        })
        .map_err(|_| {
            MethodologyResolveError::new(
                MethodologyResolveErrorKind::ResolutionInvalid,
                trace.clone(),
            )
        })?,
    );
    MethodologyResolution::new(
        SchemaVersion::V1,
        methodology_ref,
        winner_layer,
        trace.clone(),
        digest,
    )
    .map_err(|_| {
        MethodologyResolveError::new(MethodologyResolveErrorKind::ResolutionInvalid, trace)
    })
}

fn trace_item(
    layer: OverrideLayer,
    entry: &CompiledMethodologyEntry,
    disposition: OverrideDisposition,
    reason: &ReasonCode,
) -> OverrideTrace {
    OverrideTrace::new(
        SchemaVersion::V1,
        layer,
        entry.skill_id.clone(),
        disposition,
        reason.clone(),
        entry.entry_digest,
    )
}

fn control_plane_error(
    vocabulary: &ResolutionVocabulary,
    error: MethodologyResolveError,
) -> ControlPlaneError {
    let (code, retry, message_key) = match error.kind {
        MethodologyResolveErrorKind::CatalogStale
        | MethodologyResolveErrorKind::ProjectRegistryStale => (
            ControlPlaneErrorCode::MethodologyDigestMismatch,
            RetryClass::AfterInputRepair,
            vocabulary.message_stale.clone(),
        ),
        MethodologyResolveErrorKind::UnauthorizedOverride => (
            ControlPlaneErrorCode::PluginOverrideDenied,
            RetryClass::AfterInputRepair,
            vocabulary.message_override_denied.clone(),
        ),
        MethodologyResolveErrorKind::SameLayerConflict => (
            ControlPlaneErrorCode::PluginConflict,
            RetryClass::AfterInputRepair,
            vocabulary.message_plugin_conflict.clone(),
        ),
        MethodologyResolveErrorKind::NotFound => (
            ControlPlaneErrorCode::MethodologyCompactMissing,
            RetryClass::AfterInputRepair,
            vocabulary.message_not_found.clone(),
        ),
        MethodologyResolveErrorKind::Ambiguous
        | MethodologyResolveErrorKind::Deprecated
        | MethodologyResolveErrorKind::PhysicalSpawnForbidden
        | MethodologyResolveErrorKind::ResolutionInvalid => (
            ControlPlaneErrorCode::ContractValidationFailed,
            RetryClass::NoRetry,
            vocabulary.message_invalid.clone(),
        ),
    };
    let details_digest = serde_json::to_vec(&error.trace)
        .ok()
        .map(ArtifactDigest::digest);
    ControlPlaneError {
        schema_version: SchemaVersion::V1,
        code,
        retry,
        message_key,
        remediation: Vec::new(),
        details_digest,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolutionDigestMaterial<'a> {
    catalog_digest: String,
    winner_digest: String,
    winner_layer: OverrideLayer,
    trace: &'a [OverrideTrace],
}
