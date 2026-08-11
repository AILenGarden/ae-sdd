use std::{collections::BTreeMap, error::Error, fmt};

use ae_sdd_contracts::{
    ReasonCode, RouteBindingInput, RouteDecisionId, SchemaVersion, SeriesKind, SpecKind, TaskKind,
    series::{
        ImpactFact, ImpactLevel, RouteDecision, RouteDecisionError, RouteDisposition, RouteInput,
    },
};
use ae_sdd_domain::{
    ArtifactDigest, DecisionDigest, DesignRoute, InputFingerprint, WorkItemId, WorkScale,
};
/// Default minimum confidence required for automatic route approval.
pub const DEFAULT_ROUTE_CONFIDENCE_THRESHOLD_BPS: u16 = 5_000;
const MAX_CONFIRMATION_ID_BYTES: usize = 128;
const MAX_CONFIRMATION_ACTOR_BYTES: usize = 256;
const MAX_CONFIRMATION_TIMESTAMP_BYTES: usize = 64;

/// Pure deterministic classifier for frozen typed route input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteEngine {
    minimum_confidence_bps: u16,
}

impl Default for RouteEngine {
    fn default() -> Self {
        Self {
            minimum_confidence_bps: DEFAULT_ROUTE_CONFIDENCE_THRESHOLD_BPS,
        }
    }
}

impl RouteEngine {
    /// Creates a classifier with an explicit confidence threshold.
    pub fn new(minimum_confidence_bps: u16) -> Result<Self, RouteEngineError> {
        if minimum_confidence_bps > 10_000 {
            return Err(RouteEngineError::InvalidConfidenceThreshold);
        }
        Ok(Self {
            minimum_confidence_bps,
        })
    }

    /// Derives a route candidate solely from verified RA evidence.
    ///
    /// This convenience entry preserves the ordinary implementation task kind.
    /// Call [`Self::decide_from_evidence_for_task_kind`] when the upstream intake
    /// froze a different task kind; task kind never changes the RA scale mapping
    /// or the route-binding fingerprint.
    pub fn decide_from_evidence(
        &self,
        binding: &RouteBindingInput,
        work_item_id: WorkItemId,
        schema_version: SchemaVersion,
    ) -> Result<RouteDecision, RouteEngineError> {
        self.decide_from_evidence_for_task_kind(
            binding,
            work_item_id,
            schema_version,
            TaskKind::Implementation,
        )
    }

    /// Derives a route candidate from RA evidence while preserving task kind as
    /// an orthogonal frozen fact.
    pub fn decide_from_evidence_for_task_kind(
        &self,
        binding: &RouteBindingInput,
        work_item_id: WorkItemId,
        schema_version: SchemaVersion,
        task_kind: TaskKind,
    ) -> Result<RouteDecision, RouteEngineError> {
        if schema_version != SchemaVersion::V2 {
            return Err(RouteEngineError::SchemaVersionMismatch);
        }
        if binding.ra_evidence().work_item_id() != &work_item_id {
            return Err(RouteEngineError::EvidenceWorkItemMismatch);
        }
        let scale = binding.ra_evidence().scale();
        let (design_route, required_series, required_spec_kinds) = classify_scale(scale)?;
        let input_fingerprint = binding.fingerprint();
        let decision_digest = digest_evidence_route(
            &work_item_id,
            input_fingerprint,
            task_kind,
            scale,
            design_route,
            &required_series,
            &required_spec_kinds,
        );
        let decision_id = RouteDecisionId::new(format!("route:{decision_digest}"))
            .map_err(|_| RouteEngineError::InvariantViolation)?;
        RouteDecision::new(
            schema_version,
            decision_id,
            work_item_id,
            task_kind,
            scale,
            design_route,
            RouteDisposition::Approved,
            vec![
                ReasonCode::new("route.ra_scale_classified")
                    .map_err(|_| RouteEngineError::InvariantViolation)?,
            ],
            required_series,
            required_spec_kinds,
            input_fingerprint,
            None,
            decision_digest,
        )
        .map_err(RouteEngineError::DecisionContract)
    }

