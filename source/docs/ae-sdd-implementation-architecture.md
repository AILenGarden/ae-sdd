# ae-sdd 实现架构说明书

> v3.11.4 · 面向 ae-sdd 维护者。本文档描述代码实现结构、模块边界和设计-实现对齐规则；能力语义仍以 [`ae-sdd-design.md`](ae-sdd-design.md) 为入口。

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
  skill-fallbacks/              源 SKILL 瘦身前完整原文，语义 fallback
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
| 状态机 | `tools/lib/state.py` | phase、PRD/work item 状态、事件日志、StoryName/docPath 绑定 | 改状态字段需同步 gates/hook/tests；正文绑定只存指针 |
| StateStore | `tools/lib/state_store.py` | Work Item allowed-root、exclusive create、lease、fencing、revision CAS、idempotency、atomic persistence | 所有 mutation 必须经 StateStore；state/lock/lease/temp 均做 resolved containment；并发/过期/损坏场景需 subprocess 与 fail-closed 测试 |
| Typed operations | `tools/lib/operations.py` | LLM 可发现、可校验、可执行的 operation registry 与适配器 | 新 operation 必须有 schema、稳定错误码、CLI/文档和 focused tests |
| 门禁 | `tools/lib/gates.py` | GATE_REGISTRY、check_all、单 gate 实现、Work Item-scoped CodingPlan profile 选择 | 改门禁需同步 UC-02/UC-03/test_gates |
| update-check | `tools/lib/update_graph.py` | UG/UC 检查、变更影响查询 | 改图谱需同步 JSON、锚点和测试 |
| 对齐审计 | `tools/lib/alignment_audit.py` | UC-08~13 深度对齐验证 | report-only 与阻断语义需明确 |
| 迭代检查 | `tools/lib/iteration_check.py` | IC-1~4 设计-实现一致性粗筛 | 不替代人工语义复核 |
| 文档存取 | `tools/lib/document_storage.py` | intent 驱动的文档定位、保存、finalize；原生 StoryName 精确解析与元数据校验 | SKILL 不应自行拼接产物路径；禁止 fuzzy Story ID 选择 |
| Review Batch | `tools/lib/review_batch.py` / `review_loop.py` | session/batch 状态、fingerprint、失败分类、预算、retry merge、legacy projection | 新状态字段需同步 state/gates/tests；`STALLED` 不得映射为 PASS；平台失败只重试缺失角色 |
| 增量质量 | `tools/lib/baseline.py` / `verification_plan.py` / `evidence.py` | baseline delta、最小验证计划、成功证据 manifest 与安全复用 | baseline 创建必须显式批准；tampered/touched debt 阻断；缓存命中必须验证 artifact/freshness hash |
| 资产索引 | `tools/lib/assets_index.py` | assets 读取、outline、section、query、stats | 缓存变更需测试缓存失效 |
| Hook 层 | `gate_intercept.py` / `prompt_inject.py` / `stop_check.py` | 工具调用前、提示注入、响应后校验 | HARNESS 声明必须能追到实现 |
| 源 SKILL 瘦身 | `scripts/slim_source_skills.py` / `source/skill-fallbacks/**` | 按标准识别源 SKILL 语义、渲染 slim entry、保留完整 fallback、校验模板一致性 | 已瘦身文件默认跳过；schema 升级必须从 fallback 重渲染，禁止二次摘要 |
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
- G-PATH 的 SSOT 豁免按 scan root 下的严格相对路径识别，仅覆盖 canonical document-storage source entry、source full fallback 和 compiled runtime fallback；basename 相同但父目录错误的文件不得豁免。
- G-PATH 项目侧只扫描 `.ae-sdd/memory/**/*.md`、顶层 `AGENTS.md`/`CLAUDE.md`/`MEMORY.md` 与 `.harness/memory/**/*.md`；`.ae-sdd/drafts/**/*.md` 属于过程产物，不纳入该 gate。`current_story` 不作为项目路径静默过滤条件。
- scanner 输出 JSON 必须包含项目根 `root`、`status`、`scannedPaths`、顶层统计、同值 `reportStats` 和 `findings[]`；G-CODE-1 对路径安全/唯一性/scope 覆盖、exit/status、finding schema 及全部计数执行 fail-closed attestation。production eligibility 与 scanner 枚举使用同一文本代码边界：Java/Kotlin/XML/YAML/properties 加 `.py/.js/.ts`，生成目录、虚拟环境/site-packages、`__tests__` 和常规 Python/JS/TS test/spec 命名在两侧均排除。scanner 用 Python AST 只定位自身 `LINE_RULES` 与 metadata 常量赋值范围；业务代码同 URI 不豁免，真实 pom metadata 由 XML 解析确认；不提供通用 inline suppression。
- Coding scanner 仅在 XML 解析确认真实 Maven POM 根元素后豁免标准 `xmlns`/`xsi`/`schemaLocation` 元数据 URL；Java 或 XML 中的实际外部 endpoint 仍按 `hardcoded-external-url` 阻断。
- 新增 `scripts/*_scan.py` 必须加入 `scripts/build_dist.py` runtime_scripts 白名单。
- 高频 scanner 应优先支持进程内调用，子进程 CLI 仅作为兼容入口。
- Review Batch、baseline、VerificationPlan 和 evidence 均通过 `tools/lib/` 提供纯 Python API，CLI 只做参数适配；所有 fingerprint 使用 canonical JSON，避免依赖 Git/mtime。状态写入保留 `reviewLoop` 兼容投影，门禁优先读取 `reviewSession`/batch v2。
- `verification.plan` 先规范化项目内真实文件，再由 `StateStore` 以 lease + fencing token + revision CAS 原子写入 Work Item 绑定 plan；`dryRun` 不写任何状态文件。计划同时保留兼容 `inputFingerprint` 和专用 `evidenceInputFingerprint`，Evidence 命令不得使用 `planFingerprint`。G-09 与 G-CODE-1 共用 changedPaths containment、plan fingerprint 和 evidence/artifact hash 校验；G-CODE-1 再过滤测试/文档，只保留生产代码，要求 scanner `scannedPaths` 完整覆盖该 scope，空生产 scope 阻断，无 scope 保持全仓结果。evidence artifact 必须是项目内文件，record 复制到内容寻址 immutable snapshot；同 logical key 的旧 active entry 标记 superseded，finalize 只校验 active snapshot，旧 schema manifest 读取不静默改写。
- LLM 写入口统一由 `tools/lib/operations.py::OperationRegistry` 提供。`ops describe` 暴露 JSON Schema，`ops next` 暴露当前 revision/lease/nextActions，`ops execute` 执行显式 Work Item 的 typed operation；未知 operation、raw `state.patch`、项目根不匹配、路径越界和缺少 lease/revision/idempotency 均 fail closed。`ae-sdd state write` 仅保留为通过 StateStore 短租约的兼容 adapter。
- G-13 的 DR exemption 只由 `entryNode=STORY && scale=中` 触发；实现继续校验 Story 本体与成熟阶段的 Task/CodingReport/CodeReview 存在及引用关系。
- G-07/G-08/G-14/G-CODEPLAN-SRC 共用 `_resolve_codingplan_doc()`，通过 `document_storage.resolve_scoped_artifact()` 按显式 Work Item 优先定位，Story 仅作唯一兼容 fallback。G-08 根据 `state.scale` 选择 full 或 micro profile；G-14 仅对无 Story 的 standalone 微任务返回可审计 N/A。
- G-02/G-14 共用 `_resolve_story_doc()`，底层 `resolve_story_document()` 按 bound path、精确非 glob StoryName、无 StoryName 时 Story-category-only ID 路径解析。Task/Coding 等同名文档不参与；canonical 多候选同样 ambiguity。正式文件反校验 `Story ID` 元数据；歧义、漂移和非法 basename 以稳定错误码阻断。
- 🆕 v3.9.1 注册表模式：同族门禁（如 G-DR-CTX/G-STORY-CTX/G-TESTCASE-CTX/G-TASK-CTX 四个上下文加载准入门禁）用 `CONTEXT_GATE_REGISTRY` 注册表 + 单个 `_check_context_loaded` 函数服务多个 gate_id，避免每门禁重复写 scale 豁免/phase 感知/逐项校验逻辑；4 个薄封装 `check_g_*_ctx` 对齐 `CHECK_FUNCS` 的 `(project_dir, st, current_story)` 签名，内部转发到统一实现。

