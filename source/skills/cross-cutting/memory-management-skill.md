---
name: memory-management
description: Declarative role-aware memory, context projection, and compact contract for ae-sddd.
declarative_only: true
---

# Memory And Context Contract

Memory lifecycle is owned by the Rust daemon. An Agent never creates a local
memory directory, compiles source files into prompt memory, or deletes another
session's memory directly.

## Namespace

Every memory record and projection is keyed by:

```text
workspaceId / workItemId / rootSessionId / delegationId / role / contextGeneration
```

The daemon derives role and lineage from an attested session. Request fields that
claim another role, parent, sibling, workspace, or generation are untrusted and
must be rejected.

## Projection Contract

| Consumer | Allowed projection |
| --- | --- |
| root | flow digest, legal next action, child status, bounded summaries, finding counts, artifact indexes, receipts |
| series | scoped methodology, upstream artifact references, series state, deliverable contract, child summaries |
| task | allowed paths, task contract, required checks, task-local findings |
| reviewer | immutable review inputs, artifact references, verification contract; no worker scratch memory |

`context.get` returns a bounded full projection. `context.project` takes the last
`contextRevision` and digest and returns full, delta, or no-change. A stale or
unauthorized request never receives an old or broader projection.

Budgets:

- root projection: at most 64 KiB;
- ChildResult envelope: at most 64 KiB;
- child summary: at most 8 KiB;
- oversized logs, source bundles, transcripts, and evidence bodies: artifact
  reference only.

## Delegation Memory Lifecycle

1. `delegation.create` allocates a scoped namespace and deliverable contract.
2. The attested child receives its role-aware projection after
   `delegation.accept` and `session.open`.
3. The child reports a bounded result. The daemon validates artifact paths,
   hashes, kinds, required deliverables, and memory scope.
4. The daemon cleans temporary child memory and commits a durable
   `MemoryCleanupReceipt`.
5. Only then may `delegation.collect` expose the bounded result to the parent.

Cleanup replay returns the existing receipt and never deletes twice. A cleanup
failure leaves the delegation before `memory-cleaned`; the parent cannot collect
or advance.

## Compact Contract

Only authenticated `host.pressure_report` token telemetry may trigger an active
compact policy. Context bytes are not a token-pressure substitute. The default
policy requires two consecutive threshold samples, uses 800/600 permille
hysteresis, and enforces a 300-second cooldown.

Compact advances through durable snapshot, native host request, correlated ACK,
and projection rehydrate. `contextGeneration` changes only after successful
rehydrate. Unsupported hosts return explicit manual/rotate remediation; no
physical compact or ACK is inferred from an Agent message.

## Prohibitions

- No root import of child transcript, raw source bundle, scratch memory, or
  unbounded command output.
- No cross-role, cross-lineage, cross-workspace, or stale-generation reads.
- No local `.ae-sdd/memory` fallback when the daemon is unavailable.
- No prompt-only compact completion and no projection-based token estimation.
- No parent collection before artifact-validation and memory-cleanup receipts.

The expanded schema and failure semantics are in
`skill-fallbacks/skills/cross-cutting/memory-management-skill.full.md`.
