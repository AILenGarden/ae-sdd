# ae-sdd Native Workflow Execution Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

> **ae-sdd red line:** This document is a remediation roadmap, not a gate-approved CodingPlan. Before implementation, create and approve the Proposal -> RA -> DR -> Story -> TestCase -> CodingPlan chain described below. Do not edit production code, tests, configuration, schemas, build scripts, or generated runtime artifacts from this roadmap alone.

**Goal:** Turn ae-sdd from a prompt-driven state ledger with hard hooks into a Claude Code-native workflow in which the main conversation orchestrates, physical subagents execute series work with preloaded skills, and workflow state survives approvals, compaction, resume, and failure without false locks.

**Architecture:** Keep Work Item state, gates, and typed operations host-neutral. Add a Claude Code adapter that packages discoverable plugin skills and custom agents, binds physical Agent runs to dispatch records, and handles `SessionStart`, `PreCompact`, `PostCompact`, `SubagentStart`, `SubagentStop`, and `PostToolUse:Agent`. Replace turn-scoped activation with a session/work-item workflow lease, while retaining explicit pause, detach, resume, and backward-compatible legacy reads.

**Tech Stack:** Python 3.11+ CLI and hook handlers, JSON state/sidecars, Claude Code 2.1.210 plugin/skills/agents/hooks, Markdown ae-sdd documents, pytest, existing build/runtime compiler/update-graph tooling.

---

## 1. Planning Status

| Item | Status | Meaning |
| --- | --- | --- |
| Root-cause analysis | Complete | The executor, lifecycle bridge, and native skill/agent packaging are missing or incomplete. |
| Repository inspection | Complete | Current package is a single root skill; `subprocess spawn` only registers state; hooks cover only UserPromptSubmit/PreToolUse/Stop. |
| Official host capability check | Complete for Claude Code 2.1.210 | Plugin skills, plugin agents, skill preloading, and compact/subagent hooks are supported. |
| Proposal/RA/DR/Story/TestCase | Not created for this initiative | Must be created before any implementation plan is approved. |
| CodingPlan | Not created | This roadmap does not authorize coding. |
| Implementation | Not started | No production or test changes are included in this planning turn. |

## 2. Current Facts and Constraints

1. `tools/bin/ae-sdd::cmd_subprocess_spawn()` calls `state.register_subprocess_agent()` and writes state; it does not invoke Claude's Agent tool or start a session.
2. `subprocessAgents[].sessionId` can be empty, so `status=running` is not physical-execution evidence.
3. `activeAgents` and `subprocessAgents` are separate, partially overlapping ledgers.
4. The installed root `SKILL.md` is a short bootloader. Critical orchestration semantics arrive through later file reads, which are not the same as native Skill invocations.
5. Internal `skills/**/*.md` files are compiled bootloaders, not Claude plugin skills in `<name>/SKILL.md` form.
6. No `agents/` definitions are shipped.
7. `pre_compact_snapshot()` exists, but normal Claude auto-compaction is not wired to it.
8. `prompt_inject.inject()` clears activity on an ordinary continuation prompt, so `确认` and `继续` do not preserve workflow activation.
9. The existing `STORY-AE-SDD-SKILL-LOAD-DISCLOSURE-001` covers visible load declarations. It is a dependency/child scope of this initiative, not a replacement for native dispatch.
10. The design ledger currently marks D-004 as partly soft, but D-012/D-017/D-018/D-020 overstate lifecycle completeness.
11. The source tree is the only manual SSOT. `dist/ae-sdd/` and user installations remain generated outputs.
12. Existing project and global hooks must be preserved during migration; no installer may erase unrelated user hooks.

## 3. Success Criteria and Stop-Loss

### 3.1 Required acceptance metrics

| Metric | Target | Evidence |
| --- | --- | --- |
| Physical dispatch claim rate | 100% for required series agents | Every `running` agent run has a host-issued `agent_id` captured from `SubagentStart`. |
| Skill preload rate | 100% | Each series agent reports the expected plugin skill in its startup inventory/probe. |
| Continuation retention | 100% | After `/ae-sdd`, a later `确认` or `继续` remains attached to the same Work Item without repeating `/ae-sdd`. |
| Inactive false-block rate | 0% | Ordinary read-only and unrelated prompts in detached/inactive sessions are never blocked by ae-sdd. |
| Compact recovery | 100% in hook replay; pass in real manual-compact smoke | Snapshot exists before compact and recovery capsule restores Work Item, phase, active dispatch, pending decision, and next action. |
| Fake-spawn prevention | 100% | `dispatch prepare` alone remains `planned`; completion/finalization cannot treat it as a physical agent run. |
| Root critical contract size | <= 5,000 tokens | All invariants needed after compaction fit in the root skill reattachment budget. |
| Plugin validation | 0 errors and 0 strict warnings | `claude plugin validate dist/ae-sdd --strict`. |
| User recovery interventions | <= 1 per ten-scenario dogfood run | No repeated manual state surgery or hook disengage commands. |
| Utility against direct prompting | Demonstrable improvement | Same-task baseline shows fewer missed contract/test steps without unacceptable time/token overhead. |