## 7. 项目侧状态与缓存

项目侧运行状态集中在 `.ae-sdd/`：

| 路径 | 用途 |
| --- | --- |
| `.ae-sdd/config.yaml` | 项目配置 |
| `.ae-sdd/state.json` | 不再作为 active state、mirror 或 fallback；旧项目残留文件只能视为历史数据，不得参与新状态解析 |
| `.ae-sdd/session-context/` | 会话级 work-item 绑定缓存；`UserPromptSubmit` 从当前 prompt/cwd 解析真实 work-item 并写入，`PreToolUse` 只用同一 session key 读取，禁止跨会话共享 |
| `.ae-sdd/.hook-activity/` | session 级 turn token；只记录激活时间、最近时间和来源，不保存 prompt 正文；Stop 成功或普通新 prompt 清理；不参与 Work Item lease |
| `.auto-engineering/{workItemKey}/state.json` | work item 独立状态机；新建入口为 `ae-sdd state new --id <ID> --entry-node <PRD\|DR\|STORY\|TASK>`，目录名为 R6 顶层名（🆕 v3.10.1 带随机 UUID 前缀，如 `{uuid}-PRD-001`）；v3.11.3 可保存 StoryName/docPath 正文指针 |
| `.ae-sdd/memory/` | 分区 compact 记忆；task(L1) 默认任务级、project(L2) 跨任务复用，UserPromptSubmit 任务优先注入并只用 project 补充 |
| `.ae-sdd/plugins/` | 项目层插件注册 |
| `.ae-sdd/cache/` | 工具链缓存，新增缓存优先放这里 |
| `.ae-sdd/runtime-stats/` | Runtime Stats JSONL，本地观测数据，可清理，不进入版本控制 |
| `.ae-sdd/baselines/` | 用户批准的 gate baseline（默认 G-CODE-1），带 ruleset/content hash | 不自动创建；规则或触碰文件时重新确认 |
| `.ae-sdd/doc-aliases.json` | 旧文档路径到 canonical 正文的 alias registry | 只存指针，不存第二份正文 |
| `.auto-engineering/{story}/evidence/manifest.json` | Story 验证证据索引与复用条件 | 保存 canonical content hash；失败证据、manifest/input/artifact hash 不一致均不可复用 |

