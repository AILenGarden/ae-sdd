---
name: story
description: Create or review a behavior Story by loading the right evidence and template, then writing a dense, implementation-ready contract without workflow dependencies.
metadata:
  display_name: Story
  version: 0.4.0
  role: specification
  spec_type: story
  scope: global
---

# Story

This SKILL describes how to produce a useful Story, not a lifecycle for moving
documents through SDD. A Story captures one coherent behavior that a reader can
understand, implement, and verify without another Spec being present. PRD, DR,
TestCase, and project documents are optional evidence; they are never gates.

## 1. Required context and source selection

Always load the mandatory output template
[`../../templates/global/story.md`](../../templates/global/story.md) before
writing or materially reviewing a Story. The template is the output shape; this
SKILL is the writing guidance.

When the repository and project identity are known, read the registry and load
the exact project Story SKILL plus every project template it names. For a life
project, use `life-story` and its life template. Record a short context note in
the Story with each loaded path and the reason it applies.

Select source material by the question the Story must answer:

- User request, bug report, or requirement summary: intent, actor, value,
  scope, priority, and unresolved product decisions.
- PRD, when supplied: problem framing, target users, outcomes, scope, and
  product acceptance intent. Do not copy product prose that does not affect
  this behavior.
- DR, when supplied: component boundaries, decisions, state model, contracts,
  data ownership, constraints, failure and security behavior. Treat it as
  evidence, not as permission to invent missing details.
- Product prototype or UI description, when supplied: entry point, visible
  states, field labels, interaction, empty/error/loading behavior, and
  accessibility/compatibility expectations.
- Repository, project assets, API definitions, database schema, tests, and
  existing implementations: actual module names, paths, types, constants,
  wrappers, error codes, state values, and reusable patterns.

Read the smallest set that closes the behavior. If sources conflict, preserve
the conflict in an assumption/open-question or `DEC-*`; do not silently choose
the convenient version. If a fact would change scope, data, security,
compatibility, or verification, leave the Story unresolved rather than
fabricating it.

## 2. Shape the behavior before writing sections

First summarize the behavior in one sentence:

```text
When <trigger>, <actor/system> can <observable behavior>, so that <value>.
```

Then set the smallest useful boundary:

- one actor/system goal and one primary trigger;
- explicit in-scope and out-of-scope behavior;
- preconditions and postconditions/state changes;
- the owning module or boundary;
- the observable result that proves completion.

Split a Story when it needs unrelated actors, independent release decisions,
different owners, or incompatible failure/consistency rules. Do not split only
because the document has several sections. Keep related behavior in one Story
when separating it would force the implementer to reconstruct one contract from
multiple fragments.

## 3. Write for semantic density

Optimize for information per sentence, not document length.

- Prefer a precise table row over repeated prose. Put the condition, action,
  result, owner, and evidence in the same row where possible.
- Introduce a term once, then use the exact same term and ID everywhere.
- State a rule once at its owner boundary. Link to it from ACs or notes instead
  of restating it in the flow, interface, task, and test sections.
- Remove motivational commentary, generic best practices, history, and design
  alternatives that do not change this behavior.
- Replace vague adjectives (`correct`, `robust`, `user-friendly`, `fast`) with
  a value, threshold, state, response, side effect, or evidence target.
- Keep examples short and clearly marked as examples. Never let an example look
  like a project fact.
- Do not add a changelog, revision diary, review transcript, or “what changed”
  section inside a Story. Version/status metadata belongs at the top; change
  history belongs in repository history or a separate project document.

## 4. Required content

The Story must make these questions answerable without guesswork:

1. Who or what triggers the behavior, and what value is delivered?
2. What is included, excluded, and required before it starts?
3. What happens on the happy path, and which boundary owns each step?
4. What happens for invalid, unauthorized, missing, duplicate, concurrent,
   timeout, dependency-failure, partial-success, and recovery cases when
   applicable?
5. What exact REST/SPI/event/UI contract is exposed?
6. Where does every request/response field originate, how is it transformed,
   and where does it end up?
7. Which business rules and state transitions apply, including forbidden ones?
8. How are persistence, idempotency, transaction, compatibility, observability,
   and rollback concerns represented when they affect behavior?
9. How is each acceptance criterion observed and evidenced?

## 5. Contract and field rules

For every interface or event, specify the applicable method/path/topic,
direction, caller, headers/authentication, request, response, validation,
errors, versioning, timeout, retry, rate limit, idempotency, and compatibility.
Do not invent a path, wrapper, error code, class, enum, or constant: verify it
against source/evidence or mark it unknown.

For every meaningful field, close this chain:

```text
source -> entry/request/event -> application -> domain -> persistence/external
-> output/side effect
```

Record type/transform, requiredness, validation, destination, and semantic
reason. Values obtained from the current user, configuration, database,
calculation, or upstream response must say so explicitly.

## 6. Acceptance criteria

Write each `AC-*` as Given/When/Then. One AC should prove one primary,
observable result and name an evidence target such as an HTTP response,
database row, emitted event, state transition, UI state, log, metric, or test
artifact. Cover the highest-risk negative paths, not only the happy path. Do not
write “works correctly” or “handles gracefully” without the concrete result.

## 7. Boundary and implementation guidance

Keep protocol adaptation at the interface boundary, orchestration in the
application layer, business rules/invariants in the domain, and storage or
remote calls in infrastructure. A Story may include a short responsibility
skeleton or implementation hint, but it must not become a system-wide DR or a
file-by-file CodingPlan. A TestCase link is useful when available, but the
Story remains complete without it.

## 8. Evidence and quality check

Use stable `STORY-*`, `BR-*`, `AC-*`, and `Q-*` IDs. Distinguish Fact, Decision,
Assumption, Risk, and Open Question. Before marking `ready`, confirm that the
loaded context is recorded, scope/non-scope is explicit, contracts and field
provenance are closed, negative/state behavior is covered, ACs are observable,
and unresolved design-changing questions are visible.

For review, report missing evidence, duplicated or contradictory rules,
over-sized scope, boundary leaks, invented repository facts, incomplete
contracts, missing edge cases, and ACs that cannot be observed. Record an
intentional deviation as `DEC-*` with impact, owner, and migration/rollback
consequence. Related Specs are links, not prerequisites.
