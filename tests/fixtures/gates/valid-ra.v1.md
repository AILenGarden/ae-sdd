# RA-AE-SDD-RUST-DAEMON-001 — ae-sdd 全 Rust 多 Agent daemon 需求分析

## 0. 元信息

| 字段 | 值 |
| --- | --- |
| PRD | `PRD-AE-SDD-RUST-DAEMON-001` |
| Work Item | `PRD-AE-SDD-RUST-DAEMON-001` |
| 规模 | 大 |
| 类型 | 多 Agent 控制平面、状态机、并发存储、跨平台迁移 |
| 日期 | 2026-07-22 |
| 输入 | 用户对话、仓库代码盘点、operation protocol、CodingModel |

## 0.5 RequirementAnalysisModel 12 维决策记录（§0.5 RequirementAnalysisModel）

| 维度 | 结论 | 证据/动作 |
| --- | --- | --- |
| RA-01 目标 | 一个用户级 Rust daemon 服务多个 Agent；Agent 仅调用 CLI/Hook，入口自动确保 daemon ready 后获取裁决 | 来源：用户澄清“ae-sdd 被调用时检查主流程监控器，没启动就先启动，然后继续流程”与“全部用 Rust 重写” |
| RA-02 角色 | 用户、主会话、系列 Agent、任务/审查 Agent、FlowRuntime、HostRuntimeAdapter 与发行维护者权限分离 | 来源：PRD §2；主会话与 agent-orchestration contract |
| RA-03 主流程 | request→ensure-runtime(singleflight start/ready/replay)→handshake→role-aware nextAction→真实 delegation/host ACK→bounded result/collect→operation/gate→原子提交→event supervision→增量上下文/compact | 来源：PRD §5.1；用户 daemon 启动澄清；`source/standards/operation-protocol.md` |
| RA-04 异常 | 除 daemon/并发/Gate 异常外，伪 host ACK、越权 child、orphan、结果无效、event replay 与 compact timeout 均有 fail-closed 终态 | 来源：现有 subprocess/compact 路径缺少物理证明与 ACK |
| RA-05 数据 | 项目 state/artifact 保持业务权威；daemon 持久化 delegation、host action、context projection、compact cycle 与 supervisor checkpoint | 来源：现有布局与新三层执行合同 |
| RA-06 规则 | FlowRuntime 是唯一流程 reducer 与 writer；Agent 只提交 intent/result；engaged Hook、delegation、compact 异常均不得伪成功 | 来源：现有 phase 规则副本、Hook 放行、逻辑 spawn 与假 compact 缺陷 |
| RA-07 兼容 | 先冻结 golden corpus，再 shadow read/gate，再单 workspace canary；全过程禁止双写 | 来源：113 个 CLI 叶命令、18 operation、36 Gate、7 scanner 盘点 |
| RA-08 安全 | 本地 IPC 按 OS 用户隔离，并校验 endpoint token、allowed-root、symlink/junction containment | 来源：operation project/workItem 前置条件与 state store 路径约束 |
| RA-09 可观测 | request/session/turn/delegation/host-action/compact/work-item/revision/event/policy/input fingerprint 全链路关联 | 来源：stale gate、真实委派、compact 与恢复诊断需求 |
| RA-10 性能 | 100 session/10 workspace；warm handshake p95 50 ms，缓存 Hook RPC p95 50 ms，主会话 context 有硬预算 | 来源：用户“很多 Agent 同时使用”与更快 Hook/注入诉求；PRD §6 |
| RA-11 回滚 | 每 workspace canary 可退回 Python legacy；一旦切为 Rust sole-writer，该 workspace 不允许本地 fallback | 来源：迁移安全与唯一 writer 不变量 |
| RA-12 验收 | parity、并发/恢复、Gate 六态、真实三层委派、bounded context、event replay、host ACK compact 与发行扫描 | 来源：仓库盘点与 PRD §3.4~3.6 |

## 0.6 需求风险预判

| 风险 | 级别 | 处理 |
| --- | --- | --- |
| 大爆炸式改写造成行为遗漏 | 阻断 | 先冻结 corpus，按协议/存储/policy/runtime/工具/cutover 垂直切片 |
| daemon 单点异常导致 Agent 全部停滞 | 阻断 | 用户级服务、结构化健康检查、journal 恢复、显式 emergency 管理操作 |
| gate 计算完成前输入改变 | 阻断 | snapshot fingerprint + commit 前 revision/fencing/policy/inventory 复核 |
| actor 串行化被误当持久并发控制 | 阻断 | 保留跨进程 lock、lease TTL、CAS、单调 fencing 与 idempotency receipt |
| Python 与 Rust 双写造成分叉 | 阻断 | shadow 阶段只读/只比较；canary 切换使用 sole-writer 开关与 endpoint drain |
| 初始 constraints 指向 Java/Spring | 已修复 | 已替换为 Rust/runtime 项目约束；每次 Coding 仍动态执行 `get_constraints` |
| 初始本机缺 Rust toolchain | 已修复 | 已安装并验证 rustc/cargo 1.97.1 + MSVC linker；每次 shell 显式加载 PATH/DevShell并记录版本 evidence |
| 非核心 Monitor 被带入首轮扩大范围 | 严重 | `apps/ae-sdd-monitor/**` 整体延后到独立 WorkItem，本轮不修改、不迁移、不验证其行为，也不阻断核心 daemon cutover |
| daemon 将三层 Agent 压平，child 获得全局流程权限 | 阻断 | 角色、lineage 与 capability 由 daemon 签发并在每个 RPC 强制；FlowRuntime 是唯一 transition owner |
| delegation 有记录但宿主没有创建物理 session | 阻断 | HostRuntimeAdapter 必须返回可验证 childSessionId/attestation；自动化模式不支持时 fail closed |
| 主会话继续读取源码、完整系列文档或 child transcript | 严重 | 只投影 bounded ChildResult、交付物索引与 flow 摘要；full payload 留在 child artifact |
| compact trigger 被误当成 compact 已完成 | 阻断 | 只有匹配 session/contextGeneration 的 host ACK + rehydrate 才推进 CompactCycle |
| prompt 轮询驱动 correction，造成假停滞或重复纠偏 | 严重 | FlowSupervisor 只消费有序 committed event，并按 event/input/policy/decision digest 去重 |
| 多个 Agent 在 daemon 停止时同时调用造成启动风暴 | 阻断 | 独立 bootstrap lock + 锁内二次探活；只有持锁者 spawn，其他调用等待同一 endpoint ready |

## 1. 业务全景

ae-sdd 的产品边界从“每个 Agent 临时运行一组脚本”变为“本机多个 Agent 共享的工程控制平面”。价值不只是把 Python 换成 Rust，而是集中身份、状态、policy、Gate、缓存、事件和审计，使 Agent 的每个 turn 都能得到相同且可证明的合法下一步。来源：用户对话与 PRD §1。

“全部 Rust”指核心可执行控制面和工程工具全部由 Rust 实现：CLI、daemon、Hook adapter、state/artifact、operation、Gate、scanner、memory/evidence/review、assets、build/install/distribute。Markdown 方法论与模板保持数据资产；`apps/ae-sdd-monitor/**`、Electron main/preload、React renderer、CSS/Vite、Monitor daemon bridge、Monitor 打包与集成测试全部延后到独立 WorkItem。Python 仅在迁移测试环境充当 oracle，核心发行链路不加载它。

## 2. 角色枚举（§2 角色）

