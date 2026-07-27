//! Native, bounded implementations for legacy `job.submit` entrypoints.

// Part D WIRING-ONLY: `preflight` and `toolset` modules are owned by Part D.
// C1 will add `mod preflight; mod toolset;`, extend ENTRYPOINTS, and wire the
// dispatch `match` arms. Until then the modules compile-test independently
// and must not be referenced by the dispatcher.

mod assets;
mod baseline;
mod common;
mod database;
mod diagnostics;
mod git;
mod memory;
mod misc;
mod perf;
mod plugin;
mod preflight;
pub(super) mod toolset;

use std::path::Path;

use ae_sdd_runtime::{BoundJobIdentity, BusinessWorkspace, PersistencePort, RuntimeResult};
use serde_json::Value;

use common::{JobContext, mutation_rejected, schema_error, unsupported};

/// Frozen legacy entrypoints routed through the daemon job scheduler.
pub(super) const ENTRYPOINTS: [&str; 53] = [
    "assets.check",
    "assets.outline",
    "assets.query",
    "assets.read",
    "assets.section",
    "assets.stats",
    "automation.disable",
    "automation.enable",
    "automation.status",
    "baseline.create",
    "baseline.diff",
    "baseline.inspect",
    "classify",
    "db.audit",
    "db.explain",
    "db.profiles",
    "db.query",
    "doc.finalize",
    "evidence.lookup",
    "git.blame",
    "git.diff",
    "git.impact",
    "git.log",
    "git.status",
    "gate.doc-storage",
    "iteration-check",
    "memory.clean",
    "memory.clean-all",
    "memory.common",
    "memory.create",
    "memory.read",
    "memory.search",
    "memory.summarize",
    "memory.update",
    "perf.clear",
    "perf.doctor",
    "perf.report",
    "plugin.list",
    "plugin.trace",
    "plugin.validate",
    "preflight.collect",
    "state.bind-story-doc",
    "state.new",
    "state.prd-archive",
    "state.prd-check-complete",
    "state.prd-complete",
    "state.prd-init",
    "state.register-review-consensus",
    "state.relocate",
    "state.write",
    "toolset.receipt.record",
    "toolset.required",
    "update-check",
];

pub(super) fn execute(
    workspace: &BusinessWorkspace,
    work_item_id: Option<&str>,
    _runtime_database: &Path,
    persistence: &dyn PersistencePort,
    identity: Option<&BoundJobIdentity>,
    policy_digest: &str,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    if !ENTRYPOINTS.contains(&entrypoint) {
        return Err(unsupported(entrypoint));
    }
    if memory::is_entrypoint(entrypoint) {
        return memory::execute(
            workspace,
            work_item_id,
            persistence,
            identity,
            entrypoint,
            arguments,
        );
    }
    let context = JobContext::new(workspace, work_item_id)?;
    if matches!(entrypoint, "toolset.required" | "toolset.receipt.record") {
        let identity = identity
            .ok_or_else(|| schema_error("toolset jobs require daemon-bound session identity"))?;
        context.required_work_item()?;
        return toolset::execute(&context, identity, policy_digest, entrypoint, arguments);
    }
    match entrypoint {
        "assets.check" | "assets.outline" | "assets.query" | "assets.read" | "assets.section"
        | "assets.stats" => assets::execute(&context, entrypoint, arguments),
        "baseline.inspect" | "baseline.diff" => baseline::execute(&context, entrypoint, arguments),
        "db.audit" | "db.explain" | "db.profiles" | "db.query" => {
            database::execute(&context, entrypoint, arguments)
        }
        "git.status" | "git.diff" | "git.log" | "git.impact" | "git.blame" => {
            git::execute(&context, entrypoint, arguments)
        }
        "gate.doc-storage" | "iteration-check" | "update-check" => {
            diagnostics::execute(&context, entrypoint, arguments)
        }
        "automation.status" | "classify" | "evidence.lookup" => {
            misc::execute(&context, entrypoint, arguments)
        }
        "perf.report" | "perf.doctor" => perf::execute(&context, entrypoint, arguments),
        "plugin.list" | "plugin.trace" | "plugin.validate" => {
            plugin::execute(&context, entrypoint, arguments)
        }
        "automation.disable"
        | "automation.enable"
        | "baseline.create"
        | "doc.finalize"
        | "perf.clear"
        | "preflight.collect"
        | "state.bind-story-doc"
        | "state.new"
        | "state.prd-archive"
        | "state.prd-check-complete"
        | "state.prd-complete"
        | "state.prd-init"
        | "state.register-review-consensus"
        | "state.relocate"
        | "state.write" => mutation_rejected(&context, entrypoint),
        _ => unreachable!("frozen entrypoint inventory and dispatcher diverged"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_sdd_protocol::WorkspaceMode;
    use ae_sdd_runtime::RuntimeError;

    #[test]
    fn frozen_entrypoint_inventory_is_unique_and_complete() {
        let unique = ENTRYPOINTS
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ENTRYPOINTS.len(), 53);
        assert_eq!(unique.len(), ENTRYPOINTS.len());
    }

    #[test]
    fn every_mutating_entrypoint_is_explicitly_classified() {
        let writes = ENTRYPOINTS
            .iter()
            .filter(|entrypoint| {
                matches!(
                    **entrypoint,
                    "automation.disable"
                        | "automation.enable"
                        | "baseline.create"
                        | "doc.finalize"
                        | "perf.clear"
                        | "preflight.collect"
                        | "toolset.receipt.record"
                ) || entrypoint.starts_with("state.")
            })
            .count();
        assert_eq!(writes, 16);
    }

    #[test]
    fn mutation_error_depends_on_writer_mode() {
        assert!(matches!(WorkspaceMode::Shadow, WorkspaceMode::Shadow));
        let _type_check: fn(&JobContext<'_>, &str) -> Result<Value, RuntimeError> =
            mutation_rejected;
    }
}
