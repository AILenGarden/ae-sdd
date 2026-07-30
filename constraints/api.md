# 本地 RPC API 规范

## 摘要

本文件定义 `ae-sdd` client/Hook/admin 与用户级 `ae-sddd` 之间的本地 framed JSON-RPC 2.0 契约。核心 runtime 不提供 HTTP REST API。
适用场景：新增或修改 RPC method、CLI command、Hook envelope、event stream、错误码和 protocol version。

---

## 一、transport 与 framing

- Windows 使用 Named Pipe，macOS/Linux 使用 Unix Domain Socket；默认禁止 TCP/HTTP listener。
- 每个 frame 为 `4-byte big-endian unsigned length + UTF-8 JSON`；默认最大 16 MiB，0 长度、超限、非法 UTF-8 或非法 JSON 必须在解析业务 payload 前拒绝。
- 每条连接的第一个 method 必须是 `runtime.handshake`；握手前的其他 method 返回 `HANDSHAKE_REQUIRED` 并关闭连接。
- 一条连接可以并发 request/response 与 server notification；`id` 在连接内唯一，notification 不含 `id`。
- client/server 必须实现 bounded outbound queue、deadline、取消和 backpressure；慢订阅者不得阻塞 mutation/Hook fast path。

## 二、JSON-RPC envelope

```json
{
  "jsonrpc": "2.0",
  "id": "request-uuid",
  "method": "operation.execute",
  "params": {
    "protocolVersion": "1.0",
    "workspaceId": "uuid",
    "agentId": "codex:instance",
    "sessionId": "uuid",
    "turnId": "uuid",
    "deadlineMs": 250,
    "payload": {}
  }
}
```

- method 命名为小写 `<domain>.<verb>`；多词使用下划线，禁止 URL、HTTP method 或路径语义。
- `runtime.handshake` 之前只在 handshake payload 发送 `protocolRange`；协商完成后的所有 request 必须发送精确 `protocolVersion`（例如 `1.0`），禁止发送 `1.x` range。
- request ID、session/turn/delegation/host-action/compact ID 必须使用 daemon 认可的 typed ID；禁止以 display name 充当 identity。
- `deadlineMs` 是调用方剩余预算，daemon 必须取 client budget、server policy 和宿主 deadline 的最小值。
- `payload` 必须先按 method 的 versioned DTO 解码；未知字段策略由 protocol minor 声明，未知必需 enum/method fail closed。

成功响应：

```json
{
  "jsonrpc": "2.0",
  "id": "request-uuid",
  "result": {
    "schemaVersion": 1,
    "requestId": "request-uuid",
    "data": {}
  }
}
```

错误响应：

```json
{
  "jsonrpc": "2.0",
  "id": "request-uuid",
  "error": {
    "code": -32010,
    "message": "REVISION_CONFLICT",
    "data": {
      "stableCode": "REVISION_CONFLICT",
      "retryable": true,
      "remediation": "refresh_snapshot",
      "details": {}
    }
  }
}
```

- `message/stableCode` 使用稳定 ASCII enum；用户界面可本地化，不把本地化文本当机器分支。
- `details` 只能包含脱敏、versioned 字段；禁止输出内部 backtrace、secret、claim、absolute outside-root path 或完整 stdout/stderr。

## 三、handshake 与 capability negotiation

`runtime.handshake` 请求至少包含 `protocolRange`, `clientBuild`, `clientKind`, `endpointToken`, `expectedBootId`, `expectedPolicyDigest`；响应至少包含 `protocolVersion`, `bootId`, `eventStoreId`, `daemonBuild`, `capabilities`, `policyDigest`, `operationSchemaDigest`, `limits`。client 必须从同一次原子读取的受权限保护 manifest 取得 token 与两个 expected 值；daemon 比较当前值，client 再核对响应值。`eventStoreId` 在 runtime DB 初始化时生成，DB 重建时变化，用于拒绝旧 cursor 碰撞。

