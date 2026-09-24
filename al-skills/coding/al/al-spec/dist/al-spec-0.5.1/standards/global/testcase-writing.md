# TestCase Writing Reference

> Maintainer/provenance reference only. Runtime TestCase rules live in
> `skills/testcase/SKILL.md`; do not require this file when invoking the SKILL.

The TestCase document defines how a behavior will be proven. It may be written
from a Story, a requirement, a bug report, an interface contract, or direct
test intent. None of those sources is a mandatory prerequisite.

## Coverage Model

Choose the smallest sufficient set of levels and explain the choice:

| Level | Purpose | Typical evidence |
|-------|---------|------------------|
| L1 unit | pure rule, state, conversion, or validation | deterministic assertions |
| L2 interface | request/response, auth, status, and error contract | real HTTP or protocol exchange when feasible |
| L3 integration | database, queue, cache, transaction, or external adapter | real dependency or explicitly justified substitute |
| L4 end-to-end | multi-component user/system journey | environment run with captured artifacts |

## Scenario Dimensions

Select applicable dimensions rather than blindly generating cases:

- input requiredness, type, length, format, range, and field combinations;
- authentication, authorization, tenant/ownership, and sensitive data;
- concurrency, idempotency, duplicate delivery, and ordering;
- related data states, missing records, stale versions, and lifecycle states;
- multi-step sequences and recovery after partial failure;
- state-machine transition matrix when states or events exist;
- CRUD, callback/webhook, scheduled task, or integration-specific behavior.

## Test Case Rules

- Each `TC-*` names its preconditions, fixture, steps, expected result, oracle,
  cleanup, and evidence location.
- Assertions verify business fields and side effects, not only transport status.
- Mock only direct external dependencies; state what is real and why.
- Test data is deterministic, isolated, and traceable to the behavior under
  test. Never use production data.
- A skipped dimension has a reason and a compensating check or explicit risk.

## Review Checklist

- Does each important behavior have at least one executable or inspectable
  oracle?
- Are normal, invalid, unauthorized, duplicate, failure, and recovery cases
  covered where relevant?
- Is the test level appropriate for the risk?
- Can another engineer reproduce the environment and fixture?
