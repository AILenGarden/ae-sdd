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
ae-sdd memory write --phase <phase> --story <STORY-ID> --summary "..."
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

| Layer | Meaning |
|---|---|
| L0 | session scratch |
| L1 | Story/task memory |
| L2 | project memory |
| L3 | ae-sdd pattern memory |
| L4 | cold archive |

## 4. Required Write Quality

Every L1+ memory entry must include evidence. Acceptable evidence:

- file path and line number
- ae-sdd report path
- user confirmation
- DB tool result
- Git tool result
- test output or XML report
- command output summary

Evidence-free entries stay in L0 only.

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
ae-sdd memory write --phase ra --story STORY-001 --kind decision --summary "..."
ae-sdd memory search --phase coding --story STORY-001 --query "transaction"
ae-sdd memory promote --phase coding-plan --story STORY-001 --from-layer L1 --to-layer L2
ae-sdd memory summarize --phase review --story STORY-001
ae-sdd memory exit --phase ra --story STORY-001
```
