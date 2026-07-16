# ae-sdd 系统能力说明书

> v3.11.6 · 面向开发者、LLM Agent 与项目接入方
>
> 本文档是**系统能力设计入口**，说明 ae-sdd 的能力语义、边界和当前实现状态。代码分层、模块职责、运行时数据流和变更闭环统一维护在 [`ae-sdd-implementation-architecture.md`](ae-sdd-implementation-architecture.md)。若本文与代码实现冲突，以 CLI/测试输出为准，并同步修正文档。

## 过程产物模型

RA、DR、Story 是核心设计文档。Proposal、GeneratePlan、CodingReport、TestReport、CodeReview 报告和其他过程 Markdown 停止新写入；历史文件只读。普通测试设计内嵌 Story 验证矩阵，复杂矩阵才使用独立 TestCase；Task 仅用于大型并行拆分。编码前审核使用 `state.executionPlan`，测试使用 evidence manifest，Review 使用 `state.review.status/findings`。任何时候都不写 changelog。

---

## 0. 设计问题与价值总览（Design Ledger）

本表是当前系统级设计的导航和问题台账，覆盖本文件 §1~§21。它回答四个问题：为什么存在、采用什么决策、预期带来什么价值、用什么证据验证。详细设计仍在对应章节，代码职责仍以 `ae-sdd-implementation-architecture.md` 为准。

### 0.1 记录规则

- `预期价值` 是设计假设，不等同于已测收益；只有 `验证证据` 中有命令、测试或运行指标时，才能宣称已验证。
- 历史设计缺少精确版本时记录 `历史版本（精确版本待补）`，不得凭记忆编造版本号。
- 每次迭代必须在 changelog 的 `Design ledger impact` 中填写受影响的 D-xxx；没有设计语义变化时填写 `N/A: no design semantics changed`。
- 设计语义、边界、实现归属或验证方式发生变化时，必须同步本表的“最近变更”和验证证据，并按 `ae-sdd-update` 的 UG-28/UC-20 流程校验。

### 0.2 当前系统级设计台账

| ID | 设计 | 要解决的问题 | 核心决策 | 预期价值 | 验证证据/指标 | 权威入口 | 引入/最近变更/状态 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| D-001 | 端到端 Phase 编排 | AI 容易跳过设计、审核、测试或收尾节点，产物链断裂 | Phase 1/2/3、节点门禁、人工确认和出口产物组成有序状态机 | 降低流程遗漏和返工，提升交付可追溯性 | `gates check`、state phase、Story/Task/Coding/Review 链路 | §1；`source/SKILL.md` | 历史版本（精确版本待补）/v3.11.0/已实现 |
| D-002 | 智能路由 | 不同规模和入口的任务被错误地送入同一条流程，LLM 需要临时判断 | 来源、规模、已有产物、项目类型四维分类，映射到子链 | 减少错误入口和不必要上下文，缩短首次决策时间 | `ae-sdd classify`、`PHASE_FLOWS`、路由测试 | §2；`tools/lib/classify.py` | 历史版本（精确版本待补）/v3.11.0/已实现 |
| D-003 | Work Item state / Nested State | 多任务、跨 session、Story 归入和恢复时状态互相覆盖或丢失；逻辑 Story ID 无法指向项目正式文件名；恶意 Work Item token 可能逃逸隔离根 | 每个 Work Item 独立 state，支持 PRD/DR/Story 嵌套、UUID、revision、恢复和 StoryName/docPath 指针绑定；token 与 resolved path 双重 containment | 避免重复执行、跨任务污染和根外 state/sidecar 写入，让 LLM 可从 state 直接恢复并稳定定位正式 Story 正文 | state store、nested state、Story binding/state transition、path escape tests | §3；`tools/lib/state.py`、`paths.py`、`state_store.py` | v3.3~v3.10.1/v3.11.3/已实现 |
| D-004 | 多 Agent 编排 | 并行任务缺少角色边界、报告格式和故障升级规则 | 角色库、结构化派活卡、reviewer tier、activeAgents/agentReports | 降低重复劳动和自审偏差，提升并行可见性 | agent state 字段、G-09 session 独立性、编排 SKILL | §4；`agent-orchestration-skill.md` | v3.2.6/v3.11.0/部分软约束 |
| D-005 | G-XX 门禁体系 | 仅靠 LLM 自律无法稳定阻止越级、假测试和文档漂移；同一 Story/Work Item 在不同门禁可能被不同路径规则解析，RA-like 参考资料还可能误锁当前流程 | 软约束、硬门禁、扫描器三层防线，统一 GATE_REGISTRY；Plan 门禁按完整/微任务 profile 校验；G-02/G-14 共用 Story resolver；G-RA-1~6/FLOW 共用 state `raDocPath` 优先、latest formal fallback 的单一 RA resolver | 把高风险错误提前阻断，减少错误进入后续阶段的返工，同时避免微任务、正式 StoryName 和无关 RA-like 文档被错误规则误拦 | `gates check`、GATE_REGISTRY、完整/微任务 CodingPlan tests、G-02/G-14 StoryName tests、`TestGRAUnifiedSelection`、G-09/UC checks | §5；`tools/lib/gates.py` | v3.2~v3.9.1/v3.11.4/已实现 |
| D-006 | Review Batch 与增量验证 | 每次变更都全量重跑，成本高；历史证据容易被覆盖 | review batch、baseline、changedPaths、verification plan、evidence fingerprint | 缩短验证时间，减少无效重跑，同时保持证据可追溯 | `verification.plan`、evidence active/superseded、focused tests | §5 Review Batch；`verification_plan.py`、`evidence.py` | v3.10.1/v3.11.0/已实现 |
| D-007 | Typed operation + state lease | LLM 不知道调用什么、参数怎么写、哪些操作需要锁；并发写会覆盖 state，失败重试会重复推进 | `ops describe/next/execute`、JSON Schema、lease/fencing/revision/idempotency、`nextActions`、短事务锁 | 减少 LLM 理解成本、上下文读取、命令猜测、冲突和失败重试；把自然语言契约变成机器契约 | `ops describe --json`、UC-19、StateStore/operation/concurrency tests、CLI latency baseline（待持续补充） | §5 Review Batch 表、`operation-protocol.md`、`operations.py` | v3.11.0/v3.11.0/已实现 |
| D-008 | 项目资产七层索引 | LLM 每次从工程源码重新寻找约束、技术栈、模块和接口，容易遗漏上下文 | project assets 分层、生成/审计、倒排/BM25 查询、G-00 | 缩短上下文检索，提升首次回答和路由准确度 | `assets generate/check/query/stats`、G-00、asset index tests | §6；`project_assets.py`、`assets_index.py` | v3.2.4~v3.5.1/v3.11.0/已实现 |
| D-009 | Document Storage | Story/Work Item/constraints/assets 路径分散，正式 StoryName 与逻辑 ID 不同或 ID 重复时，LLM 和工具容易读错、猜错或要求改名 | intent-based resolve、Work Item-first、非 glob StoryName 精确绑定与元数据反校验、Story-category-only ID fallback、save/finalize | 减少路径猜测、跨类别假命中和文档错写，让项目正式命名可直接使用，同时保留受限旧 ID-only 兼容 | `doc resolve/save/finalize`、`state bind-story-doc`、G-DOC-STORAGE、Story resolver tests、life Story-004/006 验收 | §7；`document_storage.py`、`document-storage-skill.md` | v3.7.2/v3.11.3/已实现 |
| D-010 | 四层实例化与分发 | source、dist、用户安装、项目实例互相漂移，修复或共享 scanner 依赖无法传递 | Layer 1 source -> Layer 2 dist -> Layer 3 install -> Layer 4 init/override；独立运行时脚本及其共享 helper 必须显式进入 `runtime_scripts` 白名单 | 降低升级和环境差异造成的故障，保证 LLM 使用同一版本契约，并避免 dist scanner 因漏 helper 无法启动 | `build_dist.py`、`test_build_dist_packages_the_shared_scope_helper`、install/init、UC-15/runtime verify | §8；`build_dist.py`、`install.py`、`init.py` | v3.4.1/v3.11.4/已实现 |
| D-011 | Harness 适配层 | 不同 Agent 运行时需要不同入口，手工转译易造成版本和模板漂移 | adapter lock、source hash、tree hash、生成/回滚/备份轮转 | 减少接入和升级的人工操作，避免安装产物陈旧 | `build_harness.py`、adapter lock tests、iteration-check | §9；`.harness/`、`build_harness.py` | v3.5.6/v3.11.0/已实现 |
| D-012 | Memory 生命周期 | 长会话上下文过大，compact 后关键业务事实丢失或 scope 混用 | 实体树、boot/context/pending compact、manifest hash、生命周期 CLI | 缩短默认上下文，提升跨 session 恢复和业务事实复用 | `memory create/read/update/search/summarize`、memory tests | §10；`memory_store.py`、`memory_compiler.py` | v3.8.2~v3.10.3/v3.11.0/已实现 |
| D-013 | Plan-First 编排 | LLM 直接编码会漏需求、测试、约束和回滚路径；统一大型 Plan 模板又会误拦微重构 | 编码前始终有 CodingPlan 和用户确认；完整计划走 14 门禁，微任务走范围/实现顺序/风险回滚/验证四维轻量门禁 | 把实现前的未知风险显式化，减少中途返工，并让门禁成本与任务规模匹配 | G-07/G-08、G-CODEPLAN-SRC、G-14、Work Item scope 与 micro/full profile tests | §11；`coding-process-skill.md`、`gates.py` | v3.7.3/v3.11.2/已实现 |
| D-014 | 真实性扫描与对齐审计 | 文档可以宣称有门禁/测试/命令，但代码实际没有或已失效；全仓 RA rglob 会把参考资料、模板、事件附件和 dist 副本当作当前需求 | test/coding/RA scanners + UC-08~13 alignment audit + iteration-check；RA scanner 共享 `ra_scan_scope.py`，默认 root 审计只枚举 formal RA，Work Item 门禁显式传 repeatable `--file` | 减少“文档看似完整但执行无效”的假闭环，避免 Python/JS/TS work item 被误判为无生产 scope，并消除无关 RA-like 文档造成的 false blocker | UC-08~13、G-09、G-CODE-1、`test_ra_scan_scope.py`、RA scans、文本代码 scope regression | §12；`alignment_audit.py`、`scripts/ra_scan_scope.py`、`scripts/*_scan.py` | v3.5.11~v3.6/v3.11.4/已实现 |
| D-015 | 统一 CLI 工具链 | 工具分散且输出不统一，LLM 需要记忆多个脚本和输出格式；存量 state 缺少正式 StoryName 的可审计迁移入口；批处理若不检查中间退出码会把前序失败覆盖成成功 | `ae-sdd` 统一入口、UTF-8 JSON/stdout/stderr、稳定 exit code；提供 `state new --story-name`、幂等 `state bind-story-doc` 和四个支持 repeatable `--file` 的 RA scanner 子命令；PowerShell 编排逐命令检查 `$LASTEXITCODE` | 减少命令搜索、编码差异、解析、迁移和批处理误判成本，便于 LLM/脚本安全组合调用 | `ae-sdd --help`、Story binding CLI tests、`test_invalid_ra_prerequisite_exits_one_without_writing_or_deleting_draft`、RA CLI forwarding、GBK regression、update-check | §13；`tools/bin/ae-sdd` | 历史版本（精确版本待补）/v3.11.4/已实现 |
| D-016 | State events 操作日志 | 只有当前 state 时无法解释谁在何时推进、恢复或覆盖 | append-only events、seq、txn/node 过滤、兼容旧 state | 提升审计和故障定位效率，减少 LLM 盲目重试 | events read/filter tests、state JSON | §14；`tools/lib/state.py` | v3.4.2/v3.11.0/已实现 |
| D-017 | 三层 Hook 拦截 | 仅在命令执行后发现越级已经太晚，LLM 可能先写错产物或源码 | UserPromptSubmit、PreToolUse、Stop/监控协同；turn-scoped activity token 只在显式 ae-sdd turn 激活 | 将错误拦截前移，减少非法写入和后续修复，同时避免普通 Story 文档检查误触发门禁 | hook tests、gate_intercept、stop_check、harness | §15；`.harness/`、`gate_intercept.py` | v3.4/v3.11.0/v3.11.4/已实现 |
| D-018 | 主流程监管器 | state、gate、memory、产物和 hook 各自工作，缺少统一运行态视图 | monitor/orchestrator 聚合 phase、activity、work item 和事件 | 降低 LLM/维护者判断当前流程位置的成本 | monitor state projection、runtime stats、监控测试 | §16；`ae-sdd-monitor` 相关设计 | v3.6/v3.11.0/已实现 |
| D-019 | 流程偏移检测与矫正 | AI 或人工可能跳步、回退或修改错误 Work Item，状态表面仍可继续 | 偏移规则、矫正计数、paused/人工升级、scope 复核 | 尽早发现流程漂移，避免错误积累到交付阶段 | iteration-check、state correction、flow violation scan | §17；`iteration_check.py`、`state.py` | v3.6/v3.11.0/部分 report-only |
| D-020 | SKILL 编译与 Runtime IR | 完整 SKILL 过长，默认加载浪费上下文；source/dist/runtime 容易不一致 | source slimming、fallback、compact boot/core/outline、manifest fingerprint | 缩短 LLM 默认上下文，保持按需加载和可验证分发 | `slim_source_skills.py`、`build_dist.py`、UC-15、runtime verify | §18；`compile_skill_runtime.py` | v3.8/v3.11.0/已实现 |
| D-021 | 自动化模式 | 逐个等待人工确认耗时，但直接自动推进又会失去独立审查 | 默认关闭、Tier 3 多 reviewer、reviewConsensus、G-AUTO-CONSENSUS | 在明确开关下减少等待，同时保留高风险审查 | automation CLI、G-AUTO-CONSENSUS、UC-16 | §19；`config.py`、`state.py`、`gates.py` | v3.8.0/v3.11.0/已实现 |
| D-022 | ae-sdd Monitor | 多工作区、active task、memory、runtime stats 分散在文件中，定位异常慢 | 只读 workspace 扫描、项目/任务双层视图、响应式刷新 | 降低人工监控和 LLM 状态查询成本，不改变权威 state | `apps/ae-sdd-monitor` tests、UG-22、只读边界 | §20；`source/docs/ae-sdd-monitor-design.md` | v3.7.0/v3.11.0/已实现 |
| D-023 | Sonar Issue 修复收尾 | CodeReview 发现的问题容易重复处理、越界修改或缺少 exactly-once 证据 | issue registry、TextEdit/provenance、compile/test/rescan 闭环 | 减少重复修复和错误补丁，提升 review 收尾效率 | `test_sonar_issue_fix_skill.py`、规则 registry、CodeReview evidence | §21；`sonar-issue-fix-skill.md`、`sonar-issue-fix-rules.md` | v3.11.0/v3.11.0/已实现 |
| D-024 | Design Ledger 治理 | 设计动机、价值假设和迭代影响容易再次分散或漏记，台账本身也可能失去维护 | §0 台账 + CHANGELOG `Design ledger impact` + UG-28/UC-20 fail-closed | 降低后续 LLM 重新理解和维护者追溯成本，让设计价值记录成为可检查资产 | `UC-20`、`update-check 20/20`、台账字段/章节/版本反例测试 | §0；`update_graph.py`、`ae-sdd-update`、CHANGELOG 模板 | v3.11.1/v3.11.1/已实现 |
| D-025 | 风险驱动的有界测试策略 | 全矩阵、逐字段边界、最少用例公式和证伪比例会制造低价值测试，且没有停止条件 | 先建有限风险登记，再按边界准入、行为等价类、最低充分层级和局部数量上限选择；停止后扩展必须走预算例外 | 减少无独立缺陷发现价值的测试执行与维护成本，同时保护显式契约和高影响风险 | `test_bounded_test_strategy.py`、TC-G11/TC-10、后续项目 TestCase 数量与缺陷发现率基线 | §22；`be-testcase-strategy.md`、TestCase generate/review/template、CodingModel | v3.11.5/v3.11.5/已实现，收益待补基线 |

