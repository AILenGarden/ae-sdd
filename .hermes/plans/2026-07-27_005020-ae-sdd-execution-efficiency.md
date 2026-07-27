# ae-sdd Execution Efficiency Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** 在不削弱 ae-sdd 权威 Gate、用户审批、真实测试证据和独立 Review 的前提下，把“已批准计划后的执行”改造成可恢复、机器可执行、受预算约束的短循环，使恢复到首个补丁不超过 5 分钟，并将典型任务 Token 消耗降低至少 60%。

**Architecture:** 把系统明确拆成低频治理面与高频执行面。治理面只负责 Route、RA/Story、executionPlan 审批、阶段 Gate 和最终 Review；执行面由 Rust daemon 根据已批准计划生成 `ExecutionCapsuleV1` 和 `ExecutionSliceV1` 队列，通过 `ExecutionSupervisor` 驱动 `RED -> minimal patch -> focused GREEN -> evidence`。`FlowRuntime` 继续作为 phase/nextAction 唯一 owner；项目级 authority/journal/artifact 保存业务真相，用户级 SQLite 只保存可重建运行时元数据，CLI、Hook 与 Python 兼容层保持薄客户端。

**Tech Stack:** Rust workspace、JSON-RPC v1、typed operations、SQLite/rusqlite、项目级 mutation journal、Tokio daemon、Python pytest 兼容 oracle、Cargo test/Clippy、JSONL hash-chain evidence ledger。

---

## 1. 计划定位

这是一项独立的 ae-sdd self-update，不并入当前 daemon V-024 收尾工作。

- 当前 daemon Work Item：`PRD-AE-SDD-RUST-DAEMON-001`
- 当前 Story：`STORY-AE-SDD-C1-INTEGRATION-001`
- daemon 前置交接计划：`D:\Item\ae-sdd\.hermes\plans\2026-07-26_164000-ae-sdd-daemon-v024-handoff.md`
- 建议新 PRD：`PRD-AE-SDD-EXECUTION-EFFICIENCY-001`
- 当前工作树非常脏；实施时禁止 `git reset`、`git clean`、`git add -A`、仓库级机械重写和无关格式化。

建议按以下 Story 切分，但只在准备实施对应阶段时按需创建，不要一次生成全部文档：

1. `STORY-AE-SDD-EXECUTION-CAPSULE-001`
2. `STORY-AE-SDD-SLICE-SUPERVISOR-001`
3. `STORY-AE-SDD-INCREMENTAL-GATES-001`
4. `STORY-AE-SDD-EVIDENCE-LEDGER-001`
5. `STORY-AE-SDD-REVIEW-CONTRIBUTION-001`
6. `STORY-AE-SDD-SELF-HOSTING-001`

治理资产采用“一份 PRD/DR 共享、Story 按阶段即时展开”的方式。P0 只需要前两个 Story 获批；P1/P2 不提前消耗上下文。

## 2. 已确认的现状与问题

| 现状 | 直接后果 | 本计划的处理 |
| --- | --- | --- |
| `state.executionPlan` 已结构化且要求用户审批，但恢复后没有机器可执行的切片队列 | Agent 每次继续时重新理解全局任务 | 生成可寻址的 `ExecutionCapsuleV1` 与 `ExecutionSliceQueueV1` |
| `ContextProjection` 已支持 full/delta/no-change，但执行计划、活动切片没有专用紧凑投影 | 恢复上下文仍然偏大，重复读 Story/约束/源码 | 增加执行 capsule 投影，硬上限 16 KiB |
| `GateScheduler` 有 key cache 和 single-flight，但 `AuthoritativeGateRuntime::evaluate` 每次新建 scheduler | 跨请求缓存失效，重复 Gate/扫描 | 让 scheduler 长生命周期，并增加依赖 DAG 与 selector invalidation |
| 当前 evidence manifest 每次记录都重写整个 JSON，并原位标记 superseded | 历史不是天然 append-only，状态容易膨胀 | JSONL hash-chain ledger 为真相，manifest 变成可重建投影 |
| Review v2 contract 已有 contribution、batch、final proof，但外部只有 `review.record` | 多 reviewer 共享一个大操作和 writer lease，重复聚合 | 拆为 `review.contribute` 与 `review.finalize`，复用现有 ReviewSupervisor |
| Hook payload 是通用 `hostPayload`，没有稳定的执行工具分类 | daemon 无法可靠阻止 focused GREEN 前跑 broad test | 增加可选 `executionEvent`，由 daemon 分类和裁决 |
| 无明确“连续调查但没有产出”的机器规则 | Agent 可持续搜索、读文件、重复验证 | 4 次调查工具调用为一批，连续 3 批无进展后停止继续调查 |
| Cargo 命令没有跨会话全局资源锁 | 多 Agent 并行 Cargo 导致排队、抖动和超时 | daemon-wide OS 文件锁，公平队列，TTL 与崩溃释放 |
| 当前 lifecycle 只有 Coding/TestRunning/CodeReviewed/Completed 粗粒度终态 | “代码已验证”“可 Review”“治理已关闭”混在一起 | 新增 completion milestone，不破坏现有 ProcessPhase wire |
| self-update 时稳定 daemon 与候选 daemon 没有明确治理隔离 | 候选程序可能参与写 authority 或污染稳定运行时 | Stable Governor / Candidate ReadOnly 双角色 |
| runtime stats 主要记录命令耗时，没有 slice/token/重复读取指标 | 无法量化“慢”和“耗 Token”是否改善 | 增加 resume、slice、Gate cache、source cache、token delta 指标 |

## 3. 不可违反的架构约束

1. `FlowRuntime` 仍然是 phase 和 `NextAction` 的唯一 owner；ExecutionSupervisor 只能执行其决定，不能私自推进 phase。
2. CLI/Hook 只负责身份、参数规范化、IPC 和结果呈现，不实现 Gate、slice、evidence、Review 业务规则。
3. 用户级 SQLite 只保存可重建 runtime metadata：capsule cache、slice cursor、计数器、资源锁租约和 telemetry locator。
4. `executionPlan`、slice completion、evidence、Review 和 completion milestone 的业务真相必须写入项目级 authority/journal/artifact。
5. Rust production runtime 不得调用 Python fallback；`tools/lib` 只用于迁移兼容和 differential oracle。
6. 新 RPC/operation 必须具备唯一 owner、typed DTO、稳定错误、幂等键、权限、revision/lease 规则和验证矩阵。
7. 不手改 `dist/`；修改 `source/` 后通过 `scripts/build_dist.py` 生成。
8. 不把 monitor、prompt prose、Python state 或生成物当作 Rust control-plane authority。
9. Review 内核复用现有 `ReviewSupervisor::evaluate` 与 v2 contracts，不创建第二套 Review 状态机。
10. 任何已批准范围内的普通实现变动都不重新审批；只有 Story/AC、changedPaths、verification contract 或风险边界 digest 改变时才使审批失效。

## 4. 目标执行模型

```text
治理面（低频）
Route -> RA/Story -> executionPlan approval -> phase Gate -> final Review

执行面（高频）
resume-approved-plan
  -> full/delta/no-change ExecutionCapsuleV1
  -> FlowRuntime: ExecuteApprovedSlice
  -> focused RED
  -> minimal patch
  -> focused GREEN
  -> append evidence
  -> slice complete
  -> next slice
```

### 4.1 项目级真相

建议新增以下项目级 authority artifacts：

```text
.auto-engineering/{workItemId}/execution/queue.json
.auto-engineering/{workItemId}/execution/ledger.jsonl
.auto-engineering/{workItemId}/execution/capsule.json
.auto-engineering/{storyId}/evidence/ledger.jsonl
.auto-engineering/{storyId}/evidence/manifest.json
```

项目 state 只保存 locator/digest/cursor：

```json
{
  "executionRuntime": {
    "schemaVersion": 1,
    "capsuleRef": ".auto-engineering/WORK/execution/capsule.json",
    "capsuleDigest": "sha256:...",
    "queueRef": ".auto-engineering/WORK/execution/queue.json",
    "queueDigest": "sha256:...",
    "ledgerRef": ".auto-engineering/WORK/execution/ledger.jsonl",
    "ledgerDigest": "sha256:...",
    "activeSliceOrdinal": 2,
    "completionMilestone": "none"
  },
  "evidenceAuthority": {
    "ledgerRef": ".auto-engineering/STORY/evidence/ledger.jsonl",
    "ledgerDigest": "sha256:...",
    "manifestRef": ".auto-engineering/STORY/evidence/manifest.json",
    "manifestDigest": "sha256:..."
  }
}
```

