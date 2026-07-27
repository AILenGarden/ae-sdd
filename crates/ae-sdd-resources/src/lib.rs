#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Resource-plane implementation boundary for Part C.

mod assets;
mod document;
mod resolver;

pub use assets::{
    AssetsCheckResult, AssetsDocument, AssetsMatch, AssetsPath, AssetsPort, AssetsQueryRequest,
    AssetsQueryResult, AssetsReadRequest, AssetsRequestError, MAX_ASSETS_QUERY_BYTES,
    MAX_ASSETS_QUERY_MATCHES, MAX_ASSETS_READ_BYTES,
};
pub use document::{
    BoundedDocument, DocumentFinalizeRequest, DocumentPlanError, DocumentPlanner, DocumentPort,
    DocumentReadRequest, DocumentRequestError, DocumentSaveRequest, ResolvedDocument,
};
pub use resolver::{
    DeterministicResourceResolver, ResolutionDisposition, ResolutionTraceEntry, ResolvedResource,
    ResourceCandidate, ResourceIntent, ResourceLayer, ResourcePort, ResourceResolveError,
    ResourceResolveRequest,
};

#[cfg(test)]
mod tests;
