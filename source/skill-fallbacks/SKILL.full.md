---
name: ae-sdd
version: 4.0.0
description: Full declarative contract for the Rust ae-sdd runtime.
declarative_only: true
---

# ae-sdd Full Declarative Contract

## Boundary

`ae-sddd` owns the live process. `FlowRuntime` reduces committed state and events
to one deterministic `nextAction`; `WorkItemActor` is the only project mutation
owner. This file supplies semantic input/output contracts to an Agent. It does
not contain an executable phase loop, local Gate logic, state repair routine, or
fallback implementation.

The following are always invalid:

- inferring a phase transition from prose;
- running a repository script when the daemon is unavailable;
- treating a Host ACK as a physical child claim;
- collecting a child transcript into the root session;
- treating `ERROR`, `TIMEOUT`, `CANCELLED`, or `STALE` as Gate failure or PASS;
- marking compact complete before correlated ACK and rehydrate;
- allowing Rust and a legacy runtime to write the same workspace.

## Identity Envelope

Every scoped request is bound to the identities applicable to the method:

| Identity | Meaning |
| --- | --- |
| `workspaceId` | canonical workspace registered under an allowed root |
| `workItemId` | explicit workflow target; never guessed from a sole candidate |
| `agentId/sessionId` | Agent instance and physical conversation |
| `turnId/turnSeq` | engaged Hook turn and monotonic sequence |
| `rootSessionId/parentSessionId/delegationId` | daemon-derived lineage |
| `revision/fencing` | optimistic state and lease authority |
| `bootId/policyDigest` | endpoint and policy freshness |
| `requestId/idempotencyKey` | durable replay identity |

The daemon rejects missing, stale, cross-workspace, cross-turn, or cross-lineage
identity. Endpoint credentials authenticate transport but cannot sign offline
capabilities; boot-scoped Ed25519 capabilities bind role, lineage, paths,
operations, deadline, boot, and policy.

## Method Semantics

### Runtime and workspace

- `runtime.handshake` is the first RPC on a connection. It compares the endpoint
  token plus expected boot/policy values and returns negotiated protocol, limits,
  operation schema digest, build, and capability public key.
- `workspace.register` canonicalizes the requested root, enforces containment,
  and returns `legacy`, `shadow`, `rust-canary`, or `rust-sole-writer` mode.
- `workspace.snapshot` is authoritative after a cursor gap, watcher overflow, or
  stale context revision.
- `runtime.drain` stops new work, waits for bounded in-flight work, and is required
  before writer-mode changes, upgrade, stop, or rollback.

### Session and Hooks

- `session.open/heartbeat/close` maintain a physical session lifecycle.
- `hook.user_prompt/pre_tool/post_tool/stop` are idempotent by `hookEventId` and
  return `allow`, `deny`, `block`, or `context` with an optional `nextAction`.
- An engaged Hook fails closed within its host deadline. The adapter performs no
  document scan, Gate execution, child spawn, compact wait, or state mutation on
  the synchronous path.
- A valid offline capability may prove that a turn is not engaged. It can never
  authorize a mutation or mint another capability.

### Flow and operations

- `flow.snapshot` returns the committed process projection.
- `flow.next` returns a typed action derived from state revision, ordered event
  cursor, policy digest, input fingerprint, role, and lineage.
- `operation.describe` exposes operation scope and required preconditions.
- `operation.execute` validates workspace/work-item/role, schema, confirmation,
  lease, revision, fencing, idempotency, and fresh Gate evidence before commit.
- `lease.acquire` and `lease.break` bootstrap without an existing lease but still
  require lock, fencing, tombstone, reason, and audit protection.
- `gate.evaluate` returns one of `PASS`, `FAIL`, `ERROR`, `TIMEOUT`, `CANCELLED`,
  or `STALE`. Only fresh PASS can authorize a dependent mutation. Only fresh FAIL
  increments business correction.

### Delegation

