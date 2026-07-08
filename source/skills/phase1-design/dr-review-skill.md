---
name: dr-review
description: DR Review SKILL — 对 DR 草稿进行 5 阶段评审，输出 DR Review 报告 + DR Review UpdatePlan。当用户说"DR 评审"/"DR Review"/"检查 DR"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/dr-review-skill.full.md
source_fallback_sha256: 6a742d69c2a620450ee23cdc5b968e5136ea043d81368d8a7334104c31038307
source_original_bytes: 66567
source_original_lines: 1237
source_semantic_inventory_sha256: bf1fdceb7bd4ca194470da2d105f04e0e2813eb78755bb340b86caedaa17799f
source_slimmer: slim_source_skills.py@2
---

# DR Review — DR 文档评审 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/dr-review-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/dr-review-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/dr-review-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/dr-review-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/dr-review-skill.full.md`
- fallback_sha256: `6a742d69c2a620450ee23cdc5b968e5136ea043d81368d8a7334104c31038307`
- original_lines: 1237
- original_bytes: 66567
- semantic_inventory_sha256: `bf1fdceb7bd4ca194470da2d105f04e0e2813eb78755bb340b86caedaa17799f`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: DR Review SKILL — 对 DR 草稿进行 5 阶段评审，输出 DR Review 报告 + DR Review UpdatePlan。当用户说"DR 评审"/"DR Review"/"检查 DR"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:28 📦 文档存放前置调用（🔴 横切依赖）; L3:285 1.4 调用项目资产服务; L4:541 C2 接口路径与契约入口对齐; +3 more; keyword_hits: 66 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:127 整体流程; L3:346 与 5 阶段并行挖掘的关系（引用 §8.4.6）; L2:352 第二步：5 阶段评审（并行挖掘）; +7 more; keyword_hits: 136 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:14 🟠 门禁强度声明（v3.5.11 AA 诚实降级 / v3.9.1 部分硬化）; L3:50 标尺 1：证据标准（🔴 禁止裸结论）; L3:84 标尺 3：完整性度量（🔴 "覆盖所有"必须先有穷举清单）; +7 more; keyword_hits: 128 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:519 阶段 C：接口契约完整性; L4:534 C1 场景到接口映射; L4:541 C2 接口路径与契约入口对齐; +2 more; keyword_hits: 94 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L4:609 D2 表结构与项目资产字段索引一致; L2:1026 4. 字段链路与数据模型修订计划; keyword_hits: 69 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L1:6 DR Review — DR 文档评审 Skill; L2:28 📦 文档存放前置调用（🔴 横切依赖）; L3:265 1.2 读取 RA 文档; +8 more; keyword_hits: 97 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 29; refs: CR-DR-XXX-vN.m-UpdatePlan.md; CR-DR-XXX-vN.m.md; CR/; +26 more; keyword_hits: 44 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L2:318 第一步 bis：多 reviewer 视角切分（🆕 2026-06-25 — 落地 §8.4.2 节点专属配置）; L3:322 Tier 判定（引用 §8.4.1，本节点关键决策点识别已有支撑）; L3:334 DR Review reviewer 视角分工（§8.4.2 三原则落地）; +8 more; keyword_hits: 267 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 31 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | DR Review — DR 文档评审 Skill |
| 2 | 8 | 目标 |
| 2 | 14 | 🟠 门禁强度声明（v3.5.11 AA 诚实降级 / v3.9.1 部分硬化） |
| 2 | 28 | 📦 文档存放前置调用（🔴 横切依赖） |
| 2 | 46 | DR Review 总则（🔴 贯穿全 SKILL，违反 = 结论无效） |
| 3 | 50 | 标尺 1：证据标准（🔴 禁止裸结论） |
| 3 | 68 | 标尺 2：强弱分级（🔴 标尺贯穿，混级 = 风险被稀释） |
| 3 | 84 | 标尺 3：完整性度量（🔴 "覆盖所有"必须先有穷举清单） |
| 3 | 107 | 标尺 4：语言精确性（🔴 禁用纯主观词） |
| 2 | 127 | 整体流程 |
| 2 | 160 | Plan-first 更新原则（🔴 DR 修改硬前置） |
| 2 | 183 | 第零步：DR Review 准入检查（🔴 硬门禁，未通过禁止进入 Review） |
| 2 | 224 | 准入检查记录 |
| 2 | 234 | 第一步：读取输入 |
| 3 | 243 | 1.1 读取 DR 草稿 |
| 3 | 265 | 1.2 读取 RA 文档 |
| 3 | 274 | 1.3 读取 PRD 文档 |
| 3 | 285 | 1.4 调用项目资产服务 |
| 3 | 308 | 1.5 读取 DR 模板 |
| 3 | 312 | 1.6 读取已有 Review 记录（仅作上下文参考，不得作为跳过依据） |
| 2 | 318 | 第一步 bis：多 reviewer 视角切分（🆕 2026-06-25 — 落地 §8.4.2 节点专属配置） |
| 3 | 322 | Tier 判定（引用 §8.4.1，本节点关键决策点识别已有支撑） |
| 3 | 334 | DR Review reviewer 视角分工（§8.4.2 三原则落地） |
| 3 | 346 | 与 5 阶段并行挖掘的关系（引用 §8.4.6） |
| 2 | 352 | 第二步：5 阶段评审（并行挖掘） |
| 3 | 366 | 阶段 A：业务价值与目标对齐 |
| 4 | 370 | A0 应审查对象清单（🔴 完整性度量基线，必须先列） |
| 4 | 389 | A1 设计目标与 RA 业务全景一致性 |
| 4 | 398 | A2 业务场景覆盖 |
| 4 | 407 | A3 关键决策与 RA 设计方向论证对齐 |
| 4 | 415 | A4 风险识别与 RA 隐性假设 |
| 3 | 447 | 阶段 B：架构合理性与技术债务 |
| 4 | 451 | B0 应审查对象清单 |
| 4 | 464 | B1 架构概览与项目资产对齐 |
| 4 | 473 | B2 约束承接完整性 |
| 4 | 481 | B3 关键决策质量 |
| 4 | 489 | B4 技术债务评估 |
| 4 | 498 | B5 测试策略 |
| 3 | 519 | 阶段 C：接口契约完整性 |
| 4 | 523 | C0 应审查对象清单 |
| 4 | 534 | C1 场景到接口映射 |
| 4 | 541 | C2 接口路径与契约入口对齐 |
| 4 | 549 | C3 幂等性/版本号/限流标注 |
| 4 | 557 | C4 异常/边界场景错误码映射 |
| 4 | 565 | C5 下游 Story 接口契约一致性 |
| 3 | 586 | 阶段 D：数据模型与不变量 |
| 4 | 590 | D0 应审查对象清单 |
| 4 | 601 | D1 RA 数据要素覆盖 |
| 4 | 609 | D2 表结构与项目资产字段索引一致 |
| 4 | 617 | D3 业务不变量与约束 |
| 4 | 625 | D4 索引/分库分表/迁移 |
| 4 | 634 | D5 数据生命周期与所有权 |
| 3 | 656 | 阶段 E：Story 拆分合理性 |
| 4 | 660 | E0 应审查对象清单 |
| 4 | 671 | E1 Story 边界清晰 |
| 4 | 679 | E2 依赖关系图完整 |
| 4 | 688 | E3 优先级合理 |
| 4 | 695 | E4 Story 规模均匀 |
| 4 | 702 | E5 Story 可独立验收 |
| 4 | 710 | E6 Story 与接口契约对齐 |
| 3 | 730 | 第二步整体结论 |
| 2 | 748 | DR Review 汇总输出模板 |
| 2 | 753 | DR Review 结论：{DR-ID} |
| 3 | 760 | 问题汇总 |
| 3 | 768 | 建议改进（非阻塞，不影响进入第三步） |
| 3 | 775 | 通过标准（门禁） |
| 2 | 787 | 第三步：缺陷汇总（合理性判定 + 漏报升级） |
| 3 | 791 | 3.1 判定流程 |
| 3 | 815 | 3.2 判定结果分类（🔴 必须带归属标签） |
| 3 | 840 | 3.3 常见误报场景 |
| 3 | 850 | 3.4 RA-DEFECT 闭环触发流程（🆕 v3.2 加固 — 2026-06-24） |
| 4 | 854 | 3.4.1 触发条件 |
| 4 | 858 | 3.4.2 闭环 SOP（5 步强制） |
| 4 | 886 | 3.4.3 闭环产物清单（🔴 必填） |
| 4 | 897 | 3.4.4 闭环审计行模板 |
| 2 | 900 | RA-DEFECT 闭环审计 - {RA-ID} - {YYYY-MM-DD} |
| 4 | 917 | 3.4.5 与 requirement-analysis-skill §RA 修订影响分析的联动 |
| 2 | 941 | 第四步：生成 DR Review UpdatePlan |
| 3 | 945 | 4.1 UpdatePlan 模板 |
| 1 | 948 | DR Review UpdatePlan — {DR-ID} — {YYYY-MM-DD} |
| 2 | 955 | 0. 元信息 |
| 2 | 968 | 1. 缺陷清单 |
| 2 | 982 | 2. 修复优先级 |
| 3 | 984 | 2.1 必须修复（🔴 + 🟠） |
| 3 | 990 | 2.2 建议修复（🟡） |
| 3 | 995 | 2.3 跳过的修复 |
| 2 | 1002 | 3. 修复影响面 |
| 3 | 1004 | 3.1 涉及章节 |
| 3 | 1012 | 3.2 影响 Story 列表 |
| 3 | 1018 | 3.3 影响规模 |
| 2 | 1026 | 4. 字段链路与数据模型修订计划 |
| 2 | 1035 | 5. 跨级影响分析 |
| 2 | 1048 | 6. 验收标准 |
| 3 | 1059 | 4.2 Plan 出闸条件 |
| 2 | 1069 | 第五步：🔴 人工审核点（双支柱） |
| 3 | 1073 | 支柱 1：叙述性讲解 |
| 3 | 1092 | 支柱 2：对话内直接呈现 |
| 3 | 1099 | 等待用户确认 |
| 2 | 1114 | 第六步：触发 dr-update-skill |
| 2 | 1133 | 第七步：8 道闸 |
| 2 | 1148 | 执行清单（24 步） |
| 2 | 1181 | 与其他 SKILL 关系 |
| 3 | 1183 | 上游（输入来源） |
| 3 | 1188 | 下游（输出消费者） |
| 3 | 1193 | 横向（参照对象） |
| 2 | 1199 | 禁止事项（10 条） |
| 2 | 1216 | 退出条件 |
| 2 | 1234 | 版本 |