### 3.2 Stop-loss rules

- Do not generalize beyond the Story series until a physical Story-agent vertical slice passes plugin validation, dispatch claim, continuation, compact recovery, and inactive-session tests.
- Do not add new hard gates before the vertical slice proves that the agent receives an actionable remediation path.
- If the vertical slice costs more than 30% additional wall time/tokens versus direct Claude prompting and does not reduce material defects or omissions, reduce documents/gates before adding features.
- If Claude host identity cannot be correlated reliably, keep the capability experimental and report `physicalDelegation=false`; never fall back to a fabricated session ID.
- If compact recovery cannot deterministically rebuild the next action, block workflow finalization, not generic repository reads or unrelated writes.

## 4. Target Architecture

```text
Main Claude conversation
  -> invokes root ae-sdd skill (durable <=5k contract)
  -> workflow attach(workItem, session)
  -> dispatch prepare(role=story, requiredSkill=ae-sdd-story)
  -> Agent(agent_type=ae-sdd:story-agent, prompt includes dispatch token)
       -> SubagentStart hook injects host agent_id and claim instruction
       -> story-agent starts with ae-sdd-story skill preloaded
       -> dispatch claim(token, host agent_id)
       -> executes Story series and writes declared deliverables
       -> SubagentStop validates and completes/fails dispatch
  -> PostToolUse:Agent reconciles state and injects exact next action to root
  -> root presents user review point
  -> user says confirm/continue
  -> workflow lease remains attached and advances

PreCompact
  -> snapshot root/subagent workflow capsule
PostCompact
  -> record compact summary fingerprint
SessionStart(source=compact|resume)
  -> inject recovery capsule + next action
```

### 4.1 Ownership boundaries

| Layer | Owns | Must not own |
| --- | --- | --- |
| Root skill/main conversation | Routing, user review points, dispatch, aggregation, next action | Series implementation work |
| Series agent | One RA/DR/Story/TestCase/Coding series | Cross-series phase advancement or user approval |
| Series skill | Exact executable protocol for one series | Host session management |
| Dispatch protocol | Physical agent lifecycle and deliverable status | Business gate semantics |
| Workflow session lease | Session-to-Work Item attachment and active/paused/detached state | Work Item writer lease/fencing |
| Compact bridge | Snapshot/recovery capsule | Reconstructing business facts by guessing transcript text |
| Gates | Irreversible transition and artifact truth | Broadly locking unrelated tools because a stale phase exists |

## 5. State and API Contracts

### 5.1 Canonical agent run schema

Add `agentRuns` as the canonical Work Item field. During one compatibility release, read legacy `activeAgents` and `subprocessAgents`, but write canonical records and only dual-project legacy fields where an existing gate still requires them.

```json
{
  "runId": "run-<uuid>",
  "dispatchTokenHash": "sha256:<hex>",
  "role": "story-agent",
  "seriesType": "story",
  "entityId": "STORY-001-BE",
  "host": "claude-code",
  "parentSessionId": "<claude-session-id>",
  "hostAgentId": "<from-SubagentStart>",
  "requiredSkills": ["ae-sdd-story"],
  "status": "planned|claimed|running|completed|failed|timed_out|cancelled",
  "deliverables": [],
  "preparedAt": "<iso8601>",
  "claimedAt": null,
  "completedAt": null,
  "failure": null
}
```

Invariants:

- `planned -> claimed/running` requires a host-issued `hostAgentId` and a valid one-time dispatch token.
- `running -> completed` requires `SubagentStop` or an explicitly audited recovery operation.
- `completed` requires all declared deliverables to resolve inside permitted project/document roots.
- No LLM-supplied free-form `sessionId` is accepted as proof of physical independence.
- A token is stored only as a hash and can be claimed once.

### 5.2 Workflow session binding schema

Store session bindings under `.ae-sdd/workflow-sessions/<session-hash>.json`; keep Work Item state authoritative for phase and deliverables.

```json
{
  "schemaVersion": "1",
  "sessionId": "<claude-session-id>",
  "workItemKey": "<explicit-work-item-key>",
  "mode": "active|awaiting_user|paused|detached|completed",
  "attachedAt": "<iso8601>",
  "updatedAt": "<iso8601>",
  "lastPromptClass": "entry|continuation|side-discussion|detach|resume",
  "snapshotId": null
}
```