`delegation.create` returns a scoped deliverable contract and one-time claim.
The host receives an action through `host.action_next`; `host.action_ack` means
only that the native command was accepted. A child becomes running only after it
opens a distinct physical session and calls `delegation.accept` with trusted host
identity. Unsupported physical spawn/attestation fails closed.

Lifecycle:

```text
requested -> spawning -> running -> result-staged
          -> artifacts-validated -> memory-cleaned -> completed
```

Failure terminals are `failed`, `expired`, `cancelled`, and `orphaned`. Durable
receipts prevent replay from spawning, reporting, collecting, or cleaning twice.

`delegation.report` stages a bounded result. The runtime validates envelope size,
summary size, paths, hashes, kinds, deliverable presence, and role memory scope.
`delegation.collect` returns only after artifact-validation and memory-cleanup
receipts commit.

### Context, pressure, and compact

`context.get/project` returns full, delta, or no-change projections under role and
generation budgets. Root receives summaries, finding counts, next actions, and
artifact indexes only.

`host.pressure_report` accepts authenticated host token telemetry bound to
session, generation, and monotonic sample sequence. Default policy uses two
consecutive samples, 800/600 permille hysteresis, and a 300-second cooldown.

Compact lifecycle:

```text
pressure-detected -> snapshot-ready -> compact-requested -> host-compacting
                  -> host-acknowledged -> context-restored
```

Wrong session/generation, timeout, unsupported host, or rehydrate failure cannot
advance context generation.

## Root Orchestration

The root session repeatedly consumes typed `nextAction` values and performs only
the matching orchestration command. Semantic series work is delegated to a
physical series session. A series session may delegate task or reviewer work;
depth-2 sessions may not delegate.

The root may retain:

- current flow/state digest and legal next action;
- child status and bounded summary;
- finding counts and typed finding summaries;
- artifact path/hash/kind references;
- user decisions and committed receipts.

The root must not retain child transcripts, raw source bundles, unbounded test
logs, child scratch memory, or another role's projection.

## Route And Document Contract

Route always precedes Requirement Analysis. Analysis determines whether the
design continues through DR, Story, or directly to a compact execution plan.

| Route | Core design requirement |
| --- | --- |
| large | RA plus DR; Story when behavior/data/API contracts require it |
| medium | RA plus Story |
| small/micro | RA plus Story-lite or CodingPlan recorded in `executionPlan` |

RA, DR, and Story use templates resolved by the runtime. Story contains
contracts, fields, main flow, data model, acceptance criteria, and an AC-to-test
verification matrix. A separate TestCase exists only for a genuinely complex
matrix. The user approves the compact `executionPlan` before Coding.

Tests persist real evidence. Review persists only `status` and `findings`.
Proposal, CodingReport, TestReport, CodeReview report, and changelog workflow
files are retired outputs.

## Fail-Closed Result Contract

| Condition | Result |
| --- | --- |
| daemon unavailable | CLI nonzero; engaged Hook deny/block |
| endpoint boot/policy mismatch | `ENDPOINT_STALE` and reconnect |
| protocol major mismatch | `PROTOCOL_VERSION_UNSUPPORTED` |
| stale lease/revision/fencing | reject with current state summary |
| replay key with changed payload | `IDEMPOTENCY_KEY_REUSED` |
| invalid child identity/result | attestation/result error; parent does not advance |
| cursor gap or watcher overflow | full snapshot/reconciliation |
| compact unsupported | explicit manual/rotate remediation |

There is no local business fallback.

## Resource Index

- `standards/**`: current methodology and project contracts
- `templates/**`: canonical document and evidence shapes
- `skills/**`: phase-specific declarative input/output rules
- `skills/cross-cutting/memory-management-skill.md`: role-aware projection
  and memory lifecycle contract
- `skill-fallbacks/runtime/service-lifecycle-contract.md`: generated OS
  service descriptor and lifecycle contract
- `skill-fallbacks/runtime/cutover-contract.md`: shadow/canary/cutover and
  rollback contract
