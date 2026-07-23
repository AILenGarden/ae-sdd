use crate::{
    ArtifactDigest, ArtifactKind, EvidenceDigest, EvidenceId, ProjectRelativePath, VerificationId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRef {
    kind: ArtifactKind,
    path: ProjectRelativePath,
    digest: ArtifactDigest,
    byte_length: u64,
}

impl ArtifactRef {
    pub const fn new(
        kind: ArtifactKind,
        path: ProjectRelativePath,
        digest: ArtifactDigest,
        byte_length: u64,
    ) -> Self {
        Self {
            kind,
            path,
            digest,
            byte_length,
        }
    }

    pub const fn kind(&self) -> &ArtifactKind {
        &self.kind
    }

    pub const fn path(&self) -> &ProjectRelativePath {
        &self.path
    }

    pub const fn digest(&self) -> ArtifactDigest {
        self.digest
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceRef {
    evidence_id: EvidenceId,
    verification_id: VerificationId,
    path: ProjectRelativePath,
    digest: EvidenceDigest,
    byte_length: u64,
}

impl EvidenceRef {
    pub const fn new(
        evidence_id: EvidenceId,
        verification_id: VerificationId,
        path: ProjectRelativePath,
        digest: EvidenceDigest,
        byte_length: u64,
    ) -> Self {
        Self {
            evidence_id,
            verification_id,
            path,
            digest,
            byte_length,
        }
    }

    pub const fn evidence_id(&self) -> &EvidenceId {
        &self.evidence_id
    }

    pub const fn verification_id(&self) -> &VerificationId {
        &self.verification_id
    }

    pub const fn path(&self) -> &ProjectRelativePath {
        &self.path
    }

    pub const fn digest(&self) -> EvidenceDigest {
        self.digest
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}