Invariants:

- `确认`, `继续`, corrections, and answers to an ae-sdd review question retain attachment.
- `ae-sdd pause` keeps the binding but prevents workflow mutation.
- `ae-sdd detach` disables ae-sdd hooks for unrelated work without completing the Work Item.
- `ae-sdd resume --work-item <key>` reattaches explicitly; no single-candidate inference may silently bind a session.
- Workflow binding is separate from StateStore writer lease and from the physical agent run.

### 5.3 Typed operations

Extend `tools/lib/operations.py` with:

```text
workflow.attach
workflow.status
workflow.pause
workflow.resume
workflow.detach
dispatch.prepare
dispatch.claim
dispatch.complete
dispatch.fail
dispatch.cancel
```

Every write operation uses the existing operation envelope, revision CAS, idempotency key, containment checks, and lease rules where Work Item state mutates.

## 6. Work Item Decomposition

Create a large-route initiative instead of one oversized Story.

| Order | Work Item | Outcome | Depends on |
| --- | --- | --- | --- |
| 0 | `PRD-AE-SDD-NATIVE-WORKFLOW-001` | Problem/value scope, baseline, rollout, stop-loss | None |
| 1 | `DR-AE-SDD-NATIVE-WORKFLOW-001` | Host-neutral core + Claude adapter architecture | PRD/RA |
| 2 | `STORY-AE-SDD-CLAUDE-PLUGIN-001` | Root skill + discoverable series skill + Story agent package | DR |
| 3 | `STORY-AE-SDD-DISPATCH-PROTOCOL-001` | Physical run prepare/claim/complete protocol | DR |
| 4 | `STORY-AE-SDD-WORKFLOW-LEASE-001` | Multi-turn active/awaiting/paused/detached/resume semantics | Dispatch schema |
| 5 | `STORY-AE-SDD-COMPACT-RECOVERY-001` | Pre/PostCompact snapshot and SessionStart recovery | Workflow lease |
| 6 | `STORY-AE-SDD-STORY-VERTICAL-001` | End-to-end Story-series native execution | Plugin + dispatch + lease + compact |
| 7 | `STORY-AE-SDD-GATE-REBALANCE-001` | Gates target transitions/artifact truth, not stale broad locks | Vertical proof |
| 8 | `STORY-AE-SDD-SERIES-AGENTS-001` | RA/DR/TestCase/Coding/reviewer agents generalized | Story vertical accepted |
| 9 | `STORY-AE-SDD-HOST-E2E-001` | Real Claude Code probes, metrics, rollout gate | All prior |

`STORY-AE-SDD-SKILL-LOAD-DISCLOSURE-001` becomes a dependency of `STORY-AE-SDD-CLAUDE-PLUGIN-001`: preserve visible load declaration, but do not treat disclosure as proof that a skill was preloaded or an agent was spawned.

## 7. Step-by-Step Implementation Plan

### Task 0: Freeze the baseline and measure direct prompting

**Objective:** Establish evidence that the new engine must beat, and prohibit more rule/gate expansion before a vertical proof.

**Files:**

- Create: `ae-sdd-doc/CR/PRD-AE-SDD-NATIVE-WORKFLOW-001/PRD-AE-SDD-NATIVE-WORKFLOW-001-Proposal.md`
- Create: `ae-sdd-doc/RA/RA-AE-SDD-NATIVE-WORKFLOW-001.md`
- Create: `tools/e2e/fixtures/native-workflow/README.md`
- Create: `tools/e2e/native_workflow_scenarios.yaml`
- Create: `tools/e2e/run_direct_vs_ae_sdd_baseline.py`
- Test: `tools/tests/test_native_workflow_baseline.py`

**Steps:**

1. Define ten representative scenarios: new Story, existing Story coding, user approval continuation, side discussion, detach/resume, manual compact, failed agent, missing deliverable, parallel reviewer, completed workflow.
2. Define metrics from section 3; do not use subjective “felt better” as the only result.
3. Write a RED test requiring every scenario to have an expected outcome and a direct/ae-sdd comparison field.
4. Implement only the baseline runner and fixture parser.
5. Run `py -3 -m pytest tools/tests/test_native_workflow_baseline.py -q`; expect PASS.
6. Run the baseline manually with a fixed Claude model and record version/model/date; do not include secrets or environment dumps.
7. Commit docs/fixture only: `docs: define native workflow baseline and stop-loss`.

### Task 1: Complete the legal ae-sdd design chain

**Objective:** Produce the required Proposal -> RA -> DR -> Story -> TestCase -> CodingPlan artifacts before implementation.

**Files:**

