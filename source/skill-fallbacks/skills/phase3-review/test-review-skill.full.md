---
name: test-review
description: Test 系列 Step 3 reviewSkill。由 test-verifier 独立复核测试报告、原始证据、真实性扫描与 AC 覆盖，决定是否回到 test-generate。
---

# Test Review — 测试真实性复核 SKILL

## 与监管器 4 步的关系

本文件只负责 **Test 系列 Step 3：reviewSkill**。Loop 次数、暂停、人工审核由主流程监管器执行；退出阈值遵守 `review-loop-skill.md`。

## 强制独立性

必须通过 `agent-orchestration-skill.md` 派 `test-verifier` 执行复核。

| 要求 | 通过标准 |
|---|---|
| 独立会话 | `session_id` 与主 agent 不同 |
| 独立读取 | verifier 自行读取报告、日志、XML、测试代码 |
| 独立运行 | 能重跑关键命令或抽样命令；不能只相信主 agent 摘要 |
| 独立结论 | 输出 PASS / BLOCKED / NEEDS-RERUN 和原因 |

环境不支持物理 sub-agent 时，报告必须标 `reviewerMode: "logical-multi-perspective"`，不得冒充独立验证。

## 输入

| 输入 | 必读 |
|---|---|
| `TEST_REPORT` 最新版 | 是 |
| 原始 stdout/stderr 日志 | 是 |
| Surefire/Failsafe XML | 是 |
| `test_authenticity_scan` 报告 | 是 |
| TestCase 文档 | 是 |
| 变更测试代码与生产代码 | 是 |
| 项目资产测试规范 | 有则读 |

## 检查口径（TV-1~TV-10）

| # | 检查项 | 通过标准 |
|---|---|---|
| TV-1 | 证据链存在 | 报告引用的日志/XML/扫描报告路径真实存在 |
| TV-2 | 命令真实性 | 无 skipTests / testFailureIgnore / 未解释 excludes |
| TV-3 | XML 对账 | 报告统计与 XML 完全一致，skipped=0 或有 Story 豁免 |
| TV-4 | AC 覆盖 | 每个 AC 至少有实际执行方法 |
| TV-5 | TestCase 对账 | 应跑用例数、测试源码方法数、XML 执行数一致或有解释 |
| TV-6 | L2 真实 HTTP | 接口测试走真实 HTTP；MockMvc 降级有证据 |
| TV-7 | L3 真实 DB | 核心写路径有真实 DB/H2/Testcontainers 证据 |
| TV-8 | 失败暴露 | 失败用例不被隐藏，根因分类合理 |
| TV-9 | 禁假修复 | 无未授权修改测试、无全 Mock 核心路径、无空断言 |
| TV-10 | G-09/G-10 | `ae-sdd gates check --only G-09,G-10` 或等价检查通过 |

## 输出

复核不新增独立流程产物，默认在新版 `TEST_REPORT` 中追加“独立复核”章节并 `ae-sdd doc save --intent TEST_REPORT --work-item {W} --story-id {S?} --version "v1-r2" --content-file 草稿.md`。

章节必须包含：

| 字段 | 要求 |
|---|---|
| verifier_session_id | 必填，且不能等于主 agent |
| reviewerMode | `physical-sub-agent` / `logical-multi-perspective` |
| TV-1~TV-10 | 每项 PASS / FAIL / N/A + 证据 |
| 抽样重跑 | 命令、结果、日志路径 |
| 结论 | PASS / BLOCKED / NEEDS-RERUN |
| 回退建议 | 回 `test-generate` / 回 Coding / 回 Story/Task / 用户决策 |

## 缺陷处理

| 缺陷类型 | 处理 |
|---|---|
| 证据缺失 / XML 不一致 / 扫描报告缺失 | 回 `test-generate-skill.md` 补证据或重跑 |
| 测试失败指向生产代码 | 回 Coding 修复，再重跑 Test 系列 |
| 测试数据或测试断言错误 | 需说明原因；修改测试前必须用户确认 |
| Story / Task 设计缺陷 | 触发 Proposal 或回对应设计节点 |
| verifier 无法独立验证 | 标 BLOCKED，升级用户，不得 PASS |

## 禁止事项

| 禁止 | 正确做法 |
|---|---|
| 主 agent 自称 test-verifier | 必须独立 session_id 或降级标注 |
| 只读摘要不读证据 | 逐项读取报告、日志、XML、代码 |
| 把 warn 当 pass | warn 必须有解释和残留风险 |
| 复核阶段直接修代码/测试 | 只出结论和回退建议 |

## 执行清单

| # | 动作 | 产出 | 门禁 |
|---|---|---|---|
| 1 | 派 test-verifier | 任务卡 | session 独立 |
| 2 | 读取证据 | 证据清单 | 路径真实存在 |
| 3 | 执行 TV-1~TV-10 | 复核矩阵 | 无 BLOCKED |
| 4 | 抽样重跑 | 抽样日志 | 结果与报告一致 |
| 5 | 写复核章节 | 新版 TEST_REPORT | G-09/G-10 通过 |
