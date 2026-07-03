---
name: story-generate
description: Story 生成 SKILL - Phase 1 ① 节点的环节内具体规则。从 DR 生成完整 Story 文档，按标准输出 StoryGeneratePlan，并触发 Story Review。当用户说"生成 Story"、"写 Story"、"从 DR 生成 Story"、"Story 起草"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/story-generate-skill.full.md
source_fallback_sha256: 7a43a4f4763b357ed2e64276bfe8f2f2f2dd6d803e806d1b72c071a5623145b2
source_original_bytes: 4200
source_original_lines: 125
source_semantic_inventory_sha256: a651e8ac2087b249502d118cf2740e6c9c0f733b24c34f4c1b0892e48bee2da2
source_slimmer: slim_source_skills.py@2
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
- fallback_sha256: `7a43a4f4763b357ed2e64276bfe8f2f2f2dd6d803e806d1b72c071a5623145b2`
- original_lines: 125
- original_bytes: 4200
- semantic_inventory_sha256: `a651e8ac2087b249502d118cf2740e6c9c0f733b24c34f4c1b0892e48bee2da2`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Story 生成 SKILL - Phase 1 ① 节点的环节内具体规则。从 DR 生成完整 Story 文档，按标准输出 StoryGeneratePlan，并触发 Story Review。当用户说"生成 Story"、"写 Story"、"从 DR 生成 Story"、"Story 起草"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:21 文档存放前置调用; L2:106 第五步：触发 Story Review; keyword_hits: 10 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:33 整体流程; keyword_hits: 7 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:118 禁止事项; keyword_hits: 14 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 12 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 2 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:21 文档存放前置调用; L2:98 第四步：写入 Story 文档; keyword_hits: 14 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 11; refs: ae-sdd doc save --intent REVIEW_COMPARE --work-item {W} --story-id {S} --version "v1-to-v2" --content-file 草稿.md; ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md; ae-sdd doc save --intent STORY_GENERATE_PLAN --work-item {W} --story-id {S} --content-file 草稿.md; +8 more; keyword_hits: 26 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L2:70 第一步 bis：实现方案决策基线; keyword_hits: 8 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 4 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Story Generate - 从 DR 生成 Story Skill |
| 2 | 8 | 目标 |
| 2 | 12 | 依赖标准 |
| 2 | 21 | 文档存放前置调用 |
| 2 | 33 | 整体流程 |
| 2 | 48 | 第零步：准入检查 |
| 2 | 61 | 第一步：读取输入 |
| 2 | 70 | 第一步 bis：实现方案决策基线 |
| 2 | 82 | 第二步：Story 章节生成 |
| 2 | 94 | 第三步：合理性自检 |
| 2 | 98 | 第四步：写入 Story 文档 |
| 2 | 102 | 第四步 bis：生成 StoryGeneratePlan |
| 2 | 106 | 第五步：触发 Story Review |
| 2 | 110 | 第六步：循环判定 |
| 2 | 114 | 第七步：闸门校验 |
| 2 | 118 | 禁止事项 |

## Inline References

| ref |
| --- |
| ae-sdd doc save --intent REVIEW_COMPARE --work-item {W} --story-id {S} --version "v1-to-v2" --content-file 草稿.md |
| ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_GENERATE_PLAN --work-item {W} --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_WRITER_REPORT --work-item {W} --story-id {S} --content-file 草稿.md |
| document-storage-skill.md |
| review-loop-skill.md |
| story-generate-plan-template.md |
| story-generation-standard.md |
| story-review-skill.md |
| story-template.md |