| 角色 | 目标 | 权限边界 | 证据 |
| --- | --- | --- | --- |
| 用户 | 配置允许的 workspace roots、批准计划、查看状态、执行受审计恢复 | 日常 Agent 调用不要求手工启动 daemon；只有用户可批准 executionPlan 与危险恢复 | 用户决定；AGENTS.md |
| 主会话 Agent | 面向用户推动流程、委派系列、收集有界结果与申请 transition | 不读取或生成系列交付物正文；不能直接推进 phase | `SKILL.full.md` 主会话边界 |
| 系列 Agent | 在独立 session 中拥有单一 RA/DR/Story/TestCase/Coding 系列并可继续委派 | 只能访问该系列 memory/路径/operation；不能批准计划、破坏 lease 或推进全局 phase | agent-orchestration contract |
| 任务/审查 Agent | 执行 assignment scope 内的修改、测试、证据或审查 | 只允许 capability 明列的路径、operation 与 deliverable | harness sub-agent contract |
| daemon / FlowRuntime | 统一裁决、确定性流程 reducer 与唯一 writer | 不能执行 LLM 语义工作，也不能把 ERROR/TIMEOUT 映射为 PASS | `gates.py` 与现有 flow monitor 缺陷 |
| HostRuntimeAdapter | 调用宿主原生 session/compact API 并验证 ACK | 不支持时显式返回 UNSUPPORTED；不能用 daemon 行代替真实 Agent session | host capability contract |
| CLI/Hook adapter | 适配参数与宿主 stdin/stdout | 不执行 policy、Gate、state mutation 或 Python fallback | 用户对话 |
| 发行维护者 | 构建、安装、分发、升级、迁移 | 管理操作也通过 Rust contract 并保留审计 | `scripts/` 与 distributor 盘点 |

## 3. 场景（§3 场景）

1. 正向：首次 CLI/Hook 调用发现 daemon 未运行，singleflight 拉起后等待 manifest + handshake ready，再由多个 Agent 注册 workspace、打开 session，并行查询不同 Work Item。
2. 同一 Work Item 竞争：两个 Agent 同时 mutation，持有有效 lease/fencing 且 revision 匹配的请求提交，另一个得到稳定冲突错误。
3. 同一请求重放：网络重试使用相同 idempotencyKey 与 payload，只返回先前 receipt；payload 改变则拒绝。
4. Hook 持续控制：UserPrompt 创建 turn，PreTool/PostTool/Stop 带相同 sessionId、turnId 与 hookEventId 通信。
5. daemon 断连：普通 CLI 返回 `DAEMON_UNAVAILABLE`；engaged PreTool deny，engaged Stop block，adapter 不运行本地业务逻辑。
6. gate 长任务：worker 基于不可变 snapshot 运行；返回 actor 后复核 revision、fencingToken、policyDigest、inventoryGeneration 与 fingerprint。
7. watcher 丢事件：workspace 标脏并全量 reconciliation；关键 commit 重新计算输入 fingerprint。
8. 外部改 state：hash 改变而 revision 未增长时进入 `EXTERNAL_STATE_CONFLICT`，等待受审计恢复。
9. daemon restart：PREPARED journal 恢复或中止；运行中的 gate job 标记 `ABORTED_RESTART`。
10. 协议升级：daemon drain 后切 endpoint；major 不相交的 client 得到 `PROTOCOL_VERSION_UNSUPPORTED`。
11. 迁移 shadow：Rust 与 Python 对同一 fixture 读取和评估，比较结果但只允许现有 writer 提交。
12. canary：按 workspace 切 Rust sole-writer；稳定窗口满足后切 Agent Hooks，最后删除 Python runtime。
13. 三层委派：主会话创建 series delegation；宿主创建物理 child session 并 ACK；child 接受 capability 后独立工作、报告，主会话只 collect 有界结果。
14. 子级委派：系列 Agent 创建 task/reviewer delegation；child 越界路径、全局 transition、计划批准与 lease break 请求稳定拒绝。
15. 结果回收：child 只提交固定预算摘要、deliverable path/hash/kind、finding 统计、nextActions 与 memory snapshot hash；daemon 先验证 artifact 再完成 collect。
16. 事件监督：FlowSupervisor 从持久 cursor 重放 committed event；重复 prompt、重复 event 和乱序通知不重复计算 correction。
17. 上下文快路径：mutation/event 提交后预计算 ContextProjection；Hook 以 contextRevision/digest 请求 delta，未变化返回空增量。
18. 主动 compact：压力事件建立 snapshot，HostRuntimeAdapter 请求宿主 compact；匹配 ACK 后 rehydrate，超时/错 session/错 generation 保持未完成。

## 4. 流程与状态机（§4 流程）

```text
daemon: stopped -> starting -> serving -> draining -> stopped
session: opening -> active -> closing -> closed
turn: created -> engaged -> stopped | blocked
operation: accepted -> prepared -> committed | rejected | aborted
gate-job: queued -> running -> pass | fail | error | timeout | cancelled | stale
delegation: requested -> spawning -> running -> result-staged -> artifacts-validated -> memory-cleaned -> completed | failed | expired | cancelled | orphaned
host-action: requested -> dispatched -> acknowledged | unsupported | timed-out | failed
compact-cycle: pressure-detected -> snapshot-ready -> compact-requested -> host-compacting -> host-acknowledged -> context-restored | unsupported | timed-out | failed
flow-supervisor: replaying -> caught-up -> deciding -> idle | degraded
workspace-mode: legacy -> shadow -> rust-canary -> rust-sole-writer
```

主流程：

1. `ae-sddd` 获取用户级单例锁，创建受用户权限保护的 endpoint manifest。
2. CLI 首次请求执行 `runtime.handshake`，协商 protocol、operation schema、capabilities、build 与 policy digest。
3. `workspace.register` 将 canonical root 映射为 workspaceId；`session.open` 绑定 agentId 与外部 conversation key。
4. `hook.user_prompt` 创建 turn；显式 Work Item 可绑定 session，但所有写 operation 仍携带 workItemId。
5. daemon 解析 operation registry，按 `requiresLease/requiresRevision/requiresIdempotency/requiresConfirmation` 验证 project scope 与对应前置条件；`lease.acquire`/`lease.break` 无 active-lease 前置但仍经跨进程锁、fencing/tombstone 与审计保护。
6. FlowRuntime 依据 session role、committed event cursor 与 policy 返回 nextAction；需要语义工作时创建带 scope 的 Delegation，并要求 HostRuntimeAdapter 的物理 session attestation。
7. series/task/reviewer 在独立 session 中执行并提交 bounded ChildResult；artifact hash、role scope、input revision/fingerprint 验证通过后，主会话才能 collect 并申请 operation/transition。
8. WorkItemActor 创建 snapshot；轻量 policy 进程内判定，重 Gate 提交 scheduler。
9. GateJob 返回后复核所有 freshness 字段；仅 fresh PASS 可进入 mutation，fresh FAIL 才产生业务 correction；ERROR 进入可诊断的基础设施阻断，TIMEOUT 按 retry policy 重调度或升级，CANCELLED 终止本次判定且不产生结论，STALE 丢弃结果并基于新 snapshot 重跑。
10. store 在 Work Item state directory 的 `mutation-journal/v1/` 原子写/fsync PREPARED typed entry，再完成同目录 staged write、atomic replace 与目录 fsync，最后原子写 COMMITTED receipt/event；SQLite 只在其后更新可重建 index。
11. daemon 仅在 COMMITTED project journal 后返回 receipt 并发布全局单调 eventSeq 的事件；FlowSupervisor 从 versioned payload 去重消费并预计算 role-aware ContextProjection。
12. Hook 按 contextRevision 获取 delta；达到上下文压力阈值时创建 CompactCycle，只有宿主 ACK 与 rehydrate 后才推进 generation。
13. CLI/admin 订阅者从 cursor 续传；cursor gap 时重新请求 snapshot。

