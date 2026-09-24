# TC-{number} - {Story test design}

## Metadata and Coverage

```yaml
spec_id: TC-{number}
spec_type: testcase
status: planned
version: 0.1.0
owner: <role or person>
project_id: <stable project id or none>
updated: YYYY-MM-DD
```

- Optional related requirements, Stories, or ACs: `REQ-...`, `STORY-...`, `AC-...`
- Test objective:
- Required evidence format:

## Coverage Objective and Strategy

- Behavior and risk under test:
- Applicable type dimensions: state machine / CRUD / callback / scheduled work /
  integration / other:
- Why selected dimensions and levels are sufficient:
- Intentionally omitted levels or dimensions and compensating check:

## Environment and Data

- Test level and runner:
- Environment / feature flags:
- External dependencies and stubs:
- Fixtures, builders, seed data, and cleanup:
- Clock, locale, identity, and permission setup:

## Coverage Matrix

| AC ID | TC ID | Scenario | Test level | Automated | Oracle / evidence |
|-------|-------|----------|------------|-----------|-------------------|
| AC-001 | TC-001 | happy path | unit / integration / HTTP / E2E | yes / no | |

### Dimension Matrix

| Dimension | Applicable? | Required scenarios | Covered TC IDs | Gap or non-applicable evidence |
|-----------|-------------|--------------------|----------------|-------------------------------|
| Input boundaries | yes / no | | | |
| Authentication/authorization | yes / no | | | |
| Concurrency/idempotency | yes / no | | | |
| Related-data states | yes / no | | | |
| Multi-step sequence/recovery | yes / no | | | |
| State transitions | yes / no | | | |
| CRUD/callback/schedule/integration | yes / no | | | |

## Scenario Count and Completeness Check

| Strategy/dimension | Minimum expected cases | Planned cases | Covered IDs | Status / reason |
|--------------------|------------------------|---------------|-------------|-----------------|
| Type-specific strategy | | | | |
| Input and permission boundaries | | | | |
| Failure and recovery | | | | |

Use this section to show that the planned set is sufficient; do not use a
document-generation gate or require another Spec type.

## Test Cases

### TC-001 - {Scenario}

- Covers: `AC-001`
- Preconditions:
- Input / fixture:
- Steps:
  1.
  2.
- Expected result:
- Negative or boundary assertion:
- Cleanup:
- Automated entry point or manual evidence:

### Authenticity and Dependency Boundary

- Test data source (Story/requirement/contract/fixture evidence):
- Real HTTP / DB / queue / cache dependencies required:
- Direct external dependencies mocked and exact return values:
- Why each mock is acceptable:
- Negative/failure injection:

## Failure and Recovery Scenarios

| Scenario | Injected failure | Expected state | Recovery / retry assertion |
|----------|------------------|----------------|-----------------------------|
| | | | |

## Regression Scope

- Must-run regression suites:
- Related behavior that may regress:
- Tests safe to omit and why:

## Risks and Uncovered Items

| Risk or uncovered behavior | Reason | Compensating evidence | Owner | Status |
|----------------------------|--------|----------------------|-------|--------|
| | | | | open |

## Execution and Reporting

- Required command(s):
- Pass/fail oracle:
- Artifact locations (logs, response, snapshot, report):
- Flaky-test policy:
- Tests intentionally not covered and why:

## Optional Related Notes

- Suggested implementation verification:
- Environment prerequisites that should be documented:
