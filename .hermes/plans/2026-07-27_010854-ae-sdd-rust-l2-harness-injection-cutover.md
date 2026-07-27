# Rust L2 Harness Injection Cutover Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Restore the Rust-only post-commit distribution chain's managed L2 instruction injection and make `Execution Efficiency and Scope Discipline` part of the bilingual ae-sdd L2 SSOT.

**Architecture:** Keep `source/L2-DISCIPLINE.md` as the only prose authority. Add a pure Rust managed-instruction renderer in `ae-sdd-build`, then have `execute_post_commit` apply each changed host instruction file through the existing typed `Admin` job so containment, atomic replacement, rollback-on-write-failure, receipts, and idempotency remain native. Normal distribution updates anchored files only; it never bootstraps an unanchored file and never calls Python in the released path.

**Tech Stack:** Rust, `ae-sdd-build`, Clap, Serde, SHA-256, existing native job transaction layer, Rust unit/integration tests, Python only as a migration oracle test.

---

## Authority and execution preconditions

This file is a Harness implementation handoff, not an ae-sdd core design document and not `state.executionPlan`. Before Coding:

1. Create or bind Work Item `BUG-AE-SDD-RUST-L2-INJECTION-001`.
2. Complete Requirement Analysis and Story-lite with the acceptance criteria and verification matrix below.
3. Load authoritative `get_constraints("ae-sdd")` and `get_thinking_engine("ae-sdd")`. Those callable authorities were unavailable while this Plan was written; reading `constraints/` is useful context but does not satisfy the pre-coding gate.
4. Run G-CODEPLAN-SRC, G-14, and G-08 and obtain explicit user approval of the compact `state.executionPlan`.
5. Acquire the Work Item lease and inspect scoped diffs before editing. The current worktree is heavily dirty, including `crates/ae-sdd-build/Cargo.toml`, `crates/ae-sdd-build/src/jobs/mod.rs`, `crates/ae-sdd-build/src/jobs/compile.rs`, `crates/ae-sdd-build/src/release.rs`, and `crates/ae-sdd-build/tests/compatibility_routes.rs`; preserve all unrelated user changes.

## Current context

- Legacy `scripts/distribute.py` calls `scripts/l2_inject.py::inject_all` after skill distribution.
- The Rust-only `.githooks/post-commit` now runs `ae-sdd-build harness` and `ae-sdd-build post-commit`, but `crates/ae-sdd-build/src/post_commit.rs` currently performs only compile, verify, and skill-directory distribution.
- `crates/ae-sdd-build/src/jobs/distribute.rs` copies package directories and has no managed instruction-file behavior.
- `source/L2-DISCIPLINE.md` does not contain `Execution Efficiency and Scope Discipline`; the current Codex global `AGENTS.md` contains a shorter version outside the `ae-sdd-l2-ssot` managed block.
- `source/L2-DISCIPLINE.md` still claims it is injected by `scripts/l2_inject.py`, which is no longer valid released Rust authority.
- Harness and Hermes have no L2 global instruction file. Only Codex, Claude, and ZCode are in scope.

## Scope

### In scope

- Add the user-provided efficiency/scope rules to both `SECTION:en` and `SECTION:zh` in `source/L2-DISCIPLINE.md`.
- Port normal anchored replacement to Rust.
- Wire managed instruction targets into the Rust `post-commit` CLI and repository hook.
- Preserve missing-file/no-anchor skip behavior, byte-identical content outside the managed block, deterministic rendering, and no-op behavior when content already matches.
- Report per-host outcomes in JSON and human-readable post-commit output.
- Add native unit, integration, compatibility, and migration-oracle evidence.
- Perform one explicit, reviewed cleanup of the existing standalone Codex efficiency section after the same content exists inside the anchor.

### Out of scope

- Porting legacy automatic/manual bootstrap detection.
- Porting the legacy `--rollback` CLI or three-backup retention policy.
- Redesigning the distributor registry.
- Adding L2 injection for Harness or Hermes.
- Calling Python from the release hook or Rust runtime.
- Editing generated `dist/ae-sdd/` or `.harness/agent.md` by hand.
- General cleanup of unrelated stale Python-era documentation.

## Acceptance criteria

