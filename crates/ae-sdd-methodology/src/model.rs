use std::str::FromStr;

use ae_sdd_contracts::{BoundedText, MethodologyVariant, SeriesKind, SkillId};
use ae_sdd_domain::{ArtifactDigest, ArtifactKind, ArtifactRef, ProjectRelativePath};
use serde::{Deserialize, Serialize};

use crate::MethodologyError;

pub(crate) const CATALOG_SCHEMA_V1: &str = "ae-sdd-methodology-catalog/v1";
pub(crate) const BUNDLE_SCHEMA_V1: &str = "ae-sdd-methodology-bundle/v1";
pub(crate) const COMPACT_ARTIFACT_KIND: &str = "methodology-compact";
pub(crate) const FALLBACK_ARTIFACT_KIND: &str = "methodology-fallback";

/// How a Methodology entry participates in the runtime.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    /// A semantic workflow Series executed by a physical child Agent.
    Workflow,
    /// An inline capability consumed by another workflow or supervisor.
    Capability,
    /// A retained compatibility identity that cannot be resolved.
    Deprecated,
}

/// Whether an entry may create a physical Series session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnPolicy {
    /// The entry must execute in an independently attested Series session.
    PhysicalSeries,
    /// The entry is an inline capability and must not spawn a Series.
    Inline,
    /// The entry is retained only for compatibility and cannot execute.
    Forbidden,
}

/// Operator used by one typed route predicate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateOperator {
    /// Exact string equality.
    Equals,
    /// Stable collection or string containment.
    Contains,
    /// Presence of the named fact.
    Present,
    /// Exact string inequality.
    NotEquals,
}

/// Normalized, bounded route predicate retained in Methodology metadata.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutePredicate {
    fact: BoundedText<128>,
    operator: PredicateOperator,
    value: BoundedText<256>,
}

impl RoutePredicate {
    pub(crate) fn new(
        fact: BoundedText<128>,
        operator: PredicateOperator,
        value: BoundedText<256>,
    ) -> Self {
        Self {
            fact,
            operator,
            value,
        }
    }

    pub(crate) const fn fact(&self) -> &BoundedText<128> {
        &self.fact
    }
}

/// Content-addressed, normalized Methodology entry emitted by the compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledMethodologyEntry {
    pub(crate) skill_id: SkillId,
    pub(crate) series_kind: SeriesKind,
    pub(crate) variant: MethodologyVariant,
    pub(crate) version: BoundedText<32>,
    pub(crate) activation: Activation,
    pub(crate) spawn_policy: SpawnPolicy,
    pub(crate) compact_ref: ArtifactRef,
    pub(crate) fallback_ref: Option<ArtifactRef>,
    pub(crate) route_predicates: Vec<RoutePredicate>,
    pub(crate) required_inputs: Vec<BoundedText<128>>,
    pub(crate) deliverable_kinds: Vec<BoundedText<128>>,
    pub(crate) required_gates: Vec<BoundedText<128>>,
    pub(crate) tool_dependencies: Vec<BoundedText<128>>,
    pub(crate) entry_digest: ArtifactDigest,
}

impl CompiledMethodologyEntry {
    /// Returns the stable skill identity.
    pub const fn skill_id(&self) -> &SkillId {
        &self.skill_id
    }

    /// Returns the Series kind selected by routing.
    pub const fn series_kind(&self) -> &SeriesKind {
        &self.series_kind
    }

    /// Returns the compiled Methodology variant.
    pub const fn variant(&self) -> &MethodologyVariant {
        &self.variant
    }

    /// Returns the activation classification.
    pub const fn activation(&self) -> Activation {
        self.activation
    }

    /// Returns the physical spawn policy.
    pub const fn spawn_policy(&self) -> SpawnPolicy {
        self.spawn_policy
    }

    /// Returns the bounded compact artifact reference.
    pub const fn compact_ref(&self) -> &ArtifactRef {
        &self.compact_ref
    }

    /// Returns fallback metadata without loading its body.
    pub const fn fallback_ref(&self) -> Option<&ArtifactRef> {
        self.fallback_ref.as_ref()
    }

    /// Returns the content-bound semantic entry digest.
    pub const fn entry_digest(&self) -> ArtifactDigest {
        self.entry_digest
    }

