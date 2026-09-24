---
name: testcase
description: Create or review executable TestCase specifications by loading relevant evidence, selecting coverage, and defining replayable fixtures, oracles, and evidence.
metadata:
  display_name: TestCase
  version: 0.4.0
  role: specification
  spec_type: testcase
  scope: global
---

# TestCase

A TestCase defines how a behavior, contract, risk, or failure mode is proven.
It must make the objective, level, fixture, steps, oracle, cleanup, and evidence reproducible.

## Context and template

Resolve the repository and `project_id`, then choose exactly one template:

- If the registry maps the project to a TestCase template, load that project
  template; it may add project-specific fields and vocabulary.
- Otherwise load [`../../templates/global/testcase.md`](../../templates/global/testcase.md).

For `icec-cloud-life`, use
[`../../templates/project/life-testcase.md`](../../templates/project/life-testcase.md).
Do not load both templates. Record the project ID, selected template, and reason.

Read only evidence needed to make the test replayable: user request or defect;
PRD, DR, Story, contract, event schema, state model, or AC when supplied;
repository code, test conventions, module paths, build commands, fixtures,
schemas, migrations, configuration, feature flags, and environment/dependency
details. Classify conflicts as Fact, Decision, Assumption, Risk, or Open
Question with evidence and owner. Do not invent paths, classes, commands,
payloads, or missing behavior.

## Test objective and scope

Start with:

```text
Given <system state and risk>, execute <test action> to prove <observable
business result> at <test level>.
```

Define the system under test, behavior/risk, actor or trigger, expected business
result, side effects, test level, fixture, environment, identity, permissions,
clock, cleanup, oracle, and evidence location. Keep one specification focused on
one coherent behavior or risk set. Split only when setup, oracle, environment, ownership, or failure semantics differ.

## Coverage selection

Choose the smallest sufficient level and explain omissions:

- **L1 unit:** pure rules, state, conversion, or validation;
- **L2 interface:** request/response, authentication, status, error, or
  compatibility contract; prefer real HTTP/protocol exchange;
- **L3 integration:** database, transaction, queue, cache, lock, or adapter;
  use real dependencies when their semantics are under test;
- **L4 end-to-end:** a multi-component journey with captured environment
  evidence.

Select applicable dimensions:

- input requiredness, type, length, format, range, and combinations;
- authentication, authorization, ownership, tenant, and sensitive data;
- concurrency, idempotency, duplicate delivery, ordering, and retry;
- related-data, missing/stale records, lifecycle, and state transitions;
- CRUD, callbacks/webhooks, scheduled work, integrations, partial failure, and
  recovery.

For every omitted level or dimension, record why it is not applicable or name
the compensating evidence. Coverage is sufficient when each important outcome
and applicable risk has an executable or inspectable oracle.

## Case authoring rules

Every `TC-*` MUST include preconditions, deterministic fixture, ordered steps,
expected business result, side effects, oracle, cleanup, and evidence location.
Use stable `TC-*` IDs; optional `REQ-*`, `STORY-*`, and `AC-*` links are trace
context, not prerequisites.

- Use evidenced requests, commands, events, database operations, or UI actions;
  do not guess implementation details.
- Assert business fields, persisted data, events, state transitions, and side
  effects, not only transport status or absence of exceptions.
- Cover normal, invalid, boundary, unauthorized, missing-data, duplicate,
  concurrent, timeout, dependency-failure, partial-success, and recovery cases
  when applicable.
- Keep data isolated, deterministic, reproducible, traceable, and free of
  production data. Record seed and cleanup methods.
- Define an oracle that distinguishes pass, fail, and inconclusive. “Works
  correctly” and “handles gracefully” are not oracles.
- Record the exact test command or entry point and artifact location.
## Dependencies and mocks

Mock only direct external dependencies that are unavailable, unsafe,
nondeterministic, or outside the test's ownership. For each mock, state the
exact return or injected failure and why it is acceptable. Prefer real
semantics for persistence, transactions/rollback, locks, queues, caches,
callbacks, protocol boundaries, and idempotency when feasible.

Align the test level with the claim: unit tests do not prove transport or
persistence; interface tests prove external contracts; integration tests prove
dependency semantics; end-to-end tests prove the complete journey.

For each critical failure, record the injected condition, resulting state,
retry/timeout/compensation/degradation behavior, recovery action, and recovery
evidence. Include duplicate delivery, retry amplification, concurrency
conflicts, and partial success when relevant.

## Life project

When using the `icec-cloud-life` template, verify the actual domain, module,
Service/BFF/Web, Java/Spring profile, runner, fixture source, and project assets
before writing commands or class paths. Apply only evidenced project facts; do
not generalize one life module's stack or paths to another.

## Quality and review

Before `ready`, confirm the selected template and evidence are recorded; scope,
level, dimensions, environment, and fixtures are justified; every `TC-*` has
replayable steps, oracle, cleanup, and evidence; business outcomes and side
effects are asserted; applicable failure/recovery cases are covered; omissions
have compensating evidence or an owner; and verification-changing questions
are visible. Such an unknown blocks `ready` until resolved or explicitly
accepted with a documented deviation.

Review for weak oracles/assertions, missing boundary or failure cases,
unreproducible fixtures, unjustified mocks, wrong levels, unsupported commands
or paths, unisolated data, and unreplayable evidence. Record deviations as
`DEC-*` with impact, owner, and compensating verification.
