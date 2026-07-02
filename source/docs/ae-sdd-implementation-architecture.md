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
| Runtime 编译 | `scripts/compile_skill_runtime.py` / `tools/lib/runtime_verify.py` | compact runtime 生成与校验 | 输出必须字节级幂等 |
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
- 新增工具链模块放 `tools/lib/`，默认随 tools 复制。
- 新增独立运行时脚本放 `scripts/` 时，必须更新 `build_dist.py` 白名单。
- 分发器只能安装编译后 package，不能直接安装 `source/`。

## 9. Runtime Stats 预留架构

运行时统计方案归档在 [`plans/2026-07-02-runtime-stats-performance-plan.md`](plans/2026-07-02-runtime-stats-performance-plan.md)。

落地时建议新增：

| 模块 | 职责 |
| --- | --- |
| `tools/lib/runtime_stats.py` | command/span 统计、JSONL 落盘、慢点汇总 |
| `tools/lib/runtime_exec.py` | 统一子进程执行、UTF-8、timeout、span 接入 |
| `ae-sdd perf report` | 查询运行统计 |
| `ae-sdd perf doctor` | 根据慢点输出优化建议 |

统计不得污染 stdout；`--json` 业务输出保持可解析。

## 10. 设计-实现对齐闭环

| 防线 | 工具 | 覆盖 |
| --- | --- | --- |
| 快速同步检查 | `ae-sdd update-check` | 版本、命令、门禁、scanner 分发、runtime 编译一致性 |
| 对齐审计 | `alignment_audit.py` 注入 UC-08~13 | 门禁承诺、state 字段、幽灵命令、注册完整性 |
| 迭代检查 | `ae-sdd iteration-check` | HS 物理实现、过时描述、未接入模块粗筛 |
| Runtime 校验 | `ae-sdd runtime verify` / UC-15 | compiled runtime manifest、load_order、幂等输出 |
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

