---
name: toolset-orchestration
description: ae-sdd Toolset Layer governance. Defines how DB, Git, Memory, and future project-aware adapters are called by Skills and Gates.
---

# Toolset Orchestration Skill

## 1. Positioning

Toolset Layer is separate from Harness.

| Layer | Responsibility |
|---|---|
| Harness | permit/deny Agent tool execution by phase |
| Skill | decide workflow and output quality rules |
| Gate | block invalid phase transitions |
| Toolset | collect project evidence through stable ae-sdd commands |

Toolsets are not optional helpers. When a Skill declares a toolset dependency,
that dependency becomes part of the node contract.

## 2. P0 Toolsets

| Toolset | Skill | CLI | Status |
|---|---|---|---|
| Memory | `memory-management-skill.md` | `ae-sdd memory ...` | mandatory for associated nodes |
| Database | `database-tool-skill.md` | `ae-sdd db ...` | read-first skeleton |
| Git Insight | `git-insight-skill.md` | `ae-sdd git ...` | read-only skeleton |

## 3. Extended Toolsets

Extended toolsets are optional, project-aware adapters that do not gate node
completion. They follow the same
[`toolset-security.md`](../../standards/toolsets/toolset-security.md) rules
as P0 toolsets.

| Toolset | Skill | CLI | Status |
|---|---|---|---|
| Postman | `postman-tool-skill.md` | MCP (`mcp__postman__*`) | optional L4 supplemental HTTP verification |

Postman is the first MCP-backed toolset: it is invoked through the
PostmanMCP installed in the Agent, not through an `ae-sdd` CLI subcommand. Its
evidence is `http-external-supplemental` and does not count toward the L2/L4
dual-stage completion owned by `test-generate-skill.md`.

## 4. Mandatory Memory Rule

Associated nodes must run:

```bash
ae-sdd memory enter --phase <phase> --story <STORY-ID>
# node work
ae-sdd memory write --phase <phase> --story <STORY-ID> --summary "..."
ae-sdd memory exit --phase <phase> --story <STORY-ID>
```

`memory exit` is a hard gate. If it fails, the node is incomplete.
From v3.2.3 onward, `ae-sdd state write --phase <next>` also runs this memory
gate automatically before leaving an associated phase.

## 5. Evidence Rule

Tool outputs used by RA, CodingPlan, CodingReport, or CodeReview must be cited as
evidence. If a tool cannot collect evidence, downstream conclusions must say
`unverified` instead of guessing.

## 6. Security Rule

All toolsets must follow
[`toolset-security.md`](../../standards/toolsets/toolset-security.md).
