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
ae-sdd memory write --phase <phase> --story <STORY-ID> --summary "..."
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

## 4. Layers

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

Each memory write should include:

- phase
- story/task scope
- summary
- kind: decision, finding, issue, risk, fix, conflict, observation
- evidence references
- timestamp and actor

Evidence-free memory is allowed only in L0 scratch. L1+ memory should cite a
file, command, report, user confirmation, DB result, Git result, or test output.
