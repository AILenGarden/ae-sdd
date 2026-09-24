# STORY-{number} - {Behavior title}

> This template is required context for the `story` SKILL. Load the invoking
> SKILL first; its body is the authoritative Story writing specification.
> Project-specific rules are optional additions selected by `project_id`.
> Related documents may be cited, but none
> is required to create or review this Story.

## 1. Metadata and Intent

```yaml
spec_id: STORY-{number}
spec_type: story
scope: global-or-project
project_id: <stable project id or none>
status: draft
version: 0.1.0
owner: <role or person>
updated: YYYY-MM-DD
```

- As a `<persona/system>`, I want `<behavior>`, so that `<value>`.
- Behavior intent:
- Optional related Specs or IDs:

## 2. Scope and Context

### In Scope

-

### Out of Scope

-

### Actor, Trigger, and Preconditions

- Actor/system:
- Trigger:
- Preconditions:
- Postconditions and state changes:

## 3. Involved Components and Boundaries

| Component / layer | Role in this Story | Responsibility | Non-responsibility | Evidence |
|-------------------|-------------------|----------------|-------------------|----------|
| | | | | |

## 4. Trigger and Entry Points

| Entry kind | Actual endpoint/event/page/command | Caller | Preconditions | Evidence |
|------------|------------------------------------|--------|---------------|----------|
| REST / SPI / event / UI | | | | |

State every applicable entry point. Mark non-applicable kinds explicitly rather
than inventing an entry.

## 5. Main Flow

1. 
2. 
3. 

For each step, name the actor, input, state change, output, and owning boundary.

## 6. Implementation Decision Baseline (Required for non-trivial implementation)

Record why the proposed implementation reuses, extends, or creates a capability.
This is design evidence, not an upstream document gate or process handoff.

### Implementation Point Inventory

| # | Implementation point | Source/evidence | Capability owner | Risk |
|---|-----------------------|-----------------|------------------|------|
| 1 | | | | low / medium / high |

### Existing Capability Reuse Scan

| Implementation point | Search scope and terms | Existing capability found | Conclusion | Non-reuse reason | Evidence |
|----------------------|------------------------|---------------------------|------------|------------------|----------|
| | repository/assets/shared components/history | | reuse / extend / new | | |

### Mature Alternatives and Quality Assessment

| Implementation point | Mature pattern/solution | Adopt? | Reason/evidence |
|----------------------|-------------------------|--------|-----------------|
| | | yes / no | |

| Candidate | Usability | Efficiency | Maintainability | Robustness | Readability | Conclusion |
|----------|-----------|------------|-----------------|------------|------------|------------|
| Reuse / extend / new | | | | | | |

### Unique Capability Ownership

| Business capability | Unique implementation owner | Allowed callers | Reuse mechanism | Prohibited duplicate location | Side effects |
|---------------------|----------------------------|----------------|------------------|------------------------------|-------------|
| | | | | | |

## 7. Business Rules and Exceptions

| ID | Rule / scenario | Preconditions | Expected behavior | Error / state | Owner |
|----|-----------------|---------------|-------------------|--------------|-------|
| BR-001 | | | | | |

Include invalid input, unauthorized access, missing data, duplicate/retry,
dependency failure, timeout, partial success, and recovery when in scope.

## 8. Interface Contract

| Contract | Direction | Method / path / topic | Auth / headers | Request | Response | Errors | Idempotency |
|----------|-----------|-----------------------|----------------|---------|----------|--------|------------|
| API-001 | | | | | | | |

### Request and Response Fields

| Field | Direction | Source | Transform / type | Required | Validation | Destination | Reason |
|-------|-----------|--------|------------------|----------|------------|-------------|--------|
| | in / out | | | yes / no | | | |

### Protocol and Client Notes (Optional)

- Pagination:
- Error response and message:
- State rendering or UI behavior:
- Retry and timeout:
- Compatibility/versioning:

### REST Contract (When applicable)

| Item | Definition |
|------|------------|
| Context path and method | |
| Request validation and field errors | |
| Response wrapper and success semantics | |
| Error code/status mapping | |
| Timeout, retry, idempotency, rate limit | |
| Caller handling and compatibility | |

### SPI / Event Contract (When applicable)