    /// Computes a deterministic route without reading prose, files, or globals.
    pub fn decide(&self, input: &RouteInput) -> Result<RouteDecision, RouteEngineError> {
        let identity = route_identity(input);
        let (scale, design_route, required_series, required_spec_kinds) =
            classify_impacts(input.impact_facts())?;
        let low_confidence = input.classification_confidence_bps() < self.minimum_confidence_bps;
        let high_impact = input
            .impact_facts()
            .iter()
            .any(|fact| fact.level == ImpactLevel::High);
        let facts_conflict = impact_facts_conflict(input.impact_facts());
        let facts_missing = input.impact_facts().is_empty();
        if input.user_approval_ref().is_some_and(|approval| {
            !confirmation_is_bounded_and_canonical(
                &approval.confirmation_id,
                &approval.approved_by,
                &approval.approved_at,
            )
        }) {
            return Err(RouteEngineError::InvalidConfirmation);
        }
        let approval_binding = self.approval_binding(input)?;
        let approval_matches = input.user_approval_ref().is_some_and(|approval| {
            approval.confirmation_id == format!("route:{approval_binding}")
                && !approval.approved_by.trim().is_empty()
                && !approval.approved_at.trim().is_empty()
        });
        let disposition = if low_confidence
            || facts_missing
            || facts_conflict
            || (high_impact && !approval_matches)
        {
            RouteDisposition::AwaitUserApproval
        } else {
            RouteDisposition::Approved
        };
        let reason_codes = vec![
            ReasonCode::new(if low_confidence {
                "route.low_confidence"
            } else if facts_missing {
                "route.impact_facts_required"
            } else if facts_conflict {
                "route.fact_conflict"
            } else if high_impact && !approval_matches {
                "route.high_impact_approval_required"
            } else {
                "route.classified"
            })
            .map_err(|_| RouteEngineError::InvariantViolation)?,
        ];
        let decision_digest = digest_route(
            &identity,
            input.impact_facts(),
            self.minimum_confidence_bps,
            input.task_kind(),
            scale,
            design_route,
            disposition,
            &required_series,
            &required_spec_kinds,
        );
        let decision_id = RouteDecisionId::new(format!("route:{decision_digest}"))
            .map_err(|_| RouteEngineError::InvariantViolation)?;

        RouteDecision::new(
            identity.schema_version,
            decision_id,
            identity.work_item_id,
            input.task_kind(),
            scale,
            design_route,
            disposition,
            reason_codes,
            required_series,
            required_spec_kinds,
            identity.input_fingerprint,
            approval_matches.then_some(approval_binding),
            decision_digest,
        )
        .map_err(RouteEngineError::DecisionContract)
    }

    /// Computes the exact candidate binding required in a user confirmation ID.
    ///
    /// A matching confirmation uses `route:<lowercase sha256>` as its
    /// `confirmationId`. The binding excludes the confirmation itself, so it
    /// cannot approve a different candidate by changing only approval fields.
    pub fn approval_binding(&self, input: &RouteInput) -> Result<ArtifactDigest, RouteEngineError> {
        let identity = route_identity(input);
        let (scale, design_route, required_series, required_spec_kinds) =
            classify_impacts(input.impact_facts())?;
        let digest = digest_route(
            &identity,
            input.impact_facts(),
            self.minimum_confidence_bps,
            input.task_kind(),
            scale,
            design_route,
            RouteDisposition::AwaitUserApproval,
            &required_series,
            &required_spec_kinds,
        );
        Ok(ArtifactDigest::from_array(digest.into_array()))
    }
}

fn impact_facts_conflict(impacts: &[ImpactFact]) -> bool {
    let mut facts = BTreeMap::new();
    for fact in impacts {
        let value = (fact.level, fact.evidence_digest);
        if facts
            .insert(fact.code.as_str(), value)
            .is_some_and(|previous| previous != value)
        {
            return true;
        }
    }
    false
}

