# 项目约束（Constraints）

## 这是什么

`constraints/` 是 ae-sdd 工程实现的规则层 SSOT：它定义 Rust 本地控制平面“必须怎样实现和验证”。SKILL 定义方法、模板和输出合同；FlowRuntime 才是运行时流程状态机，二者不得复制这里的工程规则。

```text
source/skills + templates  -> 方法与内容资产
constraints/               -> Rust 工程实现约束
crates/ + bins/             -> 受约束的实现
```

适用场景：Requirement Analysis、DR、Story、CodingPlan、Coding、Test 与 Review。任何实现任务在 Coding 前必须按影响面加载对应文件。

---

## 约束文件索引

| 文件 | 职责 |
| --- | --- |
| `technology-stack.md` | Rust toolchain、依赖、IPC、持久化与供应链 |
| `project-structure.md` | Cargo workspace、crate owner、目录与文件放置 |
| `layered-arch.md` | inward dependency、FlowRuntime、Agent/host adapter 边界 |
| `code-style.md` | Rust 命名、错误、async、日志、确定性与 `unsafe` |
| `api.md` | 本地 framed JSON-RPC、方法、错误、幂等与 Hook 映射 |
| `database.md` | SQLite runtime metadata、migration、事务、索引与文件权威边界 |
| `security.md` | OS 用户 IPC、capability、路径、子进程、插件与审计 |
| `testing.md` | unit/property/golden/concurrency/crash/E2E/跨平台测试 |
| `plugin-registry-spec.md` | 三层 plugin registry schema、解析、冲突与安全扫描 |
| `implicit-constraints.md` | 无法归入上述文件的迁移和运行隐性边界 |

---

## 编写与修改原则

1. 使用“必须/禁止”和可验证的字段、命令或状态，不使用“建议/尽量”替代约束。
2. 一条规则只在一个文件定义；其他文件引用 owner 文件，避免多份 policy。
3. 新依赖、协议字段、状态、持久化表或平台差异必须同时更新对应约束、DR/Story contract 和验证矩阵。
4. 约束不能用来绕过 ae-sdd gate；约束与正式设计冲突时必须停止 Coding，先更新上游资产并重新加载 CodingModel。
5. 修改现有文件前必须检查 dirty diff；禁止覆盖用户未提交变更。

### 规则 owner

| 规则域 | 唯一 normative owner | 其他文件的处理 |
| --- | --- | --- |
| toolchain、dependency、license、IPC/DB/platform 技术选型 | `technology-stack.md` | 只能引用，不得重定义版本或 allowlist |
| wire method、DTO、前置字段、错误码 | `api.md` | 只能说明本层如何消费该 contract |
| secret、capability、身份、路径与审计安全 | `security.md` | 只能增加所属层实现责任，不得放宽 |
| crate/层/role/daemon/Monitor 边界 | `project-structure.md` + `layered-arch.md` | `implicit-constraints.md` 仅登记兼容不变量并引用 owner |
| schema、journal、event/checkpoint 持久化 | `database.md` | API/Story 只镜像 contract 字段 |
| test topology 与 evidence 门槛 | `testing.md` | Story verification matrix 绑定具体 AC/命令 |
| 切片级 RED-GREEN-REFACTOR cadence 与其 evidence 要求 | `testing.md` | AGENTS.md/SKILL/Story 只能引用；生命周期状态机由 `crates/ae-sdd-execution` 实现 |

同一规则在 DR/Story 中出现是正式设计合同镜像，不产生第二 owner；发生差异时停止 Coding并先按上表修正 owner 与正式文档。

## 最小准入检查

- 每次 Coding 前动态加载技术、结构、分层、代码、API、数据、安全、测试与 plugin 约束；本文件不证明它们已加载。
- 动态确认 Story 的每个 AC 都有真实验证入口；Test/Review 只记录实际 evidence 和 findings。
- 针对当前 state revision 动态确认 `state.executionPlan` 用户批准与 G-CODEPLAN-SRC、G-14、G-08 PASS；静态文字不构成 PASS。
- 动态确认 Monitor 排除项未被误算为 released Rust runtime。

## Agent 执行约束

- 所有 Agent 在首次写 Rust、schema、migration、build script 或测试前，必须通过 ae-sdd 加载本目录的当前 digest；对话中曾经读过旧副本不构成有效加载。
- 多 Agent 并行时必须先冻结共享 contract 与 migration 编号，再分配互斥文件 owner；Part Agent 不得修改其他 Part 的 owned path。
- pure control-plane crate 的 Review 必须检查 I/O/clock/random/global-state 依赖为 0，并用 replay/determinism test 证明相同输入产生 byte-identical decision。
- 完成声明必须同时具备 `cargo fmt`、strict Clippy、受影响 crate/模块及对应测试文件的 focused/增量测试、真实 ae-sdd evidence 与无 blocker/major finding 的 Review；workspace 全量回归仅在 release/分发门禁要求；只编译成功不算完成。
