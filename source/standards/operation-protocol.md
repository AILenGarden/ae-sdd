# AE-SDD Typed Operation Protocol

## 1. Purpose

This protocol is the machine-facing mutation boundary for LLM agents. Agents MUST discover and execute registered operations instead of editing `state.json`, lease files, evidence manifests, or generated document indexes directly.

The protocol is transport-independent. The CLI is the first transport:

```text
ae-sdd ops describe [--operation <NAME>] --json
ae-sdd ops next --project <ROOT> --work-item <ID> [--story <ID>] --json
ae-sdd ops execute --request-file <REQUEST.json> --json
ae-sdd lease acquire|status|renew|release|break ... --json
```

## 2. Required Agent Sequence

1. Call `ops describe` once per runtime version and use the returned JSON Schema.
2. Call `ops next` for the explicit Work Item.
3. For a write, call `lease.acquire` and retain `leaseId`, `fencingToken`, `expiresAt`, and the current state revision.
4. Execute a registered operation with `expectedRevision` and a unique `idempotencyKey`.
5. On `REVISION_CONFLICT`, reread state and recompute the request. Never retry a stale payload blindly.
6. Renew before expiry during long work, then release when the write series is complete.

Raw patch operations are forbidden. In particular, `state.patch` is intentionally not registered.

## 3. Request Envelope

```json
{
  "schemaVersion": "1",
  "operation": "state.transition",
  "project": "D:/Item/project",
  "projectKey": "project-key",
  "workItem": "BUG-001",
  "story": "STORY-001",
  "lease": {
    "leaseId": "lease-id",
    "fencingToken": 2
  },
  "expectedRevision": 4,
  "idempotencyKey": "session-123:transition:coding",
  "dryRun": false,
  "parameters": {
    "targetPhase": "coding"
  }
}
```

Identity is explicit: `project` and `workItem` are mandatory. `story` is a relation and MUST NOT replace Work Item scope. All lease-protected writes require `lease.leaseId`, `lease.fencingToken`, `expectedRevision`, and `idempotencyKey`.

## 4. Registered Operations

| Operation | Write | Lease | Purpose |
| --- | --- | --- | --- |
| `workitem.get` | no | no | Read canonical Work Item state and revision |
| `state.next_actions` | no | no | Return legal typed follow-up operations |
| `lease.acquire` | yes | no | Acquire or take over an expired writer lease |
| `lease.renew` | yes | yes | Extend the current lease without changing its token |
| `lease.status` | no | no | Read holder, expiry, and fencing token |
| `lease.release` | yes | yes | Release an owned lease |
| `lease.break` | yes | admin action | Break a lease with actor and reason audit |
| `state.transition` | yes | yes | Perform one legal phase transition and its gates |
| `execution.plan.set` | yes | yes | Persist the compact goal, changed paths, verification, risks, and source reads |
| `execution.plan.approve` | yes | yes | Record explicit user approval of the current compact execution plan |
| `review.record` | yes | yes | Record review status and structured findings without a Markdown report |
| `document.resolve` | no | no | Resolve a canonical Work Item-scoped document path |
| `document.save` | yes | yes | Save document content through document storage |
| `gate.check` | no | no | Evaluate named gates against explicit Work Item state |
| `verification.plan` | yes | yes | Build and optionally persist a Work Item-bound plan |
| `evidence.record` | yes | yes | Snapshot an artifact and append active evidence |
| `evidence.finalize` | yes | yes | Validate active snapshots and seal the manifest |
| `workitem.complete` | yes | yes | Run completion gates and enter `completed` |

`ops describe` is the schema authority. This table is an orientation index, not a substitute for runtime discovery.

## 5. Lease and CAS Semantics

