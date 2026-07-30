//! Pure ExecutionSupervisor progress policy.
//!
//! [`ExecutionSupervisor::decide`] is a pure reducer: it maps the current
//! [`ExecutionSupervisorCheckpointV1`] plus one bounded
//! [`ExecutionToolEventV1`] to an [`ExecutionDecisionV1`]
//! (`Allow`/`Deny`/`Defer`/`RequireProgress`) and a new checkpoint.  It never
//! reads the filesystem, a clock, randomness, a database or any global
//! state, so identical input always produces an identical decision and
//! checkpoint.
//!
//! Progress is produced only by the six machine events of implementation
//! plan §4.5 (see [`ExecutionProgressKindV1`]); repeated reads of the same
//! path/digest/range, repeated failing runs and repeated blockers never
//! count.  Inspection calls (`SourceRead`/`Search`) are accounted in batches
//! of `inspection_calls_per_batch`: a batch closed without progress
//! increments the no-progress counter, and once it reaches
//! `max_no_progress_batches` only patch, focused-test and blocker events
//! remain admissible.  Broad verification is never allowed before the
//! focused GREEN, and a completed slice denies every further event.
//!
//! Between the focused GREEN and evidence binding the checkpoint also tracks
//! the slice refactor loop ([`RefactorCycleV1`]): once a refactor starts it
//! must close with its re-observed focused GREEN before evidence may bind,
//! while a slice without refactoring needs binds evidence directly.

use std::collections::BTreeSet;

use ae_sdd_contracts::execution_runtime::{ExecutionBudgetsV1, ExecutionSliceStatus};
use ae_sdd_domain::{ArtifactDigest, ArtifactRef, ProjectRelativePath};

use crate::error::{ExecutionSupervisorError, ExecutionSupervisorFault};
use crate::policy::{
    ExecutionAllowanceV1, ExecutionDecisionV1, ExecutionOutputDirectiveV1, ExecutionProgressKindV1,
};
use crate::slice::{ExecutionSliceEvent, RefactorCycleV1, transition_slice_status};

/// Outcome of one focused verification run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusedTestOutcomeV1 {
    /// The focused verification failed.
    Fail,
    /// The focused verification passed.
    Pass,
}

/// Supervisor-tracked focused verification state for the active slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusedTestStateV1 {
    /// The focused verification has not run yet.
    Never,
    /// The latest focused run did not pass.
    Failed,
    /// The latest focused run passed.
    Green,
}

/// Bounded observation of one tool call's output.
///
/// Only the byte count, the content digest and an optional full-output
/// artifact locator cross the policy boundary; the output body never does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionToolOutputV1 {
    /// Raw output bytes produced by the tool call.
    pub bytes: u32,
    /// Digest of the full output.
    pub digest: ArtifactDigest,
    /// Locator of the persisted full output, when already available.
    pub locator: Option<ArtifactRef>,
}

/// One bounded tool event offered to the execution supervisor.
///
/// The hook/runtime classifier maps host tool calls onto these variants;
/// anything it cannot classify conservatively becomes
/// [`ExecutionToolEventV1::Other`].  Slice lifecycle changes and blocker
/// reports are offered through the dedicated variants so the supervisor
/// remains the single reducer for the whole execution surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionToolEventV1 {
    /// Reading project source within the declared slice scope.
    SourceRead {
        /// Canonical project-relative source path.
        path: ProjectRelativePath,
        /// Digest of the content that was read.
        content_digest: ArtifactDigest,
        /// Inclusive 1-based first line, when ranged.
        start_line: Option<u32>,
        /// Inclusive 1-based last line, when ranged.
        end_line: Option<u32>,
        /// Bounded output observation.
        output: ExecutionToolOutputV1,
    },
    /// Searching the workspace.
    Search {
        /// Digest of the bounded query identity.
        query_digest: ArtifactDigest,
        /// Bounded output observation.
        output: ExecutionToolOutputV1,
    },
    /// Applying a patch.
    Patch {
        /// Digest of the resulting patched content.
        result_digest: ArtifactDigest,
        /// Bounded output observation.
        output: ExecutionToolOutputV1,
    },
    /// Running the focused verification bound to the slice.
    FocusedTest {
        /// Run outcome.
        outcome: FocusedTestOutcomeV1,
        /// Bounded output observation.
        output: ExecutionToolOutputV1,
    },
    /// Running a broad verification.
    BroadTest {
        /// Bounded output observation.
        output: ExecutionToolOutputV1,
    },
    /// Appending one evidence ledger event.
    Evidence {
        /// Digest of the appended ledger event.
        event_digest: ArtifactDigest,
        /// Bounded output observation.
        output: ExecutionToolOutputV1,
    },
    /// Reporting a blocker that needs an external decision.
    Blocker {
        /// Stable machine blocker code.
        code: Box<str>,
        /// Evidence locator backing the blocker report.
        locator: ArtifactRef,
    },
    /// Offering a machine event to the slice lifecycle reducer.
    Slice(ExecutionSliceEvent),
    /// Any tool call the classifier could not place.
    Other {
        /// Bounded output observation.
        output: ExecutionToolOutputV1,
    },
}

