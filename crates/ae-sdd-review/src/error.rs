use ae_sdd_contracts::ControlPlaneErrorCode;

/// Specific identity-independence violation detected by the supervisor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityViolation {
    /// A reviewer shared the author's physical session (self-review).
    SelfReview,
    /// Two reviewers shared the same physical session.
    DuplicatePhysicalSession,
    /// A reviewer operated at root depth.
    RootReviewer,
    /// A reviewer lineage exceeded the maximum delegation depth.
    ExcessiveLineageDepth,
    /// A completed role was not backed by any reviewer identity.
    UnbackedCompletedRole,
}

/// Specific infrastructure fault that invalidates a review session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfraFault {
    /// The caller reported budget exhaustion without a valid terminal exit.
    BudgetExhausted,
    /// The caller reported missing attestation for one or more reviewers.
    MissingAttestation,
}

/// Pure Review-supervisor error returned when a collected review cannot produce a valid exit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewSupervisorError {
    /// Reviewer identity was not independent (root, self-review, duplicate session, bad lineage).
    IdentityIndependenceViolated(IdentityViolation),
    /// Review infrastructure was invalid and cannot produce PASS.
    InvalidInfra(InfraFault),
    /// The supervisor received structurally invalid collected input.
    InvalidCollectedInput(&'static str),
    /// The frozen receipt contract rejected the assembled exit.
    ReceiptRejected(String),
}

impl ReviewSupervisorError {
    /// Maps a supervisor error to a frozen stable error code.
    #[must_use]
    pub const fn error_code(&self) -> ControlPlaneErrorCode {
        match self {
            Self::IdentityIndependenceViolated(_) | Self::InvalidInfra(_) => {
                ControlPlaneErrorCode::ReviewInvalidInfra
            }
            Self::InvalidCollectedInput(_) | Self::ReceiptRejected(_) => {
                ControlPlaneErrorCode::ContractValidationFailed
            }
        }
    }
}

impl std::fmt::Display for ReviewSupervisorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdentityIndependenceViolated(violation) => write!(
                formatter,
                "reviewer identity independence violated: {violation:?}"
            ),
            Self::InvalidInfra(fault) => {
                write!(formatter, "review infrastructure invalid: {fault:?}")
            }
            Self::InvalidCollectedInput(detail) => write!(
                formatter,
                "collected review input is structurally invalid: {detail}"
            ),
            Self::ReceiptRejected(detail) => {
                write!(formatter, "frozen receipt contract rejected exit: {detail}")
            }
        }
    }
}

impl std::error::Error for ReviewSupervisorError {}