| ID | Acceptance criterion | Verification |
| --- | --- | --- |
| AC-01 | The full efficiency/scope discipline is inside both L2 language sections and no longer maintained as separate Codex-only prose. | Source-structure test plus rendered zh/en assertions. |
| AC-02 | Rust post-commit updates anchored Codex, Claude, and ZCode global instruction files from the compiled package's `L2-DISCIPLINE.md`. | Native integration test with a temporary repository/home. |
| AC-03 | Missing target files and existing files without anchors are reported as skips and are not created or modified. | Unit and integration negative tests. |
| AC-04 | Normal injection changes only the managed anchor range; all bytes before and after it remain identical. | Renderer unit test using mixed surrounding content and original line endings. |
| AC-05 | Malformed, duplicated, or unclosed anchors fail closed without changing the target file. | Renderer negative tests and filesystem digest assertions. |
| AC-06 | Re-running the same commit/content produces no target-file mutation and stable outcome/receipt behavior. | Replay test using metadata and file digest/mtime where reliable. |
| AC-07 | The release hook and native package contain no Python invocation or Python runtime dependency for L2 injection. | Compatibility route test plus release scan. |
| AC-08 | Skill package distribution remains successful independently of managed-instruction skip outcomes; a real managed-file write error is visible and never silently reported as success. | Post-commit integration tests for skip and failure paths. |
| AC-09 | Harness and Hermes remain package-distribution targets only and receive no global-file injection attempt. | CLI/hook target mapping assertion. |
| AC-10 | Existing standalone Codex efficiency prose is removed exactly once through an explicit reviewed migration, leaving one canonical copy inside the managed block. | Before/after scoped diff and anchor-count/content-count checks. |

## Proposed Rust contract