## 5. 数据（§5 数据）

### 5.1 项目内权威数据

| 对象 | 关键字段 | 读写者 | 不变量 |
| --- | --- | --- | --- |
| WorkItemState | stateUuid, phase, revision, fencingToken, executionPlan, review | daemon store | revision 单调；mutation 只能来自 daemon |
| Lease | leaseId, owner, expiresAt, fencingToken, tombstone | daemon store | fencingToken release/break 后不重置 |
| ArtifactRef | intent, docId, path, sha256, source, version | artifact service | path 位于 allowed-root；hash 与落盘内容一致 |
| EvidenceManifest | evidenceId, kind, command, exitCode, input/output digest, timestamp | evidence service | 只记录真实执行证据 |
| ProjectEvent | eventStoreId, eventSeq, bootId, workspaceId, workItemId, revision, type, schemaVersion, payload/payloadRef, digest | daemon journal | committed 后才发布；eventSeq 在 eventStore 内跨 restart 全局单调；payload 可 typed replay |
| MutationJournal | schemaVersion, mutationId, operation, revision/fencing, target digests, typed event payload, status/timestamps | `<state-dir>/mutation-journal/v1/*.json` | project-authoritative PREPARED/COMMITTED/ABORTED；SQLite 丢失后重建 receipt/event index |
| ChildResultRef | delegationId, summaryDigest, deliverables, findingCounts, nextActions, memorySnapshotHash | daemon artifact service | 主会话只读取有界摘要和索引；deliverable hash 验证后可 collect |

### 5.2 daemon 元数据

| 对象 | 关键字段 | 存储 | 权威边界 |
| --- | --- | --- | --- |
| Workspace | workspaceId, canonicalRoot, projectKey, inventoryGeneration | SQLite WAL + 内存 actor | 项目 state 不由 SQLite 替代 |
| AgentSession | agentId, sessionId, externalConversationKey, role, rootSessionId, parentSessionId, delegationId, status, heartbeatAt | SQLite WAL | 仅运行生命周期；lineage 与 role 不可由 client 自报覆盖 |
| Turn | turnId, turnSeq, workItemBinding, engaged, capability, deadline | SQLite WAL | Stop 后关闭或阻断保留 |
| Delegation | delegationId, role, series/entity/task, inputRevision, inputFingerprint, allowedOperations, allowedPaths, requiredDeliverables, deadline, status, receipt | SQLite WAL + project audit summary | host attestation 前不得 running；collect 前必须验证 artifact |
| HostAction | actionId, kind, adapterId, sessionId, requestDigest, ackStatus, ackAt | SQLite WAL | spawn/compact/cancel/message 的真实宿主动作；ACK 必须绑定请求 |
| ContextProjection | sessionId, role, contextRevision, sourceRevision, digest, byteBudget, deltaRefs | SQLite/cache + artifact ref | root/series/task 投影隔离；未变化不重复注入 |
| CompactCycle | compactId, sessionId, snapshotRef, previousGeneration, nextGeneration, hostActionId, status, deadline | SQLite WAL | 未 host ACK + rehydrate 不得完成 |
| SupervisorCheckpoint | workspaceId, workItemId, lastEventSeq, lastEventDigest, stateRevision, inputFingerprint, policyDigest, lastDecisionDigest, health | SQLite WAL + project event summary | 全局 cursor replay 幂等；background ERROR 不改变业务 correction |
| OperationReceipt | idempotencyKey, payloadHash, revisionBefore/After, result | SQLite WAL + state mutation metadata | 同 key/不同 hash 拒绝 |
| GateJob | jobId, GateKey, snapshot fields, status, outcome | SQLite WAL | restart 后不宣称 PASS |
| RuntimeEvent | eventStoreId, eventSeq, bootId, workspaceId, workItemId, eventType, schemaVersion, payloadJson/payloadRef, payloadDigest, byteLen | SQLite WAL + immutable artifact ref | store-scoped 全局单调 cursor；typed bounded payload 可跨 restart replay，digest 不替代 payload |
| EndpointManifest | pid, bootId, eventStoreId, endpoint, endpointToken, protocolRange, policyDigest, capabilityKeyId, capabilityPublicKey | 用户 runtime dir | 明文 token 仅在 DACL/0600 保护的 manifest 与当前进程内存；日志/SQLite 只存 digest；public key 只用于验签 |

### 5.3 数据权威

- 项目 `state.json`、lease、文档、memory 与 evidence 是可移植业务真相。来源：现有仓库布局。
- daemon SQLite 是 session/turn/job/event/cache 的运行元数据，不可单独证明 Work Item 已推进。来源：唯一 writer 与项目可审计性要求。
- watcher event 是缓存失效提示；文件内容、revision 和 fingerprint 才是提交前证据。来源：跨平台 watcher 可能合并或丢事件。

## 6. 规则与约束（§6 规则）

| 规则# | 主规则 | 证据 |
| --- | --- | --- |
| R1 | released runtime 的业务执行路径必须是 Rust；不得调用 Python worker 或 fallback | 用户决定“全部用 Rust 重写” |
| R2 | 一个 OS 用户级 daemon 管理多个 workspace；workspaceId 由 canonical root 与 project identity 确定 | 用户的多 Agent 共享诉求 |
| R3 | CLI 与 Hook 不是 state writer，也不复制 TransitionPolicy | `state.py`、`gate_intercept.py`、`operations.py` 的规则副本证据 |
| R4 | 同一 Work Item mutation 由 actor 串行；registry 标记的 lease-protected write 必须通过 lease、CAS、fencing、idempotency 与跨进程锁，`lease.acquire`/`lease.break` 作为无 lease bootstrap/control write 仍在锁内维护单调 fencing/tombstone/audit | `state_store.py` 与 operation registry 现有契约 |
| R5 | gate 仅在 fresh PASS 时允许 transition；fresh FAIL 才可累计业务 correction；ERROR/TIMEOUT/CANCELLED/STALE 分别进入基础设施阻断、重试/取消或重跑语义 | `flow_monitor.py` 异常伪通过缺陷 |
| R6 | engaged turn 的 PreTool 与 Stop 在 daemon unavailable、deadline 或 protocol mismatch 时 fail closed；离线只读判断只接受 boot-scoped Ed25519 capability signature | 用户要求 daemon 控制会话且共享 HMAC 会允许 client 伪造 |
| R7 | shadow 比较阶段不写 Rust 项目状态；canary workspace 只允许一个 writer | 防双写与回滚要求 |
| R8 | 项目 state/artifact 保持文件权威；SQLite 只承载 daemon 元数据、journal 与缓存索引 | 可移植和 git 可审计要求 |
| R9 | 每次请求携带 requestId、workspaceId、agentId、sessionId、turnId、deadline；project-scoped 请求增加 workItemId，lease/fencing/revision/idempotency/confirmation 按 operation registry flags 增加 | operation protocol 与多 Agent 身份隔离 |
| R10 | Rust 删除旧实现前必须完成 113 command、18 operation、36 Gate、7 scanner 与 build/install/distribute parity | 仓库只读盘点 |
| R11 | FlowRuntime/WorkItemActor 是 phase、next-action、correction、pause/resume 与 transition 的唯一确定性 reducer | 用户要求主流程监督有效且减少误判 |
| R12 | 三层 Agent 的 role、lineage、allowed paths/operations 与 required deliverables 必须由 daemon capability 强制 | 既有主会话瘦身设计与 agent-orchestration contract |
| R13 | Delegation 只有在 HostRuntimeAdapter 返回可验证物理 child session ACK 后才能 active；自动化模式不允许伪逻辑 spawn | 用户要求多个 Agent 真实共享 daemon |
| R14 | 主会话只接收 bounded ChildResult 与 flow projection；完整源码、系列文档和 child transcript 不进入 root context | 用户避免主会话上下文膨胀的原始设计 |
| R15 | 主动 compact 只由 authenticated host token pressure sample 触发，并应用同 generation high/low watermark、连续样本与 cooldown；只有 snapshot、匹配 host ACK 与 rehydrate 全部完成后才能推进 context generation；不支持或无可信 telemetry 必须显式降级 | Hook 无法普遍证明宿主 compact 已完成，projection bytes 不能替代 token pressure |
| R16 | FlowSupervisor 只消费带 typed bounded replay payload 的有序 committed event；eventSeq 在 durable DB 内跨 restart 全局单调，并按 eventSeq/inputFingerprint/policyDigest/decisionDigest 去重；重复 prompt 不改变 correction | prompt 驱动监督会造成假停滞；digest-only/boot-reset cursor 无法可靠 replay |
| R17 | 所有 daemon-bound CLI/Hook 请求必须先尝试原请求；仅在 endpoint manifest 缺失或 transport unavailable 时执行一次 singleflight bootstrap，ready 后重放同一请求；protocol/auth/policy 错误不得通过重启循环掩盖 | 用户明确要求“ae-sdd 被调用时检查并按需启动”；多 Agent 并发要求避免启动风暴 |

