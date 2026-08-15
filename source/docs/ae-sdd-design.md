# ae-sdd 系统能力说明书

> v4.0.0 · 面向开发者、LLM Agent 与项目接入方
>
> 本文档是**系统能力设计入口**，说明 ae-sdd 的能力语义、边界和当前实现状态。daemon 控制面细化见 [`ae-sdd-daemon-design.md`](ae-sdd-daemon-design.md)；代码分层、模块职责、运行时数据流和变更闭环统一维护在 [`ae-sdd-implementation-architecture.md`](ae-sdd-implementation-architecture.md)。目标语义冲突以本文的明确裁决为准；实现状态以 CLI/测试输出为证据，并必须将偏离登记为迁移项，不能反向静默覆盖设计。

## 过程产物模型

RA、DR、Story、TestCase、CodingPlan 是按任务规模选用的持久化 Spec。所有任务先执行 RA；大任务增加 DR，中/大任务增加 Story。凡存在 Story，必须对每个 Story 执行独立的 `Story -> TestCase -> CodingPlan` 子链；TestCase 不内嵌 Story，也不出现在无 Story 的微/小任务中。小任务直接从 RA 进入 CodingPlan；微任务不创建独立 CodingPlan Markdown，只使用经批准的 `state.executionPlan`。Proposal、GeneratePlan、CodingReport、TestReport、CodeReview 报告和其他过程 Markdown 停止新写入；历史文件只读。Task 仅作为大型并行执行切片，不改变 Spec 子链。实际测试使用 evidence manifest，Review 使用 `state.review.status/findings`。任何时候都不写 changelog。

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
- root 主会话只推动 typed action、创建物理委派、collect 有界 `ChildResult` 和向用户汇报。RA/DR/Story/TestCase/CodingPlan/Coding/Test/Review/Update 等系列语义工作在独立 series session 中完成；lineage 为 `root -> series -> task|reviewer`，最大深度 2。
- Host ACK 只表示原生命令已接收。child 必须使用一次性 claim 在独立物理 session 中完成 attestation，之后才可 running；reviewer 与 worker 物理隔离。
- root 只接收摘要（最多 8 KiB）、finding 统计、next actions 与 artifact path/hash/kind；ChildResult 与 root projection 各最多 64 KiB。child transcript、原始 source bundle、scratch memory 和无界日志不得注入 root。
- Hook 是薄 Rust client 快路径，只读预计算 projection/delta；不在同步路径扫描文档、启动子进程、跑重 Gate、等待 compact 或执行项目 mutation。engaged 时 daemon 不可用即 deny/block，不允许本地业务回退。
- Gate 结果保留 `PASS/FAIL/ERROR/TIMEOUT/CANCELLED/STALE` 六类。只有 fresh PASS 允许依赖 mutation，只有 fresh FAIL 增加业务 correction。
- compact 只接受可信 host token telemetry；默认使用 800/600 permille 滞回、连续两个样本与 300 秒 cooldown。仅 snapshot、匹配原生 ACK、rehydrate 全部完成后进入 `context-restored`。
- workspace writer 模式按 `legacy -> shadow -> rust-canary -> rust-sole-writer` 迁移；shadow 只读比较，任何阶段禁止双写。删除 legacy runtime 后只允许回退到上一版完整 native release，不在 Rust client 中保留隐藏 fallback。

流程统一采用双阶段裁决：Hook 后先记录 provisional `BootstrapAssessment`，再以 RA 作为所有任务（包括 ae-sdd 自更新）的首个业务 Series；只有 RA 完成并关闭输入冲突后，daemon 才冻结 `EngineeringRoute`。最终规模映射为：微任务使用 compact `state.executionPlan`，小任务使用 CodingPlan，中任务使用 `Story -> TestCase -> CodingPlan`，大任务使用 `DR -> N x (Story -> TestCase -> CodingPlan)`。用户批准 `executionPlan` 前不得 Coding。

