# Rust Daemon V-024 Completion Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Complete the Rust daemon Review Batch v2 authority path so project state, journal events, SQLite projections, Review Gates, crash recovery, and real V-024 reviewer lineage form one fail-closed, restart-safe control plane.

**Architecture:** Project state and the committed runtime journal remain authoritative. SQLite Review tables are deterministic, idempotently rebuildable projections derived from committed `review.record` events. Every authoritative Review Gate must join current workspace bytes, project state, SQLite projection, runtime identity/delegation attestations, and final proof before returning PASS.

**Tech Stack:** Rust workspace, SQLite/WAL via `rusqlite`, `ae-sdd-store` journal/CAS, `ae-sdd-runtime` persistence and identity ports, native Gate scheduler, process-level daemon tests.

---

## 0. Handoff state and safety constraints

- Work Item: `PRD-AE-SDD-RUST-DAEMON-001`
- Story: `STORY-AE-SDD-C1-INTEGRATION-001`
- ae-sdd state revision at handoff: `86`
- Phase: `coding`
- Approved `state.executionPlan`: V-001 through V-024, including the user-approved V-021 expansion.
- Gates already confirmed before coding: `G-CODEPLAN-SRC`, `G-14`, `G-08` PASS.
- Do not regenerate Story, CodingPlan, or executionPlan. Resume the approved plan.
- The old lease expires at `2026-07-26T09:20:54Z`. Acquire or renew a current lease before authoritative mutations. Never copy token 44 from `.ae-sdd/c1-operation-request.json`; the last known active fencing token was 45.
- The worktree is intentionally very dirty. Do not use `git reset`, `git checkout --`, `git clean`, or broad formatting/rewrite commands. Preserve unrelated edits.
- The previous implementation agents were interrupted. Treat current edits in `review_authority.rs` and `review.rs` as partially verified until focused checks pass.

### Already implemented before handoff

- `business.rs`: fresh on-disk state snapshot is used for semantic preparation, transition validation, after-image creation, and document targets; same-revision JSON drift fails with `EXTERNAL_STATE_CONFLICT`.
- `business.rs`: `GateEvaluate` is restricted to daemon-verified `AgentRole::Root`.
- Protocol registry: `GateEvaluate` is a write requiring idempotency.
- Debug commit-abort failpoints: `after_prepared` and `after_replace_0`.
- `ProjectMutationStore` recovery runs before commit/replay.
- `persistence.rs`: Review projection write/upsert/rebuild/load APIs exist.
- `review_authority.rs`: `PreparedReviewRecord::projection_write`, `review_projection_write_from_state`, and a new `validate_review_gate_authority(...)` implementation exist in the working tree.
- `bins/ae-sdd-daemon/tests/c1_control_plane_process.rs`: real process crash/restart cases were added.
- A small unverified fix is present in `persistence.rs`: exact projection receipt replay now returns early instead of reapplying a stale event.

### Known unfinished connections

- `business.rs` still discards the typed `PreparedReviewRecord`; it does not write a Review SQLite projection after project commit.
- The idempotent `replay_committed` branch returns before repairing a missing projection.
- Committed `review.record` event payloads do not yet contain enough replay seed to rebuild historical projections safely.
- `gate_source/contracts.rs` and `gate_source/predicate.rs` still accept state-only Review authority and do not call `validate_review_gate_authority`.
- Gate runtime context does not yet carry database, persistence, and boot identity required by the validator.
- The findings-to-remediation projection may use a child `reviewId` with a parent batch, conflicting with migration 0009 foreign keys.

---

## Task 1: Establish a compilable interrupted-change baseline

**Objective:** Determine whether the interrupted Review validator edits compile before adding new behavior.

**Files to inspect:**

- `crates/ae-sdd-contracts/src/review.rs`
- `crates/ae-sdd-integrations/src/review_authority.rs`
- `crates/ae-sdd-integrations/src/persistence.rs`
- `crates/ae-sdd-integrations/tests/review_authority.rs`

**Steps:**

