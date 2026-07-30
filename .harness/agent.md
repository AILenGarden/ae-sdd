# ae-sdd Agent Harness

<!-- ae-sdd:harness-source path="//?/D:/Item/ae-sdd/source/SKILL.md" sha256="8ee76295c3e7caccc89ea20a9bc2ee5cda8d5fe3e7e9042c44576a03732a6b4d" -->
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

<!-- ae-sdd:harness-source path="//?/D:/Item/ae-sdd/source/HARNESS.md" sha256="9c6c73e51c118d6bc05ad57c8e1dd820416f1b77d046a7f290bb496b5cd9a829" -->
# ae-sdd Agent Harness

This Harness connects an Agent host to the Rust ae-sdd daemon. It does not embed
the workflow executor. `FlowRuntime` is the sole process owner.

## Runtime Boundary

- Invoke the installed Rust `ae-sdd` binary only.
- Never run repository scripts or evaluate Gate/state logic in the Agent process.
- Never read or mutate daemon endpoint credentials directly; the Rust client
  performs manifest, handshake, boot, policy, and capability validation.
- If the daemon is unavailable, an engaged session stops. There is no local
  business fallback.

## Hook Mapping

| Host event | Rust client command |
| --- | --- |
| SessionStart (when the host supports it) | `ae-sdd runtime ensure --quiet` |
| UserPromptSubmit | `ae-sdd hook --method hook.user_prompt --request-json -` |
| PreToolUse | `ae-sdd hook --method hook.pre_tool --request-json -` |
| PostToolUse | `ae-sdd hook --method hook.post_tool --request-json -` |
| Stop | `ae-sdd hook --method hook.stop --request-json -` |

`SessionStart` is an eager prewarm, not a correctness dependency. Every
daemon-bound CLI command and each trusted, session-bound Hook still performs its
own call-first/recover-once path: if the endpoint is missing or the local
transport is unavailable, the Rust client ensures the daemon is ready and then
replays the original request once. Hosts without `SessionStart`, and sessions
in which the daemon exits after startup, therefore use the same recovery path.

A Hook runs as a separate host subprocess and can never export a binding back to
its parent, so `SessionStart` cannot hand one to later events. Each Hook instead
rebuilds its own binding from the host event: `sessionId` becomes the session
external key and `cwd` the workspace project root, then `workspace.register` and
`session.open` — both idempotent — return the workspace, session, and capability.
`engaged` is derived from the registered workspace mode, never chosen by the
client. `turnId` and `workItemId` are deliberately not synthesized: the turn is a
durable monotonic sequence only the daemon can allocate, and the Work Item is a
routing decision, so the daemon allocates the former and resolves the latter from
the session binding. A host event that carries no `sessionId`, or whose binding
cannot be created, stays fail-closed and reports the exact cause on stderr.

A fresh `/ae-sdd` session therefore starts unbound: the first Hook self-binds
workspace and session but carries no Work Item, and its response deliberately
reports no `workItemId`. The bootstrap branch belongs to the Agent, not the
Hook: the Agent calls `operation.execute` with `workitem.create`
(`{entryNode, idempotencyKey}`, no `workItemId`; `entryNode` must be PRD, DR,
or STORY, and when the caller omits the name the daemon mints
`{entryNode}-{8 lowercase hex}`, e.g. `STORY-3f9a2c1e`). On success the daemon
durably binds `session.current_work_item`, installs the project context
projection, and returns `data.workItemId` as the resolvable business key
(`data.stateMachineId` is the uuid-prefixed directory identity). Later Hooks
then attribute automatically and their responses carry `workItemId`;
`flow.next`, `flow.snapshot`, and `context.get` proceed with the returned key.
A client must never invent or send a `workItemId` when re-opening a session —
omitting the field always adopts the binding.

When the caller already has its own PRD/DR/Story documents, the same
`workitem.create` carries an optional `providedDocuments` array (at most 64
entries of `{"intent","docId","path","parentDocId"?}`; project-relative paths to
existing files only, `..`/absolute paths rejected). The daemon adopts them
register-only: it never reads, writes, or copies the user file and never mints a
default document for an adopted intent. It records `documentPaths[intent]` and
`routeDocuments[intent]=true` so the handoff skips generation for that series,
builds the PRD→DR→Story association tree in authoritative state (`prdState`,
`drStates` with nested `storyStates`, root-level `storyStates`, cross-item
`parentPrdId`/`parentDrId`), and writes the initial phase directly at create —
the deepest provided document's post-generation phase — so `flow.next` enters
review instead of generation. Without `providedDocuments`, intake is unchanged.

Binding inputs may also be injected directly through `AE_SDD_WORKSPACE_ID`,
`AE_SDD_AGENT_ID`, `AE_SDD_SESSION_ID`, `AE_SDD_CAPABILITY_TOKEN`,
`AE_SDD_TURN_ID`, and `AE_SDD_WORK_ITEM_ID`. A complete set takes precedence over
self-binding; an incomplete set falls back to it.

