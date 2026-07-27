# 工程结构规范

## 摘要

本文件定义 ae-sdd Rust Cargo workspace、crate owner、目录和测试/生成物放置规则。
适用场景：创建 crate、移动模块、拆分职责、评审依赖或添加平台实现。

---

## 一、根目录结构

```text
ae-sdd/
├── Cargo.toml                    # workspace members、统一 lint/profile/dependencies
├── Cargo.lock                    # release/CI 必须提交
├── rust-toolchain.toml           # 精确 stable toolchain
├── deny.toml                     # advisory/license/source/bans policy
├── crates/
│   ├── ae-sdd-protocol/
│   ├── ae-sdd-domain/
│   ├── ae-sdd-contracts/
│   ├── ae-sdd-policy/
│   ├── ae-sdd-methodology/
│   ├── ae-sdd-lifecycle/
│   ├── ae-sdd-store/
│   ├── ae-sdd-artifacts/
│   ├── ae-sdd-operations/
│   ├── ae-sdd-gates/
│   ├── ae-sdd-scanners/
│   ├── ae-sdd-inventory/
│   ├── ae-sdd-integrations/
│   ├── ae-sdd-governance/
│   ├── ae-sdd-flow/
│   ├── ae-sdd-delegation/
│   ├── ae-sdd-host/
│   ├── ae-sdd-context/
│   ├── ae-sdd-runtime/
│   ├── ae-sdd-client/
│   ├── ae-sdd-session/
│   ├── ae-sdd-resources/
│   ├── ae-sdd-review/
│   ├── ae-sdd-execution/
│   └── ae-sdd-build/
├── bins/
│   ├── ae-sdd-daemon/            # package ae-sdd-daemon, binary ae-sddd, composition root
│   ├── ae-sdd-cli/               # package ae-sdd-cli, binary ae-sdd, thin CLI + Hook adapter
│   └── ae-sdd-worker/            # 隔离执行有界 verification/review/tool job；不拥有流程状态
├── migrations/                   # monotonic SQLite runtime migrations
├── tests/
│   ├── contract/
│   ├── golden/
│   ├── concurrency/
│   ├── crash/
│   ├── e2e/
│   └── fixtures/
├── source/                       # 方法论、模板、standards、plugins 的人维护源
├── constraints/                  # 本规则层 SSOT
├── tools/ + scripts/             # 迁移期 Python oracle；cutover 后删除/收缩
└── apps/ae-sdd-monitor/          # 本 WorkItem 明确排除
```

- Rust production 代码只能放在 `crates/*/src` 或 `bins/*/src`；Rust integration test 放 owning package 的 `<package>/tests/*.rs`。根 `tests/` 只保存 fixture、golden、外部 harness/config 与非 Rust test data，不作为虚拟 workspace 的不可发现 Rust test target。
- 不得在 `source/`、`constraints/`、`dist/` 中混入 Rust implementation。
- 不得修改 `apps/ae-sdd-monitor/**` 来完成核心 daemon slice。

## 二、crate 职责