### 0.3 台账的证据等级

| 等级 | 含义 |
| --- | --- |
| 已测 | 有可重复命令、测试统计或运行指标直接支持 |
| 已实现 | 代码、CLI 和结构检查已存在，但收益指标仍需长期采集 |
| 待补基线 | 设计问题和机制已确认，尚未有修改前/后的量化对照 |
| 部分软约束 | 主要依赖 SKILL/LLM 自律，缺少完整物理拦截 |

## 1. 端到端流程编排（Phase 1/2/3）

### 设计

将整个研发流程切分为三个强制有序的 Phase，每个 Phase 有明确入口门禁、执行节点、人工审核点和出口产物。AI 负责驱动节点推进，不能跳过任何节点；每个人工审核节点 AI 必须主动"讲故事"（在对话内直接呈现），不能只丢文档链接。

- Phase 1 设计阶段：需求分析(RA) → DR 生成 → Story 生成(①) → 前端视角审视(①bis) → Story Review(②) → 测试用例生成(③) → 业务逻辑汇总(③bis) → 人工审核点 1
- Phase 2 实现阶段：实现方案预确认(人工审核 1.5) → Task 生成 + 全局 Task Review(④) → CodingPlan 生成(④bis) → CodingPlan 评审(人工审核 2.5) → Coding(⑤)
- Phase 3 验证阶段：完成判定⑥（10 项条件）→ 全切面一致性核查(⑥bis) → CodeReview 报告(⑦) → 全链路对称性核查(⑦bis) → 人工审核点 4
- 共 5 个人工审核节点，内容必须输出在对话中，不能要求用户自行打开文件查看

**v3.5.2 流程收尾合规自检**：在两处流程结束点增加"自检合规 → 不合规就修复"环节，防止 AI 裸 ✅ 收尾。Story 级 ⑦ter（人工审核点 4 后）5 维度自检；PRD 级 §1.7（`prd-complete` 前）强制先跑 `prd-check-complete`。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| Phase/节点定义 | `source/skills/orchestration/ae-sdd-update-skill.md` 及各 phase SKILL 文件描述节点顺序 |
| 节点进度持久化 | `state.json.phase` 字段，见 §3 流程状态持久化 |
| Story 级 ⑦ter 自检 | AI 编排执行：`gates check` 全量 + `gates check --only G-DOC-STORAGE` + `state read` + 产出物核对，SKILL 文字描述，非独立 CLI |
| PRD 级 §1.7 强制顺序 | `ae-sdd state prd-check-complete --prd <ID>`（`tools/bin/ae-sdd` state 子命令）必须先于 `ae-sdd state prd-complete --prd <ID>` |
| 4 层 AND 校验 | `tools/lib/state.py:check_prd_4_layers()`（G-PRD-1~4） |
| 审核点用户确认 token | `ae-sdd state confirm --phase <审核点>`（防 AI 自填，`tools/bin/ae-sdd` state confirm 子命令） |

**颗粒度与边界**：流程节点粒度；Phase 1 完成确认、CodingPlan 审核等关键节点锁定后不允许重跑；不可跳过任何节点，微任务走快速通道也须经路由判定后才豁免；自检不替代人工审核，不得跳过自检直接收尾。🆕 2026-07-03(B1)：明确"快速通道"仅指 `/ae-sdd-quick` escape hatch（豁免 G-00/路由判定维度，详见 conventions §3.1），**不等于 scale=微 的子链豁免**；微链（scale=微）同样必须走 code-reviewed 节点出 CodeReview 报告（conventions §3.1 质量底线：Plan-first/CodeReview 报告/state.json 更新 ❌不豁免）。

---

## 2. 智能路由（4 类需求 + 4 维判定）

### 设计

统一入口层，对所有用户输入做需求分类后路由到对应 SKILL，两套路由机制并存互补：4 维判定（来源 × 规模 × 现有产物 × 项目类型）优先，分类不明时 fallback 到 4 类需求传统路径（套 Story 7 区模板判定规模）。

路由决策算法 7 步：工作区检查(0) → 自更新识别(1.5，短路到 update-skill) → 来源识别(1.6) → 规模识别(1.7) → G-RA 准入门禁(1.8) → 关键词匹配(2) → 加载执行(3-5)。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 分类算法核心 | `tools/lib/classify.py:classify()`（行235），综合标题信号/文件名信号/关键词匹配/规模推断 |
| 规模推断（4 维判定） | `classify.py:_infer_scale_from_lines()`（行131）/ `_infer_scale_from_project_context()`（行143） |
| CLI 入口 | `ae-sdd classify --text "..."`（`tools/bin/ae-sdd` classify 子命令，行2960） |
| 合法规模枚举 | `tools/lib/state.py:VALID_SCALES = ("大", "中", "小", "微")`（行88） |
| 路由到子链 | `tools/lib/state.py:PHASE_FLOWS` 字典按 scale 选链（见 §3 表格） |

**颗粒度与边界**：路由到节点级（requirement-analysis / dr-generate / story-generate / task-generate / coding），不进行节点内子步骤路由；路由决策权归 ae-sdd 流程与用户，不归 AI 临时直觉。

---

## 3. 流程状态持久化（state.json）

### 设计

每个新需求必须先创建一个独立状态机，再进入 phase 流转。WorkItem（PRD / BUG / OPT / Story 均可）的执行进度持久化到独立 `state.json`，支持跨 session 中断恢复与多任务并行执行。AI 重入时读对应 WorkItem state 跳过已完成步骤，避免重复执行。

**WorkItem 标识约定（2026-07-09 修正，🆕 v3.10.1 UUID 前缀）**：每个状态机必须归属于真实顶层工作项。CLI 入口为 `ae-sdd state new --id <ID> --entry-node <PRD|DR|STORY|TASK>`；物理目录名采用 R6 顶层名加随机 UUID 前缀（如 `{uuid}-PRD-001` / `{uuid}-DR-005` / `{uuid}-Story-006`），保证同业务名不撞目录；`stateMachineId` 同目录名（带 UUID 前缀），`stateMachineName` 存纯业务名（如 `PRD-001`）供按业务名查找匹配，`stateUuid` 存 UUID 冗余标识。`--work-item <ID|WORKITEM-KEY>` 定位 `.auto-engineering/{WORKITEM-KEY}/state.json`，`find_work_item_state_path` 支持后缀匹配（传业务名 `PRD-001` 可命中 `{uuid}-PRD-001` 目录）。项目级 `.ae-sdd/state.json` 不允许作为 active state、mirror 或 fallback；未能唯一定位 work-item 时必须拒绝并要求显式选择。

**v3.5.15 多子链状态机**：单条 PHASE_FLOW 拆为 4 条 PHASE_FLOWS（大/中/小/微），按 scale 路由，微链最短单步合法，修复微任务 next-step 误建议跑 RA 的问题。旧 state 无 scale 字段时按 completedSteps 反推，默认"大"（最保守）。

**v3.6 暂停态**：`paused` 作为一级 phase，任何 phase 可跳入，用于 Level 3 人工升级（见 §15 流程偏移检测与矫正）。

**🆕 v3.9.0/v3.9.13 嵌套状态模型（Nested State Model）**：取代 v3.8.x 前的项目级/扁平状态模型。一个顶层 work-item 的 state.json 内维护主流程所有子系列——`prdState` + `drStates{N个DR}` + `storyStates{Story索引}`；每个 `drStates[DR-ID]` 下再维护自己的 `storyStates{N个Story各自完整流程状态}`。按 `entryNode` 选填容器：

| entryNode | state 内含容器 | 适用场景 |
|---|---|---|
| PRD | prdState + drStates + storyStates | 全新 PRD，从 PRD 出发走完整流程，允许多个 DR 子系列 |
| DR | drState + storyStates | 已有 DR，从 DR 出发（DR 所属 PRD 的 state 不存在时） |
| STORY | storyStates | 已有 Story 草稿，从 Story 出发（上层 DR/PRD state 不存在时） |
| TASK/PLAN | （flat state，不嵌套） | Bug/微任务不改 Story（R4 保留微链） |

**R2 任意节点出发 + 递归向上归入**：PRD / DR / Story / Task / BUG 都可作为顶层任务入口，但如果当前节点有合法父级，必须继续向上遍历直到真实顶层。例：Story-007 → DR-005 → PRD-001 时，最终只创建/使用 `.auto-engineering/PRD-001/state.json`；DR 写入 `drStates["DR-005"]`，Story 写入 `drStates["DR-005"].storyStates["STORY-007"]`，并同步到顶层 `storyStates` 索引。只有 DR 没有合法 PRD 父级时才创建 DR 根 state；只有 Story 没有合法 DR/PRD 父级时才创建 Story 根 state。

