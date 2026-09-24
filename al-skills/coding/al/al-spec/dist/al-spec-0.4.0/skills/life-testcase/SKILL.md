---
name: life-testcase
description: Write or review an independent life-project TestCase. This SKILL embeds the TestCase and life writing specification; load its required life template as context.
metadata:
  display_name: life TestCase
  version: 0.3.0
  role: project-specification
  spec_type: testcase
  scope: project
  project_id: icec-cloud-life
---

# life TestCase writing specification

Use this SKILL only for the confirmed `icec-cloud-life` project (short name
`life`). It is a complete, standalone TestCase writing specification. A
TestCase can be created from any behavior, contract, requirement, defect, or
test intent; other specifications, `ae-sdd`, and workflow are optional
context and never prerequisites.

## Required context

Load the mandatory project template
[`../../templates/project/life-testcase.md`](../../templates/project/life-testcase.md)
before drafting or materially reviewing, plus the global TestCase template
[`../../templates/global/testcase.md`](../../templates/global/testcase.md) for
the shared output contract. This SKILL is the authoritative life TestCase
writing specification and embeds all global TestCase and life rules;
`standards/` files are maintainer references, not runtime prerequisites.
Record the exact project ID and loaded template paths/reasons. Do not import
`ae-sdd` phases, mandatory input gates, confirmation loops, storage APIs,
review orchestration, gate IDs, or downstream handoffs.

## Document contract and coverage model

Start with `spec_id`, `spec_type: testcase`, `scope: project`, `project_id:
icec-cloud-life`, `status`, `version`, `owner`, `domain`, and `updated`. Use
stable `TC-*` IDs; related `REQ-*`, `STORY-*`, and `AC-*` links are optional.
Cite source/assets/command evidence and separate Fact, Decision, Assumption,
Risk, and Open question. `MUST`/`MUST NOT` bind, `SHOULD`/`SHOULD NOT` need a
deviation reason, and `MAY` is optional.

Choose the smallest sufficient levels and explain omissions: L1 unit for pure
rules/state/conversion/validation; L2 interface for request/response/auth/
status/errors; L3 integration for DB/queue/cache/transaction/adapter; L4
end-to-end for a multi-component journey. Select applicable dimensions:
requiredness/type/length/format/range, auth/ownership/sensitive data,
concurrency/idempotency/ordering, related-data/lifecycle, multi-step recovery,
state transitions, CRUD, callbacks, schedules, and integrations.

## TestCase authoring rules

Each `TC-*` MUST name preconditions, deterministic fixture, steps, expected
business result, side effects, oracle, cleanup, and evidence location. Assert
business fields and side effects, not only transport status. Mock only direct
external dependencies; state each mock return and why. Prefer real dependencies
for persistence, transaction/rollback, callbacks, locks, cache invalidation,
Feign/protocol integration when feasible. Keep data isolated, reproducible,
traceable, and free of production data. Record a limitation and compensating
evidence when a dimension is skipped.

## life project test rules

- Identify the actual domain/module/service/BFF/Web, environment, Java/Spring
  profile, runner, and project asset evidence before fixtures or commands.
- Default unit stack is JUnit 4.12 + Mockito + AssertJ; integration uses Spring
  Boot Test with `@Transactional` + `@Rollback` where applicable; interface
  tests prefer real HTTP with `SpringBootTest(RANDOM_PORT)` + `TestRestTemplate`.
  Use MockMvc only when real HTTP is impossible and record why.
- Mock direct external dependencies and assert business fields, state changes,
  and side effects. Core INSERT/UPDATE/DELETE, rollback, distributed lock,
  Feign, Redis invalidation, and callback/idempotency behavior require real
  dependencies when the environment permits; otherwise record limitation and
  compensating evidence.
- Cover input boundaries, authentication/authorization, concurrency, duplicate
  delivery, related-data states, multi-step recovery, and state transition
  matrices when applicable. Never use production data.
- Retain useful coverage dimensions and evidence rules from `ae-sdd`; exclude
  its phase routing, Story/PRD/asset gates, storage APIs, save/report protocol,
  user confirmation loops, gate IDs, and automatic handoffs.

## Required review gate

Before `ready`, verify this SKILL plus global and life templates are recorded;
objective, level, environment, fixtures, scope, and evidence are explicit; each
TC has a replayable oracle and cleanup; dimensions have cases or a reason and
compensating check; business outcomes and side effects are asserted; and
unresolved questions/risks/deviations are visible. A verification-changing
unknown blocks `ready`.

## Review output and changes

Report wrong life module/environment assumptions, weak oracles, missing real
dependency coverage, unjustified mocks, unisolated fixtures, unsupported
commands, missing boundary/failure/recovery cases, and unreplayable evidence.
Record deviations as `DEC-*` with impact, owner, and compensating verification.
Related Specs are optional notes only.
