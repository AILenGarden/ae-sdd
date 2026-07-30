# ae-sdd 路由可用性根因与修复方案 — Plan

> **起草日期**：2026-07-28
> **目标**：恢复 ae-sdd 强制工作流对新任务的可用性，并修掉使其失效的两条系统性根因
> **起草依据**：2026-07-28 一次微型任务（向 `source/L2-DISCIPLINE.md` 写入五条执行效率纪律）在路由阶段完全阻塞后的实查复盘
> **状态**：P0-2 / P1-1 / P2 已落地并验证；P0-1 / P1-2 经实查后规模远大于起草估计，未动手，见各节修正块
> **目标读者**：ae-sdd 维护者

---

## 0. TL;DR

一次「改一个文件里约 12 行双语 Markdown」的微型任务，把整场会话耗在路由管道上，载荷产出为零。

阻塞不是单点缺陷。两个 blocker（无法创建 Work Item、复用既有 Work Item 被 lease ledger 堵死）是同一个判断失误的两个投影：**Python→Rust 迁移把「既有 artifact 上的行为等价」当成了完成判据**。

| 项 | 性质 | 解封锁效果 |
|---|---|---|
| **RC-1** 迁移判据漏掉「创建类操作」与「读陈年 artifact」 | 根因 | — |
| **RC-2** 诊断信息在层边界被折叠成稳定错误码 | 根因 | — |
| **P0-1** 补 `workitem.create` | 解封锁 | 新任务可进入路由 |
| **P0-2** lease ledger 兼容读 + 写时归一 | 解封锁 | 25 个存量 Work Item 一次性可用 |
| **P1-1** 陈年真实 artifact 进入测试判据 | 防复发 | 唯一能防同类复发的一项 |
| **P1-2** 文档一致性测试（肯定式） | 防复发 | 挡住「删实现留文档」 |
| **P2** 错误消息带期望形状 | 降摩擦 | 本轮 5 次 schema 拒绝中约 4 次可省 |

**2026-07-28 落地状态**：P0-2、P1-1、P2 已实现并端到端验证（`lease.status` 从 `STATE_INVALID` 恢复为正常返回；25 份存量 ledger 全部可读；错误消息现在指名字段）。P0-1 与 P1-2 经实查确认规模远超起草估计，各自需要独立一轮，修正依据写在对应小节。

---

## 1. 触发场景（本会话可观测事实）

驱动到的部分，全是真调用真返回：Skill 加载契约 → daemon 启动（双 `--allowed-root`）→ `workspace.register` 得 `rust-canary`（可写）→ `session.open` 得 capability token → `operation.describe` 枚举注册表 → `workitem.get` 与 `flow.snapshot` 读到真实状态。

**路由链路本身是通的。** 卡住的是两处，第二处在推荐复用方案时未预见：

1. Rust 操作注册表只有 `workitem.get` / `workitem.complete`，**无 `workitem.create`**。唯一创建入口 `ae-sdd state new` 随 `tools/bin/ae-sdd` 在 `6ecf958` 删除，未移植。
2. 改为复用既有 Work Item 后，`lease.status` 报 `STATE_INVALID: lease ledger JSON is invalid: invalid type: string "1", expected u32`。**拿不到 lease 即无法执行任何 mutating operation**，复用路径同样不通。

编码前四上下文、`executionPlan` set/approve、G-CODEPLAN-SRC / G-14 / G-08、phase transition、evidence、review——**一个都没走到**。纪律本体的成本无从谈起；耗掉整场的全是管道。

---

## 2. 根因

### RC-1 迁移判据是「既有 artifact 上的行为等价」

被删的 `crates/ae-sdd-build/tests/migration_oracle.rs` 是 Python/Rust 语义对照测试，它**在 `6ecf958` 里与它所验证的对象一同消失**。这个判据结构上看不见两类东西：

**（一）产生 artifact 的操作。** 对照测试需要一份既有 artifact 作输入。`workitem.create` 的产物就是那份 artifact 本身，没有可对照的输入，于是它从未进入迁移清单。`crates/ae-sdd-integrations/src/business.rs:3308` 的 `read_state()` 只读不建，这不是遗漏一个函数，是判据本身照不到这一类。

**（二）读陈年 artifact 的能力。** 实查普查结果：

```
.auto-engineering/ 下 75 个目录
  Python 格式 ledger : 25
  Rust   格式 ledger : 0
  无 ledger          : 50
```

**存量全是 Python 格式，Rust 格式零个。** 而测试库里 Python 格式 ledger 的 fixture 数量同样是零——grep 命中的几处（`typed_operations_cli_e2e.rs:230`、`cli_process_contract.rs:501`）是 legacy **请求信封**，不是 ledger。

