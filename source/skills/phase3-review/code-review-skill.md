---
name: code-review
description: 端到端代码评审 SKILL — Phase 3 ⑦ 节点的环节内具体规则。与 story-review-skill.md 同等地位，覆盖"Code Review 准入/多维评审/6 大闸门/异常路径/多 Agent 编排"。当 Story 编码完成、Coding 报告 + 测试报告 + CodePlan 出炉时触发；当用户说"Code Review 报告"/"出 CR 报告"/"评审代码"/"审核 Story"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase3-review/code-review-skill.full.md
source_fallback_sha256: f0912817089e2b0f317cb6ff244f963fa67409e9813f80fc2fa833de04d1b287
source_original_bytes: 55011
source_original_lines: 1109
source_semantic_inventory_sha256: 8f321a41615c411c7f2d34bbea1c4733eb6f56bd8449c4566d4c95a066a93163
source_slimmer: slim_source_skills.py@2
---

# Code Review — 端到端代码评审 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase3-review/code-review-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase3-review/code-review-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase3-review/code-review-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase3-review/code-review-skill.md`
- fallback: `skill-fallbacks/skills/phase3-review/code-review-skill.full.md`
- fallback_sha256: `f0912817089e2b0f317cb6ff244f963fa67409e9813f80fc2fa833de04d1b287`
- original_lines: 1109
- original_bytes: 55011
- semantic_inventory_sha256: `8f321a41615c411c7f2d34bbea1c4733eb6f56bd8449c4566d4c95a066a93163`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 端到端代码评审 SKILL — Phase 3 ⑦ 节点的环节内具体规则。与 story-review-skill.md 同等地位，覆盖"Code Review 准入/多维评审/6 大闸门/异常路径/多 Agent 编排"。当 Story 编码完成、Coding 报告 + 测试报告 + CodePlan 出炉时触发；当用户说"Code Review 报告"/"出 CR 报告"/"评审代码"/"审核 Story"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:29 📦 文档存放前置调用（🔴 横切依赖）; L2:172 触发条件; L3:281 1.5 调用项目资产服务; +5 more; keyword_hits: 67 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:121 整体流程; L2:338 第二步：多维评审（六阶段并行挖掘）; L3:342 阶段 A：业务逻辑评审; +7 more; keyword_hits: 83 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:18 🟠 门禁强度声明（v3.5.11 AA 诚实降级）; L3:76 标尺 1：证据标准（🔴 禁止裸结论）; L3:100 标尺 3：完整性度量（🔴 "覆盖所有"必须先有穷举清单）; +11 more; keyword_hits: 120 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 40 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L3:567 闸门状态; keyword_hits: 45 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:29 📦 文档存放前置调用（🔴 横切依赖）; L3:48 本 SKILL 产出文档 × intent 对照; L3:315 1.7 产出：输入清单; +5 more; keyword_hits: 117 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 56; refs: .ae-sdd/tmp/{doc-id}-draft.md; SKILL.md; ae-sdd assets query "<componentName>" --project <projectKey>; +53 more; keyword_hits: 73 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L1:304 按 §1.2 提取的 §3 Task 列表逐个读：; L3:470 阶段 F：跨文档引用核查（与 §3 调用层级一致性）; L3:1027 3 种多 Agent 模式触发条件（Code Review 场景专属 = §8.4.1 Tier 判定的 Code Review 实例化）; +1 more; keyword_hits: 267 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L2:546 第四步：记入补充说明文档; keyword_hits: 18 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Code Review — 端到端代码评审 Skill |
| 2 | 18 | 🟠 门禁强度声明（v3.5.11 AA 诚实降级） |
| 2 | 29 | 📦 文档存放前置调用（🔴 横切依赖） |
| 3 | 33 | 写入 SOP（3 步） |
| 3 | 48 | 本 SKILL 产出文档 × intent 对照 |
| 2 | 60 | 目标 |
| 2 | 74 | Code Review 总则（🔴 贯穿全 SKILL，违反 = 结论无效） |
| 3 | 76 | 标尺 1：证据标准（🔴 禁止裸结论） |
| 3 | 91 | 标尺 2：🔴/🟠/🟡/🟢 分级（与 AE 体系一致） |
| 3 | 100 | 标尺 3：完整性度量（🔴 "覆盖所有"必须先有穷举清单） |
| 3 | 110 | 标尺 4：语言精确性（🔴 禁用纯主观词） |
| 2 | 121 | 整体流程 |
| 2 | 172 | 触发条件 |
| 2 | 181 | Plan-first 更新原则（🔴 CodeReview 反馈修改前硬前置） |
| 2 | 196 | 第零步：CodeReview 准入检查（🔴 硬门禁，未通过禁止进入 Review） |
| 2 | 216 | 准入检查记录 - {STORY-ID} - {YYYY-MM-DD HH:mm} |
| 2 | 231 | 第一步：读取输入 |
| 3 | 233 | 1.1 提取 Story 信息 |
| 3 | 246 | 1.2 提取统一版 CodePlan 信息 |
| 3 | 262 | 1.3 提取 Coding 报告信息 |
| 3 | 271 | 1.4 提取测试报告信息 |
| 3 | 281 | 1.5 调用项目资产服务 |
| 3 | 301 | 1.6 读取实际代码（🔴 必须读代码本身，不读报告代劳） |
| 1 | 304 | 按 §1.2 提取的 §3 Task 列表逐个读： |
| 3 | 315 | 1.7 产出：输入清单 |
| 2 | 338 | 第二步：多维评审（六阶段并行挖掘） |
| 3 | 342 | 阶段 A：业务逻辑评审 |
| 3 | 370 | 阶段 B：分层职责红线核查 |
| 3 | 401 | 阶段 C：数据库逻辑链评审 |
| 3 | 429 | 阶段 D：Test Review 结果引用核查 |
| 3 | 447 | 阶段 E：项目资产合规性核查 |
| 3 | 470 | 阶段 F：跨文档引用核查（与 §3 调用层级一致性） |
| 3 | 493 | 第二步产出：Code Review 结论初稿 |
| 2 | 496 | Code Review 结论初稿 - {STORY-ID} - {YYYY-MM-DD HH:mm} |
| 2 | 513 | 第三步：合理性判定 |
| 3 | 515 | 3.1 判定流程 |
| 3 | 532 | 3.2 判定结果分类（🔴 必须带归属标签） |
| 2 | 546 | 第四步：记入补充说明文档 |
| 2 | 551 | {N}、Code Review 第 {轮次} 轮 - {YYYY-MM-DD HH:mm} |
| 3 | 553 | Reviewer |
| 3 | 559 | 问题汇总 |
| 3 | 567 | 闸门状态 |
| 2 | 581 | 第四步 bis：🔴 触发的 Proposal（改代码前硬门禁） |
| 3 | 599 | 触发各下游 SKILL |
| 2 | 624 | 第五步：触发下游 SKILL |
| 2 | 638 | 第六步：循环判定 |
| 2 | 660 | 第七步：CodeReview 闸门全集（7 道闸，从 coding-skill 迁出） |
| 3 | 665 | 闸 1：⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 硬前置） |
| 3 | 710 | 闸 2：⑦bis 全链路对称性核查闸（🔴 流程收尾强制） |
| 3 | 748 | 闸 3：全文档回扫闸（🔴 CodeReview 必跑） |
| 3 | 769 | 闸 4：禁裸 ✅ 闸（🔴 任何检查项的 ✅ 必须附客观证据） |
| 3 | 789 | 闸 5：报告-代码对账闸（🔴 任何报告必须验证报告项在代码中真实存在） |
| 3 | 806 | 闸 6：产出物对账闸（🔴 报告完成后必须验证） |
| 3 | 822 | 闸 7：Test Review 通过引用核查闸 |
| 3 | 834 | 闸门汇总 |
| 2 | 850 | 第七步 bis 前置：Test Review 引用门禁 |
| 2 | 862 | 第七步 bis：CodeReview 报告合规性校验 |
| 2 | 885 | 禁止事项 |
| 2 | 902 | 执行清单（逐项执行，禁止跳过） |
| 2 | 933 | 📖 人工审核主动讲解规范 — Code 节点 |
| 2 | 1005 | 📋 多 Agent 评审编排 |
| 3 | 1016 | 角色 7：CodeReview Agent（`code-reviewer`） |
| 3 | 1027 | 3 种多 Agent 模式触发条件（Code Review 场景专属 = §8.4.1 Tier 判定的 Code Review 实例化） |
| 3 | 1037 | reviewer 角色分工（Code Review 场景专属 = §8.4.2 视角切分的 Code Review 实例化） |
| 3 | 1050 | 交叉对比与冲突处理（🔴 引用横切规范，不再重复定义） |
| 3 | 1071 | 各 reviewer 评审重点补充（Code Review 场景专属） |
| 3 | 1082 | 与子 SKILL 的协同 |
| 2 | 1096 | 维护 |