## 6.5 衍生规则登记表（§6.5）

| 规则# | 主规则 | 机械追问 | 衍生规则 R' | 衍生规则说明 | 衍生模式命中 | 对应 AC |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | Rust-only runtime | Python oracle 会不会进入发行路径？ | R1.1 | oracle 仅由迁移测试 profile 调用；release artifact 扫描发现 Python 执行入口即阻断 | H.5.1 发布状态变更 | AC-001 |
| R2 | 用户级多 workspace daemon | workspace alias 或路径变体会不会串状态？ | R2.1 | 注册时 canonicalize 并校验 project identity；同根返回同 workspaceId，越界路径拒绝 | H.5.2 身份状态流转 | AC-002 |
| R3 | adapter 无业务规则 | CLI/Hook 能否在 daemon 断连时本地推导？ | R3.1 | adapter 只能校验 transport capability，不得评估 Gate 或 mutation state | H.5.3 权限变更 | AC-003 |
| R4 | mutation 并发控制 | daemon restart、lease bootstrap 或旧 client 如何避免 stale writer 与 acquire 死锁？ | R4.1 | registry 决定 lease/CAS 前置；跨进程锁、lease tombstone 与单调 fencing 在 actor 生命周期外持续生效，acquire/break 不要求已有 active lease | H.5.4 并发状态变更 | AC-004 |
| R5 | Gate freshness | Gate 计算后输入变化如何处理？ | R5.1 | commit 前复核 revision、fencing、policy、inventory 与 input fingerprint；变化则 STALE | H.5.5 Gate 状态流转 | AC-005 |
| R6 | Hook fail closed | 未受控普通会话会不会被全局锁死？ | R6.1 | daemon 签发短期 turn capability；未 engaged 可返回不受控，engaged 的异常路径必须 deny/block | H.5.6 会话状态变更 | AC-006 |
| R7 | 单 writer 迁移 | shadow 与 canary 如何切换？ | R7.1 | drain endpoint 后原子切 workspace mode，切换窗口禁止双方写入，回滚也经过 drain | H.5.7 发布状态变更 | AC-007 |
| R8 | 文件业务权威 | SQLite 与项目文件不一致信谁？ | R8.1 | 项目 revision/hash 为权威，元数据重建；外部无 revision 改写进入 conflict 状态 | H.5.8 恢复状态变更 | AC-008 |
| R9 | 请求身份完整 | 重放、无 lease bootstrap 与跨 turn 冒用如何阻断？ | R9.1 | hookEventId 去重，turnSeq 单调，project operation 显式 workItem，并按 registry 验证 lease/revision/idempotency；identity 不匹配拒绝 | H.5.9 权限状态变更 | AC-009 |
| R10 | parity 后删除 | 如何证明没有 stub-pass？ | R10.1 | 每个命令/Gate/scanner 都有 golden fixture、负例与 Rust evidence；缺项不允许删除旧实现 | H.5.10 发布状态变更 | AC-010 |
| R11 | 唯一流程 reducer | daemon restart 后同一事件会不会产生不同 nextAction？ | R11.1 | 相同 state/event/policy/input 重放得到同 decision digest；只有 root capability 可申请全局 transition | H.5.11 流程状态变更 | AC-013 |
| R12/R13 | 三层角色与真实委派 | child 能否伪造 lineage/session 或越权？ | R12.1 | capability 绑定 daemon-trusted lineage 与宿主 attestation；越界 operation/path/transition 稳定拒绝 | H.5.12 权限状态变更 | AC-014 |
| R14 | bounded root context | child 返回全文或超限摘要怎么办？ | R14.1 | daemon 截断到预算、保留 hash-addressed artifact ref，并验证 deliverable 后 collect | H.5.13 上下文状态变更 | AC-015 |
| R16 | event-driven supervisor | 重复 prompt/event 或 Gate 基础设施错误会否增加 correction？ | R16.1 | 去重 replay；只有 fresh FAIL 增加业务 correction；ERROR、TIMEOUT、CANCELLED、STALE 均不改变该计数 | H.5.14 监督状态变更 | AC-016 |
| R15 | acknowledged compact | 触发命令是否等于完成？ | R15.1 | 错 session/generation、重复、超时或缺失 ACK 均不推进 generation；恢复从 snapshot 校验 | H.5.15 会话状态变更 | AC-017 |
| R17 | runtime 自动引导 | 并发首次调用是否会 spawn 多个 daemon，或把协议错误误当未启动？ | R17.1 | bootstrap lock 内二次探活；只有 manifest/transport unavailable 可触发 spawn；握手成功后只重放一次原请求 | H.5.16 运行时状态变更 | AC-018 |

## 7. 设计方向与方案比较（§7 设计方向）

| 方案 | 优点 | 缺点 | 结论 |
| --- | --- | --- | --- |
| 每个 Agent 内嵌 Rust library | 单次调用快 | 多份进程状态、重复 watcher/cache，无法统一 session 与 writer | 否决 |
| Java 常驻服务 | 工具生态成熟 | 启动与内存成本高，单 binary 分发和本地 IPC 适配较重 | 否决 |
| C++ daemon | 性能和系统 API 控制强 | 内存安全、并发状态机与跨平台依赖维护成本更高 | 否决 |
| Rust daemon + 薄 Rust CLI | 单 binary、内存安全、强类型协议、Tokio 并发与跨平台本地 IPC适配 | 初期迁移量大，编译与依赖治理需要新增约束 | 采用 |

设计方向：每个 OS 用户一个 `ae-sddd`；Windows Named Pipe，macOS/Linux UDS，默认不监听 TCP 端口；length-prefixed UTF-8 JSON-RPC 2.0；daemon-bound 请求采用 call-first/recover-once，缺失 endpoint 或 transport unavailable 时由 CLI composition singleflight 拉起 daemon、等待 ready 并重放原请求；WorkspaceActor + WorkItemActor；Gate worker 使用 snapshot/commit freshness；project file truth + SQLite WAL metadata；CLI/admin 可订阅诊断事件。

## 8. 验收标准 AC（§8 AC）

