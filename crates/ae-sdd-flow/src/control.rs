use std::{error::Error, fmt};

use ae_sdd_contracts::{
    IdempotencyKey, RouteDecisionId, SeriesId, SeriesPlan,
    series::{SeriesInput, SeriesPlanDecision},
};
use ae_sdd_domain::{ArtifactDigest, DecisionDigest};

use crate::{SeriesPlanner, SeriesPlannerError};

/// Explicit content provenance attached to every control-plane action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlProvenance {
    catalog_digest: ArtifactDigest,
    route_digest: DecisionDigest,
    series_digest: DecisionDigest,
}

impl ControlProvenance {
    /// Returns the exact Methodology Catalog snapshot digest.
    pub const fn catalog_digest(self) -> ArtifactDigest {
        self.catalog_digest
    }

    /// Returns the exact Route decision digest.
    pub const fn route_digest(self) -> DecisionDigest {
        self.route_digest
    }

    /// Returns the digest of the complete Series input and selected action.
    pub const fn series_digest(self) -> DecisionDigest {
        self.series_digest
    }
}

/// Independent control-plane action that does not extend legacy [`crate::NextAction`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlAction {
    /// Wait for explicit approval of the selected route.
    AwaitRouteApproval {
        /// Retry-safe planner identity.
        idempotency_key: IdempotencyKey,
        /// Route decision awaiting approval.
        decision_id: RouteDecisionId,
    },
    /// Dispatch one bounded physical Series plan.
    RunSeries {
        /// Retry-safe planner identity.
        idempotency_key: IdempotencyKey,
        /// Frozen physical plan.
        plan: Box<SeriesPlan>,
    },
    /// Await progress from a running Series.
    AwaitSeries {
        /// Retry-safe planner identity.
        idempotency_key: IdempotencyKey,
        /// Running Series identity.
        series_id: SeriesId,
    },
    /// Collect a validated and memory-cleaned Series result.
    CollectSeries {
        /// Retry-safe planner identity.
        idempotency_key: IdempotencyKey,
        /// Collectable Series identity.
        series_id: SeriesId,
    },
    /// All route-required Series are collected.
    Complete {
        /// Retry-safe planner identity.
        idempotency_key: IdempotencyKey,
        /// Digest of the completed Series projection.
        projection_digest: DecisionDigest,
    },
}

/// Deterministic control action plus its explicit three-layer provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlDecision {
    action: ControlAction,
    provenance: ControlProvenance,
    decision_digest: DecisionDigest,
}

impl ControlDecision {
    /// Returns the side-effect-free control action.
    pub const fn action(&self) -> &ControlAction {
        &self.action
    }

    /// Returns the explicit Catalog/Route/Series provenance.
    pub const fn provenance(&self) -> ControlProvenance {
        self.provenance
    }

    /// Returns the canonical digest of provenance and selected action.
    pub const fn decision_digest(&self) -> DecisionDigest {
        self.decision_digest
    }
}

/// Pure bridge from frozen Series input to an independent control action.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControlPlaneRuntime;

impl ControlPlaneRuntime {
    /// Computes the next control action and binds all upstream decision digests.
    pub fn next(
        catalog_digest: ArtifactDigest,
        input: &SeriesInput,
    ) -> Result<ControlDecision, ControlPlaneError> {
        if input
            .candidate_plans()
            .iter()
            .any(|plan| plan.methodology_ref().catalog_digest() != catalog_digest)
        {
            return Err(ControlPlaneError::CatalogDigestMismatch);
        }
        let series_decision = SeriesPlanner::next(input)?;
        let series_digest = digest_series(input, &series_decision)?;
        let provenance = ControlProvenance {
            catalog_digest,
            route_digest: input.route().decision_digest(),
            series_digest,
        };
        let action = map_action(series_decision);
        let decision_digest = digest_control(provenance);
        Ok(ControlDecision {
            action,
            provenance,
            decision_digest,
        })
    }
}

fn map_action(decision: SeriesPlanDecision) -> ControlAction {
    match decision {
        SeriesPlanDecision::AwaitRouteApproval {
            idempotency_key,
            decision_id,
            ..
        } => ControlAction::AwaitRouteApproval {
            idempotency_key,
            decision_id,
        },
        SeriesPlanDecision::RunSeries {
            idempotency_key,
            plan,
            ..
        } => ControlAction::RunSeries {
            idempotency_key,
            plan,
        },
        SeriesPlanDecision::AwaitSeries {
            idempotency_key,
            series_id,
            ..
        } => ControlAction::AwaitSeries {
            idempotency_key,
            series_id,
        },
        SeriesPlanDecision::CollectSeries {
            idempotency_key,
            series_id,
            ..
        } => ControlAction::CollectSeries {
            idempotency_key,
            series_id,
        },
        SeriesPlanDecision::Complete {
            idempotency_key,
            projection_digest,
            ..
        } => ControlAction::Complete {
            idempotency_key,
            projection_digest,
        },
    }
}

fn digest_series(
    input: &SeriesInput,
    decision: &SeriesPlanDecision,
) -> Result<DecisionDigest, ControlPlaneError> {
    let input_bytes =
        crate::canonical::series_input(input).map_err(|_| ControlPlaneError::ContractEncoding)?;
    let decision_bytes = crate::canonical::series_decision(decision)
        .map_err(|_| ControlPlaneError::ContractEncoding)?;
    let mut bytes = Vec::with_capacity(input_bytes.len() + decision_bytes.len() + 40);
    bytes.extend_from_slice(b"ae-sdd-series-decision/v1\0");
    bytes.extend_from_slice(&(input_bytes.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&input_bytes);
    bytes.extend_from_slice(&(decision_bytes.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&decision_bytes);
    Ok(DecisionDigest::digest(bytes))
}

fn digest_control(provenance: ControlProvenance) -> DecisionDigest {
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(b"ae-sdd-control-decision/v1\0");
    bytes.extend_from_slice(provenance.catalog_digest.as_bytes());
    bytes.extend_from_slice(provenance.route_digest.as_bytes());
    bytes.extend_from_slice(provenance.series_digest.as_bytes());
    DecisionDigest::digest(bytes)
}

/// Deterministic control-plane rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlPlaneError {
    /// A candidate plan was built from another Methodology Catalog snapshot.
    CatalogDigestMismatch,
    /// Frozen input or output could not be canonically encoded.
    ContractEncoding,
    /// The pure Series planner rejected its input.
    Series(SeriesPlannerError),
}

impl From<SeriesPlannerError> for ControlPlaneError {
    fn from(value: SeriesPlannerError) -> Self {
        Self::Series(value)
    }
}

impl fmt::Display for ControlPlaneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CatalogDigestMismatch => {
                formatter.write_str("Series plan Methodology Catalog digest mismatch")
            }
            Self::ContractEncoding => {
                formatter.write_str("control-plane frozen contract encoding failed")
            }
            Self::Series(error) => error.fmt(formatter),
        }
    }
}

impl Error for ControlPlaneError {}
