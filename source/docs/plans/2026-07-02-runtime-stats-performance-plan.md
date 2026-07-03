# Runtime Stats 与性能优化方案（2026-07-02）

## 背景

ae-sdd 的工具链能力已经从单一 SKILL 文档扩展为 CLI、门禁、扫描器、状态机、hook、runtime 编译器和分发器协同运行。当前缺少统一运行时统计，导致慢点只能靠人工 `Measure-Command` 粗测，无法在 `gates check`、`update-check`、扫描器、子进程调用之间定位瓶颈。

> 状态更新（2026-07-03）：P0 已实现，包含 `tools/lib/runtime_stats.py`、`tools/lib/runtime_exec.py`、CLI 命令级统计、gate/span 统计、`perf report/doctor/clear` 与 UTF-8 子进程包装。P1+ 仍作为后续性能优化方向。

## 当前基线

在仓库根 `D:\Item\ae-sdd` 采样：

| 命令 | 耗时 |
| --- | ---: |
| `python tools/bin/ae-sdd version --json` | 185.8ms |
| `python tools/bin/ae-sdd health --json` | 203.5ms |
| `python tools/bin/ae-sdd update-check --json` | 728.6ms |
| `python tools/bin/ae-sdd gates check --json` | 6158.2ms |

逐 gate 粗采样：

| Gate | 耗时 |
| --- | ---: |
| `G-CODE-1` | 2060.2ms |
| `G-09` | 1029.2ms |
| `G-RA-5` | 787.0ms |
| `G-RA-4` | 738.0ms |
| `G-RA-6` | 704.2ms |

直接扫描脚本采样：

| 脚本 | 耗时 |
| --- | ---: |
| `scripts/coding_authenticity_scan.py` | 1956.8ms |
| `scripts/test_authenticity_scan.py` | 1002.1ms |
| `scripts/ra_implementation_scan.py` | 330.1ms |
| `scripts/ra_depth_scan.py` | 303.4ms |
| `scripts/ra_authenticity_scan.py` | 281.7ms |

同时发现 Windows 下直接 `subprocess.run(..., text=True)` 可能触发 GBK/UTF-8 解码或编码异常。性能统计与子进程执行包装应一起治理。

## 目标

| 指标 | 目标 |
| --- | ---: |
| `version --json` | < 120ms |
| `health --json` | < 180ms |
| `update-check --json` | < 500ms |
| 首次 `gates check --json` | < 3000ms |
| 第二次 `gates check --json` | < 1500ms |
| `G-CODE-1` 单 gate | < 1200ms |
| `G-09` 单 gate | < 700ms |
| Windows UTF-8 子进程异常 | 0 |
| profiling 写入开销 | < 5ms/命令 |

## 设计

新增横切层：

| 模块 | 职责 |
| --- | --- |
| `tools/lib/runtime_stats.py` | 命令级与 span 级耗时统计、JSONL 落盘、慢点汇总 |
| `tools/lib/runtime_exec.py` | 统一子进程执行、UTF-8 环境、timeout、stdout/stderr byte 统计、span 接入 |
| `ae-sdd perf report` | 查询最近运行统计，支持 `--json` |
| `ae-sdd perf clear` | 清理本地统计文件 |
| `ae-sdd perf doctor` | 根据慢点输出机器建议 |

统计存储：

- 项目内运行：`.ae-sdd/runtime-stats/*.jsonl`
- 无 `.ae-sdd/` 时：系统临时目录下 `ae-sdd/runtime-stats/*.jsonl`
- stdout 不输出统计日志，避免污染 `--json` 业务输出
- 人读摘要走 stderr；结构化报告由 `ae-sdd perf report --json` 输出

核心字段：

| 字段 | 说明 |
| --- | --- |
| `command` | CLI 命令名，例如 `gates check` |
| `durationMs` | 墙钟耗时 |
| `cpuMs` | 进程 CPU 近似耗时 |
| `exitCode` | 命令退出码 |
| `spans[]` | 子 span，例如 `gate`（attrs.gateId=`G-CODE-1`）、`scanner:coding_authenticity` |
| `cacheHit` | 扫描器或 inventory 是否命中缓存 |
| `errorClass` | 异常类型，不记录敏感参数 |

## 优先级

### P0：可观测性与 UTF-8 收口

1. 在 `tools/bin/ae-sdd` 的 `args.func(args, parser)` 外层包 command span。
2. 在 `tools/lib/gates.py::check_all()` 内给每个 gate 包 span。
3. `GateResult` 或 `details` 增加 `durationMs`，`summarize()` 输出 `slowest`。
4. 第一批把 `gates.py` 与 `tools/bin/ae-sdd` 中的扫描器/脚本类 `subprocess.run(..., text=True)` 调用收敛到 `runtime_exec.run_command()`；`update_graph.py`、`iteration_check.py` 后续按慢点再收敛。
5. `runtime_exec` 默认 `encoding="utf-8"`、`errors="replace"`，并向子进程环境注入 `PYTHONUTF8=1`。

### P1：轻量命令 fast path

当前 `version` 也有约 186ms，说明 CLI 顶层 import 是固定成本。将 `version`、`scripts-dir`、`health --quick` 等轻量命令改为 lazy import，业务模块在命令函数内按需加载。

### P1：慢扫描器进程内调用

`G-CODE-1`、`G-09`、`G-RA-4/5/6` 目前通过子进程调用独立扫描脚本。保留脚本 CLI，同时暴露可进程内调用的函数，由 gates 层直接调用，避免重复 Python 启动与编码问题。

### P2：共享文件清单与缓存

新增 `tools/lib/file_inventory.py`：

- 一次遍历得到 `md/java/xml/yaml/pom/test-report` 文件集合
- 使用 `os.walk` 剪枝排除 `.git/node_modules/target/build/dist/.ae-sdd` 等目录
- 扫描器共享 inventory，避免重复 `rglob`
- 持久化缓存 key 包含 path、size、mtime、scanner 版本、Python 版本和配置

### P3：perf doctor

`ae-sdd perf doctor` 根据统计数据输出建议：

| 现象 | 建议 |
| --- | --- |
| gate > 1000ms | 检查 scanner/cache/inventory |
| bootstrap > 150ms | 检查 lazy import |
| subprocess > 500ms | 优先改进程内调用 |
| cache miss ratio > 80% | 输出缓存失效原因 |
| Windows decode error | 检查是否绕过 `runtime_exec` |

## 验证

新增测试：

- `tools/tests/test_runtime_stats.py`（同时覆盖 `runtime_exec` UTF-8 span）
- `tools/tests/test_cli_perf.py`
- `tools/tests/test_gates.py` 后续可增加 `durationMs/slowest/details` 覆盖
- 扫描器行为回归：优化前后 findings 数量一致

性能阈值不直接写入 CI 强约束。CI 只验证契约和行为；耗时目标由本地 benchmark 或 `perf doctor` 报告。

## 文档落点

实现后需要同步：

- `source/docs/ae-sdd-implementation-architecture.md`
- `source/docs/ae-sdd-design.md` 工具链能力摘要
- `README.md` 详细文档索引
- `source/standards/update-graph.json`（如新增 UC/UG）
- `source/CHANGELOG/`
