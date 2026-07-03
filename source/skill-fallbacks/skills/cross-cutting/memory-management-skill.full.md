---
name: memory-management
description: Phase-aware ae-sdd memory management. Mandatory for associated RA, design, CodingPlan, Coding, and Review nodes. Provides enter/write/exit/read/search/promote workflow and layered memory policy.
---

# Memory Management Skill

## 1. Core Rule

Memory is mandatory on associated nodes.

Before a node starts, it must load phase memory:

```bash
ae-sdd memory enter --phase <phase> --story <STORY-ID>
```

After the Agent outputs the node result, it must write memory:

```bash
ae-sdd memory write --scope task --phase <phase> --story <STORY-ID> --kind <kind> --summary "<one compact atomic fact>" --evidence <file:line>
```

Before leaving the node, it must run:

```bash
ae-sdd memory exit --phase <phase> --story <STORY-ID>
```

If `memory exit` fails, the node is not complete.

v3.2.3 adds automatic transition enforcement: `ae-sdd state write --phase <next>`
checks the current associated node's memory lifecycle before changing phase. A
missing `memory enter` or missing later `memory write` blocks the phase switch.

## 2. Associated Nodes

| Node | Phase | Required Memory Read | Required Memory Write |
|---|---|---|---|
| requirement-analysis-skill | `ra` | RA, design, project memory | decisions, gaps, assumptions, user-confirmed facts |
| dr-generate / story-generate / story-review | `design` | RA and design memory | design choices, conflicts, unresolved issues |
| task-generate / CodingSkill.Plan | `coding-plan` | RA, design, coding-plan memory | architecture decisions, task constraints, risk decisions |
| CodingSkill.Execute | `coding` | coding-plan and coding memory | compile/test/runtime findings, fixes, evidence |
| coding-report / code-review | `review` | coding and review memory | defects, residual risk, reusable lessons |

## 3. Layers

See [`memory-layering.md`](../../standards/toolsets/memory-layering.md).

Agent-facing memory has two primary partitions:

| Scope | Legacy layer | Meaning | Default use |
|---|---|---|---|
| `task` | L1 | Story/task memory | Default write target for node work |
| `project` | L2 | Project memory | Reusable constraints and lessons only |

Auxiliary storage remains available but must not be the normal Agent mental model:

| Scope | Legacy layer | Meaning |
|---|---|---|
| `scratch` | L0 | session scratch |
| `pattern` | L3 | ae-sdd pattern memory |
| `archive` | L4 | cold archive |

Default rule: write task memory first. Promote to project memory only when the
fact is reusable across tasks and has concrete evidence.

## 4. Required Write Quality

Memory is a compact context index, not a report, log, or retrospective.

Every L1+ memory entry must be one compact atomic fact:

- one line only; no Markdown headings, bullet lists, code fences, or copied output
- `summary` hard budgets: task <= 180 chars, project <= 140 chars, pattern <= 120 chars
- 1-3 short evidence references; cite pointers, do not paste source text/output
- write only facts that can change the next Agent decision
- split unrelated decisions/fixes/risks into separate memory entries

Allowed `kind` values:

| kind | Use |
|---|---|
| `decision` | accepted design or implementation decision |
| `constraint` | rule the next Agent must obey |
| `finding` | verified fact from code, test, DB, Git, or user input |
| `issue` | unresolved problem that blocks or changes work |
| `risk` | known future failure mode |
| `fix` | completed repair worth remembering |
| `conflict` | new evidence conflicts with old memory |
| `observation` | L0 scratch only by default |

Good compact examples:

```bash
ae-sdd memory write --scope task --phase coding --story STORY-001 --kind fix --summary "UserMapper query now filters by tenant_id to prevent cross-tenant reads." --evidence src/main/resources/mapper/UserMapper.xml:31
ae-sdd memory write --scope project --phase coding --kind constraint --summary "Order amount uses BigDecimal; double/float are forbidden for money math." --evidence source/standards/constraints/code-style.md:42
ae-sdd memory write --scope task --phase review --story STORY-001 --kind risk --summary "Default Windows subprocess text capture may fail on UTF-8 Chinese unless PYTHONUTF8=1 is set." --evidence tools/tests/test_memory_gate.py:83
```

Bad memory examples:

- "Today coding was completed and the process went smoothly..."
- pasted test output or stack traces
- multi-paragraph summaries
- evidence-free L1/L2 claims

Every L1+ memory entry must include evidence. Acceptable evidence:

- file path and line number
- ae-sdd report path
- user confirmation
- DB tool result
- Git tool result
- test output or XML report
- command output summary

Evidence-free entries stay in scratch only. Project/pattern memory must not use
`kind=observation`; promote only reusable decisions, constraints, findings,
risks, fixes, issues, or conflicts.

## 5. Conflict Handling

When new evidence conflicts with memory:

1. Write `kind=conflict`.
2. Cite old and new evidence.
3. Mark downstream conclusions unverified until resolved.
4. Do not silently overwrite earlier memory.

## 6. CLI Contract

```bash
ae-sdd memory enter --phase ra --story STORY-001
ae-sdd memory read --phase ra --story STORY-001
ae-sdd memory write --scope task --phase ra --story STORY-001 --kind decision --summary "<one compact atomic fact>" --evidence <file:line>
ae-sdd memory write --scope project --phase coding --kind constraint --summary "<project-wide fact>" --evidence <file:line>
ae-sdd memory read --scope task --phase coding --story STORY-001
ae-sdd memory read --scope project --phase coding
ae-sdd memory search --phase coding --story STORY-001 --query "transaction"
ae-sdd memory promote --phase coding-plan --story STORY-001 --from-scope task --to-scope project
ae-sdd memory summarize --phase review --story STORY-001
ae-sdd memory exit --phase ra --story STORY-001
```
