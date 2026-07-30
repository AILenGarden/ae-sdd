# ae-sdd 系统能力说明书

> v4.0.0 · 面向开发者、LLM Agent 与项目接入方
>
> 本文档是**系统能力设计入口**，说明 ae-sdd 的能力语义、边界和当前实现状态。代码分层、模块职责、运行时数据流和变更闭环统一维护在 [`ae-sdd-implementation-architecture.md`](ae-sdd-implementation-architecture.md)。若本文与代码实现冲突，以 CLI/测试输出为准，并同步修正文档。

## 过程产物模型

RA、DR、Story 是核心设计文档。Proposal、GeneratePlan、CodingReport、TestReport、CodeReview 报告和其他过程 Markdown 停止新写入；历史文件只读。普通测试设计内嵌 Story 验证矩阵，复杂矩阵才使用独立 TestCase；Task 仅用于大型并行拆分。编码前审核使用 `state.executionPlan`，测试使用 evidence manifest，Review 使用 `state.review.status/findings`。任何时候都不写 changelog。

---

## 当前 Rust Runtime 权威设计

ae-sdd 的执行形态是每个 OS 用户一个 `ae-sddd`，多个 Agent 和 workspace 通过本地 Named Pipe/UDS 共享状态、规则、缓存、调度和审计。`source/SKILL.md` 只声明方法、模板和输出契约，不再执行 phase machine；确定性执行者是 daemon 内每个 Work Item 的 `FlowRuntime`，项目 mutation 的唯一 owner 是 `WorkItemActor`。

```text
Agent Hook / ae-sdd CLI
        |
        v
protected endpoint + runtime.handshake
        |
        v
ae-sddd -> Workspace/Session/Turn actors -> FlowRuntime/FlowSupervisor
        |                                  |
        |                                  +-> Delegation + HostRuntimeAdapter
        |                                  +-> ContextProjection + CompactManager
        +-> Operations/Gates/Scanners -> journal + state + rebuildable SQLite index
```

核心不变量：

- FlowRuntime 从已提交 state、全局事件序列、policy digest 与 input fingerprint 计算唯一 `nextAction`；Agent 或 SKILL 文字不能自行推进 phase。
- root 主会话只推动 typed action、创建物理委派、collect 有界 `ChildResult` 和向用户汇报。RA/DR/Story/Coding/Test/Review 等系列语义工作在独立 series session 中完成；lineage 为 `root -> series -> task|reviewer`，最大深度 2。
- Host ACK 只表示原生命令已接收。child 必须使用一次性 claim 在独立物理 session 中完成 attestation，之后才可 running；reviewer 与 worker 物理隔离。
- root 只接收摘要（最多 8 KiB）、finding 统计、next actions 与 artifact path/hash/kind；ChildResult 与 root projection 各最多 64 KiB。child transcript、原始 source bundle、scratch memory 和无界日志不得注入 root。
- Hook 是薄 Rust client 快路径，只读预计算 projection/delta；不在同步路径扫描文档、启动子进程、跑重 Gate、等待 compact 或执行项目 mutation。engaged 时 daemon 不可用即 deny/block，不允许本地业务回退。
- Gate 结果保留 `PASS/FAIL/ERROR/TIMEOUT/CANCELLED/STALE` 六类。只有 fresh PASS 允许依赖 mutation，只有 fresh FAIL 增加业务 correction。
- compact 只接受可信 host token telemetry；默认使用 800/600 permille 滞回、连续两个样本与 300 秒 cooldown。仅 snapshot、匹配原生 ACK、rehydrate 全部完成后进入 `context-restored`。
- workspace writer 模式按 `legacy -> shadow -> rust-canary -> rust-sole-writer` 迁移；shadow 只读比较，任何阶段禁止双写。删除 legacy runtime 后只允许回退到上一版完整 native release，不在 Rust client 中保留隐藏 fallback。

路由仍遵循 v3.14 语义：先 route，再 Requirement Analysis；分析后按需选择 DR、Story 或 compact `state.executionPlan`。用户批准 `executionPlan` 前不得 Coding。

**Python 树已于 2026-07 全部删除**，本文各章节的"实现位置"已在同期全量重指到 `crates/ae-sdd-*` 与 `bins/ae-sdd-*`。能力语义由 Rust 实现继承：门禁编号、phase 链、state 字段、扫描规则均未变。

两处已知缺口在正文中显式标注，不要当作可用能力：UC-08~13 的设计-实现对齐审计（原 `alignment_audit.py`）无承接实现；`ae-sdd update-check` 仅 UC-01 为真实检查。逐条 D-xxx 到 Rust 测试的证据映射亦未建立，见 §0.2 表头说明。

实现落点的最终权威是 [`ae-sdd-implementation-architecture.md`](ae-sdd-implementation-architecture.md) §4 模块职责表。Monitor 设计与 `apps/ae-sdd-monitor/**` 不在 runtime 迁移范围。

---

## 0. 设计问题与价值总览（Design Ledger）

本表是当前系统级设计的导航和问题台账，覆盖本文件 §1~§21。它回答四个问题：为什么存在、采用什么决策、预期带来什么价值、用什么证据验证。详细设计仍在对应章节，代码职责仍以 `ae-sdd-implementation-architecture.md` 为准。

### 0.1 记录规则

- `预期价值` 是设计假设，不等同于已测收益；只有 `验证证据` 中有命令、测试或运行指标时，才能宣称已验证。
- 历史设计缺少精确版本时记录 `历史版本（精确版本待补）`，不得凭记忆编造版本号。
- 每次迭代必须在 changelog 的 `Design ledger impact` 中填写受影响的 D-xxx；没有设计语义变化时填写 `N/A: no design semantics changed`。
- 设计语义、边界、实现归属或验证方式发生变化时，必须同步本表的“最近变更”和验证证据，并按 `ae-sdd-update` 的 UG-28/UC-20 流程校验。

### 0.2 当前系统级设计台账

> **验证证据列的读法**：以 `test_*.py` 形式出现的条目是 Python 时期的证据记录，对应测试文件已随 Python 树删除，**不能再运行、也不构成当前证据**。这些设计的当前证据是 `cargo test --workspace` 下的 Rust 测试；逐条 D-xxx 到 Rust 测试的映射尚未建立，属已知缺口。非 `.py` 的条目（CLI 命令、门禁 ID、operation 名）仍然有效。

