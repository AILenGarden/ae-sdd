---
name: life-dr
description: Write or review an independent life-project DR. This SKILL embeds the DR and life writing specification; load its required life template as context.
metadata:
  display_name: life DR
  version: 0.3.0
  role: project-specification
  spec_type: dr
  scope: project
  project_id: icec-cloud-life
---

# life DR writing specification

Use this SKILL only when the confirmed project ID is `icec-cloud-life` (short
name `life`). It is a complete, standalone DR writing specification: other
specifications, `ae-sdd`, and workflow are optional context and
never prerequisites. No upstream/downstream document dependency is implied.

## Required context

Load the mandatory project template
[`../../templates/project/life-dr.md`](../../templates/project/life-dr.md)
before drafting or materially reviewing. Also load the global DR template
[`../../templates/global/dr.md`](../../templates/global/dr.md) when the project
template is used as a supplement. This SKILL is the authoritative life DR
writing specification and embeds both the global DR rules and life-specific
rules; `standards/` files are maintainer references, not
runtime prerequisites. Record the confirmed project ID, loaded templates, and
their reasons. Do not import `ae-sdd` phases, RA intake, storage APIs,
iteration/memory lifecycle, review loops, gates, hooks, or handoffs.

## Document contract, evidence, and language

Start with `spec_id`, `spec_type: dr`, `scope: project`, `project_id:
icec-cloud-life`, `status`, `version`, `owner`, and `updated`. Use stable
`DEC-*`, `REQ-*`, `AC-*`, and `Q-*` IDs; cite repository/assets/command evidence
for non-obvious facts. Keep Fact, Decision, Assumption, Risk, and Open question
distinct. `MUST`/`MUST NOT` bind, `SHOULD`/`SHOULD NOT` need a deviation reason,
and `MAY` is optional; each normative statement names actor, condition, and
observable result.

## DR ownership and rules

The DR owns design goal/scope, component responsibilities and boundaries,
constraints, architecture decisions, data ownership/model, state transitions,
contracts, failure/security/operations, migration, rollback, risks, and design
acceptance. It MUST NOT become Story prose, exhaustive test steps, or file order.

- Explain responsibilities and non-responsibilities for Interfaces, Application,
  Domain, Infrastructure, BFF, Web, SPI, and API aggregates.
- Record context, constraints, options, selected choice, rejected alternatives,
  rationale, accepted cost/risk, owner, and evidence for every non-trivial
  decision. Define keys, uniqueness, lifecycle/retention, invariants,
  transactions, consistency, allowed and forbidden state transitions.
- Define interface/event direction, request/response, validation, auth, errors,
  compatibility/versioning, idempotency, timeout, retry, rate limits, and
  observability when applicable. Analyze timeout/unavailability, duplicates,
  partial success, recovery/manual intervention, security, audit, migration,
  rollback, and capacity.

## life project rules and transcription boundary

- Confirm the repository root (`D:\Item\life`), source root (`D:\Item\life\2c`),
  actual domain/module, and project asset audit date; mark unknowns rather than
  generalizing one domain's facts to all life services.
- Identify whether each component is BFF, SPI, Service, Web, API aggregate, or
  infrastructure adapter. Verify package paths, context paths, ports, Feign
  names, and `ServiceProviderConstants` from source/assets.
- Apply the boundary: Interfaces adapt/validate; Application orchestrates and
  owns transaction coordination; Domain owns rules/invariants/state;
  Infrastructure implements persistence/Feign/messaging; BFF/Web do not access
  DB/Redis/Kafka directly.
- Keep project facts, constraints, field mappings, contracts, errors, failure
  handling, security, observability, migration, and rollback. Do not copy
  `ae-sdd` phases, prerequisites, commands, storage/state protocols, review or
  user-confirmation loops, gate IDs, hooks, or automatic skill handoffs.

## Required review gate

Before `ready`, verify this SKILL, the global template, and life template are
recorded; scope/non-goals and ownership are explicit; decisions include
alternatives/cost; life module/package/contract facts have evidence; data,
state, failures, security, migration, rollback, and verification are actionable;
IDs/questions/deviations are visible; and no `ae-sdd` process language remains.
A design-changing unknown blocks `ready`.

## Review output and changes

Report unsupported life facts, wrong module paths, boundary leaks, unowned data,
missing failure/security/operations behavior, incompatible contracts, and
untestable claims. Record intentional deviations as `DEC-*` with impact, owner,
effective version, and migration/rollback consequence. Related Specs are
optional notes only.