## Inline References

| ref |
| --- |
| .ae-sdd/tmp/{doc-id}-draft.md |
| SKILL.md |
| ae-sdd assets query "<componentName>" --project <projectKey> |
| ae-sdd assets query "<method>" --project <projectKey> |
| ae-sdd assets query "<tableName>" --project <projectKey> |
| ae-sdd assets read code-review |
| ae-sdd assets read code-review --project <projectKey> |
| ae-sdd doc resolve |
| ae-sdd doc resolve --intent CODE_REVIEW --work-item {W} --story-id {S?} |
| ae-sdd doc resolve --intent CODING_PLAN --work-item {W} --story-id {S?} |
| ae-sdd doc resolve --intent CODING_REPORT --work-item {W} --story-id {S?} |
| ae-sdd doc resolve --intent STORY --story-id {S} |
| ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?} |
| ae-sdd doc resolve --intent TEST_REPORT --work-item {W} --story-id {S?} |
| ae-sdd doc save |
| ae-sdd doc save --intent CODE_REVIEW --work-item {W} --story-id {S?} --version "v1-r1" ... |
| ae-sdd doc save --intent PROPOSAL --story-id {STORY-ID} --content-file 草稿.md |
| ae-sdd doc save --intent TRACE_MATRIX --work-item {W} --story-id {S?} --version "v1-r1" ... |
| ae-sdd update-check |
| ae-sdd-update-skill.md |
| agent-orchestration-skill.md |
| agent-orchestration-skill.md §8.4 |
| agent-orchestration-skill.md §8.4.1 |
| agent-orchestration-skill.md §8.4.2 |
| agent-orchestration-skill.md §8.4.3 交叉对比算法 |
| agent-orchestration-skill.md §任务分配协议 |
| appservice/{Resource}AppService |
| code-review-skill.md |
| coding-skill.md |
| document-storage-skill.md §1.3 路径模板 |
| domain/.../service/{Resource}DomainService |
| entity/{Resource}DO.transition() |
| findByXxx / save / update / updateStatus |
| import javax.persistence\\|@TableName\\|@Data |
| project-assets-update-skill.md |
| proposal-skill.md |
| proposal-skill.md §第二步 |
| review-loop-skill.md |
| story-review-skill.md |
| story-update-skill.md |
| task-generate-skill.md |
| templates/coding/be-codereview-template.md |
| test-generate-skill.md |
| test-review-skill.md |
| testcase-generate-skill.md |
| tools/lib/gates.py |
| {STORY-ID}-CodeReview.md |
| {STORY-ID}-CodingPlan.md |
| {STORY-ID}-CodingReport.md |
| {STORY-ID}-Report.md |
| {STORY-ID}-追溯矩阵.md |
| {projectKey}.assets.md |
| {story-prefix}-Supplement.md |
