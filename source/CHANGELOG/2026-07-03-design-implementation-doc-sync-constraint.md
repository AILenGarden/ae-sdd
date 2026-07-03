# 2026-07-03 设计文档与实现架构文档同步约束

## 变更摘要

在 ae-sdd 自更新维护流程中新增强制约束：任意 `source/` 或 `tools/` 调整后，必须判断并同步 `ae-sdd-design.md` 与 `ae-sdd-implementation-architecture.md`，或在 CHANGELOG 中显式标注不受影响。

## 详细变更

- 更新 `source/skills/orchestration/ae-sdd-update-skill.md`
  - 在子系统维护边界表中把设计/实现架构文档加入任意 source/tools 改动的连带项。
  - 在工具链 SOP 中要求能力语义变化同步 `ae-sdd-design.md`，实现边界/数据流变化同步 `ae-sdd-implementation-architecture.md`。
  - 在"内容回写到正确位置的 5 步流程"步骤 1 增加"设计/实现架构同步"后置块。
  - 在健康度清单中加入实现架构说明书存在性与同步约束检查项。
- 更新 `source/standards/update-graph.json`
  - 在 UG-08 任意 source/tools 改动连带项中加入两份架构文档的同步判断。
- 更新 `README.md`
  - 第 5 行最新变更说明改为本次同步约束。

## 触发原因

新增实现架构说明书后，ae-sdd 需要一个明确维护约束，防止后续只改 SKILL/代码而忘记同步能力设计与实现架构，重新出现文档层与实现层漂移。

## 影响范围

- 能力设计文档：本次未改变能力语义，`source/docs/ae-sdd-design.md` 不需要更新。
- 实现架构文档：本次未改变代码模块边界或数据流，`source/docs/ae-sdd-implementation-architecture.md` 不需要更新。
- 机器图谱：UG-08 已加入两份文档的人工核对连带项。

## 验证方式

- 已执行 `python tools/bin/ae-sdd iteration-check --json`，结果为 16 info / 0 warn / 0 error。
- 待执行 `ae-sdd update-check --affected` 与全量 `ae-sdd update-check`；若失败，按输出区分本次变更与既有未收口项。

## Reviewer

Codex