/// Restartable supervisor checkpoint for the active slice.
///
/// Tracks the slice status, the refactor loop, the focused verification
/// state, the frozen budgets and the investigation accounting (open batch
/// plus consecutive no-progress batches) together with the identities of the
/// last accepted progress events.  The per-batch file set is bounded by
/// `max_source_files_per_batch`, so the checkpoint footprint stays bounded
/// by the frozen budgets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSupervisorCheckpointV1 {
    slice_status: ExecutionSliceStatus,
    refactor_cycle: RefactorCycleV1,
    focused_test: FocusedTestStateV1,
    budgets: ExecutionBudgetsV1,
    no_progress_batches: u8,
    batch_calls: u8,
    batch_source_bytes: u64,
    batch_files: BTreeSet<ProjectRelativePath>,
    last_patch_digest: Option<ArtifactDigest>,
    last_evidence_digest: Option<ArtifactDigest>,
    last_blocker: Option<(Box<str>, ArtifactDigest)>,
}

impl ExecutionSupervisorCheckpointV1 {
    /// Starts a fresh checkpoint for a slice: focused state `never`, refactor
    /// loop `idle`, all investigation counters at zero and no recorded
    /// progress identities.
    pub fn new(slice_status: ExecutionSliceStatus, budgets: ExecutionBudgetsV1) -> Self {
        Self {
            slice_status,
            refactor_cycle: RefactorCycleV1::Idle,
            focused_test: FocusedTestStateV1::Never,
            budgets,
            no_progress_batches: 0,
            batch_calls: 0,
            batch_source_bytes: 0,
            batch_files: BTreeSet::new(),
            last_patch_digest: None,
            last_evidence_digest: None,
            last_blocker: None,
        }
    }

    /// Returns the tracked slice lifecycle status.
    pub const fn slice_status(&self) -> ExecutionSliceStatus {
        self.slice_status
    }

    /// Returns the tracked refactor-loop state of the slice.
    pub const fn refactor_cycle(&self) -> RefactorCycleV1 {
        self.refactor_cycle
    }

    /// Returns the tracked focused verification state.
    pub const fn focused_test(&self) -> FocusedTestStateV1 {
        self.focused_test
    }

    /// Returns the frozen budgets the checkpoint enforces.
    pub const fn budgets(&self) -> ExecutionBudgetsV1 {
        self.budgets
    }

    /// Returns the consecutive no-progress batch counter.
    pub const fn no_progress_batches(&self) -> u8 {
        self.no_progress_batches
    }

    /// Returns the inspection calls recorded in the open batch.
    pub const fn batch_calls(&self) -> u8 {
        self.batch_calls
    }

    /// Returns the retained source bytes recorded in the open batch.
    pub const fn batch_source_bytes(&self) -> u64 {
        self.batch_source_bytes
    }

    /// Returns the distinct source files recorded in the open batch.
    pub fn batch_file_count(&self) -> usize {
        self.batch_files.len()
    }

