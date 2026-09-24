---
name: dr
description: Create or review a Design Requirements document by loading relevant evidence and template, then recording explicit design decisions, contracts, and operational consequences.
metadata:
  display_name: DR
  version: 0.4.0
  role: specification
  spec_type: dr
  scope: global
---

# Design Requirements

A Design Requirements document (DR) records the technical design for one
coherent capability or system change. It makes the design understandable,
implementable, reviewable, and operable without requiring the reader to infer
ownership, contracts, state behavior, or failure handling.

This skill defines the context, writing rules, and quality checks for creating
or reviewing a DR.

## Required context and template selection

Resolve the repository and project identity before choosing a template. Apply
these DR writing rules to every project, then choose exactly one output
template:

- **Project template match:** if the registry maps the confirmed `project_id`
  to a DR template, load that project template. The project template may add
  project-specific sections, vocabulary, and constraints.
- **Global template fallback:** if the project is unknown or no exact DR
  template mapping exists, load
  [`../../templates/global/dr.md`](../../templates/global/dr.md).

For `project_id: icec-cloud-life`, the selected template is
[`../../templates/project/life-dr.md`](../../templates/project/life-dr.md).
Do not silently load both the global and project templates.

Record the project ID, selected template path, and reason in the DR's context
note. Read the smallest evidence set that closes the design. Depending on the
task, that may include:

- the user request, bug report, requirement summary, or product decision for
  the problem, goals, scope, actors, priority, and unresolved choices;
- a PRD, when supplied, for product outcomes, users, constraints, and scope;
- a Story, when supplied, for behavior boundaries, entry points, acceptance
  intent, and user-visible outcomes;
- repository constraints, architecture notes, project assets, API/event
  definitions, schemas, existing code, tests, and operational configuration
  for actual modules, paths, types, limits, conventions, and dependencies;
- external-service documentation or compatibility requirements when the design
  crosses a remote system.

If sources conflict, preserve the conflict as a Fact, Decision, Assumption,
Risk, or Open Question with evidence and an owner. Never turn a template
example, inferred module name, or familiar convention into a project fact.

## Define the design problem before filling the template

Start with one sentence:

```text
Given <problem or constraint>, design <capability/system change> so that
<observable outcome> within <explicit boundary>.
```

Then establish the design kernel:

- goals, success signals, In Scope, and Out of Scope;
- primary actors, consumers, and owning component or team;
- constraints, assumptions, and non-negotiable compatibility requirements;
- component responsibilities and non-responsibilities;
- data ownership, state changes, contracts, and observable operational results.

Keep one DR focused on one coherent design boundary. Split it only when
ownership, consistency, release, security, or rollback decisions genuinely
differ. Do not split merely because the template has many sections.

## Write for semantic density

Optimize information per sentence, not document length:

- Prefer a decision, responsibility, constraint, or failure table row that
  contains condition, choice, owner, consequence, and evidence over repeated
  explanatory prose.
- Define each term, component, data object, state, and contract ID once and
  reuse it consistently.
- Put each rule at the boundary that owns it; reference the rule by ID from
  sequences, contracts, and acceptance criteria instead of copying it.
- Keep details that affect implementation, verification, compatibility,
  security, capacity, or operation. Remove history, generic framework advice,
  and alternatives that do not affect the selected design.
- Replace vague words such as `scalable`, `robust`, `secure`, and `fast` with a
  concrete capacity, state, response, control, threshold, or evidence target.
- Keep examples short and explicitly label them as examples. Examples do not
  establish project facts.
- Use stable `DEC-*`, `REQ-*`, `AC-*`, and `Q-*` IDs. Use `MUST`/`MUST NOT` for
  binding behavior, `SHOULD`/`SHOULD NOT` only with a recorded deviation reason,
  and `MAY` for optional behavior. Every normative statement names an actor,
  condition, and observable result.

## Drafting method

Use this writing method:

1. Build an evidence ledger and classify each item as Fact, Decision,
   Assumption, Risk, or Open Question.
2. Write the design kernel: problem, goals, scope, owner, consumers,
   constraints, success signals, and non-goals.
3. Map component and layer ownership, data flows, state transitions, and
   external dependencies before selecting an implementation.
4. For every non-trivial choice, compare viable alternatives and record the
   selected option, rejected alternatives, rationale, accepted cost/risk,
   owner, and evidence.
