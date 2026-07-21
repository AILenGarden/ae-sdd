---
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/story-update-skill.full.md
source_fallback_sha256: 32f3f66a68db5504ca1650c28edf20e0e691b4665d17effba06bf33c63127d65
source_original_bytes: 2231
source_original_lines: 35
source_semantic_inventory_sha256: 85f2689ea70d51589b300472acf03606d40508430f9e715dfd71c109cefab783
source_slimmer: slim_source_skills.py@2
---

# Story Update - Story 文档更新 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/story-update-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/story-update-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/story-update-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/story-update-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/story-update-skill.full.md`
- fallback_sha256: `32f3f66a68db5504ca1650c28edf20e0e691b4665d17effba06bf33c63127d65`
- original_lines: 35
- original_bytes: 2231
- semantic_inventory_sha256: `85f2689ea70d51589b300472acf03606d40508430f9e715dfd71c109cefab783`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 安全更新既有 Story。Document Storage 返回当前模板、撰写指南和 Story 正文；`story_template_sections` 按稳定 section ID 分类变更；本 Skill 不维护章节清单。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | keyword_hits: 1 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:14 更新流程; keyword_hits: 2 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:30 输出与禁止事项; keyword_hits: 5 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 2 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 1 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L1:1 Story Update - Story 文档更新 Skill; L2:30 输出与禁止事项; keyword_hits: 12 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 3; refs: ae-sdd doc resolve --intent STORY --story-id {storyId}; ae-sdd doc save --intent STORY; path/source/content/sha256; keyword_hits: 1 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 2 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L2:24 变更分类; keyword_hits: 7 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 1 | Story Update - Story 文档更新 Skill |
| 2 | 3 | 目标 |
| 2 | 7 | 资源加载 |
| 2 | 14 | 更新流程 |
| 2 | 24 | 变更分类 |
| 2 | 30 | 输出与禁止事项 |

## Inline References

| ref |
| --- |
| ae-sdd doc resolve --intent STORY --story-id {storyId} |
| ae-sdd doc save --intent STORY |
| path/source/content/sha256 |
