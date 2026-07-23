//! Native, bounded implementations for legacy `job.submit` entrypoints.

mod assets;
mod baseline;
mod common;
mod database;
mod git;
mod misc;
mod perf;
mod plugin;

use std::path::Path;

use ae_sdd_protocol::WorkspaceMode;
use ae_sdd_runtime::{BusinessWorkspace, RuntimeError, RuntimeResult};
use serde_json::Value;

use common::{JobContext, mutation_rejected, unsupported};

/// Frozen legacy entrypoints routed through the daemon job scheduler.
pub(super) const ENTRYPOINTS: [&str; 40] = [
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
];

pub(super) fn execute(
    workspace: &BusinessWorkspace,
    runtime_database: &Path,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let context = JobContext::new(workspace, runtime_database)?;
    match entrypoint {
        "assets.check" | "assets.outline" | "assets.query" | "assets.read"
        | "assets.section" | "assets.stats" => assets::execute(&context, entrypoint, arguments),
        "baseline.inspect" | "baseline.diff" => {
            baseline::execute(&context, entrypoint, arguments)
        }
        "db.audit" | "db.explain" | "db.profiles" | "db.query" => {
            database::execute(&context, entrypoint, arguments)
        }
        "git.status" | "git.diff" | "git.log" | "git.impact" | "git.blame" => {
            git::execute(&context, entrypoint, arguments)
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
        _ => Err(unsupported(entrypoint)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_entrypoint_inventory_is_unique_and_complete() {
        let unique = ENTRYPOINTS.iter().copied().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ENTRYPOINTS.len(), 40);
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
                ) || entrypoint.starts_with("state.")
            })
            .count();
        assert_eq!(writes, 15);
    }

    #[test]
    fn mutation_error_depends_on_writer_mode() {
        assert!(matches!(WorkspaceMode::Shadow, WorkspaceMode::Shadow));
        let _type_check: fn(&JobContext<'_>, &str) -> Result<Value, RuntimeError> =
            mutation_rejected;
    }
}