- protocol major 无交集必须返回 `PROTOCOL_VERSION_UNSUPPORTED`，禁止尝试兼容执行。
- minor feature 只能在双方 capability 都存在时调用；缺失 capability 返回 `CAPABILITY_UNSUPPORTED`，不得伪造成功。
- endpoint authentication 与 OS user binding 必须在 handshake 完成，后续 request 不信任 client 自报 user。
- bootId、endpoint token 或 policy digest 改变时，旧 connection/capability 必须失效并重新握手。
- manifest 同时发布 `capabilityKeyId/capabilityPublicKey`；daemon 每次 boot 生成 Ed25519 keypair，private key 只驻留 daemon 内存。CLI/Hook 只能用 public key 验证短期 capability，不能签发或伪造 capability。

## 四、method owner

| Domain | Methods | Owner | 关键约束 |
| --- | --- | --- | --- |
| runtime | `runtime.handshake`, `runtime.status`, `runtime.drain` | daemon runtime | lifecycle/admin 权限分离 |
| workspace | `workspace.register`, `workspace.snapshot` | WorkspaceActor | canonical root + project identity 幂等 |
| workitem | `workitem.create`, `workitem.get`, `workitem.complete` | WorkItemActor | `workitem.create` workspace-scoped：`requiresWorkItem=false, requiresIdempotency=true, writes=true`；`entryNode` 仅 PRD/DR/STORY（BUG/CONFIG 拒绝）；可省略 `workItemId`，由 daemon 铸造 `{entryNode}-{8 位小写 hex}`（如 `STORY-3f9a2c1e`）；成功后持久绑定 `session.current_work_item` 并安装 project context projection |
| session/hook | `session.open`, `session.heartbeat`, `session.close`, `hook.user_prompt`, `hook.pre_tool`, `hook.post_tool`, `hook.stop` | SessionActor/policy | hookEventId 去重；engaged fail closed |
| flow | `flow.snapshot`, `flow.next` | FlowRuntime | 只读 deterministic decision；role-aware |
| delegation | `delegation.create`, `delegation.status`, `delegation.accept`, `delegation.report`, `delegation.collect`, `delegation.cancel` | DelegationService | role/lineage/grant/physical attestation |
| host | `host.register`, `host.capabilities`, `host.action_next`, `host.action_ack`, `host.pressure_report` | HostRuntimeAdapter boundary | authenticated adapter；ACK 不等于 child claim；sample 与 generation 相关 |
| context/compact | `context.get`, `context.project`, `compact.request`, `compact.status` | ContextService/CompactManager | revision/delta/budget/pressure；ACK+rehydrate |
| operation/gate | `operation.describe`, `operation.execute`, `gate.evaluate` | Operations/WorkItemActor | typed operation；fresh PASS only |
| event/job | `events.subscribe`, `job.status`, `job.cancel` | runtime scheduler | ordered cursor；bounded subscriber |

新增 method 必须先确定唯一 owner、request/response schema、权限、幂等 key、deadline、错误、event 和验证矩阵；禁止只增加 CLI parser 分支。

## 五、写请求与并发前置条件

operation registry 必须为每个 method 冻结 `scope`（runtime/workspace/work_item/session/delegation/host）、`requiresWorkspace`, `requiresWorkItem`, `writes`, `requiresLease`, `requiresRevision`, `requiresIdempotency`, `requiresConfirmation`。字段前置条件由 registry 决定，不得由 method 名称、payload 是否出现或“所有 write”猜测。runtime/workspace/session bootstrap 不得被 Work Item 前置条件锁死。`operation.execute` 的准入前置条件从 payload 的 `operation` 解析到该 TypedOperation 的 registry `OperationSpec`（registry-resolved admission），不再套用 method 级 blanket 默认值；未注册 operation 保持原 method 级 fail-closed 行为。

| 字段 | 约束 |
| --- | --- |
| `projectKey/workItemId` | 分别在 `requiresWorkspace/requiresWorkItem=true` 时必需；精确匹配，不从唯一候选猜测 |
| `leaseId` | `requiresLease=true` 时必需；active 且 owner/session/capability 匹配 |
| `fencingToken` | `requiresLease=true` 时必需；当前单调 writer generation |
| `expectedRevision` | `requiresRevision=true` 时必需；当前 project state revision |
| `idempotencyKey` | `requiresIdempotency=true` 时必需；workspace 内业务语义唯一 |
| `confirmation` | `requiresConfirmation=true` 时必需；必须是可审计用户批准 |