1. Run `git diff --check` on the four files. Expected: no whitespace or conflict-marker errors.
2. Run `cargo check -p ae-sdd-integrations --lib`. Expected: PASS.
3. Run `cargo test -p ae-sdd-integrations --test review_authority`. Expected: all existing tests PASS.
4. If compilation fails, repair only the interrupted validator edits; do not start projection wiring until this baseline is green.

---

## Task 2: Lock projection receipt replay semantics with TDD

**Objective:** Prove exact event replay is a no-op and a conflicting event payload fails closed.

**Files:**

- Modify/Test: `crates/ae-sdd-integrations/src/persistence.rs`

**Steps:**

1. Add a unit test that inserts a committed `review.record` runtime event, applies one `ReviewProjectionWrite`, and applies the identical write again.
2. Run the single test before finalizing the implementation. Expected before the early-return fix: FAIL with a stale/conflicting session or batch projection.
3. Keep `persist_review_projection_receipt(...) -> RuntimeResult<bool>` semantics:
   - `false`: no receipt existed; apply all projection rows and commit the receipt in the same transaction.
   - `true`: an exact receipt existed; return immediately from `apply_review_projection`.
   - different receipt JSON: `EXTERNAL_STATE_CONFLICT`.
4. Add a second test using the same workspace/event sequence with different typed records. Expected: fail closed.
5. Run the two focused tests. Expected: PASS.

---

## Task 3: Preserve typed Review data through semantic preparation

**Objective:** Make the post-commit path retain the already validated typed Review tuple.

**Files:**

- Modify: `crates/ae-sdd-integrations/src/business.rs` around `PreparedSemanticMutation` and the `OperationName::ReviewRecord` preparation branch.
- Test: `crates/ae-sdd-integrations/tests/review_control_plane_e2e.rs`

**Steps:**

1. Add `review_record: Option<review_authority::PreparedReviewRecord>` to `PreparedSemanticMutation`; initialize it to `None` in `plain`.
2. In the `ReviewRecord` branch, preserve `Some(prepared.clone())` while continuing to populate the state-facing `review`, `reviewSession`, and binding fields.
3. Add a failing integration test proving a successful `review.record` must create the corresponding SQLite Review projection.
4. Run the single test. Expected before Task 4: FAIL because the project commit succeeds but the projection is missing.

---

## Task 4: Add event-derived projection commit and replay repair

**Objective:** Ensure no `review.record` response succeeds unless its committed projection exists, and make retry/restart repair deterministic.

**Files:**

- Modify: `crates/ae-sdd-integrations/src/business.rs` in `ProjectBackend::mutate_state`.
- Reuse: `crates/ae-sdd-integrations/src/review_authority.rs`.
- Reuse: `crates/ae-sdd-integrations/src/persistence.rs`.
- Test: `crates/ae-sdd-integrations/tests/review_control_plane_e2e.rs`.

**Required design:**

1. For `review.record`, add a bounded replay seed to the committed event payload. `data` already contains `review`; add `reviewProjection.reviewSession` so the event can reconstruct:
   - typed session,
   - batch,
   - latest attempt,
   - latest batch receipt,
   - optional exit receipt.
2. After `store.commit(...)` succeeds, call:
   - `PreparedReviewRecord::projection_write(workspace_id, work_item_id, committed.event.event_sequence.get())`, then
   - `upsert_review_authority_projection(database, &write)`.
3. If projection persistence fails, return `EXTERNAL_STATE_CONFLICT`; never return a successful operation response after a projection failure. The project mutation is already committed, so the error must explicitly remain retryable through the same idempotency key.
4. Before returning from the `replay_committed` branch, reconstruct the exact projection write from the committed event seed and upsert/rebuild it. Only return `changed=false` after repair succeeds.
5. Before admitting a new `review.record`, scan durable events through `PersistencePort::events_after` in bounded pages, filter exact workspace/work-item `review.record` events, reconstruct writes in event order, and call `rebuild_review_authority_projections`.
6. A historical `review.record` event without a replay seed must fail closed. Do not fabricate historical sessions from the newest state. A latest-event compatibility fallback is acceptable only when event `data` exactly equals current state `review` and the current `reviewSession` joins that same review/batch/attempt.
7. Avoid consuming `semantic` before post-commit projection writing; iterate cloned semantic targets through `semantic.as_ref()`.

