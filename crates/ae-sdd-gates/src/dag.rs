//! Incremental Gate dependency planner.
//!
//! The DAG declared by [`GateRegistry::dependencies`] drives selector-based
//! invalidation: a change to an input selector invalidates the Gates that
//! declare it plus every transitive dependent. Broken declarations (cycles,
//! unknown prerequisites, duplicates) fail closed at startup so a bad
//! declaration can never silently skip a Gate.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{GateDependencySpec, GateInputSelector, GateRegistry};

/// Reason a Gate dependency declaration cannot be used. Every variant is a
/// startup-time fail-closed error; Gates must never be evaluated from a
/// rejected declaration.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GateDagError {
    /// Two declarations name the same Gate.
    #[error("duplicate Gate dependency declaration: {0}")]
    Duplicate(String),
    /// A prerequisite is not part of the declaration set.
    #[error("Gate {gate} depends on undeclared Gate {prerequisite}")]
    UnknownPrerequisite {
        /// Gate that carries the broken declaration.
        gate: String,
        /// Prerequisite missing from the declaration set.
        prerequisite: String,
    },
    /// The declarations form a dependency cycle.
    #[error("Gate dependency cycle involving: {0:?}")]
    Cycle(Vec<String>),
}

/// Incremental planner over a validated Gate dependency DAG.
#[derive(Clone, Debug)]
pub struct GateDag {
    order: Vec<&'static str>,
    specs: BTreeMap<&'static str, GateDependencySpec>,
    dependents: BTreeMap<&'static str, Vec<&'static str>>,
}

impl GateDag {
    /// Validates `specs` and returns the planner. Fails closed on duplicate
    /// declarations, unknown prerequisites and cycles.
    pub fn build(specs: &[GateDependencySpec]) -> Result<Self, GateDagError> {
        let mut by_gate: BTreeMap<&'static str, GateDependencySpec> = BTreeMap::new();
        for spec in specs {
            if by_gate.insert(spec.gate, *spec).is_some() {
                return Err(GateDagError::Duplicate(spec.gate.to_owned()));
            }
        }
        for spec in specs {
            for prerequisite in spec.prerequisites {
                if !by_gate.contains_key(prerequisite) {
                    return Err(GateDagError::UnknownPrerequisite {
                        gate: spec.gate.to_owned(),
                        prerequisite: (*prerequisite).to_owned(),
                    });
                }
            }
        }

        // Stable Kahn topological sort: always pop the lexicographically
        // smallest unblocked Gate so the order is deterministic across runs.
        let mut indegree: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut dependents: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
        for spec in specs {
            indegree.insert(spec.gate, spec.prerequisites.len());
            for prerequisite in spec.prerequisites {
                dependents.entry(*prerequisite).or_default().push(spec.gate);
            }
        }
        let mut ready: BTreeSet<&'static str> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(gate, _)| *gate)
            .collect();
        let mut order = Vec::with_capacity(specs.len());
        while let Some(gate) = ready.pop_first() {
            order.push(gate);
            if let Some(children) = dependents.get(gate) {
                for child in children {
                    let degree = indegree.get_mut(child).expect("dependent was validated");
                    *degree -= 1;
                    if *degree == 0 {
                        ready.insert(*child);
                    }
                }
            }
        }
        if order.len() != specs.len() {
            let remaining: Vec<String> = indegree
                .iter()
                .filter(|(_, degree)| **degree > 0)
                .map(|(gate, _)| (*gate).to_owned())
                .collect();
            return Err(GateDagError::Cycle(remaining));
        }
        for children in dependents.values_mut() {
            children.sort_unstable();
        }
        Ok(Self {
            order,
            specs: by_gate,
            dependents,
        })
    }

    /// Builds the planner from the canonical registry declarations.
    pub fn from_registry() -> Result<Self, GateDagError> {
        Self::build(GateRegistry::dependencies().as_slice())
    }

    /// Stable topological order of every declared Gate.
    pub fn topological_order(&self) -> &[&'static str] {
        &self.order
    }

    /// Gates that must re-evaluate when `changed` selectors changed, returned
    /// in stable topological order. Gates with an empty selector declaration
    /// cannot prove freshness, fail closed and are always included.
    pub fn affected(&self, changed: &[GateInputSelector]) -> Vec<&'static str> {
        let mut invalidated: BTreeSet<&'static str> = BTreeSet::new();
        let mut queue: Vec<&'static str> = Vec::new();
        for spec in self.specs.values() {
            if spec.selectors.is_empty()
                || spec
                    .selectors
                    .iter()
                    .any(|selector| changed.contains(selector))
            {
                invalidated.insert(spec.gate);
                queue.push(spec.gate);
            }
        }
        while let Some(gate) = queue.pop() {
            if let Some(children) = self.dependents.get(gate) {
                for child in children {
                    if invalidated.insert(*child) {
                        queue.push(*child);
                    }
                }
            }
        }
        self.order
            .iter()
            .copied()
            .filter(|gate| invalidated.contains(gate))
            .collect()
    }

    /// Whether `gate` must re-evaluate when `changed` selectors changed.
    /// Unknown Gates fail closed and always re-evaluate.
    pub fn requires_evaluation(&self, gate: &str, changed: &[GateInputSelector]) -> bool {
        if !self.specs.contains_key(gate) {
            return true;
        }
        self.affected(changed).contains(&gate)
    }
}