    /// Returns the last patch digest accepted as progress.
    pub const fn last_patch_digest(&self) -> Option<ArtifactDigest> {
        self.last_patch_digest
    }

    /// Returns the last evidence ledger event digest accepted as progress.
    pub const fn last_evidence_digest(&self) -> Option<ArtifactDigest> {
        self.last_evidence_digest
    }

    /// Returns the last blocker code + locator digest accepted as progress.
    pub fn last_blocker(&self) -> Option<(&str, ArtifactDigest)> {
        self.last_blocker
            .as_ref()
            .map(|(code, digest)| (code.as_ref(), *digest))
    }
}

/// Pure execution supervisor: machine adjudication of RED/GREEN cadence,
/// investigation batches, output budgets and broad-test timing.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExecutionSupervisor;

impl ExecutionSupervisor {
    /// Decides one tool event against the current checkpoint.
    ///
    /// Returns the decision and the next checkpoint.  Denied events leave the
    /// checkpoint untouched; a completed slice denies every event and never
    /// mutates.  The function is total and deterministic: it cannot panic,
    /// block or observe anything beyond its two arguments.
    pub fn decide(
        checkpoint: &ExecutionSupervisorCheckpointV1,
        event: &ExecutionToolEventV1,
    ) -> (ExecutionDecisionV1, ExecutionSupervisorCheckpointV1) {
        if checkpoint.slice_status == ExecutionSliceStatus::Completed {
            return Self::reject(checkpoint, false, ExecutionSupervisorFault::SliceCompleted);
        }
        match event {
            ExecutionToolEventV1::SourceRead { path, output, .. } => {
                Self::decide_source_read(checkpoint, path, output)
            }
            ExecutionToolEventV1::Search { output, .. } => {
                if let Some(rejection) = Self::investigation_gate(checkpoint) {
                    return rejection;
                }
                let mut next = checkpoint.clone();
                Self::record_inspection_call(&mut next);
                Self::allow(next, None, Some(output))
            }
            ExecutionToolEventV1::Patch {
                result_digest,
                output,
            } => {
                let mut next = checkpoint.clone();
                let repeated = checkpoint.last_patch_digest == Some(*result_digest);
                let progress = if repeated {
                    None
                } else {
                    next.last_patch_digest = Some(*result_digest);
                    Self::record_progress(&mut next);
                    Some(ExecutionProgressKindV1::NewPatchDigest)
                };
                Self::allow(next, progress, Some(output))
            }
            ExecutionToolEventV1::FocusedTest { outcome, output } => {
                let mut next = checkpoint.clone();
                let progress = match (checkpoint.focused_test, outcome) {
                    (FocusedTestStateV1::Never, _) => {
                        Some(ExecutionProgressKindV1::FirstFocusedRun)
                    }
                    (FocusedTestStateV1::Failed, FocusedTestOutcomeV1::Pass) => {
                        Some(ExecutionProgressKindV1::FocusedTurnedGreen)
                    }
                    _ => None,
                };
                next.focused_test = match outcome {
                    FocusedTestOutcomeV1::Fail => FocusedTestStateV1::Failed,
                    FocusedTestOutcomeV1::Pass => FocusedTestStateV1::Green,
                };
                if progress.is_some() {
                    Self::record_progress(&mut next);
                }
                Self::allow(next, progress, Some(output))
            }
            ExecutionToolEventV1::BroadTest { output } => {
                if checkpoint.focused_test != FocusedTestStateV1::Green {
                    return Self::reject(
                        checkpoint,
                        true,
                        ExecutionSupervisorFault::BroadTestBeforeFocusedGreen,
                    );
                }
                if let Some(rejection) = Self::investigation_gate(checkpoint) {
                    return rejection;
                }
                Self::allow(checkpoint.clone(), None, Some(output))
            }
            ExecutionToolEventV1::Evidence {
                event_digest,
                output,
            } => {
                if let Some(rejection) = Self::investigation_gate(checkpoint) {
                    return rejection;
                }
                let mut next = checkpoint.clone();
                let repeated = checkpoint.last_evidence_digest == Some(*event_digest);
                let progress = if repeated {
                    None
                } else {
                    next.last_evidence_digest = Some(*event_digest);
                    Self::record_progress(&mut next);
                    Some(ExecutionProgressKindV1::NewEvidenceEvent)
                };
                Self::allow(next, progress, Some(output))
            }
            ExecutionToolEventV1::Blocker { code, locator } => {
                let mut next = checkpoint.clone();
                let repeated =
                    checkpoint
                        .last_blocker
                        .as_ref()
                        .is_some_and(|(last_code, last_digest)| {
                            last_code.as_ref() == code.as_ref() && *last_digest == locator.digest()
                        });
                let progress = if repeated {
                    None
                } else {
                    next.last_blocker = Some((code.clone(), locator.digest()));
                    Self::record_progress(&mut next);
                    Some(ExecutionProgressKindV1::NewBlocker)
                };
                Self::allow(next, progress, None)
            }
            ExecutionToolEventV1::Slice(slice_event) => {
                match transition_slice_status(
                    checkpoint.slice_status,
                    checkpoint.refactor_cycle,
                    *slice_event,
                ) {
                    Ok((status, refactor_cycle)) => {
                        let mut next = checkpoint.clone();
                        next.slice_status = status;
                        next.refactor_cycle = refactor_cycle;
                        Self::record_progress(&mut next);
                        Self::allow(next, Some(ExecutionProgressKindV1::SliceAdvanced), None)
                    }
                    Err(_) => Self::reject(
                        checkpoint,
                        false,
                        ExecutionSupervisorFault::IllegalSliceEvent,
                    ),
                }
            }
            ExecutionToolEventV1::Other { output } => {
                if let Some(rejection) = Self::investigation_gate(checkpoint) {
                    return rejection;
                }
                Self::allow(checkpoint.clone(), None, Some(output))
            }
        }
    }

