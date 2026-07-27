use ae_sdd_contracts::{FileLockSnapshot, LifecycleInput, lifecycle::CompletionMilestoneInput};
use ae_sdd_domain::{DesignRoute, WorkScale};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ConfirmationProjection {
    pub(crate) confirmation_id: String,
    pub(crate) approved_by: String,
    pub(crate) approved_at: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct EvidenceProjection {
    pub(crate) evidence_id: String,
    pub(crate) verification_id: String,
    pub(crate) path: String,
    pub(crate) digest: String,
    pub(crate) byte_length: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct LifecycleProjection {
    pub(crate) scale: WorkScale,
    pub(crate) design_route: DesignRoute,
    pub(crate) confirmations: Vec<ConfirmationProjection>,
    pub(crate) evidence: Vec<EvidenceProjection>,
    pub(crate) file_locks: Vec<FileLockSnapshot>,
    pub(crate) completion: Option<CompletionMilestoneInput>,
}

impl LifecycleProjection {
    pub(crate) fn from_input(input: &LifecycleInput) -> Self {
        let confirmations = input
            .confirmation_refs()
            .iter()
            .map(|reference| ConfirmationProjection {
                confirmation_id: reference.confirmation_id.clone(),
                approved_by: reference.approved_by.clone(),
                approved_at: reference.approved_at.clone(),
            })
            .collect();
        let evidence = input
            .evidence_refs()
            .iter()
            .map(|reference| EvidenceProjection {
                evidence_id: reference.evidence_id().as_str().to_owned(),
                verification_id: reference.verification_id().as_str().to_owned(),
                path: reference.path().as_str().to_owned(),
                digest: reference.digest().to_string(),
                byte_length: reference.byte_length(),
            })
            .collect();
        Self {
            scale: input.scale(),
            design_route: input.design_route(),
            confirmations,
            evidence,
            file_locks: input.file_locks().to_vec(),
            completion: input.completion(),
        }
    }
}
