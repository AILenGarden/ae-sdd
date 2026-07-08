---
name: story-review
description: 根据 DR + PRD + 产品原型 + Story 模板审查 Story，记录缺陷、存疑项和误报，先写 Supplement，再触发 Proposal，禁止生成旧版计划载体。当开发者说"审查 Story"、"检查 Story"、"优化 Story"、"Story 缺陷挖掘"、"Story Review"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/story-review-skill.full.md
source_fallback_sha256: c9b85e0799ad49a323eb45620e2f1ad010001a3d0eee580d531780fae2fadb04
source_original_bytes: 4884
source_original_lines: 112
source_semantic_inventory_sha256: c42b5f001e1e4fc240ea016e1231a04bfa8c9bc556fdb9ec4ae20ced4795a00c
source_slimmer: slim_source_skills.py@2
---

# Story Review — Story 缺陷挖掘 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/story-review-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/story-review-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/story-review-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/story-review-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/story-review-skill.full.md`
- fallback_sha256: `c9b85e0799ad49a323eb45620e2f1ad010001a3d0eee580d531780fae2fadb04`
- original_lines: 112
- original_bytes: 4884
- semantic_inventory_sha256: `c42b5f001e1e4fc240ea016e1231a04bfa8c9bc556fdb9ec4ae20ced4795a00c`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 根据 DR + PRD + 产品原型 + Story 模板审查 Story，记录缺陷、存疑项和误报，先写 Supplement，再触发 Proposal，禁止生成旧版计划载体。当开发者说"审查 Story"、"检查 Story"、"优化 Story"、"Story 缺陷挖掘"、"Story Review"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:20 文档存放前置调用; keyword_hits: 10 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:60 流程; keyword_hits: 4 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:106 禁止事项; keyword_hits: 31 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 13 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 3 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:20 文档存放前置调用; L2:95 输出要求; keyword_hits: 16 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 19; refs: *DR*.md; *PRD*.md; *prd*.md; +16 more; keyword_hits: 26 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 1 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
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
| 1 | 6 | Story Review — Story 缺陷挖掘 Skill |
| 2 | 8 | 目标 |
| 2 | 12 | 依赖标准 |
| 2 | 20 | 文档存放前置调用 |
| 2 | 31 | 第零步：Story Review 准入检查 |
| 2 | 60 | 流程 |
| 2 | 73 | 核心原则 |
| 2 | 82 | A-E / F 检查口径 |
| 2 | 95 | 输出要求 |
| 2 | 102 | 退出条件 |
| 2 | 106 | 禁止事项 |

## Inline References

| ref |
| --- |
| *DR*.md |
| *PRD*.md |
| *prd*.md |
| *需求*.md |
| ae-sdd assets read story-review --project <projectKey> |
| ae-sdd doc resolve --intent DR |
| ae-sdd doc resolve --intent STORY/STORY_SUPPLEMENT --story-id {S} |
| ae-sdd doc save --intent PROPOSAL --work-item {W} --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent REVIEW_COMPARE --work-item {W} --story-id {S} --version "v1-to-v2" --content-file 草稿.md |
| ae-sdd doc save --intent STORY_REVIEW --work-item {W} --story-id {S} --version "r1" --content-file 草稿.md |
| ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md |
| constraints/*.md |
| constraints/assets/DR/PRD |
| design/ |
| document-storage-skill.md |
| get_constraints/get_assets |
| proposal-skill.md |
| review-loop-skill.md |
| standards/story/story-review-checklist.md |