`PreToolUse` 解析 Work Item 时遵循“显式优先、隐式 fail-closed”：真实 `ae-sdd` Bash 命令携带 `--work-item` 时，先通过 `paths.find_work_item_state_path()` 定位该状态，再执行原有 phase、memory 与 gate 校验；目标不存在时直接拒绝，不得回退到其他候选。未携带显式目标时，才依次使用同 session 绑定和默认候选解析；多个活跃候选仍以歧义错误阻断。有效的显式目标可写入当前 session 绑定，使同一会话的后续 Write/Edit 继续落在该 Work Item 上。

`PreToolUse` 对 ae-sdd Bash 命令只维护一套 token-aware 前缀解析：接受直接 `ae-sdd ...`、`python C:/.../ae-sdd ...`、`python "C:/.../ae-sdd" ...`，以及带引号的 Python 可执行文件路径；各 state、memory、assets 与 readonly 分支消费同一组规范化参数。解析只确认命令身份，不负责放行；`_CHAIN_RE` 与 `_REDIRECT_RE` 仍在 fast path 外层拒绝链式、命令替换和重定向载荷。引号未闭合、解释器或脚本 basename 不精确匹配时返回“非 ae-sdd 命令”，不得猜测或前缀模糊匹配。

Hook activation 与 Work Item state/lease 解耦：`prompt_inject` 仅在显式 `/ae-sdd` turn 创建 `.ae-sdd/.hook-activity/<session-hash>.json`，普通 prompt 先清理残留 token 后直接返回空 payload；`gate_intercept` 只对 active token 执行 phase/path/memory 门禁，并允许明确的 ae-sdd 写流程入口启动当前 turn；Stop CLI 在 active token 下执行 `stop_check`，成功或 fail-open 释放 token，阻断重试保留 token。旧 `.session-engaged` 文件不再作为激活依据。

