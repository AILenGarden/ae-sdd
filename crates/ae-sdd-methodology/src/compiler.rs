use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_contracts::{
    BoundedText, MAIN_NODE_SERIES_KINDS, MethodologyVariant, SeriesActivity, SeriesKind, SkillId,
};
use ae_sdd_domain::{ArtifactDigest, ArtifactKind, ArtifactRef, ProjectRelativePath};
use serde::Deserialize;

use crate::{
    Activation, CompiledMethodologyBundle, CompiledMethodologyEntry, MethodologyError,
    PredicateOperator, RoutePredicate, SpawnPolicy,
    model::{CATALOG_SCHEMA_V1, COMPACT_ARTIFACT_KIND, CatalogDigestWire, FALLBACK_ARTIFACT_KIND},
};

/// Maximum Catalog source bytes accepted by the pure compiler.
pub const MAX_CATALOG_SOURCE_BYTES: usize = 1024 * 1024;
/// Maximum Catalog entries accepted by schema v1.
pub const MAX_CATALOG_ENTRIES: usize = 128;
/// Maximum repeated metadata values per entry.
pub const MAX_ENTRY_ITEMS: usize = 32;
/// Maximum compact artifact bytes retained by runtime packages.
pub const MAX_COMPACT_BYTES: usize = 256 * 1024;
/// Maximum lazy fallback artifact bytes.
pub const MAX_FALLBACK_BYTES: usize = 1024 * 1024;
/// Production built-in entry count frozen by C0 fixtures.
pub const EXPECTED_BUILTIN_ENTRY_COUNT: usize = 31;

/// Pure content lookup used by build adapters and Catalog startup.
pub trait MethodologyAssetSource {
    /// Returns immutable bytes for one project-relative artifact.
    fn read(&self, path: &ProjectRelativePath) -> Option<&[u8]>;
}