- Create: `ae-sdd-doc/DR/DR-AE-SDD-NATIVE-WORKFLOW-001.md`
- Create: `ae-sdd-doc/Story/STORY-AE-SDD-CLAUDE-PLUGIN-001.md`
- Create: `ae-sdd-doc/Story/STORY-AE-SDD-DISPATCH-PROTOCOL-001.md`
- Create: `ae-sdd-doc/Story/STORY-AE-SDD-WORKFLOW-LEASE-001.md`
- Create: `ae-sdd-doc/Story/STORY-AE-SDD-COMPACT-RECOVERY-001.md`
- Create: `ae-sdd-doc/Story/STORY-AE-SDD-STORY-VERTICAL-001.md`
- Create matching TestCase and CodingPlan documents under `ae-sdd-doc/Test/<STORY-ID>/` and `ae-sdd-doc/Coding/<STORY-ID>/`.
- Modify: `ae-sdd-doc/STORING.md`

**Steps:**

1. Put interface contracts, state fields, transitions, errors, and rollback rules in DR/Story, not only in CodingPlan.
2. Give every AC at least one positive, negative, recovery, and compatibility TestCase where applicable.
3. Map every CodingPlan task to Story AC and TestCase IDs.
4. Run document resolution for Proposal/DR/Story/TestCase/CodingPlan and record exact paths.
5. Run G-CODEPLAN-SRC, G-14, G-07, and G-08 for each implementation Story.
6. Present CodingPlans to the user and stop until explicit approval.

### Task 2: Prove Claude plugin component discovery before changing runtime behavior

**Objective:** Verify the exact naming/preload semantics of root skill + nested skills + plugin agent on Claude Code 2.1.210.

**Files:**

- Create: `source/adapters/claude-code/skills/ae-sdd-story/SKILL.md`
- Create: `source/adapters/claude-code/agents/ae-sdd-story-agent.md`
- Create: `tools/tests/test_claude_plugin_package.py`
- Modify: `scripts/build_dist.py`
- Modify: `dist` verification logic in `tools/lib/runtime_verify.py`

**Steps:**

1. Write a RED package test requiring generated `dist/ae-sdd/skills/ae-sdd-story/SKILL.md` and `dist/ae-sdd/agents/ae-sdd-story-agent.md`.
2. Add a generated manifest field `skills: ["./"]` so the root skill remains discoverable while default `skills/` scanning adds child skills.
3. Give the Story agent a `skills:` entry for the generated Story skill; determine the correct plugin-scoped identifier through validation/probe, not assumption.
4. Build with `py -3 scripts/build_dist.py`.
5. Validate with `claude plugin validate dist/ae-sdd --strict`; expected: exit 0, no warnings.
6. Run `claude plugin inspect` or the closest supported inventory command and archive only the component inventory.
7. Add a non-mutating probe proving the agent starts with the full Story skill content.
8. If preload naming is not stable, stop and revise the DR; do not compensate with prompt-only file reads.
9. Commit: `feat: package discoverable Claude story skill and agent`.

### Task 3: Make the root skill compaction-durable

**Objective:** Keep all root orchestration invariants inside Claude's first-5,000-token skill reattachment budget.

**Files:**

- Modify: `source/skill-fallbacks/SKILL.full.md`
- Modify: `source/SKILL.md` through the source-slimming generator, not by hand after generation.
- Modify: `scripts/compile_skill_runtime.py`
- Modify: `source/docs/skill-runtime-compiler.md`
- Test: `tools/tests/test_skill_runtime_compiler.py`
- Test: `tools/tests/test_runtime_verify.py`

**Required durable contract:**

- Root is orchestrator only.
- Physical work requires Agent + claimed dispatch.
- `subprocess register` is not spawn proof.
- Exact workflow attach/pause/detach/resume semantics.
- Exact compact recovery action.
- Exact next-action query.
- Visible skill/agent load disclosure.
- Fail-closed only at irreversible transitions, with executable remediation.

**Steps:**

1. Write a RED compiler test that searches the generated root `SKILL.md`, not fallback files, for every durable invariant.
2. Add a token/character budget test and fail generation if critical contract exceeds the agreed budget.
3. Generate the root executable contract directly into the invoked skill body; do not rely on a later Read for these invariants.
4. Preserve optional detail in runtime slices/fallback.
5. Build twice and compare byte snapshots for idempotence.
6. Run `py -3 -m pytest tools/tests/test_skill_runtime_compiler.py tools/tests/test_runtime_verify.py -q`.
7. Commit: `fix: preserve workflow invariants across skill compaction`.

### Task 4: Implement canonical dispatch state with TDD

**Objective:** Make physical agent lifecycle machine-verifiable and unify the two legacy ledgers.

**Files:**