- A Work Item has at most one active writer lease.
- Default TTL is 300 seconds; legal bounds are 30 to 3600 seconds.
- `expiresAt <= now` means expired. A new owner may take over and receives a strictly larger fencing token.
- Every state mutation compares `expectedRevision` with the current revision under the Work Item lock.
- Stale fencing is checked before revision mismatch so an obsolete owner cannot be mistaken for an ordinary retry.
- State persistence uses a unique same-directory temporary file, flush/fsync, and atomic replace.
- A repeated idempotency key with the same canonical payload returns the stored response. Reuse with a different payload fails with `IDEMPOTENCY_KEY_REUSED`.
- `dryRun=true` validates lease, fencing, revision, schema, confirmation, and gates without writing state, lease, idempotency, document, or evidence files.

## 6. Evidence Semantics

For `scenarioPolicyVersion=1`, each HTTP verification references a project-contained `scenarioManifest`. Active `http-local` and `http-test-env` evidence summaries must cover the required scenario IDs and record, per scenario, `result=PASS`, at least one substantive assertion kind (`field/state/relation/invariant/effect/atomicity`), and a standalone `rerunCommand`. Status-only evidence cannot satisfy the contract.

`verification.plan` returns both the compatibility field `inputFingerprint` and the explicit `evidenceInputFingerprint`. Evidence operations MUST use `evidenceInputFingerprint`; `planFingerprint` identifies plan structure and is not an evidence input identity.

`evidence.record` copies each source artifact into a content-addressed immutable snapshot under the Story evidence directory. Recording the same `logicalKey` marks the previous active entry `superseded` and appends a new active entry. Finalization and gate validation ignore superseded entries and validate active snapshots only. Legacy manifests without lifecycle or snapshot fields remain readable and are not silently rewritten by reads.

HTTP acceptance uses the existing open `kind` and `summary` fields; it does not add a new operation or change the operation JSON Schema:

- `executionPlan.verification` marks interface ACs with `boundary=http`, `stages=[local,test-env]`, and `internalMocksAllowed=false`.
- required evidence kinds are `http-local` and `http-test-env`; `http-external-supplemental` is fault-injection evidence and never satisfies a required stage.
- each required summary contains `stage`, credential-free `baseUrl`, non-empty `buildId`, `acIds`, `internalMocks=false`, and `result=PASS`.
- G-09 requires both stages to use the current evidence input fingerprint and the same buildId; local must precede test-env. Missing test-env remains BLOCKED rather than PASS.

Compatibility classification for v3.11.7 is patch at the operation protocol layer: `registryVersion`, request/response schemas, lease semantics, manifest readability, and evidence persistence remain unchanged. The stricter behavior is gated by an explicitly declared HTTP verification contract.

## 7. Stable Failure Codes

| Code | Meaning | Required response |
| --- | --- | --- |
| `LEASE_CONFLICT` | another owner holds the lease | wait until expiry/release or coordinate with holder |
| `LEASE_EXPIRED` | caller lease expired | acquire a new lease and recompute |
| `STALE_FENCING_TOKEN` | a newer owner superseded this caller | discard the request; never retry it |
| `REVISION_CONFLICT` | state changed after the caller read it | reread and recompute |
| `IDEMPOTENCY_KEY_REUSED` | key was used for a different payload | generate a new semantic request key |
| `CONFIRMATION_REQUIRED` | protected transition lacks user confirmation | obtain the recorded confirmation token |
| `GATE_BLOCKED` | transition prerequisites failed | follow returned gate remediation |
| `SCOPE_AMBIGUOUS` | legacy Story fallback has multiple candidates | provide/correct explicit Work Item artifact |
| `OPERATION_NOT_REGISTERED` | operation is unknown or raw patch was attempted | call `ops describe` and choose a typed operation |
| `OPERATION_SCHEMA_INVALID` | request does not match the registered schema | repair fields/types from `ops describe` |

Corrupt state or lease JSON always fails closed and includes the absolute affected path. No operation may overwrite a corrupt file as remediation.

## 8. Compatibility Boundary

`ae-sdd state write` remains a deprecated compatibility adapter. It acquires a short lease and delegates persistence to `StateStore`, so revision and fencing audit fields still advance. New LLM integrations MUST use `ops execute`; direct state or manifest editing is unsupported.

## 9. Maintainer Change Contract

