# 2026-07-03 Runtime Stats P0 落地

## 背景

`gates check`、扫描器和 CLI 子进程调用已经成为 ae-sdd 工具链的主要耗时来源。此前只能靠人工 `Measure-Command` 粗测，无法持续定位慢 gate、慢 scanner 或 Windows UTF-8 子进程问题。

## 变更

- 新增 `tools/lib/runtime_stats.py`：记录 command/span 耗时、CPU 近似耗时、退出码、脱敏 argv 和 span 属性，写入 JSONL。
- 新增 `tools/lib/runtime_exec.py`：统一子进程调用，默认 UTF-8 解码、`errors="replace"`，并注入 `PYTHONUTF8=1` / `PYTHONIOENCODING=utf-8`。
- `tools/bin/ae-sdd` 总入口包裹 command event，新增 `ae-sdd perf report/doctor/clear`。
- `tools/lib/gates.py::check_all()` 为每个 gate 记录 span，`summarize()` 输出每个 gate 的 `durationMs` 与 `slowest`。
- gates 内部扫描器调用与 CLI 独立扫描命令第一批切换到 `runtime_exec.run_command()`。
- `perf clear` 清理统计文件时抑制自身统计事件，避免清理后立即重新写入 clear 事件。

## 存储与边界

- 项目内统计写入 `.ae-sdd/runtime-stats/YYYY-MM-DD.jsonl`。
- 母版 `.gitignore` 已忽略 `.ae-sdd/runtime-stats/`，避免本地观测数据进入版本控制。
- 无项目 `.ae-sdd/` 时写入系统临时目录 `ae-sdd/runtime-stats/`。
- `AE_SDD_STATS=0` 可关闭统计；`AE_SDD_STATS_DIR=<dir>` 可改写统计目录，主要用于测试。
- 统计不写业务 stdout；`--json` 业务输出保持可解析。
- 统计只记录运行元数据和有限 span 属性，不记录业务文档正文。

## 文档同步

- `source/docs/ae-sdd-design.md`：补充 `perf report/doctor/clear` 为性能诊断类 CLI，并声明 report-only 边界。
- `source/docs/ae-sdd-implementation-architecture.md`：Runtime Stats 从预留架构更新为已落地架构，补充存储、环境开关、子进程 UTF-8 边界。
- `source/docs/plans/2026-07-02-runtime-stats-performance-plan.md`：标注 P0 已实现，保留 P1+ 性能优化方向。
- `README.md`：版本摘要和实现架构文档说明已更新。

## 验证

- `python -m py_compile tools/lib/runtime_stats.py tools/lib/runtime_exec.py tools/lib/gates.py tools/bin/ae-sdd tools/tests/test_runtime_stats.py tools/tests/test_cli_perf.py`
- `python tools/tests/run.py runtime_stats -v`
- `python tools/tests/run.py cli_perf -v`
- `python tools/tests/run.py update_graph -v`
- `python tools/tests/run.py gates -v`
- `python tools/bin/ae-sdd update-check --affected tools/lib/runtime_stats.py,tools/lib/runtime_exec.py,tools/lib/gates.py,tools/bin/ae-sdd,tools/tests/test_runtime_stats.py,tools/tests/test_cli_perf.py,source/SKILL.md,source/standards/update-graph.json,source/skills/orchestration/ae-sdd-update-skill.md,source/docs/ae-sdd-design.md,source/docs/ae-sdd-implementation-architecture.md,source/docs/plans/2026-07-02-runtime-stats-performance-plan.md,README.md,source/CHANGELOG/2026-07-03-runtime-stats-p0.md --json`
- `python tools/bin/ae-sdd update-check --json`
- `python tools/bin/ae-sdd iteration-check --json`

`update-check` 全绿；`iteration-check` 通过但会在未 stage 状态下提示新增 runtime 模块仍为 untracked WIP。
