# 2026-07-03 | ae-sdd v3.8.2 - runtime-stats 可观测性 5 缺口补全

## Summary

在 life 项目实测 ae-sdd 加载消耗时发现：runtime_stats 虽已采集数据，但 5 处缺口导致诊断失真或盲区。最严重的是 CLI 顶层 import 成本（实测 ~120-200ms）完全不在统计内——`start_command` 在 `parse_args()` 之后才调用，漏掉了 20 个 lib 模块的 import 固定税，导致 `perf doctor` 误判高频 hook 命令"很快"（业务函数 <1ms），真实端到端的 200ms+ 看不见。本次补全 5 缺口：bootstrap import 成本单独计量、高频 hook 命令补 span、cpuMs/ioWaitMs 汇总、doctor 诊断改用真实 bootstrapMs、scanner span 补 scanRoot 属性。补全后 doctor 能给出真实数据驱动的"P1 lazy import"建议，且能区分"CPU 慢"与"等子进程 I/O 慢"。

## Changes

| Area | Change |
|---|---|
| runtime_stats.py | 新增 `AE_SDD_BOOT_NS` env 戳读取，`start_command` 计算 `bootstrapMs` 并提升为事件顶层字段；`finish_command` 清理 env 戳防子进程继承 |
| runtime_stats.py | `TraceRecorder` 加 `bootstrap_ms` 字段；`to_event` 条件写入 `bootstrapMs`（无戳则省略，向后兼容） |
| runtime_stats.py | `summarize_events` 补 `cpuMs`/`ioWaitMs`/`bootstrapMs` 分桶，`commands[]` 补 `avgCpuMs`/`maxCpuMs`，`byScale[]` 补 `avgCpuMs`/`avgIoWaitMs` |
| runtime_exec.py | `run_command` 加 `attrs` 形参合并进 span attrs；span attrs 补 `argsCount`/`arg0` 便于区分扫描器 |
| gates.py | 6 处 scanner 调用（test/coding/ra_authenticity/flow_violation/ra_depth/ra_implementation）补 `attrs={"scanRoot": str(project_dir)}` |
| tools/bin/ae-sdd | 顶部（所有 import 前）打 `AE_SDD_BOOT_NS` 戳 |
| tools/bin/ae-sdd | `cmd_state_read`/`cmd_state_next_step`/`cmd_classify`/`cmd_gate_intercept`/`cmd_prompt_inject`/`cmd_stop_check` 各补 1 个主 span 记录关键判定结果 |
| tools/bin/ae-sdd | `_perf_advice` 删除失真的"avgMs>150ms 含 import"判断，改用真实 `bootstrapMs`；新增 ioWait 占比 >70% 的子进程瓶颈诊断；gate span 含内嵌 scanner 时标注"含子扫描器 X"去重 |
| tools/bin/ae-sdd | `cmd_perf_report` 输出补 cpu/ioWait/bootstrap 摘要行 |
| test_runtime_stats.py | 新增 7 个测试：bootstrap 计量/env 清理/cpu+ioWait 汇总/byScale cpu/bootstrap 分桶/runtime_exec attrs 合并 |
| test_cli_perf.py | 新增 2 个端到端测试：perf report 含 bootstrapMs 维度、doctor 高 bootstrap 触发 lazy import 建议 |
| 2026-07-02-runtime-stats-performance-plan.md | 状态更新：标注 5 缺口已补全，P1 lazy import 仍未实施但已有真实数据支撑 |

## 触发原因

- life 项目实测：318 个历史事件总 span=0，高频 hook 命令（gate-intercept 237 次/天）在统计里是黑盒
- 实测发现 `version` 端到端 200ms+ 但 `perf report` 显示 0.1ms——统计口径漏掉 import 固定成本，诊断结论会错
- update-graph.json runtime_stats 规则（第 380-399 行）已要求改 runtime_stats.py 时同步 runtime_exec.py/perf 子命令/测试/plan，本次按连带项执行

## 影响范围

- 涉及运行时统计逻辑（runtime_stats.py/runtime_exec.py/gates.py/tools/bin/ae-sdd），非门禁行为变更
- 事件 schema 保持 `ae-sdd.runtimeStats.v1` 不变，新字段（bootstrapMs/cpuMs/ioWaitMs 分桶、span attrs）加法式扩展，monitor workspace.js 宽松解析不受影响
- 版本号不推进（可观测性补全，非功能契约变更）
- 无破坏性变更：旧 JSONL 事件无 bootstrapMs → summarize 用 `.get()` 容错，doctor 跳过该维诊断

## 验证方式

- `python -m unittest tools.tests.test_runtime_stats tools.tests.test_cli_perf` 17 个测试全绿（8 旧 + 9 新）
- `python -m unittest tools.tests.test_gates tools.tests.test_gate_intercept tools.tests.test_gate_intercept_v11` 116 个测试全绿
- `python tools/bin/ae-sdd update-check --only UC-14` 连带项无漏
- life 项目实测：`perf report` 显示 `bootstrap(import): avg=112.3ms p95=126.6ms`（此前看不见）；`perf doctor` 输出"p95=163.7ms>150ms 建议实施 P1 lazy import"（此前永不触发）
- gates check 的 scanner span 含 `scanRoot: D:\Item\life`（此前无此属性）

## Reviewer

陈聪
