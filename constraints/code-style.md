# Rust 代码风格规范

## 摘要

本文件定义 ae-sdd Rust 命名、公开 API、错误、日志、async/concurrency、确定性、序列化和平台代码规范。
适用场景：编码、重构、Code Review 和 AI 生成 Rust 代码。

---

## 一、格式与 lint

- 所有代码必须通过 `cargo fmt --check`，禁止手工对齐产生与 rustfmt 冲突的样式。
- 所有 target/feature 必须通过 `cargo clippy --workspace --all-targets --all-features -- -D warnings`。
- workspace 根必须统一设置 Rust/Clippy lint；crate 只能加严，不得局部 `allow` 隐藏 warning。
- `#[allow(...)]` 必须缩小到最小 item，并用注释说明不变量或上游限制；禁止 crate-wide `allow(clippy::all)`。

## 二、命名与模块

| 对象 | 规则 | 示例 |
| --- | --- | --- |
| crate/module/function/field | `snake_case` | `input_fingerprint` |
| type/trait/enum variant | `UpperCamelCase` | `GateOutcome::TimedOut` |
| constant/static | `SCREAMING_SNAKE_CASE` | `MAX_FRAME_BYTES` |
| stable wire method | `<domain>.<verb>` 小写 | `workspace.register` |
| ID newtype | 领域名 + `Id` | `SessionId`, `DelegationId` |
| error | 领域名 + `Error` | `StoreError`, `ProtocolError` |

- 禁止在多个 crate 使用裸 `String` 表示 workspace/session/workItem/revision 等关键 identity；必须使用 newtype 或明确 DTO field type。
- 禁止创建无 owner 的 `utils`, `common`, `misc` 模块；helper 必须放在拥有该 invariant 的模块。
- bool 字段使用可读谓词，如 `engaged`, `is_stale`, `requires_confirmation`；状态机优先 enum，不用多个互斥 bool。

## 三、公开 API 与注释

- 所有 public type、trait、method 和 wire field 必须有 rustdoc，说明业务语义、错误、幂等、deadline 或安全不变量；显而易见的 private helper 不写复述式注释。
- 复杂并发、atomic file、journal recovery、host attestation 和 `unsafe` 代码必须写“为什么成立”，不能只写“做了什么”。
- public API 必须使用拥有语义的 struct/enum；参数超过 4 个或存在同类型 identity 时必须使用 request/context struct。
- 禁止将 `serde_json::Value` 作为核心 domain/application API；只允许在 versioned extension payload 边界使用并立即验证。

```rust
/// Applies a mutation only when lease, revision and fencing preconditions remain fresh.
pub async fn execute_mutation(
    request: MutationRequest,
    context: MutationContext,
) -> Result<MutationReceipt, OperationError>;
```

## 四、错误与 panic

- library 使用 `Result<T, TypedError>`；错误 enum 用 `thiserror`，保留 source chain，但协议边界只暴露 stable error code、retryability 和脱敏 remediation。
- 禁止在 request、event、Gate、recovery 和 background task 路径使用 `unwrap()`、`expect()`、`panic!()`、`todo!()` 或 `unimplemented!()`。
- 仅测试或进程启动时被静态证明的不变量可使用 `expect`，message 必须说明 invariant；可恢复配置/文件错误仍必须返回 typed error。
- 禁止空 catch 等价行为（`let _ = fallible()`）；明确忽略时必须记录理由或转换成 evidence/metric。
- Gate panic、join error、I/O error 和 timeout 必须映射为 `ERROR/TIMEOUT/CANCELLED/STALE`，禁止映射为 PASS 或业务 FAIL。

## 五、async 与并发

- 禁止在 Tokio executor 上直接运行 blocking filesystem、SQLite、压缩、hash 大目录或同步 process wait；必须进入 runtime-owned 有界 blocking pool。
- 所有 channel、mailbox、queue、semaphore 和 task set 必须有容量与 overflow/backpressure 行为；禁止无界 channel 和 fire-and-forget `spawn`。
- 每个外部 I/O、host action、Gate job 和 child lifecycle 必须传播 deadline/cancellation；取消后必须 join/清理资源。
- 持锁期间禁止 `.await`，除非锁类型和临界区专门支持且有并发测试证明；不得在 actor mailbox 中同步等待长 Gate/host/build job。
- shared state 优先 actor/message 或 immutable snapshot；使用 mutex/RwLock 时必须记录 owner 和 lock ordering，禁止嵌套锁无顺序。
- 时间相关测试使用 injectable clock/Tokio paused time，禁止用 `thread::sleep`/任意延迟证明顺序。