**R5 改已管理 Story 重定位 + 重置子状态**：检测到改 Story 且该 Story 已属某 state 的 `storyStates` → `ae-sdd state relocate --story <ID>` 重定位到该 state + 只重置该 Story 子状态到 `story-generated`（兄弟 Story 不动，resetHistory 保留审计）。

**R6 顶层主体命名（🆕 v3.10.1 UUID 前缀）**：只以最顶层主体特征命名——`PRD-{特征}` / `DR-{特征}` / `Story-{特征}`（多 Story 合并如 `Story-003-004-005`）。由 `paths.build_state_machine_name()` 生成纯业务名；创建时 `paths.generate_state_uuid()` 生成随机 UUID 并拼为 `{uuid}-{业务名}` 作为目录名和 `stateMachineId`，`stateMachineName` 保留纯业务名。`paths.strip_uuid_prefix()` 用于从带前缀的标识中还原业务名。向后兼容：旧 state 目录无 UUID 前缀仍可读。

**Story 文档绑定（v3.11.3）**：`STORY-006-BE` 等 Story ID 始终是逻辑身份，不承担文件命名职责。正式文件名通过嵌套 `storyStates[storyId].storyName/docPath` 或扁平兼容字段 `storyName/storyDocPath` 绑定；`ae-sdd state new --story-name` 用于创建时绑定，`ae-sdd state bind-story-doc` 用于存量迁移。绑定只保存正文指针，不复制正文、不创建 ID-only alias。吸收到既有父 state 时 Story add + binding 是一次 lease/revision CAS；新 state 以内存完整对象经 StateStore exclusive-create 落盘；重复 bind 同一文件不增加 revision。

**R7 路由自动匹配/新建**：`/ae-sdd` 路由时 `classify.match_state()` 自动分析需求特征（提取 PRD/DR/Story ID + 判定 Bug/改 Story）→ 扫描现有嵌套 state → 命中则 relocate/absorb，未命中则 create_nested。匹配优先级：R4 微任务 → R5 Story 命中 → R2 DR 归入 PRD → R2 Story 归入 DR → R7 新建。

**v1 扁平 state 完全兼容**：旧 workitem（`stateModel` 缺省或 `"flat"`）保留可读，所有读取点通过 `state.is_nested_state()` 分流。旧 workitem 不迁移、不动。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 存储路径 | `.auto-engineering/{WORKITEM-KEY}/state.json`（🆕 v3.10.1 WORKITEM-KEY 带 UUID 前缀如 `{uuid}-PRD-001`）。`.ae-sdd/state.json` 禁止作为状态源、active mirror 或 fallback；hook/gate/CLI 均必须通过 work-item/session resolver 定位任务级 state |
| 4 条子链定义 | `tools/lib/state.py:PHASE_FLOWS`（行69-92），大链 14 phase / 中链 13 / 小链 12 / 微链 8（🆕 2026-07-03 B1：微链加回 code-reviewed，与"CodeReview 报告不豁免"对齐；含 TestCase 系列后已扩容，🆕 v3.7.x） |
| 向后兼容别名 | `PHASE_FLOW = PHASE_FLOWS["大"]`（行95，🟡 deprecated） |
| 合法 scale 枚举 | `VALID_SCALES = ("大", "中", "小", "微")`（行88） |
| phase 允许工具集 | `tools/lib/gate_intercept.py:PHASE_PERMIT`（行64） |
| 暂停/恢复 API | `state.py:pause_state()`（行663）/ `resume_state()`（行684） |
| 矫正计数 API | `state.py:increment_correction()`（行706）/ `get_correction_count()`（行725） |
| 多 Agent 状态字段 API | `state.py` 行472注释起：`activeAgents` 写入 + `agentReports` 归档（见 §4） |
| memory 生命周期强制校验 | `tools/lib/memory_gate.py:check_state_transition()`（行51），`state write --phase` 切相前调用 |
| 终态投影不变量 | `state.py:set_phase()` / `set_story_substate_phase()` 写 phase 时同步 `currentPhase/currentStep/completedSteps/pendingOutputs/codingRound`；`write_state()` 拒绝 `phase=completed` 但投影仍处于中间态的 state |
| PRD 4 层 AND 校验 | `state.py:check_prd_4_layers()` |
| CLI 入口 | `ae-sdd state new / read / write / next-step / confirm / prd-init / prd-check-complete / prd-complete / prd-archive`（`tools/bin/ae-sdd` state 子命令组） |
| Story 正文绑定 | `state.py:bind_story_document()` / `get_story_document_binding()`；CLI `state new --story-name` / `state bind-story-doc`；嵌套 state 写 `storyStates[storyId].storyName/docPath`，扁平 state 写 `storyName/storyDocPath` |
| 🆕 v3.9.0 嵌套 state schema | `state.py:init_nested_state()` / `reset_story_substate()` / `set_story_substate_phase()` / `get_active_phase()` / `get_active_story()` / `ENTRY_NODE_CONTAINERS` |
| 🆕 v3.9.0 嵌套 state 命名 | `paths.build_state_machine_name(top_node, features)`（R6 顶层命名） |
| 🆕 v3.9.0 向上归入查找 | `paths.find_nested_state_by_story_id/dr_id/prd_id`（R2 归入） |
| 🆕 v3.9.0 路由自动匹配 | `classify.match_state()` + `extract_requirement_features()`（R7） |
| 🆕 v3.9.0 entryNode 容器选择器 | `flow_enums.FlowNode.container_fields()` + `is_nested_entry()` |
| 🆕 v3.9.0 R5 relocate CLI | `ae-sdd state relocate --story <ID>`（重定位+重置子状态） |
| 🆕 v3.9.0 嵌套 state write | `ae-sdd state write --sub-story <ID> --phase <phase>` / `--add-story <ID>` |

**颗粒度与边界**：step 级（如 `step-4-coding-r2`）；不可倒退的关键门禁步骤标记为 locked；🆕 v3.9.0 嵌套模型下多个 Story 的子状态在同一个 state 的 `storyStates{}` 内各自独立流转、互不干扰（R5 重置只动目标 Story）；state 仅记录进度，不存储业务产物内容。`phase/history` 是生命周期状态，`currentPhase/currentStep/completedSteps/pendingOutputs/codingRound` 是可恢复执行用的工作流投影；终态写入必须同步两套字段，不能出现 `phase=completed` 但 `currentStep` 仍等待人工确认、`pendingOutputs` 未清空或 `codingRound` 仍为 r0 的组合。

---

## 4. 多 Agent 编排（角色库 + 派活协议）

### 设计

当单节点内存在无强依赖的子任务时，root agent 按角色库拆分派活；Review 节点支持多 reviewer 交叉审，避免自圆其说。8 个预定义角色：`story-writer / story-reviewer / testcase-writer / task-writer / coder / code-reviewer / test-verifier（强制）`。

派活协议要求结构化 YAML 任务分配卡（必填 `agent_role / story_id / input / output / standards / context / deadline / report_back`）；故障补救 4 级 SOP（重试→重新分配→降级→升级用户）；多 reviewer 框架按 Tier 判定单/双/三审。

**⚠️ 设计-实现落差**：这一整套角色库、派活协议卡格式、故障补救 SOP、多 reviewer 框架，目前**只存在于 SKILL 文本描述**（`source/skills/cross-cutting/agent-orchestration-skill.md`），是对 AI 行为的软约束，代码层没有强制执行或校验派活卡格式是否合规。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 角色库/派活协议/故障 SOP/多 reviewer 框架 | `source/skills/cross-cutting/agent-orchestration-skill.md`（纯文字描述，AI 自律执行，**无代码校验**） |
| 多 Agent 状态可见性（唯一落地的代码支撑） | `tools/lib/state.py`：`activeAgents` 字段（启动 sub-agent 时写入，行477起）/ `agentReports` 字段（完成后移入，行494起） |
| test-verifier 独立性约束 | ⑥.10 测试真实性硬门禁 G-09 要求报告带独立 `session_id`（`tools/lib/gates.py` G-09 check 函数），是唯一有 CLI 侧校验的角色约束 |

**颗粒度与边界**：节点内子任务级并行；降级为逻辑多视角时必须在报告头部标注 `reviewerMode: "logical-multi-perspective"`；root agent 保留最终冲突裁决权；除 activeAgents/agentReports 状态记录和 test-verifier session_id 校验外，其余编排规则完全依赖 AI 自律，无物理拦截。

---

## 5. 门禁体系（G-XX 三种强度）

### 设计

三种强度形成纵深防御：软约束依赖 LLM 自律，硬门禁通过 CLI 阻断，扫描器兜底覆盖硬门禁无法检测的场景。

- **软约束**：SKILL 文本说明，LLM 自律执行
- **硬门禁**：`GATE_REGISTRY` 注册，CLI `ae-sdd gates check` 阻断，不通过无法继续
- **检查器**：`*_scan.py` 静态扫描报告，兜底补硬门禁覆盖不到的场景

G-00 项目资产门卫每次 SKILL 启动前验证资产存在；G-RA 系列在需求分析阶段把关；G-CODE-1 在 Coding 完成/CodeReview 前扫描反模式；中段门禁（G-14/G-CODEPLAN-SRC/G-DOC-STORAGE）补"两头强中间空"；入口关卡三道闸（entry token / 产物落地凭证 / 代码改动准入）管住流程入口。

CodingPlan 门禁按 state `scale` 选择 profile。大/中/小任务保持 7 章节、14 关键词、Story/AC 对齐和源码核对；微任务从 Work Item 分桶读取 Plan，只要求变更范围、实现顺序、风险/回滚、验证四维完整且无未闭环项。Standalone 微任务没有 Story 时，G-14 返回带 `alignmentMode=standalone-micro` 的可审计 N/A；一旦存在父 Story，仍执行严格 Story/AC 对齐。

G-02 与 G-14 通过 `document_storage.resolve_story_document()` 共享 Story 正文解析。已绑定 `docPath` 优先，随后是精确非 glob StoryName，只有没有 StoryName 时才兼容 Story 类别的 `{STORY-ID}.md`；Task/Coding/Test/CR 同名文件不参与。正式文件必须由正文 `Story ID` 元数据反校验。多候选、元数据缺失/漂移和非法 basename 全部 fail closed，不做模糊猜测。

G-RA-1~6 与 G-RA-FLOW-VIOLATION 通过 `_resolve_selected_ra()` 共享当前 Work Item 的 RA 正文。合法 `state.raDocPath` 或 `storyStates[activeStory].raDocPath` 优先；没有 binding 时，只在 formal RA candidates 中沿用统一的 latest version/mtime fallback。Work Item 门禁把解析出的单一文件作为 `--file` 传给 scanner，`--root` 仅用于相对路径和 containment；因此 `references/`、templates、CHANGELOG、`dist/`、GeneratePlan/Impact/ReverseIssues 等 RA-like 文档既不会锁住当前流程，也不能替代 selected RA 自身的真实性、深度、流程或实现完整性检查。根目录全量审计仍保留，但使用同一 formal candidate 分类器，不再使用各 scanner 独立的宽泛 `rglob`。