struct RouteIdentity {
    schema_version: SchemaVersion,
    work_item_id: WorkItemId,
    input_fingerprint: InputFingerprint,
}

fn route_identity(input: &RouteInput) -> RouteIdentity {
    RouteIdentity {
        schema_version: input.schema_version(),
        work_item_id: input.work_item_id().clone(),
        input_fingerprint: input.input_fingerprint(),
    }
}

fn confirmation_is_bounded_and_canonical(id: &str, actor: &str, approved_at: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_CONFIRMATION_ID_BYTES
        && !id.bytes().any(|byte| byte.is_ascii_control())
        && !actor.trim().is_empty()
        && actor.len() <= MAX_CONFIRMATION_ACTOR_BYTES
        && !actor.bytes().any(|byte| byte.is_ascii_control())
        && approved_at.len() <= MAX_CONFIRMATION_TIMESTAMP_BYTES
        && approved_at
            .parse::<jiff::Timestamp>()
            .is_ok_and(|timestamp| timestamp.to_string() == approved_at)
}

/// The §7.1 four-tier mapping, returned as the scale, design depth, Series to run
/// and Spec documents that must be bound.
///
/// `required_series` and `required_spec_kinds` are returned separately because
/// §5.5 freezes them as two facts. They are *not* interchangeable even where they
/// agree: a Series is work to run, a Spec kind is a document that must exist.
fn classify_impacts(
    impacts: &[ImpactFact],
) -> Result<(WorkScale, DesignRoute, Vec<SeriesKind>, Vec<SpecKind>), RouteEngineError> {
    let highest = impacts
        .iter()
        .map(|fact| fact.level)
        .reduce(ImpactLevel::max);
    let (scale, route, names, spec_kinds): (_, _, &[&str], &[SpecKind]) = match highest {
        // §7.1 line 342 spells the micro route `RA -> executionPlan -> Coding`, and
        // its persisted-artifact column states 不要求独立 CodingPlan Markdown: the
        // approved `executionPlan` in daemon state *is* the plan. Delegating a
        // CodingPlan Series here produced the Spec the design says must not be
        // required, and left micro indistinguishable from small.
        Some(ImpactLevel::Micro) => (WorkScale::Micro, DesignRoute::CodingPlan, &[], &[]),
        // Missing facts keep the frozen low mapping: decide() holds the
        // decision at AwaitUserApproval, so an empty submission never
        // defaults to the micro route.
        None | Some(ImpactLevel::Low) => (
            WorkScale::Small,
            DesignRoute::CodingPlan,
            &["coding-plan"],
            &[SpecKind::CodingPlan],
        ),
        // A route that requires a Story requires its TestCase: the baseline
        // flow is `RA -> DR -> N x (Story -> TestCase -> CodingPlan)`, and the
        // handoff gates the TestCase Series behind `requires("testcase")`
        // reading this field. Omitting it sent medium and large routes straight
        // from Story to CodingPlan with no TestCase ever delegated.
        //
        // §7.1 line 344 ends the medium route at `... -> CodingPlan -> Coding`, so
        // the CodingPlan Series belongs here too. Without it the route stopped at
        // TestCase and no Coding work was ever planned from an approved plan Spec —
        // the same class of gap as the missing TestCase above.
        Some(ImpactLevel::Medium) => (
            WorkScale::Medium,
            DesignRoute::Story,
            &["story", "testcase", "coding-plan"],
            &[SpecKind::Story, SpecKind::TestCase, SpecKind::CodingPlan],
        ),
        // §7.1 line 345: `RA -> DR -> N x (Story -> TestCase -> CodingPlan)`.
        Some(ImpactLevel::High) => (
            WorkScale::Large,
            DesignRoute::Dr,
            &["design-review", "story", "testcase", "coding-plan"],
            &[
                SpecKind::DesignReview,
                SpecKind::Story,
                SpecKind::TestCase,
                SpecKind::CodingPlan,
            ],
        ),
    };
    let series = names
        .iter()
        .map(|name| SeriesKind::new(*name).map_err(|_| RouteEngineError::InvariantViolation))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((scale, route, series, spec_kinds.to_vec()))
}

