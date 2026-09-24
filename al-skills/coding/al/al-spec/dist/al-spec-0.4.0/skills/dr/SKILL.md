---
name: dr
description: Write or review an independent DR. This SKILL is the authoritative DR writing specification; load its required template as context.
metadata:
  display_name: DR
  version: 0.3.0
  role: specification
  spec_type: dr
  scope: global
---

# DR writing specification

Use this SKILL for a Design Requirements document. This SKILL is the complete,
standalone writing specification. A DR does not require another specification,
`ae-sdd`, or workflow; related documents are optional context only.

## Required context

Load the mandatory template [`../../templates/global/dr.md`](../../templates/global/dr.md)
before drafting or materially reviewing. The template supplies the output shape;
this SKILL supplies the normative writing rules. With a confirmed project ID,
inspect the registry and load the exact project DR SKILL and project template it
names after the global template. Record paths and reasons. `standards/` files
are maintainer references and are not runtime prerequisites. Do not import
`ae-sdd` phases, storage APIs, iteration state, reviewer loops, gates, or
automatic handoffs.

## Document contract, evidence, and language

Start with `spec_id`, `spec_type: dr`, `status`, `version`, `owner`, `project_id`,
and `updated`. Use stable `DEC-*`, `REQ-*`, `AC-*`, and `Q-*` IDs; cite evidence
for non-obvious facts and keep Fact, Decision, Assumption, Risk, and Open
question distinct. `MUST`/`MUST NOT` are binding, `SHOULD`/`SHOULD NOT` require a
deviation reason, and `MAY` is optional. Every normative statement names an
actor, condition, and observable result.

## Ownership and authoring rules

The DR owns design goal/scope, component responsibilities and boundaries,
constraints, architecture decisions, data ownership/model, state transitions,
contracts, failure/security/operations, migration, rollback, risks, and design
acceptance. It MUST NOT become user-story prose, exhaustive executable test
steps, or implementation file order.

- Describe each component's responsibility and non-responsibility in terms of
  ownership. Distinguish protocol adaptation, application orchestration,
  domain rules, persistence, and external adapters.
- For each non-trivial decision record context, constraints, options, selected
  choice, rejected alternatives, rationale, accepted cost/risk, owner, and
  evidence. Decisions must be reproducible from facts, not preference alone.
- Define data keys, uniqueness, lifecycle/retention, invariants, ownership,
  transactions, consistency, and allowed plus forbidden state transitions.
- Define each interface/event's direction, request/response, validation,
  authentication, errors, compatibility/versioning, idempotency, timeout,
  retry, rate limits, and observability when applicable.
- Analyze dependency timeout/unavailability, duplicate delivery, partial
  success, recovery/manual intervention, security, audit, observability,
  migration, rollback, and capacity consequences.
- Story decomposition, TestCase notes, and implementation hints are optional
  design information; they never create a document dependency.

## Required review gate

Before `ready`, verify that the SKILL and mandatory template are recorded;
scope/non-goals and owners are explicit; decisions include alternatives and
accepted costs; data/contracts/failures are implementable without guessing;
IDs, evidence, assumptions, contradictions, and open questions are visible;
acceptance and verification actions are observable; and the intended consumer
and output are clear. A design-changing unknown blocks `ready`.

## Review output and changes

Report unsupported decisions, leaked boundaries, unowned data, missing failure
behavior, incompatible or ambiguous contracts, untestable claims, and
design-changing questions. Record intentional deviations in a `DEC-*` with
impact, owner, effective version, and migration/rollback consequence.