### 4.2 核心 Rust contract 骨架

目标文件：`crates/ae-sdd-contracts/src/execution_runtime.rs`。

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionSliceStatus {
    Pending,
    Running,
    RedObserved,
    Patched,
    FocusedGreen,
    EvidenceBound,
    Completed,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionToolClass {
    SourceRead,
    Search,
    Patch,
    FocusedTest,
    BroadTest,
    Evidence,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionSliceV1 {
    pub slice_id: ExecutionSliceId,
    pub ordinal: u32,
    pub objective: Box<str>,
    pub depends_on: Vec<ExecutionSliceId>,
    pub path_scope: Vec<ProjectRelativePath>,
    pub source_reads: Vec<SourceReadSpecV1>,
    pub focused_verification_id: VerificationId,
    pub broad_verification_ids: Vec<VerificationId>,
    pub evidence_logical_key: Box<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionBudgetsV1 {
    pub max_capsule_bytes: u32,
    pub max_tool_output_bytes: u32,
    pub max_source_read_bytes_per_batch: u32,
    pub max_source_files_per_batch: u16,
    pub inspection_calls_per_batch: u8,
    pub max_no_progress_batches: u8,
    pub max_authority_refreshes_per_resume: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionQueueRefV1 {
    pub artifact: ArtifactRef,
    pub queue_digest: ArtifactDigest,
    pub total_slices: u32,
    pub completed_slices: u32,
    pub active_ordinal: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionCapsuleV1 {
    pub schema_version: SchemaVersion,
    pub work_item_id: WorkItemId,
    pub story_id: StoryId,
    pub source_revision: StateRevision,
    pub approved_plan_digest: ArtifactDigest,
    pub policy_digest: PolicyDigest,
    pub inventory_generation: InventoryGeneration,
    pub story_ref: ArtifactRef,
    pub constraints_ref: ArtifactRef,
    pub thinking_engine_ref: ArtifactRef,
    pub verification_ref: ArtifactRef,
    pub queue: ExecutionQueueRefV1,
    pub active_slice: ExecutionSliceV1,
    pub budgets: ExecutionBudgetsV1,
}
```

默认预算：

| Budget | 默认值 | 行为 |
| --- | ---: | --- |
| capsule hard limit | 16 KiB | 超限 fail closed |
| capsule p95 target | 8 KiB | telemetry 告警 |
| 单工具输出 | 64 KiB | 截断并保存 digest/locator |
| 单调查批次源码输出 | 24 KiB | 超限停止继续读取 |
| 单调查批次文件数 | 12 | 超限要求缩小范围 |
| 单调查批次工具调用 | 4 | 关闭一批并判定 progress |
| 连续无进展批次 | 3 | 禁止继续调查，只允许 patch/focused test/blocker |
| resume authority refresh | 1 | 第二次必须命中 cache 或返回 stale |
| daemon-wide Cargo 并发 | 1 | 其余请求 defer，不并行争抢 |

### 4.3 typed operations

| Operation | Writes | Lease/revision | 说明 |
| --- | --- | --- | --- |
| `execution.resume` | no | 无 writer lease；要求 workspace/workItem/session | 从已批准 authority 生成 full/delta/no-change capsule |
| `execution.slice.start` | yes | writer lease + revision + idempotency | CAS 启动 FlowRuntime 指定的 active ordinal |
| `execution.slice.record` | yes | writer lease + revision + idempotency | append slice progress，不写完整工具输出 |
| `review.contribute` | yes | 无全局长租约；WorkItem actor 串行 + idempotency + input fingerprint | append 一名 reviewer 的 contribution |
| `review.finalize` | yes | writer lease + revision + idempotency | 聚合 contribution、验证 final proof、调用 ReviewSupervisor |

`execution.resume` 的请求只允许携带已知 cursor：

```json
{
  "knownCapsuleDigest": "sha256:...",
  "knownContextRevision": 4
}
```

返回：

```json
{
  "projectionKind": "no-change",
  "contextRevision": 4,
  "capsuleDigest": "sha256:...",
  "capsule": null,
  "nextAction": {
    "kind": "execute-approved-slice",
    "activeOrdinal": 2,
    "queueDigest": "sha256:..."
  }
}
```

### 4.4 FlowRuntime 所有权

不新增第二个 phase 状态机。给 `FlowSnapshot` 增加可复制的 `CompletionMilestone`，给 `FlowEnvironment` 增加紧凑 `ExecutionCursor`，并由 `FlowRuntime` 产生以下 `NextAction`：

```rust
pub enum CompletionMilestone {
    None,
    ImplementationVerified,
    ReviewReady,
    GovernanceClosed,
}

pub enum NextAction {
    // existing variants...
    ResumeApprovedExecution,
    ExecuteApprovedSlice {
        active_ordinal: u32,
        queue_digest: ArtifactDigest,
    },
    FinalizeExecutionEvidence,
    CollectReviewContributions,
    FinalizeGovernance,
}
```

`Completed` 只有在 milestone 为 `GovernanceClosed` 且当前 Gate/Review/evidence digest 全部新鲜时才允许提交。

### 4.5 no-progress 判定

“进展”只能由以下机器事件产生：

- 新补丁 digest；
- focused test 首次执行；
- focused test 从非绿变为绿；
- 新 blocker code + evidence locator；
- 新 evidence ledger event；
- slice 从一个合法状态推进到下一个合法状态。

重复读取相同 path/digest/range、重复执行相同失败命令且输入未变、重新打印 state、重新运行已命中 cache 的 Gate 都不算进展。

### 4.6 Gate DAG

当前 `GateScheduler` 的 key cache 保留，但 scheduler 必须长生命周期。新增：

```rust
pub enum GateInputSelector {
    ProjectAssets,
    Story,
    Constraints,
    ThinkingEngine,
    ExecutionPlan,
    ChangedPaths,
    VerificationPlan,
    EvidenceLedger,
    ReviewBatch,
    Toolchain,
    Inventory,
}

pub struct GateDependencySpec {
    pub gate: RequiredGate,
    pub prerequisites: &'static [RequiredGate],
    pub selectors: &'static [GateInputSelector],
}
```

一次 source/evidence/review 变动只使依赖对应 selector 的 Gate 失效；Gate key 未变时直接复用 fresh PASS。

### 4.7 append-only evidence ledger

目标 contract：

```rust
pub enum EvidenceLedgerEventKind {
    Recorded,
    Superseded,
    Finalized,
    Invalidated,
}

pub struct EvidenceLedgerEventV1 {
    pub sequence: u64,
    pub event_id: EvidenceId,
    pub kind: EvidenceLedgerEventKind,
    pub logical_key: Box<str>,
    pub input_fingerprint: InputFingerprint,
    pub artifact_refs: Vec<ArtifactRef>,
    pub previous_event_digest: Option<ArtifactDigest>,
    pub event_digest: ArtifactDigest,
}
```

ledger 每行一个 canonical JSON 事件并形成 hash chain；manifest 是 ledger 的确定性 active projection。`evidence.record` 与 `evidence.finalize` 的外部名字保持稳定。

### 4.8 Review lease 重构

- `review.contribute` 只 append contribution，不尝试结束 batch。
- contribution 由 WorkItem actor 串行，使用 idempotency key 和 review/input/ruleset fingerprint CAS，不持有跨 reviewer 的全局 writer lease。
- `review.finalize` 才获取短 writer lease，组装现有 contribution 与 final proof，并调用现有 `ReviewSupervisor::evaluate`。
- `review.record` 在兼容期只作为 adapter：单个 contribution 后立即 finalize；Rust authority 不保留第二套实现。

### 4.9 Stable Governor / Candidate Daemon

- Stable Governor：唯一允许写项目 authority、发布正式 endpoint manifest、持有 writer/cargo locks 的 daemon。
- Candidate Daemon：独立 state dir、独立 endpoint、只读 business adapter、不发布为默认 endpoint，只能做 parity、Gate、projection 和测试验证。
- Promote：candidate parity PASS -> stable drain -> 原子替换 release -> 新 stable 启动 -> endpoint 切换。
- Rollback：回到上一完整 native release；禁止稳定与候选双写。

## 5. 成功指标

| 指标 | P0 门槛 | 最终目标 |
| --- | ---: | ---: |
| resume 到首个 patch | <= 5 分钟 | p95 <= 3 分钟 |
| resume full capsule | <= 16 KiB | p95 <= 8 KiB |
| no-change response | <= 1 KiB | p95 <= 512 B |
| 每次 resume authority refresh | <= 1 | 1 |
| 连续无进展调查批次 | <= 3 | <= 2 p95 |
| focused GREEN 前 broad test | 0 | 0 |
| 计划内修改重新审批率 | <= 5% | 接近 0 |
| Gate fresh cache hit | >= 60% | >= 80% |
| 重复 source-read bytes | 下降 >= 50% | 下降 >= 75% |
| 典型 golden trace Token | 下降 >= 40% | 下降 >= 60% |
| request body timeout | golden trace 为 0 | canary p99 为 0 |

## 6. 实施顺序和预计耗时

```text
P0  Contracts -> Capsule -> Flow action -> Supervisor -> Hook/cache/lock -> E2E
                          |
P1  Persistent Gate DAG -> Evidence ledger -> Milestones -> Review split
                          |
P2  Stable/Candidate isolation -> Telemetry -> SKILL/AGENTS -> rollout
```

- P0：3–4 个专注开发日，先交付可用的 resume + slice loop。
- P1：3–5 个专注开发日。
- P2：2–3 个专注开发日。
- 单人顺序完成：约 8–12 个开发日；共享 contracts 冻结后可缩短，但不得并行改共享 DTO、migration number、operation registry。

以下每个 Step 都应保持为 2–5 分钟的单一动作。每个 Task 完成后只 stage 本 Task 文件。

---

## P0 — 快速恢复与受监督执行

### Task 0: 建立独立 authority 边界

**Objective:** 确保执行效率改造不污染当前 V-024 daemon 收尾。

**Files:**
- Reference only: `.hermes/plans/2026-07-26_164000-ae-sdd-daemon-v024-handoff.md`
- Authority paths: 由 `document.resolve` 返回，禁止手猜路径

**Step 1: 核对当前 daemon Work Item**

运行只读 `workitem.get`，确认当前仍为 `PRD-AE-SDD-RUST-DAEMON-001`。

**Step 2: 创建新 PRD authority**

创建 `PRD-AE-SDD-EXECUTION-EFFICIENCY-001`，明确本计划 10 条架构约束和指标。

**Step 3: 只创建 P0 Story**

先创建 Capsule 与 Slice Supervisor 两个 Story；P1/P2 Story 延迟到对应阶段。

**Step 4: 冻结 executionPlan**

changedPaths 只覆盖 P0 所列路径；用户审批后才能进入 Task 1。

**Step 5: Gate**

预期 `G-CODEPLAN-SRC`、`G-14`、`G-08` PASS。任何失败只修 authority，不扩大产品范围。

### Task 1: 建立 golden execution fixture

**Objective:** 用一个三切片、已批准计划 fixture 固定性能与行为基线。

**Files:**
- Create: `tests/fixtures/execution-efficiency/approved-resume/state.json`
- Create: `tests/fixtures/execution-efficiency/approved-resume/queue.json`
- Create: `tests/fixtures/execution-efficiency/approved-resume/context.json`
- Create: `tests/fixtures/execution-efficiency/baseline.v1.json`
- Create: `tools/tests/test_execution_efficiency_fixture.py`

**Step 1: 写 fixture 校验测试**

测试必须验证 approved plan digest、四个必需 context refs、三个有序 slice、focused verification ID 和基线指标齐全。

**Step 2: 运行 RED**

Run: `python -m pytest tools/tests/test_execution_efficiency_fixture.py -q`  
Expected: FAIL，fixture 文件不存在。

**Step 3: 添加最小 fixture**

三切片依次为 contract、runtime wiring、process test；不得复制真实项目大 state。

**Step 4: 运行 GREEN**

Run: `python -m pytest tools/tests/test_execution_efficiency_fixture.py -q`  
Expected: PASS。

**Step 5: Commit**

```powershell
git add tests/fixtures/execution-efficiency tools/tests/test_execution_efficiency_fixture.py
git commit -m "test(execution): add efficiency golden fixture"
```

### Task 2: 冻结 ExecutionCapsule/ExecutionSlice contracts

**Objective:** 提供有界、严格、可 canonicalize 的 Rust DTO。

**Files:**
- Modify: `crates/ae-sdd-domain/src/ids.rs`
- Modify: `crates/ae-sdd-domain/src/lib.rs`
- Create: `crates/ae-sdd-contracts/src/execution_runtime.rs`
- Modify: `crates/ae-sdd-contracts/src/lib.rs`
- Create: `crates/ae-sdd-contracts/tests/execution_capsule_contract.rs`

**Step 1: 写 wire round-trip 与边界测试**

覆盖 duplicate slice ID、非连续 ordinal、空 objective、越界 path、缺 focused verification、capsule >16 KiB。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-contracts --test execution_capsule_contract`  
Expected: FAIL，contract 类型不存在。

**Step 3: 添加 `ExecutionSliceId` 与 DTO**

构造函数必须 canonical sort/dedup；字段保持 private 并提供只读 accessor。

**Step 4: 运行 GREEN**

Run: `cargo test -p ae-sdd-contracts --test execution_capsule_contract`  
Expected: PASS。

**Step 5: Commit**

```powershell
git add crates/ae-sdd-domain/src/ids.rs crates/ae-sdd-domain/src/lib.rs crates/ae-sdd-contracts/src/execution_runtime.rs crates/ae-sdd-contracts/src/lib.rs crates/ae-sdd-contracts/tests/execution_capsule_contract.rs
git commit -m "feat(contracts): add execution capsule v1"
```

### Task 3: 实现纯 capsule builder 与 slice transition

**Objective:** 从冻结 executionPlan 和 verification contract 生成确定性 queue/capsule，并校验 slice 状态机。

**Files:**
- Create: `crates/ae-sdd-execution/src/capsule.rs`
- Create: `crates/ae-sdd-execution/src/slice.rs`
- Modify: `crates/ae-sdd-execution/src/error.rs`
- Modify: `crates/ae-sdd-execution/src/lib.rs`
- Create: `crates/ae-sdd-execution/tests/capsule_builder.rs`
- Create: `crates/ae-sdd-execution/tests/slice_transition.rs`

**Step 1: 写 deterministic permutation test**

同一语义输入不同 map 顺序必须得到相同 queue/capsule digest。

**Step 2: 写非法 transition test**

禁止 `Pending -> FocusedGreen`、`RedObserved -> Completed`、GREEN 前 EvidenceBound。

**Step 3: 运行 RED**

Run: `cargo test -p ae-sdd-execution --test capsule_builder --test slice_transition`  
Expected: FAIL。

**Step 4: 实现最小 builder/reducer**

builder 只消费 typed 输入，不读 filesystem/clock/db；queue 全量写 artifact，capsule 只包含 active slice。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-execution --test capsule_builder --test slice_transition`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-execution/src crates/ae-sdd-execution/tests/capsule_builder.rs crates/ae-sdd-execution/tests/slice_transition.rs
git commit -m "feat(execution): build deterministic slice queues"
```

### Task 4: 让 FlowRuntime 产生执行 nextAction

**Objective:** 保持 FlowRuntime 对 nextAction 的唯一所有权。

**Files:**
- Modify: `crates/ae-sdd-flow/src/model.rs`
- Modify: `crates/ae-sdd-flow/src/runtime.rs`
- Modify: `crates/ae-sdd-flow/src/canonical.rs`
- Modify: `crates/ae-sdd-flow/src/lib.rs`
- Modify: `crates/ae-sdd-policy/src/transition.rs`
- Create: `crates/ae-sdd-flow/tests/execution_next_action.rs`
- Modify: `crates/ae-sdd-flow/tests/reducer_replay.rs`

**Step 1: 写 reducer test**

Coding + approved queue + active pending slice 必须返回 `ExecuteApprovedSlice`；queue digest 变化必须返回 `ResumeApprovedExecution`。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-flow --test execution_next_action`  
Expected: FAIL。

**Step 3: 添加 `ExecutionCursor` 与 NextAction variants**

cursor 只携带 ordinal、queue digest、slice status；不得把完整 capsule 塞进 FlowDecision。

**Step 4: 更新 canonical digest**

所有新增 enum tag 使用稳定显式编号，replay digest 必须跨运行一致。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-flow --test execution_next_action --test reducer_replay`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-flow crates/ae-sdd-policy/src/transition.rs
git commit -m "feat(flow): own approved slice next actions"
```

### Task 5: 注册 operations、稳定错误与 capability

**Objective:** 冻结外部 operation schema 和失败语义。

**Files:**
- Modify: `crates/ae-sdd-operations/src/registry.rs`
- Modify: `crates/ae-sdd-operations/tests/operation_registry.rs`
- Modify: `crates/ae-sdd-protocol/src/error.rs`
- Modify: `crates/ae-sdd-protocol/tests/protocol_contract.rs`
- Modify: `crates/ae-sdd-runtime/src/service_protocol.rs`
- Modify: `crates/ae-sdd-runtime/tests/service_dispatch_matrix.rs`

**Step 1: 写 registry/error uniqueness test**

新增 `execution.resume`、`execution.slice.start`、`execution.slice.record`；新增稳定错误：

- `EXECUTION_CAPSULE_STALE`
- `EXECUTION_SLICE_INVALID`
- `EXECUTION_PROGRESS_REQUIRED`
- `EXECUTION_RESOURCE_BUSY`
- `EXECUTION_BUDGET_EXCEEDED`

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-operations --test operation_registry && cargo test -p ae-sdd-protocol --test protocol_contract`  
Expected: FAIL。

**Step 3: 更新 operation registry**

`execution.resume` read-only；start/record 要求 lease、revision、idempotency；均要求 workspace/workItem/session。

**Step 4: 发布 capability**

Handshake 增加 `execution-supervisor-v1`，不新增直连 RPC method，复用 `operation.execute`。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-operations --test operation_registry && cargo test -p ae-sdd-protocol --test protocol_contract`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-operations crates/ae-sdd-protocol crates/ae-sdd-runtime/src/service_protocol.rs crates/ae-sdd-runtime/tests/service_dispatch_matrix.rs
git commit -m "feat(protocol): register execution supervisor operations"
```

### Task 6: 实现 authoritative execution.resume

**Objective:** 一次读取项目 authority，验证 approved plan 和四个 required contexts，生成或复用 capsule。

**Files:**
- Modify: `crates/ae-sdd-integrations/src/execution_authority.rs`
- Modify: `crates/ae-sdd-integrations/src/business.rs`
- Modify: `crates/ae-sdd-integrations/src/lib.rs`
- Create: `crates/ae-sdd-integrations/tests/execution_capsule_authority.rs`

**Step 1: 写 authority test**

覆盖：未审批、plan digest drift、Story drift、constraints drift、verification drift、重复 resume no-change。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-integrations --test execution_capsule_authority`  
Expected: FAIL。

**Step 3: 实现单次 authority load**

一个 operation 内只允许一次 state snapshot 和一次 resource/context bundle；重复解析必须使用本调用内缓存。

**Step 4: 写 project mutation targets**

首次生成 queue/capsule 时通过 `ProjectMutationStore` 原子写 queue、ledger seed、capsule 和 state locator/digest。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-integrations --test execution_capsule_authority`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-integrations/src/execution_authority.rs crates/ae-sdd-integrations/src/business.rs crates/ae-sdd-integrations/src/lib.rs crates/ae-sdd-integrations/tests/execution_capsule_authority.rs
git commit -m "feat(integrations): resolve approved execution capsule"
```

### Task 7: 增加 execution capsule full/delta/no-change 投影

**Objective:** 恢复时只传 active slice 与变更 context。

**Files:**
- Modify: `crates/ae-sdd-context/src/service.rs`
- Modify: `crates/ae-sdd-context/src/projection.rs`
- Modify: `crates/ae-sdd-context/src/lib.rs`
- Modify: `crates/ae-sdd-runtime/src/context_cache.rs`
- Create: `crates/ae-sdd-context/tests/execution_capsule_projection.rs`
- Create: `crates/ae-sdd-runtime/tests/execution_context_projection.rs`

**Step 1: 写 size/full/delta/no-change tests**

full <=16 KiB；只改变 active ordinal 时 delta 不携带 Story/constraints 正文；同 digest 返回 no-change。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-context --test execution_capsule_projection`  
Expected: FAIL。

**Step 3: 扩展 ContextSelector**

增加 `ExecutionCapsule`、`ExecutionQueue`、`ActiveSlice`，并将 cache key 绑定 plan/queue/capsule digest。

**Step 4: 复用现有 ContextCache**

不要新建另一套 full/delta 算法；将 typed capsule 序列化后进入现有 cache。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-context --test execution_capsule_projection && cargo test -p ae-sdd-runtime --test execution_context_projection`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-context crates/ae-sdd-runtime/src/context_cache.rs crates/ae-sdd-runtime/tests/execution_context_projection.rs
git commit -m "feat(context): project compact execution capsules"
```

### Task 8: 实现纯 ExecutionSupervisor policy

**Objective:** 机器裁决 RED/GREEN、调查批次、输出预算和 broad-test 时机。

**Files:**
- Create: `crates/ae-sdd-execution/src/supervisor.rs`
- Modify: `crates/ae-sdd-execution/src/policy.rs`
- Modify: `crates/ae-sdd-execution/src/error.rs`
- Modify: `crates/ae-sdd-execution/src/lib.rs`
- Create: `crates/ae-sdd-execution/tests/supervisor_progress_policy.rs`

**Step 1: 写 table-driven RED tests**

至少覆盖 20 条组合：source read、重复 read、patch、focused fail、focused pass、broad before/after green、3 批无进展、blocker reset、超预算输出。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-execution --test supervisor_progress_policy`  
Expected: FAIL。

**Step 3: 实现纯 reducer**

输入为当前 checkpoint + `ExecutionToolEventV1`；输出 `Allow/Deny/Defer/RequireProgress` 与新 checkpoint，不做 I/O。

**Step 4: Property tests**

证明 completed slice 不回退、broad test 在 GREEN 前永不 Allow、no-progress counter 不超过上限。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-execution --test supervisor_progress_policy`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-execution/src crates/ae-sdd-execution/tests/supervisor_progress_policy.rs
git commit -m "feat(execution): enforce bounded slice progress"
```

### Task 9: 接入 daemon session 与 Hook fast path

**Objective:** 让 PreTool/PostTool 真正执行 supervisor decision，同时保持 Hook 有界。

**Files:**
- Create: `crates/ae-sdd-runtime/src/execution_supervisor.rs`
- Modify: `crates/ae-sdd-runtime/src/model.rs`
- Modify: `crates/ae-sdd-runtime/src/service.rs`
- Modify: `crates/ae-sdd-runtime/src/service_hook_context.rs`
- Modify: `crates/ae-sdd-runtime/src/lib.rs`
- Modify: `crates/ae-sdd-policy/src/hook.rs`
- Create: `crates/ae-sdd-runtime/tests/execution_tool_guard.rs`
- Modify: `crates/ae-sdd-runtime/tests/hook_deadline_budget.rs`

**Step 1: 写 Hook schema tests**

`hostPayload.executionEvent` 必须 strict decode；老 host 无此字段时在 shadow 只记录 unclassified。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-runtime --test execution_tool_guard`  
Expected: FAIL。

**Step 3: 扩展 HookResult**

增加可选 `executionDirective`，包含 reason code、output budget、retryAfterMs、cachedReadRef；保持旧客户端可忽略。

**Step 4: 接入 supervisor**

`execution.resume` 成功后把 capsule digest/checkpoint 绑定到 authenticated session；PreTool 决策，PostTool append bounded event。

**Step 5: 验证 deadline**

Hook 内不得读项目文件或运行 Gate；只访问预计算 projection、session checkpoint 和内存 cache。

**Step 6: 运行 GREEN**

Run: `cargo test -p ae-sdd-runtime --test execution_tool_guard --test hook_deadline_budget`  
Expected: PASS。

**Step 7: Commit**

```powershell
git add crates/ae-sdd-runtime/src crates/ae-sdd-runtime/tests/execution_tool_guard.rs crates/ae-sdd-runtime/tests/hook_deadline_budget.rs crates/ae-sdd-policy/src/hook.rs
git commit -m "feat(runtime): supervise execution through hooks"
```

### Task 10: 增加 source-read cache 与 daemon-wide Cargo lock

**Objective:** 消除重复读取，并避免多个 Agent 同时争抢 Cargo。

**Files:**
- Create: `crates/ae-sdd-runtime/src/execution_cache.rs`
- Create: `crates/ae-sdd-runtime/src/execution_resources.rs`
- Modify: `crates/ae-sdd-runtime/src/config.rs`
- Modify: `crates/ae-sdd-runtime/src/service_hook_context.rs`
- Reuse/Modify if required: `crates/ae-sdd-store/src/filesystem.rs`
- Create: `crates/ae-sdd-runtime/tests/source_read_cache.rs`
- Create: `crates/ae-sdd-runtime/tests/cargo_resource_lock.rs`

**Step 1: 写 cache key tests**

key = workspace + canonical path + content digest + range；相同 key 命中，不同 digest 自动 miss。

**Step 2: 写 cross-session Cargo lock test**

两个 session 同时请求 Cargo：第一个 Allow，第二个 Defer + retryAfterMs；release/TTL 后第二个可执行。

**Step 3: 运行 RED**

Run: `cargo test -p ae-sdd-runtime --test source_read_cache --test cargo_resource_lock`  
Expected: FAIL。

**Step 4: 实现有界 cache**

LRU 只保存 digest、range 和 <=24 KiB excerpt；按 session visibility 返回，不持久化源码正文。

**Step 5: 实现 OS lock**

使用 per-user runtime state dir 下的显式 lock file 和现有 fs4/CrossProcessLock；禁止使用 workspace 根或 unresolved env var。

**Step 6: 运行 GREEN**

Run: `cargo test -p ae-sdd-runtime --test source_read_cache --test cargo_resource_lock`  
Expected: PASS。

**Step 7: Commit**

```powershell
git add crates/ae-sdd-runtime/src/execution_cache.rs crates/ae-sdd-runtime/src/execution_resources.rs crates/ae-sdd-runtime/src/config.rs crates/ae-sdd-runtime/src/service_hook_context.rs crates/ae-sdd-runtime/tests/source_read_cache.rs crates/ae-sdd-runtime/tests/cargo_resource_lock.rs crates/ae-sdd-store/src/filesystem.rs
git commit -m "feat(runtime): cache source reads and serialize cargo"
```

### Task 11: 增加 0011 runtime metadata migration 与恢复

**Objective:** daemon 重启后恢复可重建 checkpoint，而不把业务真相搬进 SQLite。

**Files:**
- Create: `migrations/0011_execution_supervisor_v1.sql`
- Modify: `crates/ae-sdd-store/src/sqlite.rs`
- Modify: `crates/ae-sdd-runtime/src/model.rs`
- Modify: `crates/ae-sdd-runtime/src/ports.rs`
- Modify: `crates/ae-sdd-integrations/src/persistence.rs`
- Create: `crates/ae-sdd-store/tests/migration_catalog_0011.rs`
- Create: `crates/ae-sdd-runtime/tests/execution_supervisor_restart.rs`

**Step 1: 写 migration catalog RED test**

要求版本 11、名称唯一、旧 DB 可升级、重复打开幂等。

**Step 2: 写 restart RED test**

重启后从 project locator/digest 重建 active slice；SQLite capsule digest 不匹配时丢弃 cache，不覆盖项目 state。

**Step 3: 运行 RED**

Run: `cargo test -p ae-sdd-store --test migration_catalog_0011 && cargo test -p ae-sdd-runtime --test execution_supervisor_restart`  
Expected: FAIL。

**Step 4: 添加最小表**

只存 workspace/workItem/session、capsule/queue digest、active ordinal、no-progress counter、cache stats、updated cursor。

**Step 5: 实现 recovery**

project authority 永远优先；SQLite 仅加速。

**Step 6: 运行 GREEN**

Run: `cargo test -p ae-sdd-store --test migration_catalog_0011 && cargo test -p ae-sdd-runtime --test execution_supervisor_restart`  
Expected: PASS。

**Step 7: Commit**

```powershell
git add migrations/0011_execution_supervisor_v1.sql crates/ae-sdd-store/src/sqlite.rs crates/ae-sdd-store/tests/migration_catalog_0011.rs crates/ae-sdd-runtime/src/model.rs crates/ae-sdd-runtime/src/ports.rs crates/ae-sdd-runtime/tests/execution_supervisor_restart.rs crates/ae-sdd-integrations/src/persistence.rs
git commit -m "feat(store): persist rebuildable execution checkpoints"
```

### Task 12: 增加 resume-approved-plan CLI 与 Python compatibility oracle

**Objective:** 给 Agent 一个单命令恢复入口，生产规则仍由 daemon 执行。

**Files:**
- Modify: `bins/ae-sdd-cli/src/main.rs`
- Create: `bins/ae-sdd-cli/tests/resume_approved_plan.rs`
- Modify: `tools/bin/ae-sdd`
- Modify: `tools/lib/operations.py`
- Create: `tools/tests/test_resume_approved_plan.py`

**Step 1: 写 Rust CLI RED test**

命令：`ae-sdd resume-approved-plan --request <json>`；CLI 只组装 `operation.execute`。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-cli --test resume_approved_plan`  
Expected: FAIL。

**Step 3: 实现薄 CLI**

CLI 不读 Story、constraints、state 或源码；只连接 daemon 并呈现 projectionKind/capsule/nextAction。

**Step 4: 写 Python parity test**

Python oracle 只在 migration profile 下生成相同 request/response shape，不成为 canary/sole-writer fallback。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-cli --test resume_approved_plan && python -m pytest tools/tests/test_resume_approved_plan.py -q`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add bins/ae-sdd-cli/src/main.rs bins/ae-sdd-cli/tests/resume_approved_plan.rs tools/bin/ae-sdd tools/lib/operations.py tools/tests/test_resume_approved_plan.py
git commit -m "feat(cli): add approved plan resume command"
```

### Task 13: P0 process E2E 与性能门槛

**Objective:** 证明真实 daemon 路径能够一次恢复并完成一个切片。

**Files:**
- Create: `bins/ae-sdd-daemon/tests/execution_efficiency_process.rs`
- Create: `crates/ae-sdd-integrations/tests/execution_supervisor_e2e.rs`
- Create: `tools/tests/test_execution_efficiency_metrics.py`
- Modify: `crates/ae-sdd-build/src/benchmark.rs`

**Step 1: 写真实进程 E2E**

流程：start daemon -> register -> session -> resume full -> resume no-change -> focused RED -> patch event -> focused GREEN -> evidence -> slice complete。

**Step 2: 断言禁止路径**

GREEN 前 broad test 返回 `EXECUTION_PROGRESS_REQUIRED`；第 13 次连续调查调用被拒绝。

**Step 3: 断言性能**

capsule <=16 KiB、no-change <=1 KiB、authorityRefreshCount=1、broadBeforeGreen=0。

**Step 4: 运行 RED/GREEN**

Run: `cargo test -p ae-sdd-daemon --test execution_efficiency_process`  
Run: `cargo test -p ae-sdd-integrations --test execution_supervisor_e2e`  
Run: `python -m pytest tools/tests/test_execution_efficiency_metrics.py -q`  
Expected: 全部 PASS。

**Step 5: P0 checkpoint**

只有上述测试和 P0 Story verification matrix 全绿才进入 P1；不提前跑 workspace 全回归。

**Step 6: Commit**

```powershell
git add bins/ae-sdd-daemon/tests/execution_efficiency_process.rs crates/ae-sdd-integrations/tests/execution_supervisor_e2e.rs tools/tests/test_execution_efficiency_metrics.py crates/ae-sdd-build/src/benchmark.rs
git commit -m "test(execution): verify fast supervised resume"
```

---

## P1 — 增量 Gate、证据与 Review

### Task 14: 让 GateScheduler 长生命周期并增加 DAG planner

**Objective:** 复用 fresh Gate PASS，只重新计算受变更 selector 影响的节点。

**Files:**
- Modify: `crates/ae-sdd-gates/src/registry.rs`
- Modify: `crates/ae-sdd-gates/src/scheduler.rs`
- Create: `crates/ae-sdd-gates/src/dag.rs`
- Modify: `crates/ae-sdd-gates/src/lib.rs`
- Modify: `crates/ae-sdd-integrations/src/gate_source/mod.rs`
- Create: `crates/ae-sdd-gates/tests/incremental_gate_dag.rs`
- Create: `crates/ae-sdd-integrations/tests/gate_cache_lifecycle.rs`

**Step 1: 写当前 bug 的 RED test**

同一个 `AuthoritativeGateRuntime` 连续 evaluate 相同 key，第二次 executor count 必须仍为 1。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-integrations --test gate_cache_lifecycle`  
Expected: FAIL，因为当前 evaluate 每次新建 scheduler。

**Step 3: 长生命周期 scheduler**

把 scheduler 放入 authority runtime/service 级缓存；workspace/workItem/policy/inventory 变化时创建新实例。

**Step 4: 添加 DAG/selector**

Gate registry 为每个 Gate 声明 prerequisites 与 selectors；检测环并稳定拓扑排序。

**Step 5: 增量 invalidation test**

仅 evidence ledger 变化时不得重新跑 RA/Story/CodingPlan Gates；Review batch 变化只影响 Review/Completed 路径。

**Step 6: 运行 GREEN**

Run: `cargo test -p ae-sdd-gates --test incremental_gate_dag && cargo test -p ae-sdd-integrations --test gate_cache_lifecycle`  
Expected: PASS。

**Step 7: Commit**

```powershell
git add crates/ae-sdd-gates crates/ae-sdd-integrations/src/gate_source/mod.rs crates/ae-sdd-integrations/tests/gate_cache_lifecycle.rs
git commit -m "feat(gates): evaluate an incremental dependency DAG"
```

### Task 15: 将 evidence 改为 append-only ledger

**Objective:** 以 hash-chain ledger 保存真实历史，manifest 只做 active projection。

**Files:**
- Create: `crates/ae-sdd-contracts/src/evidence.rs`
- Modify: `crates/ae-sdd-contracts/src/lib.rs`
- Modify: `crates/ae-sdd-integrations/src/operation_semantics/evidence.rs`
- Modify: `crates/ae-sdd-integrations/src/business.rs`
- Modify: `crates/ae-sdd-integrations/src/review_authority.rs`
- Create: `crates/ae-sdd-integrations/tests/evidence_ledger.rs`
- Modify: `tools/lib/evidence.py`
- Modify: `tools/tests/test_incremental_quality.py`

**Step 1: 写 ledger RED tests**

覆盖 append、supersede event、finalize event、hash-chain tamper、manifest rebuild、legacy manifest read。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-integrations --test evidence_ledger`  
Expected: FAIL。

**Step 3: 实现 canonical JSONL**

现有 immutable snapshot 逻辑保留；不再原位修改旧 entry。

**Step 4: manifest projection**

finalize 从 ledger 计算 active entries，写 contentHash；state 只写 ledger/manifest digest 与 locator。

**Step 5: Python oracle parity**

Python 只验证相同 ledger/projection，不允许 production Rust 调用。

**Step 6: 运行 GREEN**

Run: `cargo test -p ae-sdd-integrations --test evidence_ledger && python -m pytest tools/tests/test_incremental_quality.py -q`  
Expected: PASS。

**Step 7: Commit**

```powershell
git add crates/ae-sdd-contracts/src/evidence.rs crates/ae-sdd-contracts/src/lib.rs crates/ae-sdd-integrations/src/operation_semantics/evidence.rs crates/ae-sdd-integrations/src/business.rs crates/ae-sdd-integrations/src/review_authority.rs crates/ae-sdd-integrations/tests/evidence_ledger.rs tools/lib/evidence.py tools/tests/test_incremental_quality.py
git commit -m "feat(evidence): add append-only evidence ledger"
```

### Task 16: 增加 completion milestones

**Objective:** 区分实现已验证、可 Review、治理关闭和最终 Completed。

**Files:**
- Modify: `crates/ae-sdd-domain/src/lifecycle.rs`
- Modify: `crates/ae-sdd-contracts/src/lifecycle.rs`
- Modify: `crates/ae-sdd-flow/src/model.rs`
- Modify: `crates/ae-sdd-flow/src/runtime.rs`
- Modify: `crates/ae-sdd-flow/src/canonical.rs`
- Modify: `crates/ae-sdd-lifecycle/src/projection.rs`
- Modify: `crates/ae-sdd-lifecycle/src/validation.rs`
- Modify: `crates/ae-sdd-policy/src/transition.rs`
- Create: `crates/ae-sdd-flow/tests/completion_milestones.rs`
- Create: `crates/ae-sdd-lifecycle/tests/completion_milestones.rs`

**Step 1: 写状态矩阵 RED test**

- focused/workspace verification fresh -> ImplementationVerified
- evidence finalized -> ReviewReady
- Review PASS + final Gates -> GovernanceClosed
- 只有 GovernanceClosed -> Completed

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-flow --test completion_milestones && cargo test -p ae-sdd-lifecycle --test completion_milestones`  
Expected: FAIL。

**Step 3: 实现 milestone**

保留现有 ProcessPhase wire；milestone 是 state/FlowSnapshot 的新正交维度。

**Step 4: invalidation**

代码、verification、evidence 或 Review input digest 变化时，将 milestone 回退到最早受影响点；不能直接保留 GovernanceClosed。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-flow --test completion_milestones && cargo test -p ae-sdd-lifecycle --test completion_milestones`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-domain/src/lifecycle.rs crates/ae-sdd-contracts/src/lifecycle.rs crates/ae-sdd-flow crates/ae-sdd-lifecycle crates/ae-sdd-policy/src/transition.rs
git commit -m "feat(lifecycle): separate completion milestones"
```

### Task 17: 拆分 review.contribute 与 review.finalize

**Objective:** 让 reviewer 独立提交，最终聚合只做一次。

**Files:**
- Modify: `crates/ae-sdd-operations/src/registry.rs`
- Modify: `crates/ae-sdd-runtime/src/grant.rs`
- Modify: `crates/ae-sdd-integrations/src/review_authority.rs`
- Modify: `crates/ae-sdd-integrations/src/business.rs`
- Modify: `crates/ae-sdd-integrations/src/persistence.rs`
- Modify: `crates/ae-sdd-review/src/supervisor.rs` only if an accessor is missing
- Create: `crates/ae-sdd-integrations/tests/review_contribution_e2e.rs`
- Modify: `crates/ae-sdd-integrations/tests/review_gate_e2e.rs`

**Step 1: 写 operation/grant RED tests**

reviewer grant 包含 `review.contribute` 和 specialty；root/finalizer 才能 `review.finalize`。

**Step 2: 写并发 contribution test**

不同 reviewer 用不同 idempotency key append；重复 replay 不新增；相同 key 不同 payload 被拒绝。

**Step 3: 运行 RED**

Run: `cargo test -p ae-sdd-integrations --test review_contribution_e2e`  
Expected: FAIL。

**Step 4: 实现 contribute**

复用现有 `ReviewerContributionV2`；append 后不调用最终 PASS 判定。

**Step 5: 实现 finalize**

读取当前 contribution projection，构造 `ReviewAttemptV2`，调用 `ReviewSupervisor::evaluate`，原子写 batch/session/exit receipt。

**Step 6: 兼容 adapter**

`review.record` 调用 contribute + finalize；标记 deprecated telemetry，不复制业务规则。

**Step 7: 运行 GREEN**

Run: `cargo test -p ae-sdd-integrations --test review_contribution_e2e --test review_gate_e2e`  
Expected: PASS。

**Step 8: Commit**

```powershell
git add crates/ae-sdd-operations/src/registry.rs crates/ae-sdd-runtime/src/grant.rs crates/ae-sdd-integrations/src/review_authority.rs crates/ae-sdd-integrations/src/business.rs crates/ae-sdd-integrations/src/persistence.rs crates/ae-sdd-review/src/supervisor.rs crates/ae-sdd-integrations/tests/review_contribution_e2e.rs crates/ae-sdd-integrations/tests/review_gate_e2e.rs
git commit -m "feat(review): split contribution from finalization"
```

### Task 18: P1 端到端增量治理验证

**Objective:** 证明 slice 完成后只运行必要 Gate，并通过 evidence/review/milestone 到达 Completed。

**Files:**
- Create: `bins/ae-sdd-daemon/tests/incremental_governance_process.rs`
- Create: `crates/ae-sdd-integrations/tests/execution_to_completion_e2e.rs`
- Modify: `crates/ae-sdd-integrations/tests/lifecycle_control_plane_e2e.rs`

**Step 1: 真实流程**

完成三个 slice -> finalize evidence -> ImplementationVerified -> ReviewReady -> 两个 reviewer contribute -> finalize -> GovernanceClosed -> Completed。

**Step 2: 断言 Gate 计数**

第二、第三 slice 不重复运行 RA/Story/CodingPlan Gate；只运行受 changed path/evidence/review 影响的节点。

**Step 3: 断言 stale**

Review 后修改 changed path，GovernanceClosed 必须失效，Completed 被拒绝。

**Step 4: 运行**

Run: `cargo test -p ae-sdd-daemon --test incremental_governance_process`  
Run: `cargo test -p ae-sdd-integrations --test execution_to_completion_e2e --test lifecycle_control_plane_e2e`  
Expected: PASS。

**Step 5: Commit**

```powershell
git add bins/ae-sdd-daemon/tests/incremental_governance_process.rs crates/ae-sdd-integrations/tests/execution_to_completion_e2e.rs crates/ae-sdd-integrations/tests/lifecycle_control_plane_e2e.rs
git commit -m "test(governance): verify incremental completion flow"
```

---

## P2 — 自举隔离、遥测与发布

### Task 19: 实现 Stable Governor / Candidate ReadOnly 角色

**Objective:** self-update 时候选 daemon 永远不能写正式 authority。

**Files:**
- Modify: `bins/ae-sdd-daemon/src/main.rs`
- Modify: `crates/ae-sdd-integrations/src/endpoint.rs`
- Create: `crates/ae-sdd-integrations/src/candidate.rs`
- Modify: `crates/ae-sdd-integrations/src/lib.rs`
- Modify: `crates/ae-sdd-runtime/src/config.rs`
- Modify: `crates/ae-sdd-runtime/src/service_protocol.rs`
- Create: `bins/ae-sdd-daemon/tests/stable_candidate_isolation.rs`

**Step 1: 写 isolation RED test**

candidate 独立 state dir/endpoint；mutation operation 必须返回稳定 forbidden error；正式 endpoint manifest 仍指向 stable。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-daemon --test stable_candidate_isolation`  
Expected: FAIL。

**Step 3: 增加 daemon role**

`serve --role stable-governor|candidate-read-only`；candidate 使用 Rejecting/ReadOnly business mutation boundary。

**Step 4: 共享 Cargo lock**

stable/candidate 使用同一 per-user resource lock root，避免 candidate benchmark 与稳定任务争抢 Cargo。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-daemon --test stable_candidate_isolation`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add bins/ae-sdd-daemon/src/main.rs bins/ae-sdd-daemon/tests/stable_candidate_isolation.rs crates/ae-sdd-integrations/src/endpoint.rs crates/ae-sdd-integrations/src/candidate.rs crates/ae-sdd-integrations/src/lib.rs crates/ae-sdd-runtime/src/config.rs crates/ae-sdd-runtime/src/service_protocol.rs
git commit -m "feat(runtime): isolate stable and candidate daemons"
```

### Task 20: 实现 promote/drain/rollback 流程

**Objective:** 通过候选验证后原子切换，失败时恢复上一完整 native release。

**Files:**
- Modify: `crates/ae-sdd-build/src/release.rs`
- Modify: `crates/ae-sdd-build/src/service/executor.rs`
- Modify: `crates/ae-sdd-build/src/jobs/admin.rs`
- Modify: `bins/ae-sdd-cli/src/main.rs`
- Create: `crates/ae-sdd-build/tests/self_hosting_cutover.rs`
- Modify: `crates/ae-sdd-runtime/tests/shadow_canary_no_double_write.rs`

**Step 1: 写 cutover RED test**

candidate parity 未 PASS、stable 未 drain、Cargo lock 未释放时都拒绝 promote。

**Step 2: 运行 RED**

Run: `cargo test -p ae-sdd-build --test self_hosting_cutover`  
Expected: FAIL。

**Step 3: 实现 promote**

验证 release digest -> stable drain -> 等待连接/job/lock quiesce -> 原子切换 -> 新 stable health。

**Step 4: 实现 rollback**

新 stable health 失败时恢复 previous release；不重新启用 Python runtime。

**Step 5: 运行 GREEN**

Run: `cargo test -p ae-sdd-build --test self_hosting_cutover && cargo test -p ae-sdd-runtime --test shadow_canary_no_double_write`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add crates/ae-sdd-build/src/release.rs crates/ae-sdd-build/src/service/executor.rs crates/ae-sdd-build/src/jobs/admin.rs crates/ae-sdd-build/tests/self_hosting_cutover.rs bins/ae-sdd-cli/src/main.rs crates/ae-sdd-runtime/tests/shadow_canary_no_double_write.rs
git commit -m "feat(release): promote candidate through stable governor"
```

### Task 21: 增加性能与 Token 遥测

**Objective:** 可量化恢复速度、重复读取、Gate cache 和 Token 改善。

**Files:**
- Modify: `tools/lib/runtime_stats.py`
- Modify: `tools/tests/test_runtime_stats.py`
- Modify: `tools/tests/test_cli_perf.py`
- Modify: `crates/ae-sdd-integrations/src/jobs/perf.rs`
- Modify: `crates/ae-sdd-runtime/src/execution_supervisor.rs`
- Create: `crates/ae-sdd-integrations/tests/execution_perf_job.rs`

**Step 1: 定义无敏感内容 metrics**

- `resumeToFirstPatchMs`
- `resumeContextBytes`
- `authorityRefreshCount`
- `sourceReadBytes/sourceReadCacheHits`
- `noProgressBatches`
- `focusedTestsBeforeGreen/broadTestsBeforeGreen`
- `gateEvaluated/gateCacheHits`
- `toolOutputBytes`
- `usedTokenDelta`
- `sliceDurationMs`

**Step 2: 写 RED tests**

事件不得包含 prompt、源码正文、token secret 或原始命令环境变量。

**Step 3: 实现采集**

Token 使用优先消费 host pressure `usedTokens`；host 提供 input/output token 时作为可选细分。

**Step 4: 更新 perf doctor**

对 authorityRefresh>1、broadBeforeGreen>0、noProgress>=3、cache hit<60%、capsule>16KiB 给出稳定建议 code。

**Step 5: 运行 GREEN**

Run: `python -m pytest tools/tests/test_runtime_stats.py tools/tests/test_cli_perf.py -q`  
Run: `cargo test -p ae-sdd-integrations --test execution_perf_job`  
Expected: PASS。

**Step 6: Commit**

```powershell
git add tools/lib/runtime_stats.py tools/tests/test_runtime_stats.py tools/tests/test_cli_perf.py crates/ae-sdd-integrations/src/jobs/perf.rs crates/ae-sdd-integrations/tests/execution_perf_job.rs crates/ae-sdd-runtime/src/execution_supervisor.rs
git commit -m "feat(perf): measure execution token and latency budgets"
```

### Task 22: 更新 SKILL、operation standard 与 AGENTS

**Objective:** 让 Agent 默认采用新短循环，并把规则放回正确 SSOT。

**Files:**
- Modify: `source/skills/phase2-coding/coding-skill.md`
- Modify: `source/skills/orchestration/ae-sdd-update-skill.md`
- Modify: `source/standards/operation-protocol.md`
- Modify: `source/SKILL.md` only for route/index pointers
- Modify: `D:\Item\ae-sdd\AGENTS.md`
- Modify: `D:\al-agent-workspace\al-skills\codex\AGENTS.md`
- Modify: `C:\Users\EDY\.codex\AGENTS.md`
- Generated by script: `dist/ae-sdd/**`

**Step 1: 写源文档检查 RED test**

检查 coding skill 只描述执行环；update skill 只描述维护边界；不复制完整 DTO/Gate 表。

**Step 2: 规则落位**

coding skill 增加：

1. 一次 `resume-approved-plan`；
2. 接收 full/delta/no-change；
3. 一次只执行 active slice；
4. focused GREEN 前不跑 broad；
5. 三批无进展停止调查；
6. evidence 只存 digest/locator；
7. plan scope 内不重新审批。

**Step 3: 更新 AGENTS**

把上述规则压缩为执行约束，不复制实现细节；保持全局与 workspace mirror 完全一致。

**Step 4: 重建 generated runtime**

Run: `python scripts/build_dist.py`  
禁止手改 `dist/`。

**Step 5: 验证生成一致性**

Run: `python tools/bin/ae-sdd update-check --affected source/skills/phase2-coding/coding-skill.md source/skills/orchestration/ae-sdd-update-skill.md source/standards/operation-protocol.md --json`  
Expected: PASS。

**Step 6: Commit**

只 stage 实际 source、AGENTS 和脚本生成的对应 dist 变更；先检查 scoped diff。

### Task 23: 兼容收缩、全回归与发布判定

**Objective:** 证明新路径可发布，并以可回滚方式收缩旧路径。

**Files:**
- Modify as required: `tests/fixtures/compatibility/legacy-surface.v1.json`
- Modify as required: `tests/fixtures/compatibility/cli-routing.v1.json`
- Modify: `crates/ae-sdd-build/tests/compatibility_routes.rs`
- Modify: `crates/ae-sdd-build/tests/migration_oracle.rs`
- Modify: `source/docs/ae-sdd-design.md`
- Modify: `source/docs/ae-sdd-implementation-architecture.md`

**Step 1: focused regression**

```powershell
cargo test -p ae-sdd-contracts
cargo test -p ae-sdd-execution
cargo test -p ae-sdd-flow
cargo test -p ae-sdd-gates
cargo test -p ae-sdd-context
cargo test -p ae-sdd-runtime
cargo test -p ae-sdd-integrations
```

Expected: PASS。

**Step 2: formatting and strict lint**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: PASS。

**Step 3: workspace regression**

Run: `cargo test --workspace --all-targets`  
Expected: PASS。

**Step 4: Python oracle**

Run: `python -m pytest tools/tests -q`  
Expected: PASS；canary/sole-writer tests不得启动 Python production fallback。

**Step 5: real daemon process and cutover**

运行 P0/P1/P2 新增 process tests，以及 install/start/status/drain/upgrade/stop/uninstall、shadow/canary/rollback/no-double-write。

**Step 6: performance comparison**

使用同一 golden fixture 比较 baseline 与 candidate：

- Token 降低 >=60%；
- resume-to-first-patch <=5 min；
- capsule <=16 KiB；
- broad-before-green=0；
- authority refresh=1；
- request timeout=0。

**Step 7: independent Review**

Review 只写 `status/findings`。任何 P0/P1 finding 修复后只重跑受影响 slice/Gate，再跑最终 targeted regression。

**Step 8: release**

只有 milestone=GovernanceClosed、evidence finalized、Review PASS、全回归 PASS、candidate parity PASS 才 promote。

## 7. 验证矩阵

| ID | Acceptance criterion | Evidence |
| --- | --- | --- |
| V-EFF-001 | approved plan 可生成 deterministic queue/capsule | `execution_capsule_contract`、`capsule_builder` |
| V-EFF-002 | resume full/delta/no-change 且 <=16 KiB | `execution_capsule_projection`、process E2E |
| V-EFF-003 | 每次 resume authority refresh <=1 | authority test + telemetry |
| V-EFF-004 | 三批无进展后停止继续调查 | `supervisor_progress_policy` |
| V-EFF-005 | focused GREEN 前 broad test 次数为 0 | hook guard + process E2E |
| V-EFF-006 | source read cache 按 digest/range 正确失效 | `source_read_cache` |
| V-EFF-007 | Cargo 全局锁串行且崩溃可释放 | `cargo_resource_lock` |
| V-EFF-008 | Gate DAG 只重跑受影响节点 | `incremental_gate_dag` |
| V-EFF-009 | evidence ledger append-only、可重建、可检测篡改 | `evidence_ledger` |
| V-EFF-010 | lifecycle 四个终态语义不可跳级 | `completion_milestones` |
| V-EFF-011 | Review contribution 独立提交、finalize 一次聚合 | `review_contribution_e2e` |
| V-EFF-012 | candidate 不写正式 authority | `stable_candidate_isolation` |
| V-EFF-013 | 典型执行 Token 降低 >=60% | golden baseline/candidate perf evidence |
| V-EFF-014 | 计划内变更不触发重新审批 | execution authority E2E |
| V-EFF-015 | 完整路径无 request body timeout | real daemon process trace |

## 8. 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| operation registry 扩展影响大量 fixture | 先冻结 operation schema；新 CLI 用 native subcommand，不把新能力伪装成 legacy route |
| Hook host 无法提供 executionEvent | capability-gated rollout：shadow 记录、canary 告警、sole-writer 才强制；CLI 从 tool name/input 做保守分类 |
| strict broad-test policy 阻塞未知测试命令 | Story verification matrix 必须标注 focused/broad；未知命令默认为 Other，不冒充 focused |
| capsule 16 KiB 不够 | queue 全量放 artifact；capsule 只放 active slice、refs、digest 和预算 |
| Gate DAG 声明错误导致漏跑 | selector 缺失 fail closed 为重新评估；DAG cycle 启动失败；最终 workspace regression 不被增量 Gate 替代 |
| evidence JSONL append 的原子性 | 继续走 ProjectMutationStore/journal；语义 append-only，不用裸 append 写 |
| review.contribute 无全局 lease 产生竞争 | WorkItem actor 串行 + input/ruleset fingerprint CAS + idempotency；finalize 使用短 writer lease |
| runtime SQLite 与项目 authority 不一致 | project digest/locator 优先；SQLite 不匹配即丢弃重建 |
| self-hosting 双 daemon 双写 | candidate 强制 read-only、独立 endpoint/state、正式 endpoint 只由 stable 发布 |
| metrics 泄露 prompt/源码/token | 仅记录计数、digest、分类和字节数；测试禁止敏感字段 |
| 工作树已有大量未提交变更 | 每 Task scoped diff/stage；共享文件冲突时停止，不覆盖用户改动 |

## 9. 回滚策略

1. 所有新能力通过 `execution-supervisor-v1` capability 暴露。
2. P0 shadow 阶段只记录 supervisor decision；除 stale authority 外不强制拒绝。
3. canary 阶段强制 broad-before-green、budget 和 Cargo lock；保留 `review.record` adapter。
4. sole-writer 稳定后才默认使用 split review 与 ledger projection。
5. 回滚时禁用 capability，恢复旧客户端路径；项目 ledger/queue 均为追加或新 artifact，不删除旧 manifest。
6. candidate 失败时切回上一完整 native release；不回退到 Python production runtime。
7. migration 0011 只含可重建 metadata，旧 binary 可忽略新表；禁止 down migration 删除项目证据。

## 10. 完成定义

只有同时满足以下条件才可以宣称完成：

- 新 PRD/对应 Story 的 approved `state.executionPlan` 已执行完；
- P0/P1/P2 验证矩阵全部有真实 evidence；
- `FlowRuntime` 仍是唯一 phase/nextAction owner；
- capsule/full/delta/no-change、slice supervisor、Gate DAG、evidence ledger、review split、milestones、stable/candidate 均有真实 daemon process test；
- `cargo fmt --check`、strict Clippy、focused tests、workspace regression、Python oracle 全 PASS；
- golden trace Token 降低至少 60%，resume 到首个 patch 不超过 5 分钟；
- Review `status=passed` 且 findings 为空；
- completion milestone 为 `GovernanceClosed`，随后才提交 `Completed`；
- generated `dist/` 来自 build script，没有手工编辑；
- 全局 `AGENTS.md` 与 workspace mirror 一致。

