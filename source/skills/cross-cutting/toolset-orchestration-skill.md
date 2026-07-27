---
name: toolset-orchestration
description: ae-sdd Toolset Layer governance. Defines how DB, Git, Memory, and future project-aware adapters are called by Skills and Gates.
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/toolset-orchestration-skill.full.md
source_fallback_sha256: 869463f66da411dec2add73c78d448167fabc9928b889137be74917779ab5b63
source_original_bytes: 1786
source_original_lines: 55
source_semantic_inventory_sha256: 1340b3770323af60679f5b5b0f3e018879b551e1b88a29bf28314b9f5790dfe0
---

# Toolset Orchestration Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/toolset-orchestration-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/toolset-orchestration-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/toolset-orchestration-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/toolset-orchestration-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/toolset-orchestration-skill.full.md`
- fallback_sha256: `869463f66da411dec2add73c78d448167fabc9928b889137be74917779ab5b63`
- original_lines: 55
- original_bytes: 1786
- semantic_inventory_sha256: `1340b3770323af60679f5b5b0f3e018879b551e1b88a29bf28314b9f5790dfe0`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: ae-sdd Toolset Layer governance. Defines how DB, Git, Memory, and future project-aware adapters are called by Skills and Gates.

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 11 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | keyword_hits: 9 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 10 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 10 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | keyword_hits: 1 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 8; refs: ae-sdd db ...; ae-sdd git ...; ae-sdd memory ...; +5 more; keyword_hits: 6 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Toolset Orchestration Skill |
| 2 | 8 | 1. Positioning |
| 2 | 22 | 2. P0 Toolsets |
| 2 | 30 | 3. Mandatory Memory Rule |
| 1 | 36 | node work |
| 2 | 45 | 4. Evidence Rule |
| 2 | 51 | 5. Security Rule |

## Inline References

| ref |
| --- |
| ae-sdd db ... |
| ae-sdd git ... |
| ae-sdd memory ... |
| ae-sdd state write --phase <next> |
| database-tool-skill.md |
| git-insight-skill.md |
| memory-management-skill.md |
| toolset-security.md |