`lease.acquire` 与 `lease.break` 是明确登记的 `writes=true, requiresLease=false` bootstrap/control operation，必须在跨进程锁内维护单调 fencing 与 tombstone；前者仍按 registry 要求 idempotency，后者必须验证 admin actor/reason/audit。不得先要求 active lease 再允许 acquire/break。

- 相同 idempotencyKey + canonical payload hash 返回原 receipt；同 key 不同 hash 返回 legacy-stable `IDEMPOTENCY_KEY_REUSED`。
- actor 排队前和 commit 前都必须验证 identity/role；Gate 返回后必须重新验证 revision/fencing/policy/inventory/input fingerprint。
- response 只有在 project atomic write 和 COMMITTED journal 成功后才可包含 `revisionAfter`, `receipt`, `eventSeq`。
- shadow mode 的 Rust method 只允许 read/compare；任何 mutation 返回稳定 mode 错误。

## 六、Gate、event 与长 job

- `GateOutcome` wire enum 固定为 `PASS | FAIL | ERROR | TIMEOUT | CANCELLED | STALE`。
- `FAIL` 必须携带 findings；`ERROR` 携带 stable code/retryable；`TIMEOUT` 携带 deadline；`CANCELLED` 携带 reason；`STALE` 携带 changed dimensions。
- 只有 fresh PASS 可放行，只有 fresh FAIL 可增加业务 correction；其他状态不可折叠。
- 长 Gate/build/install/distribute 返回 `jobId`；Hook fast path 禁止同步等待长 job。
- `eventSeq` 是同一 `eventStoreId` 内跨 daemon restart 全局单调且不复用的序列；`bootId` 仅标识生产该事件的 daemon boot。cursor 为 `eventStoreId + eventSeq`，DB 重建或 gap 返回 `EVENT_CURSOR_GAP` 并要求 full snapshot。event notification 必须带 `eventStoreId + bootId + eventSeq + workspaceId + type + schemaVersion + payloadDigest`。

## 七、Delegation、ChildResult 与 context

- `delegation.create` 的 role/lineage/grants 由 daemon 从 parent capability 派生；client 只能提交 assignment intent。
- host action ACK 必须关联 `adapterId/actionId/commandSeq/requestDigest`；只有 child 使用一次性 claim 并通过 session attestation 后才能进入 running。
- ChildResult canonical payload 默认最大 64 KiB、summary 最大 8 KiB；必须包含 delegationId、outcome、summary、findings、deliverables(path/hash/kind)、evidenceRefs、requestedAction、memorySnapshotHash。
- ChildResult 禁止包含 transcript、源码全文、完整 stdout/stderr 或完整系列文档；超限内容必须成为 hash-addressed artifact ref。
- root ContextProjection 默认最大 64 KiB；`context.get` 根据 `contextRevision + digest` 返回 full/delta/no-change，role/scope 由 trusted session 派生。
- `compact.request` 只有匹配 session/generation 的宿主 ACK 和 rehydrate 完成后返回 `context-restored`；request dispatched 不等于成功。
- advisory compact 仅是流程节点建议：系列边界（Root→Series delegation collect 提交）时 `delegation.collect` 响应可携带 `compactAdvice`，`flow.next` 可返回 advisory 的 `suggest-compact` action；它不启动 compact 周期、不改变 contextGeneration，宿主可忽略。
- active compact 仍只能经 `compact.request` 或认证宿主 token-pressure 触发并由宿主执行；advisory compact 不得作为 compact 已发生或已授权的证据。
- `host.pressure_report` 只接受 authenticated adapter 为具备 `observe_context_pressure` capability 的 session 提交的单调 sampleSeq、contextGeneration、usedTokens、contextWindowTokens、source 与 observedAt。自动 compact 默认需要同 generation 连续 2 个样本达到 800 permille high watermark，低于 600 permille 才解除滞回，并有 300 秒 cooldown；阈值属于 versioned policy，可配置但必须进入 policyDigest。
- 缺少可信 token telemetry 时 daemon 只能返回 pressure unknown/manual remediation，不能用 projection bytes 伪装 token 使用率或宣称已主动 compact。