🆕 v3.9.1 上下文加载准入门禁（G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX）补齐 DR/Story/TestCase/Task 四组的"第零步准入检查"——此前这四组只有 prose 清单（dr-review/task-generate 还官方自认 report-only），AI 可不读 PRD/DR/项目资产/约束就过门禁切相。采用**注册表模式**：一个 `_check_context_loaded` 函数 + `CONTEXT_GATE_REGISTRY` 注册表服务 4 个门禁，流程一致用单函数封装，上下文差异（DR 查 RA+PRD，Story 查 DR+PRD，TestCase 查 Story，Task 查 Story+TestCase）走注册表 `required` 字段；读文件统一走 `document-storage-skill` 的 `get_constraints/get_assets` API。微链 G-TASK-CTX 用 `required_micro` 豁免 Story/TestCase。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 门禁注册表 | `tools/lib/gates.py:GATE_REGISTRY`（list，**实际 34 个**：G-00~G-14 + G-09B + G-CODEPLAN-SRC + G-DOC-STORAGE + G-DOC-CONSISTENCY + G-PATH + G-RA-1~6 + G-RA-FLOW-VIOLATION + G-CODE-1 + G-REVIEW-LOOP + G-AUTO-CONSENSUS + 🆕 v3.9.1 G-DR-CTX/G-STORY-CTX/G-TESTCASE-CTX/G-TASK-CTX） |
| CLI 统一扫描入口 | `ae-sdd gates check`（`tools/bin/ae-sdd` 行2688，帮助文本自带门禁清单） |
| 单个门禁定向检查 | `ae-sdd gates check --only <gate_id>` |
| G-00 资产门卫 | `gates.py` G-00 check 函数；不通过时可运行 `ae-sdd assets generate --project <key>` 生成/修复 baseline 资产，再用 `ae-sdd assets check` 校验 |
| G-RA-1~4 需求分析门卫 | `gates.py`，调 `scripts/ra_authenticity_scan.py`（G-RA-4，`_locate_ra_authenticity_scanner` 行1116） |
| G-RA-5 机械派生深度 | `gates.py` 行1292起，调 `scripts/ra_depth_scan.py` |
| G-RA-6 实现视角完整性 | `gates.py` 行1378起，调 `scripts/ra_implementation_scan.py`（I1~I7） |
| G-RA-FLOW-VIOLATION | `gates.py` 行1208起，调 `scripts/flow_violation_scan.py`（R1~R3规则） |
| G-RA authoritative scope | `gates.py:_resolve_selected_ra` + `scripts/ra_scan_scope.py`；G-RA-1~6/FLOW details 统一报告 `selected_file/selection_source/scope_mode`，四 scanner 接收 repeatable `--file` |
| G-CODE-1 Coding 真实性 | `gates.py` 行697起，调 `scripts/coding_authenticity_scan.py`（AP-1~AP-6反模式） |
| G-09 测试真实性 | `gates.py` 调 `scripts/test_authenticity_scan.py`（8类禁止手段）；可信 work-item scope 优先取 `VerificationPlan.changedPaths`，兼容 state changed paths，无 scope 时全仓严格扫描 |
| G-13 全链路对称性 | `entryNode=STORY` 且 `scale=中` 时仅豁免 DR 依赖并显式返回 exemption；其他入口仍要求 DR，Story/Task/CodingReport/CodeReview 下游链按阶段保持严格 |
| G-07 / G-08 / G-14 / G-CODEPLAN-SRC | `gates.py:_resolve_codingplan_doc` 统一按 Work Item-first、唯一 Story fallback 定位；完整 Plan 保持 14 门禁，微任务使用四维轻量 profile；无类骨架时 G-CODEPLAN-SRC 按既有规则跳过 |
| G-02 / G-14 Story 正文 | `gates.py:_resolve_story_doc` 统一调用 `document_storage.resolve_story_document()`；绑定路径/StoryName 校验失败时返回稳定错误码和 `state bind-story-doc` 恢复动作 |
| G-DOC-STORAGE / G-DOC-CONSISTENCY / G-PATH | `gates.py` 对应 check 函数；G-PATH 仅豁免 canonical document-storage source entry 及其 source/runtime full fallback，其他同名或 `SKILL.full.md` 文件仍受扫描 |
| 🆕 v3.9.1 G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX | `gates.py` 注册表 `CONTEXT_GATE_REGISTRY` + 统一 `_check_context_loaded` 实现 + 4 个薄封装；`gate_intercept.py:PHASE_ENTRY_GATES` 大/中/小/微链各 phase 入口挂载；复用 `get_constraints/get_assets` + `_iter_ra_files/_find_prd_files/paths.find_doc` |
| G-REVIEW-LOOP | `gates.py` + `tools/lib/review_loop.py`，`ae-sdd review-loop` 子命令 |
| 入口关卡1（entry token） | `ae-sdd enter <projectKey> [--story <ID>]`（`tools/bin/ae-sdd`），UserPromptSubmit hook 检测 `/ae-sdd` 触发词强提醒 |
| 入口关卡2（产物落地凭证） | `tools/lib/gate_intercept.py` PreToolUse 拦截，产物路径 + entry token + 产物-Phase 映射校验 |
| 入口关卡3（代码改动准入） | `gate_intercept.py:PHASE_PERMIT`，非 coding phase 或无审核点 2.5 确认 → 禁写 src/ |
| 设计-实现对齐反查 | `tools/lib/alignment_audit.py`（UC-08~UC-13，6个 check_* 函数），CLI `ae-sdd update-check`；见 §9 真实性扫描后段 |

**G-PATH 项目侧输入边界（v3.10.2）**：项目侧扫描只读取声明为记忆/约束输入的 `.ae-sdd/memory/**/*.md`、顶层 `AGENTS.md`/`CLAUDE.md`/`MEMORY.md` 和 `.harness/memory/**/*.md`。`.ae-sdd/drafts/**/*.md` 是 Review/生成过程产物，不是 canonical 正文或项目记忆，不进入 G-PATH；其流程真实性仍由对应 Review/上下文门禁负责。`current_story` 不用于静默过滤路径，避免通过伪造 Story ID 绕过项目级记忆扫描。

**颗粒度与边界**：G-00 每次 SKILL 调用前必跑；G-RA 豁免场景（微任务、BUG/配置类、重入已完成步骤）；改门禁强度必须改 `gates.py`，不能只改 SKILL.md 文字；`ae-sdd update-check` 自动检测文档-实现一致性，防文档撒谎复发。

### Review Batch v2 与增量验证

Review 的质量对象是带 `inputFingerprint` / `rulesetFingerprint` 的 `reviewSession`，不是模糊的 round 计数器。每次尝试落入 `VALID_CLEAN`、`VALID_FINDINGS`、`INVALID_INFRA`、`INVALID_PROTOCOL`、`INVALID_INPUT_DRIFT` 或 `CANCELLED`；平台失败不增加 clean streak，输入漂移必须新建 session。Tier 1/2 首批 clean 默认一次，Tier 3 在 P0/P1 修复后需要两个连续 clean batch；attempt、valid batch、remediation 和 wall-clock 任一预算耗尽进入 `STALLED`，不得当作通过。

G-CODE-1 在可信 `VerificationPlan.changedPaths` 与 G-09 evidence/hash 链存在时，仅检查当前 work-item 的生产代码；evidence 必须绑定 Story、scope、command、toolchain 与 report，artifact 只能是项目内相对路径。production scope 与 scanner 共同识别 Java/Kotlin/XML/YAML/properties 及 `.py/.js/.ts` 文本代码，并共同排除生成目录、`.venv/venv/.tox/site-packages`、`__tests__` 及 Python/JS/TS 常规测试命名。scanner 必须用安全、唯一的 `scannedPaths` 证明完整覆盖 production scope，并保证 root、exit/status、finding severity/path、顶层计数与 `reportStats` 自洽；任一缺失、越界、未知 schema 或计数漂移均 fail closed。self-hosting 例外只限 scanner 自身 AST `LINE_RULES` 与三个 metadata 常量赋值行；真实 pom metadata 由 XML 解析验证，普通业务文件使用同 URI 仍是 blocker。scope 内 blocker（含触及历史债）阻断，scope 外历史债不阻断；scope 缺失或为空时保持全仓严格扫描，测试/文档-only scope 阻断。scoped 路径不读取或创建 baseline；全仓模式的显式 baseline 仍必须经用户批准并校验完整性。

验证动作先生成 `VerificationPlan`，再按生产代码、测试代码、配置、文档分类决定最小验证集。计划分别暴露 `planFingerprint` 与 `evidenceInputFingerprint`，Evidence 命令只能使用后者。G-09 只接受 plan 指纹匹配、路径未越界且真实存在的 work-item scope；scope 内 blocker（含触及的历史债）阻断，scope 外历史债不阻断，无可信 scope 则全仓扫描。成功证据写入 `.auto-engineering/<story>/evidence/manifest.json`；record 先复制内容寻址 immutable snapshot，同 logical key 的旧 active entry 标为 superseded，finalize/gate 只校验 active snapshot。复用必须同时满足 manifest 内容 hash、Story、input/command/toolchain fingerprint、退出码、freshness window 与 snapshot hash；evidence 不能定义 scope 或充当 waiver。implementation/documentation/review 三类 fingerprint 分离，文档或审查措辞变化不得使 Maven 证据失效。文档使用单一 canonical 正文，Work Item scope 优先于 Story 关系；旧 Story 路径只允许唯一候选 fallback，多个候选必须显式报 `SCOPE_AMBIGUOUS`，不按 mtime 猜测。

| 设计点 | 实现 |
| --- | --- |
| Review Batch 状态机 | `tools/lib/review_batch.py` + `tools/lib/review_loop.py`；旧 `reviewLoop` 字段仅作兼容投影 |
| 增量 baseline | `tools/lib/baseline.py` + `ae-sdd baseline inspect/create/diff` + G-CODE-1 delta 分支 |
| 变更感知验证 | `tools/lib/verification_plan.py` + typed `verification.plan`；`evidenceInputFingerprint` 绑定 Story/work-item/changedPaths |
| 证据复用 | `tools/lib/evidence.py` + typed `evidence.record/finalize`；active/superseded + immutable snapshot |
| canonical 文档 | `tools/lib/document_storage.py` Work Item-first resolver + unique legacy Story fallback |
| LLM 操作协议 | `tools/lib/operations.py` registry + `source/standards/operation-protocol.md`；维护者变更入口由 `ae-sdd-update` 指向协议 §9，并由 UG-27/UC-19 约束 |

---

## 6. 项目资产体系（7 层索引）

### 设计

每个接入项目维护一份标准化资产文件，是所有 SKILL 的上下文基础，通过 ES 倒排索引支持按需读取。AI 读资产而非扫描全仓库，保证上下文质量与读取效率。

资产生成分两层：`ae-sdd init` 首次运行会自动生成可通过 G-00 的 baseline 资产；`ae-sdd assets generate/check` 提供可执行的生成/修复/校验入口。深度业务资产仍由 `project-assets-update-skill` 引导 AI 跑探查 SOP（读 CLAUDE.md/AGENTS.md → 扫描工程结构 → 抽典型类 → 识别分层/命名/约束）后增量完善；`assets update/audit` 仍未实现。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 资产文件路径 | `source/assets/{projectKey}/{projectKey}.assets.md`（7 层索引 §A-§G） |
| 生成/增量更新/审计引导 | `tools/lib/project_assets.py` + `ae-sdd assets generate/check` 负责 baseline 生成/校验；`source/skills/cross-cutting/project-assets-update-skill.md` §3/§4/§5 负责深度业务资产更新/审计引导 |
| ES 倒排索引 + BM25 评分 | `tools/lib/assets_index.py`（`AssetsIndex` 类：outline/query/section/stats 核心逻辑） |
| CLI 入口 | `ae-sdd assets generate / check / read / outline / section / query / stats`（`tools/bin/ae-sdd` assets 子命令组） |
| G-00 门卫自动检查 | `tools/lib/gates.py` G-00 check 函数，7 层索引缺任一层即阻断；距 lastAuditedAt > 30 天触发警告 |

**颗粒度与边界**：项目级隔离（按 projectKey）；7 层索引 §A-§G 必须齐备；RA SKILL 强制通过 `ae-sdd assets read` 接口读取，禁止直接读文件路径。

---

## 7. 文档存取层（document-storage）

### 设计

统一文档落地/读取/归档的横切能力，五维定位模型 + WorkItem 隔离键（projectKey × workItemId × intent × storyId? × 版本号 × ChangeLog）取代散点式手写路径拼接。所有 SKILL 读写 ae-sdd 生成的文档（DR/Story/TestCase/Task/CodingPlan 等）必须通过本层 API，禁止裸拼路径或裸调 `ae-sdd assets read`。Story 的逻辑 ID 与正式文件 basename 分离：项目可保留 `cs-ai-story-006-门店推荐对接与列表接口-BE.md` 等原生命名，由 state 显式绑定。

核心原则：intent 驱动定位（同一 API 按 intent 参数分流到不同文档类型的路径规则），Task/Coding/Test/CR 以 workItemId 作为分桶键，版本号与 ChangeLog 策略集中管理，避免每个 SKILL 各自实现一套路径逻辑导致漂移。StoryName 解析只接受 bound path、精确 basename、无 StoryName 时的 ID-only 兼容路径三层优先级；禁止 fuzzy ID 扫描。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 动态定位 API 契约 | `source/skills/cross-cutting/document-storage-skill.md` §4（§4.10 intent 枚举表 34个intent） |
| 路径解析 | `tools/lib/document_storage.py:resolve_path()`（行149） |
| 原生 StoryName 解析 | `document_storage.py:resolve_story_document()`；返回 path/source/candidates/rejected，正式文件校验 `Story ID` 元数据 |
| 文档保存（带版本号+ChangeLog） | `document_storage.py:save_doc()`（行366） |
| 文档定稿 | `document_storage.py:finalize_doc()`（行444） |
| 项目约束读取 | `document_storage.py:get_constraints()`（行119） |
| 项目资产列表读取 | `document_storage.py:get_assets()`（行140） |
| Git 路径/服务根路径 | `document_storage.py:get_git_path()`（行99）/ `get_service_root()`（行108） |
| 版本号推断 | `document_storage.py:get_latest_version()`（行259）/ `_normalize_version()`（行239） |
| ChangeLog 读取 | `document_storage.py:get_changelog()`（行286） |
| RA 前置条件校验 | `document_storage.py:check_ra_prerequisites()`（行320） |
| 存量文档迁移 | `document_storage.py:migrate_old_docs()`（行615） |
| CLI 入口 | `ae-sdd doc save / resolve / finalize`（`tools/bin/ae-sdd` 行2722起 doc 子命令组） |

