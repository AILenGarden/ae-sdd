---
name: memory-management
description: Phase-aware ae-sdd memory management. Mandatory for associated RA, design, CodingPlan, Coding, and Review nodes. Provides enter/write/exit/read/search/promote workflow and layered memory policy.
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/memory-management-skill.full.md
source_fallback_sha256: 03ac23650b6ac5ca9b7aa954b9eb21d2539f22af83d06c87654d01971ae08b50
source_original_bytes: 5043
source_original_lines: 132
source_semantic_inventory_sha256: d8e56aac8b74bcc5d45a491a755a0a59fd80ee4e69c929b85a83fda12ea9e963
source_slimmer: slim_source_skills.py@2
---

# Memory Management Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/memory-management-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/memory-management-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/memory-management-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/memory-management-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/memory-management-skill.full.md`
- fallback_sha256: `03ac23650b6ac5ca9b7aa954b9eb21d2539f22af83d06c87654d01971ae08b50`
- original_lines: 132
- original_bytes: 5043
- semantic_inventory_sha256: `d8e56aac8b74bcc5d45a491a755a0a59fd80ee4e69c929b85a83fda12ea9e963`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Phase-aware ae-sdd memory management. Mandatory for associated RA, design, CodingPlan, Coding, and Review nodes. Provides enter/write/exit/read/search/promote workflow and layered memory policy.

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 23 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | keyword_hits: 15 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L2:121 6. CLI Contract; keyword_hits: 19 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 22 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | keyword_hits: 4 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 2; refs: ae-sdd state write --phase <next>; memory-layering.md; keyword_hits: 6 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 1 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Memory Management Skill |
| 2 | 8 | 1. Core Rule |
| 2 | 36 | 2. Associated Nodes |
| 2 | 46 | 3. Layers |
| 2 | 58 | 4. Required Write Quality |
| 2 | 112 | 5. Conflict Handling |
| 2 | 121 | 6. CLI Contract |

## Inline References

| ref |
| --- |
| ae-sdd state write --phase <next> |
| memory-layering.md |
