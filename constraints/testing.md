# Rust 测试规范

## 摘要

本文件定义 ae-sdd Rust runtime 的测试分层、工具、fixture、并发/崩溃/跨平台边界、覆盖率和真实 evidence 要求。
适用场景：Story 验证矩阵、实现、回归、release/cutover 和 Code Review。

---

## 一、框架与工具

| 用途 | 默认工具 |
| --- | --- |
| unit/integration | Rust built-in test harness + `cargo test` |
| async/time | `tokio::test`、paused time、injectable clock |
| property/model | `proptest`（必要时 loom 验证小型并发模型） |
| golden/snapshot | versioned JSON/text fixtures；`insta` 仅在 reviewable snapshot 场景使用 |
| temp filesystem | `tempfile` + project fixture builder |
| CLI/process | `assert_cmd`/等价 process harness + `predicates` |
| coverage | `cargo llvm-cov` |
| benchmark/load | criterion 或专用 pressure harness + HDR histogram |
| dependency/security | `cargo audit`, `cargo deny` |

版本必须由 workspace dependency 和 `Cargo.lock` 固定；新增 test dependency 也受供应链规则约束。

## 二、测试分层

| 层级 | 边界 | 必须验证 | 不可替代 |
| --- | --- | --- | --- |
| unit | 单个 parser/value/reducer/scanner | 正例、边界、非法输入、exhaustive enum | 不关闭跨进程/持久化 AC |
| property/model | reducer、lease/CAS/fencing、idempotency、event replay | 任意序列不变量、重放确定性、单调性 | 不以少量 happy path 代替 |
| crate integration | public API + real adapter/fake external port | schema、transaction、error mapping、migration | 不访问 production 用户目录 |
| golden differential | Python oracle vs Rust | preserve 相同；breaking-fix 明确不同；无 stub-pass | Python 不进入 release runtime |
| process/IPC | real `ae-sddd` + real `ae-sdd` over Named Pipe/UDS | framing、handshake、deadline、ACL、restart、Hook JSON | in-memory service call 不关闭 AC |
| concurrency/crash | 多 client/process、kill/fault point | single commit、journal recovery、no fake PASS/completed | sleep-based 顺序不作为证据 |
| host contract | fake host matrix + enabled host live smoke | ACK≠claim、physical identity、cancel/timeout、authenticated pressure/hysteresis、compact ACK | daemon 自建 logical row 或 projection bytes 不算 physical session/token telemetry |
| migration/cutover | legacy/shadow/canary/sole-writer | double-write=0、drain、rollback、release scan | 手工观察不关闭 AC |
| cross-platform | actual Windows/macOS/Linux CI runners | IPC、ACL、atomic/fsync、service lifecycle | 单一 OS 或 `--platforms` 模拟不代表全平台 |
| pressure | release profile，100 sessions/10 workspaces mixed workload | fairness、no lost mutation、fixed warmup/sample、p50/p95/p99/max/CPU/RSS | debug build 或平均值不替代 release percentile |

## 三、AC 验证最小矩阵

- Rust-only：release binary/package/Hook config 扫描，Python worker/fallback 入口计数必须为 0。
- workspace/session：10 workspace、100 session、canonical path alias 与 cross-workspace negative cases。
- Hook fail-closed：daemon kill、protocol mismatch、deadline、duplicate hookEventId、engaged/未 engaged 矩阵。
- Store：multi-process lock、lease expiry/break、revision CAS、fencing monotonicity、idempotency conflict、kill-point recovery。
- Gate：36 Gate + 7 scanner inventory，六类 GateOutcome truth table，state/policy/inventory/input 改变后的 STALE。
- Flow：相同 state/event/policy/input restart/replay 的 decision digest/nextAction 相同；child transition 被拒绝；prompt-only correction delta=0。
- Delegation：root→series→task/reviewer maxDepth、role/path/operation matrix、fake/duplicate/out-of-order/timeout ACK、reviewer physical isolation。
- ChildResult/context：64 KiB/8 KiB/64 KiB 边界、path/hash/required deliverable、跨 role memory leakage=0、delta/no-change。
- Compact：authenticated high/low/consecutive/cooldown samples、supported/unknown/unsupported/wrong session/wrong generation/timeout/restart；无匹配 ACK + rehydrate 时 false context-restored=0。
- Cross-platform/cutover：install/start/status/drain/upgrade/stop/uninstall、endpoint ACL、shadow/canary/rollback/sole-writer。

Story 每个 AC 必须至少映射一个可执行验证入口和预期 evidence；不能只写“人工确认”。

## 四、fixture 与 golden corpus

- fixture 必须有稳定 ID、schema version、输入、预期 outcome、owner、来源和 `preserve | breaking-fix | removed-deprecated | native-addition` 分类；`native-addition` 专用于无 Python 前身的 Rust 原生能力，此类条目没有差分 oracle。
- 113 command、23 operation（18 个迁移自 Python 面 + 5 个 `native-addition`）、36 Gate、7 scanner 和工程工具必须由 parser/registry/runtime trace 三源 inventory 对齐，计数与映射由测试生成审计。
- golden 只存确定性输出；timestamp、absolute temp path、UUID、port/pipe name 必须 normalization 后比较。
- 更新 golden 必须展示 semantic diff 并由 Story/DR 解释；禁止用 blanket accept 命令覆盖不理解的差异。
- Python oracle 只能在 `migration_oracle` profile/read-only fixture 中运行；Rust canary/sole-writer test 禁止调用它。

