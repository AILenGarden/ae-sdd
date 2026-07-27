---
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/story-generate-skill.full.md
source_fallback_sha256: 54489dd7fa67e5151863adbe4f7c29f7c16e9595f5826c64565fe0f4868545e1
source_original_bytes: 3725
source_original_lines: 78
source_semantic_inventory_sha256: f0e67ed4b957004afe1e853bfac07f1dbe3336caaa29a66adc310425e9d5b31c
---

# Story Generate - 从 DR 生成 Story Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/story-generate-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/story-generate-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/story-generate-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/story-generate-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/story-generate-skill.full.md`
- fallback_sha256: `54489dd7fa67e5151863adbe4f7c29f7c16e9595f5826c64565fe0f4868545e1`
- original_lines: 78
- original_bytes: 3725
- semantic_inventory_sha256: `f0e67ed4b957004afe1e853bfac07f1dbe3336caaa29a66adc310425e9d5b31c`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 从已确认的上游输入生成结构化 Story。模板定义结构，撰写指南定义写法，Document Storage 定位并读取资源，`story_template_sections` 负责章节解析；本 Skill 只负责阶段编排。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | keyword_hits: 7 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:24 流程; keyword_hits: 3 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:72 禁止事项; keyword_hits: 13 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 5 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 1 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | headings: L2:7 输出边界; keyword_hits: 14 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 3; refs: ae-sdd doc resolve --intent STORY; path/source/content/sha256; state.review.status/findings; keyword_hits: 1 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 3 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 1 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 1 | Story Generate - 从 DR 生成 Story Skill |
| 2 | 3 | 目标 |
| 2 | 7 | 输出边界 |
| 2 | 13 | 资源加载 |
| 2 | 24 | 流程 |
| 3 | 26 | 0. 输入准入 |
| 3 | 35 | 1. 方案决策基线 |
| 3 | 39 | 2. 主要章节草稿 |
| 3 | 51 | 3. 副章节派生 |
| 3 | 55 | 4. 写入与循环 |
| 2 | 62 | 验收 |
| 2 | 72 | 禁止事项 |

## Inline References

| ref |
| --- |
| ae-sdd doc resolve --intent STORY |
| path/source/content/sha256 |
| state.review.status/findings |
