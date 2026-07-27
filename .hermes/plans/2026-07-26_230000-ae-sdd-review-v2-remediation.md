# Review Batch v2 缺陷整改 Plan

- Work Item: PRD-AE-SDD-RUST-DAEMON-001
- Story: STORY-AE-SDD-C1-INTEGRATION-001
- 来源: 2026-07-26 V-024 交接实施过程中实测发现
- 前序 plan: `.hermes/plans/2026-07-26_164000-ae-sdd-daemon-v024-handoff.md`

## 0. 本文档的定位与证据标准

本文档记录 V-024 实施过程中**实测发现**的设计不当与实现缺陷。分三类:

- **第 1 节**: 已确认且已修复。作为历史与回归依据,不是整改目标。
- **第 2、3 节**: 已确认但**未修复**。这些是本次整改的目标。
- **第 4 节**: 流程与验证方法问题。影响后续所有工作的可信度。

证据标准: 每条都注明是「实测确认」还是「推断待验」。未经执行验证的判断不写入
整改目标。Task 9 的三份独立 Review (BE/AR/QA) 仍在运行,其 findings 可能追加
条目,届时以增量方式并入第 2、3 节。

## 0.1 安全约束(整改期间持续有效)

- 工作树刻意保持 dirty,含其他 agent 的在途工作。禁止 `git reset`、
  `git checkout --`、`git clean`、`git stash`、全量重格式化。
- `crates/ae-sdd-contracts` 与 `migrations/*.sql` 冻结。不得改写、不得重编号。
- 一律 fail closed。不得放宽 CHECK、不得对未注册枚举做兜底、不得在耐久状态
  缺失时返回成功。
- 不得用 `#[allow(...)]` 消警告。

## 1. 已确认并已修复(历史记录)

### 1.1 `persistence.rs` 126 处续行符吞空格,生成非法 SQL