原因是结构性的：`.auto-engineering/` 被 `.gitignore:97` 排除，真实陈年 state 无法作为 fixture 提交，测试只能用 Rust 自己造的新鲜数据。于是「Rust 侧测试全绿」与「用户的 25 个 Work Item 全部不可操作」可以同时成立，且 CI 永远不会响。

**文档债同源。** 现行文档（排除只读 CHANGELOG）仍有 197 处指向已删 Python 入口，其中 6 处明确写 `ae-sdd state new`（宽松匹配 `state new` 为 9 处），包括 `source/docs/ae-sdd-implementation-architecture.md:201` 与 `source/docs/ae-sdd-design.md:158` 把它当作现行创建入口。现有测试只有否定式断言（`compatibility_routes.rs:435` 断言 hook 不含 "python"），没有肯定式的「文档提到的命令必须真实存在」。

### RC-2 诊断信息在层边界被折叠

`OperationSchemaInvalid: business payload violates its strict schema` 不说期望什么形状、也不说哪个键出错。而 `operation.describe` 其实已经把每个 operation 的完整 fields 注册表吐出来了——信息在系统里，只是没走到报错点。

这不是一次疏忽，是跨层惯例。已记录的同型：`store_error()` 把所有底层 `StoreError` 压成同一条消息，要看真因得写临时探针直接调 `SqliteRuntimeRepository::open`。

本轮代价：五次连续 schema 拒绝才凑出第一个成功的业务调用——`canonicalRoot` 应为 `projectRoot`、缺 `idempotencyKey`、缺 `workItemId`、`parameters` 应为 `payload`、`state.next_actions` 非注册方法。**没有一条能从 `--help` 看出来**，全靠读 `model.rs`、`rpc_adapter.rs` 和测试 helper 反推。

另有一处误导：workItemId 需带 entry-node 前缀（`Bug-BUG-...`），传裸 ID 时报 `ProjectMismatch: could not be resolved unambiguously`。读起来像「撞了多个」，实际是「一个都没撞上」（`business.rs:3324` 用 `matches.len() != 1` 合并了两种情形），因此白跑一轮去查重复目录。

---

## 3. 方案

### P0-2 lease ledger 兼容读 + 写时归一（**先做，用它破环**）

**落点**：`crates/ae-sdd-store/src/lease.rs`，`TryFrom<LeaseLedgerWire> for LeaseLedger`（现 325 行）与 `LeaseLedgerWire`（现 269 行）。

**仓库内已有可照搬的先例**，不需要发明模式。`crates/ae-sdd-integrations/src/operation_semantics/evidence.rs` 头部注释写明：legacy manifest 无 ledger 时保持 read-compatible，**只在下次 record 时才获得 ledger**。把这套 lazy upgrade-on-write 搬到 lease：

- `schemaVersion` 接受字符串 `"1"` 与数字 `1` 两种形态
- 缺 `lastFencingToken` 时从顶层 `fencingToken` 或 `history` 末项派生
- owner 从对象 `{agentId, sessionId, host, pid}` 映射为 Rust 的字符串 owner
- `status: "released"` → `active: None`；`history` 折成 `tombstones`
- **只在读时兼容，下次写入自然落 Rust 格式**，不做批量迁移

25 个 Work Item 一次性解封，用户无感，无停机步骤。

### P0-1 补 `workitem.create`

**落点**：`crates/ae-sdd-integrations/src/business.rs`，紧邻 `read_state()`（现 3308 行）新增 create 语义；在操作注册表登记为 `writes: true`、`requiresIdempotency: true`、`requiresLease: false`（创建时尚无 lease 可持）。

**不变量照抄已删 `state_store.py` 的既有语义**（`source/docs/ae-sdd-design.md:158` 已完整定义，不需重新设计）：目录名 `{uuid}-{R6顶层名}`、`stateMachineId` 同目录名、`stateMachineName` 存纯业务名、`stateUuid` 存 UUID、`state.json` 走 exclusive-create（已存在不覆盖）、失败不留半状态。

> **2026-07-28 实查修正，落点比起草时描述的更深。** 起草时写作「紧邻 `read_state()` 新增 create 语义」，实际不止于此：
>
> - `crates/ae-sdd-operations/src/registry.rs:160` 的 `spec()` 对**所有** 23 个操作硬编码 `requires_work_item: true`、`scope: OperationScope::WorkItem`。`OperationScope::Workspace` 在操作注册表里从未被使用过，`workitem.create` 会是第一个 workspace-scoped typed operation。
> - `crates/ae-sdd-integrations/src/business.rs:1801` 的 `ProjectBackend::open()` 无条件执行 `read_state(workspace, require_work_item(params)?)`，而 `business.rs:692` 的 `operation.execute` 分派必经该构造。因此创建类操作在抵达任何 handler 之前就会失败，需要一条 state 之前的新路径。
> - 连带项：`OPERATION_COUNT` 23→24、注册表 digest、`tests/fixtures/compatibility/legacy-surface.v1.json`（`audit.rs:127` 要求与 `OperationName::ALL` 精确双向匹配；`disposition: native-addition` 已有先例，`source` 可指 Rust 路径）。
>
> 仍建议做，但它是一轮独立的实现 + 测试，不是本轮的收尾动作。