    pub(crate) fn digest_material(&self) -> EntryDigestWire {
        EntryDigestWire::from_entry(self)
    }
}

/// Canonical compiled Methodology bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledMethodologyBundle {
    pub(crate) catalog_version: BoundedText<32>,
    pub(crate) entries: Vec<CompiledMethodologyEntry>,
    pub(crate) catalog_digest: ArtifactDigest,
}

impl CompiledMethodologyBundle {
    /// Returns the number of compiled entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns normalized entries in canonical identity order.
    pub fn entries(&self) -> &[CompiledMethodologyEntry] {
        &self.entries
    }

    /// Returns the canonical Catalog snapshot digest.
    pub const fn catalog_digest(&self) -> ArtifactDigest {
        self.catalog_digest
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ArtifactMetadataWire {
    path: String,
    digest: String,
    byte_length: u64,
}

impl ArtifactMetadataWire {
    fn from_ref(value: &ArtifactRef) -> Self {
        Self {
            path: value.path().to_string(),
            digest: value.digest().to_string(),
            byte_length: value.byte_length(),
        }
    }

    fn into_ref(self, kind: &'static str) -> Result<ArtifactRef, MethodologyError> {
        let kind = ArtifactKind::new(kind).map_err(|error| MethodologyError::InvalidField {
            field: "artifactKind",
            value: error.to_string(),
        })?;
        let path = ProjectRelativePath::new(self.path)
            .map_err(|error| MethodologyError::InvalidPath(error.to_string()))?;
        let digest = ArtifactDigest::from_str(&self.digest).map_err(|error| {
            MethodologyError::InvalidField {
                field: "artifactDigest",
                value: error.to_string(),
            }
        })?;
        Ok(ArtifactRef::new(kind, path, digest, self.byte_length))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CompiledEntryWire {
    skill_id: String,
    series_kind: String,
    variant: String,
    version: String,
    activation: Activation,
    spawn_policy: SpawnPolicy,
    compact_ref: ArtifactMetadataWire,
    fallback_ref: RequiredNullableArtifactMetadata,
    route_predicates: Vec<RoutePredicate>,
    required_inputs: Vec<BoundedText<128>>,
    deliverable_kinds: Vec<BoundedText<128>>,
    required_gates: Vec<BoundedText<128>>,
    tool_dependencies: Vec<BoundedText<128>>,
    entry_digest: String,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum RequiredNullableArtifactMetadata {
    Value(ArtifactMetadataWire),
    Null,
}

impl RequiredNullableArtifactMetadata {
    fn into_option(self) -> Option<ArtifactMetadataWire> {
        match self {
            Self::Value(value) => Some(value),
            Self::Null => None,
        }
    }
}

impl CompiledEntryWire {
    fn from_entry(value: &CompiledMethodologyEntry) -> Self {
        Self {
            skill_id: value.skill_id.to_string(),
            series_kind: value.series_kind.to_string(),
            variant: value.variant.to_string(),
            version: value.version.to_string(),
            activation: value.activation,
            spawn_policy: value.spawn_policy,
            compact_ref: ArtifactMetadataWire::from_ref(&value.compact_ref),
            fallback_ref: value.fallback_ref.as_ref().map_or(
                RequiredNullableArtifactMetadata::Null,
                |reference| {
                    RequiredNullableArtifactMetadata::Value(ArtifactMetadataWire::from_ref(
                        reference,
                    ))
                },
            ),
            route_predicates: value.route_predicates.clone(),
            required_inputs: value.required_inputs.clone(),
            deliverable_kinds: value.deliverable_kinds.clone(),
            required_gates: value.required_gates.clone(),
            tool_dependencies: value.tool_dependencies.clone(),
            entry_digest: value.entry_digest.to_string(),
        }
    }

    fn into_entry(self) -> Result<CompiledMethodologyEntry, MethodologyError> {
        let entry = CompiledMethodologyEntry {
            skill_id: SkillId::new(self.skill_id).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "skillId",
                    value: error.to_string(),
                }
            })?,
            series_kind: SeriesKind::new(self.series_kind).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "seriesKind",
                    value: error.to_string(),
                }
            })?,
            variant: MethodologyVariant::new(self.variant).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "variant",
                    value: error.to_string(),
                }
            })?,
            version: BoundedText::new(self.version).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "version",
                    value: error.to_string(),
                }
            })?,
            activation: self.activation,
            spawn_policy: self.spawn_policy,
            compact_ref: self.compact_ref.into_ref(COMPACT_ARTIFACT_KIND)?,
            fallback_ref: self
                .fallback_ref
                .into_option()
                .map(|value| value.into_ref(FALLBACK_ARTIFACT_KIND))
                .transpose()?,
            route_predicates: self.route_predicates,
            required_inputs: self.required_inputs,
            deliverable_kinds: self.deliverable_kinds,
            required_gates: self.required_gates,
            tool_dependencies: self.tool_dependencies,
            entry_digest: ArtifactDigest::from_str(&self.entry_digest).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "entryDigest",
                    value: error.to_string(),
                }
            })?,
        };
        Ok(entry)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntryDigestWire {
    skill_id: String,
    series_kind: String,
    variant: String,
    version: String,
    activation: Activation,
    spawn_policy: SpawnPolicy,
    compact_ref: ArtifactMetadataWire,
    fallback_ref: Option<ArtifactMetadataWire>,
    route_predicates: Vec<RoutePredicate>,
    required_inputs: Vec<BoundedText<128>>,
    deliverable_kinds: Vec<BoundedText<128>>,
    required_gates: Vec<BoundedText<128>>,
    tool_dependencies: Vec<BoundedText<128>>,
}