**Python 树已于 2026-07 全部删除**，本文各章节的实现位置统一指向 `crates/ae-sdd-*` 与 `bins/ae-sdd-*`。Rust 已继承大量门禁、state、扫描和恢复基础，但旧 route-first phase 链、nested state、micro CodingPlan 与 JSON handoff 不再代表目标语义；它们必须按本文和 daemon 专题设计迁移。正文明确标为“当前基础/兼容”的条目不得被宣称为目标已实现。

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
| D-001 | 端到端 Phase 编排 | AI 容易跳过分析、设计、审核、测试或收尾节点，产物链断裂 | `BootstrapAssessment -> RA -> EngineeringRoute` 后按规模执行适用 Spec Series、计划、实现、测试和 Review；节点门禁与人工确认组成有序状态机 | 降低流程遗漏和返工，提升交付可追溯性 | typed next action、Series receipt、`gates check`、execution projection | §1；`source/SKILL.md`；daemon 专题设计 | v4.0 语义冻结/实现迁移中 |
| D-002 | RA-first 双阶段裁决 | 首轮低信息分类若直接冻结工程路线，RA 无法纠正任务类型、规模和影响面 | 启动评估只记录 provisional proposal；所有任务先 RA，冲突关闭后才冻结 EngineeringRoute | 解耦 intake 与最终路线，减少过度/不足设计并保留可审计差异 | BootstrapAssessment、RA receipt、route decision digest、来源冲突测试 | §2；`crates/ae-sdd-flow/src/route.rs`；daemon 专题设计 | v4.0 语义冻结/实现迁移中 |
| D-003 | Work Item / Flow Run 与三类关系 | 多任务、跨 session、Series 重试、Spec 复用和委派恢复时，嵌套业务状态会混淆执行位置、文档血缘与 Agent 授权 | WorkItemId 与 FlowRunId 分离；执行流程树、Spec graph、delegation tree 使用独立身份和边；state 只保存权威投影与引用 | 避免重复执行、跨任务污染和关系误判，让恢复、重试与 Spec 复用可独立审计 | state store、Flow/Series IDs、Spec graph、delegation、path containment tests | §3；`crates/ae-sdd-store`、`crates/ae-sdd-runtime`；daemon 专题设计 | v4.0 语义冻结/部分基础已实现 |
| D-004 | 多 Agent 编排 | 并行任务缺少角色边界、报告格式和故障升级规则 | 角色库、结构化派活卡、reviewer tier、activeAgents/agentReports | 降低重复劳动和自审偏差，提升并行可见性 | agent state 字段、G-09 session 独立性、编排 SKILL | §4；`agent-orchestration-skill.md` | v3.2.6/v3.11.0/部分软约束 |
| D-005 | G-XX 门禁体系 | 仅靠 LLM 自律无法稳定阻止越级、假测试和文档漂移；同一 Story/Work Item 在不同门禁可能被不同路径规则解析 | 软约束、硬门禁、扫描器三层防线，统一 GATE_REGISTRY；micro 校验 executionPlan，small+ 校验 CodingPlan；所有路线强制 RA，Story 路线校验 Story-TestCase-Plan binding | 把高风险错误提前阻断，减少返工，同时避免不适用 Spec 和无关 RA-like 文档误拦 | `gates check`、GATE_REGISTRY、RA/plan/binding tests、G-09/UC checks | §5；`crates/ae-sdd-gates` | v4.0 语义冻结/既有门禁基础已实现、路由级联待迁移 |
| D-006 | Review Batch 与增量验证 | 每次变更都全量重跑，成本高；历史证据容易被覆盖 | review batch、baseline、changedPaths、verification plan、evidence fingerprint | 缩短验证时间，减少无效重跑，同时保持证据可追溯 | `verification.plan`、evidence active/superseded、focused tests | §5 Review Batch；`ae-sdd-execution/src/plan.rs`、`ae-sdd-contracts/src/evidence.rs` | v3.10.1/v3.11.0/已实现 |
| D-007 | Typed operation + state lease | LLM 不知道调用什么、参数怎么写、哪些操作需要锁；并发写会覆盖 state，失败重试会重复推进 | `ops describe/next/execute`、JSON Schema、lease/fencing/revision/idempotency、`nextActions`、短事务锁 | 减少 LLM 理解成本、上下文读取、命令猜测、冲突和失败重试；把自然语言契约变成机器契约 | `ops describe --json`、UC-19、StateStore/operation/concurrency tests、CLI latency baseline（待持续补充） | §5 Review Batch 表、`operation-protocol.md`、`ae-sdd-operations/src/registry.rs` | v3.11.0/v3.11.0/已实现 |
| D-008 | 项目资产七层索引 | LLM 每次从工程源码重新寻找约束、技术栈、模块和接口，容易遗漏上下文 | project assets 分层、生成/审计、倒排/BM25 查询、G-00 | 缩短上下文检索，提升首次回答和路由准确度 | `assets generate/check/query/stats`、G-00、asset index tests | §6；`ae-sdd-resources/src/assets.rs`、`ae-sdd-inventory` | v3.2.4~v3.5.1/v3.11.0/已实现 |
| D-009 | Document Storage | Story/Work Item/constraints/assets/只读模板资源路径分散，正式 StoryName 与逻辑 ID 不同或项目覆盖存在时，LLM 和工具容易读错、猜错或重复读取 | intent-based resolve、Work Item-first、精确 StoryName 绑定；只读资源按项目覆盖→内置回退统一返回 `path/source/content/sha256`；save/finalize 拒绝只读 intent | 减少路径猜测、跨类别假命中、读取竞态和文档错写，让调用 Skill 消费同一权威正文与指纹 | `doc resolve/save/finalize`、`resolve_read_resource`、G-DOC-STORAGE、Story/resource resolver tests | §7；`ae-sdd-resources`（`document.rs`/`resolver.rs`）、`document-storage-skill.md` | v3.7.2/v3.12.1/已实现 |
| D-010 | 四层实例化与分发 | source、dist、用户安装、项目实例互相漂移，修复或共享 scanner 依赖无法传递 | Layer 1 source -> Layer 2 dist -> Layer 3 install -> Layer 4 init/override；新增 scanner 必须注册进 `ae-sdd-scanners::registry` | 降低升级和环境差异造成的故障，保证 LLM 使用同一版本契约，并避免 dist scanner 因漏 helper 无法启动 | `ae-sdd-build` 打包测试、install/init、UC-15/runtime verify | §8；`ae-sdd-build`（`post_commit.rs`/`distributor_registry.rs`） | v3.4.1/v3.11.4/已实现 |
| D-011 | Harness 适配层 | 不同 Agent 运行时需要不同入口，手工转译易造成版本和模板漂移 | adapter lock、source hash、tree hash、生成/回滚/备份轮转 | 减少接入和升级的人工操作，避免安装产物陈旧 | harness build、adapter lock tests、iteration-check | §9；`.harness/`、`ae-sdd-build/src/harness_build.rs` | v3.5.6/v3.11.0/已实现 |
| D-012 | Memory 生命周期 | 长会话上下文过大，compact 后关键业务事实丢失或 scope 混用 | 实体树、boot/context/pending compact、manifest hash、生命周期 CLI | 缩短默认上下文，提升跨 session 恢复和业务事实复用 | `memory create/read/update/search/summarize`、memory tests | §10；`ae-sdd-artifacts/src/memory.rs`、`ae-sdd-context`（`projection.rs`/`compact.rs`） | v3.8.2~v3.10.3/v3.11.0/已实现 |
| D-013 | Plan-First 分级产物 | LLM 直接编码会漏需求、测试、约束和回滚路径；把微任务也强制写成独立 CodingPlan 又会制造过程噪声 | 所有任务编码前都有经批准的 `state.executionPlan`；微任务只用四维 executionPlan，小任务及以上生成正式 CodingPlan；Story 路线先完成对应 TestCase | 把实现前未知风险显式化，同时让持久化 Spec 成本与任务规模匹配 | executionPlan/CodingPlan gates、Story-TestCase-Plan binding、Work Item scope tests | §11；`coding-process-skill.md`、`ae-sdd-gates` | v4.0 语义冻结/实现迁移中 |
| D-014 | 真实性扫描与对齐审计 | 文档可以宣称有门禁/测试/命令，但代码实际没有或已失效；全仓 RA rglob 会把参考资料、模板、事件附件和 dist 副本当作当前需求 | test/coding/RA scanners + UC-08~13 alignment audit + iteration-check；RA scanner 共享 `ae-sdd-scanners/src/scope.rs`，默认 root 审计只枚举 formal RA，Work Item 门禁显式传 repeatable `--file` | 减少“文档看似完整但执行无效”的假闭环，避免 Python/JS/TS work item 被误判为无生产 scope，并消除无关 RA-like 文档造成的 false blocker | UC-08~13、G-09、G-CODE-1、`test_ra_scan_scope.py`、RA scans、文本代码 scope regression | §12；`crates/ae-sdd-scanners`（`registry.rs`/`scope.rs`）、`crates/ae-sdd-integrations/src/jobs/diagnostics/` | v3.5.11~v3.6/v3.11.4/**部分**：7 个 scanner 与 IC-1~4 完整；UC-08~13 alignment audit 随 Python 删除后无承接实现 |
| D-015 | 统一 CLI 工具链 | 工具分散且输出不统一，LLM 需要记忆多个脚本和输出格式；存量 state 缺少正式 StoryName 的可审计迁移入口；批处理若不检查中间退出码会把前序失败覆盖成成功 | `ae-sdd` 统一入口、UTF-8 JSON/stdout/stderr、稳定 exit code；提供 `state new --story-name`、幂等 `state bind-story-doc` 和四个支持 repeatable `--file` 的 RA scanner 子命令；PowerShell 编排逐命令检查 `$LASTEXITCODE` | 减少命令搜索、编码差异、解析、迁移和批处理误判成本，便于 LLM/脚本安全组合调用 | `ae-sdd --help`、Story binding CLI tests、`test_invalid_ra_prerequisite_exits_one_without_writing_or_deleting_draft`、RA CLI forwarding、GBK regression、update-check | §13；`bins/ae-sdd-cli` | 历史版本（精确版本待补）/v3.11.4/已实现（v4 起为薄 client，业务在 daemon） |
| D-016 | State events 操作日志 | 只有当前 state 时无法解释谁在何时推进、恢复或覆盖 | append-only events、seq、txn/node 过滤、兼容旧 state | 提升审计和故障定位效率，减少 LLM 盲目重试 | events read/filter tests、state JSON | §14；`crates/ae-sdd-store/src/journal.rs`、`crates/ae-sdd-runtime/src/flow_supervisor.rs` | v3.4.2/v3.11.0/已实现 |
| D-017 | 三层 Hook 拦截 | 仅在命令执行后发现越级已经太晚，LLM 可能先写错产物或源码 | UserPromptSubmit、PreToolUse、Stop/监控协同；turn-scoped activity token 只在显式 ae-sdd turn 激活 | 将错误拦截前移，减少非法写入和后续修复，同时避免普通 Story 文档检查误触发门禁 | hook tests、gate_intercept、stop_check、harness | §15；`.harness/`、`ae-sdd-policy/src/hook.rs` | v3.4/v3.11.0/v3.11.4/已实现 |
| D-018 | 主流程监管器 | state、gate、memory、产物和 hook 各自工作，缺少统一运行态视图 | FlowRuntime/FlowSupervisor 以 typed Series 事务聚合主节点、SeriesRun、子节点、pending output 和事件；Monitor 只读投影 | 降低 Agent/维护者判断当前流程位置的成本，并阻止 prompt 自报推进状态 | supervisor projection、SeriesProgressEvent、checkpoint/replay、Monitor read-only tests | §16；daemon 专题设计；`ae-sdd-runtime` | v4.0 语义冻结/部分基础已实现 |
| D-019 | 流程偏移检测与矫正 | AI 或人工可能跳步、回退或修改错误 Work Item，状态表面仍可继续 | 偏移规则、矫正计数、paused/人工升级、scope 复核 | 尽早发现流程漂移，避免错误积累到交付阶段 | iteration-check、state correction、flow violation scan | §17；`ae-sdd-integrations/src/jobs/diagnostics/iteration.rs`、`ae-sdd-flow` | v3.6/v3.11.0/部分 report-only |
| D-020 | SKILL 编译与 Runtime IR | 完整 SKILL 过长，默认加载浪费上下文；source/dist/runtime 容易不一致 | source slimming、fallback、compact boot/core/outline、manifest fingerprint | 缩短 LLM 默认上下文，保持按需加载和可验证分发 | SKILL 编译与 verify、UC-15、runtime verify | §18；`ae-sdd-methodology`（`compiler.rs`/`verifier.rs`） | v3.8/v3.11.0/已实现 |
| D-021 | 自动化模式 | 逐个等待人工确认耗时，但直接自动推进又会失去独立审查 | 默认关闭、Tier 3 多 reviewer、reviewConsensus、G-AUTO-CONSENSUS | 在明确开关下减少等待，同时保留高风险审查 | automation CLI、G-AUTO-CONSENSUS、UC-16 | §19；`ae-sdd-runtime/src/config.rs`、`ae-sdd-flow`、`ae-sdd-gates` | v3.8.0/v3.11.0/已实现 |
| D-022 | ae-sdd Monitor | 多工作区、active task、memory、runtime stats 分散在文件中，定位异常慢 | 只读 workspace 扫描、项目/任务双层视图、响应式刷新 | 降低人工监控和 LLM 状态查询成本，不改变权威 state | `apps/ae-sdd-monitor` tests、UG-22、只读边界 | §20；`source/docs/ae-sdd-monitor-design.md` | v3.7.0/v3.11.0/已实现 |
| D-023 | Sonar Issue 修复收尾 | CodeReview 发现的问题容易重复处理、越界修改或缺少 exactly-once 证据 | issue registry、TextEdit/provenance、compile/test/rescan 闭环 | 减少重复修复和错误补丁，提升 review 收尾效率 | `test_sonar_issue_fix_skill.py`、规则 registry、CodeReview evidence | §21；`sonar-issue-fix-skill.md`、`sonar-issue-fix-rules.md` | v3.11.0/v3.11.0/已实现 |
| D-024 | Design Ledger 治理 | 设计动机、价值假设和迭代影响容易再次分散或漏记，台账本身也可能失去维护 | §0 台账 + CHANGELOG `Design ledger impact` + UG-28/UC-20 fail-closed | 降低后续 LLM 重新理解和维护者追溯成本，让设计价值记录成为可检查资产 | `UC-20`、`update-check 20/20`、台账字段/章节/版本反例测试 | §0；`ae-sdd-integrations/src/jobs/diagnostics/update.rs`、`ae-sdd-update`、CHANGELOG 模板 | v3.11.1/v3.11.1/已实现 |
| D-025 | Story-TestCase 子流程与有界测试策略 | Story 验收若直接混入 CodingPlan 或无限扩张测试矩阵，会同时造成漏测、职责混合和维护成本失控 | 每个 Story 在 CodingPlan 前执行独立 TestCase Series；先建有限风险登记，再按边界准入、等价类、最低充分层级和局部数量上限选择 | 让验证设计可独立监督和追踪，同时减少无独立缺陷发现价值的用例 | Story/TestCase/CodingPlan 绑定、TC-G11/TC-10、后续数量与缺陷发现率基线 | §22；TestCase generate/review/template；daemon 专题设计 | v4.0 流程语义冻结/策略已实现、typed wiring 待迁移 |
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

将整个研发流程切分为三个强制有序的 Phase，每个 Phase 有明确入口门禁、执行节点、人工审核点和出口产物。Agent 负责执行 daemon 下发的 typed 事务，不能跳过任何**适用**节点；每个人工审核节点必须在对话内直接呈现待确认内容，不能只丢文档链接。

- Phase 1 分析与设计阶段：启动评估 → RA → 工程路由 → 按规模进入 DR/Story → Story Review → 人工审核点 1。启动评估不冻结最终 route；所有任务（含自更新）都先完成 RA。
- Phase 2 计划与实现阶段：有 Story 的分支先执行独立 `TestCase -> CodingPlan`；小任务直接执行 CodingPlan；微任务只生成 `state.executionPlan`。计划经人工审核后进入 Coding；Task 仅在需要大型并行执行切片时从已批准计划派生，不插入 Story 与 TestCase/CodingPlan 之间。
- Phase 3 验证阶段：完成判定⑥（10 项条件）→ 全切面一致性核查(⑥bis) → CodeReview 报告(⑦) → 全链路对称性核查(⑦bis) → 人工审核点 4
- 共 5 个人工审核节点，内容必须输出在对话中，不能要求用户自行打开文件查看

**v3.5.2 流程收尾合规自检**：在两处流程结束点增加"自检合规 → 不合规就修复"环节，防止 AI 裸 ✅ 收尾。Story 级 ⑦ter（人工审核点 4 后）5 维度自检；PRD 级 §1.7（`prd-complete` 前）强制先跑 `prd-check-complete`。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| Phase/节点定义 | 目标由 FlowRuntime + versioned SeriesPlan/Series rules 决定；现有 SKILL/静态 phase 链只作方法和迁移输入，不能推进状态 |
| 节点进度持久化 | committed event + `currentMainNode/currentSeriesRunId/currentSubNode` projection；现有 `state.json.phase` 只作兼容字段 |
| Story 级 ⑦ter 自检 | AI 编排执行：`gates check` 全量 + `gates check --only G-DOC-STORAGE` + `state read` + 产出物核对，SKILL 文字描述，非独立 CLI |
| PRD 级 §1.7 强制顺序 | `workitem.complete` 的 PRD 前置检查必须先于 `ae-sdd state prd-complete --prd <ID>` |
| 4 层 AND 校验 | `crates/ae-sdd-lifecycle/src/validation.rs` 的 4 层 AND 校验（G-PRD-1~4） |
| 审核点用户确认 token | `ae-sdd state confirm --phase <审核点>`（防 AI 自填，经 typed operation 写入） |

**颗粒度与边界**：流程节点粒度；RA、路线批准、Spec 审核和 executionPlan 审核等关键事实锁定后不得静默重跑或覆盖。微任务只豁免独立 CodingPlan/TestCase 等不适用 Spec，不豁免 RA、Plan-first、测试、独立 Review、state 更新或完成门禁。`/ae-sdd-quick` 也不得成为跳过 RA-first 和 daemon typed transition 的旁路。

---

## 2. 启动评估与 RA 后工程路由

### 设计

统一入口必须把低信息启动判断与权威工程路线分开：

1. `BootstrapAssessment`：Hook 后判断 `taskKindProposal`（`self_update`/`implementation`）、`scaleProposal`（微/小/中/大）和输入来源（口述、原型/Demo、PRD，可同时存在），并记录事实、未知项和冲突。该结果只用于初始化 Work Item/Flow Run 与准备 RA 上下文。
2. `Requirement Analysis`：所有任务均创建 RA SeriesRun。RA 融合三类来源、逐条保留 source refs，并将实质冲突转入 `awaiting_user`；采用既有 PRD/RA 只形成 DocumentBinding，不等于本次 RA 已完成。
3. `EngineeringRoute`：RA receipt 验证通过且冲突关闭后，daemon 才冻结最终 `taskKind/finalScale/requiredSeries/requiredSpecKinds`，再由用户批准。

最终路线只有四种规模映射：微 `RA -> executionPlan`；小 `RA -> CodingPlan`；中 `RA -> Story -> TestCase -> CodingPlan`；大 `RA -> DR -> N x (Story -> TestCase -> CodingPlan)`。`self_update` 不在启动阶段短路，而是在 RA 后进入 Update Series，并继续遵守同一规模深度。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 启动评估基础 | `crates/ae-sdd-integrations/src/jobs/misc.rs` 的 classify job 可提供标题、文件名、关键词和规模信号；其输出必须降级为 provisional proposal，不得直接冻结 route |
| RA 与多来源 | RA skill/scanner 和文档解析已有基础；口述、Demo、PRD 的 typed source、逐条 trace 与冲突状态仍需接入 daemon |
| 工程路由 | `crates/ae-sdd-flow/src/route.rs` 已有 route engine，但当前在 RA 前冻结路线，属于待迁移实现；目标输入必须改为 validated RA result |
| CLI 入口 | `ae-sdd classify --text "..."` 仅可作为启动评估入口；权威 route 由 daemon typed operation 提交 |
| 合法规模枚举 | `crates/ae-sdd-domain` 的 `WorkScale` 枚举（大/中/小/微） |
| 路由到子链 | `crates/ae-sdd-policy/src/transition.rs` 的现有静态链需迁移为 daemon 冻结的 typed SeriesPlan；Story 分支必须显式含 TestCase 和 CodingPlan |

**颗粒度与边界**：启动评估只产生 proposal；工程路由到 Series 级，Series 内子节点由对应 typed 规则监督。最终路由决策权归 daemon policy 与用户批准，不归 Agent 临时直觉，也不得由文档文件名、`entryNode` 或既有 phase 反推。

---

## 3. 流程状态持久化（state.json）

### 设计

每个用户任务对应稳定 `WorkItemId`；每次主流程执行对应独立、时间有序的 `FlowRunId`。Hook 的启动评估完成后由 daemon 创建或定位 Work Item 并铸造 Flow Run。`createdAt`、可信 initiator session/Agent identity 和显示名称分别保存用于检索与审计，不拼入安全身份。

所有 Work Item 都从统一 intake 开始。PRD、DR、Story、TestCase 或 CodingPlan 是输入/Spec binding，不是主流程入口，不得通过 `entryNode` 直接预设规模、跳过 RA 或推进 phase。采用既有文档只绑定其 `DocumentId/DocumentVersionId/contentDigest`；本次流程仍创建对应 SeriesRun 并验证当前输入是否被覆盖。

state 只持久化流程权威事实与可恢复投影，至少包含：

- `workItemId/flowRunId/stateRevision/policyDigest/inputFingerprint`
- `currentMainNode/currentSeriesId/currentSeriesRunId/currentSubNode`
- `pendingActions/pendingOutputs`、批准、Gate、evidence、review 和中断/恢复状态
- Spec、artifact、delegation 和事件的稳定引用，不保存正文或 child transcript

以下三类关系必须分别建模，不能继续压入 PRD/DR/Story 嵌套 state：

1. **执行流程树**：Work Item、Flow Run、Series、SeriesRun 与当前子节点。
2. **Spec graph**：RA、DR、Story、TestCase、CodingPlan 的 `DocumentId`、不可变版本及派生/引用边。
3. **delegation tree**：root、series、task/reviewer session 之间的授权边。

一个逻辑 Series 可有多个重试 run；一个 Spec 可被多个 Work Item 引用；一个 Agent session 可因恢复而更换。三者都不得复制或改变其他关系中的逻辑身份。暂停、stale、interrupted 和恢复必须由 committed event 与 revision/fencing 规则推进。

旧 `prdState/drStates/storyStates`、`stateMachineId/stateMachineName/stateUuid`、`entryNode` 和 flat state 只允许作为兼容读取或只读 UI projection。迁移完成前保留显式 adapter；新流程不得继续把它们当作路由、关系或完成 authority，也不得隐式重建空状态掩盖损坏。

### 实现

| 设计点 | 当前基础与迁移要求 |
| --- | --- |
| Work Item state store | `.auto-engineering/{WORKITEM-KEY}/state.json`、revision/lease/fencing/journal 与 containment 已有基础，应保留；项目级 `.ae-sdd/state.json` 不得成为 active mirror/fallback |
| Work Item intake | `operation.execute workitem.create` 已存在，但 `entryNode` 仍参与当前合同/实现，必须迁移为统一 intake；文档种类只作为 binding hint |
| 流程链 | `crates/ae-sdd-policy/src/transition.rs` 当前仍在 RA 前冻结 route，且以静态 phase 链表达 Series；必须改为 RA-first typed SeriesPlan |
| Flow/Series 身份 | WorkItemId、delegation 与部分 SeriesId 已有基础；生产 `FlowRunId/SeriesRunId`、retryOf 和当前位置投影待补 |
| Spec 身份与关系 | Story path binding、路径 containment 和 digest 已有基础；稳定 DocumentVersion 与跨 Work Item Spec graph 待补 |
| 旧 nested/flat state | 保留兼容读取、迁移和只读 projection；停止作为新 Work Item 的领域模型 |
| 暂停与恢复 | `ae-sdd-flow/src/control.rs`、checkpoint/replay 和 daemon recovery 基础应保留，并扩展到 Series 子节点 |
| 终态不变量 | 完成必须同时校验 required Series、Spec graph、executionPlan、fresh Gate/evidence/review 与空 pending outputs，不能只同步 phase 别名 |

**颗粒度与边界**：主流程记录到 Series，Series 记录到受版本控制的子节点；不可倒退的批准和 Gate 事实绑定 revision/digest。state 只记录业务事实、进度和引用，不存储 Spec 正文、源码、完整 prompt 或 child transcript。执行树、Spec graph 与 delegation tree 可分别重建和查询，任何一个 projection 都不得反向充当另外两类关系的 authority。

---

## 4. 多 Agent 编排（角色库 + 派活协议）

### 设计

root Agent 不自行拆分业务流程，只提交 daemon 允许的 typed action。FlowRuntime 为 RA/DR/Story/TestCase/CodingPlan/Coding/Test/Review/Update 创建 SeriesPlan；DelegationSupervisor 再从计划派生物理 child、最小 capability 和有界结果合同。Series 内存在无强依赖的实现/验证切片时，series Agent 才能在授权下创建 task/reviewer；lineage 固定为 `root -> series -> task|reviewer`，最大深度 2。

派活 authority 是 `SeriesPlan + InstructionEnvelope + DelegationId/SeriesRunId`，至少绑定输入 artifact、SKILL/规范 digest、允许操作/路径、交付物、deadline、重试和结果 schema。自然语言或 YAML 任务卡只能作为展示 projection，不能替代 typed contract。Review worker 与被审 worker 必须使用独立物理 session。

角色库与 reviewer Tier 仍可由 `agent-orchestration-skill.md` 提供方法指导，但 daemon 必须验证 role、lineage、claim/attestation、grant、result digest 和 cleanup receipt；root 或 child 的自报身份没有授权效力。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 方法角色库/Reviewer Tier | `source/skills/cross-cutting/agent-orchestration-skill.md` 提供语义指导；不得充当 runtime authority |
| typed Series 基础 | `ae-sdd-contracts/ae-sdd-flow` 已有 SeriesPlan/Receipt/Planner，必须接入唯一生产控制链 |
| 委派与结果 | `ae-sdd-delegation`、`DelegationSupervisor`、Host ACK/claim/attestation 与 bounded `ChildResult` 已有基础，需绑定 FlowRun/SeriesRun |
| 独立性约束 | capability、可信 session/lineage 与 G-09/review rules 共同校验；不能只信报告中的 `session_id` |
| Agent 注册/Host 适配 | `source/docs/agent-registry-protocol.md` 与 harness adapter 负责物理模板/命令映射；不得预创建共享业务 session 或决定 Series 顺序 |

**颗粒度与边界**：不同 Story 分支可在依赖满足后并行；同一 Story 的 `Story -> TestCase -> CodingPlan` 不可并行穿越。root 负责用户交互和有界汇总，不拥有最终工程 route 或需求冲突裁决权；这些分别由 daemon policy 和用户批准形成 committed fact。逻辑多视角不能替代要求物理独立的 reviewer。

---

## 5. 门禁体系（G-XX 三种强度）

### 设计

三种强度形成纵深防御：软约束依赖 LLM 自律，硬门禁通过 CLI 阻断，扫描器兜底覆盖硬门禁无法检测的场景。

- **软约束**：SKILL 文本说明，LLM 自律执行
- **硬门禁**：`GATE_REGISTRY` 注册，CLI `ae-sdd gates check` 阻断，不通过无法继续
- **检查器**：`ae-sdd-scanners` 的 7 个扫描器，兜底补硬门禁覆盖不到的场景

G-00 项目资产门卫每次 SKILL 启动前验证资产存在；G-RA 系列在需求分析阶段把关；G-CODE-1 在 Coding 完成/CodeReview 前扫描反模式；中段门禁（G-14/G-CODEPLAN-SRC/G-DOC-STORAGE）补"两头强中间空"；入口关卡三道闸（entry token / 产物落地凭证 / 代码改动准入）管住流程入口。

计划门禁按最终 `scale` 和 route 选择产物。小/中/大任务的正式 CodingPlan 保持完整章节、Story/AC 对齐（适用时）和源码核对；微任务不解析或生成独立 CodingPlan 文档，只校验 `state.executionPlan` 的变更范围、实现顺序、风险/回滚和验证四维完整且无未闭环项。Story 路线还必须证明 CodingPlan 绑定同一 Story 的已批准 TestCase；小任务无 Story，因此该绑定为可审计 N/A。

G-02 与 G-14 通过 `document_storage.resolve_story_document()` 共享 Story 正文解析。已绑定 `docPath` 优先，随后是精确非 glob StoryName，只有没有 StoryName 时才兼容 Story 类别的 `{STORY-ID}.md`；Task/Coding/Test/CR 同名文件不参与。正式文件必须由正文 `Story ID` 元数据反校验。多候选、元数据缺失/漂移和非法 basename 全部 fail closed，不做模糊猜测。

G-RA-1~6 与 G-RA-FLOW-VIOLATION 通过同一个 authoritative RA resolver 读取当前 Work Item 的 `/documentPaths/RA` 标量。resolver 只接受 `ae-sdd-doc/RA/` 下的单个普通 Markdown 文件并执行 containment 校验；没有 Story fallback、目录猜测、首文件命中或“任意 RA 存在”退路。`RequirementAnalysis` selector 精确绑定 Work Item、RA path/bytes、已验证 receipt 与 source revision；`RouteBinding` 另外绑定 candidate、approval、scale evidence、closure receipt、frozen route 和 blocking conflicts。无明确 binding 或任一 digest 不一致均 fail closed。

上下文加载准入门禁（G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX）复用注册表式 `_check_context_loaded`。目标依赖固定为：DR 查 RA；Story 查 RA 和适用 DR；TestCase 必须查唯一 Story；Story 路线的 CodingPlan 必须查该 Story 及其已批准 TestCase；Task 只从已批准计划派生。微/小路线因不存在 Story 可对 TestCase 依赖返回 `not_applicable`，但不得伪造已完成 TestCase。现有 gate registry 尚未完整表达 CodingPlan 的 Story-TestCase 绑定，属于待补 typed 校验。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 门禁注册表 | `crates/ae-sdd-gates/src/registry.rs:GateRegistry`（list，实际 36 个；包含 G-HTTP-1 场景推导门禁） |
| 统一评估入口 | `gate.evaluate` RPC → `crates/ae-sdd-gates/src/evaluator.rs` |
| 单个门禁定向检查 | 同一 RPC 指定 gate id；重 Gate 经 `scheduler.rs` 有界排队 |
| G-00 资产门卫 | `ae-sdd-gates` G-00 check；不通过时用 assets 相关 operation 生成/修复 baseline 资产再校验 |
| G-RA-1 | 当前 Work Item 的唯一 v2 SRS 与 collected/validated RA receipt 在 DocumentId/version/content digest/source revision 上精确一致 |
| G-RA-2 | 有界 `RaCore` scanner 校验 SRS Core、ID 唯一性和 placeholder/结构边界 |
| G-RA-3 | 有界 `RaApplicability` scanner 校验七维判定、applicable 章节和 unknown GAP 闭合 |
| G-RA-4 | 有界 `RaClosure` scanner 校验 REQ-REF、REQ-AC、blocking GAP 与六维 scale evidence |
| G-RA-5 / G-RA-6 | 分别转发 G-RA-3 / G-RA-4 的真实结果并给 replacement diagnostic；不进入自动 required set |
| G-RA-FLOW-VIOLATION | `RequirementAnalysis + RouteBinding` predicate；验证 RA-first、approval、receipt/digest/scale/candidate binding 后才允许 freeze route |
| G-RA authoritative scope | `crates/ae-sdd-integrations/src/gate_source/ra_binding.rs` 统一 resolver；key/predicate/scanner adapter 禁止各自猜 RA |
| G-CODE-1 Coding 真实性 | `ae-sdd-gates` + `coding-authenticity` scanner（AP-1~AP-6 反模式） |
| G-09 测试真实性 | `ae-sdd-gates` + `test-authenticity` scanner（8 类禁止手段）；可信 work-item scope 优先取 `VerificationPlan.changedPaths`，兼容 state changed paths，无 scope 时全仓严格扫描 |
| G-13 全链路对称性 | 当前仍含 `entryNode` 分支；目标改为按 EngineeringRoute 校验 DR 可选性，以及每个 Story 的 `Story -> TestCase -> CodingPlan` 完整链 |
| G-07 / G-08 / G-14 / G-CODEPLAN-SRC | 完整 CodingPlan 校验基础保留；现有 micro CodingPlan profile 的四维语义必须迁移到 `state.executionPlan`，并为 Story 路线增加 TestCase binding 校验 |
| G-02 / G-14 Story 正文 | `ae-sdd-gates` + `ae-sdd-resources` 的 Story resolver；绑定路径/StoryName 校验失败时返回稳定错误码和恢复动作 |
| G-DOC-STORAGE / G-DOC-CONSISTENCY / G-PATH | `ae-sdd-gates` 对应 check；G-PATH 仅豁免 canonical document-storage source entry 及其 source/runtime full fallback，其他同名或 `SKILL.full.md` 文件仍受扫描 |
| G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX | `ae-sdd-gates/src/registry.rs` 四条注册项共用同一 context-loaded 判定，避免每门禁重写 scale 豁免与 phase 感知；各 phase 入口由 `crates/ae-sdd-policy/src/hook.rs` 挂载 |
| G-REVIEW-LOOP | `ae-sdd-gates` + `crates/ae-sdd-review/src/supervisor.rs` |
| 入口关卡1（session 绑定） | `session.open`；每个宿主 Hook 事件由 `bins/ae-sdd-cli/src/main.rs` 的 `bind_host_session` 从 `sessionId` + `cwd` 幂等重建绑定（Hook 是独立子进程，无法继承 SessionStart 的绑定） |
| 入口关卡2（产物落地凭证） | `crates/ae-sdd-policy/src/hook.rs` 的 PreToolUse 裁决，产物路径 + phase 映射校验 |
| 入口关卡3（代码改动准入） | 同上 HookGuard：非 coding phase 或无审核点 2.5 确认 → 禁写 src/ |
| 设计-实现对齐反查 | 原 `alignment_audit.py`（UC-08~UC-13）随 Python 树删除，**当前无承接实现**；`ae-sdd update-check` 仅 UC-01 为真实检查，见实现架构 §11 |

**G-PATH 项目侧输入边界（v3.10.2）**：项目侧扫描只读取声明为记忆/约束输入的 `.ae-sdd/memory/**/*.md`、顶层 `AGENTS.md`/`CLAUDE.md`/`MEMORY.md` 和 `.harness/memory/**/*.md`。`.ae-sdd/drafts/**/*.md` 是 Review/生成过程产物，不是 canonical 正文或项目记忆，不进入 G-PATH；其流程真实性仍由对应 Review/上下文门禁负责。`current_story` 不用于静默过滤路径，避免通过伪造 Story ID 绕过项目级记忆扫描。

**颗粒度与边界**：G-00 每次 SKILL 调用前必跑；所有任务必须完成本次 RA，微任务、BUG/配置类、自更新和既有文档 adoption 均无 RA 旁路。重入只能凭同一 Work Item/Flow Run 中已验证且未 stale 的 RA receipt 跳过重复执行。改门禁强度必须改 `ae-sdd-gates/src/registry.rs`，不能只改 SKILL.md 文字；`ae-sdd update-check` 自动检测文档-实现一致性，防文档撒谎复发。

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

统一文档落地、读取、注册、版本和归档的横切能力。所有 SKILL 读写 ae-sdd 生成的 RA/DR/Story/TestCase/CodingPlan 等 Spec 必须通过本层 typed API，禁止裸拼路径或裸调 `ae-sdd assets read`。`DocumentId` 表示稳定逻辑身份，`DocumentVersionId/contentDigest` 表示不可变内容版本，workspace-relative path 只是可变定位；Story 等业务逻辑 ID 与正式文件 basename 继续分离。

核心原则：intent 驱动解析，daemon 文档注册器根据受控 path/digest 创建或定位 DocumentId；内容变化创建新 DocumentVersion，移动/重命名不改变 DocumentId，复制后独立演进则创建新 ID 并记录 `derived_from`。Spec graph 至少表达 `analyzes/drives/decomposes_to/verified_by/constrains/implements/references/supersedes/derived_from`；Work Item 通过引用边挂靠 graph，不能复制文档节点。Task/Coding/Test/Review 等过程 artifact 以 WorkItem/SeriesRun 分桶，但不混入 Spec graph。禁止 fuzzy ID 扫描。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 动态定位 API 契约 | `source/skills/cross-cutting/document-storage-skill.md` §4（§4.10 intent 枚举表 34个intent） |
| 路径解析 | `ae-sdd-resources/src/resolver.rs` |
| 原生 StoryName 解析 | `ae-sdd-resources` Story resolver；返回 path/source/candidates/rejected，正式文件校验 `Story ID` 元数据 |
| 文档保存（带版本号+ChangeLog） | `ae-sdd-resources/src/document.rs` 的 save |
| 文档定稿 | `ae-sdd-resources/src/document.rs` 的 finalize |
| 稳定文档身份/版本 | 路径 containment 与 sha256 已有基础；生产 DocumentId/DocumentVersionId registry 尚待接入 |
| Spec graph | 当前单 Work Item 文档树只可保留为只读 projection；跨 Work Item graph、关系边和按 DocumentId 挂靠 operation 尚待实现 |
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

**颗粒度与边界**：所有 ae-sdd 生成文档的读写必须走本层 API，不允许 SKILL 各自维护路径拼接逻辑。只读资源由本层一次性完成定位、UTF-8 读取和 sha256 计算；调用方消费返回正文，不得二次打开路径。只读 intent 不进入可写路径表，save/finalize 必须 fail closed。path、业务名、inode 或调用方自报 docId 都不能充当稳定文档身份；Spec graph 与执行流程树必须分离。

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

memory 存储编译后的 compact 文档，按 Series/业务实体平级分层（ra/dr/story/testcase/codingplan/coding/common）。子流程 Agent 首次进入时从 daemon 下发的 DocumentRef/context bundle 读取源上下文，经 document-storage 校验后编译为 compact 并写入自己的 memory；后续读取使用带 fingerprint 的 memory。子流程结束清理自己的 scratch memory，common 保留。

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

**颗粒度与边界**：RA/DR/Story/TestCase/CodingPlan/Coding 等 Series 各有独立 memory 实体；TestCase 与 CodingPlan memory 必须携带对应 Story/SeriesRun identity，禁止跨 Story 复用。子流程结束删自己的 memory，common 保留；从 0 重建 = clean-all（保留 common）；恢复流程按 fingerprint 读取，无匹配项才重建。上下文由 daemon 根据 SeriesPlan 投影，不能仅按全局 phase 猜测实体。

---

## 11. Plan-First 编排（分级 executionPlan/CodingPlan）

### 设计

编码前必须存在经用户确认的 `state.executionPlan`，将变更范围、实现顺序、风险/回滚和验证点锁定。微任务只生成该四维计划，不创建独立 CodingPlan Markdown；小/中/大任务必须先创建或采用正式 CodingPlan Spec，再由 daemon 将其批准版本投影为当前 `executionPlan`。中/大任务的 CodingPlan 只能在对应 `Story -> TestCase` 完成后创建，且必须绑定同一 Story 与 TestCase。Agent 不得自行调整已批准计划。

CodingModel 11 维决策（嵌入每个 Task 文档）：并发控制/幂等策略/事务边界/缓存策略/错误码/异常处理/状态机实现/外部依赖/数据模型/可观测性/复用能力。实现方案决策基线（最高优先级 4 步强制）：① 现有能力复用扫描 → ② 业内成熟方案参考 → ③ 五维代码质量评估 → ④ 核心能力归属唯一。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| CodingPlan 生成时机 | 小任务在 RA 后；中/大任务在对应 Story 的 TestCase 批准后。现有 Phase 2 ④bis/task-writer 顺序需迁移，Task 不得先于或夹入 `Story -> TestCase -> CodingPlan` |
| CodingPlan 定位 | `ae-sdd-gates` + `ae-sdd-resources/src/resolver.rs`，Work Item-first、唯一 Story fallback 语义 |
| 完整 CodingPlan 门禁 | `ae-sdd-gates` 的完整 profile 适用于小/中/大；中/大额外校验 Story、TestCase 与计划的 identity/digest 链 |
| 微任务四维门禁 | 现有 micro CodingPlan profile 的四维校验逻辑迁移到 `state.executionPlan`；不得要求文件、标题或 Story 章节 |
| G-14 | 有 Story 时严格核对 Story/AC/TestCase；小任务和微任务无 Story 时返回可审计 N/A |
| G-CODEPLAN-SRC | 有类骨架时校验源码标记和真实路径；微任务无类骨架时显式跳过 |
| 上下文加载 | 小任务加载 RA；Story 路线加载 RA、适用 DR、Story 和同一 Story 的 TestCase；统一由 daemon ContextProjection + document-storage 提供 |
| 人工审核点 2.5 | AI 主动 walkthrough（SKILL 文字流程，非 CLI），逐条等用户确认 |

**颗粒度与边界**：Plan-first 不可跳过，但持久化产物按规模区分。micro 的 authority 是 approved `state.executionPlan`；small/medium/large 的 authority 是已批准 CodingPlan 对应的 executionPlan projection。Story、TestCase、CodingPlan 任一内容版本变化都使旧 plan approval stale，必须重新投影并确认；编码中偏离计划需用户批准新的 revision。

---

## 12. 真实性扫描（静态扫描器 + 设计-实现对齐验证器）

### 设计

防止 LLM 伪造测试通过、伪造需求分析内容、伪造设计-实现对齐的静态扫描器，作为不可绕过的硬门禁运行时依赖。输出统一 JSON 契约，BLOCKER=0 才算通过。

RA SRS 使用三个有界结构 scanner；route freeze 使用 typed predicate，不再以关键词 scanner 代替状态、receipt 与 route binding 校验。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 测试真实性扫描（⑥.10/G-09硬门禁） | `test-authenticity` scanner：通用假测试规则 + `mock-http-boundary` + `http-internal-mock` + Surefire XML；由 G-09 消费 |
| Coding 真实性扫描（G-CODE-1） | `coding-authenticity` scanner：AP-1~AP-6 反模式库；由 G-CODE-1 触发 |
| RA Core（G-RA-2） | `RaCore` scanner：v2 Core、唯一 ID、placeholder 与结构/大小边界 |
| RA applicability（G-RA-3/G-RA-5） | `RaApplicability` scanner：七维状态与条件章节/GAP 一致性；G-RA-5 是兼容入口 |
| RA closure（G-RA-4/G-RA-6） | `RaClosure` scanner：traceability、blocking GAP、analysisState 与六维 scale；G-RA-6 是兼容入口 |
| RA flow binding | `G-RA-FLOW-VIOLATION` typed predicate 校验 `RequirementAnalysis + RouteBinding`，不扫描 prose 关键词 |
| RA 扫描作用域 | integrations authoritative resolver 只解析 `/documentPaths/RA`，scanner 接收已经选择且通过 containment 的单文件 bytes |
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

## 14. committed 流程事件与可恢复投影

### 设计

单一 `phase/history` 无法解释一次 Work Item 中的 Flow Run、Series 重试、Spec 版本、委派和恢复。daemon 必须将流程变化先提交为有全局顺序、可重放且幂等的 committed event，再由 reducer 生成 `currentMainNode/currentSeriesRunId/currentSubNode/pendingOutputs` 等 state projection。

事件至少覆盖：Work Item/Flow Run 创建与恢复、BootstrapAssessment、需求来源/冲突/用户裁决、RA receipt、EngineeringRoute proposed/approved/changed、Spec binding/version/graph edge、SeriesPlan/claim/progress/result/terminal、Gate 六态与 stale、executionPlan、artifact/evidence/review、deadline/cancel/recovery。事件绑定 `workItemId/flowRunId/seriesId/seriesRunId/delegationId/stateRevision/inputFingerprint/policyDigest` 中适用字段。

关键设计决策：① event ID 与 idempotency key 去重，重放同一 Hook/operation 不重复推进；② journal 使用 PREPARED/COMMITTED、revision/lease/fencing 与 checkpoint 保证恢复；③ event 只记录业务事实和引用，不记录 Agent 内部推理、完整 prompt、Spec/源码正文或无界日志；④ state 和 Monitor 都是可重建 projection，不与 event 双写两套业务 authority；⑤ 旧 `events/phase/history/txnName` 仅兼容读取。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| ordered journal/checkpoint | `crates/ae-sdd-store/src/journal.rs` 与 runtime checkpoint/replay 已有基础，应保留 |
| Flow event/reducer | `ae-sdd-flow` 已有 event 和 pure reducer 基础；现有枚举缺少 FlowRun/SeriesRun/subnode/document identity，需版本化扩展 |
| supervisor events | `ae-sdd-runtime/src/flow_supervisor.rs` 已持久化部分 control decision；生产 request/delegation/document 路径尚未统一接入 |
| Gate/evidence/review | 各模块已有 typed outcome/fingerprint 基础，必须通过同一 committed event 流回 reducer |
| 旧 state events | 保留兼容读取和迁移 projection；不得宣称旧 8 类 phase event 已覆盖目标语义 |

**颗粒度与边界**：业务事件只追加且不可改写；修正通过新 event 和 `supersedes/retryOf` 表达。高频 telemetry 可进入独立有界 runtime stream，但任何推进流程的事实必须持久提交。只有 committed、身份完整且 fingerprint/revision 匹配的事件能改变权威 projection。

---

## 15. Hook 层（三层拦截体系）

### 设计

各 Host Adapter 的 Hook 作为薄 Rust client 实现物理级流程纪律，覆盖工具调用拦截、上下文投影和响应后校验，不依赖 Agent 自律。Hook 只提交可信 session/turn/event 与工具事实，并读取 daemon 预计算 decision/projection；同步路径不得扫描文档、运行重 Gate、创建 child、执行项目 mutation 或自行编排业务流程。

Hook 默认处于 inactive。只有当前用户 turn 显式进入 `/ae-sdd`，或执行明确的 ae-sdd 写流程入口时，才创建 session 级 turn token；普通提示（包括 Story 文档对齐）不会解析或注入 Work Item。Stop 成功、fail-open 或下一条普通提示都会清理 token，Stop 阻断重试时暂时保留 token。token 不等同于 Work Item writer lease，也不读取项目级 state 作为激活信号。

- **PreToolUse**：Agent 每次调用工具前触发，daemon 按 session capability、当前 typed action 和授权路径裁决
- **UserPromptSubmit**：用户每次提交消息前触发，打开/恢复 session，提交 Hook event 并注入 daemon 已生成的 InstructionEnvelope/projection
- **Stop**：Agent 每次回复结束后触发，将结束事实交给 daemon 校验，不以自然语言完成声明推进状态

**决策 1B（v3.6）**：废弃 `◆ STATE:`/`◆ LOADED:` 自报标记检测（"防君子不防小人，可谎报"），改为纯产物核查（gates check 结果为唯一权威判定）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| Hook client | `bins/ae-sdd-cli` 与 Host Adapter 负责协议转换、runtime handshake、event ID 和有界超时；不承载业务 fallback |
| PreToolUse policy | `ae-sdd-policy/src/hook.rs` 提供工具/路径裁决基础；目标 authority 来自 daemon 当前 InstructionEnvelope/capability，而非单一 phase 字符串 |
| UserPromptSubmit | 精确 ae-sdd 触发创建 engaged turn，并从 daemon 获取 BootstrapAssessment 或当前 Series projection；普通 prompt 不激活 Work Item |
| Stop | 提交 stop event，daemon 校验 pending output、Gate、receipt 与重试预算；未证明完成时可阻断或返回 correction |
| Harness 安装 | 各 harness adapter 生成对应配置，必须从同一 dist/runtime contract 派生 |
| quick 兼容入口 | `/ae-sdd-quick` 只能选择更紧凑的交互/上下文 profile，不得跳过 RA、typed transition、写权限、测试或 Review |

**颗粒度与边界**：Hook 层只做协议、鉴权、物理拦截和 projection 交付，业务判断由 daemon 执行。inactive 请求可正常放行；engaged 状态下 daemon 不可用、协议不兼容或 decision 超时必须 fail closed，不能降级到本地旧逻辑。改 Hook 必须同步 harness 合同与生成物，禁止手工编辑已安装脚本。

---

## 16. 主流程监管器（typed Series 目标；基础部分实现）

> `ae-sdd-runtime/src/flow_supervisor.rs` 已具备偏移检测、暂停/恢复、矫正计数、事件和 Gate 基础；RA-first、生产 typed Series control、SeriesRun 子节点与按 Series 上下文尚未完整接通，因此本节不得再标记为“完整实现”。

### 设计

`/ae-sdd` 触发后，FlowRuntime/FlowSupervisor 负责全生命周期编排：初始化、启动评估、RA、工程路由、各 Series 循环执行、监督与收尾，不执行具体语义工作。它只裁决“下一步事务是什么、委派给谁、输入和权限是什么、结果是否有效”；Agent/SKILL 负责完成 daemon 已提交的事务，不能推进流程权威状态。

标准序列：工作区/Session 恢复 → BootstrapAssessment → Work Item/Flow Run → RA Series → EngineeringRoute/用户批准 → 按 scale 执行后续 Series → Coding/Test/Review → 完成判定。每个 Series 均由 `SeriesPlan -> InstructionEnvelope -> child claim -> progress -> ChildResult -> validated receipt` 闭环；有 Story 的分支必须按 `Story -> TestCase -> CodingPlan` 推进。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 偏移检测核心逻辑 | `ae-sdd-runtime/src/flow_supervisor.rs` 的偏移检测、产物核查和矫正次数升级基础 |
| phase→gate 映射表 | `flow_supervisor.rs` 的 phase→gate 映射，覆盖 ra-generated~test-running 各 phase |
| 升级判定 | `flow_supervisor.rs` 的升级判定，矫正次数 ≥ `CORRECTION_THRESHOLD_PAUSE`(3) |
| 矫正消息生成 | `flow_supervisor.rs` 的矫正消息生成，severity 1/2/3 三级文本模板 |
| gates check 子进程调用 | `flow_supervisor.rs` 进程内调用 `ae-sdd-gates`，有界超时；engaged 下不降级放行 |
| Hook 侧调用入口 | Rust Hook client/daemon service 调用 supervisor；engaged 下异常不静默降级，Hook 同步路径保持薄客户端 |
| 状态暂停/恢复 | `ae-sdd-flow/src/control.rs` 的 pause/resume |
| 矫正次数持久化 | `ae-sdd-flow/src/control.rs` 的矫正计数，写入 `state.correctionCounts` |
| 待接生产链路 | `SeriesPlan/ControlPlaneRuntime/ContextService/DelegationSupervisor` 必须成为唯一生产链路，替换 JSON handoff 与 phase-to-skill 推断 |

**颗粒度与边界**：监管器不执行业务工作，只做 typed 编排、授权、监督和校验；ae-sdd 自更新同样先 RA，再由监管器进入 Update Series，不能休眠或旁路。退出条件是 required Series、Spec、executionPlan、Gate、evidence、独立 Review、cleanup 和用户审核全部有效，或用户明确中止；单一 `phase=completed` 字符串不足以证明完成。

---

## 17. 流程偏移检测与矫正（基础已实现；typed 监督待接）

> 本节与 §16 共用 `flow_supervisor.rs` 的偏移、Gate 和矫正基础；现有 phase-based 检测尚未覆盖 SeriesRun/subnode、Spec graph 和 InstructionEnvelope，因此不是完整目标实现。

### 设计

识别 Agent 在执行过程中的物理越权和语义漂移，按 typed policy 拒绝、correction、retry 或 `awaiting_user/paused`。物理越权由 capability/PreToolUse 拦截；语义漂移由 FlowSupervisor 对照当前 SeriesPlan、允许子节点、required outputs、fingerprint、Gate 和 receipt 判断。

漂移类型至少包括：跳过 required Series（尤其绕过 RA 或 `Story -> TestCase -> CodingPlan`）、未授权操作/路径、错误 Story/TestCase/CodingPlan binding、输入或 policy stale、同一子节点超预算停滞、缺失 artifact/evidence/cleanup、伪完成和旁路。

矫正级别：Level 1 静默注入（用户不可见，AI 可感知）/ Level 2 矫正提示词（AI 须说明修复计划，同步骤最多3次）/ Level 3 人工升级（`state.phase=paused`，流程暂停待用户决策）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 漂移结果数据类 | `flow_supervisor.rs` 的 drift 结果类型：`drift_type/severity/gate_id/gate_passed/gate_message/phase/correction_count/message` |
| 阈值常量 | `flow_supervisor.rs`：矫正告警阈值 5 / 暂停阈值 3 / gate 检查超时上界 |
| phase-based 产物核查 | `detect_drift()` 依据 `get_phase_gate_map()` 跑 gates check，不信任 Agent 自报；保留为迁移基础，但需改为 SeriesPlan/subnode-aware |
| Layer 3 矫正次数升级（解 B2） | `detect_drift()` 行204：`correction_count >= CORRECTION_THRESHOLD_PAUSE` → severity=3 |
| severity=1 矫正文本 | `build_correction_message()` 行290-296：静默提醒 |
| severity=2 矫正文本 | `build_correction_message()` 行298-309：含矫正次数/失败门禁详情 |
| severity=3 暂停文本 | `build_correction_message()` 行311-323：含继续/跳过/回退/查看四个用户决策选项 |
| Series/subnode→Gate 映射 | 现有 9 phase map 只作兼容；目标由 versioned Series rules 显式挂载 RA/DR/Story/TestCase/CodingPlan/Coding/Test/Review Gate |
| Stop hook 职责精简 | `hook.stop` 路径：无自报检测，职责为防空响应 + 防无限循环 |
| 当前错误降级 | 现有异常返回 `drift_type="none"` 的行为与 engaged fail-closed 冲突，必须删除；ERROR/TIMEOUT/STALE 保持独立并阻断依赖推进 |

**颗粒度与边界**：不信任 Agent 自报；FlowSupervisor 保持 pure decision，WorkItemActor 经 lease/revision/fencing 提交状态。Gate `FAIL`、基础设施 `ERROR/TIMEOUT` 与输入 `STALE` 不得互相折叠。correction budget 耗尽后必须进入可恢复的暂停/人工升级状态，不能通过重复注入 prompt 或 fail-open 继续推进。

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

TestCase 是每个 Story 在 CodingPlan 前必经的独立子 Series，顺序固定为 `Story -> TestCase -> CodingPlan`。它只适用于含 Story 的中/大路线：中任务有一个对应 TestCase，大任务为每个 Story 分别创建 TestCase；微/小任务不创建 TestCase。TestCase receipt 必须绑定 Story identity 和内容 digest，未批准或 stale 时不得规划对应 CodingPlan。TestCase 是实现前的验证设计，不等于实现后的 Test Series、测试代码或 evidence manifest。

TestCase 内容质量以独立缺陷发现价值和可追溯证据衡量，不以测试总数、字段/状态数量或证伪用例比例衡量。生成阶段先从 Story 的 AC、契约、不变量、改动分支、历史缺陷、项目坑库和高影响风险建立有限风险登记，再按边界测试准入、行为等价类、最低充分层级和局部数量上限选择最小充分组合。

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
| 流程接入 | daemon 必须为每个 Story 创建独立 TestCase SeriesPlan/SeriesRun，并将 validated receipt 作为对应 CodingPlan 前置；当前生产 typed wiring 尚待迁移 |

**颗粒度与边界**：TestCase 的方法与内容判断仍主要由 Markdown SKILL/标准和独立 Review 承担；流程位置、Story 绑定、文档版本、Series 状态和 CodingPlan 前置条件必须由 daemon typed contract 与 Gate 强制。Monitor 只读投影。generate 与 review 双重检查选择证据，但“独立失败机制”的语义判断仍依赖 Agent；长期价值需通过真实项目的 TestCase 数量、维护耗时和缺陷发现率补充基线。

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
