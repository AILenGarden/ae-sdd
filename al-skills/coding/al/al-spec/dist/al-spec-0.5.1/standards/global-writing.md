# ALSpec Embedded-Baseline Reference

> Maintainer reference only. The selected type `skills/*/SKILL.md` is the
> runtime-authoritative writing specification; this file is not a required
> context file and must not be treated as a separate SKILL.

This reference covers the current DR, Story, and TestCase types. A project layer may add stricter rules or fill in project facts, but
it must not remove traceability or weaken an explicit global requirement without
a recorded deviation decision.

## 1. Document Contract

Every document starts with metadata:

```yaml
spec_id: STORY-001
spec_type: story
status: draft
version: 0.1.0
owner: <role or person>
project_id: <stable project id or none for a reusable global Spec>
updated: YYYY-MM-DD
```

Use the three current types exactly as follows:

| Type | Owns | Must not own |
|------|------|--------------|
| DR | architecture, boundaries, data model, contracts, decisions, risks, cross-story rules | user-story prose, exhaustive test steps, implementation file order |
| Story | one independently deliverable behavior, interface/data flow, edge cases, AC | system-wide architecture decisions or test execution transcripts |
| TestCase | executable scenarios, fixtures, oracle, environment, coverage and evidence | changing the behavior being tested or introducing new requirements |

## 2. Normative Language

- `MUST` and `MUST NOT` are binding requirements.
- `SHOULD` and `SHOULD NOT` are defaults with an explicit deviation reason.
- `MAY` is optional behavior.
- Every normative statement names its actor, condition, and observable result.
- Avoid unbounded adjectives (`fast`, `robust`, `user-friendly`, `scalable`)
  unless followed by a measurable threshold or a referenced constraint.

## 3. IDs and Traceability

Use stable IDs and never recycle an ID after publication:

| Artifact | ID pattern |
|----------|------------|
| Requirement | `REQ-###` |
| Architecture decision | `DEC-###` |
| Story | `STORY-###` |
| Acceptance criterion | `AC-###` |
| Test case | `TC-###` |
| Open question | `Q-###` |

When cross-document context is available, every `AC-*` should reference at
least one `REQ-*` or `STORY-*`, and every `TC-*` should reference an `AC-*`.
These references are recommended trace links, not creation prerequisites.
References are links, not copied paragraphs.

## 4. Evidence and Unknowns

Record the source of each non-obvious fact: repository path, command output,
decision record, user statement, or external contract. Keep four states
distinct:

- **Fact** - directly evidenced and safe to rely on.
- **Decision** - selected option with rejected alternatives and accepted cost.
- **Assumption** - temporary premise that could change the design.
- **Open question** - unresolved choice with an owner and resolution trigger.

An unresolved question that can change this document's scope, data, security,
compatibility, or verification is a blocker for marking this document ready.
Do not hide it in prose.

## 5. Required Behavior Coverage

At the appropriate layer, cover:

- happy path and validation failures;
- authorization and sensitive-data handling;
- dependency timeout/unavailability and retry or compensation;
- idempotency and concurrency for side effects;
- compatibility, migration, rollback, and operational observability;
- non-goals and explicit boundaries.

## 6. Review Gate

A document is `ready` only when the following are true:

1. The global standard, matching template, and project layer are named in a
   context note. Related Specs are listed only when they were provided or
   deliberately consulted.
2. Scope, non-goals, owners, status, and version are present.
3. IDs are unique, trace links are bidirectional where practical, and no copied
   requirement has drifted from its source.
4. Acceptance criteria are observable and point to evidence targets.
5. Contradictions, assumptions, and open questions are visible and dispositioned.
6. The document has a clear intended consumer, output, and verification command
   or review action. A consumer may be a person, code change, test, or another
   optional Spec; it is not a lifecycle dependency.

## 7. Change Protocol

Each Spec owns its declared scope. When a document references another Spec and
finds a contradiction, record the contradiction and the chosen interpretation
explicitly; do not silently rewrite the referenced document or treat the
reference as a prerequisite. Record intentional deviations in a `DEC-*` entry
with impact, owner, and rollback or migration consequence.