## 五、test double 边界

- pure reducer/port consumer 可以使用 deterministic fake；fake 必须能注入 timeout、cancel、panic/error、duplicate、reorder 和 stale。
- AC 验收禁止 mock 内部 TransitionPolicy、WorkItemActor、StateStore atomic write、Gate freshness 或 protocol framing。
- filesystem/SQLite acceptance 使用临时真实目录/数据库；禁止生产目录和用户 runtime DB。
- external Agent host/OS service 可使用 contract fake，但任何标记为 supported 的宿主/平台必须有 live smoke evidence。
- fake ACK 不得自动创建 child claim；测试必须明确证明 ACK-only 不能进入 running。

## 六、并发、时间与故障注入

- 禁止 `Thread.sleep()`、`tokio::time::sleep()` 或任意延时用来断言并发顺序；使用 barrier、Notify、channel、fake clock 和 paused time。
- randomized concurrency test 必须记录 seed；失败 seed 可直接重放。
- 所有 background task 必须在测试结束前 join/cancel；禁止遗留 daemon、pipe/socket、DB handle 或 child process。
- fault points 至少覆盖 PREPARED 前后、atomic replace/fsync、COMMITTED 前后、event publish、Gate running、Host ACK/claim、compact ACK/rehydrate。
- crash 测试断言 disk state/receipt/event，不接受仅看 process exit code。
- sanitizer/Miri 可运行的 pure/platform-independent crate 应进入定期 CI；unsafe platform adapter 必须有 target-specific negative tests。

## 七、性能与资源

| 指标 | 门槛 |
| --- | --- |
| warm handshake p95 | <= 50 ms |
| cached read p95 | <= 100 ms |
| cached Hook RPC p95 | <= 50 ms |
| invalidated non-external Hook p95 | <= 250 ms |
| lost/duplicate project mutation | 0 |
| false Gate PASS / false delegation completed / false compact context-restored | 0 |
| cross-workspace/role leakage | 0 |

- performance evidence 必须使用 release profile，并记录机器/OS/build、样本数、warmup、HDR histogram config、p50/p95/p99/max、CPU/RSS 和 error count；debug `cargo test/run` 不得关闭性能 AC。
- Hook load 中同步长作业计数必须为 0；达到 mailbox/queue 上限时验证 backpressure/fail-closed，不允许 OOM。
- benchmark regression 阈值与 baseline 必须 versioned；单次本机噪声不得直接修改产品阈值。

## 八、覆盖率与质量门槛

- 新 Rust workspace 整体 line coverage 必须 >= 80%；`domain/policy/flow/store/delegation/context/protocol` 必须 >= 90%。
- 覆盖率不能替代 AC/negative/property/crash 证据；critical transition/error branch 必须在 verification matrix 中显式命中。
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace --all-features`, `cargo llvm-cov`, `cargo audit`, `cargo deny check` 任一失败均阻断 release。
- flaky test 不得简单 retry 后忽略；必须隔离原因、记录 finding，并在恢复前保持 gate blocked。

## 九、目录与命名

- unit test 名使用 `given_when_then` 或清晰行为名，如 `stale_fencing_token_cannot_commit`。
- crate integration 放 `<crate>/tests/<contract>.rs`；根级场景按 `contract/golden/concurrency/crash/e2e/fixtures` 分类。
- test helper 只放相邻 `tests/support` 或 fixture builder；禁止生产 crate 暴露仅为测试的 public API。
- 测试输出写临时目录；需要 evidence 的摘要由 ae-sdd evidence contract 收集，不提交 ad-hoc TestReport。

## 十、真实 evidence

- evidence 必须来自实际执行，至少记录 command/runner、toolchain/build digest、start/end、exit code、stdout/stderr digest、fixture/seed/input digest 和 AC/verification ID。
- Test 只登记真实 evidence；Review 只登记 `status/findings`。禁止创建 TestReport、CodingReport 或 CodeReview report 文件。
- cancelled、timeout、skipped、unsupported 与 stale 不能登记为 PASS；平台/宿主不支持必须按 Story 的显式降级 AC 判定。

## 十一、禁止事项

- 禁止以 in-memory function call 代替 Named Pipe/UDS、process restart 或 filesystem commit 的验收。
- 禁止只断言 exit code/错误存在；必须断言 stable code、state/revision/event/artifact 不变量。
- 禁止使用 production 用户数据、runtime DB、endpoint token 或 Agent transcript 作为 fixture。
- 禁止 snapshot 包含 secret、absolute user path、随机字段或无界日志。
- 禁止 always-pass、ignored critical test、空 assertion、只覆盖 happy path 或用 retry 隐藏 race。