| ID | 设计 | 要解决的问题 | 核心决策 | 预期价值 | 验证证据/指标 | 权威入口 | 引入/最近变更/状态 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| D-001 | 端到端 Phase 编排 | AI 容易跳过设计、审核、测试或收尾节点，产物链断裂 | Phase 1/2/3、节点门禁、人工确认和出口产物组成有序状态机 | 降低流程遗漏和返工，提升交付可追溯性 | `gates check`、state phase、Story/Task/Coding/Review 链路 | §1；`source/SKILL.md` | 历史版本（精确版本待补）/v3.11.0/已实现 |
| D-002 | 路由后需求分析 | 仅按规模把任务送入固定文档链，RA 被误作 DR 前置 | 先 route，再产出本次任务 RA 需求说明书，再按分析结论选择 DR/Story/CodingPlan | 解耦分析与设计深度，减少过度设计并保留可审计事实 | `ae-sdd classify`、`routeDecision`、`PHASE_FLOWS`、路由测试 | §2；`crates/ae-sdd-integrations/src/jobs/misc.rs`（classify）、`crates/ae-sdd-flow/src/route.rs` | v3.14/实现中 |
| D-003 | Work Item state / Nested State | 多任务、跨 session、Story 归入和恢复时状态互相覆盖或丢失；逻辑 Story ID 无法指向项目正式文件名；恶意 Work Item token 可能逃逸隔离根 | 每个 Work Item 独立 state，支持 PRD/DR/Story 嵌套、UUID、revision、恢复和 StoryName/docPath 指针绑定；token 与 resolved path 双重 containment | 避免重复执行、跨任务污染和根外 state/sidecar 写入，让 LLM 可从 state 直接恢复并稳定定位正式 Story 正文 | state store、nested state、Story binding/state transition、path escape tests | §3；`crates/ae-sdd-store`（`authority.rs`/`lease.rs`/`journal.rs`）、`crates/ae-sdd-domain/src/path.rs` | v3.3~v3.10.1/v3.11.3/已实现 |
| D-004 | 多 Agent 编排 | 并行任务缺少角色边界、报告格式和故障升级规则 | 角色库、结构化派活卡、reviewer tier、activeAgents/agentReports | 降低重复劳动和自审偏差，提升并行可见性 | agent state 字段、G-09 session 独立性、编排 SKILL | §4；`agent-orchestration-skill.md` | v3.2.6/v3.11.0/部分软约束 |
| D-005 | G-XX 门禁体系 | 仅靠 LLM 自律无法稳定阻止越级、假测试和文档漂移；同一 Story/Work Item 在不同门禁可能被不同路径规则解析，RA-like 参考资料还可能误锁当前流程 | 软约束、硬门禁、扫描器三层防线，统一 GATE_REGISTRY；Plan 门禁按完整/微任务 profile 校验；G-02/G-14 共用 Story resolver；G-RA-1~6/FLOW 共用 state `raDocPath` 优先、latest formal fallback 的单一 RA resolver | 把高风险错误提前阻断，减少错误进入后续阶段的返工，同时避免微任务、正式 StoryName 和无关 RA-like 文档被错误规则误拦 | `gates check`、GATE_REGISTRY、完整/微任务 CodingPlan tests、G-02/G-14 StoryName tests、`TestGRAUnifiedSelection`、G-09/UC checks | §5；`crates/ae-sdd-gates`（`registry.rs`/`evaluator.rs`/`scheduler.rs`） | v3.2~v3.9.1/v3.11.4/已实现 |
| D-006 | Review Batch 与增量验证 | 每次变更都全量重跑，成本高；历史证据容易被覆盖 | review batch、baseline、changedPaths、verification plan、evidence fingerprint | 缩短验证时间，减少无效重跑，同时保持证据可追溯 | `verification.plan`、evidence active/superseded、focused tests | §5 Review Batch；`ae-sdd-execution/src/plan.rs`、`ae-sdd-contracts/src/evidence.rs` | v3.10.1/v3.11.0/已实现 |
| D-007 | Typed operation + state lease | LLM 不知道调用什么、参数怎么写、哪些操作需要锁；并发写会覆盖 state，失败重试会重复推进 | `ops describe/next/execute`、JSON Schema、lease/fencing/revision/idempotency、`nextActions`、短事务锁 | 减少 LLM 理解成本、上下文读取、命令猜测、冲突和失败重试；把自然语言契约变成机器契约 | `ops describe --json`、UC-19、StateStore/operation/concurrency tests、CLI latency baseline（待持续补充） | §5 Review Batch 表、`operation-protocol.md`、`ae-sdd-operations/src/registry.rs` | v3.11.0/v3.11.0/已实现 |
| D-008 | 项目资产七层索引 | LLM 每次从工程源码重新寻找约束、技术栈、模块和接口，容易遗漏上下文 | project assets 分层、生成/审计、倒排/BM25 查询、G-00 | 缩短上下文检索，提升首次回答和路由准确度 | `assets generate/check/query/stats`、G-00、asset index tests | §6；`ae-sdd-resources/src/assets.rs`、`ae-sdd-inventory` | v3.2.4~v3.5.1/v3.11.0/已实现 |
| D-009 | Document Storage | Story/Work Item/constraints/assets/只读模板资源路径分散，正式 StoryName 与逻辑 ID 不同或项目覆盖存在时，LLM 和工具容易读错、猜错或重复读取 | intent-based resolve、Work Item-first、精确 StoryName 绑定；只读资源按项目覆盖→内置回退统一返回 `path/source/content/sha256`；save/finalize 拒绝只读 intent | 减少路径猜测、跨类别假命中、读取竞态和文档错写，让调用 Skill 消费同一权威正文与指纹 | `doc resolve/save/finalize`、`resolve_read_resource`、G-DOC-STORAGE、Story/resource resolver tests | §7；`ae-sdd-resources`（`document.rs`/`resolver.rs`）、`document-storage-skill.md` | v3.7.2/v3.12.1/已实现 |
| D-010 | 四层实例化与分发 | source、dist、用户安装、项目实例互相漂移，修复或共享 scanner 依赖无法传递 | Layer 1 source -> Layer 2 dist -> Layer 3 install -> Layer 4 init/override；新增 scanner 必须注册进 `ae-sdd-scanners::registry` | 降低升级和环境差异造成的故障，保证 LLM 使用同一版本契约，并避免 dist scanner 因漏 helper 无法启动 | `ae-sdd-build` 打包测试、install/init、UC-15/runtime verify | §8；`ae-sdd-build`（`post_commit.rs`/`distributor_registry.rs`） | v3.4.1/v3.11.4/已实现 |
| D-011 | Harness 适配层 | 不同 Agent 运行时需要不同入口，手工转译易造成版本和模板漂移 | adapter lock、source hash、tree hash、生成/回滚/备份轮转 | 减少接入和升级的人工操作，避免安装产物陈旧 | harness build、adapter lock tests、iteration-check | §9；`.harness/`、`ae-sdd-build/src/harness_build.rs` | v3.5.6/v3.11.0/已实现 |
| D-012 | Memory 生命周期 | 长会话上下文过大，compact 后关键业务事实丢失或 scope 混用 | 实体树、boot/context/pending compact、manifest hash、生命周期 CLI | 缩短默认上下文，提升跨 session 恢复和业务事实复用 | `memory create/read/update/search/summarize`、memory tests | §10；`ae-sdd-artifacts/src/memory.rs`、`ae-sdd-context`（`projection.rs`/`compact.rs`） | v3.8.2~v3.10.3/v3.11.0/已实现 |
| D-013 | Plan-First 编排 | LLM 直接编码会漏需求、测试、约束和回滚路径；统一大型 Plan 模板又会误拦微重构 | 编码前始终有 CodingPlan 和用户确认；完整计划走 14 门禁，微任务走范围/实现顺序/风险回滚/验证四维轻量门禁 | 把实现前的未知风险显式化，减少中途返工，并让门禁成本与任务规模匹配 | G-07/G-08、G-CODEPLAN-SRC、G-14、Work Item scope 与 micro/full profile tests | §11；`coding-process-skill.md`、`ae-sdd-gates` | v3.7.3/v3.11.2/已实现 |
| D-014 | 真实性扫描与对齐审计 | 文档可以宣称有门禁/测试/命令，但代码实际没有或已失效；全仓 RA rglob 会把参考资料、模板、事件附件和 dist 副本当作当前需求 | test/coding/RA scanners + UC-08~13 alignment audit + iteration-check；RA scanner 共享 `ae-sdd-scanners/src/scope.rs`，默认 root 审计只枚举 formal RA，Work Item 门禁显式传 repeatable `--file` | 减少“文档看似完整但执行无效”的假闭环，避免 Python/JS/TS work item 被误判为无生产 scope，并消除无关 RA-like 文档造成的 false blocker | UC-08~13、G-09、G-CODE-1、`test_ra_scan_scope.py`、RA scans、文本代码 scope regression | §12；`crates/ae-sdd-scanners`（`registry.rs`/`scope.rs`）、`crates/ae-sdd-integrations/src/jobs/diagnostics/` | v3.5.11~v3.6/v3.11.4/**部分**：7 个 scanner 与 IC-1~4 完整；UC-08~13 alignment audit 随 Python 删除后无承接实现 |
| D-015 | 统一 CLI 工具链 | 工具分散且输出不统一，LLM 需要记忆多个脚本和输出格式；存量 state 缺少正式 StoryName 的可审计迁移入口；批处理若不检查中间退出码会把前序失败覆盖成成功 | `ae-sdd` 统一入口、UTF-8 JSON/stdout/stderr、稳定 exit code；提供 `state new --story-name`、幂等 `state bind-story-doc` 和四个支持 repeatable `--file` 的 RA scanner 子命令；PowerShell 编排逐命令检查 `$LASTEXITCODE` | 减少命令搜索、编码差异、解析、迁移和批处理误判成本，便于 LLM/脚本安全组合调用 | `ae-sdd --help`、Story binding CLI tests、`test_invalid_ra_prerequisite_exits_one_without_writing_or_deleting_draft`、RA CLI forwarding、GBK regression、update-check | §13；`bins/ae-sdd-cli` | 历史版本（精确版本待补）/v3.11.4/已实现（v4 起为薄 client，业务在 daemon） |
| D-016 | State events 操作日志 | 只有当前 state 时无法解释谁在何时推进、恢复或覆盖 | append-only events、seq、txn/node 过滤、兼容旧 state | 提升审计和故障定位效率，减少 LLM 盲目重试 | events read/filter tests、state JSON | §14；`crates/ae-sdd-store/src/journal.rs`、`crates/ae-sdd-runtime/src/flow_supervisor.rs` | v3.4.2/v3.11.0/已实现 |
| D-017 | 三层 Hook 拦截 | 仅在命令执行后发现越级已经太晚，LLM 可能先写错产物或源码 | UserPromptSubmit、PreToolUse、Stop/监控协同；turn-scoped activity token 只在显式 ae-sdd turn 激活 | 将错误拦截前移，减少非法写入和后续修复，同时避免普通 Story 文档检查误触发门禁 | hook tests、gate_intercept、stop_check、harness | §15；`.harness/`、`ae-sdd-policy/src/hook.rs` | v3.4/v3.11.0/v3.11.4/已实现 |
| D-018 | 主流程监管器 | state、gate、memory、产物和 hook 各自工作，缺少统一运行态视图 | monitor/orchestrator 聚合 phase、activity、work item 和事件 | 降低 LLM/维护者判断当前流程位置的成本 | monitor state projection、runtime stats、监控测试 | §16；`ae-sdd-monitor` 相关设计 | v3.6/v3.11.0/已实现 |
| D-019 | 流程偏移检测与矫正 | AI 或人工可能跳步、回退或修改错误 Work Item，状态表面仍可继续 | 偏移规则、矫正计数、paused/人工升级、scope 复核 | 尽早发现流程漂移，避免错误积累到交付阶段 | iteration-check、state correction、flow violation scan | §17；`ae-sdd-integrations/src/jobs/diagnostics/iteration.rs`、`ae-sdd-flow` | v3.6/v3.11.0/部分 report-only |
| D-020 | SKILL 编译与 Runtime IR | 完整 SKILL 过长，默认加载浪费上下文；source/dist/runtime 容易不一致 | source slimming、fallback、compact boot/core/outline、manifest fingerprint | 缩短 LLM 默认上下文，保持按需加载和可验证分发 | SKILL 编译与 verify、UC-15、runtime verify | §18；`ae-sdd-methodology`（`compiler.rs`/`verifier.rs`） | v3.8/v3.11.0/已实现 |
| D-021 | 自动化模式 | 逐个等待人工确认耗时，但直接自动推进又会失去独立审查 | 默认关闭、Tier 3 多 reviewer、reviewConsensus、G-AUTO-CONSENSUS | 在明确开关下减少等待，同时保留高风险审查 | automation CLI、G-AUTO-CONSENSUS、UC-16 | §19；`ae-sdd-runtime/src/config.rs`、`ae-sdd-flow`、`ae-sdd-gates` | v3.8.0/v3.11.0/已实现 |
| D-022 | ae-sdd Monitor | 多工作区、active task、memory、runtime stats 分散在文件中，定位异常慢 | 只读 workspace 扫描、项目/任务双层视图、响应式刷新 | 降低人工监控和 LLM 状态查询成本，不改变权威 state | `apps/ae-sdd-monitor` tests、UG-22、只读边界 | §20；`source/docs/ae-sdd-monitor-design.md` | v3.7.0/v3.11.0/已实现 |
| D-023 | Sonar Issue 修复收尾 | CodeReview 发现的问题容易重复处理、越界修改或缺少 exactly-once 证据 | issue registry、TextEdit/provenance、compile/test/rescan 闭环 | 减少重复修复和错误补丁，提升 review 收尾效率 | `test_sonar_issue_fix_skill.py`、规则 registry、CodeReview evidence | §21；`sonar-issue-fix-skill.md`、`sonar-issue-fix-rules.md` | v3.11.0/v3.11.0/已实现 |
| D-024 | Design Ledger 治理 | 设计动机、价值假设和迭代影响容易再次分散或漏记，台账本身也可能失去维护 | §0 台账 + CHANGELOG `Design ledger impact` + UG-28/UC-20 fail-closed | 降低后续 LLM 重新理解和维护者追溯成本，让设计价值记录成为可检查资产 | `UC-20`、`update-check 20/20`、台账字段/章节/版本反例测试 | §0；`ae-sdd-integrations/src/jobs/diagnostics/update.rs`、`ae-sdd-update`、CHANGELOG 模板 | v3.11.1/v3.11.1/已实现 |
| D-025 | 风险驱动的有界测试策略 | 全矩阵、逐字段边界、最少用例公式和证伪比例会制造低价值测试，且没有停止条件 | 先建有限风险登记，再按边界准入、行为等价类、最低充分层级和局部数量上限选择；停止后扩展必须走预算例外 | 减少无独立缺陷发现价值的测试执行与维护成本，同时保护显式契约和高影响风险 | `test_bounded_test_strategy.py`、TC-G11/TC-10、后续项目 TestCase 数量与缺陷发现率基线 | §22；`be-testcase-strategy.md`、TestCase generate/review/template、CodingModel | v3.11.5/v3.11.5/已实现，收益待补基线 |
| D-026 | 真实 HTTP 双阶段接口验收 | MockMvc 或 `RANDOM_PORT + @MockBean` 只验证模拟边界，且仅本地结果无法证明部署后的接口可用 | Story/plan 声明 HTTP AC；G-08/G-14 校验双阶段和边界；scanner 阻断 MockMvc/内部 mock；G-09 要求同 buildId 的 `http-local` 与 `http-test-env` immutable evidence | 防止 mock 测试和未部署结果冒充接口完成，把本地实现与测试环境部署纳入同一验收事实链 | HTTP 验收 policy 测试、G-08/G-14/G-09、scanner/evidence tests | §12、§22；`ae-sdd-gates`、`ae-sdd-contracts/src/evidence.rs`、`test-authenticity` scanner | v3.11.7/v3.11.7/已实现 |
| D-027 | Story 模板章节分层与导航 | 语义清单无法稳定对应完整模板章节，章节增删、重命名或改层级会迫使三个 Story Skill 同步硬编码列表；长 Story 和多接口契约还需要稳定跳转与清晰分组 | 模板以稳定 section ID 和 `primary/secondary` 元数据声明唯一边界；独立撰写指南按 ID 定义 SOP；Document Storage 返回正文/指纹；纯函数动态取章节和校验导航；生成 Story 保留显式锚点与 ID-only 标记；章节按分析→设计→实现排序，核心与补充区隔离 | 模板或层级变化无需改 Skill，标题重命名不破坏 Review/Update；目录无断链；接口块可独立定位；正式 Story 省略不适用章节并降低认知负担 | `test_story_template_sections.py`、`test_story_content_layering.py`、Document Storage CLI tests、slim/runtime 验证 | §23；Story 模板/指南、`ae-sdd-resources` 的 section 解析、Story 三件套 | v3.12/v3.12.1/已实现 |

| D-028 | 能力驱动的测试场景推导 | 固定 CRUD 示例和大量 Mock 单测不能根据接口语义发现真实缺陷，真实 HTTP 也可能只有状态码断言 | 从能力、状态、独立观察面、不变量、扰动和失败机制推导最小场景；G-HTTP-1 校验 manifest，G-09 对账 scenario evidence | 让测试计划随接口语义变化并能解释检错价值，删除无独立失败机制的肤浅测试 | `test_scenario_derivation.py`、`test_http_scenario_contract.py`、`test_scenario_effectiveness.py` | §22；场景推导与 HTTP manifest 校验（G-HTTP-1 路径）、`be-http-scenario-strategy.md` | v3.12/已实现 |

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

- Phase 1 设计阶段：路由 → 需求分析(RA) → 按需 DR/Story/CodingPlan → Story Review/验证矩阵 → 人工审核点 1
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
| PRD 级 §1.7 强制顺序 | `workitem.complete` 的 PRD 前置检查必须先于 `ae-sdd state prd-complete --prd <ID>` |
| 4 层 AND 校验 | `crates/ae-sdd-lifecycle/src/validation.rs` 的 4 层 AND 校验（G-PRD-1~4） |
| 审核点用户确认 token | `ae-sdd state confirm --phase <审核点>`（防 AI 自填，经 typed operation 写入） |

**颗粒度与边界**：流程节点粒度；Phase 1 完成确认、CodingPlan 审核等关键节点锁定后不允许重跑；不可跳过任何节点，微任务走快速通道也须经路由判定后才豁免；自检不替代人工审核，不得跳过自检直接收尾。🆕 2026-07-03(B1)：明确"快速通道"仅指 `/ae-sdd-quick` escape hatch（豁免 G-00/路由判定维度，详见 conventions §3.1），**不等于 scale=微 的子链豁免**；微链（scale=微）同样必须走 code-reviewed 节点出 CodeReview 报告（conventions §3.1 质量底线：Plan-first/CodeReview 报告/state.json 更新 ❌不豁免）。

---

## 2. 智能路由（4 类需求 + 4 维判定）

### 设计

统一入口层，对所有用户输入做需求分类后路由到对应 SKILL，两套路由机制并存互补：4 维判定（来源 × 规模 × 现有产物 × 项目类型）优先，分类不明时 fallback 到 4 类需求传统路径（套 Story 7 区模板判定规模）。

路由决策算法 7 步：工作区检查(0) → 自更新识别(1.5，短路到 update-skill) → 来源识别(1.6) → 规模识别(1.7) → G-RA 准入门禁(1.8) → 关键词匹配(2) → 加载执行(3-5)。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 分类算法核心 | `crates/ae-sdd-integrations/src/jobs/misc.rs` 的 classify job，综合标题信号/文件名信号/关键词匹配/规模推断 |
| 规模推断（4 维判定） | classify job 的行数与项目上下文规模推断 |
| CLI 入口 | `ae-sdd classify --text "..."`（classify job entrypoint） |
| 合法规模枚举 | `crates/ae-sdd-domain` 的 `WorkScale` 枚举（大/中/小/微） |
| 路由到子链 | `crates/ae-sdd-policy/src/transition.rs` 按 scale + DesignRoute 选链（见 §3 表格） |

**颗粒度与边界**：路由到节点级（requirement-analysis / dr-generate / story-generate / task-generate / coding），不进行节点内子步骤路由；路由决策权归 ae-sdd 流程与用户，不归 AI 临时直觉。

---

## 3. 流程状态持久化（state.json）

### 设计

每个新需求必须先创建一个独立状态机，再进入 phase 流转。WorkItem（PRD / BUG / OPT / Story 均可）的执行进度持久化到独立 `state.json`，支持跨 session 中断恢复与多任务并行执行。AI 重入时读对应 WorkItem state 跳过已完成步骤，避免重复执行。

**WorkItem 标识约定（2026-07-09 修正，🆕 v3.10.1 UUID 前缀）**：每个状态机必须归属于真实顶层工作项。创建入口为 `operation.execute workitem.create`（workspace-scoped，`requiresWorkItem=false`，`requiresIdempotency=true`，`writes=true`；`entryNode` 必填且仅 PRD/DR/STORY，BUG/CONFIG 走扁平微链被拒绝；调用方可省略 `workItemId`，由 daemon 铸造 `{entryNode}-{8 位小写 hex}` 业务键，如 `STORY-3f9a2c1e`）；物理目录名采用 R6 顶层名加随机 UUID 前缀（如 `{uuid}-PRD-001` / `{uuid}-DR-005` / `{uuid}-Story-006`），保证同业务名不撞目录；`stateMachineId` 同目录名（带 UUID 前缀），`stateMachineName` 存纯业务名（如 `PRD-001`）供按业务名查找匹配，`stateUuid` 存 UUID 冗余标识。`--work-item <ID|WORKITEM-KEY>` 定位 `.auto-engineering/{WORKITEM-KEY}/state.json`，`find_work_item_state_path` 支持后缀匹配（传业务名 `PRD-001` 可命中 `{uuid}-PRD-001` 目录）。项目级 `.ae-sdd/state.json` 不允许作为 active state、mirror 或 fallback；未能唯一定位 work-item 时必须拒绝并要求显式选择。

**v3.14 多子链状态机**：4 条 PHASE_FLOWS 统一从 `route-selected`、`requirement-analyzed` 进入；`routeDecision.selectedDesign` 决定后续 DR/Story/CodingPlan 分支。旧 state 无新字段时按旧 phase 兼容读取。

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

**R7 路由自动匹配/新建**（⚠️ 已被取代：bootstrap 创建改由 daemon 侧 `workitem.create` 完成——调用方可省略 `workItemId`，daemon 铸造 `{entryNode}-{8 位小写 hex}` 业务键并持久绑定 session，不再由 Python 时代的 classify 自动匹配/新建）：`/ae-sdd` 路由时 `classify.match_state()` 自动分析需求特征（提取 PRD/DR/Story ID + 判定 Bug/改 Story）→ 扫描现有嵌套 state → 命中则 relocate/absorb，未命中则 create_nested。匹配优先级：R4 微任务 → R5 Story 命中 → R2 DR 归入 PRD → R2 Story 归入 DR → R7 新建。

**v1 扁平 state 完全兼容**：旧 workitem（`stateModel` 缺省或 `"flat"`）保留可读，所有读取点通过 `state.is_nested_state()` 分流。旧 workitem 不迁移、不动。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 存储路径 | `.auto-engineering/{WORKITEM-KEY}/state.json`（🆕 v3.10.1 WORKITEM-KEY 带 UUID 前缀如 `{uuid}-PRD-001`）。`.ae-sdd/state.json` 禁止作为状态源、active mirror 或 fallback；hook/gate/CLI 均必须通过 work-item/session resolver 定位任务级 state |
| 4 条子链定义 | `crates/ae-sdd-policy/src/transition.rs` 的 phase 链常量，大链 11 phase / 中链 10 / 小链 8 / 微链 8（🆕 2026-07-03 B1：微链加回 code-reviewed，与"CodeReview 报告不豁免"对齐；含 TestCase 系列后已扩容，🆕 v3.7.x） |
| 向后兼容别名 | `PHASE_FLOW = PHASE_FLOWS["大"]`（行95，🟡 deprecated） |
| 合法 scale 枚举 | `VALID_SCALES = ("大", "中", "小", "微")`（行88） |
| phase 允许工具集 | `crates/ae-sdd-policy/src/hook.rs` 的 phase 工具集裁决 |
| 暂停/恢复 API | `ae-sdd-flow/src/control.rs` 的 pause/resume |
| 矫正计数 API | `ae-sdd-flow/src/control.rs` 的矫正计数 |
| 多 Agent 状态字段 API | `ae-sdd-delegation/src/aggregate.rs`：delegation 记录与结果归档（见 §4） |
| memory 生命周期强制校验 | `crates/ae-sdd-artifacts/src/memory.rs`，phase 切换前由 transition 校验调用 |
| 终态投影不变量 | 写生命周期 phase 时由 `ae-sdd-lifecycle/src/projection.rs` 同步 `currentPhase/currentStep/completedSteps/pendingOutputs/codingRound`；落盘前拒绝 `phase=completed` 但投影仍处于中间态的 state |
| PRD 4 层 AND 校验 | `ae-sdd-lifecycle/src/validation.rs` 的 4 层 AND 校验 |
| CLI 入口 | 新建：`operation.execute workitem.create`（见上文设计；daemon 铸造业务键并绑定 session）；读写流转：`ae-sdd state read / write / next-step / confirm / prd-init / prd-check-complete / prd-complete / prd-archive`（typed lifecycle operations） |
| Story 正文绑定 | `ae-sdd-lifecycle` 的 Story binding（经 typed operation 写入）；嵌套 state 写 `storyStates[storyId].storyName/docPath`，扁平 state 写 `storyName/storyDocPath` |
| 🆕 v3.9.0 嵌套 state schema | `ae-sdd-lifecycle`（`engine.rs`/`projection.rs`）的嵌套 state 初始化、Story 子态重置与 active phase/story 投影 |
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
| 多 Agent 状态可见性（唯一落地的代码支撑） | `crates/ae-sdd-delegation/src/aggregate.rs`：delegation 记录与 bounded `ChildResult` 汇聚 |
| test-verifier 独立性约束 | ⑥.10 测试真实性硬门禁 G-09 要求报告带独立 `session_id`（`ae-sdd-gates` 的 G-09 check），是唯一有 CLI 侧校验的角色约束 |
| 子系列 Agent 与注册器协议 | `source/docs/agent-registry-protocol.md`：5 子系列 Agent（ra/dr/story/coding/test）预创建模板 + 分发注册协议（ClaudeCode `.claude/agents/` / Codex / ZCode 适配）；角色库 SSOT 见 `agent-orchestration-skill.full.md` §3.1（已按子系列重组）|

**颗粒度与边界**：节点内子任务级并行；降级为逻辑多视角时必须在报告头部标注 `reviewerMode: "logical-multi-perspective"`；root agent 保留最终冲突裁决权；除 activeAgents/agentReports 状态记录和 test-verifier session_id 校验外，其余编排规则完全依赖 AI 自律，无物理拦截。

---

## 5. 门禁体系（G-XX 三种强度）

### 设计

三种强度形成纵深防御：软约束依赖 LLM 自律，硬门禁通过 CLI 阻断，扫描器兜底覆盖硬门禁无法检测的场景。

- **软约束**：SKILL 文本说明，LLM 自律执行
- **硬门禁**：`GATE_REGISTRY` 注册，CLI `ae-sdd gates check` 阻断，不通过无法继续
- **检查器**：`ae-sdd-scanners` 的 7 个扫描器，兜底补硬门禁覆盖不到的场景

G-00 项目资产门卫每次 SKILL 启动前验证资产存在；G-RA 系列在需求分析阶段把关；G-CODE-1 在 Coding 完成/CodeReview 前扫描反模式；中段门禁（G-14/G-CODEPLAN-SRC/G-DOC-STORAGE）补"两头强中间空"；入口关卡三道闸（entry token / 产物落地凭证 / 代码改动准入）管住流程入口。

CodingPlan 门禁按 state `scale` 选择 profile。大/中/小任务保持 7 章节、14 关键词、Story/AC 对齐和源码核对；微任务从 Work Item 分桶读取 Plan，只要求变更范围、实现顺序、风险/回滚、验证四维完整且无未闭环项。Standalone 微任务没有 Story 时，G-14 返回带 `alignmentMode=standalone-micro` 的可审计 N/A；一旦存在父 Story，仍执行严格 Story/AC 对齐。

G-02 与 G-14 通过 `document_storage.resolve_story_document()` 共享 Story 正文解析。已绑定 `docPath` 优先，随后是精确非 glob StoryName，只有没有 StoryName 时才兼容 Story 类别的 `{STORY-ID}.md`；Task/Coding/Test/CR 同名文件不参与。正式文件必须由正文 `Story ID` 元数据反校验。多候选、元数据缺失/漂移和非法 basename 全部 fail closed，不做模糊猜测。

G-RA-1~6 与 G-RA-FLOW-VIOLATION 通过 `_resolve_selected_ra()` 共享当前 Work Item 的 RA 正文。合法 `state.raDocPath` 或 `storyStates[activeStory].raDocPath` 优先；没有 binding 时，只在 formal RA candidates 中沿用统一的 latest version/mtime fallback。Work Item 门禁把解析出的单一文件作为 `--file` 传给 scanner，`--root` 仅用于相对路径和 containment；因此 `references/`、templates、CHANGELOG、`dist/`、GeneratePlan/Impact/ReverseIssues 等 RA-like 文档既不会锁住当前流程，也不能替代 selected RA 自身的真实性、深度、流程或实现完整性检查。根目录全量审计仍保留，但使用同一 formal candidate 分类器，不再使用各 scanner 独立的宽泛 `rglob`。

🆕 v3.9.1 上下文加载准入门禁（G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX）补齐 DR/Story/TestCase/Task 四组的"第零步准入检查"——此前这四组只有 prose 清单（dr-review/task-generate 还官方自认 report-only），AI 可不读 PRD/DR/项目资产/约束就过门禁切相。采用**注册表模式**：一个 `_check_context_loaded` 函数 + `CONTEXT_GATE_REGISTRY` 注册表服务 4 个门禁，流程一致用单函数封装，上下文差异（DR 查 RA，直接 Story 查 RA、DR 分支查 DR，TestCase 查 Story，Task 查 Story+TestCase）走注册表 `required` 字段；读文件统一走 `document-storage-skill` 的 `get_constraints/get_assets` API。微链 G-TASK-CTX 用 `required_micro` 豁免 Story/TestCase。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 门禁注册表 | `crates/ae-sdd-gates/src/registry.rs:GateRegistry`（list，实际 36 个；包含 G-HTTP-1 场景推导门禁） |
| 统一评估入口 | `gate.evaluate` RPC → `crates/ae-sdd-gates/src/evaluator.rs` |
| 单个门禁定向检查 | 同一 RPC 指定 gate id；重 Gate 经 `scheduler.rs` 有界排队 |
| G-00 资产门卫 | `ae-sdd-gates` G-00 check；不通过时用 assets 相关 operation 生成/修复 baseline 资产再校验 |
| G-RA-1~4 需求分析门卫 | `ae-sdd-gates` + `ae-sdd-scanners` 的 `ra-authenticity` scanner（G-RA-4） |
| G-RA-5 机械派生深度 | `ae-sdd-gates` + `ra-depth` scanner |
| G-RA-6 实现视角完整性 | `ae-sdd-gates` + `ra-implementation` scanner（I1~I7） |
| G-RA-FLOW-VIOLATION | `ae-sdd-gates` + `ra-flow-violation` scanner（R1~R3 规则） |
| G-RA authoritative scope | `crates/ae-sdd-scanners/src/scope.rs` 单一 resolver；G-RA-1~6/FLOW details 统一报告 `selected_file/selection_source/scope_mode`，四个 RA scanner 共享同一 scope 判定 |
| G-CODE-1 Coding 真实性 | `ae-sdd-gates` + `coding-authenticity` scanner（AP-1~AP-6 反模式） |
| G-09 测试真实性 | `ae-sdd-gates` + `test-authenticity` scanner（8 类禁止手段）；可信 work-item scope 优先取 `VerificationPlan.changedPaths`，兼容 state changed paths，无 scope 时全仓严格扫描 |
| G-13 全链路对称性 | `entryNode=STORY` 且 `scale=中` 时仅豁免 DR 依赖并显式返回 exemption；其他入口仍要求 DR，Story/Task/CodingReport/CodeReview 下游链按阶段保持严格 |
| G-07 / G-08 / G-14 / G-CODEPLAN-SRC | `ae-sdd-gates` + `crates/ae-sdd-resources/src/resolver.rs` 按 Work Item-first、唯一 Story fallback 定位；完整 Plan 保持 14 门禁，微任务使用四维轻量 profile；无类骨架时 G-CODEPLAN-SRC 按既有规则跳过 |
| G-02 / G-14 Story 正文 | `ae-sdd-gates` + `ae-sdd-resources` 的 Story resolver；绑定路径/StoryName 校验失败时返回稳定错误码和恢复动作 |
| G-DOC-STORAGE / G-DOC-CONSISTENCY / G-PATH | `ae-sdd-gates` 对应 check；G-PATH 仅豁免 canonical document-storage source entry 及其 source/runtime full fallback，其他同名或 `SKILL.full.md` 文件仍受扫描 |
| G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX | `ae-sdd-gates/src/registry.rs` 四条注册项共用同一 context-loaded 判定，避免每门禁重写 scale 豁免与 phase 感知；各 phase 入口由 `crates/ae-sdd-policy/src/hook.rs` 挂载 |
| G-REVIEW-LOOP | `ae-sdd-gates` + `crates/ae-sdd-review/src/supervisor.rs` |
| 入口关卡1（session 绑定） | `session.open`；每个宿主 Hook 事件由 `bins/ae-sdd-cli/src/main.rs` 的 `bind_host_session` 从 `sessionId` + `cwd` 幂等重建绑定（Hook 是独立子进程，无法继承 SessionStart 的绑定） |
| 入口关卡2（产物落地凭证） | `crates/ae-sdd-policy/src/hook.rs` 的 PreToolUse 裁决，产物路径 + phase 映射校验 |
| 入口关卡3（代码改动准入） | 同上 HookGuard：非 coding phase 或无审核点 2.5 确认 → 禁写 src/ |
| 设计-实现对齐反查 | 原 `alignment_audit.py`（UC-08~UC-13）随 Python 树删除，**当前无承接实现**；`ae-sdd update-check` 仅 UC-01 为真实检查，见实现架构 §11 |

**G-PATH 项目侧输入边界（v3.10.2）**：项目侧扫描只读取声明为记忆/约束输入的 `.ae-sdd/memory/**/*.md`、顶层 `AGENTS.md`/`CLAUDE.md`/`MEMORY.md` 和 `.harness/memory/**/*.md`。`.ae-sdd/drafts/**/*.md` 是 Review/生成过程产物，不是 canonical 正文或项目记忆，不进入 G-PATH；其流程真实性仍由对应 Review/上下文门禁负责。`current_story` 不用于静默过滤路径，避免通过伪造 Story ID 绕过项目级记忆扫描。

**颗粒度与边界**：G-00 每次 SKILL 调用前必跑；G-RA 豁免场景（微任务、BUG/配置类、重入已完成步骤）；改门禁强度必须改 `ae-sdd-gates/src/registry.rs`，不能只改 SKILL.md 文字；`ae-sdd update-check` 自动检测文档-实现一致性，防文档撒谎复发。

### Review Batch v2 与增量验证

Review 的质量对象是带 `inputFingerprint` / `rulesetFingerprint` 的 `reviewSession`，不是模糊的 round 计数器。每次尝试落入 `VALID_CLEAN`、`VALID_FINDINGS`、`INVALID_INFRA`、`INVALID_PROTOCOL`、`INVALID_INPUT_DRIFT` 或 `CANCELLED`；平台失败不增加 clean streak，输入漂移必须新建 session。`cleanTarget` 恒为 1：任何 Tier、任何 repair class 都是一批 `VALID_CLEAN` 即通过，风险改由 required reviewer 集完整性与 `finalProofRequirement`（Tier 2 deterministic gates、Tier 3 增量最终验证，全量套件仅 release/分发门禁执行）承担；attempt、valid batch、remediation 和 wall-clock 任一预算耗尽进入 `STALLED`，不得当作通过。

G-CODE-1 在可信 `VerificationPlan.changedPaths` 与 G-09 evidence/hash 链存在时，仅检查当前 work-item 的生产代码；evidence 必须绑定 Story、scope、command、toolchain 与 report，artifact 只能是项目内相对路径。production scope 与 scanner 共同识别 Java/Kotlin/XML/YAML/properties 及 `.py/.js/.ts` 文本代码，并共同排除生成目录、`.venv/venv/.tox/site-packages`、`__tests__` 及 Python/JS/TS 常规测试命名。scanner 必须用安全、唯一的 `scannedPaths` 证明完整覆盖 production scope，并保证 root、exit/status、finding severity/path、顶层计数与 `reportStats` 自洽；任一缺失、越界、未知 schema 或计数漂移均 fail closed。self-hosting 例外只限 scanner 自身 AST `LINE_RULES` 与三个 metadata 常量赋值行；真实 pom metadata 由 XML 解析验证，普通业务文件使用同 URI 仍是 blocker。scope 内 blocker（含触及历史债）阻断，scope 外历史债不阻断；scope 缺失或为空时保持全仓严格扫描，测试/文档-only scope 阻断。scoped 路径不读取或创建 baseline；全仓模式的显式 baseline 仍必须经用户批准并校验完整性。

验证动作先生成 `VerificationPlan`，再按生产代码、测试代码、配置、文档分类决定最小验证集。计划分别暴露 `planFingerprint` 与 `evidenceInputFingerprint`，Evidence 命令只能使用后者。G-09 只接受 plan 指纹匹配、路径未越界且真实存在的 work-item scope；scope 内 blocker（含触及的历史债）阻断，scope 外历史债不阻断，无可信 scope 则全仓扫描。成功证据写入 `.auto-engineering/<story>/evidence/manifest.json`；record 先复制内容寻址 immutable snapshot，同 logical key 的旧 active entry 标为 superseded，finalize/gate 只校验 active snapshot。复用必须同时满足 manifest 内容 hash、Story、input/command/toolchain fingerprint、退出码、freshness window 与 snapshot hash；evidence 不能定义 scope 或充当 waiver。implementation/documentation/review 三类 fingerprint 分离，文档或审查措辞变化不得使 Maven 证据失效。文档使用单一 canonical 正文，Work Item scope 优先于 Story 关系；旧 Story 路径只允许唯一候选 fallback，多个候选必须显式报 `SCOPE_AMBIGUOUS`，不按 mtime 猜测。

| 设计点 | 实现 |
| --- | --- |
| Review Batch 状态机 | `crates/ae-sdd-review`（`model.rs`/`fingerprint.rs`/`policy.rs`/`supervisor.rs`）；旧 `reviewLoop` 字段仅作兼容投影 |
| 增量 baseline | `crates/ae-sdd-integrations/src/jobs/baseline.rs` + G-CODE-1 delta 分支 |
| 变更感知验证 | `crates/ae-sdd-execution/src/plan.rs` + typed `verification.plan`；`evidenceInputFingerprint` 绑定 Story/work-item/changedPaths |
| 证据复用 | `crates/ae-sdd-contracts/src/evidence.rs` + typed `evidence.record/finalize`；active/superseded + immutable snapshot |
| canonical 文档 | `crates/ae-sdd-resources`（`document.rs`/`resolver.rs`）Work Item-first resolver + unique legacy Story fallback |
| LLM 操作协议 | `crates/ae-sdd-operations/src/registry.rs` + `source/standards/operation-protocol.md`；维护者变更入口由 `ae-sdd-update` 指向协议 §9，并由 UG-27/UC-19 约束 |

---

## 6. 项目资产体系（7 层索引）

### 设计

每个接入项目维护一份标准化资产文件，是所有 SKILL 的上下文基础，通过 ES 倒排索引支持按需读取。AI 读资产而非扫描全仓库，保证上下文质量与读取效率。

资产生成分两层：`ae-sdd init` 首次运行会自动生成可通过 G-00 的 baseline 资产；`ae-sdd assets generate/check` 提供可执行的生成/修复/校验入口。深度业务资产仍由 `project-assets-update-skill` 引导 AI 跑探查 SOP（读 CLAUDE.md/AGENTS.md → 扫描工程结构 → 抽典型类 → 识别分层/命名/约束）后增量完善；`assets update/audit` 仍未实现。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 资产文件路径 | `source/assets/{projectKey}/{projectKey}.assets.md`（7 层索引 §A-§G） |
| 生成/增量更新/审计引导 | `ae-sdd-resources/src/assets.rs` + assets operation 负责 baseline 生成/校验；`source/skills/cross-cutting/project-assets-update-skill.md` §3/§4/§5 负责深度业务资产更新/审计引导 |
| ES 倒排索引 + BM25 评分 | `crates/ae-sdd-inventory`（`inventory.rs`/`selector.rs`/`cache.rs`：outline/query/section/stats） |
| CLI 入口 | `ae-sdd assets generate / check / read / outline / section / query / stats`（assets typed operations） |
| G-00 门卫自动检查 | `ae-sdd-gates` 的 G-00 check，7 层索引缺任一层即阻断；距 lastAuditedAt > 30 天触发警告 |

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
| 路径解析 | `ae-sdd-resources/src/resolver.rs` |
| 原生 StoryName 解析 | `ae-sdd-resources` Story resolver；返回 path/source/candidates/rejected，正式文件校验 `Story ID` 元数据 |
| 文档保存（带版本号+ChangeLog） | `ae-sdd-resources/src/document.rs` 的 save |
| 文档定稿 | `ae-sdd-resources/src/document.rs` 的 finalize |
| 项目约束读取 | `ae-sdd-resources` 的 constraints 读取 |
| 项目资产列表读取 | `ae-sdd-resources/src/assets.rs` 的资产列表读取 |
| Story 只读资源 | `ae-sdd-resources/src/resolver.rs` 的只读资源解析；`STORY_TEMPLATE` / `STORY_WRITING_GUIDE` 返回 path/source/content/sha256，项目覆盖优先 |
| Git 路径/服务根路径 | `ae-sdd-resources/src/resolver.rs` 的 git/service 根解析 |
| 版本号推断 | `ae-sdd-resources/src/document.rs` 的版本推断与归一化 |
| ChangeLog 读取 | `ae-sdd-resources/src/document.rs` 的 ChangeLog 读取 |
| RA 前置条件校验 | `ae-sdd-gates` 的 RA 前置校验 |
| 存量文档迁移 | 存量迁移已随 Python 树删除，**无承接实现** |
| CLI 入口 | `ae-sdd doc save / resolve / finalize`（document typed operations） |

**已补齐**：thinking engine 解析已在 `ae-sdd-resources/src/resolver.rs` 实现，并收录到 `document-storage-skill.md` §4.2。编码流程引用该 API 时会优先读取项目/文档工作区覆盖版本，找不到时回退到 ae-sdd 自带 `standards/thinking/be-coding-thinking-engine.md`，返回 `path/source/content/sha256`。

**颗粒度与边界**：所有 ae-sdd 生成文档的读写必须走本层 API，不允许 SKILL 各自维护路径拼接逻辑。只读资源由本层一次性完成定位、UTF-8 读取和 sha256 计算；调用方消费返回正文，不得二次打开路径。只读 intent 不进入可写路径表，save/finalize 必须 fail closed。

---

## 8. 实例化体系（4 层架构）

### 设计

母版到项目落地的 4 层分发体系，保证 SSOT 的同时支持项目级 override。Layer 1 母版是唯一编辑点，经脚本构建为分发包，再由安装脚本装入用户环境；接入项目通过 Layer 4 实例做 override，不修改母版。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| Layer 1 母版（SSOT） | `source/`，开发者唯一编辑点，git 跟踪 |
| Layer 2 分发包构建 | `ae-sdd-build post-commit: compile` → `dist/ae-sdd/`（注入 VERSION + plugin.json，剥离 CHANGELOG/docs），git ignored |
| Layer 3 用户安装 | `ae-sdd-build post-commit: distribute` → `~/.claude/skills/ae-sdd/`，Claude Code 实际加载 |
| Layer 4 项目实例创建 | `ae-sdd init <dir> <key>`（init 路径），生成 `config.yaml/state.json/assets//overrides/` |
| Override 解析 | 项目 `overrides/` 优先于母版 defaults，由各 SKILL 读取时的路径解析规则实现 |
| 版本号同步 | `ae-sdd bump <ver>`（`ae-sdd-build bump`），同步 SKILL.md / README.md 与 crate 版本 |
| 一步到位开发流 | `cargo build --workspace` + `ae-sdd-build post-commit`（build + distribute） |