- Create: `tools/lib/agent_dispatch.py`
- Modify: `tools/lib/state.py`
- Modify: `tools/lib/state_store.py` only if new atomic mutation support is required.
- Modify: `tools/lib/operations.py`
- Modify: `tools/bin/ae-sdd`
- Test: `tools/tests/test_agent_dispatch.py`
- Test: `tools/tests/test_agent_dispatch_concurrency.py`
- Modify: `tools/tests/test_subprocess_agent.py` for compatibility/deprecation assertions.

**Steps:**

1. Write RED tests for prepare, one-time claim, host ID requirement, duplicate claim, wrong token, completion, failure, timeout, cancellation, containment, revision conflict, and idempotent retry.
2. Implement the `agentRuns` schema and pure transition validator.
3. Implement `dispatch.prepare/claim/complete/fail/cancel` typed operations.
4. Add CLI convenience commands that call the typed registry instead of writing state directly.
5. Keep `ae-sdd subprocess spawn` as a deprecated alias for `dispatch prepare` for one release, but change its output to `dispatch planned; no physical agent started`.
6. Add compatibility projection for gates still reading `activeAgents`; do not let legacy state establish physical proof.
7. Run focused tests, then state/operations concurrency tests.
8. Commit: `feat: add host-bound agent dispatch protocol`.

### Task 5: Replace turn activity with a workflow session lease

**Objective:** Preserve multi-turn continuity without reviving stale-session false locks.

**Files:**

- Create: `tools/lib/workflow_session.py`
- Modify: `tools/lib/work_item_context.py`
- Modify: `tools/lib/prompt_inject.py`
- Modify: `tools/lib/gate_intercept.py`
- Modify: `tools/lib/stop_check.py`
- Modify: `tools/lib/operations.py`
- Modify: `tools/bin/ae-sdd`
- Test: `tools/tests/test_workflow_session.py`
- Test: `tools/tests/test_prompt_inject_plugin.py`
- Test: `tools/tests/test_gate_intercept.py`
- Test: `tools/tests/test_stop_check.py`

**Steps:**

1. Write RED tests for attach, approval continuation, ordinary workflow answer, side discussion, pause, detach, explicit resume, completed cleanup, stale TTL diagnostics, and cross-session isolation.
2. Create `.ae-sdd/workflow-sessions/` sidecars with containment-safe hashed filenames.
3. Keep `.hook-activity/` only as short-lived per-turn execution evidence if still needed; it must no longer be workflow ownership.
4. Change UserPromptSubmit classification so continuation/approval remains attached and unrelated prompts do not mutate state.
5. Require explicit Work Item on resume; remove single-candidate auto-binding from the workflow path.
6. Change Stop behavior: `awaiting_user` persists, `completed/detached` releases, blocked retry remains attached with a bounded retry count.
7. Run focused hook/session tests.
8. Commit: `feat: add persistent workflow session attachment`.

### Task 6: Wire the real compact lifecycle

**Objective:** Snapshot before manual/auto compact and restore exact workflow state after compact/resume.

**Files:**

- Create: `tools/lib/compact_bridge.py`
- Modify: `tools/lib/memory_store.py`
- Modify: `tools/lib/prompt_inject.py`
- Modify: `tools/bin/ae-sdd`
- Modify: `cmd_init_hooks()` in `tools/bin/ae-sdd`
- Modify: `scripts/init.py`
- Test: `tools/tests/test_compact_bridge.py`
- Test: `tools/tests/test_compact_reload.py`
- Test: `tools/tests/test_fixes_v14.py`

**Hook additions:**

```text
PreCompact manual|auto -> ae-sdd hook pre-compact
PostCompact manual|auto -> ae-sdd hook post-compact
SessionStart startup|resume|compact -> ae-sdd hook session-start
```

**Steps:**

1. Write RED payload-replay tests using official Claude fields: `session_id`, `cwd`, `trigger`, `compact_summary`, and `source`.
2. Snapshot only structured state: work item, phase, workflow mode, current/active dispatch, pending review question, next action, and memory fingerprint.
3. Never parse lagging transcript content as the sole source of truth.
4. On `SessionStart(source=compact|resume)`, return a bounded `additionalContext` recovery capsule.
5. On `PostCompact`, record the compact summary hash/metadata for diagnostics; do not treat the summary as authority over state.
6. Preserve unrelated user hooks during `init-hooks --force` and add idempotent event entries.
7. Run hook replay tests and a real manual `/compact` smoke in a disposable fixture project.
8. Commit: `feat: bridge Claude compact lifecycle to workflow state`.

### Task 7: Bind SubagentStart/SubagentStop to dispatch

**Objective:** Prove that a planned dispatch became a physical Claude subagent and reconcile its result automatically.