fn classify_scale(
    scale: WorkScale,
) -> Result<(DesignRoute, Vec<SeriesKind>, Vec<SpecKind>), RouteEngineError> {
    let (route, names, spec_kinds): (_, &[&str], &[SpecKind]) = match scale {
        WorkScale::Micro => (DesignRoute::CodingPlan, &[], &[]),
        WorkScale::Small => (
            DesignRoute::CodingPlan,
            &["coding-plan"],
            &[SpecKind::CodingPlan],
        ),
        WorkScale::Medium => (
            DesignRoute::Story,
            &["story", "testcase", "coding-plan"],
            &[SpecKind::Story, SpecKind::TestCase, SpecKind::CodingPlan],
        ),
        WorkScale::Large => (
            DesignRoute::Dr,
            &["design-review", "story", "testcase", "coding-plan"],
            &[
                SpecKind::DesignReview,
                SpecKind::Story,
                SpecKind::TestCase,
                SpecKind::CodingPlan,
            ],
        ),
    };
    let series = names
        .iter()
        .map(|name| SeriesKind::new(*name).map_err(|_| RouteEngineError::InvariantViolation))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((route, series, spec_kinds.to_vec()))
}

fn digest_evidence_route(
    work_item_id: &WorkItemId,
    input_fingerprint: InputFingerprint,
    task_kind: TaskKind,
    scale: WorkScale,
    route: DesignRoute,
    required_series: &[SeriesKind],
    required_spec_kinds: &[SpecKind],
) -> DecisionDigest {
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(b"ae-sdd-route-candidate/v2\0");
    encode_text(&mut bytes, work_item_id.as_str());
    bytes.extend_from_slice(input_fingerprint.as_bytes());
    encode_text(&mut bytes, task_kind.as_wire());
    bytes.push(scale_tag(scale));
    bytes.push(route_tag(route));
    bytes.extend_from_slice(&(required_series.len() as u64).to_be_bytes());
    for series in required_series {
        encode_text(&mut bytes, series.as_str());
    }
    bytes.extend_from_slice(&(required_spec_kinds.len() as u64).to_be_bytes());
    for spec_kind in required_spec_kinds {
        encode_text(&mut bytes, spec_kind.as_wire());
    }
    DecisionDigest::digest(bytes)
}

#[allow(clippy::too_many_arguments)]
fn digest_route(
    identity: &RouteIdentity,
    impacts: &[ImpactFact],
    threshold: u16,
    task_kind: TaskKind,
    scale: WorkScale,
    route: DesignRoute,
    disposition: RouteDisposition,
    required_series: &[SeriesKind],
    required_spec_kinds: &[SpecKind],
) -> DecisionDigest {
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(b"ae-sdd-route-decision/v1\0");
    encode_text(&mut bytes, identity.work_item_id.as_str());
    bytes.extend_from_slice(identity.input_fingerprint.as_bytes());
    bytes.extend_from_slice(&threshold.to_be_bytes());
    // The task kind is one of §5.5's frozen facts, so it belongs in the digest:
    // two decisions differing only in task kind are different decisions, and the
    // digest is what a user confirmation binds to.
    encode_text(&mut bytes, task_kind.as_wire());
    bytes.push(scale_tag(scale));
    bytes.push(route_tag(route));
    bytes.push(disposition_tag(disposition));
    let mut canonical_impacts: Vec<_> = impacts.iter().collect();
    canonical_impacts.sort_by(|left, right| {
        left.code
            .as_str()
            .cmp(right.code.as_str())
            .then_with(|| impact_tag(left.level).cmp(&impact_tag(right.level)))
            .then_with(|| {
                left.evidence_digest
                    .as_ref()
                    .map(|digest| digest.as_bytes())
                    .cmp(
                        &right
                            .evidence_digest
                            .as_ref()
                            .map(|digest| digest.as_bytes()),
                    )
            })
    });
    canonical_impacts.dedup_by(|left, right| {
        left.code == right.code
            && left.level == right.level
            && left.evidence_digest == right.evidence_digest
    });
    bytes.extend_from_slice(&(canonical_impacts.len() as u64).to_be_bytes());
    for fact in canonical_impacts {
        encode_text(&mut bytes, fact.code.as_str());
        bytes.push(impact_tag(fact.level));
        match fact.evidence_digest {
            Some(digest) => {
                bytes.push(1);
                bytes.extend_from_slice(digest.as_bytes());
            }
            None => bytes.push(0),
        }
    }
    bytes.extend_from_slice(&(required_series.len() as u64).to_be_bytes());
    for series in required_series {
        encode_text(&mut bytes, series.as_str());
    }
    // Digested separately from `required_series` rather than folded in, so a route
    // that runs the same Series but requires a different Spec set cannot collide.
    // That is exactly the micro/small pair: identical design depth, different
    // required Specs.
    bytes.extend_from_slice(&(required_spec_kinds.len() as u64).to_be_bytes());
    for spec_kind in required_spec_kinds {
        encode_text(&mut bytes, spec_kind.as_wire());
    }
    DecisionDigest::digest(bytes)
}

