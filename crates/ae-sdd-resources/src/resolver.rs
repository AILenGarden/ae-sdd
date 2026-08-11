use std::collections::BTreeSet;

use ae_sdd_contracts::SchemaVersion;
use ae_sdd_domain::{ArtifactDigest, ArtifactKind, ArtifactRef, InventoryGeneration, ProjectKey};
use thiserror::Error;

/// Maximum number of candidates considered for one resource resolution.
pub const MAX_RESOURCE_CANDIDATES: usize = 32;

/// Stable resource operation intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceIntent {
    /// Resolve an artifact for a bounded read.
    Read,
    /// Resolve an artifact for validation.
    Check,
    /// Resolve an artifact for a bounded query.
    Query,
    /// Resolve a target used only to construct a mutation plan.
    WritePlan,
}

impl ResourceIntent {
    fn tag(self) -> &'static [u8] {
        match self {
            Self::Read => b"read",
            Self::Check => b"check",
            Self::Query => b"query",
            Self::WritePlan => b"write-plan",
        }
    }
}

/// Auditable origin of a resource candidate.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceLayer {
    /// Explicit project-owned override declared by authoritative configuration.
    DeclaredOverride,
    /// Current canonical project layout.
    Canonical,
    /// Supported read-only compatibility layout.
    Legacy,
}

impl ResourceLayer {
    const fn tag(self) -> &'static [u8] {
        match self {
            Self::DeclaredOverride => b"declared-override",
            Self::Canonical => b"canonical",
            Self::Legacy => b"legacy",
        }
    }
}

/// One content-addressed candidate presented to the pure resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceCandidate {
    layer: ResourceLayer,
    artifact_ref: ArtifactRef,
}

impl ResourceCandidate {
    /// Constructs a typed candidate.
    pub const fn new(layer: ResourceLayer, artifact_ref: ArtifactRef) -> Self {
        Self {
            layer,
            artifact_ref,
        }
    }

    /// Returns the candidate source layer.
    pub const fn layer(&self) -> ResourceLayer {
        self.layer
    }

    /// Returns the content-addressed candidate reference.
    pub const fn artifact_ref(&self) -> &ArtifactRef {
        &self.artifact_ref
    }
}

/// Validated deterministic resource-resolution input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceResolveRequest {
    schema_version: SchemaVersion,
    project_key: ProjectKey,
    resource_kind: ArtifactKind,
    intent: ResourceIntent,
    candidates: Vec<ResourceCandidate>,
    override_authorized: bool,
    inventory_generation: InventoryGeneration,
}

impl ResourceResolveRequest {
    /// Validates and canonicalizes a resource request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        project_key: ProjectKey,
        resource_kind: ArtifactKind,
        intent: ResourceIntent,
        mut candidates: Vec<ResourceCandidate>,
        override_authorized: bool,
        inventory_generation: InventoryGeneration,
    ) -> Result<Self, ResourceResolveError> {
        if candidates.is_empty() {
            return Err(ResourceResolveError::NoCandidates);
        }
        if candidates.len() > MAX_RESOURCE_CANDIDATES {
            return Err(ResourceResolveError::CandidateLimitExceeded {
                max_candidates: MAX_RESOURCE_CANDIDATES,
            });
        }
        if candidates
            .iter()
            .any(|candidate| candidate.artifact_ref.kind() != &resource_kind)
        {
            return Err(ResourceResolveError::CandidateKindMismatch);
        }
        let override_count = candidates
            .iter()
            .filter(|candidate| candidate.layer == ResourceLayer::DeclaredOverride)
            .count();
        if override_count > 0 && !override_authorized {
            return Err(ResourceResolveError::OverrideNotAuthorized);
        }
        if override_count > 1 {
            return Err(ResourceResolveError::AmbiguousDeclaredOverride);
        }
        candidates.sort_by(|left, right| {
            left.layer.cmp(&right.layer).then_with(|| {
                left.artifact_ref
                    .path()
                    .as_str()
                    .cmp(right.artifact_ref.path().as_str())
            })
        });
        let mut paths = BTreeSet::new();
        if candidates
            .iter()
            .any(|candidate| !paths.insert(candidate.artifact_ref.path().as_str().to_owned()))
        {
            return Err(ResourceResolveError::DuplicateCandidatePath);
        }
        Ok(Self {
            schema_version,
            project_key,
            resource_kind,
            intent,
            candidates,
            override_authorized,
            inventory_generation,
        })
    }

    /// Returns the schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the project identity.
    pub const fn project_key(&self) -> &ProjectKey {
        &self.project_key
    }

    /// Returns the required resource kind.
    pub const fn resource_kind(&self) -> &ArtifactKind {
        &self.resource_kind
    }

    /// Returns the requested operation intent.
    pub const fn intent(&self) -> ResourceIntent {
        self.intent
    }

    /// Returns candidates in canonical priority/path order.
    pub fn candidates(&self) -> &[ResourceCandidate] {
        &self.candidates
    }

    /// Returns whether a declared override was authorized.
    pub const fn override_authorized(&self) -> bool {
        self.override_authorized
    }

    /// Returns the inventory generation used by resolution.
    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }
}

/// Stable reason attached to one resolution-trace row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionDisposition {
    /// Candidate was selected.
    Winner,
    /// Candidate was valid but lost to a higher-priority candidate.
    LowerPriority,
}

