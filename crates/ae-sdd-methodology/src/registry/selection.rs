use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_contracts::{OverrideDisposition, OverrideLayer};
use ae_sdd_domain::ArtifactDigest;

use crate::{OverrideAuthorization, catalog::layer_priority};

use super::{
    digest::registry_decision_digest,
    model::{
        RegistryCandidate, RegistryResolution, RegistryResolveError, RegistryTrace,
        RegistryTraceReason, RegistryViolation, RegistryWinner,
    },
};

/// Maximum candidates accepted by one pure registry resolution.
pub const MAX_REGISTRY_CANDIDATES: usize = 1_024;

/// Resolves all candidate targets using L1 project > L2 global > L3 repository.
pub fn resolve_registry(
    candidates: Vec<RegistryCandidate>,
) -> Result<RegistryResolution, RegistryResolveError> {
    if candidates.len() > MAX_REGISTRY_CANDIDATES {
        let violations = vec![RegistryViolation::CandidateLimit {
            limit: MAX_REGISTRY_CANDIDATES,
            actual: candidates.len(),
        }];
        let trace = Vec::new();
        return Err(RegistryResolveError {
            decision_digest: registry_decision_digest(&[], &trace, &violations),
            violations,
            trace,
        });
    }

    let analysis = analyze_selection(&candidates);
    let mut violations = Vec::new();
    for index in &analysis.order {
        if analysis.unauthorized.contains(index) {
            let candidate = &candidates[*index];
            violations.push(RegistryViolation::Unauthorized {
                layer: candidate.layer,
                name: candidate.name.clone(),
                target: candidate.target.clone(),
            });
        }
    }
    violations.extend(analysis.name_conflicts.iter().map(|index| {
        RegistryViolation::SameLayerNameConflict {
            layer: candidates[*index].layer,
            name: candidates[*index].name.clone(),
        }
    }));
    violations.extend(analysis.target_conflicts.iter().map(|index| {
        RegistryViolation::SameLayerTargetConflict {
            layer: candidates[*index].layer,
            target: candidates[*index].target.clone(),
        }
    }));

    if !violations.is_empty() {
        let trace = analysis
            .order
            .iter()
            .map(|index| {
                let candidate = &candidates[*index];
                let reason = if analysis.unauthorized.contains(index) {
                    RegistryTraceReason::Unauthorized
                } else if analysis.name_conflict_indices.contains(index) {
                    RegistryTraceReason::SameLayerNameConflict
                } else if analysis.target_conflict_indices.contains(index) {
                    RegistryTraceReason::SameLayerTargetConflict
                } else {
                    RegistryTraceReason::ResolutionBlocked
                };
                registry_trace(candidate, OverrideDisposition::Rejected, reason)
            })
            .collect::<Vec<_>>();
        return Err(RegistryResolveError {
            decision_digest: registry_decision_digest(&[], &trace, &violations),
            violations,
            trace,
        });
    }

    let winners = analysis
        .winners
        .values()
        .map(|index| RegistryWinner {
            candidate: candidates[*index].clone(),
        })
        .collect::<Vec<_>>();
    let trace = analysis
        .order
        .iter()
        .map(|index| {
            let candidate = &candidates[*index];
            let selected = analysis
                .winners
                .get(candidate.target.as_str())
                .is_some_and(|winner| winner == index);
            registry_trace(
                candidate,
                if selected {
                    OverrideDisposition::Selected
                } else {
                    OverrideDisposition::Shadowed
                },
                if selected {
                    RegistryTraceReason::Selected
                } else {
                    RegistryTraceReason::HigherPrioritySelected
                },
            )
        })
        .collect::<Vec<_>>();
    Ok(RegistryResolution {
        decision_digest: registry_decision_digest(&winners, &trace, &[]),
        winners,
        trace,
    })
}

fn registry_trace(
    candidate: &RegistryCandidate,
    disposition: OverrideDisposition,
    reason: RegistryTraceReason,
) -> RegistryTrace {
    RegistryTrace {
        layer: candidate.layer,
        name: candidate.name.clone(),
        target: candidate.target.clone(),
        disposition,
        reason,
        source_digest: candidate.source_digest,
        content_digest: candidate.content_digest,
    }
}

pub(crate) trait SelectionCandidateView {
    fn selection_name(&self) -> &str;
    fn selection_target(&self) -> &str;
    fn selection_layer(&self) -> OverrideLayer;
    fn selection_source_digest(&self) -> ArtifactDigest;
    fn selection_content_digest(&self) -> ArtifactDigest;
    fn selection_authorized(&self) -> bool;
}