**颗粒度与边界**：Layer 2/3/4 不手工改；fork（完整复制）是显式 opt-in；override 优先级：项目 overrides/ > 母版 defaults。

---

## 9. Harness 适配层

### 设计

将 ae-sdd SKILL.md 自动转译为 Harness 格式的 agent.md，使 ae-sdd 能作为 Harness 团队级 agent 被编排。不需要手工维护两套定义，转译脚本从 SKILL.md + HARNESS.md 生成 agent.md，母版升级后重跑即可同步。

**⚠️ 文档滞后修正**：原文档写转译脚本是 `convert-ae-sdd-to-harness.ps1`——该文件已不存在。实际实现是 `crates/ae-sdd-build/src/harness_build.rs`，逐功能对齐原 PS1 版本（版本号 fallback / frontmatter 解析 / 多维幂等锁 / 模板渲染 / mount 失败回滚等）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 转译脚本 | `crates/ae-sdd-build/src/harness_build.rs`（PS1 → Python → Rust 迁移已完成） |
| 产物路径 | `.harness/agent.md` + `.harness/README.md` |
| 幂等锁 | `.harness/.adapter.lock`（JSON：`adapter_version/ae_sdd_version/source_input_sha256/source_commit/templateHash/converted_at`），由 `harness_build.rs` 读取 |
| 版本号三级 fallback | `harness_build.rs` 的版本推断（SKILL.md frontmatter → commit msg vX.Y.Z → git short hash） |
| tree-hash amend 检测 | `harness_build.rs` 的 tree-hash 比对（区分 amend 和真实内容变更） |
| SKILL frontmatter 解析 | `harness_build.rs` 的 frontmatter 解析 |
| 模板渲染 | `harness_build.rs` 的模板渲染（`{{VAR}}` 占位符替换） |
| harness CLI 探测与 mount | `harness_build.rs` 的 harness CLI 探测与执行，mount 失败自动回滚产物 |
| 备份轮转 | `harness_build.rs` 的备份轮转（保留最近 3 个 `.bak.<ts>`） |
| CLI 用法 | `ae-sdd-build harness`（dry-run/force/unmount/clean/no-mount 由参数控制） |