| AC | Given | When | Then |
| --- | --- | --- | --- |
| AC-001 | release profile 完成构建 | 扫描二进制、安装包与 hook 配置 | 不含 Python worker/fallback/脚本业务入口；CLI/Hook 只连接 daemon |
| AC-002 | 10 个 workspace、100 个 Agent session | 并发注册、查询与 heartbeat | workspace/session 隔离，canonical alias 不生成重复 workspace |
| AC-003 | daemon 启动失败、超时或协议 major 不兼容 | CLI、Hook 发起请求 | bootstrap 失败后普通 CLI 结构化非零；engaged Hook deny/block，不本地裁决；协议错误不循环重启 |
| AC-004 | 两个 client 竞争同一 Work Item，且含旧 lease/revision/fencing 请求 | 并发 mutation 与 daemon restart | 仅一个合法提交；stale 请求稳定拒绝；fencing 不重置 |
| AC-005 | GateJob 运行中 state、policy 或输入文件变化 | job 返回并尝试 transition | 结果标记 STALE 或重跑，旧 PASS 不提交 mutation |
| AC-006 | engaged 与未 engaged turn 各一组 | daemon timeout、PreTool、Stop | engaged 写工具与 Stop 阻断；未 engaged 仅凭有效 capability 返回不受控 |
| AC-007 | workspace 处于 legacy/shadow | 执行 canary 切换、回滚与 daemon upgrade | drain 后原子切 mode，全程没有 Python/Rust 双写 |
| AC-008 | daemon 在 PREPARED、Gate running、event publish 前后被 kill | restart 与 reconciliation | state 保持可解析；committed mutation 可重放 receipt；中断 Gate 不成为 PASS |
| AC-009 | 重复 hookEventId/idempotencyKey、跨 turn 与跨 workspace 冒用 | 请求执行 | 重复无副作用；payload/identity 不同返回稳定冲突/鉴权错误 |
| AC-010 | parity corpus 覆盖全部现有能力 | 运行 Rust/Python differential 与 Rust native suite | 113 command、18 operation、36 Gate、7 scanner、工程工具逐项有 PASS 证据且无 stub-pass |
| AC-012 | Windows/macOS/Linux 与 CI runner | 安装、启动、drain、停止、升级、卸载 | 生命周期契约一致，endpoint 权限只允许当前 OS 用户 |
| AC-013 | 同一 Work Item 的 state revision、event log、policy digest 与 input fingerprint 固定 | restart/replay 并由 root/child 分别请求 transition | decision digest 与 nextAction 确定一致；只有 root intent 可进入合法 transition，child 请求被拒绝 |
| AC-014 | main→series→task/reviewer 的 delegation lineage 与各宿主 capability matrix | spawn、accept、report、collect，并注入伪造/重复/超时 ACK 与越界操作 | 只有已 attested 物理 session 可 running；scope/lineage 强制；自动化不支持时 fail closed |
| AC-015 | child 产生超限摘要、完整 transcript 与多个 deliverable | report/collect/context injection | root 只收到预算内摘要和 path/hash/kind 索引；artifact hash 校验；series/task memory 不泄漏到 root；缓存 Hook RPC p95 不超过 50 ms |
| AC-016 | 重复/乱序 committed event、重复 prompt 与 Gate 六类 outcome | FlowSupervisor replay/decision | 相同输入只产生一次 decision；只有 fresh FAIL 增加 correction；ERROR/TIMEOUT/CANCELLED/STALE 不误判 |
| AC-017 | 宿主提交连续高/低 token pressure sample，并覆盖支持、不支持、超时、错误 session/generation/cooldown 场景 | sample→hysteresis trigger→snapshot→compact request→ACK→rehydrate | 达到 policy 才主动触发且不抖动；仅匹配 ACK+rehydrate 后进入 context-restored 并推进 generation；无 telemetry/不支持返回 UNKNOWN/UNSUPPORTED/manual 或 rotate，不伪造成功 |
| AC-018 | Windows 上 daemon 未运行，且 32 个 CLI/Hook 同时首次调用 | 发起 daemon-bound 请求 | 只产生一个 serving `ae-sddd`；全部调用等待同一 endpoint ready 后继续原请求；无 orphan daemon、Python fallback 或 TCP listener |

## 8.5 衍生 AC 登记表（§8.5）

| 衍生 AC | 对应规则 R' | Given | When | Then | 验证入口 |
| --- | --- | --- | --- | --- | --- |
| AC-001 | R1.1 | release artifact | 执行依赖与入口扫描 | Python 业务入口计数为 0 | release artifact scan |
| AC-002 | R2.1 | 路径 alias 与多个 project | workspace.register | canonical identity 稳定且隔离 | workspace property tests |
| AC-003 | R3.1 | daemon 断连 | CLI/Hook 请求 | adapter 不执行 Gate/state 推导 | transport negative tests |
| AC-004 | R4.1 | 竞争 writer 与 restart | mutation | lease/CAS/fencing/lock 保持不变量 | concurrency + crash tests |
| AC-005 | R5.1 | Gate snapshot 已建立 | 输入或 policy 改变 | stale PASS 不提交 | TOCTOU deterministic tests |
| AC-006 | R6.1 | 两类 turn capability | Hook 断连或 deadline | engaged 阻断，未 engaged 按 capability 处理 | hook replay |
| AC-007 | R7.1 | legacy/shadow workspace | drain/canary/rollback | sole-writer 原子切换 | migration E2E |
| AC-008 | R8.1 | SQLite 与 project file 不一致 | restart/reconcile | project revision/hash 胜出，冲突显式 | recovery tests |
| AC-009 | R9.1 | 重放与身份冒用输入 | operation/hook execute | 去重或稳定拒绝 | protocol security tests |
| AC-010 | R10.1 | 完整 inventory manifest | parity suite | 每项映射 fixture、负例与证据 | parity matrix audit |
| AC-013 | R11.1 | 固定 flow snapshot/event log | replay 与角色 transition 请求 | decision 确定且 transition owner 唯一 | reducer property/replay tests |
| AC-014 | R12.1 | 三层 delegation 与 host action | spawn/ACK/越权/超时 | 真实 session、lineage 与 scope 均被验证 | host attestation + role negative matrix |
| AC-015 | R14.1 | 超限 child output | report/collect/inject | root budget 不超限、artifact 可追溯且 Hook 命中延迟达标 | bounded result/context isolation/load tests |
| AC-016 | R16.1 | event 与 Gate outcome 矩阵 | replay/supervise | decision 去重且 correction 语义正确 | event replay/outcome property tests |
| AC-017 | R15.1 | compact capability/ACK 矩阵 | request/timeout/restart/rehydrate | generation 只在有效 ACK 后推进 | compact lifecycle contract tests |
| AC-018 | R17.1 | stopped/healthy/broken/protocol-error daemon 与并发 callers | CLI/Hook request | stopped 自动拉起并重放；healthy 不 spawn；失败 fail closed；并发仅一个 serving instance | Windows native cold-start integration |

## 8.6 衍生覆盖率（§8.6）

| K/M | 覆盖率 | 结论 |
| --- | --- | --- |
| 15/15 | 100% | 15 条衍生规则均在 §8.5 映射到独立验证入口 |

本轮 AC-001 至 AC-010、AC-012 至 AC-017 均在 Story 验证矩阵中映射自动化命令或跨平台 E2E；AC-011 随 Monitor 整体延后，不属于本轮验收。当前范围内验收覆盖率目标为 100%。

## 9. 隐性假设与证伪（§9 假设）