Rust 字符串字面量行尾 `\` 会吞掉换行**与下一行前导空白**,拼出 `SETstatus`、
`updated_atWHERE`、`last_event_seqAND`。实测报错:
`near "SETstatus": syntax error`。已在续行符前补空格修复。

### 1.2 `ReviewBatchStatusV2` wire 与 DB 域不一致

契约 wire 为大写 `VALID_CLEAN`,migration 0009 的 CHECK 要求小写 `valid_clean`,
两者均冻结。实测报错: `CHECK constraint failed: latest_status IN(...)`。已新增
`projection_batch_status()` 做映射,未注册值 fail closed。

### 1.3 `matches!(status, "valid_clean" | "valid_findings")` 恒为 false

同 1.2 的大小写不一致导致该判断永假,`attempted_count` / `effective_count`
被静默算错。这是**静默错值**,比报错更危险。随 1.2 一并修复。

### 1.4 `persist_review_remediation` 父子身份绑错

remediation 行的 `review_id` 绑到了**子** session,而外键按**父** review 建,
且 `CHECK(next_review_id<>review_id)` 被直接违反 —— 任何 findings → remediation
→ 子会话序列必然失败。另有 `target_revision = source + 1` 描述了一个从未存在的
版本。已改为从类型化 `parentReviewId` 推导父身份并加 fail-closed 校验。

### 1.5 `story_document` 返回未 join 的相对路径

`safe_document_path(root, value)` 内部用 `root.join(value)` 校验,但
`story_document` 丢弃 join 结果、返回原始相对 `docPath`。调用方
`plan_story_aligned` 于是对着**进程 CWD** 读取,静默读不到文档,`ac_ids` 返回
空集,G-14 以错误理由 FAIL。已修复为返回校验时实际使用的已解析路径。

## 2. 设计不当 —— 整改目标

### 2.1 [高] 整段代码路径从未执行过

**实测证据**: 1.1~1.4 四个缺陷**同时**潜伏在同一条路径上。126 处非法 SQL 无法
在任何一次真实执行中存活;三个域缺陷也一样。唯一解释是 Review Batch v2 投影
写入路径此前从未被执行过。

**为何是设计问题而非单纯漏测**: 缺陷密度说明这段代码是「写完即认为完成」的,
没有任何执行反馈闭环。同类风险在其他未被 e2e 覆盖的路径上同样存在。

**整改方向**: 识别所有「已实现但无执行覆盖」的耐久写入路径,建立最小可执行
探针。判据不是行覆盖率,而是「该路径是否至少被真实执行过一次」。

### 2.2 [高] `host.register` / `host.action_next` / `host.action_ack` 无任何实现

**实测证据**: 仓库仅有 `ae-sdd-cli` / `ae-sdd-daemon` / `ae-sdd-worker` 三个
bin。`claude-code` / `codex` 仅是 `ae-sdd-build` 的 hook 安装目标,不是会话生成
器。运行时库 `adapters=0, delegations=0, attestations=0, host_actions=0`。

**后果**: 物理 attestation 是 Tier 2+ Review 的**强制**前置(
`delegation_supervisor.rs::accept` 缺 ack 直接报
`host ACK is required before child claim`),但这条链在 mock 之外从未跑过。
一个强制性权威机制没有可运行的参考实现。

**整改方向**: 提供一个最小参考宿主适配器(可作为 example 或 test-only bin),
使 attestation 链可在 CI 中真实走通。这是 2.1 的一个具体实例,但优先级独立,
因为它阻塞的是权威模型的核心。

### 2.3 [中] 校验函数算出解析值后丢弃

**实测证据**: 1.5 的根因。`safe_document_path` 计算 `root.join(value)` 用于
校验,随后丢弃,调用方拿到未解析的原值。

**为何是设计问题**: 这是一个**可复制的反模式** —— 校验与解析被拆开,校验通过
不代表调用方拿到的是被校验的那个东西。

**整改方向**: 扫描全仓同类模式(校验函数内部构造 canonical 值但返回原始输入),
改为返回已解析值或同时返回两者。此项为**推断待验**: 除 1.5 外尚未确认其他
实例,需先扫描。

### 2.4 [中] 幂等 upsert 使 no-op 对行检查不可观测

**实测证据**: 前序 plan 的 Task 2 前提是「删掉早退逻辑 → 行计数测试会 FAIL」。
实测**不成立**: 下游 `persist_review_session/_batch/_attempt/_contributions/
_exit_receipt` 全是带身份谓词的 `INSERT ... ON CONFLICT DO UPDATE`,第二次 apply
会原样重写相同值,行数与内容均不变。按 plan 字面写出的测试是**空洞的**。

**已采用的解法**: 跨连接 `PRAGMA data_version`。SQLite 仅在**其他**连接真正
提交页变更时递增该计数器,计数器不动即证明零写入。

**整改方向**: 把这个证明手法固化为项目约定并文档化。凡断言「重放是 no-op」的
测试,必须使用写入检测型 observable,不得用行计数。否则同类空洞测试会反复出现。

### 2.5 [中] 多 reviewer 的 lease 串行化未在授权模型中体现

**实测证据**: Tier 2+ 需要多个专业依次记录,而 Work Item lease 独占且
owner-bound(`LeaseOwner` 由认证 session 派生),AR 无法借用 BE 的 lease
(实测 `LEASE_MISMATCH`)。正确流程是每个 reviewer 依次 acquire → record →
release。但 Reviewer grant 的默认操作集里**没有** `lease.release`。

**为何是设计问题**: 「多 reviewer 必须串行传递独占 lease」是一条真实的协议
约束,却既未文档化,也未体现在 grant 模型的默认值里。任何按直觉编排多
reviewer 的调用方都会撞上 `LEASE_MISMATCH`,且错误消息不指向真实原因。

**整改方向**: 要么把 `lease.release` 纳入 Reviewer 角色的默认 grant,要么在
`ManageOwnLease` 的文档与错误消息中显式说明串行传递要求。二者择一,不要都不做。

### 2.6 [低] wire 与 DB 枚举大小写约定不一致

**实测证据**: 1.2 的根因。契约用 UPPER_SNAKE,DB 用 lower_snake,两侧均冻结,
只能在中间层做映射。

**整改方向**: 无法通过改契约或改 migration 解决(均冻结)。整改内容是**建立
约定并加护栏**: 新增枚举列时必须显式声明映射方向,并要求映射函数对未注册值
fail closed。可考虑加一个测试遍历所有 review 枚举,断言 wire 值与 DB 域之间
存在显式映射。

## 3. 实现缺陷 —— 整改目标

### 3.1 [高] `sqlite_error` 丢弃底层错误,诊断成本极高

**实测证据**: `persistence.rs:3009`:

```rust
fn sqlite_error(_error: rusqlite::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime SQLite operation failed",
    )
}
```

底层错误被 `_error` 丢弃。本次会话中我三次不得不临时打补丁把它打印出来才能定位
1.1、1.2 和一处 UNIQUE 冲突。同一函数 `store_error` 有相同问题。

**为何是缺陷而非有意脱敏**: 对外响应脱敏是正确的,但错误在**跨越进程边界之前**
就已丢失,连守护进程自己的日志也拿不到。这不是脱敏,是信息销毁。

**整改方向**: 保留 `StableErrorCode` 与对外消息不变(协议契约),但把底层错误
接入 daemon 的结构化日志或 `remediation` 字段。必须确认不会把工作区路径、
SQL 参数值等敏感内容泄漏到对外响应里。

### 3.2 [中] `write_attestation` 无条件 INSERT,幂等性依赖跨层前置检查

**实测证据**: `persistence.rs:416` 是裸 `INSERT INTO delegation_attestation_v1
(...) VALUES(...)`,非 upsert。同一 attestation 以不同 idempotency key 重放会撞
`UNIQUE(workspace_id, delegation_id, physical_session_id, attestation_digest,
grant_digest)`。本次会话的 review_gate_e2e 夹具实际踩到过。

**当前是否活跃缺陷**: 不是。真正的重放被 `commit_identity_bundle` 的 receipt
前置检查挡住了。

**为何仍需整改**: 保护它的守卫在**另一层**。同一事务内的 INSERT 自身没有幂等
保证,任何绕过或重构该前置检查的改动都会让它变成活跃缺陷,而且表现为
`EXTERNAL_STATE_CONFLICT` 而非清晰的诊断。

**整改方向**: 让 INSERT 自身表达意图 —— 要么改 upsert 并保持 digest 不变量,
要么在 INSERT 前显式断言不存在,给出精确错误。同时在紧邻处注释说明前置检查
在哪一层。

### 3.3 [低] release 构建 2 个 warning

**实测证据**: `business.rs:69` 的 `Operation(String)` 字段 0 从未被读,
`business.rs:74` 的 `scope_matches` 从未被用。二者属
`ProcessAbortCommitFault`,由 `#[cfg(debug_assertions)]` 门控,release 下消费方
消失。debug 构建 0 警告。

