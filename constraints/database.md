# SQLite 与持久化规范

## 摘要

本文件定义 ae-sdd daemon 的 SQLite schema、migration、事务、索引和权威边界。业务状态存于**项目级** SQLite，随项目可移植；不迁移到远端或跨项目共享数据库。
适用场景：新增/修改 runtime 表、项目状态表、repository、journal、cache、migration 或恢复逻辑。

---

## 零、数据权威边界

- WorkItem 状态（项目级 `state.db`）、lease、正式文档、memory、evidence、review 和 artifact 是项目内可移植业务真相。
- **用户级** SQLite 只保存 workspace/session/turn/delegation/host-action/context/compact/supervisor/job/event/cache/receipt index 等 daemon 运行元数据，以及「workspace → 项目库路径」的跨 workspace 索引。
- phase、executionPlan、plan approval、test evidence 或 review completion 禁止只存在**用户级** SQLite；它们属项目级业务真相。
- 每个 Work Item 的 authoritative mutation journal 固定在其 state directory 的 `mutation-journal/v1/<revision-after>-<mutation-id>.json`；它与项目级 `state.db`、lease 和 artifact 同属 project truth。
- **恢复方向按载体区分**：用户级 SQLite 丢失时必须能从 project files、COMMITTED journal/event payload 和宿主重新握手重建安全状态；项目级 `state.db` 损坏时从最近全量快照 + 其后 COMMITTED journal 重放重建（见 `DR-AE-SDD-DAEMON-AUTHORITY-001` 决策 4），不可静默重建空库。两种情况下，无法证明的 running job/delegation/compact 一律不得恢复为成功。
- 项目级权威与用户级索引冲突时以项目 revision/hash 为准；hash 改变但 revision 未增长必须进入 `EXTERNAL_STATE_CONFLICT`，禁止自动覆盖。
- SQLite 初始化时生成 immutable `event_store_id`；daemon restart 保持，DB 重建时轮换。外部 cursor 必须绑定 `event_store_id + event_seq`，禁止在新 DB 上误认旧序列。

## 一、技术与连接配置

| 项目 | 约束 |
| --- | --- |
| 引擎 | SQLite 3，默认通过 `rusqlite` bundled feature |
| journal | `PRAGMA journal_mode=WAL` |
| durability | `PRAGMA synchronous=FULL`；不得为性能测试改为 OFF |
| integrity | `PRAGMA foreign_keys=ON`；启动和恢复后执行 schema/integrity 检查 |
| busy | 有界 `busy_timeout`，默认不超过 1000 ms；request deadline 始终优先 |
| connection | runtime-owned 单 writer queue + 有界 read connections；禁止每请求新建连接 |
| async boundary | 所有 SQLite 调用在有界 blocking executor/DB worker 上执行，禁止阻塞 Tokio core thread |

- 数据库位于当前 OS 用户 runtime dir，目录/文件权限遵循 `security.md`。
- 禁止加载 SQLite extension、attach 任意外部数据库或使用共享网络盘作为默认 runtime DB。
- 一个 protocol major 使用独立 schema/database identity；升级必须先 drain。

## 二、表与 owner

首版 runtime schema 至少覆盖：

| 表 | Owner | 业务含义 |
| --- | --- | --- |
| `workspace` | runtime | canonical root、project identity、mode、inventory generation |
| `agent_session` | runtime | Agent physical/logical session、trusted role/lineage、heartbeat |
| `turn` | runtime | prompt/PreTool/PostTool/Stop lifecycle |
| `delegation` | delegation | root/parent/child、assignment、deadline、status、receipt digest |
| `delegation_request_receipt` | delegation | parent-scoped create idempotency key/request digest/delegation/response digest |
| `delegation_grant` | delegation | operation/path/deliverable capability 子集 |
| `child_result` | delegation/artifacts | bounded canonical result digest 与 validation status |
| `memory_cleanup_receipt` | delegation/context | validated result 后的 namespace snapshot/cleanup proof |
| `host_adapter_instance` | host | authenticated adapter capability/sequence/health |
| `host_action` | host | spawn/send/wait/cancel/compact command 与相关 ACK |
| `context_projection` | context | role-aware projection revision/digest/budget/cache |
| `context_pressure_sample` | context/host | authenticated token usage sample、generation/sampleSeq 与 bounded retention |
| `compact_cycle` | context | snapshot、generation、host action、ACK、rehydrate lifecycle |
| `supervisor_checkpoint` | flow | per-workspace/WorkItem global event cursor/decision digest/health |
| `operation_receipt` | store/operations | idempotency payload hash、revision before/after、result digest |
| `hook_event_receipt` | runtime/policy | session + hookEventId request digest 与原 decision/event cursor |
| `gate_job` | gates/runtime | GateKey snapshot、状态、outcome/evidence digest |
| `runtime_event` | runtime | 跨 restart 全局单调 eventSeq、bootId provenance 与可重放的 versioned bounded committed payload/payloadRef |
| `inventory_entry` | inventory | workspace-relative path、metadata、content hash、generation |