impl ResolutionDisposition {
    const fn tag(self) -> &'static [u8] {
        match self {
            Self::Winner => b"winner",
            Self::LowerPriority => b"lower-priority",
        }
    }
}

/// One ordered, content-addressed resolution trace row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionTraceEntry {
    candidate: ResourceCandidate,
    disposition: ResolutionDisposition,
}

impl ResolutionTraceEntry {
    /// Returns the candidate layer.
    pub const fn layer(&self) -> ResourceLayer {
        self.candidate.layer
    }

    /// Returns the candidate reference.
    pub const fn artifact_ref(&self) -> &ArtifactRef {
        &self.candidate.artifact_ref
    }

    /// Returns the stable trace disposition.
    pub const fn disposition(&self) -> ResolutionDisposition {
        self.disposition
    }
}

/// Deterministic resource winner and complete ordered trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedResource {
    winner: ArtifactRef,
    source_layer: ResourceLayer,
    trace: Vec<ResolutionTraceEntry>,
    inventory_generation: InventoryGeneration,
    resolution_digest: ArtifactDigest,
}

impl ResolvedResource {
    /// Returns the selected content-addressed reference.
    pub const fn winner(&self) -> &ArtifactRef {
        &self.winner
    }

    /// Returns the selected source layer.
    pub const fn source_layer(&self) -> ResourceLayer {
        self.source_layer
    }

    /// Returns the complete canonical trace.
    pub fn trace(&self) -> &[ResolutionTraceEntry] {
        &self.trace
    }

    /// Returns the inventory generation used by resolution.
    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }

    /// Returns the digest of the request, winner, and ordered trace.
    pub const fn resolution_digest(&self) -> ArtifactDigest {
        self.resolution_digest
    }
}

/// Pure application port for resource selection.
pub trait ResourcePort {
    /// Port-specific failure type.
    type Error;

    /// Resolves one typed request without filesystem access or mutation.
    fn resolve(&self, request: &ResourceResolveRequest) -> Result<ResolvedResource, Self::Error>;
}

/// Stateless canonical resource resolver.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeterministicResourceResolver;

impl ResourcePort for DeterministicResourceResolver {
    type Error = ResourceResolveError;

    fn resolve(&self, request: &ResourceResolveRequest) -> Result<ResolvedResource, Self::Error> {
        let Some(winner) = request.candidates.first() else {
            return Err(ResourceResolveError::NoCandidates);
        };
        let trace = request
            .candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| ResolutionTraceEntry {
                candidate: candidate.clone(),
                disposition: if index == 0 {
                    ResolutionDisposition::Winner
                } else {
                    ResolutionDisposition::LowerPriority
                },
            })
            .collect::<Vec<_>>();
        let resolution_digest = resolution_digest(request, &trace);
        Ok(ResolvedResource {
            winner: winner.artifact_ref.clone(),
            source_layer: winner.layer,
            trace,
            inventory_generation: request.inventory_generation,
            resolution_digest,
        })
    }
}

fn resolution_digest(
    request: &ResourceResolveRequest,
    trace: &[ResolutionTraceEntry],
) -> ArtifactDigest {
    fn push(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u64).to_be_bytes());
        target.extend_from_slice(value);
    }

    let mut encoded = Vec::new();
    push(&mut encoded, b"ae-sdd/resource-resolution/v1");
    push(
        &mut encoded,
        match request.schema_version {
            SchemaVersion::V1 => b"v1",
            SchemaVersion::V2 => b"v2",
        },
    );
    push(&mut encoded, request.project_key.as_str().as_bytes());
    push(&mut encoded, request.resource_kind.as_str().as_bytes());
    push(&mut encoded, request.intent.tag());
    encoded.push(u8::from(request.override_authorized));
    encoded.extend_from_slice(&request.inventory_generation.get().to_be_bytes());
    encoded.extend_from_slice(&(trace.len() as u64).to_be_bytes());
    for entry in trace {
        push(&mut encoded, entry.layer().tag());
        push(&mut encoded, entry.disposition().tag());
        push(
            &mut encoded,
            entry.artifact_ref().path().as_str().as_bytes(),
        );
        encoded.extend_from_slice(entry.artifact_ref().digest().as_bytes());
        encoded.extend_from_slice(&entry.artifact_ref().byte_length().to_be_bytes());
    }
    ArtifactDigest::digest(encoded)
}

/// Validation failures produced before resolution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceResolveError {
    /// No candidate was supplied.
    #[error("resource resolution requires at least one candidate")]
    NoCandidates,
    /// Candidate count exceeded the bounded contract.
    #[error("resource candidates exceed the {max_candidates}-item limit")]
    CandidateLimitExceeded {
        /// Maximum accepted candidate count.
        max_candidates: usize,
    },
    /// A candidate kind did not match the requested kind.
    #[error("resource candidate kind does not match the requested kind")]
    CandidateKindMismatch,
    /// An override candidate was supplied without declared authority.
    #[error("declared resource override is not authorized")]
    OverrideNotAuthorized,
    /// More than one declared override was supplied.
    #[error("declared resource override is ambiguous")]
    AmbiguousDeclaredOverride,
    /// Two candidates used the same project-relative path.
    #[error("resource candidates contain a duplicate path")]
    DuplicateCandidatePath,
}
