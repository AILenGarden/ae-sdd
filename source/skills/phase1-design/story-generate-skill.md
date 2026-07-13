---
name: story-generate
description: Story 生成 SKILL - Phase 1 ① 节点的环节内具体规则。从 DR 生成完整 Story 文档，按标准输出 StoryGeneratePlan，并触发 Story Review。当用户说"生成 Story"、"写 Story"、"从 DR 生成 Story"、"Story 起草"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/story-generate-skill.full.md
source_fallback_sha256: d17d65aba3ae14b7446049a2eff5b96937f45027e10892e7c19fe40db39160b6
source_original_bytes: 15212
source_original_lines: 248
source_semantic_inventory_sha256: d89df41c94e9bd3d0d2849f49e7b7066bc16eb1cfbb1f776303aaad9c0f9791f
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
- fallback_sha256: `d17d65aba3ae14b7446049a2eff5b96937f45027e10892e7c19fe40db39160b6`
- original_lines: 248
- original_bytes: 15212
- semantic_inventory_sha256: `d89df41c94e9bd3d0d2849f49e7b7066bc16eb1cfbb1f776303aaad9c0f9791f`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Story 生成 SKILL - Phase 1 ① 节点的环节内具体规则。从 DR 生成完整 Story 文档，按标准输出 StoryGeneratePlan，并触发 Story Review。当用户说"生成 Story"、"写 Story"、"从 DR 生成 Story"、"Story 起草"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:40 文档存放前置调用; L2:210 第五步：触发 Story Review; keyword_hits: 13 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:53 整体流程; keyword_hits: 25 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:81 0.2 13 项 prose 自检表（必须全部 ✅）; L3:100 0.3 🔴 机械门禁（保留 v3.9.1 命令，v3.9.3 扩展检查项）; L2:237 禁止事项; keyword_hits: 51 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:100 0.3 🔴 机械门禁（保留 v3.9.1 命令，v3.9.3 扩展检查项）; keyword_hits: 31 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L3:161 3bis.1 字段来源追溯; L3:179 3bis.3 跨文档字段对齐; keyword_hits: 38 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:19 输出边界（最高优先级）; L2:40 文档存放前置调用; L3:179 3bis.3 跨文档字段对齐; +1 more; keyword_hits: 50 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 21; refs: ae-sdd doc resolve --intent STORY --story-id {ID}; ae-sdd doc save --intent REVIEW_COMPARE --work-item {W} --story-id {S} --version "v1-to-v2" --content-file 草稿.md; ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md; +18 more; keyword_hits: 47 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L2:124 第一步 bis：实现方案决策基线; L3:179 3bis.3 跨文档字段对齐; L3:186 3bis.4 设计来源标注（章节级）; keyword_hits: 94 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 17 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

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
| 2 | 15 | 目标 |
| 2 | 19 | 输出边界（最高优先级） |
| 2 | 29 | 依赖标准 |
| 2 | 40 | 文档存放前置调用 |
| 2 | 53 | 整体流程 |
| 2 | 70 | 第零步：准入检查（🆕 v3.9.3 强化为 SSOT 化） |
| 3 | 74 | 0.1 加载 SSOT |
| 3 | 81 | 0.2 13 项 prose 自检表（必须全部 ✅） |
| 3 | 100 | 0.3 🔴 机械门禁（保留 v3.9.1 命令，v3.9.3 扩展检查项） |
| 2 | 110 | 第一步：读取输入（🆕 v3.9.3 强化为引用 SSOT） |
| 2 | 124 | 第一步 bis：实现方案决策基线 |
| 2 | 136 | 第二步：Story 章节生成（🆕 v3.9.3 强化为按映射表生成） |
| 2 | 153 | 第三步：合理性自检 |
| 2 | 157 | 第三步 bis：来源追溯与验证（🆕 v3.9.3 新增） |
| 3 | 161 | 3bis.1 字段来源追溯 |
| 3 | 171 | 3bis.2 不合理入参检测 |
| 3 | 179 | 3bis.3 跨文档字段对齐 |
| 3 | 186 | 3bis.4 设计来源标注（章节级） |
| 2 | 200 | 第四步：写入 Story 文档 |
| 2 | 206 | 第四步 bis：生成 StoryGeneratePlan |
| 2 | 210 | 第五步：触发 Story Review |
| 2 | 214 | 第六步：循环判定 |
| 2 | 218 | 第七步：闸门校验（🆕 v3.9.3 由 8 道闸 → 10 道闸） |
| 2 | 237 | 禁止事项 |

## Inline References

| ref |
| --- |
| ae-sdd doc resolve --intent STORY --story-id {ID} |
| ae-sdd doc save --intent REVIEW_COMPARE --work-item {W} --story-id {S} --version "v1-to-v2" --content-file 草稿.md |
| ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_GENERATE_PLAN --work-item {W} --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_SOURCE_TRACE --work-item {W} --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_WRITER_REPORT --work-item {W} --story-id {S} --content-file 草稿.md |
| assets.md §3/§5/§7 |
| constraints/*.md |
| constraints/assets/DR/PRD/dependsStory/sourceTrace |
| document-storage-skill.md |
| get_constraints/get_assets |
| review-loop-skill.md |
| source/standards/story/story-generation-standard.md §2.5 |
| source/standards/story/story-input-checklist.md |
| story-generate-plan-template.md |
| story-generation-standard.md |
| story-generation-standard.md §4 |
| story-input-checklist.md |
| story-review-skill.md |
| story-template.md |
