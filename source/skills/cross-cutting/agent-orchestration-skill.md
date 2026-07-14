---
name: agent-orchestration
description: Agent 编排 SKILL — 任务节点**内**的子任务拆分 + 多 Agent 并行 + 负载均衡 + 故障检测 + 故障补救 + 多 reviewer 默认编排。**澄清（2026-06-06）：** 任务拆分不是按流程节点（流程串行无法并行），而是按同一节点内的子任务（可并行）。🆕 2026-06-25：新增 §8.4 多 reviewer 默认编排框架（横切规范 SSOT）——所有 Review 节点的"是否启用多 reviewer / 启用几个 / 视角怎么切 / 冲突怎么解"统一归本 SKILL 管理，对抗 AI 逻辑自洽陷阱。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/agent-orchestration-skill.full.md
source_fallback_sha256: e4f05b2477b1adab8eecddcfb535ab55c0480ad06eb380d433c6887a0124d5bc
source_original_bytes: 48582
source_original_lines: 906
source_semantic_inventory_sha256: 8fcca3427d8e27feb91fbed88594dd60f55aa937f5775c1443684cb0f4dabfc5
source_slimmer: slim_source_skills.py@2
---

# Agent Orchestration — 任务节点内子任务编排 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/agent-orchestration-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/agent-orchestration-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/agent-orchestration-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/agent-orchestration-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/agent-orchestration-skill.full.md`
- fallback_sha256: `e4f05b2477b1adab8eecddcfb535ab55c0480ad06eb380d433c6887a0124d5bc`
- original_lines: 906
- original_bytes: 48582
- semantic_inventory_sha256: `8fcca3427d8e27feb91fbed88594dd60f55aa937f5775c1443684cb0f4dabfc5`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Agent 编排 SKILL — 任务节点**内**的子任务拆分 + 多 Agent 并行 + 负载均衡 + 故障检测 + 故障补救 + 多 reviewer 默认编排。**澄清（2026-06-06）：** 任务拆分不是按流程节点（流程串行无法并行），而是按同一节点内的子任务（可并行）。🆕 2026-06-25：新增 §8.4 多 reviewer 默认编排框架（横切规范 SSOT）——所有 Review 节点的"是否启用多 reviewer / 启用几个 / 视角怎么切 / 冲突怎么解"统一归本 SKILL 管理，对抗 AI 逻辑自洽陷阱。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:358 8.4 多 reviewer 默认编排框架（🔴 横切规范 — 所有 Review 节点适用）; L3:821 10.1 调用方式（节点 SKILL 如何调用本 SKILL）; L4:827 步骤 X：决定是否拆子任务（调用 agent-orchestration-skill.md）; keyword_hits: 31 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L3:328 8.1 汇总流程; L3:524 8.4.6 与 5 阶段并行挖掘的关系（🔴 正交叠加，不冲突）; L2:591 8.5 🆕 v3.10.3 3层 Agent 模型（主流程 -> 子流程Agent -> sub-subAgent）; +5 more; keyword_hits: 94 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:33 标尺 1：独立性判定（🔴 子任务必须真正独立才能并行）; L2:839 11. 禁止事项; keyword_hits: 87 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 15 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L2:758 9. 状态跟踪; L3:760 9.1 状态字段（追加到 state.json）; L3:800 9.2 状态展示; keyword_hits: 62 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L3:72 1.2 拆法（按"输出维度"）; L2:151 4. 任务分配卡（Prompt 模板）; L3:342 8.2 一致性检查（🔴 多 Agent 输出的漂移风险）; keyword_hits: 101 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 40; refs: *-CodeReview-v{N}-r{M}.md; *-Coding-CoderReport-r{M}.md; *-PRD-SummaryReport.md; +37 more; headings: L4:827 步骤 X：决定是否拆子任务（调用 agent-orchestration-skill.md）; keyword_hits: 43 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:652 8.5.4 与 §8.4 多 reviewer 的衔接; L1:826 节点 SKILL 在 §整体流程 中插入; keyword_hits: 138 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 11 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Agent Orchestration — 任务节点内子任务编排 Skill |
| 2 | 21 | 0. 目标 |
| 2 | 31 | 总则（🔴 贯穿全 SKILL） |
| 3 | 33 | 标尺 1：独立性判定（🔴 子任务必须真正独立才能并行） |
| 3 | 39 | 标尺 2：粒度节制（🔴 不为并行而并行） |
| 3 | 45 | 标尺 3：故障早发现 |
| 3 | 51 | 标尺 4：补救有上限 |
| 2 | 59 | 1. 子任务拆分原则（🔴 关键：拆与不拆的判定） |
| 3 | 61 | 1.1 拆与不拆决策表 |
| 3 | 72 | 1.2 拆法（按"输出维度"） |
| 3 | 84 | 1.3 不拆的反例（🔴 严禁拆） |
| 2 | 96 | 2. Agent 数量决策 |
| 3 | 98 | 2.1 数量决策矩阵 |
| 3 | 107 | 2.2 Agent 数量上限 |
| 3 | 112 | 2.3 资源评估 |
| 2 | 120 | 3. Agent 角色分配（从角色库选） |
| 3 | 124 | 3.1 角色库 |
| 3 | 141 | 3.2 角色与子任务匹配 |
| 2 | 151 | 4. 任务分配卡（Prompt 模板） |
| 1 | 156 | 任务分配卡 |
| 3 | 190 | 4.1 任务分配卡填写原则 |
| 2 | 203 | 5. 负载均衡策略 |
| 3 | 205 | 5.1 平衡原则 |
| 3 | 211 | 5.2 资源评估公式 |
| 3 | 225 | 5.3 平衡策略 |
| 2 | 236 | 6. 故障检测（🔴 4 大故障源） |
| 3 | 238 | 6.1 4 大故障源 + 检测方法 |
| 3 | 247 | 6.2 故障检测时机 |
| 3 | 257 | 6.3 故障日志 |
| 2 | 268 | 7. 故障补救（🔴 4 级补救 SOP） |
| 3 | 270 | 7.1 补救决策树 |
| 3 | 295 | 7.2 重试（Retry） |
| 3 | 301 | 7.3 重新分配（Re-assign） |
| 3 | 307 | 7.4 降级（Degrade） |
| 3 | 315 | 7.5 升级用户（Escalate） |
| 2 | 326 | 8. 结果汇总（🔴 root agent 的关键职责） |
| 3 | 328 | 8.1 汇总流程 |
| 3 | 342 | 8.2 一致性检查（🔴 多 Agent 输出的漂移风险） |
| 3 | 348 | 8.3 交叉对比（🔴 多 reviewer 模式） |
| 2 | 358 | 8.4 多 reviewer 默认编排框架（🔴 横切规范 — 所有 Review 节点适用） |
| 3 | 373 | 8.4.0 Review Batch v2 执行约束 |
| 3 | 388 | 8.4.1 何时启用多 reviewer（Tier 判定 — 默认启用，按复杂度分级） |
| 3 | 413 | 8.4.2 reviewer 视角切分原则（🔴 反"同模型同盲区"的核心） |
| 3 | 437 | 8.4.3 交叉对比算法（root agent 执行，reviewer 全部返回后跑） |
| 3 | 471 | 8.4.4 不一致项处理决策树 |
| 3 | 501 | 8.4.5 降级规则（环境不支持物理 sub-agent 时） |
| 3 | 524 | 8.4.6 与 5 阶段并行挖掘的关系（🔴 正交叠加，不冲突） |
| 3 | 561 | 8.4.7 多 reviewer 与现有机制的兼容性 |
| 3 | 576 | 8.4.8 多 reviewer 执行清单（root agent） |
| 2 | 591 | 8.5 🆕 v3.10.3 3层 Agent 模型（主流程 -> 子流程Agent -> sub-subAgent） |
| 3 | 597 | 8.5.1 主会话 vs sub-agent 职责划分 |
| 3 | 609 | 8.5.2 不派活的例外（保留路径） |
| 3 | 618 | 8.5.3 单 sub-agent 派活协议 |
| 3 | 652 | 8.5.4 与 §8.4 多 reviewer 的衔接 |
| 2 | 660 | 8.7 🆕 v3.10.3 子流程Agent协议 |
| 3 | 664 | 8.7.1 委托契约（主流程 -> 子流程Agent） |
| 1 | 667 | 委托契约 |
| 3 | 684 | 8.7.2 sub-subAgent 派活（子流程Agent -> sub-subAgent） |
| 3 | 692 | 8.7.3 交付物回传（子流程Agent -> 主流程） |
| 3 | 706 | 8.7.4 故障处理 |
| 2 | 716 | 8.6 🆕 v3.5.5 节点级派活清单（审核点 → sub-agent 映射） |
| 3 | 720 | 8.6.1 审核点 → 派活映射表 |
| 3 | 733 | 8.6.2 单节点派活粒度建议 |
| 3 | 744 | 8.6.3 节点边界上下文压力软提示（🆕 v3.5.5） |
| 2 | 758 | 9. 状态跟踪 |
| 3 | 760 | 9.1 状态字段（追加到 state.json） |
| 3 | 800 | 9.2 状态展示 |
| 2 | 812 | 10. 与现有 SKILL 的衔接 |
| 3 | 821 | 10.1 调用方式（节点 SKILL 如何调用本 SKILL） |
| 1 | 826 | 节点 SKILL 在 §整体流程 中插入 |
| 4 | 827 | 步骤 X：决定是否拆子任务（调用 agent-orchestration-skill.md） |
| 2 | 839 | 11. 禁止事项 |
| 2 | 854 | 12. 执行清单 |
| 2 | 874 | 维护 |
| 2 | 895 | Delivery priority and stop-loss policy (v3.10.9) |