**颗粒度与边界**：禁止手工编辑 agent.md；母版（source/SKILL.md / source/HARNESS.md / harness 模板）升级后必须重跑 `ae-sdd-build harness` 重新生成；harness 格式由 Harness 规范决定，转译脚本负责映射；`.adapter.lock` 多维比对（source_input_sha256 + ae_sdd_version + adapter_version + templateHash）任一漂移触发重转，`source_commit` 只作诊断，不参与幂等判断，避免提交生成物后继续漂移。

---

## 10. 记忆层（🆕 v3.10.3 业务实体树 + 编译文档容器）

### 设计

memory 存储编译后的 compact 文档，按业务实体树平级分层（prd/dr/story/testcase/coding/common）。子流程Agent 首次进入时主动读源上下文（从 document-storage）-> 编译成 compact -> 写 memory；后续读上下文 = 读 memory；子流程结束删自己的 memory，common 保留。

**v3.10.3 核心变化**：废弃 5 层原文索引 + enter/exit 生命周期门禁，改为业务实体树 + 编译文档容器。memory 不再是"compact context index"（原文短索引），而是"编译后的工作上下文"（高密度 compact.md 文档）。

common 层只存项目级可复用约束（BigDecimal/幂等/禁大事务/架构规范），必须轻（`COMMON_MAX_CHARS = 2048` 字符硬限制），跨子流程保留。严禁存任何特定 PRD/DR/Story 的细节。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 业务实体树存储 | `ae-sdd-artifacts/src/memory.rs`：`memory/{entity_type}/{entity_id}/` 目录，每实体含 boot/context/pending 3 个 compact.md + manifest.json |
| 编译器 | `ae-sdd-context`（`projection.rs`/`compact.rs`）：读源上下文 -> 编译 3 个 compact slice + manifest |
| common 提取 | `ae-sdd-context/src/compact.rs` 的 common 提取：从源上下文提取项目级可复用约束，自动去重，大小限 2048 字符 |
| 生命周期 API | `ae-sdd-artifacts/src/memory.rs`：create（创建=编译）/ read（读 compact）/ update（增量更新 slice）/ clean（删单实体或全部，保留 common） |
| compact snapshot | `ae-sdd-context/src/compact.rs` 的 compact 前/后上下文保存与重载 |
| state->entity 映射 | `ae-sdd-artifacts/src/memory.rs` 的 phase → entity type 映射 |
| 存储格式 | compact.md（Markdown 表格/列表）+ manifest.json（source hash + slice hash + fingerprint） |
| CLI 入口 | `ae-sdd memory create/read/update/clean/clean-all/common/search/summarize` |
| memory_gate（废弃） | 已随 Python 树删除；memory 生命周期校验并入 transition 校验 |

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
| CodingPlan 定位 | `ae-sdd-gates` + `ae-sdd-resources/src/resolver.rs`，Work Item-first、唯一 Story fallback 语义 |
| 完整任务 14 门禁 | `ae-sdd-gates` 的完整 CodingPlan profile；大/中/小任务保持原严格检查 |
| 微任务四维门禁 | `ae-sdd-gates` 的 micro CodingPlan profile；标题支持中英文 alias，检查范围/实现顺序/风险回滚/验证和未闭环标记 |
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
| 测试真实性扫描（⑥.10/G-09硬门禁） | `test-authenticity` scanner：通用假测试规则 + `mock-http-boundary` + `http-internal-mock` + Surefire XML；由 G-09 消费 |
| Coding 真实性扫描（G-CODE-1） | `coding-authenticity` scanner：AP-1~AP-6 反模式库；由 G-CODE-1 触发 |
| RA 真实性扫描（G-RA-4） | `ra-authenticity` scanner：8 类禁止规则（vague-ellipsis/no-evidence/fabricated-field等） |
| RA 流程违规审计（G-RA-FLOW-VIOLATION） | `ra-flow-violation` scanner：R1~R3 规则（12维决策记录/8维度挖掘/缺口管理） |
| RA 机械派生深度（G-RA-5） | `ra-depth` scanner：验证每条规则 R 机械追问 6 问 → 衍生 R′ |
| RA 实现视角完整性（G-RA-6） | `ra-implementation` scanner：I1~I7 检查 |
| RA 扫描作用域 | `ae-sdd-scanners/src/scope.rs`：formal RA candidate 分类、explicit file containment、稳定排序、excluded reason 和结构化错误；四个 RA scanner 共享 |
| 外挂内容安全扫描（插件加载防护） | `plugin-content` scanner：PC-001~PC-010（危险删除/任意命令执行/远程脚本执行等），由 `ae-sdd-methodology` 的 plugin 加载路径调用 |
| 设计-实现对齐验证器（AA） | 原 Python 实现已删除，**当前无承接实现**。反向对账"doc 承诺门禁↔gates 注册↔实现真实性"这一覆盖面目前空缺 |
| 设计-实现一致性迭代检查（IC） | `ae-sdd-integrations/src/jobs/diagnostics/iteration.rs`：IC-1~IC-4 机器粗筛（report-only 不阻断），CLI `ae-sdd iteration-check` |