禁止创建“万能 metadata”表承载未经 schema 的业务 JSON。确需 versioned payload 时，必须同时保留 schema_version、payload_digest、byte_len 和 owner，并在 decode 时按 owner/event_type + schema_version 选择 typed decoder。`runtime_event` 必须保存 canonical bounded `payload_json` 或 immutable `payload_ref` 二者之一，不能只有 digest；digest 只用于完整性验证，不能替代 replay 输入。

## 三、命名与字段类型

- 表名和字段名使用小写 `snake_case`；表名使用单数，如 `agent_session`。
- UUID/领域 ID 持久化为 canonical lowercase text，必须在 repository 边界解析为 newtype，禁止在 domain 中传裸字符串。
- sequence/revision/fencing/generation/size/duration 使用非负 `INTEGER`，读取时检查溢出和负值。
- bool 使用 `INTEGER NOT NULL CHECK (field IN (0,1))`。
- enum 使用稳定 lowercase snake_case/明确 wire mapping，并加 CHECK 或 repository validation；禁止存 Rust `Debug` 输出。
- timestamp 使用 UTC RFC 3339 text；duration/deadline budget 使用整数毫秒。禁止混用本地时区。
- digest 使用固定 64 字符小写 hex，写入前后验证；secret/token/claim 不得落库，必要时只存 SHA-256/HMAC digest。
- nullable 字段必须有真实“尚不存在/不适用”语义；禁止以空字符串、0 或 nil UUID 代替 NULL。

每张表必须有主键、owner 说明、created/updated 语义和状态机约束；不强制复制业务系统的 created_by/updated_by 模板。

## 四、索引与约束

必须建立并测试以下关键约束：

| 表 | 索引/约束 | 目的 |
| --- | --- | --- |
| `workspace` | UNIQUE(canonical_root, project_key) | 路径 alias 收敛 |
| `agent_session` | INDEX(status, heartbeat_at) + partial UNIQUE(workspace_id, external_key_hash) for opening/active | expiry/recovery + session.open replay |
| `turn` | UNIQUE(session_id, turn_seq) | turn 单调 |
| `delegation` | INDEX(parent_delegation_id, status, deadline) | child join/timeout/orphan |
| `delegation_request_receipt` | PRIMARY KEY(workspace_id, parent_session_id, idempotency_key) | response-loss 不重复物理 spawn |
| `memory_cleanup_receipt` | PRIMARY KEY(delegation_id) | completed 前置证明 |
| `host_action` | UNIQUE(adapter_id, command_seq) + UNIQUE(adapter_id, ack_id) | command 与 duplicate/out-of-order ACK 去重 |
| `context_projection` | UNIQUE(session_id, context_revision) | Hook delta/no-change lookup |
| `context_pressure_sample` | UNIQUE(adapter_id, session_id, sample_seq) + INDEX(session_id, context_generation, sample_seq) | hysteresis/cooldown replay |
| `supervisor_checkpoint` | PRIMARY KEY(workspace_id, work_item_id) + last_event_seq/last_event_digest | durable replay cursor |
| `operation_receipt` | PRIMARY KEY(workspace_id, idempotency_key) | retry safety |
| `hook_event_receipt` | PRIMARY KEY(session_id, hook_event_id) | duplicate Hook 返回原 decision |
| `gate_job` | INDEX(gate_key, status) | singleflight lookup |
| `runtime_event` | INTEGER PRIMARY KEY event_seq（全局单调、不复用）+ INDEX(workspace_id, event_seq) + INDEX(workspace_id, work_item_id, event_seq) | 跨 restart replay 与 ordered subscription |
| `inventory_entry` | PRIMARY KEY(workspace_id, path) | selector/fingerprint |

- foreign key 必须显式声明；删除 owner 行时采用明确 lifecycle cleanup，禁止依赖级联删除隐藏审计数据。
- 新索引必须由实际 query/EXPLAIN 或 pressure evidence支撑；禁止为了“可能查询”无限加索引。
- 查询必须列出字段，禁止 `SELECT *`；分页/stream 必须有稳定 order 与硬 limit。
- supervisor checkpoint 只能在 decision/side effect checkpoint 同一事务提交后推进；retention 不得删除任何 active checkpoint 之后仍需 replay 的 event 或其 payloadRef。

## 五、SQL 与 repository

- 所有值必须使用参数绑定；禁止 `format!`/字符串拼接构造 SQL value、identifier 或条件。
- 动态 identifier 必须来自 compile-time allowlist；禁止由 RPC/plugin 输入直接形成表名、列名或 ORDER BY。
- repository 只负责 row mapping、transaction 和数据存取，不实现 FlowRuntime、role、Gate 或业务 transition。
- SQL 必须集中在 owning repository/migration，禁止散落在 actor、CLI、Hook 或 domain。
- 禁止 trigger、stored business logic、user-defined extension 和 ORM 自动 schema mutation；状态变更由 Rust application service 显式完成。
- batch 写使用显式 transaction 和数量上限；禁止循环逐行 autocommit。

## 六、事务与 project file 提交