## 六、确定性与状态机

- phase、Gate status、delegation、compact、session 和 workspace mode 必须使用 exhaustive enum + 显式 transition function。
- reducer 输入必须显式包含 state revision、ordered event cursor、policy digest 和 input fingerprint；不得读取 wall clock、random 或全局环境产生隐藏分支。
- 参与 idempotency/decision/artifact digest 的值必须先 canonicalize：稳定 field order、`BTreeMap`、规范 path、UTC time 和明确 null/empty 语义。
- 浮点数不得用于 revision、deadline、size、token budget、event sequence 或 deterministic digest。
- 重放同 command/event 必须返回旧 receipt/no-op 或稳定冲突，不能产生第二次 side effect。
- `ae-sdd-methodology`、`ae-sdd-flow`、`ae-sdd-lifecycle` 的公开决策 API 只能接收 immutable typed input 并返回 frozen DTO/decision/mutation intent；禁止在纯控制面内部读取文件、环境变量、时钟、随机数、数据库或执行 side effect。
- Methodology 的 activation、spawn policy、route predicate、required input、deliverable 与 Gate 必须来自 versioned machine-readable Catalog；禁止解析或猜测 Markdown 正文来生成运行时语义。
- Methodology bundle、Series plan 与 lifecycle plan 的编码必须 byte-stable；集合先 canonical sort/deduplicate，digest 必须绑定 schema、完整语义字段和 content-addressed artifact metadata。

## 七、序列化与兼容

- wire/project schema 必须声明 `schemaVersion` 或 protocol version；新增可选字段需有向后兼容默认，删除/改义字段需要 major migration。
- enum wire value 必须显式定义并测试；禁止依赖 Rust variant debug/display 自动形成协议。
- 对未知必需 enum/operation 必须 fail closed；对 negotiated optional field 可忽略但必须保留 capability 语义。
- 禁止把内部 error debug、SQLite row、absolute path、claim/token 或完整 stdout 直接 serialize 给 client。
- frame、字符串、数组、map、嵌套深度和 ChildResult/ContextProjection 都必须在 decode 前后校验预算。
- 跨 Part frozen DTO/port 的字段、枚举或错误码不得由单个 Part 原地改义；必须由合同 owner 提升 schema/version，并同时更新 fixture、golden digest、迁移和所有消费者。
- content digest 只能表述“内容已绑定/可校验”；没有私钥签名、可信公钥和 trust-anchor 验证链时禁止在 API、日志或文档中称其为 signed artifact。

## 八、日志与 tracing

- 使用 `tracing` event/span，字段名固定且结构化；禁止拼接 JSON 字符串日志。
- request span 至少包含 `request_id`, `workspace_id`, `session_id`, `turn_id`, `method`, `outcome`, `elapsed_ms`；mutation/host/compact 按需增加 work item、revision、event、delegation/action/generation。
- error 日志保留 source chain，warn 表示可恢复偏差，info 只记录生命周期/commit 边界，debug/trace 不得输出敏感正文。
- 循环或高频 Hook 内禁止逐项 info；使用聚合 metric、sampling 或 trace level。
- 日志脱敏和审计字段以 `security.md` 为准。

## 八之二、daemon 诊断轨

上一节管 `tracing` 生命周期日志。诊断轨是**另一条独立通路**，权威定义在
`ae-sdd-contracts::diagnostics`，写入在 `ae-sdd-runtime::diagnostics`，读取在
`ae-sdd runtime trace`。

- 诊断轨不经 `tracing`，改用 typed record + serde 序列化为 JSONL。这不是违反上一节
  的「禁止拼接 JSON 字符串日志」：字段由 struct/enum 在编译期固定，没有手写拼接。
  用 struct 而非 `tracing` 宏是为了让字段清单成为编译期契约，宏漏字段编译器不管。
- 轨道划分固定为两条：`trace`（Hook 调用与 daemon 答复）与 `ops`（任务节点变更与
  缺陷）。分轨的唯一理由是防挤占——`trace` 占绝大多数字节，合并会让一阵 Hook 流量
  把低频的缺陷记录挤出保留窗口。禁止把两轨合并为一个文件。
