# ae-sdd Phase Memory Layering Standard

## 1. Purpose

Agent-native memory is not enough for ae-sdd. ae-sdd needs project-scoped,
phase-aware, auditable memory that can be cited by Requirement Analysis,
CodingPlan, Coding, and Review nodes.

Memory is a mandatory toolset for associated nodes, not an optional note-taking
feature.

## 2. Mandatory Node Contract

Every associated node must execute this sequence:

```bash
ae-sdd memory enter --phase <phase> --story <STORY-ID>
# run the Skill work
ae-sdd memory write --scope task --phase <phase> --story <STORY-ID> --kind <kind> --summary "<one compact atomic fact>" --evidence <file:line>
ae-sdd memory exit --phase <phase> --story <STORY-ID>
```

`memory exit` is a gate. It fails when no `memory write` happened after the
latest `memory enter` for the same phase/story/task scope.

Starting in v3.2.3, this is also enforced by the state transition layer.
`ae-sdd state write --phase <next>` checks the memory lifecycle before leaving
associated phases. The transition is blocked when the current phase has no
matching `memory enter` and later `memory write`.

## 3. Associated Nodes

| Node | Phase | Mandatory Before Work | Mandatory After Work |
|---|---|---|---|
| requirement-analysis-skill | `ra` | read RA/design/project memory | write RA decisions, gaps, assumptions |
| dr-generate / story-generate / story-review | `design` | read RA/design memory | write design decisions and unresolved conflicts |
| task-generate / CodingSkill.Plan | `coding-plan` | read RA/design/coding-plan memory | write architecture decisions, risk choices, task constraints |
| CodingSkill.Execute | `coding` | read coding-plan/coding memory | write compile/test/runtime findings and fixes |
| coding-report / code-review | `review` | read coding/review memory | write defects, residual risks, reusable lessons |

## 4. Partitions and Layers

Agent-facing memory has two primary partitions:

| Scope | Legacy Layer | Content | Default Storage |
|---|---|---|---|
| task | L1 | phase decisions for a Story or Task | `.ae-sdd/memory/story/{story}/...` |
| project | L2 | reusable project constraints and lessons | `.ae-sdd/memory/project/*.jsonl` |

Default write target is `--scope task`. `--scope project` is only for facts that
remain true across tasks. Task memory must not be read by unrelated tasks; cross
task reuse must happen through project memory.

Auxiliary layers remain available for lifecycle management:

| Layer | Name | Content | Default Storage |
|---|---|---|---|
| L0 | Session scratch | transient observations, command summaries | `.ae-sdd/memory/session/*.jsonl` |
| L1 | Story/task memory | phase decisions for a Story or Task | `.ae-sdd/memory/story/{story}/...` |
| L2 | Project memory | reusable project constraints and lessons | `.ae-sdd/memory/project/*.jsonl` |
| L3 | ae-sdd pattern memory | cross-project patterns and anti-patterns | `.ae-sdd/memory/global-patterns/*.jsonl` |
| L4 | Cold archive | postmortems, historical reports, ADR links | `.ae-sdd/memory/archive/*.jsonl` |

## 5. Promotion Rules

- L0 may be deleted after the session.
- L1 may be promoted to L2 only when it has concrete evidence.
- L2 may be promoted to L3 only after repeated cross-project occurrence or user
  approval.
- L3 changes require ae-sdd versioned changelog entries.
- L4 is read-mostly and should not be injected wholesale into context.

## 6. Conflict Rules

If new evidence conflicts with existing memory:

1. Do not overwrite the old memory silently.
2. Write a new memory entry with `kind=conflict`.
3. Cite both sources.
4. Keep downstream conclusions blocked or marked unverified until resolved.

## 7. Minimum Memory Content

Memory is a compact context index, not a process log. Each task/project memory write is
one atomic fact that can change a later Agent decision.

Hard compact budgets:

| Scope | Legacy Layer | Summary budget | Evidence |
|---|---|---:|---|
| scratch | L0 | <= 240 chars | optional |
| task | L1 | <= 180 chars | 1-3 short references required |
| project | L2 | <= 140 chars | 1-3 short references required |
| pattern | L3 | <= 120 chars | 1-3 short references required plus changelog/user approval |
| archive | L4 | <= 180 chars | references required for non-scratch entries |

Write rules:

- One line only: no Markdown headings, bullet lists, code fences, stack traces,
  copied command output, or multi-paragraph summaries.
- One memory entry = one decision, constraint, finding, issue, risk, fix, or
  conflict. Split unrelated facts into separate entries.
- Evidence is a pointer such as `file:line`, report path, test name, command
  summary, DB result id, Git commit, or user confirmation. Do not paste source
  text or output into memory.
- Project/pattern entries must not use `kind=observation`; promote only reusable facts.
- If a fact is useful only during the current tool call, keep it in scratch.

Each memory write should include:

- phase
- story/task scope
- summary
- kind: decision, constraint, finding, issue, risk, fix, conflict, observation
- evidence references
- timestamp and actor

Evidence-free memory is allowed only in L0 scratch. L1+ memory should cite a
file, command, report, user confirmation, DB result, Git result, or test output.