A per-user daemon that serves several workspaces must receive their trusted
parent roots through `AE_SDD_ALLOWED_ROOTS` before its first bootstrap (a normal
OS path list; `;` on Windows). If it is unset, bootstrap admits the current
workspace only and later clients cannot silently widen that security boundary.

`-` means one JSON request on stdin. The wrapper is:

```json
{
  "params": {},
  "engaged": true,
  "offlineCapability": "optional-ed25519-capability",
  "nowUnixMs": 0
}
```

The Rust client writes one host-compatible Hook outcome to stdout. On an engaged
transport failure it still emits the host's deny/block JSON contract within the
Hook deadline; it does not run a local Gate.

Hosts may additionally map native lifecycle events when supported:

| Host capability | Runtime contract |
| --- | --- |
| SessionStart | prewarm with `runtime ensure --quiet`; each Hook self-binds its typed session from the host event |
| SubagentStart | `delegation.accept`, then `session.open` |
| SubagentStop | `delegation.report`, then `session.close` |
| PreCompact | `compact.request` |
| PostCompact | `context.project`; native ACK is a separate `host.action_ack` |

Unsupported events remain explicitly unsupported. The adapter must not invent a
physical claim or compact ACK.

On Windows the daemon listens on a per-user Named Pipe described by the protected
endpoint manifest. It does not bind a TCP/HTTP port, so no daemon port setting or
firewall rule is required. A current-user Scheduled Task may run `ae-sdd runtime
ensure` at sign-in as an optional latency prewarm; Hooks and CLI recovery remain
the authoritative startup mechanism.

## Agent Roles

| Role | Responsibility | Context allowed |
| --- | --- | --- |
| root | advance typed actions, delegate, collect, ask user, report | flow projection plus bounded child results |
| series | own one RA/DR/Story/Coding/Test/Review series | series projection and scoped sources |
| task | implement one scoped task | task projection and allowed paths |
| reviewer | independently review scoped artifacts | review projection, no worker scratch memory |

The lineage is `root -> series -> task|reviewer`; maximum depth is 2.

## Root Main Session

The main session must stay small:

1. Read the Hook-provided context or request `flow.next`.
2. Execute only the returned orchestration action.
3. Use `delegation.create` for semantic series work.
4. Wait for physical host attestation, not a logical prompt-only child.
5. Use `delegation.collect` and retain only the validated bounded result.
6. Report progress/result to the user and request the next runtime action.

The main session never imports a child transcript, raw source bundle, unbounded
test output, or child memory. It may retain summaries (8 KiB maximum), finding
counts, artifact path/hash/kind references, receipts, and next actions. Only root
may request a global transition, and `FlowRuntime` still decides whether it is
legal.

## Codex HostAdapter Lifecycle

Registration originates from the Codex host surface; the daemon never
auto-registers an adapter and never simulates the native lifecycle. After every
daemon boot or endpoint rotation, the Codex host handshakes against the current
endpoint manifest and calls tokenless `host.register` with `adapterId=codex`
and only truthful capabilities — the implemented slice advertises exactly
`create` and `attest`; a capability the native surface cannot perform is
omitted, never advertised. The caller omits `capabilityToken`: the client binds
the boot-scoped endpoint credential from the manifest in memory and discards
any supplied value, so the credential never appears in argv, stdin JSON,
stdout, stderr, logs, or evidence.

Every follow-up HostAdapter RPC replays registration on the same connection:
`--host-register-json` carries the register params and the client performs
`call_after(host.register, target)`, because adapter identity is bound to the
physical connection, not to request payload. `host.action_next` and
`host.action_ack` therefore run without the caller ever holding the endpoint
credential.

A `create` action maps to a genuinely independent Codex Agent session. The host
sends `host.action_ack` with the exact `actionId`, `commandSeq`, `hostTaskId`,
and child `sessionId` only after the native host accepted the action; the
one-time claim is delivered to the child, which calls `delegation.accept` and
then `session.open` with the accepted identities.

Registration is a boot-scoped capability declaration, not evidence of physical
work. An ACK without the child claim never advances a delegation to running.
Boot rotation invalidates prior authority, so the host re-handshakes and
re-registers before consuming any new action.

## Process Contract

Route precedes Requirement Analysis. RA then selects DR, Story, or compact
`executionPlan` depth. Coding begins only after required upstream documents,
verification mapping, blocker Gates, and explicit user plan approval are
committed. Testing records real evidence. Review records status/findings only.

Documents the user already supplied take the adoption path instead of the
generation path: `workitem.create.providedDocuments` registers them at intake,
`routeDocuments[intent]=true` skips that series' generation, and the Work Item
resumes at the deepest provided document's post-generation phase. The document
association tree shown in status output is consumed from the daemon's derived
`documentTree` projection on `flow.snapshot` and the context projection; neither
Hook nor Agent scans project files to assemble a tree locally.

At any disagreement, daemon state and typed errors override this Harness. Method,
template, and output details are declared in the package-root `SKILL.md`.

