# ae-sdd Agent Harness

<!-- ae-sdd:harness-source path="//?/D:/Item/ae-sdd/source/SKILL.md" sha256="28fc23d4c96e7f39e7751f0a263b0cd23c5cd4575460b785b9b952f242bc483f" -->
---
name: ae-sdd
version: 3.14.0
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
| Workspace | `workspace.register`, `workspace.snapshot`, `workspace.mode_transition` | Resolve a canonical workspace and perform confirmed drained writer-mode transitions |
| Session/Hook | `session.open`, `session.heartbeat`, `session.close`, `hook.user_prompt`, `hook.pre_tool`, `hook.post_tool`, `hook.stop` | Bind physical session and turn; return a host-compatible decision |
| Flow | `flow.snapshot`, `flow.next` | Return the committed process projection and legal `nextAction` |
| Delegation | `delegation.create`, `delegation.status`, `delegation.accept`, `delegation.report`, `delegation.collect`, `delegation.cancel` | Create and attest physical child work; validate bounded results |
| Operations/Gates | `operation.describe`, `operation.execute`, `gate.evaluate` | Execute typed reads or guarded mutations; non-PASS never becomes PASS |
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

## Root Session Contract

The root Agent session is an orchestrator and reporter, not a series worker.

1. Request `flow.next` and follow the returned typed action.
2. When semantic work is required, request `delegation.create`; do not simulate a
   child by changing prompts inside the root session.
3. Let the host adapter create an independent physical session. Host ACK alone is
   not proof; the child must claim the issued identity.
4. Collect only a validated bounded `ChildResult` after artifact validation and
   memory cleanup have committed.
5. Report the bounded summary, findings, artifact references, and next actions to
   the user. Do not import child transcripts or source payloads into root context.
6. Only a root transition intent may enter the guarded transition path; the
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
- Active compact may be triggered only from authenticated host token-pressure
  samples. Projection bytes are not token telemetry.
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

<!-- ae-sdd:harness-source path="//?/D:/Item/ae-sdd/source/HARNESS.md" sha256="6b4b8de5166c00f445f13857e02599bafc70a159edb3080493c9158fb0d96be4" -->
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
An unbound host event is prewarmed when possible but remains fail-closed because
there is no authenticated session/capability to send to the daemon.

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
| SessionStart | prewarm with `runtime ensure --quiet`; the first engaged turn opens the typed session |
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

## Process Contract

Route precedes Requirement Analysis. RA then selects DR, Story, or compact
`executionPlan` depth. Coding begins only after required upstream documents,
verification mapping, blocker Gates, and explicit user plan approval are
committed. Testing records real evidence. Review records status/findings only.

At any disagreement, daemon state and typed errors override this Harness. Method,
template, and output details are declared in the package-root `SKILL.md`.