**Files:**

- Create: `tools/lib/subagent_hooks.py`
- Modify: `tools/bin/ae-sdd`
- Modify: `cmd_init_hooks()` in `tools/bin/ae-sdd`
- Test: `tools/tests/test_subagent_hooks.py`
- Test: `tools/tests/test_agent_dispatch.py`

**Hook additions:**

```text
SubagentStart matcher ae-sdd:* -> inject host agent_id + claim requirement
SubagentStop matcher ae-sdd:* -> validate deliverables + complete/fail dispatch
PostToolUse matcher Agent -> reconcile root next action
```

**Steps:**

1. Write RED tests for plugin-scoped `agent_type`, `agent_id`, missing dispatch token, mismatched role, missing deliverables, failed agent, blocked SubagentStop retry, and successful completion.
2. Make the delegation prompt carry a one-time dispatch token; make SubagentStart inject the host agent ID and exact claim command.
3. Require the preloaded series skill to claim before business work.
4. Use `last_assistant_message` and declared artifact paths at SubagentStop; do not trust a self-reported `completed` word alone.
5. Complete/fail the canonical run and project compatibility fields.
6. Return an exact next action to the root after Agent tool completion.
7. Commit: `feat: bind Claude subagent lifecycle to dispatch records`.

### Task 8: Deliver the Story-series vertical slice

**Objective:** Demonstrate the intended user experience end to end before generalizing.

**Files:**

- Finalize: `source/adapters/claude-code/skills/ae-sdd-story/SKILL.md`
- Finalize: `source/adapters/claude-code/agents/ae-sdd-story-agent.md`
- Modify: `source/skill-fallbacks/skills/cross-cutting/agent-orchestration-skill.full.md`
- Regenerate: `source/skills/cross-cutting/agent-orchestration-skill.md`
- Test: `tools/e2e/test_story_vertical.py`
- Test: `tools/tests/test_claude_plugin_package.py`

**Steps:**

1. Make the Story skill self-contained enough to execute after preload; supporting references may be read, but role/claim/input/output/gate/return contracts must be in the preloaded body.
2. Restrict the Story agent to one Story series and declared paths.
3. Root prepares a dispatch, invokes the plugin Story agent, waits for completion, aggregates deliverables, and presents the review point.
4. User `确认` advances without a second `/ae-sdd` invocation.
5. Run manual compact between agent completion and user approval; verify recovery.
6. Detach and issue unrelated commands; verify zero ae-sdd blocks.
7. Compare against the direct-prompt baseline and enforce stop-loss.
8. Ask the user to accept or reject generalization based on evidence.
9. Commit: `feat: complete native Story workflow vertical slice`.

### Task 9: Rebalance hard gates after the vertical slice

**Objective:** Keep fail-closed protection at irreversible boundaries while eliminating broad stale locks.

**Files:**

- Modify: `tools/lib/gate_intercept.py`
- Modify: `tools/lib/gates.py`
- Modify: `tools/lib/flow_monitor.py`
- Modify: `source/HARNESS.md`
- Modify: `source/docs/ae-sdd-design.md`
- Test: `tools/tests/test_gate_intercept.py`
- Test: `tools/tests/test_gate_intercept_v11.py`
- Test: `tools/tests/test_gates.py`
- Test: `tools/tests/test_workflow_session.py`

**Steps:**

1. Inventory every PreToolUse deny branch and classify it as transition truth, path containment, destructive action, stale-state protection, or advisory workflow guidance.
2. Keep hard enforcement for containment, destructive actions, unauthorized state transitions, unapproved CodingPlan execution, and false completion evidence.
3. Convert broad phase/tool bans that have actionable state alternatives into scoped checks or advisory context.
4. Every deny result must include Work Item, workflow mode, failed invariant, exact remediation command, and detach command where appropriate.
5. Add negative tests proving inactive/detached sessions and unrelated read-only operations are unaffected.
6. Run the historical `life` lock scenarios as regression fixtures.
7. Commit: `fix: scope hard gates to irreversible workflow boundaries`.

### Task 10: Generalize to remaining series and reviewers

**Objective:** Add RA, DR, TestCase, Coding, and reviewer agents only after Story acceptance.

**Files:**

- Create: `source/adapters/claude-code/skills/ae-sdd-{ra,dr,testcase,coding,review}/SKILL.md`
- Create: `source/adapters/claude-code/agents/ae-sdd-{ra,dr,testcase,coding,reviewer}.md`
- Modify: `scripts/build_dist.py`
- Modify: `scripts/compile_skill_runtime.py`
- Modify: `tools/lib/review_loop.py`
- Modify: `tools/lib/review_batch.py`
- Test: `tools/tests/test_claude_series_agents.py`
- Test: `tools/tests/test_review_loop.py`
- Test: `tools/tests/test_review_batch.py`