**整改方向**: 给该类型加上与其消费方一致的 cfg 门控,使 release 下不参与编译。
不得用 `#[allow(dead_code)]` 消警告。

## 4. 流程与验证方法问题

这些不是代码缺陷,但直接决定本次及后续所有结论的可信度。**若不整改,后续
整改结果本身也不可信。**

### 4.1 [高] subagent 报告完成但改动从未落盘

**实测证据**: 前序 Task 8 的 subagent 报告 `review_gate_e2e` 15 passed 且已修
四个夹具。实测为 11 passed / 4 failed,文件 mtime 仍停在 Task 6 的时间戳。它
声称的修复从未到达磁盘。我自己也有一次 Write 报成功但文件不存在(由独立
workflow 核实 `docs/plans/` 下无该文件)。

**整改方向**: 任何「已完成」声明必须附带**可复核的数值证据**(测试计数变化、
文件 mtime、回读内容),且由声明方之外的一方核验。本 plan 后续所有 Task 都
必须遵守。

### 4.2 [高] 工具环境静默失效 5 次

**实测证据**: 本次会话 Bash / Read / Grep 连续返回空输出共 5 次;两次 subagent
分别死于 `server_error` 与 `Connection closed mid-response`;一次 workflow 的
三个阶段全部因 401 未启动。

