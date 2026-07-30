---
name: ae-sdd
version: 4.0.0
description: |
  Declarative ae-sdd method, template, and output contract. The Rust daemon owns
  workflow state, gates, transitions, delegation, context projection, and audit.
runtime_contract: daemon-v1
declarative_only: true
triggers:
  - "/ae-sdd"
  - "启动自动化工程"
  - "端到端实现"
  - "继续流程"
  - "继续上次"
  - "ae-sdd-quick"
---

# ae-sdd Declarative Contract

This SKILL declares how an Agent talks to ae-sdd. It is not the workflow
executor. The deterministic process executor is the Rust `FlowRuntime` instance
inside the per-user `ae-sddd` daemon.

## Authority

| Concern | Authority |
| --- | --- |
| Legal next action, phase, gate result, correction, transition | `FlowRuntime` and committed daemon state |
| Project mutation | `WorkItemActor` under lease, revision, fencing, and idempotency checks |
| Child lifecycle and physical identity | `Delegation` plus trusted `HostRuntimeAdapter` attestation |
| Context injection and compact lifecycle | `ContextProjection` and `CompactManager` |
| Methodology, templates, output shape | this package under `source/standards`, `source/templates`, and `source/skills` |

Prompt text, a local file scan, a child self-report, and an Agent's own phase
inference never override daemon state.

## Client Contract

- Use the Rust `ae-sdd` CLI only. Do not invoke repository scripts, Python,
  local Gate implementations, or local state mutation as a fallback.
- Every business request follows a successful endpoint-manifest read and
  `runtime.handshake`; the client validates `bootId`, `policyDigest`, protocol,
  limits, and the daemon capability public key.
- If the daemon is unavailable or the endpoint is stale, reads return a
  structured error and an engaged Hook denies or blocks. The Agent must not
  approximate a result locally.
- Project-scoped methods carry explicit workspace, work-item, session, turn,
  revision, lease/fencing, idempotency, and confirmation fields when required by
  the operation registry.

## Runtime Methods

| Area | Methods | Contract |
| --- | --- | --- |
| Runtime | `runtime.handshake`, `runtime.status`, `runtime.drain` | Authenticate endpoint and expose lifecycle/capabilities |
| Workspace | `workspace.register`, `workspace.snapshot`, `workspace.mode_transition` | Register Shadow only; exact `/ae-sdd` may request the confirmed Hook-only Shadow-to-RustCanary bootstrap edge; admin cutover remains drained and parity-checked |
| Session/Hook | `session.open`, `session.heartbeat`, `session.close`, `hook.user_prompt`, `hook.pre_tool`, `hook.post_tool`, `hook.stop` | Bind physical session and turn; return a host-compatible decision |
| Flow | `flow.snapshot`, `flow.next` | Return the committed process projection and legal `nextAction` |
| Delegation | `delegation.create`, `delegation.status`, `delegation.accept`, `delegation.report`, `delegation.collect`, `delegation.cancel` | Create and attest physical child work; validate bounded results |
| Operations/Gates | `operation.describe`, `operation.execute` (incl. `workitem.create`, `route.decide`), `gate.evaluate` | Execute typed reads or guarded mutations; `/ae-sdd` invokes daemon-owned `workitem.create(entryNode=ROUTE)` idempotently and binds the session; `route.decide` accepts typed intent/impact/confidence facts and persists the `RouteEngine` decision under revision, lease, and idempotency authority; non-PASS never becomes PASS |
| Events/Jobs | `events.subscribe`, `job.submit`, `job.status`, `job.cancel` | Submit, observe, and cancel authorized bounded background work |
| Context | `context.get`, `context.project` | Return role-aware full, delta, or no-change projections |
| Compact | `compact.request`, `compact.status` | Track snapshot, host request, correlated ACK, and rehydrate |
| Host | `host.register`, `host.capabilities`, `host.action_next`, `host.action_ack`, `host.pressure_report` | Bridge native spawn/send/wait/cancel/compact and authenticated pressure telemetry |

## Declared Routes

The Agent supplies intent and available facts; `FlowRuntime` selects and advances
the route. The current methodology declares these minimum routes:

| Size | Required design chain before Coding |
| --- | --- |
| large | Route -> Requirement Analysis -> DR/Story/CodingPlan -> approved `executionPlan` |
| medium | Route -> Requirement Analysis -> Story/CodingPlan -> approved `executionPlan` |
| small/micro | Route -> Requirement Analysis -> CodingPlan or Story-lite -> approved `executionPlan` |