| 假设 | 风险 | 证伪方式 |
| --- | --- | --- |
| 一个用户级 daemon 足以覆盖常见本地 Agent 场景 | 多用户或容器隔离需求冲突 | 多 OS 用户与 CI 前台实例测试；endpoint scope 明确到用户 |
| JSON-RPC framing 满足 CLI、Hook 与 admin client | 大 prompt 或 event backpressure 破坏延迟 | 1/16/64 MiB payload、并发响应、慢订阅者测试 |
| 项目文件继续作为业务真相可支持并发 | 外部编辑与网络盘语义造成冲突 | external mutation、atomic replace、reconciliation、网络盘降级测试 |
| Rust 可覆盖 Python 动态 plugin 行为 | 动态 import 语义难以直接复制 | 将 plugin contract 数据化并用 content scanner/golden fixture 验证 |
| 100 session/10 workspace 是首版合理容量 | 真实使用超过预算 | 压测记录 queue、CPU、内存、p95/p99，并配置有界限流 |
| 旧 Python 可作为兼容 oracle | 旧实现本身包含缺陷 | golden 标注 preserve/fix；Hook 异常放行等列为 breaking fix |

## §9-bis 业务模式匹配表

H.5 模式全检覆盖下列六项，每项均给出适用编号或具体不适用理由。

| 套用的模式 | 命中的衍生影响编号 | 转化为规则/AC | 备注（不适用理由） |
| --- | --- | --- | --- |
| 账号状态变更 |  |  | 不适用：daemon session 不读取或修改业务账号状态 |
| 订单状态变更 |  |  | 不适用：本需求不含订单实体与订单流程 |
| 支付状态变更 |  |  | 不适用：本需求不含支付实体与支付协议 |
| 登录态变更 | R2.1, R6.1 | AC-002, AC-006 | 适用：OS 用户 endpoint 与 Agent session capability 具有身份生命周期 |
| 权限变更 | R3.1, R9.1 | AC-003, AC-009 | 适用：CLI/Hook 权限收敛且写请求增加身份前置条件 |
| 定时任务状态 | R5.1, R8.1 | AC-005, AC-008 | 适用：Gate scheduler、reconciliation 与 restart job 具有显式状态 |

## §9-ter 跨域级联效应表

| 触发动作 | 受影响域 | 受影响状态机/事件/缓存/MQ | 时效要求 | 对应 AC |
| --- | --- | --- | --- | --- |
| workspace 注册 | protocol/runtime/store | 状态机：opening→active；事件：WorkspaceRegistered；缓存：inventory generation 建立；MQ：不发布 broker 消息，使用进程内 event bus；聚合根：WorkspaceActor | warm p95 100 ms 内 | AC-002 |
| Gate 完成 | policy/gates/store | 状态机：running→pass/fail/stale；事件：GateCompleted；缓存：GateKey singleflight 失效；MQ：不发布 broker 消息，使用持久 eventSeq；聚合根：WorkItemActor | 结果返回后同一提交临界区复核 | AC-005 |
| Hook 断连 | client/runtime/policy | 状态机：engaged→blocked；事件：TransportUnavailable；缓存：turn capability 按 expiry 失效；MQ：不发布 broker 消息；聚合根：SessionActor | PreTool/Stop deadline 内返回宿主 JSON | AC-003, AC-006 |
| canary 切换 | runtime/build/integrations | 状态机：shadow→rust-canary→rust-sole-writer；事件：WorkspaceModeChanged；缓存：policy/inventory cache 全量失效；MQ：不发布 broker 消息；聚合根：WorkspaceActor | drain 完成后单次原子切换 | AC-007 |
| daemon restart | runtime/store/inventory | 状态机：starting→serving；事件：RecoveryCompleted；缓存：全量重建；MQ：不发布 broker 消息，CLI/admin 订阅者按 cursor 续传；聚合根：Runtime | 启动预算 5 秒内或返回健康失败 | AC-008, AC-012 |
| delegation spawn/collect | flow/delegation/host/context | 状态机：requested→running→validated→completed；事件：HostActionAcked/ChildResultValidated；缓存：role projection；MQ：不发布远程 broker 消息，使用 durable runtime event/outbox；聚合根：WorkItemActor | ACK/claim/deadline 内，未 attested 不运行 | AC-013, AC-014, AC-015 |
| committed flow event | flow/supervisor/policy | 状态机：replaying→deciding→idle；事件：FlowDecisionCommitted；缓存：decision digest；MQ：不发布远程 broker 消息，使用 per-WorkItem durable outbox/mailbox；聚合根：WorkItemActor | commit 后异步快速收敛，不依赖 prompt | AC-016 |
| compact request | context/host/runtime | 状态机：authenticated sample→hysteresis/cooldown→pressure-detected→snapshot→host ACK→rehydrate；事件：PressureObserved/CompactRequested/Acknowledged/Restored；缓存：snapshot/projection generation 在 rehydrate 后失效并重建；MQ：不发布远程 broker 消息，使用 SessionActor host-action outbox；聚合根：SessionActor | sampleSeq/generation 单调；deadline 内，否则显式 unknown/unsupported/timeout | AC-017 |

## 9-quater 实现视角七要素

### 9-quater.1 数据源清单（I1）

| 来源类型 | 名称 | 读/写 | Owner | 权威源 | 证据 |
| --- | --- | --- | --- | --- | --- |
| 文件 | state、lease、doc、memory、evidence、constraints、templates | 读写 | daemon store/artifacts | 项目文件 | `.auto-engineering`, `ae-sdd-doc`, `.ae-sdd` |
| API/接口 | JSON-RPC request/response、Hook stdin/stdout、CLI/admin snapshot/event | 读写 | protocol/runtime | versioned schema | operation protocol、现有 Hook contract |
| 宿主 API | session create/send/wait/cancel/attest、token pressure telemetry、Pre/PostCompact 或 app-server compact | 读写 | host adapters | 宿主返回的 identity/ACK/sampleSeq/generation | `.harness/agent.md` 与宿主 capability matrix |
| 配置 | project config、policy、plugin registry、daemon endpoint manifest | 读写 | config/runtime | schema + policy digest | `.ae-sdd/config.yaml`, source standards |
| 第三方进程 | git、toolchain、test runner、OS service manager | 读/执行 | integrations/build | 实际 exit code 与 stdout/stderr digest | 现有 `git_insight.py`, build/install scripts |
| 日志/审计/指标 | request trace、operation receipt、Gate evidence、event cursor | 写 | runtime/evidence | committed journal | `.ae-sdd/runtime-stats`, evidence manifest |
| 缓存/SQLite | inventory、session/turn/delegation/host-action/context/compact/supervisor、singleflight result | 读写 | runtime | daemon 可重建或由 project event 恢复的运行元数据 | 新 Rust runtime 设计 |

数据库业务表、Redis 业务 key、对象存储与远程 MQ 不成为本需求的项目状态权威；如 external tool 调用出现，只记录输入输出 digest 与 evidence。

### 9-quater.2 数据流链路（I2）

| 来源 | 入口 | 处理服务/领域 | 持久化/落点 | 输出/观测 | 事务与一致性 |
| --- | --- | --- | --- | --- | --- |
| Agent 参数 | CLI/Hook API | protocol handshake + runtime routing | session/turn SQLite metadata | JSON-RPC response + trace | requestId/hookEventId 去重 |
| 主会话 intent | delegation.create | FlowRuntime + DelegationService + HostRuntimeAdapter | delegation/host-action/context projection | attested child session 或 UNSUPPORTED | lineage/capability/ACK/claim 校验 |
| child result | delegation.report/collect | role scope + artifact validation | ChildResultRef + deliverable artifacts | bounded root projection + nextAction | size/path/hash/revision 一致后才 collect |
| operation request | operation.execute | policy + WorkItemActor | project state/artifact + receipt | revisionAfter + eventSeq | lease/CAS/fencing + PREPARED/COMMITTED |
| workspace 文件 | inventory watcher/API | selector + fingerprint + Gate scheduler | cache index/GateJob metadata | GateResult/evidence | watcher 仅失效提示，commit 前重算 |
| CLI/admin 诊断请求 | workspace.snapshot/events.subscribe | runtime projection | event cursor | structured snapshot/event | cursor gap 重新 snapshot |
| build/admin 命令 | CLI RPC | build/integrations job | generated dist/install target | job status/evidence | allowed-root + staging + atomic promote |
| context pressure/event | host.pressure_report/compact.request/context.get | ContextService + HostRuntimeAdapter | PressureSample/CompactCycle/snapshot/projection | delta context 或 context-restored compact | sampleSeq/generation/hysteresis/cooldown/ACK 相关性校验 |

