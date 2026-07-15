# YYYY-MM-DD | ae-sdd vX.Y.Z - <一句话标题>

<!--
  CHANGELOG 模板 — 每次变更按本骨架填写后另存为同名 .md（去掉 _template）。
  命名规则：YYYY-MM-DD-vX.Y.Z-<kebab-case-主题>.md
  位置：source/CHANGELOG/
  维护要求：ae-sdd-update-skill.md §更新流程 步骤 6「落档 CHANGELOG」强制使用本模板。
-->

## Summary

<!--
  2-5 句话讲清楚：改了什么、为什么改、解决了什么问题。
  先讲「背景/痛点」，再讲「本次做了什么」，最后讲「达成的效果」。
  禁止只写"更新了 XX 文件"这种无信息量描述。
-->

<本次变更的背景、动机与达成效果的叙述>

## Design ledger impact

<!--
  每次迭代必须填写：
  - 设计语义有变化：列出受影响的 D-xxx，例如 `updated: D-007, D-020`。
  - 没有设计语义变化：明确填写 `N/A: no design semantics changed`。
  不允许用对话说明、空白或“待补”代替这个结论；设计问题、预期价值和验证证据回到
  `source/docs/ae-sdd-design.md` §0 Design Ledger 维护，机器校验由 UG-28/UC-20 负责。
-->

| Design ID | Impact |
|---|---|
| `D-xxx` or `N/A` | <设计语义变化、无变化原因和对应验证证据> |

## Changes

<!--
  逐条列出改动项。用表格，列固定为 Area | Change。
  Area = 子系统/文件模块名（如 ae-sdd-update-skill / SKILL.md / paths.py / tools/bin/ae-sdd / gates.py / README）。
  Change = 具体改了什么（动词开头，可定位）。
  一行一个原子改动；同类改动可合并，但不要把不相关改动塞一行。
-->

| Area | Change |
|---|---|
| <Area> | <具体改动，动词开头，可定位> |
| <Area> | <具体改动> |

## 触发原因

<!--
  为什么做这次变更。来源可能是：
  - 用户反馈/需求
  - update-check UC-XX 检查发现的不一致
  - 前序变更的连带项（引用 update-graph 的 rule id）
  - 技术债/已知缺口补齐
  列 bullet，每条说清根因。
-->

- <触发原因 1>
- <触发原因 2>

## 影响范围

<!--
  本次变更的影响边界。说明：
  - 是否涉及运行时逻辑/门禁行为/CLI 命令（纯文档 vs 代码改动）
  - 是否改变已有门禁、子 SKILL 职责边界、文档存放路径
  - 版本号是否推进（推进则需 UC-01 三处一致）
  - 是否有破坏性变更（用户/下游需适配）
-->

- <影响范围说明 1>
- <影响范围说明 2>

## 验证方式

<!--
  如何验证本次变更正确。来源可能是：
  - `python tools/bin/ae-sdd update-check` 对应 UC-XX 全绿
  - `python tools/tests/run.py` 全部通过
  - 人工核对某项一致性
  - 具体命令的输出预期
  列 bullet，每条可执行、可核对。
-->

- `python tools/bin/ae-sdd update-check` <对应 UC-XX> 全绿。
- `python tools/tests/run.py` 全部通过。
- <其他验证方式>

## Reviewer

<!--
  变更评审人（git user.name）。单人项目也填，留痕。
-->

<Reviewer 姓名>