    /// Returns the exhaustion rejection when the consecutive no-progress
    /// batch budget is spent.
    fn investigation_gate(
        checkpoint: &ExecutionSupervisorCheckpointV1,
    ) -> Option<(ExecutionDecisionV1, ExecutionSupervisorCheckpointV1)> {
        if checkpoint.no_progress_batches >= checkpoint.budgets.max_no_progress_batches() {
            return Some(Self::reject(
                checkpoint,
                false,
                ExecutionSupervisorFault::InvestigationExhausted,
            ));
        }
        None
    }

    fn decide_source_read(
        checkpoint: &ExecutionSupervisorCheckpointV1,
        path: &ProjectRelativePath,
        output: &ExecutionToolOutputV1,
    ) -> (ExecutionDecisionV1, ExecutionSupervisorCheckpointV1) {
        if let Some(rejection) = Self::investigation_gate(checkpoint) {
            return rejection;
        }
        let budgets = checkpoint.budgets;
        let retained = u64::from(output.bytes.min(budgets.max_tool_output_bytes()));
        let new_file = !checkpoint.batch_files.contains(path);
        if new_file
            && checkpoint.batch_files.len() >= usize::from(budgets.max_source_files_per_batch())
        {
            return Self::reject(
                checkpoint,
                false,
                ExecutionSupervisorFault::BatchSourceFilesExceeded,
            );
        }
        if checkpoint.batch_source_bytes + retained
            > u64::from(budgets.max_source_read_bytes_per_batch())
        {
            return Self::reject(
                checkpoint,
                false,
                ExecutionSupervisorFault::BatchSourceBytesExceeded,
            );
        }
        let mut next = checkpoint.clone();
        next.batch_files.insert(path.clone());
        next.batch_source_bytes += retained;
        Self::record_inspection_call(&mut next);
        Self::allow(next, None, Some(output))
    }

    /// Records one inspection call and closes the batch — as one more
    /// consecutive no-progress batch — once the call budget is spent.
    fn record_inspection_call(next: &mut ExecutionSupervisorCheckpointV1) {
        next.batch_calls = next.batch_calls.saturating_add(1);
        if next.batch_calls >= next.budgets.inspection_calls_per_batch() {
            next.no_progress_batches = next
                .no_progress_batches
                .saturating_add(1)
                .min(next.budgets.max_no_progress_batches());
            Self::reset_batch(next);
        }
    }