**颗粒度与边界**：测试真实性扫描是 ⑥.10 硬门禁；MockMvc/application-context-bound client 不属于 socket-level HTTP，真实端口测试中替换内部 Service/Repository/Mapper/Application 仍是 blocker。扫描器均不可被 SKILL 文字描述替代；RA scanner 的 `--file` 是 Work Item authoritative scope，`--root` 是 formal RA 全量审计，两者同时出现时 file scope 优先；missing/outside/non-Markdown explicit file 返回非 0 与 `INVALID_RA_SCAN_SCOPE` JSON。AA（UC-08~13）阻断式，IC（IC-1~4）report-only 不阻断；扫描器路径变更须同步更新 update-graph.json。

---

## 13. 工具链 CLI

### 设计

ae-sdd Python CLI，将 SKILL 规则工具化，实现"规则描述 + 工具执行"双轨 SSOT。规则在 SKILL.md 描述，执行在 CLI 实现，两者通过 `ae-sdd update-check` 自动验证一致性；CLI 是门禁、状态、资产、记忆等能力的统一执行入口。

**⚠️ 文档滞后修正**：原文档标题写"14 大类子命令"，实际顶层子命令组已达 30 个（新增 `doc / enter / context-pressure / ra-gate / flow-violation-scan / ra-depth-scan / ra-implementation-scan / review-loop / plugin / iteration-check / perf` 等）；原表格中列出的 `route / sync-tools / run / quick / proposal` 顶层命令**不存在**，已从下表删除。