`tools/lib/state.py` 保留状态字段、phase 流程和终态不变量；`tools/lib/state_store.py` 是所有 Work Item mutation 的唯一并发所有者。StateStore 在构造、锁、lease、state 与临时替换文件处校验 resolved path 仍位于 allowed root；`create()` 在事务锁内独占创建，已有 state 不覆盖。mutation 在同一 Work Item 锁内二次读取 lease/state，检查 fencing token 与 revision CAS，再用临时文件 + fsync + atomic replace 写回；lease 过期接管会递增 fencing token，重复 idempotency key 返回原结果。`phase`/`history` 表示生命周期主状态；`currentPhase`、`currentStep`、`completedSteps`、`pendingOutputs`、`codingRound` 是工作流投影字段，不能独立滞留在旧步骤。`set_phase()` 和 `set_story_substate_phase()` 写入生命周期 phase 时必须级联同步投影；`write_state()` 落盘前执行终态不变量校验。Story 正文绑定在嵌套 state 使用 `storyStates[storyId].storyName/docPath`，扁平兼容 state 使用 `storyName/storyDocPath`；`state bind-story-doc` 在解析成功后通过短租约 mutation 写入，重复绑定不增加 revision；`state new --story-name` 吸收既有父 state 时把 Story add + binding 合并为一次 CAS，新 state 则以完整内存对象 exclusive-create，失败不留半状态。

新增项目侧文件必须说明是否可删、是否进入版本控制、是否参与 gate。

## 8. 构建与分发

构建链路：

```text
source/
  -> scripts/slim_source_skills.py
  -> scripts/build_dist.py
  -> scripts/compile_skill_runtime.py
  -> dist/ae-sdd/
  -> scripts/install.py 或 scripts/distribute.py
  -> Agent skills runtime
```

规则：

- `dist/ae-sdd/` 是构建产物，不手工维护。
- `source/SKILL.md` 与 `source/skills/**/*.md` 可以是 slim entry，但必须符合 `ae-sdd-source-slim/v2`，完整原文必须在 `source/skill-fallbacks/**`。
- `scripts/slim_source_skills.py --validate` 必须能验证 fallback 哈希、语义 inventory hash、标准/模板路径和模板重渲染一致性。
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
  -> source/skill-fallbacks/SKILL.full.md               # 源瘦身前完整语义
  -> dist/ae-sdd/SKILL.md
  -> runtime/{boot,route,gates,flow,macros}.compact.md
  -> runtime/fallback/SKILL.full.md

source/skills/**/*.md
  -> source/skill-fallbacks/skills/**/*.full.md         # 源瘦身前完整语义
  -> dist/ae-sdd/skills/**/*.md
  -> runtime/subskills.compact.md
  -> runtime/skills/**/{manifest.json,boot.compact.md,outline.compact.md,fallback/SKILL.full.md}