**已补齐**：`get_thinking_engine(projectKey)` 已在 `tools/lib/document_storage.py` 实现，并收录到 `document-storage-skill.md` §4.2。编码流程引用该 API 时会优先读取项目/文档工作区覆盖版本，找不到时回退到 ae-sdd 自带 `standards/thinking/be-coding-thinking-engine.md`，返回 `path/source/content/sha256`。

**颗粒度与边界**：所有 ae-sdd 生成文档的读写必须走本层 API，不允许 SKILL 各自维护路径拼接逻辑；version/ChangeLog 策略变更须同步 §4.10 intent 枚举表的"实现状态"列（✅已实现/📝待实现）。

---

## 8. 实例化体系（4 层架构）

### 设计

母版到项目落地的 4 层分发体系，保证 SSOT 的同时支持项目级 override。Layer 1 母版是唯一编辑点，经脚本构建为分发包，再由安装脚本装入用户环境；接入项目通过 Layer 4 实例做 override，不修改母版。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| Layer 1 母版（SSOT） | `source/`，开发者唯一编辑点，git 跟踪 |
| Layer 2 分发包构建 | `scripts/build_dist.py` → `dist/ae-sdd/`（注入 VERSION + plugin.json，剥离 CHANGELOG/docs），git ignored |
| Layer 3 用户安装 | `scripts/install.py` → `~/.claude/skills/ae-sdd/`，Claude Code 实际加载 |
| Layer 4 项目实例创建 | `ae-sdd init <dir> <key>`（`tools/bin/ae-sdd` init 子命令），生成 `config.yaml/state.json/assets//overrides/` |
| Override 解析 | 项目 `overrides/` 优先于母版 defaults，由各 SKILL 读取时的路径解析规则实现 |
| 版本号同步 | `ae-sdd bump <ver>`（`tools/bin/ae-sdd` 行3056），同步 SKILL.md / paths.py / README.md 三处 |
| 一步到位开发流 | `scripts/dev-sync.sh` → `scripts/dev_sync.py`（build + install），跑前必须 `ae-sdd update-check` 全绿 |

**颗粒度与边界**：Layer 2/3/4 不手工改；fork（完整复制）是显式 opt-in；override 优先级：项目 overrides/ > 母版 defaults。

---

## 9. Harness 适配层

### 设计

将 ae-sdd SKILL.md 自动转译为 Mavis harness 格式的 agent.md，使 ae-sdd 能作为 Mavis 团队级 agent 被编排。不需要手工维护两套定义，转译脚本从 SKILL.md + HARNESS.md 生成 agent.md，母版升级后重跑即可同步。

**⚠️ 文档滞后修正**：原文档写转译脚本是 `convert-ae-sdd-to-harness.ps1`——该文件已不存在。实际实现是 `scripts/build_harness.py`（Python 重写版，脚本头部注释明确写"由 convert-ae-sdd-to-harness.ps1 迁移而来"），逐功能对齐原 PS1 版本（版本号 fallback / frontmatter 解析 / 多维幂等锁 / 模板渲染 / mount 失败回滚等）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 转译脚本 | `scripts/build_harness.py`（非 `.ps1`，PS1→Python 迁移已完成） |
| 产物路径 | `.harness/agent.md` + `.harness/README.md` |
| 幂等锁 | `.harness/.adapter.lock`（JSON：`adapter_version/ae_sdd_version/source_input_sha256/source_commit/templateHash/converted_at`），`build_harness.py:read_adapter_lock()` |
| 版本号三级 fallback | `build_harness.py:get_ae_sdd_version()`（行88，SKILL.md frontmatter → commit msg vX.Y.Z → git short hash） |
| tree-hash amend 检测 | `build_harness.py:get_tree_hash()`（行147，🆕 v3.5.6，区分 amend 和真实内容变更） |
| SKILL frontmatter 解析 | `build_harness.py:parse_skill_frontmatter()`（行168） |
| 模板渲染 | `build_harness.py:render_template()`（行207，`{{VAR}}` 占位符替换） |
| mavis CLI 探测与 mount | `build_harness.py:find_mavis_cmd()`（行233）/ `run_mavis()`（行252），mount 失败自动回滚产物（行488起） |
| 备份轮转 | `build_harness.py:cleanup_old_bak()`（行68，保留最近3个 `.bak.<ts>`） |
| CLI 用法 | `python scripts/build_harness.py [--dry-run/--force/--unmount/--clean/--no-mount]` |

**颗粒度与边界**：禁止手工编辑 agent.md；母版（source/SKILL.md / source/HARNESS.md / harness 模板）升级后必须重跑 `build_harness.py` 重新生成；harness 格式由 Mavis 规范决定，转译脚本负责映射；`.adapter.lock` 多维比对（source_input_sha256 + ae_sdd_version + adapter_version + templateHash）任一漂移触发重转，`source_commit` 只作诊断，不参与幂等判断，避免提交生成物后继续漂移。

---

## 10. 记忆层（🆕 v3.10.3 业务实体树 + 编译文档容器）

### 设计

memory 存储编译后的 compact 文档，按业务实体树平级分层（prd/dr/story/testcase/coding/common）。子流程Agent 首次进入时主动读源上下文（从 document-storage）-> 编译成 compact -> 写 memory；后续读上下文 = 读 memory；子流程结束删自己的 memory，common 保留。

**v3.10.3 核心变化**：废弃 5 层原文索引 + enter/exit 生命周期门禁，改为业务实体树 + 编译文档容器。memory 不再是"compact context index"（原文短索引），而是"编译后的工作上下文"（高密度 compact.md 文档）。

common 层只存项目级可复用约束（BigDecimal/幂等/禁大事务/架构规范），必须轻（`COMMON_MAX_CHARS = 2048` 字符硬限制），跨子流程保留。严禁存任何特定 PRD/DR/Story 的细节。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 业务实体树存储 | `tools/lib/memory_store.py`：`memory/{entity_type}/{entity_id}/` 目录，每实体含 boot/context/pending 3 个 compact.md + manifest.json |
| 编译器 | `tools/lib/memory_compiler.py:compile_source_to_memory()`：读源上下文 -> 编译 3 个 compact slice + manifest |
| common 提取 | `memory_compiler.py:extract_common()`：从源上下文提取项目级可复用约束，自动去重，大小限 2048 字符 |
| 生命周期 API | `memory_store.py:create_memory()`（创建=编译）/ `read_memory()`（读 compact）/ `update_memory()`（增量更新 slice）/ `clean_memory()`（删单实体）/ `clean_all_memory()`（删所有，保留 common） |
| compact snapshot | `memory_store.py:pre_compact_snapshot()` / `post_compact_reload()`（compact 前/后上下文保存与重载） |
| state->entity 映射 | `memory_store.py:STATE_PHASE_TO_ENTITY_TYPE` + `entity_type_for_state_phase()` |
| 存储格式 | compact.md（Markdown 表格/列表）+ manifest.json（source hash + slice hash + fingerprint） |
| CLI 入口 | `ae-sdd memory create/read/update/clean/clean-all/common/search/summarize` |
| memory_gate（废弃） | `tools/lib/memory_gate.py` 改为 passthrough（check_state_transition 永远 pass），批 3 删除 |

**颗粒度与边界**：5 大子流程（RA/DR/Story/TestCase/Coding）各有独立 memory 实体；子流程结束删自己的 memory，common 保留；从0重建 = clean-all（保留 common）；回归流程 = 先读无则建（不 clean-all）。UserPromptSubmit 从对应实体的 memory 读 compact 文档注入（boot+context+pending 全文 + common 约束）。

---

## 11. Plan-First 编排（完整 14 门禁 + 微任务四维门禁）

### 设计

编码前必须先生成并经用户确认 CodingPlan，将变更范围、实现顺序、风险/回滚和验证点锁定。CodingPlan 是 Coding 阶段的唯一依据，AI 不得自行调整 Plan 内容。完整任务必须通过 14 条门禁；微任务不得伪造大型 Story 章节，改走四维轻量门禁，但 Plan-first 和用户确认仍不可跳过。

CodingModel 11 维决策（嵌入每个 Task 文档）：并发控制/幂等策略/事务边界/缓存策略/错误码/异常处理/状态机实现/外部依赖/数据模型/可观测性/复用能力。实现方案决策基线（最高优先级 4 步强制）：① 现有能力复用扫描 → ② 业内成熟方案参考 → ③ 五维代码质量评估 → ④ 核心能力归属唯一。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| CodingPlan 生成时机 | Phase 2 ④bis，由 task-writer 汇总，见 `source/skills/phase2-coding/coding-process-skill.md` |
| CodingPlan 定位 | `tools/lib/gates.py:_resolve_codingplan_doc` 复用 document-storage Work Item-first、唯一 Story fallback 语义 |
| 完整任务 14 门禁 | `tools/lib/gates.py:CODINGPLAN_14GATES_KEYWORDS`；大/中/小任务保持原严格检查 |
| 微任务四维门禁 | `tools/lib/gates.py:MICRO_CODINGPLAN_DIMENSIONS`；标题支持中英文 alias，检查范围/实现顺序/风险回滚/验证和未闭环标记 |
| G-14 | 有 Story 时严格核对 Story/AC/Proposal；standalone 微任务无 Story 时返回可审计 N/A |
| G-CODEPLAN-SRC | 有类骨架时校验源码标记和真实路径；微任务无类骨架时显式跳过 |
| 5 上下文加载（🆕 v3.7.3） | `coding-process-skill.md`：项目约束/技术约束/Story/Task/TestCase，统一走 `document-storage-skill` API 读取（见 §7） |
| 人工审核点 2.5 | AI 主动 walkthrough（SKILL 文字流程，非 CLI），逐条等用户确认 |

**颗粒度与边界**：CodingPlan 在编码前必须生成并经用户确认，不可跳过；编码过程中偏离 Plan 需用户实时确认；CodingPlan 变更触发 storyVersion 累加。

---

## 12. 真实性扫描（静态扫描器 + 设计-实现对齐验证器）

### 设计

防止 LLM 伪造测试通过、伪造需求分析内容、伪造设计-实现对齐的静态扫描器，作为不可绕过的硬门禁运行时依赖。输出统一 JSON 契约，BLOCKER=0 才算通过。

**⚠️ 文档滞后修正**：原文档标题写"3 个静态扫描器"，实际共 6 个扫描器（测试/Coding/RA真实性 3个 + RA流程违规/RA机械派生深度/RA实现视角完整性 3个），外加 2 个对齐验证工具（AA全维对齐验证器 + IC迭代检查器）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 测试真实性扫描（⑥.10/G-09硬门禁） | `scripts/test_authenticity_scan.py`：8类禁止手段 + Surefire XML 解析 + AC覆盖率100%验证；由 test-verifier sub-agent 独立执行 |
| Coding 真实性扫描（G-CODE-1） | `scripts/coding_authenticity_scan.py`：AP-1~AP-6反模式库；`ae-sdd gate coding-required` 自动触发 |
| RA 真实性扫描（G-RA-4） | `scripts/ra_authenticity_scan.py`：8类禁止规则（vague-ellipsis/no-evidence/fabricated-field等） |
| RA 流程违规审计（G-RA-FLOW-VIOLATION） | `scripts/flow_violation_scan.py`：R1~R3规则（12维决策记录/8维度挖掘/缺口管理） |
| RA 机械派生深度（G-RA-5） | `scripts/ra_depth_scan.py`：验证每条规则 R 机械追问 6 问 → 衍生 R′ |
| RA 实现视角完整性（G-RA-6） | `scripts/ra_implementation_scan.py`：I1~I7 检查 |
| RA 扫描作用域 | `scripts/ra_scan_scope.py`：formal RA candidate 分类、explicit file containment、稳定排序、excluded reason 和结构化错误；四个 RA scanner 共享 |
| 外挂内容安全扫描（插件加载防护） | `scripts/plugin_content_scan.py`：PC-001~PC-010（危险删除/任意命令执行/远程脚本执行等），由 `tools/lib/plugin_loader.py:_scan_plugin_content()`（行725起）调用 |
| 设计-实现对齐验证器（AA） | `tools/lib/alignment_audit.py`：UC-08~UC-13（6个 check_uc0x 函数），反向对账"doc 承诺门禁↔gates 注册↔实现真实性"，CLI `ae-sdd update-check` |
| 设计-实现一致性迭代检查（IC） | `tools/lib/iteration_check.py`：IC-1~IC-4 机器粗筛（report-only 不阻断），CLI `ae-sdd iteration-check` |