- **`trace` 轨逐条记录每次 Hook 调用，是上一节「高频 Hook 内禁止逐项 info」的显式
  例外。** 该禁令针对 `tracing` 的 info 级噪音；诊断轨不进 info、不进 `daemon.log`，
  分轨已经达到禁令要保护的目的（低频关键信息不被高频流量淹没）。禁止以该禁令为由把
  `trace` 轨改成采样或聚合：逐条是需求，缺一条即留痕断裂。
- 每次 Hook 调用写 `hook.in` 与 `hook.out` 两条，共用 `hook_event_id` 配对。禁止合并
  为一条：`hook.in` 无配对是 daemon 未返回（崩溃或线程丢失）的唯一廉价证据，合并后
  该信号消失。`hook.in` 必须在做任何工作之前写入。
- `turn_id` 是跨四类记录的主关联轴，凡能取到必须携带。诊断轨不要求 `request_id`：
  Hook 侧的配对键是 `hook_event_id`，节点侧是 `idempotency_key`/`revision`。
- 缺陷轨只收 panic、invariant、worker 异常、store/migration 故障、encode 故障。
  **策略拒绝（role denial、blocker gate 失败、lease 冲突）不是缺陷**，禁止写入缺陷
  轨——它们是系统按设计工作，混入会把真缺陷埋掉；它们通过其他记录的 `ok: false` 可查。
- 同一缺陷按 `(kind, code_site, 归一化 message)` 指纹去重：首次写完整记录，其后只累加
  计数。指纹表必须有上限，超限即落盘并重置——归一化只折叠数字串，带变化路径或 ID 的
  消息会持续产生新指纹，无上限即内存泄漏。
- 保留只用字节表达，禁止按时间过期，禁止后台定时清理任务。写满即轮转、固定保留段数、
  挤掉最旧段。诊断轨是优化用的一次性诊断物，不是合规留存物；`security.md` 要求的不可变
  evidence 由 receipt 与项目文件承载，不由诊断轨承载。当前上限：`trace` 4 MiB × 4 段、
  `ops` 2 MiB × 3 段（含 live 段），合计约 22 MiB。
- 轮转必须先关闭文件句柄再 rename。Windows 不允许 rename 仍被持有的文件，顺序写反的
  表现是轮转静默失效、段文件无界增长，与原因毫无相似之处。
- 诊断轨写入路径不得让 daemon 失败：writer 线程死亡、磁盘写满或队列饱和只能退化为丢行。
  `ops` 轨阻塞等待而不丢；`trace` 轨可丢但必须累计并落盘丢弃计数，禁止静默丢弃。
- 禁止把 context/projection 正文、prompt、transcript、tool 输出正文、token 或 secret
  写入诊断轨；只允许有界标识、稳定 code 与 digest。脱敏边界同样以 `security.md` 为准。

## 九、进程、文件与 SQLite

- 外部进程使用 program + args，不经过 shell；保留 exit code、有限 stdout/stderr digest、deadline 和 cancellation evidence。
- 文件 mutation 只能经 store/artifact adapter；禁止业务模块直接 `std::fs::write` 覆盖项目权威文件。
- SQL 必须是静态或受控 builder 生成并绑定参数，禁止字符串插值；数据库调用只经 repository adapter。
- path 比较必须使用 canonical/normalized path 类型，不得用字符串 `starts_with` 作为安全 containment。

## 十、`unsafe` 与平台代码

- 非 platform integration crate 必须 `#![forbid(unsafe_code)]`。
- platform adapter 只有在 safe API 无法满足 Named Pipe/DACL/atomic semantics 时可使用 `unsafe`；unsafe block 必须最小化并附 `// SAFETY:` 前置条件、所有权、生命周期和线程安全证明。
- 新增或扩大 unsafe 必须有 target-platform test、Miri/等价可行检查和独立 review；未验证平台不得宣称 capability supported。

## 十一、禁止事项

- 禁止复制 TransitionPolicy、Gate truth table、role grants 或 stable error mapping。
- 禁止用字符串状态、magic number 或 magic path 代替 enum/constant/config。
- 禁止为消除编译错误使用宽泛 `clone()`、`Arc<Mutex<_>>` 或 `Box<dyn Any>`；必须说明 ownership boundary。
- 禁止在 production code 留 stub、always-pass Gate、空 adapter 或“暂时返回成功”。
- 禁止把 Monitor 或 generated dist 当 Rust library 调用。