### 实现

| 入口 | `bins/ae-sdd-cli`（薄 client，四个子命令），业务模块见各章节 |
| --- | --- |
| 输出协议 | JSON 走 stdout，日志走 stderr，pipeline 友好 |

| 类别 | 实际子命令（按当前 CLI/RPC 现状） |
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

**Windows CLI 启动契约**：CLI 现为原生 `ae-sdd.exe`，无解释器前缀，Windows 上可直接由 PowerShell `&`、ShellExecute 或 `Start-Process` 调用。裸命令命中 `%LOCALAPPDATA%\Programs\ae-sdd\ae-sdd.exe`，安装期写入 `HKCU\Environment\Path` 并广播环境变化，当前父进程仍需重启才能继承。不安装同名 `.ps1` shim，避免 PowerShell 命令优先级抢占。

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
| 3 个枚举 | `ae-sdd-flow/src/model.rs`：`FlowNode`(6成员) / `FlowSkill`(15成员) / `FlowEventType`(8成员：routed-to/skill-completed/gate-blocked/gate-cleared/user-confirmed/phase-changed/reopened/aborted) |
| 事件数据类 | `FlowEvent`，必填 `seq/ts/event/node/by`，条件必填 `skill/gate_id/phase`，可选 `txnName/from_node/reason/output/meta` |
| 5 个工厂函数 | `make_routed_to` / `make_skill_completed` / `make_gate_blocked` / `make_phase_changed` / `make_user_confirmed` |
| 追加事件 API | `ae-sdd-store/src/journal.rs` 的事件追加：自动填 seq/ts |
| 查询事件 API | `ae-sdd-store/src/journal.rs` 的事件查询：txn/type/node 三维过滤 |