- 单 SQLite transaction 只能覆盖 daemon metadata；它不能假装与 project filesystem 构成原子分布式事务。
- project mutation journal entry 是 versioned typed document，至少包含 `schemaVersion, mutationId, workspaceId, workItemId, operation, idempotencyKeyDigest, canonicalPayloadDigest, revisionBefore, revisionAfter, fencingToken, targetFiles[{path,beforeDigest,afterDigest,stagedRef}], event{type,schemaVersion,payload/payloadRef,digest,byteLen}, status, preparedAt, committedAt`；secret 与绝对越界路径不得进入。
- Journal v1 remains readable with legacy untagged write items in `targetFiles`. Journal v2 retains the same top-level `targetFiles` field but tags each item: `write` carries project-relative path, before/after digests, byte length, and staged ref; `delete` carries project-relative path and the expected pre-delete digest. This wire-only v2 change consumes no SQLite migration number; migration `0016` remains unused.
- A PREPARED v2 delete completes only when the current digest matches its expectation, is already complete when the path is absent, and enters `EXTERNAL_STATE_CONFLICT` on digest drift. Durable removal must fsync the parent directory. A destination write and source-draft delete share one PREPARED/COMMITTED journal and no receipt is visible before COMMITTED.
- 提交顺序固定为：在跨进程锁内原子写/fsync `PREPARED` journal → 写并 fsync 同目录 staged targets → 逐项 atomic replace + directory fsync → 原子将 journal 替换为 `COMMITTED`（携带 receipt/event）并 fsync journal directory → 才向 SQLite 插入可重建 receipt/event index、推进 supervisor cursor并对外应答。任何阶段崩溃都按 journal target digest 完成或回滚到可证明状态，不得猜 committed。
- `ABORTED` 只能在证明 project targets 均未提交或已安全回滚后写入；部分 target 无法证明时进入 `EXTERNAL_STATE_CONFLICT`。COMMITTED journal 在其 receipt/event/artifact 仍被 state/evidence/checkpoint 引用时不得清理。
- event 只能在 project commit 成功后可见；SQLite rollback、disk full、fsync error 或 process crash 不得产生 fake committed/PASS。
- transaction 内禁止 await host/Gate/process/file scan；先取得 immutable snapshot，在 I/O 返回后重新验证 freshness，再进入短 transaction/commit 临界区。
- idempotency receipt 必须保存 canonical payload hash 和 revision before/after；同 key 不同 hash 必须拒绝。
- `delegation.create`、`session.open`、Hook event、host action/ACK 与 ChildResult replay 必须有数据库唯一键和原 response/result digest；进程内 map 不能作为幂等证明。

## 七、migration

- migration 文件放根 `migrations/`，使用单调编号和不可变 checksum，例如 `0001_runtime_base.sql`。
- 已发布 migration 禁止原地修改；修复必须追加新 migration。
- 每次启动在获取 daemon 单例锁后、开放 endpoint 前执行 migration；migration 必须在 transaction 中更新 schema version/checksum。
- schema version 使用明确 migration table 或 `PRAGMA user_version`，不得从“表是否存在”猜版本。
- migration 必须覆盖 empty DB、上一发布版本、重复执行、途中 crash、损坏/不兼容版本；失败时 daemon 不进入 serving。
- rollback 通过上一版完整 binary + 兼容 reader/备份执行，不依赖不安全的自动 down migration。

## 八、保留、清理与隐私

- runtime event、job、projection/cache 可按 versioned retention policy 清理；operation receipt、host/delegation/compact audit 在其引用有效期内不得提前删除。
- Review session 的 `parent_review_id` 必须原样保存为业务 lineage 锚点，但不得外键依赖父 Review 投影行；父事件可能已按保留策略清理，或尚未在本次 daemon 世代重放。
- cleanup 本身必须是有界 job、可取消、可审计，不得在 Hook request 中执行 VACUUM/大范围删除。
- 本节只管 SQLite。daemon 诊断轨（Hook 留痕、节点变更、缺陷）是 state directory 下的
  JSONL 文件，不入 SQLite，保留按字节轮转且无定时清理任务，规则见 `code-style.md` §八之二。
- 禁止在 SQLite 存储 prompt、child transcript、源码、完整系列文档、endpoint token、claim token、credential 或无界 stdout/stderr。
- artifact 正文应落在项目允许路径并以 hash ref 关联；root projection 只存有界摘要/引用。

## 九、验证清单

- [ ] WAL/foreign_keys/synchronous/busy 配置已断言。
- [ ] schema/migration checksum、empty/upgrade/repeat/crash 测试通过。
- [ ] unique/FK/CHECK/index 与 query plan 已覆盖正反例。
- [ ] 单 writer + bounded reader 在 100 sessions/10 workspaces 下无丢 mutation、无 event 重排。
- [ ] DB 删除/损坏后不会把未证明状态恢复为 PASS/completed。
- [ ] project hash/revision conflict 能稳定进入 `EXTERNAL_STATE_CONFLICT`。
- [ ] release DB/日志扫描不含 secret、prompt、transcript 或源码正文。
