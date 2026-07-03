---
name: story-update
description: 根据 Proposal、Story 补充说明文档和 Story 模板更新 Story 主文档。当 Story Review、Coding 或其他渠道发现 Story 缺陷时触发，或开发者说"更新 Story"、"同步补充说明到 Story"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/story-update-skill.full.md
source_fallback_sha256: 8815c0df2758aa371ba5e847c8ad64305aa9d3ecaccaa4c8d17929c80f5bf208
source_original_bytes: 2780
source_original_lines: 72
source_semantic_inventory_sha256: 6cba8709a254e685bfba126b1ef8c044775bda25e1faa563b9fb4f1f288aa1ed
source_slimmer: slim_source_skills.py@2
---

# Story Update — Story 文档更新 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/story-update-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/story-update-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/story-update-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/story-update-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/story-update-skill.full.md`
- fallback_sha256: `8815c0df2758aa371ba5e847c8ad64305aa9d3ecaccaa4c8d17929c80f5bf208`
- original_lines: 72
- original_bytes: 2780
- semantic_inventory_sha256: `6cba8709a254e685bfba126b1ef8c044775bda25e1faa563b9fb4f1f288aa1ed`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 根据 Proposal、Story 补充说明文档和 Story 模板更新 Story 主文档。当 Story Review、Coding 或其他渠道发现 Story 缺陷时触发，或开发者说"更新 Story"、"同步补充说明到 Story"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:19 文档存放前置调用; L2:57 触发下游; keyword_hits: 13 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:29 流程; keyword_hits: 1 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:66 禁止事项; keyword_hits: 9 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 7 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 5 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L1:6 Story Update — Story 文档更新 Skill; L2:19 文档存放前置调用; keyword_hits: 11 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 5; refs: ae-sdd doc save --intent PROPOSAL --work-item {W} --story-id {S} --content-file 草稿.md; ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md; ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md; +2 more; keyword_hits: 12 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 1 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 6 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Story Update — Story 文档更新 Skill |
| 2 | 8 | 目标 |
| 2 | 12 | 依赖标准 |
| 2 | 19 | 文档存放前置调用 |
| 2 | 29 | 流程 |
| 2 | 42 | 核心原则 |
| 2 | 49 | 执行规则 |
| 2 | 57 | 触发下游 |
| 2 | 66 | 禁止事项 |

## Inline References

| ref |
| --- |
| ae-sdd doc save --intent PROPOSAL --work-item {W} --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md |
| document-storage-skill.md |
| proposal-skill.md |