**颗粒度与边界**：测试真实性扫描是 ⑥.10 硬门禁；扫描器均不可被 SKILL 文字描述替代；RA scanner 的 `--file` 是 Work Item authoritative scope，`--root` 是 formal RA 全量审计，两者同时出现时 file scope 优先；missing/outside/non-Markdown explicit file 返回非 0 与 `INVALID_RA_SCAN_SCOPE` JSON。AA（UC-08~13）阻断式，IC（IC-1~4）report-only 不阻断；扫描器路径变更须同步更新 update-graph.json。

---

## 13. 工具链 CLI

### 设计

ae-sdd Python CLI，将 SKILL 规则工具化，实现"规则描述 + 工具执行"双轨 SSOT。规则在 SKILL.md 描述，执行在 CLI 实现，两者通过 `ae-sdd update-check` 自动验证一致性；CLI 是门禁、状态、资产、记忆等能力的统一执行入口。

**⚠️ 文档滞后修正**：原文档标题写"14 大类子命令"，实际顶层子命令组已达 30 个（新增 `doc / enter / context-pressure / ra-gate / flow-violation-scan / ra-depth-scan / ra-implementation-scan / review-loop / plugin / iteration-check / perf` 等）；原表格中列出的 `route / sync-tools / run / quick / proposal` 顶层命令**不存在**，已从下表删除。

### 实现

| 入口 | `tools/bin/ae-sdd`（Python 3），lib 模块见各章节 |
| --- | --- |
| 输出协议 | JSON 走 stdout，日志走 stderr，pipeline 友好 |

| 类别 | 实际子命令（按 `tools/bin/ae-sdd` 现状） |
| --- | --- |
| 资产类 | `assets generate / check / read / outline / section / query / stats` |
| 文档存取类 | `doc save / resolve / finalize` |
| 状态机类 | `state read / write / next-step / confirm / prd-init / prd-check-complete / prd-complete / prd-archive` |
| 入口凭证类 | `enter <projectKey> [--story <ID>]` |
| 路由类 | `classify` |
| 门禁类 | `gates check / gate ra-required / gate coding-required / gate doc-storage / gate-intercept / ra-gate` |
| 记忆类 | `memory enter / write / exit / read / search / promote / summarize` |
| 数据库类（只读） | `db profiles / query / explain / audit` |
| Git 类（只读） | `git status / diff / log / blame / impact` |
| 更新图谱 | `update-check [--only UC-XX] / update-check --affected <文件>` |
| 迭代检查 | `iteration-check` |
| 上下文压力 | `context-pressure` |
| RA 扫描类 | `ra-authenticity-scan / ra-depth-scan / ra-implementation-scan / flow-violation-scan`，均支持 repeatable `--file` 与 `--root` |
| Review 类 | `review-loop` |
| 性能诊断类 | `perf report / perf doctor / perf clear` |
| 版本类 | `bump <ver> / version` |
| 维护类 | `health / init / init-hooks / runtime / plugin / scripts-dir / prompt-inject / stop-check` |

**批处理退出码边界**：单个 `ae-sdd` 命令的退出码是权威结果；连续执行多个命令时，shell 最终退出码只代表最后一条命令。PowerShell 编排必须在每条可能失败的命令后读取并判断 `$LASTEXITCODE`（或设置显式 fail-fast 包装），不能在前一条 `doc save`/gate 已返回 1 后继续运行成功命令，再把最终 0 误记为前一条 CLI 成功。`doc save --intent RA` prerequisite 失败会返回 1、不生成正式 RA、保留草稿；该契约由 subprocess 回归测试验证。

- `ae-sdd health` 9 项自检：子 SKILL 章节完整性 / 项目资产双源一致 / 规则-工具同步 / 门禁覆盖度 / TR-1~7 / 扫描器就绪 / CHANGELOG 版本一致
- `ae-sdd update-check`：权威源 `source/standards/update-graph.json`（UC-01~UC-16，比早期 UC-01~06 更完整），版本号三处一致 / 门禁注册一致 / 命令契约闭环 / 扫描器分发 / 健康度清单覆盖 / 文档-实现一致性 / runtime 编译一致性 / 自动化级联一致性等；dev-sync 前必须全绿
- `ae-sdd iteration-check`：IC-1~4 机器粗筛（report-only），接管人工 SOP 步骤 2/3/4
- `ae-sdd perf report/doctor/clear`：运行时统计查询、慢点建议和本地统计清理；统计只记录命令/span 耗时、退出码、脱敏 argv 与有限属性，不记录业务文档正文

**颗粒度与边界**：db/git 工具为只读；G-00 由 AI Agent 手动 `gates check --only G-00`（非 CLI 自动触发）；update-check 权威源是 `source/standards/update-graph.json`；改 CLI 命令契约须同步 update-graph.json；iteration-check/context-pressure/perf 均 report-only，不阻断 dev-sync。

---

## 14. state.json events 操作日志设计

### 设计

v3.4.0 引入 PRD 级 state.json + phase 状态机，但子任务级操作的可追溯性弱：`phase` 字段只记"当前阶段"没有轨迹，`history` 字段粒度过粗，门禁拦截只在触发时记录结果没有"何时由谁触发"的 audit trail。

**方案**：state.json 增加 append-only `events` 数组，每条记录一次"流程动作"（路由/门禁/阶段切换/用户确认等）。**不替代**现有字段——events 与 `phase`/`history`/`currentStory`/`currentTask` 共存：phase = 当前阶段（高频读），history = 阶段切换摘要（低频读），events = 细粒度操作（运维审计/复盘）。

关键设计决策：① 继承 `str, Enum` 使 JSON 序列化直接得字符串，日志人工可读；② append-only 不去重，保证 audit trail 真实性；③ `txnName` 区分 PRD/Story/Task/Plan 事件；④ 不强制落盘，事件写入与持久化解耦，调用方按需 write_state；⑤ 向后兼容，旧 v1 state.json 无 events 字段时不报错。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 3 个枚举 | `tools/lib/flow_enums.py`：`FlowNode`(6成员) / `FlowSkill`(15成员) / `FlowEventType`(8成员：routed-to/skill-completed/gate-blocked/gate-cleared/user-confirmed/phase-changed/reopened/aborted) |
| 事件数据类 | `FlowEvent`，必填 `seq/ts/event/node/by`，条件必填 `skill/gate_id/phase`，可选 `txnName/from_node/reason/output/meta` |
| 5 个工厂函数 | `make_routed_to` / `make_skill_completed` / `make_gate_blocked` / `make_phase_changed` / `make_user_confirmed` |
| 追加事件 API | `tools/lib/state.py:append_event(state, event)`：原地追加，自动填 seq/ts，不自动落盘 |
| 查询事件 API | `tools/lib/state.py:get_events(state, *, txn_name=None, event_type=None, node=None)`：三维过滤 |

**⚠️ 已知缺口（业务调用方尚未全部接入）**：

| 缺失点 | 预期接入方 | 状态 |
| --- | --- | --- |
| 路由时记录 `routed-to` | `cmd_classify` / router | 待接入 |
| 阶段切换时记录 `phase-changed` | `cmd_state_write` | 待接入 |
| 门禁检查时记录 `gate-blocked`/`gate-cleared` | `gates.py` 各 check_gXX() | 待接入 |
| SKILL 完成时记录 `skill-completed` | 各 SKILL orchestrator | 待接入 |
| 用户确认审核点时记录 `user-confirmed` | ae-sdd-update-skill §审核点双支柱 | 待接入 |

lib 层 schema 已就绪且测试通过；一旦业务调用方接入，events 立即生效，无需 schema 升级。

**颗粒度与边界**：events 只追加不修改历史记录；txnName 为 None 表示 PRD 级事件；事件写入与磁盘持久化解耦，避免高频事件导致 IO 风暴。

---

## 15. Hook 层（三层拦截体系）

### 设计

三个 Claude Code hook 实现物理级流程纪律，覆盖工具调用拦截、上下文注入、响应后校验三个时机，不依赖 LLM 自律。

Hook 默认处于 inactive。只有当前用户 turn 显式进入 `/ae-sdd`，或执行明确的 ae-sdd 写流程入口时，才创建 session 级 turn token；普通提示（包括 Story 文档对齐）不会解析或注入 Work Item。Stop 成功、fail-open 或下一条普通提示都会清理 token，Stop 阻断重试时暂时保留 token。token 不等同于 Work Item writer lease，也不读取项目级 state 作为激活信号。

- **PreToolUse**：AI 每次调用工具前触发，按 phase 限定允许工具集
- **UserPromptSubmit**：用户每次提交消息前触发，注入状态上下文 + 主流程监管器逻辑（见 §16）
- **Stop**：AI 每次回复结束后触发，防无限循环 + 检测结构性错误

**决策 1B（v3.6）**：废弃 `◆ STATE:`/`◆ LOADED:` 自报标记检测（"防君子不防小人，可谎报"），改为纯产物核查（gates check 结果为唯一权威判定）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| PreToolUse hook | `tools/lib/gate_intercept.py`：先检查当前 session turn token；inactive 时直接放行；active 后由 `PHASE_PERMIT` 按 phase 限定工具集；只读工具任何 phase 放行；`paused` phase 禁全部写操作；链式 Bash（含 `&&`/`\|\|`/`;`/`\|`）不认为只读；输出 `permissionDecision: "deny"` + `systemMessage`，exit 始终 0 |
| UserPromptSubmit hook | `tools/lib/prompt_inject.py`：只有 `/ae-sdd` 触发词才创建 turn token、读取 state 并注入阶段上下文；普通 prompt 清理残留 token 后返回 `{}`；active turn 再执行快速通道、重置 Stop 重试计数、母版版本漂移探测、主流程监管器和 compact memory 注入 |
| Stop hook | `tools/bin/ae-sdd` 先检查 turn token；inactive 直接放行；active 时调用 `tools/lib/stop_check.py`，成功或 fail-open 后释放 token，阻断重试保留 token；`.stop_retry_count` 仍为 MAX_RETRY=2，继续检测 PRD compact 卡住态 |
| Hook 安装 | `ae-sdd init-hooks` 写三段配置到 `.claude/settings.json` |
| 快速通道 | 用户说 `/ae-sdd-quick` → `.quick_channel` 写入 → PreToolUse 跳过 phase 工具拦截（产物落地关卡仍生效） |

**颗粒度与边界**：hook 层只做物理拦截（工具权限/上下文注入/结构校验），不做业务逻辑判断；hook 任何异常降级不阻断主流程（全流程 try/except，exit 始终 0）；改 hook 须同步 HARNESS.md HS 规则；禁止手工编辑已安装的 hook 脚本，改完须重跑 `ae-sdd init-hooks`。

---

## 16. 主流程监管器（v3.6，已实现）

> 文档修正说明：本节原标"🆕 v3.6 规划中"，经代码核实（`tools/lib/flow_monitor.py` 全文 326 行 + `prompt_inject.py:_run_flow_monitor()` 行242 + `state.py` 暂停/恢复/矫正计数 API 均已存在并接入 UserPromptSubmit hook），确认**已完整实现并生效**，非规划态。

### 设计

`/ae-sdd` 触发后全生命周期的编排角色，负责工作区初始化、资产检查、智能路由、各系列循环执行、收尾交付，不执行任何具体业务工作——是 ae-sdd 的"项目经理"，只管"下一步该做什么、由谁做、做完了没有"。角色定义在 SKILL.md，执行实体是 UserPromptSubmit hook 的 Python 逻辑（决策 2B）。