fn encode_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

const fn impact_tag(value: ImpactLevel) -> u8 {
    match value {
        ImpactLevel::Low => 0,
        ImpactLevel::Medium => 1,
        ImpactLevel::High => 2,
        // Micro was added after the existing tags were frozen; it takes the
        // next free tag so the frozen values are never renumbered.
        ImpactLevel::Micro => 3,
    }
}

const fn scale_tag(value: WorkScale) -> u8 {
    match value {
        WorkScale::Large => 0,
        WorkScale::Medium => 1,
        WorkScale::Small => 2,
        WorkScale::Micro => 3,
    }
}

const fn route_tag(value: DesignRoute) -> u8 {
    match value {
        DesignRoute::Dr => 0,
        DesignRoute::Story => 1,
        DesignRoute::CodingPlan => 2,
    }
}

const fn disposition_tag(value: RouteDisposition) -> u8 {
    match value {
        RouteDisposition::AwaitUserApproval => 0,
        RouteDisposition::Approved => 1,
        RouteDisposition::Denied => 2,
        RouteDisposition::Superseded => 3,
    }
}

/// Deterministic route-classification failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteEngineError {
    /// The configured threshold exceeded 10,000 basis points.
    InvalidConfidenceThreshold,
    /// Frozen wire identity could not be decoded.
    ContractEncoding,
    /// A confirmation exceeded its bounds or was not canonical UTC RFC3339.
    InvalidConfirmation,
    /// RA evidence belongs to a different Work Item.
    EvidenceWorkItemMismatch,
    /// RA-derived routing requires the v2 contract.
    SchemaVersionMismatch,
    /// A compile-time-known contract value was unexpectedly invalid.
    InvariantViolation,
    /// The frozen route-decision contract rejected the computed decision.
    DecisionContract(RouteDecisionError),
}

impl fmt::Display for RouteEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfidenceThreshold => {
                formatter.write_str("route confidence threshold exceeds 10,000 basis points")
            }
            Self::ContractEncoding => formatter.write_str("RouteInput wire identity is invalid"),
            Self::InvalidConfirmation => {
                formatter.write_str("route confirmation is unbounded or non-canonical")
            }
            Self::EvidenceWorkItemMismatch => {
                formatter.write_str("RA evidence belongs to a different Work Item")
            }
            Self::SchemaVersionMismatch => {
                formatter.write_str("RA-derived routing requires schema v2")
            }
            Self::InvariantViolation => {
                formatter.write_str("built-in route vocabulary violates its frozen contract")
            }
            Self::DecisionContract(error) => error.fmt(formatter),
        }
    }
}

impl Error for RouteEngineError {}