Requirement Analysis occurs after routing. Its conclusion may select DR, Story,
or a compact `state.executionPlan`. RA, DR, and Story remain the core design
documents; TestCase is optional for a genuinely complex verification matrix.
Only the user can approve `executionPlan` or explicitly select the quick route.

## Hook Activation Semantics

- `engaged` is session-scoped and is derived by the daemon from the registered
  workspace writer mode. An engaged ordinary prompt may receive the session's
  current context projection, but it never creates or rebinds a Work Item.
- `/ae-sdd` is the command-level bootstrap trigger. On an engaged unbound root
  session, the daemon allocates the turn, creates one `ROUTE-*` Work Item,
  durably binds it, and returns an `analyze-route` next action. Repeating the
  command on the bound session is idempotent.
- The host reopens the external session for every host event and refreshes the
  boot-scoped capability. Session TTL expiry and daemon restart are recovery
  details and are not exposed as user workflow steps.
- `workspace.register` never activates RustCanary. Exact `/ae-sdd` uses a
  distinct audited bootstrap activation that permits only Shadow-to-RustCanary;
  it cannot act as a general mode transition. Same-event session-open retries
  replay, while later Hook events commit a durable TTL refresh and retain the
  Work Item, role, delegation, grant, and physical attestation.

## Root Session Contract

The root Agent session is an orchestrator and reporter, not a series worker.

1. Intake first: the host forwards `/ae-sdd` with cwd and its external session
   identity only. The daemon creates and binds a `ROUTE-*` Work Item; neither
   Host nor Agent supplies `engaged`, `sessionId`, `turnId`, or `workItemId`.
2. Request `flow.next` and follow `analyze-route`. Submit typed facts through
   `route.decide`; never infer scale, design route, or `targetPhase` locally.
3. When semantic work is required, request `delegation.create`; do not simulate a
   child by changing prompts inside the root session.
4. Let the host adapter create an independent physical session. Host ACK alone is
   not proof; the child must claim the issued identity.
5. Collect only a validated bounded `ChildResult` after artifact validation and
   memory cleanup have committed.
6. Report the bounded summary, findings, artifact references, and next actions to
   the user. Do not import child transcripts or source payloads into root context.
7. Only a root transition intent may enter the guarded transition path; the
   runtime remains the sole transition owner.

The lineage is `root -> series -> task|reviewer`; maximum delegation depth is 2.
Task and reviewer sessions cannot delegate further, and reviewers must remain
physically independent from the work they review.

## ChildResult Contract

| Field | Requirement |
| --- | --- |
| `summary` | UTF-8, at most 8 KiB, no transcript |
| result envelope | at most 64 KiB |
| `findings` | typed severity/code/message entries with bounded counts |
| `artifacts` | path, sha256, kind, and optional schema reference only |
| `nextActions` | typed suggestions; never an executed transition |
| completion | accepted only after required deliverables, artifact validation, and `MemoryCleanupReceipt` |

The root context projection is at most 64 KiB. Oversized data becomes an
artifact reference or is rejected; it is never silently injected.

## Context And Compact

- Context is role-, lineage-, workspace-, work-item-, and generation-scoped.
- Hooks request a delta using `contextRevision` and digest; they do not scan the
  workspace or rebuild prompt context synchronously.
- Advisory compact is a flow suggestion only: at a collected Root→Series
  boundary the daemon may return a `suggest-compact` `flow.next` action or a
  `compactAdvice` field from `delegation.collect`. It never mutates context,
  never starts a compact cycle, and may be ignored by the host.
- Active compact is host-executed and may be triggered only from authenticated
  host token-pressure samples. Projection bytes are not token telemetry.
- A compact cycle reaches `context-restored` only after a durable snapshot,
  correlated native host ACK, and successful projection rehydrate. Unsupported
  hosts return `manual` or `rotate`; no ACK is fabricated.

## Methodology Resources

- Project and technical constraints: `standards/**`
- RA/DR/Story and verification templates: `templates/**`
- Phase-specific semantic contracts: `skills/**`
- Full declarative method contract: `skill-fallbacks/SKILL.full.md`
- Runtime/service/cutover contracts: `skill-fallbacks/runtime/**`

These resources define inputs and outputs. They do not execute the phase machine.

## Completion Contract

Completion can be reported only when the daemon returns a legal terminal state,
real verification evidence is finalized, and review status/findings are
committed. No Proposal, CodingReport, TestReport, CodeReview report, or changelog
file is created by the runtime workflow.
