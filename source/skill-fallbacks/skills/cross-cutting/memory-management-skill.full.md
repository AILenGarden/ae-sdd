---
name: memory-management
description: Full declarative contract for role-aware daemon memory and context projection.
declarative_only: true
---

# Memory Management Full Contract

## Ownership

`ae-sddd` owns memory namespaces, projection caches, revisions, cleanup receipts,
pressure samples, and compact cycles. The methodology package defines what each
role may see; it does not implement storage or ask the Agent to manage local
compact files.

## Stored Records

| Record | Required identity | Purpose |
| --- | --- | --- |
| memory namespace | workspace/work-item/root/delegation/role | isolate child working context |
| context projection | projection ID/revision/digest/generation/role | bounded injection payload |
| cleanup receipt | delegation/namespace/input digest/receipt ID | prove idempotent temporary-memory cleanup |
| pressure sample | host/session/generation/sample sequence | authenticated token pressure input |
| compact cycle | compact/session/generation/action/ACK/snapshot | durable compact lifecycle |

SQLite may index these records, but project state and the project-authoritative
mutation journal remain authoritative for project mutations. Secrets, prompts,
transcripts, and claim tokens do not enter ordinary logs.

## Isolation Rules

- Role and lineage come from a daemon-verified capability and physical session
  attestation, never from a payload enum alone.
- A series sees only its series namespace and bounded descendants.
- A task or reviewer cannot enumerate sibling namespaces or delegate again.
- A reviewer receives immutable inputs and may not reuse the worker's scratch
  context as review evidence.
- A root receives only validated summaries and references. The root has no API to
  retrieve a child transcript.
- A stale context generation or revision produces refresh/deny, never an old
  delta.

## Projection Shape

A projection contains only fields needed for the consumer's current typed
action:

```json
{
  "projectionId": "uuid",
  "contextRevision": 1,
  "contextGeneration": 1,
  "digest": "sha256",
  "mode": "full|delta|no-change",
  "role": "root|series|task|reviewer",
  "flow": {},
  "methodologyRefs": [],
  "artifactRefs": [],
  "childResults": [],
  "nextAction": {}
}
```

The runtime validates byte, depth, string, and collection limits before a Hook
receives the payload. When a value exceeds budget, the producer emits an
artifact reference or rejects the result; silent truncation cannot change
meaning.

## Child Result And Cleanup

The following ordering is mandatory:

```text
result-staged -> artifacts-validated -> memory-cleaned -> completed
```

`delegation.report` validates summary <= 8 KiB and envelope <= 64 KiB, artifact
containment and sha256, declared kinds, required deliverables, finding bounds,
and the child's namespace. Validation failures keep the result staged and return
a stable error.

Memory cleanup is a durable idempotent mutation. Its receipt binds namespace,
delegation, canonical input digest, actor, timestamp, and resulting projection
digest. Reusing an idempotency key with changed input is rejected. Restart replay
returns the original receipt.

`delegation.collect` succeeds only when both the artifact validation receipt and
memory cleanup receipt are committed. It exposes summary, finding statistics,
next actions, and artifact path/hash/kind references; it cannot expose scratch
memory or transcripts.

## Pressure Policy

`host.pressure_report` requires a registered host adapter, authenticated session,
matching generation, monotonic sample sequence, observed token count/capacity,
and sample timestamp. Defaults:

| Parameter | Value |
| --- | --- |
| high threshold | 800 permille |
| low/re-arm threshold | 600 permille |
| consecutive samples | 2 |
| cooldown | 300 seconds |

Duplicate, stale, unauthenticated, wrong-session, or wrong-generation samples do
not trigger. Hosts without trusted telemetry report `unknown`; projection bytes
must not be converted into a fake token percentage.

## Compact State Machine

| State | Required evidence to advance |
| --- | --- |
| pressure-detected | policy decision and durable current-generation snapshot |
| snapshot-ready | host action dispatch receipt |
| compact-requested | registered host begins native compact |
| host-compacting | correlated native ACK/PostCompact event |
| host-acknowledged | role-aware projection rehydrate succeeds |
| context-restored | generation CAS and restored projection digest commit |

An ACK binds compact ID, host action ID, session ID, and generation. A matching
ACK does not itself advance generation. Timeout becomes `timed-out`; unsupported
host becomes `unsupported`; invalid ACK or rehydrate becomes `failed`. Manual or
session-rotate remediation is explicit and never recorded as a false native
compact success.

## Agent Use

The Agent asks for `context.get` or `context.project`, consumes the returned
bounded projection, reports a bounded ChildResult, and follows the next typed
action. It never constructs a memory path, reads another role's files, deletes a
directory, or claims compact completion in prose.