| Item | Definition |
|------|------------|
| Service/event name and direction | |
| Payload and version | |
| Consumer/provider responsibility | |
| Duplicate/order semantics | |
| Failure, retry, dead-letter or compensation | |

## 9. Data Model and Persistence (When Applicable)

### Entity / Table Changes

| Entity/table | Operation | Field or index change | Key / uniqueness | Owner |
|--------------|-----------|-----------------------|-----------------|-------|
| | | | | |

### CRUD and Field Mapping

| Operation | Trigger | Exact key/WHERE condition | From -> to | Transaction / concurrency |
|-----------|---------|---------------------------|------------|--------------------------|
| | | | | |

### Field Chain (Source to Destination)

| Data item | Source | Entry field | Application field | Domain field | Persistence field | DB/external field | Output field | Conversion/validation |
|-----------|--------|-------------|-------------------|--------------|-------------------|-------------------|--------------|-----------------------|
| | | | | | | | | |

### Constants and Magic Values

| Constant/value | Value | Source/evidence | Configuration entry | Meaning |
|----------------|-------|-----------------|--------------------|---------|
| | | | | |

## 10. State and Configuration (When Applicable)

- State transitions:
- Forbidden transitions:
- Constants/enums and their source:
- Configuration keys, defaults, and rollout:

### State Transition Matrix

| Current state | Business command/event | Allowed? | Next state | Rejection/error | Evidence |
|---------------|------------------------|----------|------------|----------------|----------|
| | | yes / no | | | |

## 11. Pseudocode and Responsibility Skeleton (Optional)

Show only the layer-correct call skeleton. Domain owns rules and transitions;
Application owns orchestration and transaction boundaries; repositories only
store/read; interfaces only adapt and validate. Do not turn this section into
production code.

```text
Entry -> validate/adapt -> Application orchestrates -> Domain decides ->
Repository/adapter stores or calls -> response/event
```

## 12. Acceptance Criteria

| ID | Related rule/requirement (optional) | Given | When | Then (observable) | Evidence target |
|----|-------------------------------------|-------|------|------------------|-----------------|
| AC-001 | | | | | |

Each criterion has one observable result. Do not use "works correctly" or
"handles gracefully" without a concrete response, state, record, event, or
metric.

## 13. Non-Functional Behavior

- Security/privacy:
- Performance/capacity threshold:
- Consistency/idempotency/concurrency:
- Compatibility/migration:
- Logging/metrics/tracing:
- Rollback or feature flag:

### Idempotency and Failure Compensation

| Operation | Idempotency key | Duplicate behavior | Concurrent same-key behavior | Compensation/degradation |
|-----------|-----------------|--------------------|------------------------------|--------------------------|
| | | | | |

## 14. Test Considerations (Optional)

| AC / rule | Suggested test level | Data / environment | Real vs mock | Evidence |
|-----------|----------------------|--------------------|--------------|----------|
| AC-001 | | | | |

## 15. Implementation Notes (Optional)

- Responsibility hints for implementation:
- Reusable component or pattern to inspect:
- Files/symbols only when confirmed in the repository:
- Local task ordering, if any:

## 16. Frontend Contract and Experience (Optional)

When a Story has a frontend consumer, document frontend-facing behavior here;
backend-only Stories may mark this section not applicable.

### Frontend Contract

| API/event | Request/response shape | Loading/empty/error state | Retry behavior | Evidence |
|-----------|------------------------|---------------------------|----------------|----------|
| | | | | |

### State Presentation

| State value | Meaning | UI color/icon/text | Action enabled? | Terminal? | Synchronization |
|-------------|---------|-------------------|-----------------|-----------|----------------|
| | | | | | |

### Boundary and Error Presentation

| Scenario | Trigger | Frontend behavior | Observable UI |
|----------|---------|-------------------|----------------|
| Empty data / long field / concurrency conflict / weak network | | | |

### Frontend Integration Notes

- Frontend owner/team:
- Mock or contract environment:
- Browser/app compatibility:
- Component and E2E test entry points:

## 17. Risks, Questions, and Deviations

| ID | Risk / question / deviation | Impact | Owner | Decision trigger / mitigation | Status |
|----|-----------------------------|--------|-------|-------------------------------|--------|
| Q-001 | | | | | open |

## 18. Optional Related Notes

- Design considerations:
- Test design considerations:
- Implementation plan considerations:
- External references:
