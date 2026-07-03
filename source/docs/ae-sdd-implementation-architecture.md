# ae-sdd 实现架构说明书

> v3.7.4 · 面向 ae-sdd 维护者。本文档描述代码实现结构、模块边界和设计-实现对齐规则；能力语义仍以 [`ae-sdd-design.md`](ae-sdd-design.md) 为入口。

## 1. 文档边界

| 文档 | 职责 | 不承担 |
| --- | --- | --- |
| `source/SKILL.md` | Agent 运行入口、流程编排、门禁与子 SKILL 路由 | 代码模块架构说明 |
| `source/docs/ae-sdd-design.md` | 系统能力设计：能力是什么、为什么存在、当前能力边界 | 每个函数/文件的完整实现地图 |
| `source/docs/ae-sdd-implementation-architecture.md` | 实现架构：代码分层、模块边界、运行时数据流、变更闭环 | 历史方案细节和发版流水账 |
| `source/docs/plans/*.md` | 单次方案、调研、迁移记录 | 当前架构权威状态 |
| `source/CHANGELOG/*.md` | 变更原因、影响范围、验证方式 | 设计正文 |

原则：能力文档写稳定语义，实施细节写实现架构，历史推导写 plans 或 CHANGELOG。

## 2. 实现分层

```text
source/                         母版文档与方法论 SSOT
  SKILL.md                      Agent 主入口
  skills/                       子 SKILL
  docs/                         能力设计、实现架构、方案归档
  standards/                    机器可读或人读标准
  templates/                    产物模板

tools/                          CLI 工具链
  bin/ae-sdd                    argparse 入口与命令分发
  lib/                          业务实现模块
  tests/                        工具链测试

scripts/                        构建、安装、分发与运行时扫描脚本
dist/ae-sdd/                    编译后分发包，构建产物，不手工维护
  SKILL.md                      编译后主入口 bootloader
  skills/**/*.md                编译后子 SKILL bootloader，非原文
  runtime/                      compact runtime、manifest、fallback 原文
    subskills.compact.md        子 SKILL 编译入口索引
    skills/**                   子 SKILL 局部 manifest/boot/outline/fallback
harness/                        派生适配层，不手工改生成物
```

## 3. 运行时数据流

```text
用户/Agent 调用 ae-sdd
  -> tools/bin/ae-sdd 解析命令
  -> tools/lib/* 执行业务逻辑
  -> scripts/*_scan.py 提供运行时扫描能力
  -> .ae-sdd/state.json / memory / cache 持久化项目侧状态
  -> output.py 输出结构化结果
```

硬门禁判断以 CLI 输出为准。SKILL 文档可以声明流程纪律，但不能替代 `tools/lib/gates.py`、`tools/lib/state.py`、`tools/lib/update_graph.py` 等工具链实现。

## 4. 模块职责

| 子系统 | 主要文件 | 职责 | 变更要求 |
| --- | --- | --- | --- |
| CLI 入口 | `tools/bin/ae-sdd` | 命令注册、参数解析、输出模式分发 | 新命令必须补测试和 SKILL/README 引用 |
| 输出层 | `tools/lib/output.py` | stdout/stderr 约定、JSON 输出 | 不在业务模块手写不一致输出 |
| 路径层 | `tools/lib/paths.py` | 母版、项目、state、文档路径定位 | 禁止各模块重复拼路径 |
| 状态机 | `tools/lib/state.py` | phase、PRD/work item 状态、事件日志 | 改状态字段需同步 gates/hook/tests |
| 门禁 | `tools/lib/gates.py` | GATE_REGISTRY、check_all、单 gate 实现 | 改门禁需同步 UC-02/UC-03/test_gates |
| update-check | `tools/lib/update_graph.py` | UG/UC 检查、变更影响查询 | 改图谱需同步 JSON、锚点和测试 |
| 对齐审计 | `tools/lib/alignment_audit.py` | UC-08~13 深度对齐验证 | report-only 与阻断语义需明确 |
| 迭代检查 | `tools/lib/iteration_check.py` | IC-1~4 设计-实现一致性粗筛 | 不替代人工语义复核 |
| 文档存取 | `tools/lib/document_storage.py` | intent 驱动的文档定位、保存、finalize | SKILL 不应自行拼接产物路径 |
| 资产索引 | `tools/lib/assets_index.py` | assets 读取、outline、section、query、stats | 缓存变更需测试缓存失效 |
| Hook 层 | `gate_intercept.py` / `prompt_inject.py` / `stop_check.py` | 工具调用前、提示注入、响应后校验 | HARNESS 声明必须能追到实现 |
| Runtime 编译 | `scripts/compile_skill_runtime.py` / `tools/lib/runtime_verify.py` | 主入口 compact、全量子 SKILL bootloader、局部 runtime、fallback 原文生成与校验 | `SKILL.md`、`runtime/**`、`skills/**/*.md` 输出必须字节级幂等 |
| 构建分发 | `scripts/build_dist.py` / `scripts/distribute.py` | source -> dist -> runtime 安装 | 不把手工改动写入 dist |
| 运行时扫描器 | `scripts/*_scan.py` | 静态扫描，输出 JSON 契约 | 新 scanner 需入 build_dist 白名单 |

## 5. CLI 入口规则

`tools/bin/ae-sdd` 应保持为薄入口：

- argparse 注册命令和参数。
- 命令函数可以做少量参数适配。
- 业务逻辑下沉到 `tools/lib/`。
- 新增子命令必须有 `--json` 契约测试。
- 命令引用必须被 `update-check` 覆盖，避免文档声明幽灵命令。

当前 `tools/bin/ae-sdd` 已承载较多命令函数，后续新能力应优先新增 `tools/lib/<capability>.py`，入口只做分发。

## 6. Gate 与 Scanner 规则

门禁分两层：

| 层 | 位置 | 说明 |
| --- | --- | --- |
| gate 编排 | `tools/lib/gates.py::GATE_REGISTRY` / `check_all()` | gate ID、名称、强度、分发到具体检查 |
| scanner 规则 | `scripts/*_scan.py` 或 `tools/lib/*` | 具体静态扫描与 findings 生成 |

规则：

- `GATE_REGISTRY` 是门禁列表的代码权威。
- `check_all()` 必须覆盖每个 gate，不能 stub-pass 掩盖缺口。
- scanner 输出 JSON 必须包含 `status`、统计字段和 `findings[]`。
- 新增 `scripts/*_scan.py` 必须加入 `scripts/build_dist.py` runtime_scripts 白名单。
- 高频 scanner 应优先支持进程内调用，子进程 CLI 仅作为兼容入口。

## 7. 项目侧状态与缓存

项目侧运行状态集中在 `.ae-sdd/`：

| 路径 | 用途 |
| --- | --- |
| `.ae-sdd/config.yaml` | 项目配置 |
| `.ae-sdd/state.json` | 活跃项目或 work item 状态镜像 |
| `.auto-engineering/{workItem}/state.json` | work item 隔离状态 |
| `.ae-sdd/memory/` | 分层记忆 |
| `.ae-sdd/plugins/` | 项目层插件注册 |
| `.ae-sdd/cache/` | 工具链缓存，新增缓存优先放这里 |
| `.ae-sdd/runtime-stats/` | Runtime Stats JSONL，本地观测数据，可清理，不进入版本控制 |

新增项目侧文件必须说明是否可删、是否进入版本控制、是否参与 gate。

## 8. 构建与分发

构建链路：

```text
source/
  -> scripts/build_dist.py
  -> scripts/compile_skill_runtime.py
  -> dist/ae-sdd/
  -> scripts/install.py 或 scripts/distribute.py
  -> Agent skills runtime
```

规则：

- `dist/ae-sdd/` 是构建产物，不手工维护。
- runtime compact 文件由编译器生成，不手改。
- `dist/ae-sdd/SKILL.md` 必须是编译后的主入口 bootloader。
- `dist/ae-sdd/skills/**/*.md` 必须是编译后的子 SKILL bootloader，不允许保留 `source/skills/**/*.md` 原文。
- 子 SKILL 原文 fallback 只允许出现在 `runtime/skills/**/fallback/SKILL.full.md`。
- `runtime/manifest.json` 必须记录 `subskills` 与 `extracts.subskill_count`，并与实际 `source/skills/**/*.md` 数量一致。
- `runtime/subskills.compact.md` 是子 SKILL 入口索引，路由到每个子 SKILL 的局部 `manifest.json`、`boot.compact.md`、`outline.compact.md` 和 fallback。
- 新增工具链模块放 `tools/lib/`，默认随 tools 复制。
- 新增独立运行时脚本放 `scripts/` 时，必须更新 `build_dist.py` 白名单。
- 分发器只能安装编译后 package，不能直接安装 `source/`。

Runtime 编译数据流：

```text
source/SKILL.md
  -> dist/ae-sdd/SKILL.md
  -> runtime/{boot,route,gates,flow,macros}.compact.md
  -> runtime/fallback/SKILL.full.md

source/skills/**/*.md
  -> dist/ae-sdd/skills/**/*.md
  -> runtime/subskills.compact.md
  -> runtime/skills/**/{manifest.json,boot.compact.md,outline.compact.md,fallback/SKILL.full.md}
```

实现边界：

- 编译器只读取母版与工具注册表，不解释业务流程，不替代 `ae-sdd gates check`。
- `scripts/build_dist.py` 负责把母版复制为 dist，再调用 `scripts/compile_skill_runtime.py` 生成 runtime。
- `tools/lib/runtime_verify.py` 负责校验 installed package 是否为完整 compiled runtime；如果存在 source child SKILL 而缺少 compiled 子入口，必须报错。
- `tools/lib/update_graph.py` 的 UC-15 必须把 `SKILL.md`、`runtime/**`、`skills/**/*.md` 都纳入幂等快照。

## 9. Runtime Stats 架构

运行时统计 P0 已落地；性能优化的阶段性方案归档在 [`plans/2026-07-02-runtime-stats-performance-plan.md`](plans/2026-07-02-runtime-stats-performance-plan.md)。

| 模块 | 职责 |
| --- | --- |
| `tools/lib/runtime_stats.py` | command/span 统计、JSONL 落盘、慢点汇总、敏感 argv 脱敏、`AE_SDD_STATS`/`AE_SDD_STATS_DIR` 环境开关 |
| `tools/lib/runtime_exec.py` | 统一子进程执行、UTF-8、timeout、span 接入 |
| `tools/bin/ae-sdd` | 在 `args.func(args, parser)` 外层记录命令事件；`perf report/doctor/clear` 查询、诊断、清理统计 |
| `tools/lib/gates.py` | `check_all()` 为每个 gate 增加 span，并在 `summarize()` 输出 `durationMs` 与 `slowest` |

统计存储与输出规则：

- 项目内运行写入 `.ae-sdd/runtime-stats/YYYY-MM-DD.jsonl`；无项目 `.ae-sdd/` 时写入系统临时目录 `ae-sdd/runtime-stats/`。
- 测试和临时环境可用 `AE_SDD_STATS_DIR=<dir>` 改写存储目录；`AE_SDD_STATS=0` 可关闭统计。
- 统计不得污染业务 stdout；`--json` 业务输出保持可解析。查询统计必须显式调用 `ae-sdd perf report --json`。
- `perf clear` 清理当前统计文件并抑制自身 command event，避免刚清理又写入一条 clear 记录。
- 子进程调用默认注入 `PYTHONUTF8=1` 和 `PYTHONIOENCODING=utf-8`，并使用 `encoding="utf-8", errors="replace"` 解码。

边界：Runtime Stats 只记录命令名、脱敏 argv、耗时、退出码和 span 属性，不记录业务文档正文；它用于定位慢点，不作为硬门禁。

## 10. 设计-实现对齐闭环

| 防线 | 工具 | 覆盖 |
| --- | --- | --- |
| 快速同步检查 | `ae-sdd update-check` | 版本、命令、门禁、scanner 分发、runtime 编译一致性 |
| 对齐审计 | `alignment_audit.py` 注入 UC-08~13 | 门禁承诺、state 字段、幽灵命令、注册完整性 |
| 迭代检查 | `ae-sdd iteration-check` | HS 物理实现、过时描述、未接入模块粗筛 |
| Runtime 校验 | `ae-sdd runtime verify` / UC-15 | compiled runtime manifest、load_order、全量子 SKILL compiled entry、`SKILL.md` + `runtime/**` + `skills/**/*.md` 幂等输出 |
| 单元测试 | `tools/tests/` | 代码契约 |

重大实现变更流程：

```text
1. 查 ae-sdd-design.md 确认能力语义
2. 查本文件确认代码层落点
3. ae-sdd update-check --affected <files>
4. 修改代码/文档/测试
5. ae-sdd update-check
6. 跑相关 tools/tests
7. 写 source/CHANGELOG
```

## 11. 新实现设计写入规则

| 内容 | 写入位置 |
| --- | --- |
| 能力为什么存在、用户语义、流程边界 | `source/docs/ae-sdd-design.md` |
| 模块分层、文件职责、数据流、缓存、子进程、hook、build/distribute | 本文件 |
| 单次技术方案、阶段性取舍、性能基线 | `source/docs/plans/*.md` |
| 发版事实、影响范围、验证命令 | `source/CHANGELOG/*.md` |
| Agent 执行入口和路由 | `source/SKILL.md` |
| 阶段内具体规则 | 对应 `source/skills/**/**-skill.md` |
| 机器可读依赖闭环 | `source/standards/update-graph.json` |