**后果**: 我曾在环境已不可信的情况下继续重试,陷入循环。

**整改方向**: 固化判据 —— 同一工具连续两次返回空即视为环境失效,立即停止重试
并切换执行路径(workflow / subagent 独立环境),而非继续在失效环境内尝试。

### 4.3 [中] 存在与本工作无关的既有失败

**实测证据**: `cargo test -p ae-sdd-cli --test legacy_rpc_adapter` 失败 1 项。
在 HEAD 的干净 worktree 上实测 7/7 通过;逐文件回灌定位到**他人未提交**的
`registry.rs` 改动把 `GateEvaluate` 的 `requires_idempotency` 由 `false` 翻为
`true`。该文件本工作全程未触碰。

**整改方向**: 不由本工作修复。需与该改动的负责人对齐 —— `GateEvaluate` 是否
真的应当要求 idempotency key;若是,`legacy_rpc_adapter` 的期望需同步更新。

### 4.4 [中] 证据强度弱于验收措辞之处

诚实记录两处,不得在验收清单上按「已达标」勾选:

- **Tier 3 per-join 归因**: 测试证明了 job / receipt locator / manifest /
  journal 四项联查的**聚合**行为,但未分别归因。四项中单项回归可能不被发现。
- **崩溃恢复的 happens-before**: 场景 2 在可观测结果上投影恢复先于成功返回,
  但真正的因果证明需要在「投影写入」与「响应返回」之间注入失败点。未做。

覆盖率(`cargo llvm-cov`)属前序 plan 的 Task 11,不在本次范围,状态为
**未验证**而非达标。

## 5. 整改任务

任务顺序按依赖排列。每个任务完成后必须给出可复核数值证据(见 4.1)。

### Task 1: 恢复 SQLite 错误可诊断性

- 目标: 3.1。修 `sqlite_error` 与 `store_error`。
- 文件: `crates/ae-sdd-integrations/src/persistence.rs`
- 要求: 对外 `StableErrorCode` 与消息**不变**(协议契约),底层错误进结构化
  日志或 `remediation`。必须验证不泄漏工作区路径与 SQL 参数值。
- 先做本项,因为它降低后续所有任务的诊断成本。
- 验收: 构造一个已知的 CHECK 失败,确认底层原因可从日志取得,且对外响应未变。

### Task 2: 让 `write_attestation` 自身表达幂等意图

- 目标: 3.2。
- 文件: `crates/ae-sdd-integrations/src/persistence.rs`
- 要求: 先写一个失败测试 —— 同一 attestation 以不同 idempotency key 重放。
  再决定改 upsert 还是显式前置断言。不得放宽 UNIQUE。
- 验收: 该测试转绿,且原有 `runtime_identity_persistence` 测试不回归。

### Task 3: 修 release 构建 warning

- 目标: 3.3。
- 文件: `crates/ae-sdd-integrations/src/business.rs`
- 要求: cfg 门控与消费方一致。禁止 `#[allow]`。
- 验收: `cargo build --workspace --release` 0 warning,且 debug 下失败点仍可用
  (`c1_control_plane_process` 6/6 保持绿)。

### Task 4: 提供最小参考宿主适配器

- 目标: 2.2。
- 要求: 实现 `host.register` / `host.action_next` / `host.action_ack`,使
  attestation 链可在 CI 中真实走通。可作为 example 或 test-only bin。