5. Close data, interface, state, failure, security, observability, migration,
   compatibility, and rollback consequences before writing acceptance criteria.
6. Derive `AC-*` rows from design outcomes and decisions; each AC proves one
   observable result and names its evidence target.
7. Run a compression pass: remove repeated, orphaned, decorative, and
   out-of-scope content while retaining the single authoritative statement of
   each design rule and contract.

## Design decisions and ownership

For each non-trivial decision, record the context and constraints, options
considered, selected option, rejected alternatives and why, accepted cost or
risk, owner, decision date or effective version, and evidence. Use stable
`DEC-*` IDs. A preference without constraints or evidence is not a sufficient
rationale.

Describe every component's responsibility and non-responsibility in terms of
ownership. Keep protocol adaptation at interfaces, orchestration and
transaction coordination in the application boundary, business rules and
invariants in the domain, and persistence or remote calls in infrastructure
when those layers exist in the repository. Do not assign a responsibility to a
layer merely because it is common elsewhere; verify it against project
evidence.

For each owned capability, state its allowed callers, data owner, reuse
mechanism, prohibited duplicate location, and side effects. A DR may show cross-component collaboration.

## Data, state, and consistency rules

For every meaningful entity, table, event, or cache record, specify its owner,
key and uniqueness, lifecycle and retention, field meaning, invariants,
transaction boundary, consistency expectation, and migration/rollback impact.
Explain non-obvious denormalization, indexes, defaults, and derived values.

For stateful behavior, define allowed transitions and their triggers, forbidden
transitions and observable rejection, terminal states, idempotency, duplicate
delivery, concurrency, and recovery. Do not expose a vague “set target status”
operation as the business contract when the design requires a named command or
event.

For every important field or value, close the chain when applicable:

```text
source -> entry/request/event -> application -> domain -> persistence/external
-> output/side effect
```

State the type or transform, requiredness, validation, destination, and reason.
Say explicitly whether a value comes from the caller, configuration, database,
calculation, or an upstream response.

## Contract and failure rules

For each REST, SPI, event, UI, or external-system contract, specify the
applicable method/path/topic, direction, caller, authentication and headers,
request, response, validation, errors, versioning, compatibility, timeout,
retry, rate limit, idempotency, and observability. Verify paths, wrappers,
error codes, classes, enums, constants, and payload fields against evidence or
mark them unknown; never invent them.

Analyze each critical dependency for timeout, unavailability, duplicate
delivery, retry amplification, partial success, ordering, compensation,
degradation, recovery, and manual intervention. State the resulting system
state and the evidence that indicates recovery or requires an operator.

Make security and operations part of the design when they change behavior:
authorization, sensitive-data handling, audit records, logs, metrics, traces,
alerts, capacity thresholds, rollout controls, migration sequencing,
backward-compatible behavior, and rollback. Do not defer a design-changing
consequence to an unspecified implementation note.

## Acceptance and verification rules

Describe design acceptance as observable outcomes, not intentions. Write every
`AC-*` using Given/When/Then with one observable result and an evidence target
such as an API response, database constraint, state transition, event,
configuration value, log, metric, trace, deployment check, or test artifact.
Avoid “the architecture is sound”, “the system is scalable”, or “failure is
handled gracefully” without a measurable or inspectable result.

The DR owns design decisions and their verification intent. It may include
verification guidance and optional Story/TestCase mappings, but it must not
become exhaustive executable test steps or a file-by-file CodingPlan.

## Quality check and review

Before marking a DR `ready`, confirm that the selected template and material
evidence are recorded, the problem and design boundary are clear, goals and
non-goals are explicit, every non-trivial decision has alternatives and an
accepted cost/risk, component and data ownership are closed, contracts and
state transitions are implementable without guessing, applicable failure/
security/operations behavior is covered, migration and rollback consequences
are visible, every AC is observable, and design-changing questions are listed.

A design-changing unknown blocks `ready` until it is resolved or an explicit
owner accepts the documented risk and deviation.

For review, report unsupported decisions, duplicated rules, oversized scope,
boundary leaks, unowned data, invented repository facts, incomplete or
incompatible contracts, missing failure modes, unobservable outcomes, and
design-changing questions. Record an intentional deviation as `DEC-*` with
impact, owner, effective version, and migration/rollback consequence.