## Inline References

| ref |
| --- |
| *-CodeReview-v{N}-r{M}.md |
| *-Coding-CoderReport-r{M}.md |
| *-PRD-SummaryReport.md |
| *-Story-WriterReport.md |
| *-Task-WriterReport.md |
| *-TestCase-Review-r{N}.md |
| *-TestCase-WriterReport.md |
| .ae-sdd/config.yaml |
| .auto-engineering/{PRD-ID}/summary.md |
| SKILL.md |
| SKILL.md §统一入口 |
| ae-sdd doc resolve --intent STORY --story-id STORY-001-BE |
| ae-sdd doc resolve --intent STORY --story-id {STORY-ID} |
| ae-sdd doc save --intent PROPOSAL --story-id STORY-001-BE --content-file 草稿.md |
| ae-sdd state/gates/iteration-check/context-pressure |
| ae-sdd subprocess collect --agent-id <ID> --deliverables '[...]' |
| ae-sdd subprocess spawn --series <type> --entity-id <ID> |
| ae-sdd subprocess status |
| agent-orchestration-skill.md |
| code-review-skill.md |
| code-review-skill.md §多 Agent 评审编排 |
| coding / code-review |
| coding / code-review / story-generate |
| coding-skill.md |
| document-storage-skill.md |
| dr-review-skill.md |
| project-assets-update-skill.md |
| proposal-skill.md |
| requirement-analysis-skill.md |
| review-loop-skill.md |
| scripts/test_authenticity_scan.py |
| story-generate / coding / code-review |
| story-generate-skill.md |
| story-review-skill.md |
| task-generate-skill.md |
| task-generate-skill.md §5bis |
| testcase-generate-skill.md |
| {STORY-ID}-CodingPlan.md |
| {STORY-ID}-Report-v{N}-r{M}.md |
| {STORY-ID}-Task实现方案.md |