**⚠️ 已知缺口（业务调用方尚未全部接入）**：

| 缺失点 | 预期接入方 | 状态 |
| --- | --- | --- |
| 路由时记录 `routed-to` | `cmd_classify` / router | 待接入 |
| 阶段切换时记录 `phase-changed` | `cmd_state_write` | 待接入 |
| 门禁检查时记录 `gate-blocked`/`gate-cleared` | `ae-sdd-gates/src/evaluator.rs` | 待接入 |
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
| PreToolUse hook | `ae-sdd-policy/src/hook.rs`：先检查当前 session turn 绑定；inactive 时直接放行；active 后由 `PHASE_PERMIT` 按 phase 限定工具集；只读工具任何 phase 放行；`paused` phase 禁全部写操作；链式 Bash（含 `&&`/`\|\|`/`;`/`\|`）不认为只读；输出 `permissionDecision: "deny"` + `systemMessage`，exit 始终 0 |
| UserPromptSubmit hook | `hook.user_prompt` 路径：只有 `/ae-sdd` 触发词才创建 turn 绑定、读取 state 并注入阶段上下文；普通 prompt 清理残留 token 后返回 `{}`；active turn 再执行快速通道、重置 Stop 重试计数、母版版本漂移探测、主流程监管器和 compact memory 注入 |
| Stop hook | `hook.stop` 先检查 turn 绑定；inactive 直接放行；active 时执行 stop 校验，成功或 fail-open 后释放 token，阻断重试保留 token；`.stop_retry_count` 仍为 MAX_RETRY=2，继续检测 PRD compact 卡住态 |
| Hook 安装 | `ae-sdd init-hooks` 写三段配置到 `.claude/settings.json` |
| 快速通道 | 用户说 `/ae-sdd-quick` → `.quick_channel` 写入 → PreToolUse 跳过 phase 工具拦截（产物落地关卡仍生效） |

**颗粒度与边界**：hook 层只做物理拦截（工具权限/上下文注入/结构校验），不做业务逻辑判断；hook 任何异常降级不阻断主流程（全流程 try/except，exit 始终 0）；改 hook 须同步 HARNESS.md HS 规则；禁止手工编辑已安装的 hook 脚本，改完须重跑 `ae-sdd init-hooks`。

---

## 16. 主流程监管器（v3.6，已实现）

> 文档修正说明：本节原标"🆕 v3.6 规划中"，经代码核实（监管器逻辑与暂停/恢复/矫正计数均已在 `ae-sdd-runtime/src/flow_supervisor.rs` 实现并接入 `hook.user_prompt`），确认**已完整实现并生效**，非规划态。

### 设计

`/ae-sdd` 触发后全生命周期的编排角色，负责工作区初始化、资产检查、智能路由、各系列循环执行、收尾交付，不执行任何具体业务工作——是 ae-sdd 的"项目经理"，只管"下一步该做什么、由谁做、做完了没有"。角色定义在 SKILL.md，执行实体是 UserPromptSubmit hook 的 Python 逻辑（决策 2B）。

5 步标准启动序列：Step 1 工作区确认与初始化 → Step 2 项目资产检查(G-00) → Step 3 智能路由（双层裁定）→ Step 4 按 scale 子链执行编码流程 → Step 5 收尾交付。每系列执行协议含 sub-step 0~5（compact清理 → SKILL调用声明 → 生成 → Review → Loop控制 → 人工审核）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 偏移检测核心逻辑 | `ae-sdd-runtime/src/flow_supervisor.rs` 的偏移检测，Layer 1 产物核查 + Layer 3 矫正次数升级 |
| phase→gate 映射表 | `flow_supervisor.rs` 的 phase→gate 映射，覆盖 ra-generated~test-running 各 phase |
| 升级判定 | `flow_supervisor.rs` 的升级判定，矫正次数 ≥ `CORRECTION_THRESHOLD_PAUSE`(3) |
| 矫正消息生成 | `flow_supervisor.rs` 的矫正消息生成，severity 1/2/3 三级文本模板 |
| gates check 子进程调用 | `flow_supervisor.rs` 进程内调用 `ae-sdd-gates`，有界超时；engaged 下不降级放行 |
| Hook 侧调用入口 | `hook.user_prompt` 路径调用 `flow_supervisor`，每轮执行；engaged 下异常不静默降级 |
| 状态暂停/恢复 | `ae-sdd-flow/src/control.rs` 的 pause/resume |
| 矫正次数持久化 | `ae-sdd-flow/src/control.rs` 的矫正计数，写入 `state.correctionCounts` |
| SKILL 侧角色定义 | `source/skills/orchestration/ae-sdd-update-skill.md`（5步启动序列文字描述） |

**颗粒度与边界**：监管器角色定义在 SKILL.md，执行实体在 UserPromptSubmit hook（Python，决策 2B）；监管器不执行业务工作，只做编排+校验；ae-sdd 自更新触发时监管器休眠；退出条件：`phase=completed` 或用户手动中止；每系列入口必做 compact（防上下文污染）。

---

## 17. 流程偏移检测与矫正（v3.6，已实现）

> 文档修正说明：本节原标"🆕 v3.6 规划中"，实际与 §16 共用同一套 `flow_supervisor.rs` 实现，已完整生效。

### 设计

识别 AI 在执行过程中的语义漂移行为，按 3 级矫正机制自动纠正，超出阈值升级人工干预并暂停流程。流程偏移分两类：物理越权（已由 PreToolUse hook 拦截）和语义漂移（本模块处理）。

漂移类型：B1 跳步（跳过 Review 宣布进入下一系列）/ B2 停滞（同一 phase 经 N 轮产物未过 gates check）/ B3 伪完成（声称完成但门禁未过，原依赖自报检测，v3.6 改产物核查）/ B4 旁路（题外话后未回到流程）。