Create `crates/ae-sdd-build/src/managed_instructions.rs` with crate-owned types similar to:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionLanguage {
    Zh,
    En,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedInstructionTarget {
    pub host: String,
    pub language: InstructionLanguage,
    pub target_file: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedInstructionStatus {
    Updated,
    Unchanged,
    MissingTarget,
    MissingAnchor,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedInstructionOutcome {
    pub host: String,
    pub target_file: String,
    pub status: ManagedInstructionStatus,
    pub content_hash: Option<String>,
    pub job: Option<JobExecution>,
}
```

The pure renderer must:

1. Extract exactly one requested `<!-- SECTION:{lang} --> ... <!-- /SECTION:{lang} -->` body.
2. Locate exactly one complete `BEGIN/END ae-sdd-l2-ssot` span.
3. Render a deterministic audit header from Git revision, body hash, and a Rust adapter version; do not include wall-clock time in the content.
4. Preserve the target's existing newline convention and every byte outside the anchor span.
5. Return `MissingAnchor` without proposing a write when no anchor exists.
6. Return an error for partial or duplicate anchors.
7. Return `Unchanged` when the rendered target is byte-identical.

For each changed host, call the existing native `Admin` job with:

```rust
NativeJobRequest {
    entrypoint: "post-commit.managed-instructions".to_owned(),
    actor: "git:post-commit".to_owned(),
    reason: format!("refresh ae-sdd managed instructions for {}", target.host),
    idempotency_key: format!(
        "post-commit-{}-managed-instructions-{}",
        request.commit_id, target.host
    ),
    mode: ExecutionMode::Apply,
    allowed_roots: request.allowed_roots.clone(),
    job: JobInput::Admin(InitInput {
        project_root: target_parent,
        changes: vec![AdminChange {
            relative_path: target_file_name,
            contents: rendered,
            permission: PermissionClass::PrivateFile,
        }],
    }),
}
```

Add `post-commit.managed-instructions` to the native entrypoint allowlist as `NativeJobKind::Admin`; do not add a new public job kind or schema variant for this bounded fix.

## Step-by-step implementation plan

### Task 1: Establish the ae-sdd Work Item and baseline

**Objective:** Make implementation legally executable and capture the current failing behavior.

**Files:**

- Create/update through ae-sdd state/doc APIs: RA and Story-lite for `BUG-AE-SDD-RUST-L2-INJECTION-001`
- Do not write a separate CodingPlan markdown file.

**Steps:**

1. Resolve/create the Work Item and bind the Story-lite.
2. Put AC-01 through AC-10 and their verification commands into the Story matrix.
3. Load authoritative constraints/CodingModel and obtain the writer lease.
4. Run the focused baseline tests:

   ```powershell
   cargo test -p ae-sdd-build post_commit::tests::typed_post_commit_compiles_verifies_distributes_and_replays -- --exact
   cargo test -p ae-sdd-build --test compatibility_routes post_commit_and_harness_docs_use_rust_typed_argv_only -- --exact
   python -m unittest tools.tests.test_l2_inject
   ```

5. Add a failing source assertion showing `Execution Efficiency and Scope Discipline` is not inside `SECTION:en`/`SECTION:zh`.
6. Add a failing Rust integration assertion showing post-commit leaves an anchored Codex fixture unchanged.
7. Obtain user approval of `state.executionPlan` before the first production-code edit.

### Task 2: Move efficiency discipline into the L2 SSOT

**Objective:** Make the detailed rules canonical and host-neutral.

**Files:**

- Modify: `source/L2-DISCIPLINE.md`
- Test: `crates/ae-sdd-build/tests/compatibility_routes.rs`

**Steps:**

1. Change the file header to identify Rust `ae-sdd-build post-commit` as released injection authority and `scripts/l2_inject.py` as migration-oracle/manual legacy tooling only.
2. Insert the user-provided English section inside `SECTION:en`, after `Hard constraints`.
3. Insert a semantically equivalent Chinese section inside `SECTION:zh` at the corresponding location.
4. Keep these rules out of `source/HARNESS.md`; L2 session discipline is the correct authority boundary.
5. Test that both language slices contain the five detailed subsections: Fast resume, shortest verified slice, bounded investigation/output, Agent coordination, and progress control.
6. Run the focused source-structure test and confirm GREEN.

### Task 3: Implement the pure Rust anchored renderer

**Objective:** Port the safety-critical normal injection behavior without filesystem mutation.

**Files:**

- Create: `crates/ae-sdd-build/src/managed_instructions.rs`
- Modify: `crates/ae-sdd-build/src/lib.rs`
- Unit test: adjacent `#[cfg(test)] mod tests` in `managed_instructions.rs`

**RED tests:**

- Selects zh and en bodies exactly.
- Replaces only the anchor span.
- Preserves LF and CRLF outside the span.
- Returns missing-anchor without rendering a change.
- Rejects unclosed, reversed, and duplicated anchors.
- Produces deterministic header/body for the same revision and source.
- Returns unchanged for byte-identical rendered content.

**Implementation:**

- Use bounded UTF-8 reads and SHA-256 already available in `ae-sdd-build`.
- Do not shell out to Git; receive the commit revision from `PostCommitRequest`.
- Do not read distributor registry state inside the renderer.
- Keep parsing/rendering pure so the migration oracle can compare semantics without touching a user home.

**Verification:**

```powershell
cargo test -p ae-sdd-build managed_instructions::tests --lib
```

Expected: all new renderer tests pass.

### Task 4: Apply changed targets through the native Admin transaction

**Objective:** Perform contained atomic writes and expose per-host outcomes.

**Files:**

- Modify: `crates/ae-sdd-build/src/jobs/model.rs`
- Modify: `crates/ae-sdd-build/src/post_commit.rs`
- Modify: `crates/ae-sdd-build/src/lib.rs`
- Test: unit tests in `post_commit.rs`

**Steps:**

1. Add `post-commit.managed-instructions` to `NATIVE_ENTRYPOINTS` as `Admin`.
2. Extend `PostCommitRequest` with `managed_instruction_targets: Vec<ManagedInstructionTarget>`.
3. Extend `PostCommitExecution` with `managed_instructions: Vec<ManagedInstructionOutcome>`.
4. After compile/verify/distribute, read `package_directory/L2-DISCIPLINE.md`, render each explicit target in stable host-name order, and apply changed files through one native Admin transaction per host.
5. Treat missing target, missing anchor, and unchanged content as successful reported outcomes.
6. Treat invalid source markers, malformed anchors, containment violations, and real write failures as `PostCommitError::ManagedInstructions` after skill distribution has completed; the hook must return nonzero and report the failure instead of printing success.
7. Ensure no managed target can escape `allowed_roots` and no symlink target is followed.

**Verification:**

```powershell
cargo test -p ae-sdd-build post_commit::tests --lib
```

Expected: compile/distribute replay remains green; new update/skip/error tests pass.

### Task 5: Add typed CLI flags and hook wiring

**Objective:** Make the released Rust post-commit path provide explicit host instruction targets.

**Files:**

- Modify: `crates/ae-sdd-build/src/main.rs`
- Modify: `.githooks/post-commit`
- Modify: `crates/ae-sdd-build/tests/compatibility_routes.rs`

**CLI additions:**

```rust
#[arg(long)]
codex_instructions: Option<PathBuf>,
#[arg(long)]
claude_instructions: Option<PathBuf>,
#[arg(long)]
zcode_instructions: Option<PathBuf>,
```

Map them explicitly to:

- Codex -> English -> `$USER_HOME/.codex/AGENTS.md`
- Claude -> Chinese -> `$USER_HOME/.claude/CLAUDE.md`
- ZCode -> Chinese -> `$USER_HOME/.zcode/AGENTS.md`

Do not infer global instruction paths from skill target directory strings.

Update `.githooks/post-commit` to pass the three optional files while retaining package targets for Claude, Codex, ZCode, Harness, and Hermes. Keep the hook free of `python`, request-file generation, and shell-composed JSON.

**Verification:**

```powershell
cargo test -p ae-sdd-build --test compatibility_routes post_commit_and_harness_docs_use_rust_typed_argv_only -- --exact
cargo test -p ae-sdd-build --test compatibility_routes build_cli_post_commit_compatibility_release_and_benchmark_paths_are_safe -- --exact
```

Expected: hook assertions include all three instruction flags, exclude Harness/Hermes instruction flags, and still reject Python invocation.

### Task 6: Add end-to-end native filesystem evidence

**Objective:** Prove the real CLI updates only managed regions in a temporary home.

**Files:**

- Create: `crates/ae-sdd-build/tests/managed_instruction_sync.rs`

**Test matrix:**

1. Codex anchored English file updates.
2. Claude/ZCode anchored Chinese files update.
3. Missing Harness/Hermes instruction targets are never attempted.
4. Missing file skips.
5. Unanchored file skips and remains byte-identical.
6. Malformed anchor fails and remains byte-identical.
7. Content outside anchors remains byte-identical.
8. Second execution is idempotent and does not alter the file.
9. Skill package target still contains `SKILL.md` when all managed targets skip.
10. A managed target outside allowed roots is rejected.

**Verification:**

```powershell
cargo test -p ae-sdd-build --test managed_instruction_sync
```

Expected: all scenarios pass without accessing the production user profile.

### Task 7: Preserve migration parity without making Python runtime authority

**Objective:** Show the Rust renderer preserves the legacy normal-injection contract.

**Files:**

- Modify: `crates/ae-sdd-build/tests/migration_oracle.rs`
- Read-only oracle: `scripts/l2_inject.py`
- Read-only legacy tests: `tools/tests/test_l2_inject.py`

**Steps:**

1. Add migration-oracle fixtures for anchored update, no-anchor skip, language selection, and outside-region preservation.
2. Normalize audit-header fields before comparison because Rust intentionally removes wall-clock timestamps.
3. Compare semantic body and untouched outside bytes, not Python backup filenames.
4. Assert the released hook/package dependency scan contains no Python L2 entrypoint.

**Verification:**

```powershell
cargo test -p ae-sdd-build --test migration_oracle managed_instruction -- --exact
python -m unittest tools.tests.test_l2_inject
```

Expected: Rust/Python preserved semantics agree; Python remains test-only.

### Task 8: Update current-truth architecture and dependency graph

**Objective:** Prevent the same migration omission from recurring.

**Files:**

- Modify: `source/docs/ae-sdd-implementation-architecture.md`
- Modify: `source/standards/update-graph.json`
- Modify if required by update-check: `RELEASING.md`
- Do not create or modify `source/CHANGELOG/*`.

**Required current truth:**

- Rust post-commit stages are harness generation, compile, verify, skill distribution, then managed L2 instruction sync.
- `source/L2-DISCIPLINE.md` is prose SSOT.
- `ae-sdd-build` is released executor.
- `scripts/l2_inject.py` is migration/manual legacy tooling and test oracle only.
- A change to L2 source, post-commit target mapping, or managed renderer must trigger the relevant native tests and release scan.

**Verification:**

```powershell
python 'C:\Users\EDY\.codex\skills\ae-sdd\tools\bin\ae-sdd' update-check --affected source/L2-DISCIPLINE.md crates/ae-sdd-build/src/managed_instructions.rs crates/ae-sdd-build/src/post_commit.rs crates/ae-sdd-build/src/main.rs .githooks/post-commit
```

Expected: no missing mandatory synchronization edge.

### Task 9: Perform the explicit one-time installed-file migration

**Objective:** Remove the current duplicate Codex-only efficiency section after it becomes managed content.

**Targets outside the repository:**

- `C:\Users\EDY\.codex\AGENTS.md`
- Workspace mirror named by its Sync Discipline, if still authoritative at execution time.

**Safety sequence:**

1. Require explicit user approval in `state.executionPlan` for these external-file writes.
2. Back up each exact target.
3. Run the new Rust sync in dry/test context or apply through the approved post-commit flow.
4. Verify the full detailed efficiency section exists inside the managed anchor.
5. Remove only the standalone `## Execution Efficiency and Scope Discipline` section outside the anchor; preserve `Skill Source` and `Sync Discipline` byte-for-byte.
6. Confirm exactly one efficiency heading remains and it is between BEGIN/END markers.
7. Keep this as an operational migration step; do not teach normal injection to delete arbitrary content outside anchors.

### Task 10: Final verification and Review

**Objective:** Produce the required focused and regression evidence without claiming completion early.

**Commands:**

```powershell
cargo fmt --all -- --check
cargo clippy -p ae-sdd-build --all-targets --all-features -- -D warnings
cargo test -p ae-sdd-build --all-features
cargo test --workspace --all-features
python -m unittest tools.tests.test_l2_inject
```

Then run the Story verification matrix through ae-sdd evidence tooling, execute required gates/update-check, and request independent Review. Completion requires finalized evidence plus `state.review.status/findings` with no blocker or major finding.

## Likely files changed

| Path | Change |
| --- | --- |
| `source/L2-DISCIPLINE.md` | Add bilingual efficiency discipline and correct released authority comment. |
| `crates/ae-sdd-build/src/managed_instructions.rs` | New pure renderer and native target sync. |
| `crates/ae-sdd-build/src/lib.rs` | Export crate-owned managed-instruction API used by CLI/post-commit. |
| `crates/ae-sdd-build/src/jobs/model.rs` | Admit the bounded Admin entrypoint. |
| `crates/ae-sdd-build/src/post_commit.rs` | Add managed target request/result and execute sync after distribution. |
| `crates/ae-sdd-build/src/main.rs` | Add explicit Codex/Claude/ZCode instruction-file flags. |
| `.githooks/post-commit` | Pass global instruction targets to Rust post-commit. |
| `crates/ae-sdd-build/tests/managed_instruction_sync.rs` | Native end-to-end filesystem contract. |
| `crates/ae-sdd-build/tests/compatibility_routes.rs` | Rust-only hook/CLI and L2 source assertions. |
| `crates/ae-sdd-build/tests/migration_oracle.rs` | Test-only legacy semantic parity. |
| `source/docs/ae-sdd-implementation-architecture.md` | Current Rust distribution/injection ownership. |
| `source/standards/update-graph.json` | Prevent L2/Rust/hook/test drift. |
| `RELEASING.md` | Only if update-check requires native release-boundary clarification. |

## Risks and tradeoffs

- **Dirty overlapping work:** `ae-sdd-build` files already contain user edits. Implementation must use scoped diffs and stop on semantic overlap rather than overwrite them.
- **External-file authority:** normal tests must use temporary homes. Real global-file cleanup requires explicit approval and backup.
- **Partial host success:** one Admin transaction per host preserves per-file atomicity but not cross-host atomicity. This matches the bounded need and avoids adding a generalized multi-root job schema; every outcome must be reported.
- **Legacy backup parity:** native transactions roll back failed writes, but this slice does not port legacy persistent backup rotation or rollback CLI. The one-time installed-file migration performs an explicit backup.
- **Header compatibility:** removing timestamps improves deterministic replay. Tests must normalize legacy header metadata while preserving the accepted anchor regex.
- **No-anchor behavior:** automatic bootstrap remains forbidden; a host without anchors must receive a visible skip, not a silent new global instruction block.

## Definition of done

- All AC-01 through AC-10 are mapped to real evidence.
- Rust post-commit is the sole released automatic L2 injector.
- The detailed efficiency discipline is present once, inside the managed L2 block.
- Hook/package/release scans show no Python runtime authority.
- Focused tests, strict formatting/Clippy, workspace regression, ae-sdd evidence, and independent Review all pass.

