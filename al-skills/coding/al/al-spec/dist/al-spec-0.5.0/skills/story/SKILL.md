---
name: story
description: Create or review a behavior Story by loading relevant evidence and template, then writing a dense, implementation-ready contract.
metadata:
  display_name: Story
  version: 0.5.0
  role: specification
  spec_type: story
  scope: global
---

# Story

A Story is an independently understandable behavior specification. It defines
one coherent behavior that can be understood, implemented, and verified by the
delivery team.

This skill defines the context, writing rules, and quality checks for creating or reviewing a Story.

## Required context and template selection

Resolve the repository and project identity before choosing a template. Apply
these Story writing rules to every project, then choose exactly one output
template:

- **Project template match:** if the registry maps the confirmed `project_id`
  to a Story template, load that project template. The project template may
  add project-specific sections and vocabulary.
- **Global template fallback:** if the project is unknown or no exact Story
  template mapping exists, load
  [`../../templates/global/story.md`](../../templates/global/story.md).

For `project_id: icec-cloud-life`, the selected template is
[`../../templates/project/life-story.md`](../../templates/project/life-story.md).
Do not silently load both the global and project templates.

Record the project ID, selected template path, and reason in the Story's
context note. Read the smallest evidence set that closes the behavior. Depending
on the task, that may include:

- user request, bug report, or requirement summary for intent, actor, value,
  scope, priority, and unresolved product decisions;
- PRD, when supplied, for problem framing, users, outcomes, product scope, and
  product acceptance intent;
- DR, when supplied, for component boundaries, decisions, state, contracts,
  data ownership, constraints, failure, security, and observability;
- product prototype or UI description for entry points, fields, interaction,
  visible states, loading/empty/error behavior, and compatibility;
- repository, project assets, API definitions, schemas, existing code, and
  tests for actual modules, paths, types, constants, wrappers, error codes,
  state values, and reusable patterns.

If sources conflict, preserve the conflict as a Fact/Decision/Assumption/Risk/
Open Question with evidence and an owner. Never turn a template example,
inferred class name, or familiar convention into a project fact.

## Define the behavior before filling the template

Start with one sentence:

```text
When <trigger>, <actor/system> can <observable behavior>, so that <value>.
```

Then establish the smallest useful boundary:

- one primary actor or system goal and trigger;
- explicit In Scope and Out of Scope;
- Preconditions, Postconditions, and state changes;
- owning component or module;
- observable result proving completion.

Split a Story only when actors, owners, release decisions, or failure/
consistency rules genuinely differ. Do not split merely because the template
has many sections.

## Write for semantic density

Optimize information per sentence, not document length:

- Prefer a table row containing condition, action, result, owner, and evidence
  over repeated explanatory prose.
- Define a term, field, state, or ID once and reuse it consistently.
- Put each rule at the boundary that owns it; refer to it from flows and ACs by
  ID instead of copying it into every section.
- Keep only details that affect this behavior's implementation, verification,
  compatibility, security, or operation. Remove history, motivation, generic
  framework advice, and irrelevant alternatives.
- Replace vague words such as `correct`, `robust`, `fast`, and `user-friendly`
  with a concrete state, response, side effect, threshold, or evidence target.
- Keep examples short and explicitly label them as examples. Examples do not
  establish project facts.

## Drafting method

Use this writing method:

1. Build an evidence ledger and classify each item as Fact, Decision,
   Assumption, Risk, or Open Question.
2. Write the behavior kernel: actor, trigger, value, scope, owner,
   preconditions, postconditions, and observable result.
3. Build a scenario matrix for the happy path and applicable invalid,
   unauthorized, missing-data, duplicate, concurrent, timeout, dependency-
   failure, partial-success, and recovery cases.
4. Close the interface and field contracts before writing acceptance criteria.
5. Derive `AC-*` rows from the scenarios and rules; each AC proves one
   observable result and names its evidence target.
6. Run a compression pass: remove repeated, orphaned, decorative, and
   out-of-scope content while retaining the single authoritative statement of
   each rule and contract.

## Contract and field rules

For each REST, SPI, event, or UI contract, specify the applicable method/path/
topic, direction, caller, headers/authentication, request, response, validation,
errors, versioning, timeout, retry, rate limit, idempotency, and compatibility.
Verify paths, wrappers, error codes, classes, enums, and constants against
evidence or mark them unknown; never invent them.

For every meaningful field, close this chain:

```text
source -> entry/request/event -> application -> domain -> persistence/external
-> output/side effect
```

State the type/transform, requiredness, validation, destination, and semantic
reason. Say explicitly when a value comes from the current user, configuration,
database, calculation, or upstream response.

## Behavior and acceptance rules

Describe the happy path and applicable boundary/failure/recovery behavior. Keep
protocol adaptation in the interface boundary, orchestration in application,
business rules and invariants in domain, and persistence or remote calls in
infrastructure. A Story may include implementation hints, but not system-wide
architecture or a file-by-file coding plan.

Write every `AC-*` as Given/When/Then with one observable result and an evidence
target such as an HTTP response, database row, event, state transition, UI
state, log, metric, or test artifact. Do not write “works correctly” or
“handles gracefully” without the concrete result.

## Quality check and review

Before marking a Story `ready`, confirm that the selected template and material
evidence are recorded, the behavior boundary and owner are clear, contracts and
field provenance are closed, applicable negative/state behavior is covered,
every AC is observable, and design-changing questions are visible.

For review, report missing or contradictory evidence, duplicated rules,
oversized scope, boundary leaks, invented repository facts, incomplete
contracts, missing edge cases, and unobservable ACs. Record an intentional
deviation as `DEC-*` with impact, owner, and migration/rollback consequence.