| crate | Owner 职责 | 禁止职责 |
| --- | --- | --- |
| `ae-sdd-protocol` | versioned RPC DTO、framing、capability、stable error 与 wire enums | socket I/O、policy、文件访问 |
| `ae-sdd-domain` | state/phase/route/value object/invariant；共享 `AgentRole`、`AgentLineage`、`ScopedGrant` vocabulary | async runtime、数据库、文件、宿主 API |
| `ae-sdd-contracts` | 跨 Part 冻结的 versioned DTO、port trait、大小上限与 extension contract；复用 domain/protocol owner 类型 | 流程编排、I/O、持久化、复制 domain/protocol 语义 owner |
| `ae-sdd-policy` | 唯一 TransitionPolicy、Hook/Gate/role 规则 | Gate I/O、状态落盘、CLI parsing |
| `ae-sdd-methodology` | 机器声明 Methodology Catalog 的纯编译、content digest、校验与 L1>L2>L3>L0 deterministic resolve | 从 Markdown 推断语义、filesystem/env/clock/random、物理 Agent 创建 |
| `ae-sdd-lifecycle` | 基于 frozen input 与唯一 TransitionPolicy 生成生命周期 decision/mutation intent | 直接写 store、执行 host action、复制 phase table |
| `ae-sdd-store` | lock/lease/CAS/fencing/idempotency/journal/atomic file/recovery | phase 路由与 Agent 语义 |
| `ae-sdd-artifacts` | doc/assets/memory/evidence/resource 读取与 hash-addressed artifact | session transport、全局 transition |
| `ae-sdd-operations` | typed operation registry 与 application orchestration | CLI UX、重复 policy table |
| `ae-sdd-gates` | 36 Gate 实现与 typed GateOutcome | Python subprocess、transition mutation |
| `ae-sdd-scanners` | 7 个 native scanner 与 scope/parser registry | 独立脚本业务入口、隐式 PASS |
| `ae-sdd-inventory` | selector/fingerprint/watch/reconcile/cache invalidation | 把 watcher event 当业务真相 |
| `ae-sdd-integrations` | git/SQLite/toolchain/plugin/distributor/OS service 平台 adapter | domain/policy 规则 |
| `ae-sdd-governance` | update graph、alignment audit | runtime transport |
| `ae-sdd-flow` | pure reducer、RouteEngine、SeriesPlanner、nextAction、FlowSupervisor、event replay/checkpoint | LLM 语义工作、host/store/filesystem I/O、物理 Agent 创建 |
| `ae-sdd-delegation` | Delegation/capability/ChildResult lifecycle、claim 与 validation；使用 domain-owned role/lineage/grant value objects | host-specific session creation、复制 role/grant permission table |
| `ae-sdd-host` | HostRuntimeAdapter trait、capability negotiation、action/ACK/attestation | policy、Gate、memory 内容 |
| `ae-sdd-context` | role-aware projection/delta/budget/snapshot/rehydrate/CompactCycle | 伪造宿主 ACK、推进业务 phase |
| `ae-sdd-runtime` | actor、scheduler、durable event/outbox、workspace/session/turn lifecycle | CLI rendering、方法论正文 |
| `ae-sdd-client` | Named Pipe/UDS client、deadline/reconnect | 本地 Gate/state fallback |
| `ae-sdd-session` | session bootstrap/binding 的 application contract 与纯状态决策 | 直接启动 daemon/host process、持久化实现 |
| `ae-sdd-resources` | document/memory/context resource resolve 与 transaction plan 的 application owner | 绕过 artifact/store port 直接覆盖权威文件 |
| `ae-sdd-review` | review session/findings/exit policy 的 application owner | 伪造 reviewer identity、直接执行外部工具 |
| `ae-sdd-execution` | verification/tool execution plan 与 receipt 的 application owner | shell 拼接、无界输出、把 skipped/timeout 记为 PASS |
| `ae-sdd-build` | library + `ae-sdd-build` bin；compile/init/install/distribute/harness/migrate admin jobs | 绕过 daemon/store 合同 |
| `ae-sdd-daemon` / bin `ae-sddd` | RPC server、lifecycle、concrete adapter composition root | command UX 与业务规则定义 |
| `ae-sdd-cli` / bin `ae-sdd` | 参数解析、client 调用、Hook stdin/stdout 映射 | policy/store/Gate/scanner 实现 |
| `ae-sdd-worker` / bin | daemon 管理的隔离 job process、bounded stdin/stdout、deadline/cancel 映射 | 路由、phase、Gate truth、会话权威状态 |

## 三、crate 内部结构

通用 library crate 使用下列最小结构；只在存在真实复杂度时增加模块。

```text
src/
├── lib.rs          # 公开模块与稳定 re-export，禁止 composition logic
├── model.rs        # 本 crate 拥有的类型/状态
├── error.rs        # typed internal error
├── service.rs      # application behavior 或 pure service
├── ports.rs        # 指向外部能力的 trait（按需）
└── adapters/       # 仅 adapter-owning crate 使用
```