5 步标准启动序列：Step 1 工作区确认与初始化 → Step 2 项目资产检查(G-00) → Step 3 智能路由（双层裁定）→ Step 4 按 scale 子链执行编码流程 → Step 5 收尾交付。每系列执行协议含 sub-step 0~5（compact清理 → SKILL调用声明 → 生成 → Review → Loop控制 → 人工审核）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 偏移检测核心逻辑 | `tools/lib/flow_monitor.py:detect_drift(state, ade_sdd)`（行144），Layer 1 产物核查 + Layer 3 矫正次数升级 |
| phase→gate 映射表 | `flow_monitor.py:get_phase_gate_map()`（行70），覆盖 ra-generated~test-running 各 phase |
| 升级判定 | `flow_monitor.py:should_escalate(state)`（行236），矫正次数 ≥ `CORRECTION_THRESHOLD_PAUSE`(3) |
| 矫正消息生成 | `flow_monitor.py:build_correction_message(drift)`（行272），severity 1/2/3 三级文本模板 |
| gates check 子进程调用 | `flow_monitor.py:_run_gates_check(gate_id, ade_sdd)`（行115），10s 超时降级放行 |
| Hook 侧调用入口 | `tools/lib/prompt_inject.py:_run_flow_monitor(ade_sdd, state)`（行242），每轮 UserPromptSubmit 执行，异常降级返回 None |
| 状态暂停/恢复 | `tools/lib/state.py:pause_state()`（行663）/ `resume_state()`（行684） |
| 矫正次数持久化 | `state.py:increment_correction()`（行706）/ `get_correction_count()`（行725），写入 `state.correctionCounts` |
| SKILL 侧角色定义 | `source/skills/orchestration/ae-sdd-update-skill.md`（5步启动序列文字描述） |

**颗粒度与边界**：监管器角色定义在 SKILL.md，执行实体在 UserPromptSubmit hook（Python，决策 2B）；监管器不执行业务工作，只做编排+校验；ae-sdd 自更新触发时监管器休眠；退出条件：`phase=completed` 或用户手动中止；每系列入口必做 compact（防上下文污染）。

---

## 17. 流程偏移检测与矫正（v3.6，已实现）

> 文档修正说明：本节原标"🆕 v3.6 规划中"，实际与 §16 共用同一套 `flow_monitor.py` 实现，已完整生效。

### 设计

识别 AI 在执行过程中的语义漂移行为，按 3 级矫正机制自动纠正，超出阈值升级人工干预并暂停流程。流程偏移分两类：物理越权（已由 PreToolUse hook 拦截）和语义漂移（本模块处理）。

漂移类型：B1 跳步（跳过 Review 宣布进入下一系列）/ B2 停滞（同一 phase 经 N 轮产物未过 gates check）/ B3 伪完成（声称完成但门禁未过，原依赖自报检测，v3.6 改产物核查）/ B4 旁路（题外话后未回到流程）。

矫正级别：Level 1 静默注入（用户不可见，AI 可感知）/ Level 2 矫正提示词（AI 须说明修复计划，同步骤最多3次）/ Level 3 人工升级（`state.phase=paused`，流程暂停待用户决策）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 漂移结果数据类 | `flow_monitor.py:DriftResult`（行45起）：`drift_type/severity/gate_id/gate_passed/gate_message/phase/correction_count/message` |
| 阈值常量 | `flow_monitor.py`：`CORRECTION_THRESHOLD_WARN=5`（行36）/ `CORRECTION_THRESHOLD_PAUSE=3`（行38）/ `GATES_CHECK_TIMEOUT=10`（行40） |
| Layer 1 产物核查（解 B1/B3） | `detect_drift()`（行144-225）依据 `get_phase_gate_map()` 跑 gates check，不通过判定漂移，不信任 AI 自报 |
| Layer 3 矫正次数升级（解 B2） | `detect_drift()` 行204：`correction_count >= CORRECTION_THRESHOLD_PAUSE` → severity=3 |
| severity=1 矫正文本 | `build_correction_message()` 行290-296：静默提醒 |
| severity=2 矫正文本 | `build_correction_message()` 行298-309：含矫正次数/失败门禁详情 |
| severity=3 暂停文本 | `build_correction_message()` 行311-323：含继续/跳过/回退/查看四个用户决策选项 |
| phase→gate 映射（9个phase） | `get_phase_gate_map()`（行70-89）：ra-generated→G-RA-1~4，dr-generated→G-01，story-generated→G-02，story-reviewed→G-03，task-generated/task-reviewed→G-04，coding-process→G-08，coding→G-CODEPLAN-SRC，code-reviewed→G-05，test-running→G-09 |
| Stop hook 职责精简 | `tools/lib/stop_check.py`：已删除 `◆ STATE:`/`◆ LOADED:` 自报检测，职责简化为防空响应+防无限循环 |
| 全流程降级保护 | `detect_drift()` 行227-233：任何异常返回 `drift_type="none"` 不阻断主流程 |

**颗粒度与边界**：只检测语义漂移（物理越权由 PreToolUse 拦截）；gates check 结果为唯一权威判定，不信任 AI 自报（决策1B）；`correctionCounts` 写入 state.json 跨 session 持久化；`flow_monitor.py` 是纯计算模块（只返回结论不直接写 state），写 state 由 `prompt_inject.py` 调用 state API 执行；Level 3 暂停后续接检测自动识别 `phase=paused` 并播报。

---

## 18. SKILL 编译与 Runtime IR（v3.8 设计）

### 设计

ae-sdd 分为两种物理版本：`source/` 是未编译母版，面向维护者；`dist/ae-sdd/` 是编译后的实例化运行包，面向各 Agent。正式发版只能分发编译后版本，不能直接把 `source/` 安装到 Agent skills 目录。

编译目标不是把 SKILL 变成不可读"机械码"，而是生成短、结构化、可审查的 runtime compact slices。Agent 主入口 `dist/ae-sdd/SKILL.md` 变为 bootloader，只声明加载顺序、冲突优先级和 fallback 规则；子 SKILL 在 `dist/ae-sdd/skills/**/*.md` 中也必须变为 compiled bootloader，完整 Markdown 原文只保存在 runtime fallback 中，只有 compact 不足时才延迟读取。

源 SKILL 入口在进入 runtime 编译前允许标准化瘦身，但瘦身不是自由删减。`scripts/slim_source_skills.py` 必须先把完整原文锚定到 `source/skill-fallbacks/**`，再按 `source/standards/skill-source-slimming-standard.md` 和 `source/templates/skill/source-skill-slim-entry-template.md` 渲染 slim entry。每个 slim entry 必须包含语义识别清单，覆盖身份/触发、流程/路由、门禁/约束、工具/API、状态/数据、产物/文档、资源引用、设计对齐和 fallback-only 细节；已瘦身文件默认跳过，schema 升级只能从 fallback 重渲染，禁止从 slim entry 二次瘦身。

源瘦身的语义边界：slim entry 是索引和加载契约，`source/skill-fallbacks/**` 是完整语义锚点。runtime 编译器遇到 `source_slimmed: true` 时，fallback 和 outline 抽取必须来自 `source_fallback`，不能来自 slim entry。

编译后运行包的核心结构：

```text
dist/ae-sdd/
├── SKILL.md                         # compiled bootloader
└── runtime/
    ├── manifest.json                # 版本、source hash、runtime_fingerprint、load_order
    ├── boot.compact.md              # 加载契约
    ├── route.compact.md             # 路由压缩表
    ├── subskills.compact.md         # 子 SKILL 编译入口索引
    ├── gates.compact.md             # 门禁压缩表
    ├── flow.compact.md              # 状态机压缩表
    ├── macros.compact.md            # 公共动作宏
    ├── skills/**                    # 子 SKILL 局部 manifest/boot/outline/fallback
    └── fallback/SKILL.full.md       # 原始主入口 fallback
├── skills/**/*.md                   # 子 SKILL compiled bootloader，非原文
```

冲突优先级：用户最新明确指令（不得绕过硬门禁） > CLI/gate/state 真实输出 > runtime compact > 子 SKILL compiled bootloader/outline > 子 SKILL fallback > 主入口 fallback > 历史说明文档。

完整说明见 [`source/docs/skill-runtime-compiler.md`](skill-runtime-compiler.md)。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 未编译母版 | `source/`，唯一人工编辑点 |
| 源 SKILL 瘦身 | `scripts/slim_source_skills.py`，按 v2 标准生成 slim entry；完整原文进入 `source/skill-fallbacks/**` |
| 源瘦身标准/模板 | `source/standards/skill-source-slimming-standard.md` + `source/templates/skill/source-skill-slim-entry-template.md` |
| 编译实例包 | `dist/ae-sdd/`，由 `scripts/build_dist.py` 生成，git ignored |
| Runtime 编译器 | `scripts/compile_skill_runtime.py`，生成 `runtime/*.compact.md`、`runtime/skills/**`，并替换 dist 主入口和子 SKILL 入口为 bootloader |
| 通用编译器 SKILL | `standalone-skills/skill-runtime-compiler/`，可复制到其它 agent/仓库，用于把任意 `SKILL.md` 包编译成同级 `<name>-compiled/` |
| 编译接入点 | `scripts/build_dist.py` 在复制 source/tools/scripts 后调用 runtime 编译器 |
| 门禁 compact 数据源 | `tools/lib/gates.py:GATE_REGISTRY` |
| 状态机 compact 数据源 | `tools/lib/state.py:PHASE_FLOWS` |
| 机器校验 manifest | `runtime/manifest.json`：`compiled=true`、`deterministic=true`、`runtime_fingerprint`、`load_order`、`source_checksums` |
| 分发入口 | `scripts/distribute.py` 只接受 `dist/ae-sdd/` 或基于它生成的 Agent 专属产物 |

**颗粒度与边界**：第一期编译 boot/route/subskills/gates/flow/macros 六类高收益规则，并把所有子 SKILL 入口编译为薄 bootloader；子 SKILL 的完整长流程、模板正文、设计背景仍按需 fallback。源瘦身只压入口，不压语义；`source_fallback_sha256` 和语义 inventory hash 是防丢语义的机器锚点。compact runtime 不替代 CLI gate，任何硬门禁判断以 `ae-sdd gates check`、`state` 等工具输出为准。runtime 编译产物必须字节级幂等，不写入墙钟时间；同一份 `source/`、同一版编译器、同一份 `GATE_REGISTRY` 与 `PHASE_FLOWS` 重复编译，`dist/ae-sdd/SKILL.md`、`dist/ae-sdd/runtime/**` 和 `dist/ae-sdd/skills/**/*.md` 必须完全一致。

**当前实现状态**：源 SKILL 标准化瘦身 v2、瘦身标准/模板、第一期编译器、构建接入、runtime manifest、六类 compact slice、全量子 SKILL compiled bootloader、fallback 原文锚点、字节级幂等测试、`ae-sdd runtime verify`、`update-check` UC-15 runtime 编译一致性检查、分发器 compiled-only 校验均已落地。另有通用 `skill-runtime-compiler` standalone SKILL，可把任意 SKILL 包编译到同级 `<name>-compiled/`，源包不变，并由 UC-15 覆盖幂等性。后续扩展重点是外层 `dist` 包可复现构建、子 SKILL 语义 compact 增强、Agent 专属二次编译约束细化。详细清单见 [`source/docs/skill-runtime-compiler.md`](skill-runtime-compiler.md) §14~§15。

---

## 19. 自动化模式（v3.8.0，已实现）

### 设计

默认关闭的"全自动化开关"。开启后 6 个人工审核点（1/1.5/2/2.5/4/5）改走 Tier 3 多 reviewer 联审共识，跳过所有人工✅，实现 ae-sdd 输入→结果。联审机制复用 `agent-orchestration-skill §8.4`（Tier 判定 + 视角正交 + 交叉对比 + 降级规则），本模块只管开关与审核点行为分叉。

