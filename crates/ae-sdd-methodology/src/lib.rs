#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Pure Methodology bundle compiler, verifier, Catalog, and deterministic resolver.

mod catalog;
mod compiler;
mod error;
mod model;
mod registry;
mod resolver;
mod verifier;

pub use catalog::{MethodologyCatalog, MethodologyOverride, OverrideAuthorization, OverrideScope};
pub use compiler::{
    EXPECTED_BUILTIN_ENTRY_COUNT, MAX_CATALOG_ENTRIES, MAX_CATALOG_SOURCE_BYTES, MAX_COMPACT_BYTES,
    MAX_ENTRY_ITEMS, MAX_FALLBACK_BYTES, MethodologyAssetSource, compile_catalog,
    verify_builtin_coverage,
};
pub use error::MethodologyError;
pub use model::{
    Activation, CompiledMethodologyBundle, CompiledMethodologyEntry, PredicateOperator,
    RoutePredicate, SpawnPolicy,
};
pub use registry::{
    MAX_REGISTRY_CANDIDATES, RegistryCandidate, RegistryCandidateError, RegistryResolution,
    RegistryResolveError, RegistryTrace, RegistryTraceReason, RegistryViolation, RegistryWinner,
    resolve_registry,
};
pub use resolver::{MethodologyResolveError, MethodologyResolveErrorKind};
pub use verifier::verify_bundle;

/// Encodes a compiled bundle as deterministic compact JSON.
pub fn encode_bundle(bundle: &CompiledMethodologyBundle) -> Result<Vec<u8>, MethodologyError> {
    Ok(serde_json::to_vec(&model::BundleWire::from_bundle(bundle))?)
}

/// Decodes strict compiled bundle JSON without performing artifact I/O.
pub fn decode_bundle(bytes: &[u8]) -> Result<CompiledMethodologyBundle, MethodologyError> {
    let bundle = serde_json::from_slice::<model::BundleWire>(bytes)?.into_bundle()?;
    verifier::verify_bundle_metadata(&bundle)?;
    Ok(bundle)
}