    /// Progress (plan §4.5) resets both the open batch and the consecutive
    /// no-progress batch counter.
    fn record_progress(next: &mut ExecutionSupervisorCheckpointV1) {
        next.no_progress_batches = 0;
        Self::reset_batch(next);
    }

    fn reset_batch(next: &mut ExecutionSupervisorCheckpointV1) {
        next.batch_calls = 0;
        next.batch_source_bytes = 0;
        next.batch_files.clear();
    }

    fn allow(
        next: ExecutionSupervisorCheckpointV1,
        progress: Option<ExecutionProgressKindV1>,
        output: Option<&ExecutionToolOutputV1>,
    ) -> (ExecutionDecisionV1, ExecutionSupervisorCheckpointV1) {
        let directive = output.map(|output| Self::output_directive(output, &next.budgets));
        (
            ExecutionDecisionV1::Allow(ExecutionAllowanceV1 {
                progress,
                output: directive,
            }),
            next,
        )
    }

    fn reject(
        checkpoint: &ExecutionSupervisorCheckpointV1,
        require_progress: bool,
        fault: ExecutionSupervisorFault,
    ) -> (ExecutionDecisionV1, ExecutionSupervisorCheckpointV1) {
        let error = ExecutionSupervisorError::new(fault);
        let decision = if require_progress {
            ExecutionDecisionV1::RequireProgress(error)
        } else {
            ExecutionDecisionV1::Deny(error)
        };
        (decision, checkpoint.clone())
    }

    /// Retained-output rule: one tool call never retains more than
    /// `max_tool_output_bytes`; when truncation engages, the full output is
    /// bound to its digest and (when persisted) artifact locator.
    fn output_directive(
        output: &ExecutionToolOutputV1,
        budgets: &ExecutionBudgetsV1,
    ) -> ExecutionOutputDirectiveV1 {
        let truncated = output.bytes > budgets.max_tool_output_bytes();
        ExecutionOutputDirectiveV1 {
            retained_bytes: output.bytes.min(budgets.max_tool_output_bytes()),
            truncated,
            output_digest: truncated.then_some(output.digest),
            output_locator: if truncated {
                output.locator.clone()
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(bytes: u32) -> ExecutionToolOutputV1 {
        ExecutionToolOutputV1 {
            bytes,
            digest: ArtifactDigest::digest(b"unit-output"),
            locator: None,
        }
    }

    #[test]
    fn denied_events_return_the_checkpoint_untouched() {
        let checkpoint = ExecutionSupervisorCheckpointV1::new(
            ExecutionSliceStatus::Running,
            ExecutionBudgetsV1::default(),
        );
        let (decision, next) = ExecutionSupervisor::decide(
            &checkpoint,
            &ExecutionToolEventV1::BroadTest {
                output: output(1024),
            },
        );
        assert!(matches!(decision, ExecutionDecisionV1::RequireProgress(_)));
        assert_eq!(next, checkpoint);
    }

    #[test]
    fn checkpoint_starts_with_zeroed_investigation_counters() {
        let checkpoint = ExecutionSupervisorCheckpointV1::new(
            ExecutionSliceStatus::Pending,
            ExecutionBudgetsV1::default(),
        );
        assert_eq!(checkpoint.focused_test(), FocusedTestStateV1::Never);
        assert_eq!(checkpoint.refactor_cycle(), RefactorCycleV1::Idle);
        assert_eq!(checkpoint.no_progress_batches(), 0);
        assert_eq!(checkpoint.batch_calls(), 0);
        assert_eq!(checkpoint.batch_source_bytes(), 0);
        assert_eq!(checkpoint.batch_file_count(), 0);
        assert_eq!(checkpoint.last_patch_digest(), None);
        assert_eq!(checkpoint.last_evidence_digest(), None);
        assert_eq!(checkpoint.last_blocker(), None);
    }
}