## 八、Hook 映射

- Hook adapter 只解码宿主 stdin、调用 RPC、把 `HookDecision` 映射为宿主兼容 JSON/exit code；禁止执行本地 Gate、state read/write 或 Python fallback。
- engaged PreTool 遇到 daemon unavailable、protocol mismatch、deadline、Gate ERROR/TIMEOUT 时必须 deny；engaged Stop 必须 block。
- 未 engaged 只能依据 daemon 预签且 client 可只读验证的短期 capability 返回“不受控”；禁止把无法验证解释为未 engaged。
- duplicate hookEventId 必须返回原 decision，不产生第二个 turn/event/correction。

## 九、稳定错误码最小集

| 类别 | 稳定错误码 |
| --- | --- |
| transport/auth | `DAEMON_UNAVAILABLE`, `HANDSHAKE_REQUIRED`, `PROTOCOL_VERSION_UNSUPPORTED`, `ENDPOINT_AUTH_FAILED`, `ENDPOINT_STALE` |
| workspace/state | `WORKSPACE_OUTSIDE_ALLOWED_ROOT`, `PROJECT_MISMATCH`, `EXTERNAL_STATE_CONFLICT`, `REVISION_CONFLICT` |
| writer | `LEASE_REQUIRED`, `LEASE_CONFLICT`, `STALE_FENCING_TOKEN`, `IDEMPOTENCY_KEY_REUSED` |
| Gate | `STALE_GATE_RESULT`, `GATE_ERROR`, `GATE_TIMEOUT` |
| role/host | `ROLE_OPERATION_FORBIDDEN`, `RUN_DEPTH_EXCEEDED`, `HOST_CAPABILITY_UNSUPPORTED`, `HOST_ACK_TIMEOUT`, `DELEGATION_ATTESTATION_FAILED` |
| result/context | `CHILD_RESULT_INVALID`, `CHILD_RESULT_TOO_LARGE`, `CONTEXT_REVISION_STALE`, `CONTEXT_BUDGET_EXCEEDED` |
| compact | `COMPACT_UNSUPPORTED`, `COMPACT_ACK_TIMEOUT`, `COMPACT_ACK_INVALID` |

错误码的 numeric JSON-RPC 映射由 `ae-sdd-protocol` 单点维护；CLI、Hook 和 daemon 禁止各自复制映射表。

## 十、禁止事项

- 禁止以 HTTP status、URL、REST controller 或网络端口表达核心 runtime contract。
- 禁止返回通用“success=false”而丢失 typed outcome；禁止错误时仍用 success result。
- 禁止 method 使用位置参数数组、动态字符串 map 或未版本化 `serde_json::Value` 代替 typed DTO。
- 禁止 client 通过自报 role、session lineage、WorkItem binding 或 ACK outcome 获得权限。
- 禁止 protocol minor 在未 negotiation 时改变已有字段语义。

## Bootstrap activation and session recovery

`workspace.register` always creates or resolves a `Shadow` workspace. It must
never create or promote `RustCanary`. Exact `/ae-sdd` activation uses the
existing `workspace.mode_transition` RPC with the strict Hook-only payload
`{"bootstrapActivation":true}` and command confirmation. That branch permits
only `Shadow -> RustCanary`, emits `workspace.bootstrap_activated`, and cannot
select a target mode, bypass later parity, enter sole-writer mode, or reverse a
transition. Admin migration retains its drained/parity-checked contract.

Each Host Hook event reopens `session.open` with an idempotency identity scoped
to the external session plus the Hook event. A retry of the same event replays;
a later event commits a refreshed durable expiry and boot capability while
preserving Work Item, role, delegation, grant, and physical attestation.