```

实现边界：

- 编译器只读取母版与工具注册表，不解释业务流程，不替代 `ae-sdd gates check`。
- 源瘦身器负责修改 `source/SKILL.md` 与 `source/skills/**/*.md`，runtime 编译器不负责瘦身源文件。
- runtime 编译器发现 `source_slimmed: true` 时，必须从 `source_fallback` 读取完整原文作为 runtime fallback 和 outline 抽取输入。
- `scripts/build_dist.py` 负责把母版复制为 dist，再调用 `scripts/compile_skill_runtime.py` 生成 runtime。
- `tools/lib/runtime_verify.py` 负责校验 installed package 是否为完整 compiled runtime；如果存在 source child SKILL 而缺少 compiled 子入口，必须报错。
- `tools/lib/update_graph.py` 的 UC-15 必须把 `SKILL.md`、`runtime/**`、`skills/**/*.md` 都纳入幂等快照。

## 9. Runtime Stats 架构

运行时统计 P0 已落地；性能优化的阶段性方案归档在 [`plans/2026-07-02-runtime-stats-performance-plan.md`](plans/2026-07-02-runtime-stats-performance-plan.md)。

> 🆕 2026-07-03(B2/B3)：
> - **P1 lazy import 未实施**：plan 文档 P1 声称将 CLI 顶层 import 改 lazy import 以降 bootstrap 固定成本（实测 ~186ms），至今未落地。`perf doctor` 在 avg>150ms 时会提示该挂账。源 SKILL 瘦身（降 Agent token，已落地）与本项（降进程 ms，未落地）是两套不同成本层，勿混。
> - **scale 维度已加入**：runtime_stats 事件现记录 `scale` 字段（从项目 state.json 探测），`summarize_events` 输出 `byScale` 分桶与 `scaleRatios`（微/小/中 vs 大 的平均开销比），用于诊断"微任务 vs 大任务开销比例失调"。

| 模块 | 职责 |
| --- | --- |
| `tools/lib/runtime_stats.py` | command/span 统计、JSONL 落盘、慢点汇总、敏感 argv 脱敏、`AE_SDD_STATS`/`AE_SDD_STATS_DIR` 环境开关、🆕 scale 探测与按 scale 分桶（B3） |
| `tools/lib/runtime_exec.py` | 统一子进程执行、UTF-8、timeout、span 接入 |
| `tools/bin/ae-sdd` | 在 `args.func(args, parser)` 外层记录命令事件；`perf report/doctor/clear` 查询、诊断、清理统计；🆕 `_perf_advice` 含 scale 比例失调规则与 lazy import 挂账提示（B2/B3） |
| `tools/lib/gates.py` | `check_all()` 为每个 gate 增加 span，并在 `summarize()` 输出 `durationMs` 与 `slowest` |

统计存储与输出规则：

- 项目内运行写入 `.ae-sdd/runtime-stats/YYYY-MM-DD.jsonl`；无项目 `.ae-sdd/` 时写入系统临时目录 `ae-sdd/runtime-stats/`。
- 测试和临时环境可用 `AE_SDD_STATS_DIR=<dir>` 改写存储目录；`AE_SDD_STATS=0` 可关闭统计。
- 统计不得污染业务 stdout；`--json` 业务输出保持可解析。查询统计必须显式调用 `ae-sdd perf report --json`。
- `perf clear` 清理当前统计文件并抑制自身 command event，避免刚清理又写入一条 clear 记录。
- 子进程调用默认注入 `PYTHONUTF8=1` 和 `PYTHONIOENCODING=utf-8`，并使用 `encoding="utf-8", errors="replace"` 解码。
- CLI 入口在导入业务模块前将 stdout/stderr `reconfigure(encoding="utf-8")`；回归测试会在父环境声明 `PYTHONIOENCODING=gbk` 时严格按 UTF-8 解码 gate 输出，防止 Windows 代码页回退。
- 🆕 2026-07-03(B3)：`scale` 字段由 `start_command` 内部 `_detect_scale()` 从项目 state.json 读取（无则 null，不阻断业务），写入事件顶层；`summarize_events` 的 `byScale`/`scaleRatios` 用于 `perf doctor` 比例失调诊断。

边界：Runtime Stats 只记录命令名、脱敏 argv、耗时、退出码、span 属性和 scale（任务规模），不记录业务文档正文；它用于定位慢点与比例失调，不作为硬门禁。

## 9.5 分发器注册表架构（🆕 2026-07-03 注册表模式）

分发目标从「`__init__.py` 硬编码 Python 列表」改为「外部 JSON 注册表 + 协议模板」，支持注册/注销/扫描。

### 注册表文件

`~/.ae-sdd/distributors.json`（用户环境态，与 plugins/ 同级）。首次运行无文件时用种子初始化（含 claude/codex/zcode/hermes/mavis，mavis 默认 `enabled:false` 反映无 daemon 环境）。

每条目字段：`name` / `protocol`(copytree|harness_mount) / `target_path` / `detect`(always|path_exists|cli_exists) / `detect_cli` / `enabled` / `registered_at` / `notes`。

### 协议模板（内置，数据填参构造实例）

| 协议 | 模板类 | 适用 | 复杂度 |
| --- | --- | --- | --- |
| `copytree` | `CopytreeDistributor` | claude/codex/zcode/hermes 及同类 | 备份→复制→校验→清旧 .bak |
| `harness_mount` | `HarnessMountDistributor` | mavis 及同类 | compile(build_harness)→mount→verify→cleanup(-N 副本+sqlite) |

注册一个 Agent = 选协议模板 + 填 target_path/detect 参数构造实例；注销 = 注册表除名，实例不再构造。旧 5 个 `*.py` 子类降级为兼容 shim，逻辑迁入模板。

### CLI 管理

`ae-sdd distributor list|register|unregister|enable|disable|scan`。注销 mavis：`ae-sdd distributor disable mavis`（软注销，保留条目可恢复）或 `unregister mavis`（硬注销，删条目）。`scan` 扫描 `~/.*/skills/` 识别已安装 Agent 并建议注册命令，不越权委托 Agent 安装。

### 数据流

```
~/.ae-sdd/distributors.json (enabled + detect 过滤)
  → _registry.get_active_distributors() 构造实例
  → distribute.py 遍历实例调 install()
  → CopytreeDistributor: copytree 编译后 dist
  → HarnessMountDistributor: build_harness + mavis harness mount