矫正级别：Level 1 静默注入（用户不可见，AI 可感知）/ Level 2 矫正提示词（AI 须说明修复计划，同步骤最多3次）/ Level 3 人工升级（`state.phase=paused`，流程暂停待用户决策）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 漂移结果数据类 | `flow_supervisor.rs` 的 drift 结果类型：`drift_type/severity/gate_id/gate_passed/gate_message/phase/correction_count/message` |
| 阈值常量 | `flow_supervisor.rs`：矫正告警阈值 5 / 暂停阈值 3 / gate 检查超时上界 |
| Layer 1 产物核查（解 B1/B3） | `detect_drift()`（行144-225）依据 `get_phase_gate_map()` 跑 gates check，不通过判定漂移，不信任 AI 自报 |
| Layer 3 矫正次数升级（解 B2） | `detect_drift()` 行204：`correction_count >= CORRECTION_THRESHOLD_PAUSE` → severity=3 |
| severity=1 矫正文本 | `build_correction_message()` 行290-296：静默提醒 |
| severity=2 矫正文本 | `build_correction_message()` 行298-309：含矫正次数/失败门禁详情 |
| severity=3 暂停文本 | `build_correction_message()` 行311-323：含继续/跳过/回退/查看四个用户决策选项 |
| phase→gate 映射（9个phase） | `get_phase_gate_map()`（行70-89）：ra-generated→G-RA-1~4，dr-generated→G-01，story-generated→G-02，story-reviewed→G-03，task-generated/task-reviewed→G-04，coding-process→G-08，coding→G-CODEPLAN-SRC，code-reviewed→G-05，test-running→G-09 |
| Stop hook 职责精简 | `hook.stop` 路径：无自报检测，职责为防空响应 + 防无限循环 |
| 全流程降级保护 | `detect_drift()` 行227-233：任何异常返回 `drift_type="none"` 不阻断主流程 |

**颗粒度与边界**：只检测语义漂移（物理越权由 PreToolUse 拦截）；gates check 结果为唯一权威判定，不信任 AI 自报（决策1B）；`correctionCounts` 写入 state.json 跨 session 持久化；`flow_supervisor.rs` 是纯计算模块（只返回结论不直接写 state），写 state 由 `ae-sdd-store` 经 lease mutation 执行；Level 3 暂停后续接检测自动识别 `phase=paused` 并播报。

---

## 18. SKILL 编译与 Runtime IR（v3.8 设计）

### 设计

ae-sdd 分为两种物理版本：`source/` 是未编译母版，面向维护者；`dist/ae-sdd/` 是编译后的实例化运行包，面向各 Agent。正式发版只能分发编译后版本，不能直接把 `source/` 安装到 Agent skills 目录。

编译目标不是把 SKILL 变成不可读"机械码"，而是生成短、结构化、可审查的 runtime compact slices。Agent 主入口 `dist/ae-sdd/SKILL.md` 变为 bootloader，只声明加载顺序、冲突优先级和 fallback 规则；子 SKILL 在 `dist/ae-sdd/skills/**/*.md` 中也必须变为 compiled bootloader，完整 Markdown 原文只保存在 runtime fallback 中，只有 compact 不足时才延迟读取。

源 SKILL 入口在进入 runtime 编译前允许标准化瘦身，但瘦身不是自由删减。`ae-sdd-methodology` 的瘦身路径必须先把完整原文锚定到 `source/skill-fallbacks/**`，再按 `source/standards/skill-source-slimming-standard.md` 和 `source/templates/skill/source-skill-slim-entry-template.md` 渲染 slim entry。每个 slim entry 必须包含语义识别清单，覆盖身份/触发、流程/路由、门禁/约束、工具/API、状态/数据、产物/文档、资源引用、设计对齐和 fallback-only 细节；已瘦身文件默认跳过，schema 升级只能从 fallback 重渲染，禁止从 slim entry 二次瘦身。

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
| 源 SKILL 瘦身 | `ae-sdd-methodology`，按 v2 标准生成 slim entry；完整原文进入 `source/skill-fallbacks/**` |
| 源瘦身标准/模板 | `source/standards/skill-source-slimming-standard.md` + `source/templates/skill/source-skill-slim-entry-template.md` |
| 编译实例包 | `dist/ae-sdd/`，由 `ae-sdd-build post-commit: compile` 生成，git ignored |
| Runtime 编译器 | `ae-sdd-methodology/src/compiler.rs`，生成 `runtime/*.compact.md`、`runtime/skills/**`，并替换 dist 主入口和子 SKILL 入口为 bootloader |
| 通用编译器 SKILL | `standalone-skills/skill-runtime-compiler/`，可复制到其它 agent/仓库，用于把任意 `SKILL.md` 包编译成同级 `<name>-compiled/` |
| 编译接入点 | `ae-sdd-build post-commit: compile` 在复制 source 后调用 runtime 编译器 |
| 门禁 compact 数据源 | `crates/ae-sdd-gates/src/registry.rs:GateRegistry` |
| 状态机 compact 数据源 | `ae-sdd-policy/src/transition.rs` 的 phase 链常量 |
| 机器校验 manifest | `runtime/manifest.json`：`compiled=true`、`deterministic=true`、`runtime_fingerprint`、`load_order`、`source_checksums` |
| 分发入口 | `ae-sdd-build post-commit: distribute` 只接受 `dist/ae-sdd/` 或基于它生成的 Agent 专属产物 |

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
| 配置 SSOT | `.ae-sdd/config.yaml` 的 `automation` 段（init 期生成，默认 `enabled: false`）|
| 配置加载 | `ae-sdd-runtime/src/config.rs`：automation 默认值、启用判定、reviewer tier 与自动化审核点读取 |
| CLI 开关 | `automation status/enable/disable`（enable 写 `enabledAt` 审计时间戳，AI 不得自行改）|
| 开工前预收集 | `ae-sdd preflight collect`：扫输入材料+资产 7 层索引，识别 6 类待补信息，写 `.ae-sdd/preflight-info.yaml` |
| 联审共识 state | `ae-sdd-review/src/model.rs`：`reviewConsensus[point]` 字段与登记/读取 |
| 联审共识写入 | `ae-sdd state register-review-consensus --point {N} --passed {true\|false}` |
| 联审共识门禁 | `ae-sdd-gates` 的 G-AUTO-CONSENSUS（blocker，36 门禁之一）：自动化模式下 review 节点切相前校验 `reviewConsensus[point].passed=true` + reviewer 独立性（复用 G-09B 模式）|
| Tier 强制 | `agent-orchestration-skill.md §8.4.1`：`automation.enabled=true` → 强制 Tier 3，覆盖规模判定 |
| 降级禁止 | `agent-orchestration-skill.md §8.4.5`：自动化模式禁逻辑多视角降级，必须物理 3 独立 session |
| 流程编排 | `source/SKILL.md §🚀 自动化模式` + Step1 自动化检测 + Step1.5 预收集 + 监管器步骤4 联审共识双模式 |
| 级联检查 | UC-16 automation 级联检查：校验 config/gates/state/CLI/init/SKILL 六处齐备 |
| 级联图谱 | `source/standards/update-graph.json:UG-20`：trigger 含 automation config、gates registry、flow transition 与 CLI |

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

### 真实 HTTP 接口验收硬边界

有界测试控制“测多少”，不能削弱接口 AC 的验证边界。接口 AC 固定使用 `boundary=http`、`stages=[local,test-env]`、`internalMocksAllowed=false`：先在 loopback 真实端口验证 Controller→Service→Repository/Mapper→测试 DB，再以同一 buildId 请求非 loopback 测试环境。MockMvc、直接 Controller 调用、内部 MockBean/SpyBean、只有本地结果或 external supplemental evidence 均不能关闭接口 AC。

| 设计点 | 实现方式 |
| --- | --- |
| 计划契约 | G-08 校验 HTTP verification 字段；G-14 从 Story 验证矩阵反查接口 AC boundary |
| 源码真实性 | `test_authenticity_scan.py` 输出 `mock-http-boundary` / `http-internal-mock` BLOCKER |
| 运行证据 | `evidence.validate_http_acceptance_manifest()` 校验阶段、URL、buildId、AC、顺序、artifact 与 internalMocks |
| 完成门禁 | G-09 在通用真实性 evidence 之外要求 active `http-local` + `http-test-env`；环境不可达保持 BLOCKED |

## 23. Story 主/副内容分层

### 设计

Story 的主要内容是对任务最直接描述的完整模板章节，而不是跨章节语义片段。模板在每个 H2 前用稳定 section ID、显式锚点和 `primary/secondary` 元数据声明唯一边界；独立撰写指南按 section ID 定义适用条件、必填性、来源、写法和 Review 口径，Skill 不保存章节清单。章节按分析→设计→实现排序，核心设计集中，任务/人工操作/未决问题隔离到补充区。

Document Storage 返回模板和指南的 `path/source/content/sha256`，纯解析函数从模板正文动态取得主要/副章节。生成 Story 保留 ID-only 隐藏标记，Review/Update 按 ID 使用当前模板层级，因此标题重命名或层级调整不要求修改 Skill。主要章节先执行 `scope=primary`，通过后派生副章节并执行 `scope=full`；主要章节变化使依赖副章节失效。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 章节与层级 SSOT | `STORY_TEMPLATE`：每个 H2 的稳定 ID、layer、顺序和空白结构 |
| 撰写 SOP SSOT | `STORY_WRITING_GUIDE`：按 section ID 定义规则，不复制层级名单 |
| 资源读取 | `document_storage.resolve_read_resource()` 返回正文与 sha256；只读 intent 禁止写入 |
| 动态解析 | `ae-sdd-resources` 的 section 解析：主要/副章节、ID 分类、指南覆盖、导航锚点和历史精确迁移 |
| Generate/Review/Update | 三件套只消费资源正文和解析结果；生成 Story 输出 ID-only 标记；primary → full |
| 验证 | `test_story_template_sections.py`、`test_story_content_layering.py`、Document Storage/CLI、slim/runtime |

**颗粒度与边界**：层级变量只存在于模板，生成 Story 只保留稳定 ID，不新增 state 字段。历史 Story 完全无 ID 时只允许标题精确唯一迁移；部分 ID、未知标题或歧义均 fail closed，禁止语义猜测。
