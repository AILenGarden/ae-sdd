# 2026-07-03 | ae-sdd v3.8.0 - WorkItem 独立状态机入口强化

## Summary

用户反馈 ae-sdd 的任务管理仍然混乱：新需求没有强制新建独立 `state.json`，且状态机目录缺少同时包含 ID 与名称的可读标识。本次新增 `ae-sdd state new --id <ID> --name "<需求名>"` 作为新需求状态机入口，目录名统一为 `{ID}--{name}`，并让后续 `state read/write/next-step` 默认跟随 active work item。裸 `state write` 在没有 active work item 时会拒绝写项目级 state，避免新需求继续污染 `.ae-sdd/state.json`。

## Changes

| Area | Change |
|---|---|
| `tools/lib/paths.py` | 新增 work item 目录名规范、`.auto-engineering/` 根路径 helper，以及按 ID/目录名/metadata 反查已有状态机的路径解析。 |
| `tools/bin/ae-sdd` | 新增 `state new` 子命令，写入 `workItemId`、`workItemName`、`workItemKey`、`activeStatePath`；`state read/write/next-step` 默认跟随 active 状态机。 |
| `tools/bin/ae-sdd` | `state write` 无 active work item 时默认拒绝写项目级 state；新增 `--project-state` 作为 legacy 项目级状态逃生口。 |
| `tools/bin/ae-sdd` | `enter`、`state confirm`、`context-pressure`、`review-loop`、`state lock/unlock`、`register-review-consensus` 增加或复用 `--work-item`/active 状态机解析。 |
| `apps/ae-sdd-monitor` | 工作项投影保留并展示 `workItemId`、`workItemName`、`workItemKey`，不再只依赖旧目录名或 `currentStory`。 |
| `source/docs` / `source/HARNESS.md` / `README.md` | 同步独立状态机入口、`{ID}--{name}` 目录约定、active mirror 语义和 legacy 项目级写入规则。 |
| `tools/tests` / `apps/ae-sdd-monitor/test` | 增加路径解析、`state new`、active write、无 active 裸写拒绝、monitor work item 元数据投影测试。 |

## 触发原因

- 用户反馈每次新需求都应该启动一个新的状态机 `state.json`，当前项目级 state / work item 分桶混用导致任务管理混乱。
- 现有 `.auto-engineering/{work-item}/state.json` 只把 work item 作为可选参数，缺少“新需求必须先建状态机”的 CLI 不变量。
- 旧目录仅有 ID，缺少名称，多个需求长期并行时不可读、难定位。

## 影响范围

- 涉及 CLI 行为：新增 `ae-sdd state new`，并改变无 active work item 时裸 `state write` 的默认行为。
- 涉及状态 schema 的渐进增强：新增 `workItemId`、`workItemName`、`workItemKey`、`stateMachineId`、`stateMachineName`、`createdAt`、`lastUpdated` 字段；旧 state 仍兼容。
- 涉及项目侧路径语义：新建状态机默认写 `.auto-engineering/{ID}--{name}/state.json`；旧 `.auto-engineering/{ID}/state.json` 仍可读取。
- 涉及设计文档与实现架构文档：已同步 `ae-sdd-design.md`、`ae-sdd-implementation-architecture.md`、`ae-sdd-monitor-design.md`、`HARNESS.md` 和 README。
- 破坏性边界：未创建 active work item 时，`ae-sdd state write --phase ...` 不再静默写 `.ae-sdd/state.json`；需要维护 legacy 项目级 state 时必须加 `--project-state`。

## 验证方式

- `python -m py_compile tools\lib\paths.py tools\bin\ae-sdd`
- `python -m unittest tools.tests.test_paths tools.tests.test_cli_state_work_item tools.tests.test_memory_gate -v`
- `python -m unittest tools.tests.test_state tools.tests.test_paths tools.tests.test_cli_state_work_item tools.tests.test_memory_gate tools.tests.test_gate_intercept tools.tests.test_gate_intercept_v11 -v`
- `npm test`（`apps/ae-sdd-monitor`）
- `python scripts\build_harness.py --source . --no-mount`
- `python tools\bin\ae-sdd update-check`

## Reviewer

陈聪
