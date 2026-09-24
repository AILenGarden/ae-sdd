---
name: life-story
description: Create or review a life-project Story by loading project evidence and templates, then writing a dense, implementation-ready behavior contract.
metadata:
  display_name: life Story
  version: 0.4.0
  role: project-specification
  spec_type: story
  scope: project
  project_id: icec-cloud-life
---

# life Story

This is the `icec-cloud-life` project version of the Story writing SKILL. It is
standalone: other Specs and the `ae-sdd` workflow are optional context, not
prerequisites. It is the authoritative life Story writing guidance and describes
how to write a useful life Story, not how to route documents through a process.

## 1. Required context and source selection

Always load both required templates before writing or materially reviewing:

1. [`../../templates/global/story.md`](../../templates/global/story.md), the
   shared Story output contract;
2. [`../../templates/project/life-story.md`](../../templates/project/life-story.md),
   the life project's content template.

Also confirm the repository identity and load the exact life project context:

- repository root and source root;
- `AGENTS.md`, project constraints, API/database conventions, and relevant
  project assets;
- the target domain/module source, existing endpoint/event definitions, schema,
  tests, and reusable implementations;
- PRD, DR, product prototype, or user request summary only when supplied or
  useful for this behavior.

Record the loaded paths and why they apply. `standards/` files are maintainer
references, not runtime prerequisites. Do not load `ae-sdd` phases, generation
stages, storage APIs, review loops, gates, or automatic handoffs.

## 2. Choose and reconcile sources

Use each source for the question it can answer:

- User request, bug, or requirement summary: intent, actor, value, priority,
  scope, and unresolved product choices.
- PRD, if supplied: problem, target user, outcome, scope, and product-level
  acceptance intent. Keep only details that constrain this behavior.
- DR, if supplied: boundaries, decisions, state model, data ownership,
  contracts, constraints, failure, security, and observability. Do not copy a
  system-wide design into a single behavior.
- Product prototype or UI specification, if supplied: entry point, interaction,
  visible states, loading/empty/error behavior, and compatibility needs.
- Life repository/assets/source/tests: actual domain, module shape, package,
  context path, ports, Feign/service constants, wrappers, error codes, fields,
  tables, and existing patterns.

Read the smallest set that closes the Story. When sources disagree, retain the
conflict as an explicit assumption, question, or `DEC-*` with evidence and
owner. Never upgrade a template example or one domain's fact into a universal
life-project fact.

## 3. Define a small, independent behavior

Summarize the behavior as:

```text
When <trigger>, <actor/system> can <observable behavior>, so that <value>.
```

The Story should have one primary goal, trigger, owning boundary, and
observable result. State in-scope and out-of-scope behavior, preconditions,
postconditions, and state changes. Split only when actors, owners, release
decisions, or failure/consistency rules genuinely differ. Related Story or
TestCase links remain optional references, not dependencies.

## 4. Write with high semantic density

- Prefer tables with condition, action, result, owner, and evidence over prose
  that repeats the same information.
- Introduce a term, field, state, or ID once and reuse it consistently.
- Put a rule at its owning boundary. Link to it from the flow or AC instead of
  restating it in every section.
- Remove history, motivation, generic framework advice, and alternatives that
  do not change this behavior.
- Replace vague words such as “correct”, “robust”, “fast”, or “user-friendly”
  with a concrete state, response, side effect, threshold, or evidence target.
- Keep examples short and explicitly marked. Examples are not project facts.
- Do not write a changelog, review transcript, revision diary, or “what changed”
  section inside the Story. Keep version/status metadata at the top and change
  history in repository history or a separate project document.

## 5. life boundary and contract rules

- Name the exact domain and module (`life-cs`, `life-im`, `life-user`,
  `life-vehicle`, `life-workticket`, or another evidenced module). Verify the
  module shape instead of assuming all life services have the same layout.
- Confirm the actual BFF/Web -> SPI/Feign -> Service path, or another observed
  call boundary. Verify package roots, context paths, ports, wrappers, error
  codes, Feign names, service constants, and class names from source/assets.
- Keep protocol adaptation in Interfaces, orchestration in Application,
  business rules/invariants in Domain, and persistence/remote calls in
  Infrastructure. BFF/Web must not directly access DB/Redis/Kafka.
- For every REST/SPI/event contract, specify direction, method/path/topic,
  headers/auth, request/response, validation, errors, versioning, timeout,
  retry, rate limit, idempotency, and compatibility where applicable.
- For every field, close source -> Request/Event -> Application -> Domain ->
  PO/DO/DB or external field -> output. State type/transform, requiredness,
  validation, destination, and semantic reason.
- For DB changes, record table/column/index, exact key or WHERE condition,
  CRUD behavior, transaction boundary, and concurrency/idempotency behavior.

## 6. Behavior and acceptance rules

Describe the happy path and applicable invalid, unauthorized, missing-data,
duplicate, concurrent, timeout, dependency-failure, partial-success, and
recovery paths. Write every `AC-*` as Given/When/Then with one observable result
and an evidence target such as HTTP response, row, event, state, UI, log,
metric, or test artifact.

## 7. Review quality bar

Before marking `ready`, verify both templates and all material evidence are
recorded; the behavior boundary and owner are clear; contracts and field
provenance are closed; life module facts are sourced; negative/state behavior
and non-functional constraints are covered; every AC is observable; and
assumptions, conflicts, and questions are visible. A design-changing unknown
blocks `ready`.

For review, report missing or contradictory evidence, copied/repeated content,
oversized scope, invented life paths/constants, layer-boundary leaks, incomplete
contracts or field mappings, missing edge cases, and unobservable ACs. Record
intentional deviations as `DEC-*` with impact, owner, and migration/rollback
consequence. Related Specs are links, never prerequisites.