This section is the handoff contract for anyone changing the typed-operation or Work Item lease implementation. It is normative for maintenance; the current conversation is not a source of truth.

### 9.1 Authority And Truth

Use this order when a description and an implementation disagree:

1. `ae-sdd ops describe --json` is the runtime schema authority for operation names, request fields, output fields, and `registryVersion`.
2. This protocol is the normative semantic contract for leases, CAS, idempotency, scope, failure codes, and compatibility.
3. `source/docs/ae-sdd-design.md` defines user/LLM-visible capability and boundaries.
4. `source/docs/ae-sdd-implementation-architecture.md` defines module ownership and data flow.
5. `source/standards/update-graph.json` defines the mandatory change cascade; `ae-sdd update-check --affected` is the required query.
6. Tests, `update-check`, and runtime verification are release evidence, not substitutes for the contract.

If a conflict is found, stop the change, record the mismatch, repair the source of truth, and rerun the affected checks. Do not silently make prose describe an unimplemented operation.

### 9.2 Compatibility Classification

`schemaVersion` and `registryVersion` are explicit compatibility markers:

| Change | Required classification |
| --- | --- |
| Internal implementation/performance change with identical JSON and behavior | patch |
| New operation, optional request/response field, or additive error code that old clients can ignore | minor |
| Removing/renaming an operation, making an optional field required, changing a field type, changing default semantics, or changing error meaning | major |
| Incompatible state/lease/evidence persistence format | major plus a migration or compatibility adapter |

The version classification must be recorded in this protocol and covered by a compatibility test. A version bump without a contract/test update is incomplete.

### 9.3 Operation Admission

An operation may be registered only when it represents a stable atomic intent rather than a generic patch. The implementation must provide:

- a strict JSON Schema with `additionalProperties: false`;
- a stable error code and actionable remediation;
- explicit Work Item scope and path containment checks;
- lease/fencing/revision/idempotency preconditions for writes;
- dry-run behavior or an explicit reason why dry-run is not applicable;
- `nextActions` guidance for the normal follow-up path;
- unit, CLI, failure, retry, and concurrency coverage proportional to the blast radius;
- a documented adapter that reuses existing domain/gate/document/evidence logic instead of creating a second ruleset.

`state.patch`, arbitrary JSON Pointer writes, silent fallback to a different Work Item, and blind retry of a stale mutation are permanent non-goals.

### 9.4 Required Change Set

Before editing a typed operation, lease, scope resolver, or evidence adapter:

1. Read this section and run `ae-sdd ops describe --json` once for the current registry version.
2. Query `ae-sdd update-check --affected <changed-files>` and treat every `UG-27` affected item as an explicit checklist.
3. Update the capability design when user/LLM semantics or boundaries change.
4. Update the implementation architecture when module ownership, persistence, locking, hooks, build, or data flow changes.
5. Update `ae-sdd-update`'s full fallback source so future maintainers can discover the rule through the self-update route; regenerate the slim entry and compiled runtime.
6. Update the registry, protocol, tests, update graph, README/version markers, and current design facts as indicated by the affected set. Never write a changelog. Do not hand-edit `dist/` or installed runtime copies.

### 9.5 Definition Of Done

A typed-operation iteration is complete only when all of the following are true:

- Story has AC and verification scenarios, and `state.executionPlan` has implementation mapping plus user confirmation;
- focused tests cover positive, invalid, stale, retry, corruption, scope, and concurrency behavior;
- `ae-sdd update-check --affected <changed-files> --json` and the required full checks pass;
- `cargo test --workspace --locked` passes when shared infrastructure changed (named exception to the incremental-testing rule; the full suite otherwise runs only at release/distribution gates);
- the `ae-sdd-build` compile job and `ae-sdd runtime verify --json` pass;
- `ops describe` from the built runtime matches the source registry;
- this protocol records compatibility classification, while tests and runtime verification record executable evidence;
- no untracked implementation module, stale design header, ghost command, or unresolved `iteration-check` blocker remains.

The maintainer must leave a concrete command, exit code, test statistic, or blocker for every checklist item. "Documented in the conversation" is not evidence.