```

### 边界

注册表只管"分发到哪、用什么协议"，不管编译（编译在 `build_dist.py`，分发前的硬约束保留）。注册表是用户环境态，不进 git；母版不预置注册表，首次运行种子生成。

## 10. ae-sdd Monitor 架构

Monitor 是本仓库下的独立桌面应用，位置为 `apps/ae-sdd-monitor/`。它读取项目侧 ae-sdd 状态文件并做 UI 投影，不进入 `dist/ae-sdd/` runtime 编译链，也不作为 Agent skill 分发内容。

| 模块 | 职责 |
| --- | --- |
| `apps/ae-sdd-monitor/src/main.js` | Electron 主进程、窗口生命周期、目录选择、路径打开 IPC、UI 偏好读写、父目录文件 watcher |
| `apps/ae-sdd-monitor/src/preload.js` | 只暴露受控 `monitorApi` 与 watcher 事件订阅，隔离 renderer 与 Node 能力 |
| `apps/ae-sdd-monitor/src/workspace.js` | 扫描父目录、识别 `.ae-sdd/` 工作区、读取 state/config/runtime-stats/memory、派生展示状态、阶段轴、workItemKey 身份、任务列表和活跃任务 |
| `apps/ae-sdd-monitor/renderer/src/App.tsx` | React + TypeScript renderer；左侧项目/任务两级 keyed 列表、筛选、右侧详情 Tab、本地 UI 状态、响应式静默刷新、任务级局部更新、目录选择反馈、交互动效触发和偏好恢复 |
| `apps/ae-sdd-monitor/renderer/src/main.tsx` / `renderer/index.html` | Vite renderer 入口，构建到 `dist/renderer/` 后由 Electron 主进程加载 |
| `apps/ae-sdd-monitor/src/styles.css` | 黑白圆角类 Mac 外观、iOS 风格轻量交互动效、折叠/切换/按压反馈和 reduced-motion 降级 |
| `apps/ae-sdd-monitor/test/workspace.test.js` | 扫描、YAML 读取、work item、Memory、Runtime Stats 聚合的契约测试 |
| `apps/ae-sdd-monitor/scripts/package-win.ps1` | Windows 本地打包、安装 zip、自解压 setup 生成 |
| `apps/ae-sdd-monitor/scripts/package-mac.sh` | macOS 本地打包入口，调用 electron-builder 生成 dmg/zip |
| `apps/ae-sdd-monitor/scripts/package-mac-unsigned.ps1` | 跨平台生成未签名 macOS `.app.zip`，基于 Electron darwin runtime 注入 app 资源 |
| Electron userData `preferences.json` | 保存上次父目录、选中工作区、选中任务、自动刷新开关和主题；不写项目侧 `.ae-sdd/` |

数据流：

```text
用户选择父目录
  -> Electron dialog 返回 rootPath
  -> main.js 保存/读取 userData/preferences.json
  -> workspace.js 递归扫描包含 .ae-sdd/ 的目录
  -> 读取 .ae-sdd/config.yaml / .ae-sdd/state.json
  -> 读取 .auto-engineering/{workItemKey}/state.json
  -> 读取 .ae-sdd/memory/**/*.jsonl / .ae-sdd/memory/.stage/*.json
  -> 读取 .ae-sdd/runtime-stats/*.jsonl
  -> workspace.js 派生 phaseTimeline / activeWorkItems / tasks / memory
  -> React renderer 展示项目/任务两级列表、阶段轴、Memory、事件流、活跃任务和详情
  -> main.js 监听 .ae-sdd/ 与 .auto-engineering/ 文件变化并通过 preload 通知 renderer
  -> React renderer 依靠稳定 key 和 props diff 更新对应组件；同一项目任务切换只更新右侧数据片段和侧边栏选中态
  -> styles.css 提供只作用于 UI 的折叠、切换、按压和悬浮动效
```

边界：

- Monitor 全程只读；扫描、刷新、切换 Tab 不得写项目文件。
- Monitor 的状态枚举是 UI 派生值，不新增 ae-sdd state schema。
- Monitor 的偏好文件只保存用户界面上下文，不保存 ae-sdd 业务状态。
- 响应式刷新采用 main 侧 `fs.watch` + renderer 侧 debounce：`.ae-sdd/` 与 `.auto-engineering/` 变化触发静默刷新；低频轮询只作为 watcher 漏事件兜底；不使用会改变项目状态的命令。
- renderer 不得在任务切换或静默刷新时先把 `detail` 置空再整页重画；React 组件不得用整块 `innerHTML` 替换侧边栏/详情页，只有首次加载或无详情态才显示空态。
- 交互动效只在 renderer/CSS 层表达本地 UI 反馈；不得触发 ae-sdd 命令、不得写 `.ae-sdd/`、不得成为状态权威。
- `PHASE_FLOWS`、state 字段、Memory JSONL/stage 字段、Runtime Stats JSONL 字段变化时，必须同步 [`ae-sdd-monitor-design.md`](ae-sdd-monitor-design.md)、`workspace.js` 和测试。
- Monitor 不运行 `ae-sdd gates check`，不替代 CLI/gate 的硬判断；最多展示已有 state/runtime 线索。
- Mac `.dmg`/签名最终构建必须在 macOS runner 上完成；Windows runner 可生成 Windows setup exe/zip 和未签名 macOS `.app.zip`。
- `source/standards/update-graph.json:UG-22` 负责把 ae-sdd 设计/实现/state/runtime 变化级联到 Monitor 文档、解析器、测试和 README。

## 11. 设计-实现对齐闭环

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

## 12. 新实现设计写入规则

每个系统级设计必须先在主设计文档的 Design Ledger 记录“要解决的问题、核心决策、预期价值、验证证据和版本状态”。实现架构文档只记录模块边界和数据流，不重复叙述设计动机。每次迭代必须在 changelog 填写 `Design ledger impact`：设计语义变化指向 D-xxx；无设计语义变化明确填写 N/A。维护者通过 `ae-sdd update-check --affected` 查询 UG-28，并运行 UC-20；对话中的说明不能代替台账或机器证据。

| 内容 | 写入位置 |
| --- | --- |
| 能力为什么存在、用户语义、流程边界 | `source/docs/ae-sdd-design.md` 对应章节 + §0 Design Ledger |
| 设计 ID、问题、预期价值、验证证据、版本状态 | `source/docs/ae-sdd-design.md` §0 Design Ledger |
| 模块分层、文件职责、数据流、缓存、子进程、hook、build/distribute | 本文件 |
| 单次技术方案、阶段性取舍、性能基线 | `source/docs/plans/*.md` |
| 发版事实、影响范围、验证命令、Design ledger impact | `source/CHANGELOG/*.md` |
| Agent 执行入口和路由 | `source/SKILL.md` |
| 阶段内具体规则 | 对应 `source/skills/**/**-skill.md` |
| 机器可读依赖闭环 | `source/standards/update-graph.json` |