数据流为 source → ingress/API → domain/service → persistence/cache → output/observability；项目 mutation 在一个 WorkItemActor 与 StateStore 提交边界内保持一致性。

### 9-quater.3 术语、定义与不变量（I3）

| 术语/字段 | 定义/枚举 | 空值与 ID 规则 | 不变量 | 权威源 |
| --- | --- | --- | --- | --- |
| agentId | Agent 类型与稳定实例身份 | 必填，不等于 sessionId | 不授予 Work Item 写权限 | protocol |
| sessionId | 一次 Agent conversation 的内部 UUID | daemon 生成；external key 可空 | workspace 内唯一；物理 child 需宿主 attestation | runtime metadata |
| role/lineage | root、series、task、reviewer 与 root/parent/delegation identity | daemon 派生，不接受 client 覆盖 | depth<=2；reviewer 与被审 worker session 不同 | delegation capability |
| delegationId | 一次有边界的语义工作委派 | daemon 生成；child 必填 | host ACK/claim 前不 running；result 校验前不 completed | runtime + project receipt |
| turnId/turnSeq | UserPrompt 创建的轮次身份 | engaged Hook 必填 | turnSeq 单调，Stop 后关闭或保持 blocked | runtime metadata |
| workItemId | 项目状态机业务身份 | `requiresWorkItem=true` 时必填 | session binding 不替代显式字段；workspace/runtime bootstrap 不要求 | project state |
| revision/fencingToken | 乐观版本与 writer 世代 | 分别按 requiresRevision/requiresLease flag 必填 | 两者单调；stale 值拒绝；lease acquire/break 不要求已有 token | project state/lease |
| policyDigest | TransitionPolicy 编译内容 hash | handshake/Gate snapshot 必填 | commit 时必须匹配 | daemon binary |
| GateStatus | PASS/FAIL/ERROR/TIMEOUT/CANCELLED/STALE | 不可空 | enforce 仅 fresh PASS 放行；仅 fresh FAIL 增加业务 correction | gate contract |
| contextRevision/generation | 角色投影版本与宿主 compact 世代 | Hook/compact 必填 | delta 单调；未匹配 ACK 不推进 generation | context/compact metadata |
| workspace mode | legacy/shadow/rust-canary/rust-sole-writer | 每 workspace 必填 | shadow 不写，sole-writer 不 fallback | migration state |

时间统一为 UTC ISO-8601，duration 使用毫秒；path 保存 canonical absolute root 与项目相对 artifact path，空值不得被解释为默认 Work Item。

### 9-quater.4 现有实现与复用证据（I4）

| 对象 | 代码/路径证据 | 结论 | 理由 |
| --- | --- | --- | --- |
| operation envelope 与 18 operation | `source/standards/operation-protocol.md`, `tools/lib/operations.py` | 契约复用，Rust 新建实现 | 已有 version/flags/error/confirmation 语义 |
| lease/CAS/fencing/idempotency | `tools/lib/state_store.py` | 行为复用，Rust 新建实现 | 并发不变量已有测试证据 |
| phase/route/state invariants | `tools/lib/state.py` | 规则复用并收敛为 domain/policy SSOT | 避免四份 flow table |
| 36 Gate 与 GateResult | `tools/lib/gates.py` | 逐 Gate 改造迁移 | 保留外部字段，修复 error-as-pass |
| 7 scanner | `scripts/*_scan.py` | 算法迁移到 Rust registry | 消除子进程与动态 import |
| artifact/memory/evidence/assets | `tools/lib/document_storage.py`, `memory_store.py`, `evidence.py`, `project_assets.py` | 分模块改造迁移 | 保持项目文件格式与路径契约 |
| build/install/distributor | `scripts/`, `scripts/distributors/` | Rust build crate 新建，输入输出契约复用 | released toolchain 不能依赖 Python |
| project assets | `ae-sdd assets read`, git 代码反查 | 作为迁移 inventory 与 golden source 复用 | 每个 capability 绑定路径、fixture、owner |
| subprocess/memory/compact 旧路径 | `tools/bin/ae-sdd`, `state.py`, `prompt_inject.py`, `memory_store.py`, `context_pressure.py` | 仅作 breaking-fix 与迁移 oracle，不复用控制语义 | 当前仅登记 logical spawn、role-blind memory 且 compact 无 host ACK |

### 9-quater.5 高成本或难实现设计反驳（I5）

| 方案 | 高成本/风险 | 不采用理由 | 更低成本替代方案 |
| --- | --- | --- | --- |
| 一次提交删除全部 Python | 行为遗漏、回滚困难、dirty worktree 冲突 | 无法证明 113 命令与 36 Gate parity | 分阶段 shadow/canary，最后独立删除 slice |
| 每个 Agent 内嵌完整 runtime | 重复 watcher/cache/SQLite 与内存，写冲突增加 | 违背共享 daemon 目标 | 薄 client + 用户级 daemon |
| 全部状态只放 SQLite | git 不可审计、项目不可移植、离线恢复困难 | 破坏现有 artifact 契约 | project file truth + SQLite runtime metadata |
| gRPC/HTTP loopback 首版 | codegen/TCP 暴露/端口与代理成本 | CLI/Hook/admin 只需要本机 typed RPC | framed JSON-RPC over Named Pipe/UDS |
| Gate 在 actor mailbox 内同步执行 | 长任务阻塞同 Work Item heartbeat 与取消 | 并发与延迟不可控 | snapshot job + return-time freshness validation |
| 逐行翻译 1564 个 Python tests | 维护成本高且固化旧缺陷 | 测试数量不等于契约覆盖 | golden differential + Rust unit/property/concurrency/E2E |
| daemon 自己扮演所有子 Agent | 无独立 LLM context/identity，无法证明三层执行 | 破坏主会话瘦身与审查隔离 | HostRuntimeAdapter 创建真实 session，daemon 只控制 lifecycle/capability |
| Hook 直接执行 spawn/scan/compact 长动作 | deadline 抖动、递归扫描与宿主状态不可知 | 无法满足快速 Hook 和可靠 ACK | Hook 快路径只入队/读投影，长动作由后台 actor + host action 完成 |

### 9-quater.6 开发者疑问答复矩阵（I6）