impl SelectionCandidateView for RegistryCandidate {
    fn selection_name(&self) -> &str {
        self.name.as_str()
    }

    fn selection_target(&self) -> &str {
        self.target.as_str()
    }

    fn selection_layer(&self) -> OverrideLayer {
        self.layer
    }

    fn selection_source_digest(&self) -> ArtifactDigest {
        self.source_digest
    }

    fn selection_content_digest(&self) -> ArtifactDigest {
        self.content_digest
    }

    fn selection_authorized(&self) -> bool {
        self.authorization == OverrideAuthorization::Authorized
    }
}

pub(crate) struct SelectionAnalysis {
    pub(crate) order: Vec<usize>,
    pub(crate) winners: BTreeMap<String, usize>,
    pub(crate) unauthorized: BTreeSet<usize>,
    pub(crate) name_conflicts: Vec<usize>,
    pub(crate) target_conflicts: Vec<usize>,
    pub(crate) name_conflict_indices: BTreeSet<usize>,
    pub(crate) target_conflict_indices: BTreeSet<usize>,
}

pub(crate) fn analyze_selection(candidates: &[impl SelectionCandidateView]) -> SelectionAnalysis {
    let mut order = (0..candidates.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        let left = &candidates[*left];
        let right = &candidates[*right];
        left.selection_target()
            .cmp(right.selection_target())
            .then_with(|| {
                layer_priority(left.selection_layer()).cmp(&layer_priority(right.selection_layer()))
            })
            .then_with(|| left.selection_name().cmp(right.selection_name()))
            .then_with(|| {
                left.selection_source_digest()
                    .cmp(&right.selection_source_digest())
            })
            .then_with(|| {
                left.selection_content_digest()
                    .cmp(&right.selection_content_digest())
            })
            .then_with(|| {
                authorization_order(left.selection_authorized())
                    .cmp(&authorization_order(right.selection_authorized()))
            })
    });

    let unauthorized = order
        .iter()
        .copied()
        .filter(|index| !candidates[*index].selection_authorized())
        .collect();
    let mut names = BTreeMap::<(u8, String), Vec<usize>>::new();
    let mut targets = BTreeMap::<(u8, String), Vec<usize>>::new();
    for index in &order {
        let candidate = &candidates[*index];
        let priority = layer_priority(candidate.selection_layer());
        names
            .entry((priority, candidate.selection_name().to_owned()))
            .or_default()
            .push(*index);
        targets
            .entry((priority, candidate.selection_target().to_owned()))
            .or_default()
            .push(*index);
    }
    let mut name_conflicts = Vec::new();
    let mut name_conflict_indices = BTreeSet::new();
    for (_, indices) in names {
        if indices.len() > 1 {
            name_conflicts.push(indices[0]);
            name_conflict_indices.extend(indices);
        }
    }
    let mut target_conflicts = Vec::new();
    let mut target_conflict_indices = BTreeSet::new();
    for (_, indices) in targets {
        if indices.len() > 1 {
            target_conflicts.push(indices[0]);
            target_conflict_indices.extend(indices);
        }
    }
    let mut name_winners = BTreeMap::<String, usize>::new();
    let mut target_winners = BTreeMap::<String, usize>::new();
    for index in &order {
        let candidate = &candidates[*index];
        insert_highest_layer(
            candidates,
            &mut name_winners,
            candidate.selection_name(),
            *index,
        );
        insert_highest_layer(
            candidates,
            &mut target_winners,
            candidate.selection_target(),
            *index,
        );
    }
    let winners = target_winners
        .into_iter()
        .filter(|(_, index)| {
            name_winners
                .get(candidates[*index].selection_name())
                .is_some_and(|name_winner| name_winner == index)
        })
        .collect();
    SelectionAnalysis {
        order,
        winners,
        unauthorized,
        name_conflicts,
        target_conflicts,
        name_conflict_indices,
        target_conflict_indices,
    }
}

fn insert_highest_layer(
    candidates: &[impl SelectionCandidateView],
    winners: &mut BTreeMap<String, usize>,
    key: &str,
    candidate_index: usize,
) {
    if let Some(current_index) = winners.get_mut(key) {
        let current_priority = layer_priority(candidates[*current_index].selection_layer());
        let candidate_priority = layer_priority(candidates[candidate_index].selection_layer());
        if candidate_priority < current_priority {
            *current_index = candidate_index;
        }
    } else {
        winners.insert(key.to_owned(), candidate_index);
    }
}

pub(super) const fn authorization_order(authorized: bool) -> u8 {
    if authorized { 0 } else { 1 }
}