**Steps:**

1. Extract shared agent contract generation rather than copying five prompts.
2. Keep series-specific AC/gates/deliverables in the corresponding Skill.
3. Route reviewer Tier requirements through canonical dispatch runs with host IDs.
4. Remove acceptance of arbitrary `sessionId != root` as sufficient physical-independence proof.
5. Limit nested agents until serial series execution is stable; add nested reviewers as a separate tested capability.
6. Run one end-to-end scenario per series and one Tier-3 reviewer scenario.
7. Commit: `feat: add native agents for all ae-sdd series`.

### Task 11: Add host-level E2E and observability

**Objective:** Make real host behavior, not only Python helper behavior, a release gate.

**Files:**

- Create: `tools/e2e/run_claude_native_workflow.py`
- Create: `tools/e2e/test_claude_native_workflow.py`
- Create: `tools/lib/workflow_metrics.py`
- Modify: `tools/lib/runtime_stats.py`
- Modify: `apps/ae-sdd-monitor/` only for read-only projection after CLI/state stabilizes.
- Test: `tools/tests/test_workflow_metrics.py`

**Scenarios:**

1. Plugin inventory contains root skill, series skills, and series agents.
2. Story dispatch is claimed by a real host agent ID.
3. Agent starts with the required Skill preloaded.
4. `确认/继续` preserves attachment.
5. Detach prevents false locks.
6. Manual compact recovers exact next action.
7. Simulated auto-compact payload follows the same handler.
8. Failed/missing-deliverable agent cannot complete the workflow.
9. Parallel reviewers have distinct host IDs.
10. Legacy state loads without being mistaken for physical proof.

**Steps:**

1. Keep deterministic hook payload replay in normal CI.
2. Put real Claude invocation behind `AE_SDD_RUN_CLAUDE_E2E=1` and an explicit cost/model configuration.
3. Redact prompts/results before storing fixtures; never archive credentials or full environment variables.
4. Emit the section 3 metrics as JSON and a concise Markdown report.
5. Fail release promotion when physical claim, preload, continuation, false-block, or compact metrics miss target.
6. Commit: `test: add Claude-native workflow acceptance suite`.

### Task 12: Synchronize design ledger, update graph, packaging, and release

**Objective:** Make the new architecture truthful, reproducible, and impossible to silently regress.

**Files:**

- Modify: `source/docs/ae-sdd-design.md`
- Modify: `source/docs/ae-sdd-implementation-architecture.md`
- Modify: `source/docs/skill-runtime-compiler.md`
- Modify: `source/HARNESS.md`
- Modify: `source/standards/update-graph.json`
- Modify: `tools/lib/update_graph.py`
- Modify: `tools/tests/test_update_graph.py`
- Modify: `source/skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md`
- Regenerate: `source/skills/orchestration/ae-sdd-update-skill.md`
- Modify: `README.md`
- Create: `source/CHANGELOG/<release>-native-workflow-execution.md`
- Modify version sources only after acceptance.

**Design ledger impact:**

- Add D-025: Host-native workflow execution and lifecycle bridge.
- Update D-004: physical agent evidence and canonical dispatch.
- Update D-012: real Pre/PostCompact and SessionStart recovery boundary.
- Update D-017: workflow session attachment replaces turn-only ownership.
- Update D-018: root orchestrator is main conversation + native Agent dispatch, not UserPromptSubmit alone.
- Update D-020: critical root contract and standalone series skills fit Claude compaction/preload semantics.
- Update D-014: add host-level truth checks so structural tests cannot certify fake execution.

**New update contract:**

- Add UG-29 for workflow engine/Claude adapter/agent skill/hook lifecycle changes.
- Add UC-21 to check plugin agents/skills/hooks, critical root contract, dispatch operations, host-level test presence, Design Ledger coverage, and changelog impact.

**Verification commands:**

```powershell
py -3 scripts/build_dist.py
claude plugin validate dist/ae-sdd --strict
py -3 tools/bin/ae-sdd runtime verify --json
py -3 tools/bin/ae-sdd update-check --only UC-15 --json
py -3 tools/bin/ae-sdd update-check --only UC-20 --json
py -3 tools/bin/ae-sdd update-check --only UC-21 --json
py -3 -m pytest tools/tests/test_agent_dispatch.py tools/tests/test_workflow_session.py tools/tests/test_compact_bridge.py tools/tests/test_subagent_hooks.py tools/tests/test_claude_plugin_package.py -q
```

Run the broader affected suite returned by `update-check --affected`; do not claim full completion if the real Claude E2E is skipped.

## 8. Rollout and Compatibility

