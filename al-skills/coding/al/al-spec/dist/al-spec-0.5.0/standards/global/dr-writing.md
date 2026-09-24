# DR Writing Reference

> Maintainer/provenance reference only. Runtime DR rules live in
> `skills/dr/SKILL.md`; do not require this file when invoking the SKILL.

The DR records a technical design for a capability. It is a design document,
not a workflow recipe. The rules below are adapted from the useful design
content of the former `ae-sdd` DR material; Phase routing, `ae-sdd` commands,
document-storage APIs, iteration state, reviewer orchestration, and gate
bookkeeping are intentionally excluded.

## Required Content

- design goal, scope, non-goals, and evidence inspected;
- constraints and their source paths;
- component responsibilities and boundaries;
- non-obvious decisions with alternatives, rationale, and accepted cost;
- data model, ownership, lifecycle, invariants, and state transitions;
- interfaces/events, payloads, errors, compatibility, and idempotency;
- failure modes, security, audit, observability, migration, and rollback;
- risks, open questions, and optional related-document notes.

## Design Rules

- Explain boundaries in terms of responsibility and ownership, not only boxes
  on a diagram.
- Every non-trivial decision records at least one rejected alternative and the
  cost accepted by choosing the selected option.
- State transitions describe allowed triggers and forbidden transitions. Do not
  expose a generic "set target status" operation as the business contract.
- Data tables identify keys, uniqueness, lifecycle, retention, and consistency
  expectations. Explain non-obvious denormalization or indexes.
- Interface rows include direction, request/response shape, validation, error
  semantics, timeout/retry behavior, and compatibility policy.
- Failure analysis covers dependency timeout/unavailability, duplicate delivery,
  partial success, and recovery or manual intervention.
- Story decomposition is an optional design aid, not a dependency on a Story
  document or a requirement to create one.

## Review Checklist

- Can each component's responsibility and non-responsibility be stated?
- Are architecture decisions reproducible from evidence rather than preference?
- Are data ownership and transaction/consistency boundaries explicit?
- Could an implementer define the contract without guessing missing error or
  compatibility behavior?
- Are risks and unknowns separated from settled decisions?
