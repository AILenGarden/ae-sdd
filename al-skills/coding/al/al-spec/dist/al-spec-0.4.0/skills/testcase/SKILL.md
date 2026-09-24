---
name: testcase
description: Write or review independent executable TestCases. This SKILL is the authoritative TestCase writing specification; load its required template as context.
metadata:
  display_name: TestCase
  version: 0.3.0
  role: specification
  spec_type: testcase
  scope: global
---

# TestCase writing specification

Use this SKILL for independent executable TestCase specifications. A TestCase
may start from a requirement, behavior, defect, contract, or direct test
intent; other specifications, `ae-sdd`, and workflow are optional context
and never prerequisites.

## Required context

Load the mandatory template [`../../templates/global/testcase.md`](../../templates/global/testcase.md)
before drafting or materially reviewing. The template supplies the output
shape; this SKILL supplies the normative writing rules. With a confirmed
project ID, inspect the registry and load the exact project TestCase SKILL and
project template after the global template. Record paths and reasons.
`standards/` files are maintainer references, not runtime prerequisites. Do not
import phases, input gates, confirmation loops, storage APIs, review
orchestration, or test-gate state.

## Document contract and coverage model

Start with `spec_id`, `spec_type: testcase`, `status`, `version`, `owner`,
`project_id`, and `updated`. Use stable `TC-*` IDs; related `REQ-*`, `STORY-*`,
and `AC-*` links are optional. Cite evidence and keep facts, decisions,
assumptions, risks, and questions distinct. Use `MUST`/`MUST NOT` for binding
rules, `SHOULD`/`SHOULD NOT` with a deviation reason, and `MAY` for optional
behavior.

Choose the smallest sufficient levels and explain omissions:

- L1 unit: pure rules, state, conversion, or validation;
- L2 interface: request/response, auth, status, and error contract;
- L3 integration: database, queue, cache, transaction, or adapter behavior;
- L4 end-to-end: a multi-component user/system journey.

Select applicable dimensions rather than blindly generating cases: input
requiredness/type/length/format/range, auth/ownership/sensitive data,
concurrency/idempotency/ordering, related-data and lifecycle states, multi-step
recovery, state transitions, CRUD, callbacks, schedules, and integrations.

## Authoring rules

Each `TC-*` MUST name preconditions, deterministic fixture, steps, expected
business result, side effects, oracle, cleanup, and evidence location. Assert
business fields and side effects, not merely transport status. Mock only direct
external dependencies; state each mock return and why it is acceptable. Prefer
real dependencies for persistence, transaction/rollback, callbacks, locks,
cache invalidation, and protocol integration when feasible. Keep data isolated,
reproducible, traceable, and free of production data. Record a limitation and
compensating evidence when an applicable dimension is skipped.

Cover normal, invalid, unauthorized, missing-data, duplicate/concurrent,
dependency-failure, timeout, partial-success, and recovery behavior when
applicable. A missing Story or AC is recorded as missing context, never
fabricated.

## Required review gate

Before `ready`, verify the SKILL and mandatory template are recorded; objective,
levels, fixtures, environment, scope, and evidence are explicit; each TC has a
replayable oracle and cleanup; important dimensions have cases or a reason and
compensating check; assertions cover business outcomes and side effects; and
unresolved questions/risks are visible. A verification-changing unknown blocks
`ready`.

## Review output and changes

Report untestable oracles, weak assertions, missing boundary/failure/recovery
cases, unreproducible fixtures, unjustified mocks, wrong test level, and
evidence that cannot be replayed. Record deviations in a `DEC-*` with impact,
owner, and compensating verification.