- 一个类型由一个 crate 拥有；禁止在多 crate 复制同名 wire/domain model。
- inward crate 定义它需要的 port trait，outward crate 实现；禁止为了“共享”创建无 owner 的 `common`/`utils` 大杂烩 crate。
- `lib.rs` 只声明模块和稳定 API；单文件超过约 500 行或同时承担两个状态机时必须按职责拆分。
- platform-specific 代码放 `adapters/windows.rs`、`adapters/unix.rs` 或等价 `cfg` 模块；公共业务模块禁止散落 `cfg(target_os)`。

## 四、依赖方向

```text
domain                     protocol
  ↑                           ↑
contracts  (frozen cross-Part DTO/port boundary)
  ↑
policy + methodology
  ↑
flow/lifecycle/delegation/context/session/resources/review/execution/operations
  ↑
runtime (depends on inward contracts and port traits, never concrete adapters)

store/artifacts/gates/scanners/inventory/host/integrations
  -> depend inward only to implement ports

ae-sdd-daemon / ae-sdd-build binaries
  -> depend on runtime/application plus concrete adapters and inject them

ae-sdd-cli -> client -> protocol
```

- `domain` 与 `policy` 必须保持无 I/O、无 Tokio、无 SQLite、无平台 API。
- `contracts`、`methodology`、`flow` 与 `lifecycle` 的控制面路径必须保持纯确定性：不得读取 filesystem、environment、wall clock、random、process 或 global mutable state；所有事实必须经 typed input/port snapshot 显式传入。
- frozen contract 的 owner 是 `ae-sdd-contracts`；并行 Part 不得各自修改共享 DTO/port。语义变化必须由协调者先提升 schema/version、更新 golden fixture 与所有消费者，再继续实现。
- binary 是 composition root，可以装配具体 adapter；library 禁止反向依赖 binary。
- `ae-sdd-runtime` 禁止依赖 store/integrations/OS concrete adapter；adapter crate 可以依赖拥有 port trait 的 inward crate以实现该 trait。`ae-sdd-host` 只拥有 host port/contract，宿主 concrete implementation 位于 integrations。
- CLI/Hook 只依赖 `client + protocol` 以及必要的渲染模块，禁止依赖 store/gates/policy 的 concrete implementation。
- crate graph 必须无环；新增依赖边必须在 CodingPlan source/dependency list 中说明。

## 五、测试与 fixture 放置

- 同模块白盒 unit/property test 放 `src/**` 的 `#[cfg(test)] mod tests`。
- crate public contract integration test 放 `<crate>/tests/*.rs`。
- 跨 crate、进程、平台、crash、migration、host contract 和 pressure 的 Rust test 放最接近 composition boundary 的 owning package（通常 `crates/ae-sdd-runtime/tests`、`crates/ae-sdd-build/tests` 或 binary package `tests`）；其 fixture/golden/config 放根 `tests/<category>`。
- golden fixture 必须不可变、有 schema/version、输入 digest 和 preserve/breaking-fix 分类；不得把执行时临时输出提交为 golden。
- Python oracle 只可由 migration test harness 调用，测试 module 名和 feature 必须显式包含 `migration_oracle`，release dependency graph 中计数必须为 0。

## 六、生成物与迁移

- `target/`、runtime DB、socket/pipe manifest、临时 journal 和测试输出不得提交。
- `dist/`、安装包、service descriptor 和 compiled SKILL runtime 是生成物，只能由 `ae-sdd-build` 从受版本控制 source 生成。
- migration 期间旧 `tools/`/`scripts/` 按 compatibility manifest 删除；未有 Rust evidence 的入口不得提前删除。
- 所有路径 owner 在 task slice 中唯一；共享文件修改前必须重读 dirty diff 并采用增量 patch。

## 七、禁止事项

- 禁止创建第二套 TransitionPolicy、phase table、Gate truth table 或 role permission table。
- 禁止建立 `common`, `shared`, `helpers` 作为无边界依赖汇聚点。
- 禁止从 domain/policy 调用 filesystem、SQLite、clock、random、process 或网络；这些能力必须通过显式输入或 port。
- 禁止让 `ae-sdd` CLI、Hook adapter、HostRuntimeAdapter 或 build job 直接 mutation 项目 state。
- 禁止把 Monitor 源码、Python runtime 或 generated dist 纳入 Rust core crate graph。