impl MethodologyAssetSource for BTreeMap<ProjectRelativePath, Vec<u8>> {
    fn read(&self, path: &ProjectRelativePath) -> Option<&[u8]> {
        self.get(path).map(Vec::as_slice)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceCatalogWire {
    schema_version: String,
    catalog_version: String,
    entries: Vec<SourceEntryWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceEntryWire {
    skill_id: String,
    series_kind: String,
    /// Skill role serving `series_kind`, absent for non-Series entries.
    ///
    /// The schema requires this for `activation: workflow` and the compiler
    /// enforces the same rule, so `None` here means a capability or deprecated
    /// entry rather than an unvalidated Series.
    #[serde(default)]
    activity: Option<String>,
    variant: String,
    version: String,
    activation: Activation,
    spawn_policy: SpawnPolicy,
    compact_ref: String,
    fallback_ref: RequiredNullableString,
    route_predicates: Vec<SourcePredicateWire>,
    required_inputs: Vec<String>,
    deliverable_kinds: Vec<String>,
    required_gates: Vec<String>,
    tool_dependencies: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RequiredNullableString {
    Value(String),
    Null,
}

impl RequiredNullableString {
    fn into_option(self) -> Option<String> {
        match self {
            Self::Value(value) => Some(value),
            Self::Null => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourcePredicateWire {
    fact: String,
    operator: PredicateOperator,
    value: String,
}

/// Compiles strict Methodology Catalog JSON and content bytes into a canonical bundle.
pub fn compile_catalog(
    source: &[u8],
    assets: &impl MethodologyAssetSource,
) -> Result<CompiledMethodologyBundle, MethodologyError> {
    if source.len() > MAX_CATALOG_SOURCE_BYTES {
        return Err(MethodologyError::SourceTooLarge);
    }
    let source: SourceCatalogWire = serde_json::from_slice(source)?;
    if source.schema_version != CATALOG_SCHEMA_V1 {
        return Err(MethodologyError::UnsupportedSchema(source.schema_version));
    }
    validate_semver("catalogVersion", &source.catalog_version)?;
    if source.entries.is_empty() {
        return Err(MethodologyError::EmptyCatalog);
    }
    if source.entries.len() > MAX_CATALOG_ENTRIES {
        return Err(MethodologyError::CollectionLimit {
            field: "entries",
            limit: MAX_CATALOG_ENTRIES,
        });
    }

    let mut identities = BTreeSet::new();
    let mut entries = Vec::with_capacity(source.entries.len());
    for entry in source.entries {
        let identity = (entry.skill_id.clone(), entry.variant.clone());
        if !identities.insert(identity.clone()) {
            return Err(MethodologyError::DuplicateEntry {
                skill_id: identity.0,
                variant: identity.1,
            });
        }
        entries.push(compile_entry(entry, assets)?);
    }
    entries.sort_by(|left, right| {
        (left.skill_id.as_str(), left.variant.as_str())
            .cmp(&(right.skill_id.as_str(), right.variant.as_str()))
    });
    let catalog_version = BoundedText::new(source.catalog_version).map_err(|error| {
        MethodologyError::InvalidField {
            field: "catalogVersion",
            value: error.to_string(),
        }
    })?;
    let catalog_digest = digest_json(&CatalogDigestWire::new(&catalog_version, &entries))?;
    Ok(CompiledMethodologyBundle {
        catalog_version,
        entries,
        catalog_digest,
    })
}

fn compile_entry(
    source: SourceEntryWire,
    assets: &impl MethodologyAssetSource,
) -> Result<CompiledMethodologyEntry, MethodologyError> {
    validate_semver("version", &source.version)?;
    validate_activation(source.activation, source.spawn_policy)?;
    let pre_route_ra =
        source.activation == Activation::Workflow && source.series_kind == "requirement-analysis";
    if source.activation == Activation::Workflow
        && ((!pre_route_ra && source.route_predicates.is_empty())
            || source.deliverable_kinds.is_empty())
    {
        return Err(MethodologyError::IncompleteWorkflow);
    }

    let compact_path = parse_path(&source.compact_ref)?;
    let fallback_path = source
        .fallback_ref
        .into_option()
        .as_deref()
        .map(parse_path)
        .transpose()?;
    if fallback_path.as_ref() == Some(&compact_path) {
        return Err(MethodologyError::DuplicateArtifactReference);
    }
    let compact_ref = artifact_ref(
        COMPACT_ARTIFACT_KIND,
        compact_path,
        assets,
        MAX_COMPACT_BYTES,
        true,
    )?;
    let fallback_ref = fallback_path
        .map(|path| {
            artifact_ref(
                FALLBACK_ARTIFACT_KIND,
                path,
                assets,
                MAX_FALLBACK_BYTES,
                false,
            )
        })
        .transpose()?;

    let mut entry = CompiledMethodologyEntry {
        skill_id: SkillId::new(source.skill_id).map_err(|error| {
            MethodologyError::InvalidField {
                field: "skillId",
                value: error.to_string(),
            }
        })?,
        series_kind: series_kind_for(&source.series_kind, source.activation)?,
        activity: activity_for(source.activity.as_deref(), source.activation)?,
        variant: MethodologyVariant::new(source.variant).map_err(|error| {
            MethodologyError::InvalidField {
                field: "variant",
                value: error.to_string(),
            }
        })?,
        version: BoundedText::new(source.version).map_err(|error| {
            MethodologyError::InvalidField {
                field: "version",
                value: error.to_string(),
            }
        })?,
        activation: source.activation,
        spawn_policy: source.spawn_policy,
        compact_ref,
        fallback_ref,
        route_predicates: normalize_predicates(source.route_predicates)?,
        required_inputs: normalize_keys("requiredInputs", source.required_inputs)?,
        deliverable_kinds: normalize_keys("deliverableKinds", source.deliverable_kinds)?,
        required_gates: normalize_keys("requiredGates", source.required_gates)?,
        tool_dependencies: normalize_keys("toolDependencies", source.tool_dependencies)?,
        entry_digest: ArtifactDigest::digest([]),
    };
    entry.entry_digest = digest_json(&entry.digest_material())?;
    Ok(entry)
}

pub(crate) fn validate_activation(
    activation: Activation,
    spawn_policy: SpawnPolicy,
) -> Result<(), MethodologyError> {
    let valid = matches!(
        (activation, spawn_policy),
        (Activation::Workflow, SpawnPolicy::PhysicalSeries)
            | (Activation::Capability, SpawnPolicy::Inline)
            | (Activation::Deprecated, SpawnPolicy::Forbidden)
    );
    if valid {
        Ok(())
    } else {
        Err(MethodologyError::InvalidActivationPolicy)
    }
}

fn normalize_predicates(
    source: Vec<SourcePredicateWire>,
) -> Result<Vec<RoutePredicate>, MethodologyError> {
    if source.len() > MAX_ENTRY_ITEMS {
        return Err(MethodologyError::CollectionLimit {
            field: "routePredicates",
            limit: MAX_ENTRY_ITEMS,
        });
    }
    let mut predicates = source
        .into_iter()
        .map(|predicate| {
            validate_portable("routePredicates.fact", &predicate.fact)?;
            let fact = BoundedText::new(predicate.fact).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "routePredicates.fact",
                    value: error.to_string(),
                }
            })?;
            let value = BoundedText::new(predicate.value).map_err(|error| {
                MethodologyError::InvalidField {
                    field: "routePredicates.value",
                    value: error.to_string(),
                }
            })?;
            Ok(RoutePredicate::new(fact, predicate.operator, value))
        })
        .collect::<Result<Vec<_>, MethodologyError>>()?;
    predicates.sort();
    predicates.dedup();
    Ok(predicates)
}

fn normalize_keys(
    field: &'static str,
    source: Vec<String>,
) -> Result<Vec<BoundedText<128>>, MethodologyError> {
    if source.len() > MAX_ENTRY_ITEMS {
        return Err(MethodologyError::CollectionLimit {
            field,
            limit: MAX_ENTRY_ITEMS,
        });
    }
    let mut seen = BTreeSet::new();
    let mut values = Vec::with_capacity(source.len());
    for value in source {
        validate_portable(field, &value)?;
        if !seen.insert(value.clone()) {
            return Err(MethodologyError::DuplicateListValue { field, value });
        }
        values.push(
            BoundedText::new(value).map_err(|error| MethodologyError::InvalidField {
                field,
                value: error.to_string(),
            })?,
        );
    }
    values.sort();
    Ok(values)
}

pub(crate) fn validate_portable(field: &'static str, value: &str) -> Result<(), MethodologyError> {
    let valid_start = value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric);
    let valid_tail = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte));
    if value.len() > 128 || !valid_start || !valid_tail {
        return Err(MethodologyError::InvalidField {
            field,
            value: "non-portable identifier".to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn validate_semver(field: &'static str, value: &str) -> Result<(), MethodologyError> {
    let mut components = value.split('.');
    let valid = (0..3).all(|_| {
        components.next().is_some_and(|component| {
            !component.is_empty()
                && component.bytes().all(|byte| byte.is_ascii_digit())
                && (component == "0" || !component.starts_with('0'))
        })
    }) && components.next().is_none();
    if valid {
        Ok(())
    } else {
        Err(MethodologyError::InvalidField {
            field,
            value: "invalid semantic version".to_owned(),
        })
    }
}

/// Validates `seriesKind`, additionally requiring a frozen main node for Series.
///
/// A well-formed portable id is not enough for `activation: workflow`. The
/// resolver selects slices by exact `series_kind` equality against the main node
/// `FlowRuntime` asks for, so a Series spelled `story-generate` would leave the
/// `story` main node unresolvable while still compiling cleanly.
fn series_kind_for(value: &str, activation: Activation) -> Result<SeriesKind, MethodologyError> {
    let kind = SeriesKind::new(value).map_err(|error| MethodologyError::InvalidField {
        field: "seriesKind",
        value: error.to_string(),
    })?;
    if activation == Activation::Workflow && !MAIN_NODE_SERIES_KINDS.contains(&kind.as_str()) {
        return Err(MethodologyError::InvalidField {
            field: "seriesKind",
            value: format!("{value} is not a frozen main node"),
        });
    }
    Ok(kind)
}

/// Validates `activity` against the activation it accompanies.
///
/// Series must name a role so `(seriesKind, activity)` stays unique; non-Series
/// entries must not, because they have no Series to hold a role within.
fn activity_for(
    value: Option<&str>,
    activation: Activation,
) -> Result<Option<SeriesActivity>, MethodologyError> {
    match (value, activation) {
        (Some(raw), Activation::Workflow) => {
            SeriesActivity::from_wire(raw)
                .map(Some)
                .ok_or_else(|| MethodologyError::InvalidField {
                    field: "activity",
                    value: format!("{raw} is not a frozen activity"),
                })
        }
        (None, Activation::Workflow) => Err(MethodologyError::InvalidField {
            field: "activity",
            value: "a Series entry must name its activity".to_owned(),
        }),
        (Some(raw), _) => Err(MethodologyError::InvalidField {
            field: "activity",
            value: format!("a non-Series entry must not carry an activity, found {raw}"),
        }),
        (None, _) => Ok(None),
    }
}

fn parse_path(value: &str) -> Result<ProjectRelativePath, MethodologyError> {
    ProjectRelativePath::new(value)
        .map_err(|error| MethodologyError::InvalidPath(error.to_string()))
}

fn artifact_ref(
    kind: &'static str,
    path: ProjectRelativePath,
    assets: &impl MethodologyAssetSource,
    max_bytes: usize,
    compact: bool,
) -> Result<ArtifactRef, MethodologyError> {
    let bytes = assets.read(&path).ok_or_else(|| {
        if compact {
            MethodologyError::CompactMissing(path.to_string())
        } else {
            MethodologyError::FallbackMissing(path.to_string())
        }
    })?;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(MethodologyError::InvalidArtifactSize {
            path: path.to_string(),
            actual: bytes.len(),
        });
    }
    let kind = ArtifactKind::new(kind).map_err(|error| MethodologyError::InvalidField {
        field: "artifactKind",
        value: error.to_string(),
    })?;
    let byte_length =
        u64::try_from(bytes.len()).map_err(|_| MethodologyError::InvalidArtifactSize {
            path: path.to_string(),
            actual: bytes.len(),
        })?;
    Ok(ArtifactRef::new(
        kind,
        path,
        ArtifactDigest::digest(bytes),
        byte_length,
    ))
}

pub(crate) fn digest_json(
    value: &impl serde::Serialize,
) -> Result<ArtifactDigest, MethodologyError> {
    Ok(ArtifactDigest::digest(serde_json::to_vec(value)?))
}

/// Verifies the frozen production built-in inventory and activation counts.
pub fn verify_builtin_coverage(bundle: &CompiledMethodologyBundle) -> Result<(), MethodologyError> {
    if bundle.entry_count() != EXPECTED_BUILTIN_ENTRY_COUNT {
        return Err(MethodologyError::CoverageMismatch("entry count"));
    }
    let workflow = bundle
        .entries
        .iter()
        .filter(|entry| entry.activation == Activation::Workflow)
        .count();
    let capability = bundle
        .entries
        .iter()
        .filter(|entry| entry.activation == Activation::Capability)
        .count();
    let deprecated = bundle
        .entries
        .iter()
        .filter(|entry| entry.activation == Activation::Deprecated)
        .count();
    if (workflow, capability, deprecated) != (15, 14, 2) {
        return Err(MethodologyError::CoverageMismatch("activation counts"));
    }
    let deprecated_identities = bundle
        .entries
        .iter()
        .filter(|entry| entry.activation == Activation::Deprecated)
        .map(|entry| entry.skill_id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_deprecated =
        BTreeSet::from(["cross-cutting.proposal", "phase2-coding.coding-report"]);
    if deprecated_identities != expected_deprecated {
        return Err(MethodologyError::CoverageMismatch("deprecated identities"));
    }
    Ok(())
}