### Phase A: Observe

- Add `workflowEngine.mode: legacy|observe|native-claude` to project config.
- Default existing installations to `legacy`; allow opt-in `observe` to collect comparisons without changing gates.
- Package agents/skills and validate them, but do not require native dispatch yet.

### Phase B: Story vertical opt-in

- Enable `native-claude` only for the Story series.
- Preserve deprecated CLI aliases with explicit warnings.
- Dogfood in `ae-sdd` and one external project with a disposable Work Item.

### Phase C: Native default for Claude

- Make Claude default native only after all section 3 targets pass.
- Keep other runtimes on explicit capability results:

```json
{
  "host": "codex",
  "physicalDelegation": false,
  "compactLifecycle": false,
  "mode": "logical-explicit",
  "limitations": ["No host adapter installed"]
}
```

- Never call logical fallback “physical multi-agent.”

### Phase D: Legacy removal

- Remove `subprocess spawn` only after at least one compatibility release.
- Migrate `activeAgents`/`subprocessAgents` readers to `agentRuns`, then stop dual projection.
- Remove `.hook-activity` as workflow ownership only after session lease adoption is proven.

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Plugin skill namespacing differs from assumptions | Agents start without required skill | Capability spike + strict validation + real preload probe before implementation. |
| Static plugin hooks cannot find Python consistently | Hooks fail on some systems | Keep interpreter-aware `init-hooks` for v1; migrate to plugin hooks only after a cross-platform launcher is proven. |
| Workflow lease recreates stale session lock | Original UX failure returns | Explicit attach/detach, no single-candidate binding, inactive tests, bounded TTL diagnostics. |
| Agent claims without executing | False physical proof | Host `agent_id` + one-time token + SubagentStop + artifact validation. |
| Compaction summary conflicts with state | Wrong recovery | State/capsule is authority; summary is diagnostic only. |
| Generated package diverges from source | Fix disappears on next build | Source adapter SSOT + deterministic build + UC-15/UC-21. |
| Five agents multiply context/cost | Utility remains worse than direct prompting | Story-only vertical slice, serial series agents, stop-loss metrics before generalization. |
| Existing load-disclosure work duplicates scope | Conflicting contracts | Make disclosure a child dependency of plugin packaging and close it through the same generated root/series contracts. |
| Reviewer gates still trust arbitrary IDs | Fake independence persists | Migrate G-09/G-09B/G-AUTO-CONSENSUS to canonical host-bound runs. |
| Real Claude E2E is flaky or expensive | CI instability | Deterministic payload replay in CI, explicit opt-in live suite for release/dogfood. |

## 10. Review Checklist Before Coding

- [ ] Proposal clearly states why ae-sdd must outperform direct prompting.
- [ ] RA covers user, root agent, series agent, reviewer, hook, installer, and non-Claude host roles.
- [ ] DR defines ownership and state transitions without relying on prose inference.
- [ ] Each Story has interface/field tables and ACs.
- [ ] Each TestCase document covers positive, negative, recovery, compatibility, and real-host behavior.
- [ ] CodingPlans reference Story AC and TestCase IDs in every task.
- [ ] The Story vertical slice is independently shippable and removable.
- [ ] Plugin naming/preload behavior is proven on Claude Code 2.1.210 or the then-current supported version.
- [ ] No plan step hand-edits `dist/` or installed skill directories.
- [ ] No state can say `running` without host-issued identity.
- [ ] `确认/继续`, detach/resume, compact/resume, and failure all have deterministic tests.
- [ ] Gate changes include false-positive regressions from the `life` incidents.
- [ ] Real Claude E2E execution and any skipped evidence are reported honestly.
- [ ] D-004/D-012/D-014/D-017/D-018/D-020/D-025 and CHANGELOG impact are synchronized.
- [ ] User explicitly approves each CodingPlan before implementation starts.

## 11. Recommended First Approval Boundary

Approve only Tasks 0-8 through the Story vertical slice. Do not pre-authorize Tasks 9-12.

The vertical slice must demonstrate this exact observable sequence:

```text
user invokes /ae-sdd for an explicit Work Item
root attaches workflow and declares the Story skill/agent
root prepares dispatch
Claude Agent tool starts ae-sdd Story agent
SubagentStart supplies a real host agent_id
Story agent has ae-sdd-story preloaded and claims dispatch
Story deliverable is produced and validated
SubagentStop completes the canonical run
root presents review point
user says confirm/continue without /ae-sdd
workflow continues
manual compact and resume preserve the same next action
detach makes unrelated work completely free of ae-sdd locks
```

Only after this sequence passes and beats the direct-prompt baseline should the team approve gate rebalancing, remaining series agents, default rollout, and legacy removal.
