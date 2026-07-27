# 分层架构规范

## 摘要

本文件定义 ae-sdd Rust 本地控制平面的分层职责、依赖方向、Agent/daemon 边界和 mutation 链路。
适用场景：设计跨 crate 调用、FlowRuntime、Gate、Hook、HostRuntimeAdapter 或持久化实现。

---

## 一、整体架构

```text
Root / Series / Task / Reviewer Agent sessions
                    |
            thin ae-sdd CLI / Hook
                    |
       Named Pipe / Unix Domain Socket
                    |
        ae-sddd RPC + Runtime actors
                    |
     FlowRuntime + FlowSupervisor (control)
          |          |          |
  Operations      Delegation   Context/Compact
          |          |          |
   Policy/Gates   Host adapter  Artifact refs
          \          |          /
          Store + project files + SQLite metadata
```

daemon 是 Agent 之外的控制平面，不是第四层 Agent。Agent 执行语义工作，FlowRuntime 执行确定性流程规则，HostRuntimeAdapter 只桥接宿主原生生命周期。

## 二、分层职责

### 1. Protocol / Domain

**负责：** versioned DTO、稳定枚举、state/value object、不变量和纯状态转换输入。

**禁止：** 文件、SQLite、Tokio、clock/random、进程、宿主 API、日志 side effect。

### 2. Policy / Flow

**负责：** phase→required Gate、role grants、nextAction、correction、pause/resume、transition 和 event replay 的唯一确定性实现。

**规则：**
- reducer 只依赖显式 `state revision + ordered events + policy digest + input fingerprint`。
- 相同输入必须产生相同 decision digest。
- 只有 fresh `PASS` 可放行；只有 fresh `FAIL` 增加业务 correction。
- `ERROR/TIMEOUT/CANCELLED/STALE` 保持独立语义；background infra error 只改变 supervisor health。

**禁止：** 读取 prompt 猜流程、直接执行 Gate I/O、写 state、调用宿主、以重复 prompt 驱动 correction。

### 3. Application services

`ae-sdd-operations`、`ae-sdd-delegation`、`ae-sdd-context` 负责用例编排：验证 command、调用 policy/port、组织事务边界、产生 event/outbox。

**禁止：** 复制 domain invariant、绕过 store precondition、在 application service 中硬编码平台分支。

### 4. Adapters

Store、artifact、Gate、scanner、inventory、SQLite、filesystem、git、toolchain、HostRuntimeAdapter 和 OS service 都是 adapter。

**负责：** 技术 I/O、协议转换、deadline/cancellation、typed error 和 evidence。

**禁止：** 决定 phase/nextAction、把技术异常映射为 PASS、接受 client 自报 role/lineage、把 watcher/ACK 当 committed truth。

### 5. Runtime / Binaries

- Runtime 建立 WorkspaceActor、SessionActor、WorkItemActor、有界 scheduler、durable event/outbox 与公平配额。
- `ae-sddd` 只做 lifecycle、RPC server 和 dependency composition。
- `ae-sdd` 只做 CLI/Hook envelope 适配与输出。

Hook fast path 只能鉴权、路由、读取预计算 projection 或入队事件；禁止同步执行 recursive scan、build、spawn wait 或 compact wait。

## 三、三层 Agent 与 capability

| 角色 | 可以 | 禁止 |
| --- | --- | --- |
| root | route、创建 series delegation、collect bounded result、汇报、申请全局 transition | 读取/生成系列交付物全文、扩大 child grant、替 child 完成语义工作 |
| series | 拥有一个 RA/DR/Story/TestCase/Coding 系列，创建 task/reviewer，提交 bounded result | 批准 executionPlan、推进全局 phase、break lease、访问 sibling series |
| task | 修改/测试 assignment paths，提交 evidence/result | 再委派、全局编排、超出 allowed operations/paths |
| reviewer | 在独立 session 读取授权 diff/evidence，提交 findings/status | 修改实现、与被审 worker 共用 session、再委派 |

- role、lineage 与 grant 由 daemon 派生；RPC payload 中对应字段只能作为期望值核对，不能授予权限。
- Delegation 只有 host action ACK + trusted child claim/attestation 后才能 running。
- parent 只能在所有 required child 到合法终态、artifact hash 验证和 memory cleanup receipt 成功后完成。

## 四、mutation 与 Gate 链路

```text
request
  -> endpoint/session/turn/role validation
  -> WorkItemActor bounded mailbox
  -> load locked state + lease/CAS/fencing/idempotency
  -> immutable GateSnapshot
  -> scheduler evaluates Gate
  -> reload + freshness revalidation
  -> fresh PASS only
  -> PREPARED journal
  -> atomic project mutation + fsync
  -> COMMITTED receipt
  -> durable event/outbox
  -> FlowSupervisor + ContextProjection refresh
```

- actor 串行不能替代跨进程锁、lease、CAS、fencing 和 idempotency。
- 长 Gate 必须离开 actor mailbox 执行；返回后必须重新验证 revision、fencing、policy、inventory 和完整 input fingerprint。
- event 只能在 COMMITTED 后发布；subscriber、Hook 或 watcher 不得看到“预提交成功”。

## 五、数据权威与恢复

- project state（项目级 `state.db`）/artifact 与 state directory 下的 versioned mutation journal 是可移植业务权威；**用户级** SQLite 只是 runtime metadata、cursor、cache、event/receipt 的可重建索引，不是 authoritative mutation journal。
- 用户级索引与 project 不一致时以 project revision/hash 为准重建 metadata；hash 改变但 revision 未增长时进入 `EXTERNAL_STATE_CONFLICT`，禁止自动覆盖。
- daemon restart 后 running Gate 不得恢复为 PASS；未证明的 host action、delegation 和 compact cycle 必须恢复为可诊断中断状态。
- watcher overflow 或 event cursor gap 必须 full reconcile/snapshot，不得猜增量。

## 六、依赖倒置与测试替身

- 需要 filesystem、clock、random、process、host 或 store 的 inward service 必须依赖最小 port trait。
- production adapter 在 binary composition root 注入；test fake 必须保持真实错误/timeout/cancellation contract。
- AC 验收不得用 mock 替代内部 FlowRuntime、StateStore、Gate freshness 或 project atomic write 链路。
- external host/OS capability 可使用 deterministic fake 做故障矩阵，但宣称支持的平台/宿主必须有 live smoke evidence。

## 七、架构红线

- 禁止 CLI、Hook、HostRuntimeAdapter、scanner 或 store 自行决定合法下一步。
- 禁止 domain/policy 依赖 adapter concrete type。
- 禁止将完整 child output、transcript、源码或系列文档注入 root context。
- 禁止把 host request/ACK、compact trigger、watcher event 或 process exit=0 单独当作业务完成。
- 禁止无界 channel、无界 task spawn、无 deadline I/O 或在 Tokio core thread 执行 blocking 操作。
- 禁止 shadow 阶段双写；workspace writer mode 必须经 drain 原子切换。