## Inline References

| ref |
| --- |
| CR-DR-XXX-vN.m-UpdatePlan.md |
| CR-DR-XXX-vN.m.md |
| CR/ |
| ChangeLog/ |
| ChangeLog/CR-DR-XXX-changelog.md |
| ae-sdd assets query "<componentName>" --project <projectKey> |
| ae-sdd assets query "<method>" --project <projectKey> |
| ae-sdd assets query "<tableName>" --project <projectKey> |
| ae-sdd assets read dr-generate --project <projectKey> |
| ae-sdd doc finalize |
| ae-sdd doc resolve --intent DR_SUPPLEMENT --doc-id {drId} |
| ae-sdd doc save |
| ae-sdd doc save --changelog-note |
| ae-sdd doc save --intent DR_SUPPLEMENT |
| ae-sdd doc save --intent DR_SUPPLEMENT --doc-id {drId} --content-file 草稿.md |
| ae-sdd doc save --intent DR_SUPPLEMENT --doc-id {drId} --content-file 草稿.md --changelog-note "UpdatePlan" |
| ae-sdd doc save --intent RA --doc-id {raId} |
| ae-sdd doc save --intent STORY_REVIEW --work-item {W} --story-id {S} |
| ae-sdd update-check |
| agent-orchestration-skill.md §8.4 多 reviewer 默认编排框架 |
| agent-orchestration-skill.md §8.4.4 不一致项处理决策树 |
| constraints/assets/RA/PRD |
| document-storage-skill.md |
| get_constraints/get_assets |
| review-loop-skill.md |
| templates/design/dr-review-update-plan-template.md |
| templates/design/dr-template.md |
| tools/lib/gates.py |
| {projectKey}.assets.md |