同步更新 `source/docs/ae-sdd-implementation-architecture.md:201` 与 `ae-sdd-design.md:158`，把 `ae-sdd state new` 换成新入口。

### P1-1 陈年真实 artifact 进入测试判据（唯一防复发项）

把那 25 份 Python 期 artifact 脱敏后放进 `tests/fixtures/aged-state/`（现成样本已在手），让每个 typed operation 至少有一个用例跑在「Python 留下的东西」上。

**判据从「Rust 能读 Rust 造的东西」改成「Rust 能读 Python 留下的东西」。** 这一改，本轮两个 blocker 在 CI 里都会响。这是四项里唯一真正防复发的。

### P1-2 文档一致性测试（肯定式）

扫现行文档（排除 `source/CHANGELOG/`）中所有 `ae-sdd <subcommand>` 引用，断言每个都能在 RPC 方法表或 CLI 子命令表里找到。

> **2026-07-28 实查修正，本项规模远大于起草时的估计。** 现行文档出现 **61 个不同的 `ae-sdd <subcommand>` 形态**，而 Rust CLI 只有 4 个真实子命令（`runtime` / `rpc` / `hook` / `resume-approved-plan`）。高频项如 `ae-sdd doc`（291 次）、`ae-sdd assets`（239 次）、`ae-sdd state`（56 次）全部来自 Python CLI。按原设想直接落一条严格断言，会在约 900 行上失败——那是一轮文档迁移，不是一条测试。
>
> 拆成两步更可执行：先落测试但以「已知例外清单」开口（只锁住不再新增），再按文档逐个迁移并缩短清单。清单归零后去掉开口。

### P2 错误消息带期望形状

- `OperationSchemaInvalid` 附上注册表已有的 fields 清单 + 实际收到的多余/缺失键名。稳定错误码不变，只在 message/detail 补诊断。
- 松开 `store_error()` 的折叠：保留稳定码，附 `reason` 诊断字段。
- `business.rs:3324` 把「零命中」与「多命中」分成两条消息。

**capability token 的 90 秒 TTL 不动**——对 host 集成的 hook 流程是合理的，只对手工 CLI 难用。正解是 CLI 侧透明续期，不是调长 TTL。

---

## 4. 引导环及其破法

P0-1 与 P0-2 都是代码改动，按强制工作流都需要一个 Work Item；而「开不出 Work Item」正是 P0-1 要修的东西。已实查：25 个 ledger 全是 Python 格式，**没有一个干净的 Work Item 可借**。环是真的。

**破法：P0-2 作为一次性豁免的引导补丁。**

它自我消解的特征很干净：改动只落 `lease.rs` 一处兼容读，不新增语义、不改写任何存量文件；一旦落地，25 个 Work Item 全部可用，P0-1 及其后所有工作都能走正规路由。豁免范围仅此一个补丁，且它修完之后豁免的前提就不存在了。

需要用户显式授权这一次豁免。这是本 Plan 唯一需要豁免的地方。

---

## 5. 建议顺序

```text
P0-2（豁免引导，解封 25 个 Work Item）
  → P0-1（走正规路由，补 workitem.create）
  → P1-1（陈年 fixture 进判据）
  → P1-2（文档一致性测试）
  → P2（错误消息）
```

P1-1 排在 P1-2 之前：前者能防两个 blocker 复发，后者只防文档漂移。

---

## 6. 不做的事

- 不动 `source/CHANGELOG/` 的 409 处历史引用（L2 第 11 条：历史只读）
- 不做 ledger 批量迁移。写时归一足够，批量迁移会动 25 份用户状态且不可逆
- 不调 capability token TTL
- 不重设计协议导向的 CLI。任务导向的便利层是否要补，是独立议题；本 Plan 只恢复可用性
- 不在本 Plan 内实施任何一项

---

## 7. 自评估说明

**可观测事实**（本会话实查，含命令与行号）：ledger 普查 25/0/50、测试库 Python-ledger fixture 为零、`migration_oracle.rs` 删除于 `6ecf958`、文档债 197/19 处、五次 schema 拒绝的具体形态、注册表无 `workitem.create`。

**推断**：RC-1 把两个 blocker 归因于同一判据失误，是对因果的推断而非观测。反证方式：若在陈年 fixture 判据下这两个 blocker 仍不会响，则归因错误。

**未查**：`.auto-engineering/` 那 50 个无 ledger 目录是否另有兼容问题（本轮未触及）；Python 期 `state.json` 本身是否也存在 Rust 读不了的字段形态——只验证了 ledger，`workitem.get` 在两个 Work Item 上读成功不足以覆盖全部 25 个。