**Tests:**

1. Commit creates projection.
2. Delete projection rows, retry the same idempotency key, and verify replay restores them before returning success.
3. Simulate project commit followed by projection failure; a new idempotency key must repair prior history before admitting another attempt.
4. Tamper an event replay seed; rebuild must return `EXTERNAL_STATE_CONFLICT`.
5. Run `cargo test -p ae-sdd-integrations --test review_control_plane_e2e`.

---

## Task 5: Resolve remediation projection parent/child identity

**Objective:** Make findings -> committed remediation -> child review session project without violating migration 0009 foreign keys.

**Files:**

- Inspect: `migrations/0009_review_batch_v2.sql`
- Modify: `crates/ae-sdd-integrations/src/persistence.rs`, especially `persist_review_remediation`.
- Test: `crates/ae-sdd-integrations/tests/review_control_plane_e2e.rs` or `review_authority.rs`.

**Steps:**

1. Add a failing test with a closed findings batch followed by an input-changing remediation and child review session.
2. Confirm the remediation row references the parent review and its findings batch, while the attempt/session may belong to the child review.
3. Prefer fixing the write identity from typed `parentReviewId`; do not alter migration numbering unless the frozen schema truly cannot represent the contract.
4. Verify retry is idempotent and a mismatched parent/batch fails closed.
5. Run the focused remediation test and the full `review_authority` test target.

---

## Task 6: Wire authoritative Review validation into native Gates

**Objective:** Make every Review Gate depend on workspace bytes, state, SQLite projection, reviewer lineage, and final proof—not state JSON alone.

**Files:**

- Modify: `crates/ae-sdd-integrations/src/gate_source/mod.rs`
- Modify: `crates/ae-sdd-integrations/src/gate_source/key.rs`
- Modify: `crates/ae-sdd-integrations/src/gate_source/scanner.rs`
- Modify: `crates/ae-sdd-integrations/src/gate_source/contracts.rs`
- Modify: `crates/ae-sdd-integrations/src/gate_source/predicate.rs`
- Modify: `crates/ae-sdd-integrations/src/business.rs` at all production `AuthoritativeGateRuntime` constructors.
- Test: `crates/ae-sdd-integrations/tests/review_gate_e2e.rs`

**Steps:**

1. Extend the production Gate context with the runtime database path, `Arc<dyn PersistencePort>`, daemon boot ID, and the authenticated `BusinessWorkspace` needed by `validate_review_gate_authority`.
2. Preserve a lightweight constructor for non-production/unit use, but it must fail Review predicates closed when Review authority dependencies are absent.
3. Route these predicates through `validate_review_gate_authority(...)`:
   - `review.findings.recorded`
   - `review.loop.exit_satisfied`
   - `review.independence.valid`
   - `review.depth.valid`
   - `review.automation_consensus.valid_or_exempt` when automation is enabled.
4. Convert validator failure to predicate `false` so the Gate outcome is FAIL, not PASS. Preserve structured diagnostic evidence where the Gate API supports it.
5. Update all production constructors in:
   - `NativeBusinessAdapter::gate_evaluate_one`
   - `ProjectBackend::gate_check`
   - `ProjectBackend::prepare_transition_commit`
6. The deterministic non-Review gates used while preparing final proof may continue using the lightweight constructor to avoid recursive Review validation.

**Tests:**

1. Valid state plus missing SQLite projection => Review Gate FAIL.
2. Projection/state drift => FAIL.
3. Workspace source change after clean receipt => FAIL as stale.
4. Expired/revoked reviewer session or attestation => FAIL.
5. Wrong specialty, duplicate physical session, or author-as-reviewer => FAIL.
6. Valid Tier 2 authority => all required Review Gates PASS.
7. Valid Tier 3 without job/manifest/journal proof => FAIL; complete proof => PASS.
8. Run `cargo test -p ae-sdd-integrations --test review_gate_e2e`.