核心立场：
- **默认关闭**：`automation.enabled=false`，回退现状（每审核点等用户✅）
- **开启即强制 Tier 3**：覆盖规模/关键决策判定，自动化模式跳过人工✅必须最高强度联审兜底
- **禁逻辑多视角降级**：自动化模式必须物理 3 独立 session reviewer，环境不支持 → `state.phase=paused`（不得用 logical-multi-perspective 凑数）
- **开工前信息预收集**：开工前一次性向用户收集所有必需信息（第三方凭证/复用选择/环境配置/命名约定/对接方/数据初始化），开工后不再打断
- **阻断出口**：联审 3 轮矫正未决 → `state.phase=paused`（默认），输出完整问题清单等用户介入，避免 AI 带病狂奔

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 配置 SSOT | `.ae-sdd/config.yaml` 的 `automation` 段（`scripts/init.py:CONFIG_TEMPLATE` 生成，默认 `enabled: false`）|
| 配置加载 | `tools/lib/config.py`：`AUTOMATION_DEFAULTS` + `load_automation_config()` + `is_automation_enabled()`/`get_reviewer_tier()`/`get_automated_points()` |
| CLI 开关 | `tools/bin/ae-sdd`：`automation status/enable/disable`（enable 写 `enabledAt` 审计时间戳，AI 不得自行改）|
| 开工前预收集 | `ae-sdd preflight collect`：扫输入材料+资产 7 层索引，识别 6 类待补信息，写 `.ae-sdd/preflight-info.yaml` |
| 联审共识 state | `tools/lib/state.py`：`reviewConsensus[point]` 字段 + `register_review_consensus()`/`get_review_consensus()` |
| 联审共识写入 | `ae-sdd state register-review-consensus --point {N} --passed {true\|false}` |
| 联审共识门禁 | `tools/lib/gates.py:G-AUTO-CONSENSUS`（blocker，34 门禁之一）：自动化模式下 review 节点切相前校验 `reviewConsensus[point].passed=true` + reviewer 独立性（复用 G-09B 模式）|
| Tier 强制 | `agent-orchestration-skill.md §8.4.1`：`automation.enabled=true` → 强制 Tier 3，覆盖规模判定 |
| 降级禁止 | `agent-orchestration-skill.md §8.4.5`：自动化模式禁逻辑多视角降级，必须物理 3 独立 session |
| 流程编排 | `source/SKILL.md §🚀 自动化模式` + Step1 自动化检测 + Step1.5 预收集 + 监管器步骤4 联审共识双模式 |
| 级联检查 | `tools/lib/update_graph.py:check_uc16_automation_cascade`（UC-16）：校验 config.py/gates/state/CLI/init/SKILL 六处齐备 |
| 级联图谱 | `source/standards/update-graph.json:UG-20`：trigger 含 config.py/init.py/gates.py/state.py/ae-sdd |

**颗粒度与边界**：自动化模式不新增 phase，复用现有 PHASE_FLOWS 状态机骨架，只在审核点行为分叉（默认 vs 联审共识）；reviewConsensus 仅作用于 review 节点，不挂 PRD 4 层 AND 闸；6 审核点讲解规范（§📖）不变，自动化模式仍讲解，听众从"用户"变成"3 个 reviewer"；G-09B/G-REVIEW-LOOP 现有逻辑复用，不重复实现。

---

## 20. ae-sdd Monitor（只读可视化投影）

### 设计

ae-sdd Monitor 是 ae-sdd 的本地桌面可视化投影层，用于在一个父目录下发现多个 ae-sdd 工作区，并集中查看每个工作区的当前 phase、派生状态、最近活动、工作项、Memory 状态和 Runtime Stats。它服务的是“监管运行状态”和“快速定位异常/暂停/完成态”，不改变 ae-sdd 主流程。

核心立场：

- **只读投影**：Monitor 不写 `.ae-sdd/`、不写 `.auto-engineering/`、不执行 gate，不把 UI 状态反写为 ae-sdd 状态。
- **主设计优先**：phase、scale、entry node、门禁语义和 Runtime Stats 含义以本文档其它章节与实现架构文档为准，Monitor 只能跟随展示。
- **多工作区入口**：用户选择父目录，Monitor 扫描其中所有包含 `.ae-sdd/` 的工作区；左侧列表用于选择，右侧详情用于观察。
- **两级导航/看板**：左侧必须分项目与任务两级且项目可折叠，点击项目切项目级看板，点击任务切任务级看板，点击折叠控件只展开/收起任务；右侧顶部必须同时体现当前项目与当前任务。
- **局部响应式切换**：同一项目下点击任务不得清空或重建整个详情页；Monitor 只能更新当前任务、指标、Tab 内容和侧边栏选中态。
- **React renderer**：Monitor 的桌面壳仍由 Electron 提供，本地 UI 内胆必须由 React + TypeScript 组件实现；侧边栏项目/任务树必须通过稳定 key 保持节点连续性，禁止回退到整块 DOM 替换造成闪烁。
- **结束态也可观察**：`completed`、`paused`、`idle`、`invalid` 等状态都必须可展示，不能只服务正在运行的工作区。
- **多活跃任务可见**：Monitor 必须同时展示根 state、activeAgents 和 `.auto-engineering/{WORKITEM-KEY}` 中的活跃/未完成工作项，不能压缩成单个 activeWorkItem。
- **Memory 状态可见**：Monitor 必须展示 `.ae-sdd/memory` 的项目级/任务级 memory 数量、最近记录、活跃 scope 与阻断 scope，不能只看 phase/state。
- **阶段轴可见**：时间线必须展示完整 phase 链和当前节点说明，即使 state history/events 为空也能判断工作区处于哪个节点。
- **响应式观察**：默认通过文件系统事件监听 `.ae-sdd/` 与 `.auto-engineering/` 变化并静默刷新当前项目/任务；低频轮询只作为兜底；自动刷新仍然只读，不执行 gate 或 memory 命令。
- **动效连续性**：项目折叠、Tab 切换和按钮按压可以使用轻量 iOS 风格动效，但加载、任务切换和响应式刷新不得持续闪烁；动效只能表达 UI 反馈，不得暗示 ae-sdd 状态被写入或 gate 被执行；必须支持系统 reduced-motion 降级。
- **体验连续性**：目录选择必须有即时反馈；重启后必须恢复上次父目录、选中工作区和选中任务；类 Mac 窗口三点必须是真实窗口控制。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 独立设计文档 | [`source/docs/ae-sdd-monitor-design.md`](ae-sdd-monitor-design.md) |
| 应用位置 | `apps/ae-sdd-monitor/` |
| 扫描入口 | `apps/ae-sdd-monitor/src/workspace.js:scanForWorkspaces()` |
| 状态读取 | 只读取 `.auto-engineering/{WORKITEM-KEY}/state.json`；展示 `workItemKey/stateMachineId/currentWorkItem`，字段缺失时从 R6 顶层目录名回退推导 |
| Memory 读取 | `.ae-sdd/memory/**/*.jsonl`、`.ae-sdd/memory/.stage/*.json`；展示项目/任务 memory、活跃 scope 与阻断 scope |
| 性能读取 | `.ae-sdd/runtime-stats/*.jsonl` |
| 展示结构 | 左侧可折叠项目/任务两级列表 + 右侧当前项目/当前任务两级看板 + 总览/阶段轴与事件流/Memory/活跃任务与工作项/性能/原始状态 |
| UI 动效 | `src/styles.css` 与 `renderer/src/App.tsx` 只实现本地交互反馈、折叠过渡、Tab/detail 过渡和 reduced-motion 降级；响应式刷新不得让看板闪烁，不改变 ae-sdd 项目文件 |
| 偏好保存 | Electron userData `preferences.json`，保存父目录、选中工作区、选中任务、项目折叠状态、自动刷新开关和主题 |
| 发包目标 | Windows setup exe/zip + macOS dmg/zip |
| 同步闭环 | `source/standards/update-graph.json:UG-22` |

**颗粒度与边界**：Monitor 是伴随工具，不是 ae-sdd runtime 的一部分；它可以把文件状态可视化，但不能成为 gate、state 或 Runtime Stats 的权威源。任何涉及状态 schema、阶段链、Runtime Stats JSONL 字段或项目侧路径的变化，必须同步 Monitor 独立设计文档、解析代码、测试和 README。

---

## 21. Sonar Issue 修复与 CodeReview 收尾

### 设计

Sonar 修复是 CodeReview 的节点内收尾能力，而不是新的主流程 phase。每条 issue 必须唯一归入 `upstream-edit`、`registry`、`reasoned`、`manual` 四种模式之一；只有完整上游 `TextEdit(range, newText)` 或独立维护的低风险注册表配方可以在防陈旧、防越界、防重叠和原子性校验后自动应用。

CodeReview 在第六步循环收敛之后、第七步最终闸门之前调用 Sonar Issue Fix，每个评审会话恰好一次。没有 Sonar 配置或数据时也必须产生 `N/A` 结果并占用本会话调用令牌。Sonar 如果修改源码，重开受影响的 compile、测试和评审证据，但同一会话不第二次调用；再次复扫需求留给新的 CodeReview 会话，避免递归。

复用边界：采用 SonarLint/IntelliJ 公布的 `TextEdit` 协议形状和官方 issue/rule/quality gate 输入，不自动操纵 IDEA 灯泡，也不假设 MCP 提供 quick-fix payload。首版注册表仅启用有严格前置条件和负例的 `java:S1128` unused import 删除。SonarJava 的 `Sonar Source-Available License v1.0` 不允许被当作可复制的 analyzer 实现来源；规则配方必须独立撰写，安全/taint/hotspot、认证、密码学、并发、事务和公共 API 问题保持人工处理。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 节点内工作流 | `source/skills/phase3-review/sonar-issue-fix-skill.md`，完整语义保存在对应 `source/skill-fallbacks/**.full.md` |
| CodeReview 挂靠 | `code-review-skill.md` 第六步 bis；会话调用计数、N/A、源码变化后的验证重开均由 SKILL 协议约束 |
| 规则注册表 | `source/standards/review/sonar-issue-fix-rules.md`；当前仅 `java:S1128` 为 enabled |
| 数据协议 | issue 归一化 + `baseSha256` + 有序非重叠 `TextEdit` + provenance + 验证证据 |
| 验证 | compile、受影响测试、Sonar 复扫、原 issue 消失、Blocker/Critical 零回归和 quality gate 对比 |
| 契约测试 | `tools/tests/test_sonar_issue_fix_skill.py`，覆盖模式、安全边界、许可证、索引和 exactly-once 调用位置 |

**颗粒度与边界**：这是 Markdown SKILL/规则层能力，不新增 CLI、gate、state schema、scanner 或后台服务；现有实现架构边界不变。若未来增加可执行补丁引擎或 MCP adapter，必须另行更新实现架构文档、威胁模型和工具级测试，不能在本注册表中暗增代码执行能力。

---

## 22. 风险驱动的有界测试策略

### 设计

TestCase 质量以独立缺陷发现价值和可追溯证据衡量，不以测试总数、字段/状态数量或证伪用例比例衡量。生成阶段先从 AC、契约、不变量、改动分支、历史缺陷、项目坑库和高影响风险建立有限风险登记，再按边界测试准入、行为等价类、最低充分层级和局部数量上限选择最小充分组合。

核心边界：

- 同一 validator、错误分支、失败影响和层级证据默认只保留一个代表。
- 字段组合只覆盖业务依赖，独立字段不做笛卡尔积；状态转换按 guard、副作用和失败机制分区。
- 安全、权限、金额、数据丢失、事务、并发、幂等、不可逆状态和显式契约边界命中适用条件时不得因预算静默排除。
- AC、已准入风险、改动分支和历史回归均有证据，且剩余候选不增加新失败机制、控制流、契约、协议、断言或层级证据时，必须停止。
- 超出局部数量上限必须记录新增价值、执行/维护成本、不可合并原因和确认人；固定全局用例上限同样禁止，因为它会让复杂高风险 Story 欠测。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 策略 SSOT | `source/standards/testing/be-testcase-strategy.md` |
| 生成端 | `testcase-generate-skill.full.md` 的有限风险登记、选择决策、TC-G11 |
| 评审端 | `testcase-review-skill.full.md` 的 TC-1~TC-10，独立复核漏测与无界扩张 |
| 产物契约 | `source/templates/testcase/be-testcase-template.md` 的风险登记、停止条件证据和预算例外 |
| CodingModel | `source/standards/thinking/be-coding-thinking-engine.md` 只把六类场景作为候选来源，不设数量配额 |
| 回归证据 | `tools/tests/test_bounded_test_strategy.py` 阻止旧公式、占比门禁和无条件跨层覆盖回流 |

**颗粒度与边界**：当前能力是 Markdown SKILL/标准层软约束，不新增 CLI、gate、state、scanner、Document Storage 或 Monitor 行为。generate 与 review 双重检查选择证据，但“独立失败机制”的语义判断仍依赖 Agent；长期价值需通过真实项目的 TestCase 数量、维护耗时和缺陷发现率补充基线。