impl EntryDigestWire {
    fn from_entry(value: &CompiledMethodologyEntry) -> Self {
        Self {
            skill_id: value.skill_id.to_string(),
            series_kind: value.series_kind.to_string(),
            variant: value.variant.to_string(),
            version: value.version.to_string(),
            activation: value.activation,
            spawn_policy: value.spawn_policy,
            compact_ref: ArtifactMetadataWire::from_ref(&value.compact_ref),
            fallback_ref: value
                .fallback_ref
                .as_ref()
                .map(ArtifactMetadataWire::from_ref),
            route_predicates: value.route_predicates.clone(),
            required_inputs: value.required_inputs.clone(),
            deliverable_kinds: value.deliverable_kinds.clone(),
            required_gates: value.required_gates.clone(),
            tool_dependencies: value.tool_dependencies.clone(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BundleWire {
    schema_version: String,
    catalog_version: String,
    entries: Vec<CompiledEntryWire>,
    catalog_digest: String,
}

impl BundleWire {
    pub(crate) fn from_bundle(value: &CompiledMethodologyBundle) -> Self {
        Self {
            schema_version: BUNDLE_SCHEMA_V1.to_owned(),
            catalog_version: value.catalog_version.to_string(),
            entries: value
                .entries
                .iter()
                .map(CompiledEntryWire::from_entry)
                .collect(),
            catalog_digest: value.catalog_digest.to_string(),
        }
    }

    pub(crate) fn into_bundle(self) -> Result<CompiledMethodologyBundle, MethodologyError> {
        if self.schema_version != BUNDLE_SCHEMA_V1 {
            return Err(MethodologyError::UnsupportedSchema(self.schema_version));
        }
        let entries = self
            .entries
            .into_iter()
            .map(CompiledEntryWire::into_entry)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CompiledMethodologyBundle {
            catalog_version: BoundedText::new(self.catalog_version).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "catalogVersion",
                    value: error.to_string(),
                }
            })?,
            entries,
            catalog_digest: ArtifactDigest::from_str(&self.catalog_digest).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "catalogDigest",
                    value: error.to_string(),
                }
            })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogDigestWire {
    schema_version: &'static str,
    catalog_version: String,
    entries: Vec<CompiledEntryWire>,
}

impl CatalogDigestWire {
    pub(crate) fn new(
        catalog_version: &BoundedText<32>,
        entries: &[CompiledMethodologyEntry],
    ) -> Self {
        Self {
            schema_version: BUNDLE_SCHEMA_V1,
            catalog_version: catalog_version.to_string(),
            entries: entries.iter().map(CompiledEntryWire::from_entry).collect(),
        }
    }
}
