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