| 开发者疑问？ | 答案 | 证据 | 状态 | 是否阻断 DR |
| --- | --- | --- | --- | --- |
| daemon 是每 workspace 一个还是全局一个？ | 每 OS 用户一个，内部按 workspace actor 隔离 | 用户多 Agent 诉求、PRD F1 | 结论明确 | 否 |
| CLI 可以在 daemon 断连时直接读 state 吗？ | 不可以；普通 CLI 返回结构化错误，Hook 按 engaged contract 阻断 | R3/R6、AC-003/006 | 结论明确 | 否 |
| Monitor 是否阻断本轮 daemon cutover？ | 否；整个应用、bridge、打包与集成测试延后到独立 WorkItem | 用户 2026-07-23 范围决定 | 结论明确 | 否 |
| actor 已串行，为何还保留 lease/CAS/fencing？ | restart、旧 client、外部进程与 split-brain 仍需持久防护 | R4、AC-004 | 结论明确 | 否 |
| Rust 可以在 Python parity 前开始删旧代码吗？ | 不可以；删除依赖 inventory 完整与对应 evidence | R10、AC-010 | 结论明确 | 否 |
| 初始 Java/Spring constraints 是否仍阻断？ | 否；已替换为 Rust/runtime constraints，但每次 Coding 必须动态重新加载 | 风险表、PRD §8、当前 constraints | remediation 已完成 | 否 |
| Rust toolchain 是否可用？ | 是；rustc/cargo 1.97.1 与 MSVC linker 已 smoke 验证，命令环境需显式加载 DevShell/PATH | toolchain smoke evidence | remediation 已完成 | 否 |
| 流程执行者还是 SKILL 吗？ | 不是；SKILL 是声明式方法/模板/输出合同，FlowRuntime 是确定性执行与监督者，Agent 是语义 worker | R11、DR 决策 9 | 结论明确 | 否 |
| 主会话会不会再次读完整 child 内容？ | 不会；daemon 只提供 bounded ChildResult 与索引，full content 留在 artifact | R14、AC-015 | 结论明确 | 否 |
| daemon 能否主动 compact？ | 可；仅依据 authenticated host token pressure sample 经 watermark/hysteresis/cooldown 触发，且只有匹配 ACK + rehydrate 才进入 context-restored；无 telemetry/不支持显式降级 | R15、AC-017 | 结论明确 | 否 |

### 9-quater.7 DR 生成交接包（I7）

| DR 输入 | 内容 |
| --- | --- |
| 接口/API | 原 SPI + exact `<domain>.<verb>` methods；`flow.next`, `flow.snapshot`, `delegation.*`, `host.action_next`, `host.action_ack`, `host.pressure_report`, `context.get`, `context.project`, `compact.request`, `compact.status` |
| 数据模型/表 | project state/lease/artifact/evidence/event/mutation-journal；SQLite workspace/session/turn/delegation/request+cleanup+Hook receipts/host-action/context/compact/supervisor/job/cache index |
| 状态/事务/一致性 | daemon/session/turn/operation/gate/delegation/host-action/compact/supervisor/workspace-mode；actor + registry-scoped lease/CAS/fencing；project-authoritative PREPARED/COMMITTED journal |
| 非功能/性能/权限 | 100 session/10 workspace；Hook/context/child-result 硬预算；role grants；OS 用户 IPC ACL；token；allowed-root；日志脱敏 |
| 测试/验收 | Rust unit/property/reducer replay/role negative/host attestation/bounded result/compact ACK/golden/differential/concurrency/crash/hook load/cross-platform |
| 迁移/回滚/灰度 | freeze→domain/store→Gate/scanner→daemon/CLI→artifact/build→service/canary→delete Python；shadow 禁止双写 |

## 10. 缺口与未决问题（§10 缺口）

| 缺口 | 级别 | 处理与状态 |
| --- | --- | --- |
| 初始 `constraints/*.md` 是 Java 8/Spring Boot 约束 | 已关闭 | 已替换 Rust daemon/CLI/tooling 约束；当前 revision 重新调用 `get_constraints` 后才可 Coding |
| 初始本机缺少 Rust/MSVC toolchain | 已关闭 | 已安装 rustc/cargo 1.97.1 与 VS Build Tools，并完成 link smoke；PATH/DevShell 进入每次命令 evidence |
| 现有 worktree 在 runtime/docs/scripts/tests 有大量用户修改 | 严重 | 已定位；每个 slice 前读取 diff，新增 Rust 路径优先，重叠文件逐项合并 |
| 113 CLI 命令尚未形成机器可读 parity manifest | 严重 | Story 1 从 parser/operation registry 生成 inventory 与 golden corpus |
| breaking fix 与 preserve 行为尚未逐项标注 | 严重 | Story 1 建 compatibility classification；至少包含 Hook fail-open 与 Gate error-as-pass，明确排除 `apps/ae-sdd-monitor/**` |
| Rust 依赖与 MSRV 供应链策略尚未写入项目约束 | 严重 | constraints remediation 中确定 stable channel、deny/audit、license 与 lockfile 策略 |

历史阻断的 constraints 与 toolchain remediation 已完成；正式文档、executionPlan、当前 revision gates 必须在进入 Coding 前重新确认，静态文字不作为证据。

## 11. 规模裁定（§11 规模）

| 维度 | 评分（1-4） | 说明 |
| --- | --- | --- |
| 功能/角色范围 | 4 | daemon、多个 Agent、CLI/Hook、build/install/distribute |
| 接口变更 | 4 | 新本地 RPC、session/turn、event stream，同时保持 18 operation |
| 架构决策 | 4 | 唯一 writer、actor/scheduler、file truth + SQLite metadata、跨平台 IPC |
| 数据变更 | 4 | state/lease compatibility、journal、runtime metadata、event cursor |
| 测试层级 | 4 | golden/differential/property/concurrency/crash/cross-platform/pressure |

规模为“大”；路由决策为 RA → DR → program Story（6 个迁移 slice，T4 细分 Runtime/Delegation/Context）→ compact executionPlan → 用户批准 → 分阶段 Coding/Test/Review。采用 strangler/canary 迁移，不采用一次性替换。

## 12. 5 问自检记录

1. 谁受益：同时运行 Codex、Claude Code、ZCode、Harness、Hermes 或多个同类 Agent 的工程用户。
2. 何时失败：daemon 断连、协议错配、writer 竞争、Gate stale、外部改 state、restart、watcher overflow、parity 漏项。
3. 如何证明：request/receipt/event/journal、revision/fencing、golden diff、Hook replay、跨平台压力与 crash evidence。
4. 如何回滚：workspace 级 drain 与 mode 切换；shadow 不写；canary 在删除 Python 前可退回 legacy writer。
5. 为什么现在做：用户目标已经从单 Agent 脚本执行变为多 Agent 共用运行时，现有分散脚本与复制 policy 无法提供一致控制。

5 问自检通过率 100%；每问都映射风险、AC 或迁移机制。

## 13. RA 质量闸判定

| 闸 | 结果 | 证据 |
| --- | --- | --- |
| RA-G01: PASS | 输入保真 | 用户两次澄清 daemon 目标并决定全 Rust |
| RA-G02: PASS | 12 维完整 | §0.5 RA-01 至 RA-12 |
| RA-G03: PASS | 角色完整 | §2 |
| RA-G04: PASS | 场景完整 | §3 正向、并发、断连、迁移与恢复 |
| RA-G05: PASS | 流程/状态机完整 | §4 六类状态机与十步主流程 |
| RA-G06: PASS | 数据与权威边界完整 | §5 project truth 与 daemon metadata |
| RA-G07: PASS | 主规则/衍生规则完整 | §6 与 §6.5，10/10 映射 |
| RA-G08: PASS | AC 与衍生覆盖完整 | §8、§8.5、§8.6 |
| RA-G09: PASS | 业务模式与跨域级联完整 | §9-bis 与 §9-ter |
| RA-G10: PASS | 方案比较与复用方向完整 | §7 与 §9-quater.4 |
| RA-G11: PASS | 假设可证伪 | §9 |
| RA-G12: PASS | 缺口与规模处理明确 | §10 与 §11 |
| RA-G13: PASS | 实现七要素完整 | §9-quater.1 至 §9-quater.7 |
| RA-G14: PASS | 迁移边界明确 | Rust runtime、content/UI 边界、shadow/canary |
| RA-G15: PASS | 安全与回滚明确 | IPC ACL/token、allowed-root、drain/mode |
| RA-G16: PASS | 下游可执行 | DR 交接包与 6 Story 路由 |