---

## Task 7: Complete process crash/restart verification

**Objective:** Prove daemon recovery, project authority, projection rebuilding, and idempotent retry across real process death.

**Files:**

- Verify/adjust: `bins/ae-sdd-daemon/tests/c1_control_plane_process.rs`
- Modify only if required: `crates/ae-sdd-integrations/src/business.rs`

**Scenarios:**

1. Abort at `after_prepared`, restart daemon, retry request, assert one committed mutation/event/projection.
2. Abort at `after_replace_0`, restart daemon, recover project state/journal, retry request, assert projection is restored before success.
3. Restart with project commit present but Review projection deleted, replay same idempotency key, assert repair.
4. Ensure no duplicate attempts, findings, contributions, receipts, or event sequences.

Run:

```powershell
cargo test -p ae-sdd-daemon --test c1_control_plane_process -- --nocapture
```

Expected: all process tests PASS without hangs or orphan daemon processes.

---

## Task 8: Focused and workspace quality gates

**Objective:** Establish release-quality evidence before real V-024 execution.

Run in this order:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p ae-sdd-integrations --test review_authority
cargo test -p ae-sdd-integrations --test review_control_plane_e2e
cargo test -p ae-sdd-integrations --test review_gate_e2e
cargo test -p ae-sdd-integrations --test typed_operations_cli_e2e
cargo test -p ae-sdd-integrations --test flow_authority
cargo test -p ae-sdd-protocol --test protocol_contract
cargo test -p ae-sdd-daemon --test c1_control_plane_process -- --nocapture
cargo test --workspace --all-targets --all-features
```

Rules:

- Fix warnings; do not add broad `allow` attributes to silence new dead code.
- Do not run broad auto-formatting until active edits are stable; formatting must not rewrite unrelated user files.
- Record real command output as ae-sdd evidence; do not create TestReport/CodingReport files.

---

## Task 9: Real V-024 authority run and legal completion

**Objective:** Exercise the released daemon with real typed Root -> Series -> Task/Reviewer lineage and complete the Work Item legally.

**Steps:**

1. Build the release daemon binary.
2. Start a fresh daemon boot and register the workspace.
3. Establish typed Root, Series, exactly one Task author, and independent BE/AR/QA Reviewer sessions with physical delegation attestations and exact specialty grants.
4. Execute the required Review Batch v2 attempts through daemon RPC only.
5. Verify SQLite projection rows, committed project journal, state receipt, input/ruleset fingerprints, and final proof all join exactly.
6. Evaluate the three Review Gates through root-authorized `GateEvaluate`; require PASS.
7. Record V-019 through V-024 evidence against the approved verification matrix.
8. Run independent Review and resolve all findings.
9. Transition only through authoritative daemon operations: `coding -> test-running -> code-reviewed -> completed`.
10. Claim completion only after finalized evidence, PASS Review Gates, and review status with no findings.

---

## Final acceptance checklist

- [ ] Project commit cannot return success without its Review projection.
- [ ] Same-key replay repairs a missing projection.
- [ ] New Review attempts cannot bypass an earlier projection failure.
- [ ] Projection rebuild is deterministic from committed events.
- [ ] Missing/tampered projection causes Review Gate FAIL.
- [ ] Workspace byte drift invalidates old clean authority.
- [ ] Reviewer lineage, specialty grants, and attestations are daemon-verified.
- [ ] Tier 3 proof joins job, receipt locator, manifest, journal, state, and fingerprints.
- [ ] Findings/remediation/child session projection respects parent batch foreign keys.
- [ ] Crash/restart process tests PASS.
- [ ] Strict fmt, Clippy, focused tests, and workspace regression PASS.
- [ ] Real V-024 evidence and independent Review are complete before lifecycle completion.