- 验收: 一个自动化测试完成 `host.register` → `delegation.create` →
  `host.action_next` → `session.open` → `host.action_ack` → `delegation.accept`
  全链并落库真实 attestation,不使用任何 mock 旁路,FK 全程开启。
- 注: 这是本 plan 中最大的一项,且是 Tier 2+ Review 能否在 CI 中被验证的前提。

### Task 5: 扫描并修「校验后丢弃解析值」反模式

- 目标: 2.3。**先验证再修**。
- 要求: 先扫描全仓,列出所有「校验函数内部构造 canonical 值但返回原始输入」的
  实例。若除已修的 `safe_document_path` 外无其他实例,如实报告并关闭此项。
- 验收: 扫描结果清单 + 每个实例的处置结论。

### Task 6: 固化 no-op 证明约定

- 目标: 2.4。
- 要求: 文档化「断言重放 no-op 必须使用写入检测型 observable」,并说明行计数
  在幂等 upsert 下不可用及其原因。可考虑加 lint 或 review checklist 项。
- 验收: 约定成文,且现有 `review_projection_replay_of_identical_write_is_a_no_op`
  被引为参考实现。

### Task 7: 处置 reviewer lease 串行化

- 目标: 2.5。二选一,不得都不做。
- 要求: 要么把 `lease.release` 纳入 Reviewer 默认 grant,要么在文档与
  `RoleOperationForbidden` 的错误消息中显式说明串行传递要求。
- 验收: 一个按直觉编排多 reviewer 的调用方能从错误消息本身推断出正确做法。

### Task 8: 枚举映射护栏

- 目标: 2.6。
- 要求: 加测试遍历 review 相关枚举,断言 wire 值与 DB CHECK 域之间存在显式
  映射,且未注册值 fail closed。
- 验收: 该测试对现有枚举全绿;人为新增一个未映射变体时该测试转红。

### Task 9: 识别无执行覆盖的耐久写入路径

- 目标: 2.1。这是根因项,放在最后是因为它需要前面各项的经验输入。
- 要求: 列出所有「已实现但从未被真实执行」的耐久写入路径,给出最小探针。判据
  是「是否至少真实执行过一次」,不是行覆盖率。
- 验收: 路径清单 + 每条的执行证据或明确的缺口声明。

### Task 10: 对齐 `registry.rs` 的 idempotency 改动

- 目标: 4.3。**不由本工作单方面修复。**
- 要求: 与该改动负责人确认 `GateEvaluate` 是否应要求 idempotency key。若是,
  同步更新 `legacy_rpc_adapter` 期望;若否,回退该字段。
- 验收: `cargo test --workspace --all-targets --all-features` 全绿,或有书面
  结论说明为何保留。

## 6. 验收清单

- [ ] SQLite 底层错误可从日志取得,对外响应契约未变,无敏感值泄漏
- [ ] `write_attestation` 重放语义由自身表达,有失败测试保护
- [ ] `cargo build --workspace --release` 0 warning,未使用 `#[allow]`
- [ ] attestation 全链在 CI 中真实走通,无 mock 旁路,FK 全程开启
- [ ] 「校验后丢弃解析值」反模式已扫描,每个实例有处置结论
- [ ] no-op 证明约定成文,行计数不可用的原因已说明
- [ ] reviewer lease 串行化已文档化或已纳入默认 grant
- [ ] 枚举映射护栏测试存在,未映射变体会使其转红
- [ ] 无执行覆盖的耐久写入路径已列清,每条有执行证据或缺口声明
- [ ] `registry.rs` 的 idempotency 改动已对齐,全量回归全绿
- [ ] 每项整改均有可复核数值证据,且由声明方之外一方核验(见 4.1)

## 7. 待并入

Task 9 的三份独立 Review (BE / AR / QA) 仍在运行。其 findings 到达后以增量方式
并入第 2、3 节,并在第 5 节追加对应任务。**在 findings 并入之前,本 plan 不应
被视为完整。**

