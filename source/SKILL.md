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
| Operations/Gates | `operation.describe`, `operation.execute` (incl. `workitem.create`, `route.decide`), `gate.evaluate` | Execute typed reads or guarded mutations; `/ae-sdd` invokes daemon-owned `workitem.create(entryNode=ROUTE)` idempotently and binds the session; after RA closes, `route.decide` derives a candidate from the verified SRS receipt and scale evidence, while `RouteSelected` freezes authority only after the bound approval Gate passes; non-PASS never becomes PASS |
| Events/Jobs | `events.subscribe`, `job.submit`, `job.status`, `job.cancel` | Submit, observe, and cancel authorized bounded background work |
| Context | `context.get`, `context.project` | Return role-aware full, delta, or no-change projections |
| Compact | `compact.request`, `compact.status` | Track snapshot, host request, correlated ACK, and rehydrate |
| Host | `host.register`, `host.capabilities`, `host.action_next`, `host.action_ack`, `host.pressure_report` | Bridge native spawn/send/wait/cancel/compact and authenticated pressure telemetry |

## Provided Documents Adoption

When the caller already has PRD/DR/Story documents, `workitem.create` accepts an
optional `providedDocuments` array (at most 64 entries) instead of minting new
document paths:

```json
{"intent":"PRD"|"DR"|"STORY", "docId":"PRD-001", "path":"docs/PRD-001.md", "parentDocId":"DR-001"?}
```

- `path` must be project-relative, reject `..`/absolute forms under the existing
  traversal validation, and name an existing file. `parentDocId` links STORY to
  its parent DR and DR to its parent PRD (PRD has none). Invalid `intent`,
  duplicate `docId`, or a `parentDocId` naming no provided document is a schema
  error.
- Adoption is register-only: the daemon never reads, writes, or copies the user
  file and never mints the default document for an adopted intent.
  `documentPaths[intent]` records the first provided path of that intent (intents
  without one keep the existing minted default) and
  `routeDocuments[intent]=true` makes the handoff skip that series' generation.
- At create the daemon also writes the association tree into authoritative state
  (`prdState.docPath`, `drStates[docId]` entries with nested `storyStates`,
  root-level `storyStates`, cross-item `parentPrdId`/`parentDrId`) and writes the
  initial phase directly, outside TransitionPolicy: the deepest provided
  document's post-generation phase (`dr-generated`, or `story-generated` when a
  STORY entry node provides the story; PRD-only keeps `initialized`). Flow
  therefore resumes at review, not generation.
- `flow.snapshot` and the context projection add a derived `documentTree` field,
  computed at projection time from `prdState`/`drStates`/`storyStates`/
  `documentPaths` and never persisted:

```json
{"prd":{"docId","docPath","phase"}|null,"drs":[{"drId","docPath","phase","stories":[{"storyId","docPath","phase"}]}],"stories":[root-level stories, same shape]}
```

The document association tree in the status table is authoritatively provided by
this daemon projection; the Agent must not scan project files to assemble one
locally.

## Declared Routes

The Agent supplies intent and available facts; `FlowRuntime` selects and advances
the route. Requirement Analysis is the *first* business Series for every task,
including self-update: the Hook records only a provisional `BootstrapAssessment`,
and RA produces exactly one adaptive `ae-sdd-ra-srs/v2` SRS. `G-RA-1..4` close
the content receipt at `RequirementAnalyzed`; `G-RA-FLOW-VIOLATION` validates
the SRS/receipt/scale/candidate/approval binding before the authoritative
`EngineeringRoute` is frozen at `RouteSelected`. `G-RA-5/6` remain real
compatibility diagnostics but are not automatic transition Gates. The current
methodology declares these minimum routes:

| Size | Required design chain before Coding |
| --- | --- |
| large | RA -> DR -> N x (Story -> TestCase -> CodingPlan) -> approved `executionPlan` |
| medium | RA -> Story -> TestCase -> CodingPlan -> approved `executionPlan` |
| small | RA -> CodingPlan -> approved `executionPlan` |
| micro | RA -> approved compact `state.executionPlan` |

Wherever a Story exists, every Story runs its own independent
`Story -> TestCase -> CodingPlan` subchain and its TestCase receipt binds that
Story's identity — a sibling's TestCase never satisfies it. TestCase is neither
optional nor conditional on matrix complexity, and it does not appear on the
micro or small routes, which have no Story. Micro creates no separate CodingPlan
Markdown; it uses the approved `state.executionPlan` alone. Only the user can
approve `executionPlan`.

> Terminology and route semantics follow `source/docs/ae-sdd-design.md` §2 and
> §过程产物模型; this file only states how an Agent drives them.

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
