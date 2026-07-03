---
name: coding-report
description: Coding 报告产出 SKILL — Phase 2 ⑤ Coding 完成后的报告产出环节。统一规范 Coding 报告的章节结构、变更文件清单、编译/测试结果、已知问题、异常路径触发条件。与 `be-coding-report-template.md` 配套使用。🆕 2026-06-06 新建，填补 AE 体系 Phase 2 ⑤ SKILL 缺口。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase2-coding/coding-report-skill.full.md
source_fallback_sha256: 0a6ef12337b2703190070ee8cdd662088a6dbaa5ce4135851f221c5cc757746d
source_original_bytes: 13224
source_original_lines: 317
source_semantic_inventory_sha256: c0f2a734724fe41c3e38dcba872fb948a4829940a289b20449751381544bf038
source_slimmer: slim_source_skills.py@2
---

# Coding Report — Coding 报告产出 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase2-coding/coding-report-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase2-coding/coding-report-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase2-coding/coding-report-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase2-coding/coding-report-skill.md`
- fallback: `skill-fallbacks/skills/phase2-coding/coding-report-skill.full.md`
- fallback_sha256: `0a6ef12337b2703190070ee8cdd662088a6dbaa5ce4135851f221c5cc757746d`
- original_lines: 317
- original_bytes: 13224
- semantic_inventory_sha256: `c0f2a734724fe41c3e38dcba872fb948a4829940a289b20449751381544bf038`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Coding 报告产出 SKILL — Phase 2 ⑤ Coding 完成后的报告产出环节。统一规范 Coding 报告的章节结构、变更文件清单、编译/测试结果、已知问题、异常路径触发条件。与 `be-coding-report-template.md` 配套使用。🆕 2026-06-06 新建，填补 AE 体系 Phase 2 ⑤ SKILL 缺口。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:18 📦 文档存放前置调用（🔴 横切依赖）; L2:102 触发条件; L3:227 §九 异常路径触发（如有）; +1 more; keyword_hits: 30 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:69 整体流程; keyword_hits: 10 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:48 标尺 1：客观性（🔴 禁止主观描述）; L2:269 禁止事项; keyword_hits: 26 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 15 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 13 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L1:6 Coding Report — Coding 报告产出 Skill; L2:18 📦 文档存放前置调用（🔴 横切依赖）; L2:46 Coding Report 总则; +1 more; keyword_hits: 51 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 22; refs: SKILL.md; ae-sdd assets read coding --project <projectKey>; ae-sdd doc resolve --intent STORY --story-id {S}; +19 more; keyword_hits: 31 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:126 §一 元信息; L3:137 §二 本轮变更文件清单（🔴 按项目 §3 层级自上而下）; L3:147 §三 编译 + 服务启动结果; +6 more; keyword_hits: 50 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L3:137 §二 本轮变更文件清单（🔴 按项目 §3 层级自上而下）; keyword_hits: 16 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Coding Report — Coding 报告产出 Skill |
| 2 | 18 | 📦 文档存放前置调用（🔴 横切依赖） |
| 2 | 35 | 目标 |
| 2 | 46 | Coding Report 总则 |
| 3 | 48 | 标尺 1：客观性（🔴 禁止主观描述） |
| 3 | 57 | 标尺 2：完整性（🔴 必填章节不能空） |
| 3 | 61 | 标尺 3：可追溯性（🔴 每个声明附证据） |
| 2 | 69 | 整体流程 |
| 2 | 102 | 触发条件 |
| 2 | 111 | 第一步：读取输入 |
| 2 | 124 | 第二步：填充 9 章节内容 |
| 3 | 126 | §一 元信息 |
| 3 | 137 | §二 本轮变更文件清单（🔴 按项目 §3 层级自上而下） |
| 3 | 147 | §三 编译 + 服务启动结果 |
| 3 | 161 | §四 测试结果 |
| 3 | 191 | §五 已知问题（🔴 AI 自报，不藏不报） |
| 3 | 199 | §六 产出物对账 |
| 3 | 209 | §七 闸门自检 |
| 3 | 221 | §八 下一步建议 |
| 3 | 227 | §九 异常路径触发（如有） |
| 2 | 242 | 第三步：合理性自检 |
| 2 | 253 | 第四步：生成 Coding 报告 |
| 2 | 261 | 第五步：触发下游 SKILL |
| 2 | 269 | 禁止事项 |
| 2 | 282 | 执行清单 |
| 2 | 303 | 维护 |

## Inline References

| ref |
| --- |
| SKILL.md |
| ae-sdd assets read coding --project <projectKey> |
| ae-sdd doc resolve --intent STORY --story-id {S} |
| ae-sdd doc save |
| ae-sdd doc save --intent CODING_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent CODING_REPORT --work-item {W} --story-id {S?} --version "v1-r1" --content-file 草稿.md |
| ae-sdd doc save --intent TEST_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent TEST_REPORT --work-item {W} --story-id {S?} --version "v1-r1" --content-file 草稿.md |
| ae-sdd doc save --intent TRACE_MATRIX --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent TRACE_MATRIX --work-item {W} --story-id {S?} --version "v1-r1" --content-file 草稿.md |
| archive/{date}/ |
| be-coding-report-template.md |
| code-review-skill.md |
| coding-report-skill.md |
| coding-skill.md |
| curl /actuator/health |
| document-storage-skill.md |
| java -jar {bff-module}/target/*.jar |
| templates/coding/be-coding-report-template.md |
| {STORY-ID}-CodingPlan.md |
| {STORY-ID}-CodingReport-v{N}-r{M}.md |
| {STORY-ID}-Report-v{N}-r{M}.md |
