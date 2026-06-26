# 2026-06-26 | ae-sdd - 流程一致性修复：save_doc 矩阵补全 + 落地门禁闭环

## Summary

排查发现整个 ae-sdd 体系存在两类系统性缺口：一是部分 Skill 的产出文档未在 save_doc 调用矩阵中登记，导致 AI 执行时缺乏依据、文件游离到随机路径（典型案例：task-generate-skill 生成 CodingPlan 后未调用 documentSkill，文件存错目录）；二是多个 Skill 在"触发下游 SKILL"的流程节点前，缺少"文档已通过 documentStorage 落地"的显式门禁，形成"生成但未存、未确认落地就触发下游"的漏洞。本次对全链路 12 个文件统一补全，并在 document-storage-skill §15.1.1 新增 intent 枚举表作为全局 SSOT，防止 intent 散用失管。

## Changes

| Area | Change |
|---|---|
| `SKILL.md` | 删除 §⑤ Coding 中重复的触发定义（第二版遗漏 CodingPlan 为输入，形成误导） |
| `SKILL.md` | Phase ④→⑤ 前置条件表新增第 5.5 项：统一版 CodingPlan 已通过 documentStorage 落地（G-DOC-STORAGE ✅） |
| `task-generate-skill.md` | save_doc 调用矩阵补 `CODING_PLAN` 行（原矩阵完全缺失此条目） |
| `task-generate-skill.md` | save_doc 调用矩阵补 `TASK_IMPL_PLAN`（Task 实现方案）行 |
| `task-generate-skill.md` | 第六步汇总流程：步骤 5 改为"先落地存储（resolve_path + save_doc）"，步骤 6 改为"再给用户审核"，原步骤 5 顺延为步骤 6 |
| `task-generate-skill.md` | 第六步 bis 实现方案末尾补落地存储指令（调用 `TASK_IMPL_PLAN` intent，落地成功后才进入第六步） |
| `document-storage-skill.md` | 新增 §15.1.1 intent 枚举表（28 个 intent 统一登记），覆盖此前各 Skill 散用但未在 document-storage 中定义的 `TASK_SUPPLEMENT` / `TASK_WRITER_REPORT` / `TASK_REVIEW` / `TASK_IMPL_PLAN` / `CODING_PLAN` / `TEST_REPORT` / `TRACE_MATRIX` / `PROPOSAL` / `PROPOSAL_ARCHIVE` / `RA_IMPACT` / `RA_REVERSE_ISSUES` / `STORY_GENERATE_PLAN` / `STORY_WRITER_REPORT` / `REVIEW_COMPARE` / `TESTCASE_COMPLIANCE_REPORT` 等 |
| `code-review-skill.md` | 删除 L585 已声明废弃但 L600 仍保留的完整 CodeReviewUpdatePlan 模板正文（废弃声明与保留模板并存的矛盾） |
| `coding-report-skill.md` | 整体流程图和执行清单均补"第四步 bis / 步骤 12.5：落地存储"节点，触发 Code Review 前强制 resolve_path + save_doc |
| `proposal-skill.md` | 第四步末补 save_doc 调用说明和落地门禁；执行清单补步骤 8.5，落地成功后才触发下游 SKILL |
| `dr-update-skill.md` | save_doc 矩阵补 `STORY_SUPPLEMENT`（DR 变更通知写入受影响 Story 的产出路径之前缺失） |
| `dr-update-skill.md` | 第五步"通知受影响 Story"前补落地门禁：DR 主文档已落地后才通知下游 |
| `requirement-analysis-skill.md` | save_doc 矩阵补 `RA_IMPACT`（RA 修订影响分析报告）和 `RA_REVERSE_ISSUES`（RA 反向问题登记）两行 |
| `requirement-analysis-skill.md` | 第六步"触发下游 SKILL"前补落地门禁：RA 文档已落地后才触发 |
| `story-generate-skill.md` | save_doc 矩阵补 `STORY_GENERATE_PLAN` 行 |
| `story-generate-skill.md` | 第六步循环判定流程图中补"🔴 落地存储确认"步骤，写入后先落地再触发 Story Review |
| `story-review-skill.md` | 第五步"触发 Story Update SKILL"前补落地门禁：Review 报告和 UpdatePlan 已落地后才触发 |
| `dr-generate-skill.md` | 第六步"触发下游 SKILL"前补落地门禁：DR 文档已落地后才触发 DR Review |
| `dr-review-skill.md` | 第六步"触发 dr-update-skill"前补落地门禁：Review 报告和 UpdatePlan 已落地后才触发 |
| `testcase-generate-skill.md` | 执行清单补步骤 5.5：合规性校验报告落地存储（`TESTCASE_COMPLIANCE_REPORT`）；步骤 6 门禁补充"resolve_path + save_doc 成功后才算完成" |

## 触发原因

- 用户反馈：CodingPlan 生成后未调用 documentSkill，文档游离到错误目录（直接根因）
- 全链路扫描发现：同类缺口不止于 CodingPlan，整个流程链路中存在系统性"生成但无落地指令"漏洞
- document-storage-skill §15.2 说明"各 SKILL 必加 save_doc 矩阵"，但无统一 intent 枚举表，导致各 Skill 自行使用未登记 intent，失去 SSOT 管控

## 影响范围

- 纯文档变更，不涉及 CLI 工具代码或运行时逻辑
- 变更已有流程顺序：task-generate-skill 第六步的"输出给用户审核"从步骤 5 顺延为步骤 6，步骤 5 改为落地存储——对用户无感知，但 AI 执行顺序有变
- 所有变更均为补充，无删除现有有效门禁或职责边界的破坏性变更（code-review-skill 删除的模板在 L585 已明确声明废弃）
- document-storage-skill §15.1.1 为新增章节，不影响已有 §15.1 / §15.2 内容
- 版本号未推进（本次为文档一致性补全，不新增功能）

## 验证方式

- 人工核对：`task-generate-skill.md` save_doc 矩阵包含 CODING_PLAN + TASK_IMPL_PLAN 两行 ✅
- 人工核对：`SKILL.md` Phase ④→⑤ 前置条件表第 5.5 项存在 ✅
- 人工核对：`SKILL.md` §⑤ Coding 节只有一处"触发"定义，输入包含 CodingPlan ✅
- 人工核对：`document-storage-skill.md §15.1.1` 枚举表存在，包含 28 个 intent ✅
- `python tools/bin/ae-sdd update-check` 全绿（如 update-check 工具已覆盖文档一致性检查）

## Reviewer

陈聪
