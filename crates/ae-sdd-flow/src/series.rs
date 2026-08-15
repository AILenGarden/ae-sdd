use std::{error::Error, fmt};

use ae_sdd_contracts::{
    RequirementAnalysisSeriesInput, SchemaVersion,
    series::{RouteDisposition, SeriesInput, SeriesPlanDecision, SeriesReceiptStatus},
};
use ae_sdd_domain::DecisionDigest;
use serde::Deserialize;

/// Pure deterministic planner for physical Series actions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SeriesPlanner;

impl SeriesPlanner {
    /// Selects the next pre-route Requirement Analysis action without route input.
    pub fn next_requirement_analysis(
        input: &RequirementAnalysisSeriesInput,
    ) -> Result<SeriesPlanDecision, SeriesPlannerError> {
        let plan = input.candidate_plan();
        let Some(receipt) = input.existing_receipt() else {
            return Ok(SeriesPlanDecision::RunSeries {
                schema_version: SchemaVersion::V1,
                idempotency_key: input.idempotency_key().clone(),
                plan: Box::new(plan.clone()),
            });
        };
        match receipt.status() {
            SeriesReceiptStatus::Planned => Ok(SeriesPlanDecision::RunSeries {
                schema_version: SchemaVersion::V1,
                idempotency_key: input.idempotency_key().clone(),
                plan: Box::new(plan.clone()),
            }),
            SeriesReceiptStatus::Running => Ok(SeriesPlanDecision::AwaitSeries {
                schema_version: SchemaVersion::V1,
                idempotency_key: input.idempotency_key().clone(),
                series_id: plan.series_id().clone(),
            }),
            SeriesReceiptStatus::ResultStaged if receipt.is_collectable() => {
                Ok(SeriesPlanDecision::CollectSeries {
                    schema_version: SchemaVersion::V1,
                    idempotency_key: input.idempotency_key().clone(),
                    series_id: plan.series_id().clone(),
                })
            }
            SeriesReceiptStatus::ResultStaged => Ok(SeriesPlanDecision::AwaitSeries {
                schema_version: SchemaVersion::V1,
                idempotency_key: input.idempotency_key().clone(),
                series_id: plan.series_id().clone(),
            }),
            SeriesReceiptStatus::Collected => Ok(SeriesPlanDecision::Complete {
                schema_version: SchemaVersion::V1,
                idempotency_key: input.idempotency_key().clone(),
                projection_digest: requirement_analysis_completion_digest(input)?,
            }),
            SeriesReceiptStatus::Cancelled | SeriesReceiptStatus::Failed => {
                Err(SeriesPlannerError::TerminalReceipt)
            }
        }
    }

    /// Selects the next route-ordered Series action without performing I/O.
    pub fn next(input: &SeriesInput) -> Result<SeriesPlanDecision, SeriesPlannerError> {
        let identity = series_identity(input)?;
        let schema_version = identity.schema_version;
        if identity.existing_receipts.iter().any(|receipt| {
            receipt.source_revision != identity.state_revision
                || receipt.input_fingerprint != identity.input_fingerprint
        }) {
            return Err(SeriesPlannerError::StaleReceipt);
        }
        if identity
            .candidate_plans
            .iter()
            .any(|plan| plan.input_fingerprint != identity.input_fingerprint)
        {
            return Err(SeriesPlannerError::StalePlan);
        }
        for receipt in input.existing_receipts() {
            let Some(plan) = input
                .candidate_plans()
                .iter()
                .find(|plan| plan.series_id() == receipt.series_id())
            else {
                return Err(SeriesPlannerError::OrphanReceipt);
            };
            if receipt.plan_digest() != plan.plan_digest() {
                return Err(SeriesPlannerError::ReceiptPlanConflict);
            }
            if matches!(
                receipt.status(),
                SeriesReceiptStatus::Cancelled | SeriesReceiptStatus::Failed
            ) {
                return Err(SeriesPlannerError::TerminalReceipt);
            }
        }
        if input.route().disposition() != RouteDisposition::Approved {
            return Ok(SeriesPlanDecision::AwaitRouteApproval {
                schema_version,
                idempotency_key: input.idempotency_key().clone(),
                decision_id: input.route().decision_id().clone(),
            });
        }
        if input.route().required_series().is_empty() {
            return Err(SeriesPlannerError::MissingRequiredSeries);
        }
        for required in input.route().required_series() {
            let plan = input
                .candidate_plans()
                .iter()
                .find(|candidate| candidate.series_kind() == required)
                .ok_or(SeriesPlannerError::MissingCandidate)?;
            let Some(receipt) = input
                .existing_receipts()
                .iter()
                .find(|receipt| receipt.series_id() == plan.series_id())
            else {
                return Ok(SeriesPlanDecision::RunSeries {
                    schema_version,
                    idempotency_key: input.idempotency_key().clone(),
                    plan: Box::new(plan.clone()),
                });
            };
            match receipt.status() {
                SeriesReceiptStatus::Planned => {
                    return Ok(SeriesPlanDecision::RunSeries {
                        schema_version,
                        idempotency_key: input.idempotency_key().clone(),
                        plan: Box::new(plan.clone()),
                    });
                }
                SeriesReceiptStatus::Running => {
                    return Ok(SeriesPlanDecision::AwaitSeries {
                        schema_version,
                        idempotency_key: input.idempotency_key().clone(),
                        series_id: plan.series_id().clone(),
                    });
                }
                SeriesReceiptStatus::ResultStaged if receipt.is_collectable() => {
                    return Ok(SeriesPlanDecision::CollectSeries {
                        schema_version,
                        idempotency_key: input.idempotency_key().clone(),
                        series_id: plan.series_id().clone(),
                    });
                }
                SeriesReceiptStatus::ResultStaged => {
                    return Ok(SeriesPlanDecision::AwaitSeries {
                        schema_version,
                        idempotency_key: input.idempotency_key().clone(),
                        series_id: plan.series_id().clone(),
                    });
                }
                SeriesReceiptStatus::Collected => {}
                SeriesReceiptStatus::Cancelled | SeriesReceiptStatus::Failed => {
                    return Err(SeriesPlannerError::TerminalReceipt);
                }
            }
        }
        Ok(SeriesPlanDecision::Complete {
            schema_version,
            idempotency_key: input.idempotency_key().clone(),
            projection_digest: completion_digest(input)?,
        })
    }
}

