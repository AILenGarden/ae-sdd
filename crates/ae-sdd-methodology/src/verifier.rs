use std::collections::BTreeSet;

use ae_sdd_domain::{ArtifactDigest, ArtifactRef};

use crate::{
    Activation, CompiledMethodologyBundle, CompiledMethodologyEntry, MAX_CATALOG_ENTRIES,
    MAX_COMPACT_BYTES, MAX_ENTRY_ITEMS, MAX_FALLBACK_BYTES, MethodologyAssetSource,
    MethodologyError,
    compiler::{digest_json, validate_activation, validate_portable, validate_semver},
    model::{COMPACT_ARTIFACT_KIND, CatalogDigestWire, FALLBACK_ARTIFACT_KIND},
};

/// Verifies canonical bundle metadata and all compact/fallback artifact contents.
///
/// Build and release adapters call this after providing immutable bytes. Catalog
/// startup uses the metadata verifier plus compact-only checks so it never needs
/// to read lazy fallback bodies.
pub fn verify_bundle(
    bundle: &CompiledMethodologyBundle,
    assets: &impl MethodologyAssetSource,
) -> Result<(), MethodologyError> {
    verify_bundle_metadata(bundle)?;
    for entry in bundle.entries() {
        verify_artifact(entry.compact_ref(), assets)?;
        if let Some(reference) = entry.fallback_ref() {
            verify_artifact(reference, assets)?;
        }
    }
    Ok(())
}

pub(crate) fn verify_bundle_metadata(
    bundle: &CompiledMethodologyBundle,
) -> Result<(), MethodologyError> {
    validate_semver("catalogVersion", bundle.catalog_version.as_str())?;
    if bundle.entries.is_empty() {
        return Err(MethodologyError::EmptyCatalog);
    }
    if bundle.entries.len() > MAX_CATALOG_ENTRIES {
        return Err(MethodologyError::CollectionLimit {
            field: "entries",
            limit: MAX_CATALOG_ENTRIES,
        });
    }
    let mut previous: Option<(&str, &str)> = None;
    let mut identities = BTreeSet::new();
    for entry in &bundle.entries {
        let identity = (entry.skill_id.as_str(), entry.variant.as_str());
        if previous.is_some_and(|value| value >= identity) {
            return Err(MethodologyError::NonCanonicalEntryOrder);
        }
        previous = Some(identity);
        if !identities.insert((identity.0.to_owned(), identity.1.to_owned())) {
            return Err(MethodologyError::DuplicateEntry {
                skill_id: identity.0.to_owned(),
                variant: identity.1.to_owned(),
            });
        }
        verify_entry_metadata(entry)?;
    }
    let expected = digest_json(&CatalogDigestWire::new(
        &bundle.catalog_version,
        &bundle.entries,
    ))?;
    if expected != bundle.catalog_digest {
        return Err(MethodologyError::CatalogDigestMismatch);
    }
    Ok(())
}

pub(crate) fn verify_entry_metadata(
    entry: &CompiledMethodologyEntry,
) -> Result<(), MethodologyError> {
    validate_semver("version", entry.version.as_str())?;
    validate_activation(entry.activation, entry.spawn_policy)?;
    if entry.activation == Activation::Workflow
        && (entry.route_predicates.is_empty() || entry.deliverable_kinds.is_empty())
    {
        return Err(MethodologyError::IncompleteWorkflow);
    }
    if entry.fallback_ref.as_ref().map(ArtifactRef::path) == Some(entry.compact_ref.path()) {
        return Err(MethodologyError::DuplicateArtifactReference);
    }
    verify_artifact_metadata(&entry.compact_ref, COMPACT_ARTIFACT_KIND, MAX_COMPACT_BYTES)?;
    if let Some(reference) = &entry.fallback_ref {
        verify_artifact_metadata(reference, FALLBACK_ARTIFACT_KIND, MAX_FALLBACK_BYTES)?;
    }
    verify_collection("routePredicates", &entry.route_predicates, |predicate| {
        validate_portable("routePredicates.fact", predicate.fact().as_str())
    })?;
    verify_collection("requiredInputs", &entry.required_inputs, |value| {
        validate_portable("requiredInputs", value.as_str())
    })?;
    verify_collection("deliverableKinds", &entry.deliverable_kinds, |value| {
        validate_portable("deliverableKinds", value.as_str())
    })?;
    verify_collection("requiredGates", &entry.required_gates, |value| {
        validate_portable("requiredGates", value.as_str())
    })?;
    verify_collection("toolDependencies", &entry.tool_dependencies, |value| {
        validate_portable("toolDependencies", value.as_str())
    })?;
    let expected = digest_json(&entry.digest_material())?;
    if expected != entry.entry_digest {
        return Err(MethodologyError::EntryDigestMismatch(
            entry.skill_id.to_string(),
        ));
    }
    Ok(())
}

fn verify_artifact_metadata(
    reference: &ArtifactRef,
    expected_kind: &'static str,
    max_bytes: usize,
) -> Result<(), MethodologyError> {
    if reference.kind().as_str() != expected_kind {
        return Err(MethodologyError::InvalidField {
            field: "artifactKind",
            value: reference.kind().to_string(),
        });
    }
    let actual = match usize::try_from(reference.byte_length()) {
        Ok(actual) => actual,
        Err(_) => usize::MAX,
    };
    if actual == 0 || actual > max_bytes {
        return Err(MethodologyError::InvalidArtifactSize {
            path: reference.path().to_string(),
            actual,
        });
    }
    Ok(())
}

fn verify_collection<T: Ord>(
    field: &'static str,
    values: &[T],
    mut validate: impl FnMut(&T) -> Result<(), MethodologyError>,
) -> Result<(), MethodologyError> {
    if values.len() > MAX_ENTRY_ITEMS {
        return Err(MethodologyError::CollectionLimit {
            field,
            limit: MAX_ENTRY_ITEMS,
        });
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MethodologyError::NonCanonicalMetadata(field));
    }
    for value in values {
        validate(value)?;
    }
    Ok(())
}

pub(crate) fn verify_artifact(
    reference: &ArtifactRef,
    assets: &impl MethodologyAssetSource,
) -> Result<(), MethodologyError> {
    let bytes = assets.read(reference.path()).ok_or_else(|| {
        if reference.kind().as_str() == FALLBACK_ARTIFACT_KIND {
            MethodologyError::FallbackMissing(reference.path().to_string())
        } else {
            MethodologyError::CompactMissing(reference.path().to_string())
        }
    })?;
    let length_matches = u64::try_from(bytes.len()) == Ok(reference.byte_length());
    if !length_matches || ArtifactDigest::digest(bytes) != reference.digest() {
        return Err(MethodologyError::ArtifactTampered(
            reference.path().to_string(),
        ));
    }
    Ok(())
}
