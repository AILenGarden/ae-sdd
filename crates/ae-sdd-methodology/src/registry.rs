mod digest;
mod model;
mod selection;

pub use model::{
    RegistryCandidate, RegistryCandidateError, RegistryResolution, RegistryResolveError,
    RegistryTrace, RegistryTraceReason, RegistryViolation, RegistryWinner,
};
pub use selection::{MAX_REGISTRY_CANDIDATES, resolve_registry};

pub(crate) use selection::{SelectionCandidateView, analyze_selection};