fn requirement_analysis_completion_digest(
    input: &RequirementAnalysisSeriesInput,
) -> Result<DecisionDigest, SeriesPlannerError> {
    let bytes = serde_json::to_vec(input).map_err(|_| SeriesPlannerError::ContractEncoding)?;
    let mut canonical = Vec::with_capacity(bytes.len() + 32);
    canonical.extend_from_slice(b"ae-sdd-pre-route-ra-complete/v1\0");
    canonical.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    canonical.extend_from_slice(&bytes);
    Ok(DecisionDigest::digest(canonical))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesSchemaWire {
    schema_version: SchemaVersion,
    state_revision: u64,
    input_fingerprint: String,
    existing_receipts: Vec<SeriesReceiptIdentityWire>,
    candidate_plans: Vec<SeriesPlanIdentityWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesReceiptIdentityWire {
    source_revision: u64,
    input_fingerprint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesPlanIdentityWire {
    input_fingerprint: String,
}

fn series_identity(input: &SeriesInput) -> Result<SeriesSchemaWire, SeriesPlannerError> {
    let bytes = serde_json::to_vec(input).map_err(|_| SeriesPlannerError::ContractEncoding)?;
    serde_json::from_slice::<SeriesSchemaWire>(&bytes)
        .map_err(|_| SeriesPlannerError::ContractEncoding)
}

/// Deterministic Series-planning rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesPlannerError {
    /// The frozen Series input could not be encoded.
    ContractEncoding,
    /// An approved route contained no required Series.
    MissingRequiredSeries,
    /// A required Series lacked its frozen candidate plan.
    MissingCandidate,
    /// A receipt reused a Series identity for another plan digest.
    ReceiptPlanConflict,
    /// A receipt named no candidate plan in the frozen input.
    OrphanReceipt,
    /// A receipt was produced from a different authoritative revision or fingerprint.
    StaleReceipt,
    /// A candidate plan was produced from another input fingerprint.
    StalePlan,
    /// A required Series ended cancelled or failed.
    TerminalReceipt,
}

impl fmt::Display for SeriesPlannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ContractEncoding => "SeriesInput wire identity is invalid",
            Self::MissingRequiredSeries => "approved route contains no required Series",
            Self::MissingCandidate => "required Series has no candidate plan",
            Self::ReceiptPlanConflict => "Series receipt plan digest conflicts with its candidate",
            Self::OrphanReceipt => "Series receipt has no matching candidate plan",
            Self::StaleReceipt => "Series receipt is stale for this input revision or fingerprint",
            Self::StalePlan => "Series candidate plan is stale for this input fingerprint",
            Self::TerminalReceipt => "required Series is cancelled or failed",
        })
    }
}

impl Error for SeriesPlannerError {}

fn completion_digest(input: &SeriesInput) -> Result<DecisionDigest, SeriesPlannerError> {
    let encoded =
        crate::canonical::series_input(input).map_err(|_| SeriesPlannerError::ContractEncoding)?;
    let mut bytes = Vec::with_capacity(encoded.len() + 32);
    bytes.extend_from_slice(b"ae-sdd-series-complete/v1\0");
    bytes.extend_from_slice(&encoded);
    Ok(DecisionDigest::digest(bytes))
}
