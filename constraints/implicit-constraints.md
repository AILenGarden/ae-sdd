# 隐性约定

## 摘要

本文件只记录无法归入其他约束文件、但会改变 ae-sdd Rust daemon 正确性的项目级约定。
适用场景：跨 slice 协作、迁移切换、Agent 宿主集成和生成物处理。

---

## 一、流程与 Agent 边界

- daemon 位于 Agent 层级之外；`FlowRuntime + WorkItemActor` 是 phase、nextAction、correction、pause/resume 和 transition 的唯一确定性 owner。
- SKILL 只提供声明式方法、模板和输出合同；Agent 负责语义工作，不得在 prompt、Hook 或 CLI 中复制流程状态机。
- 三层执行固定为 `root -> series -> task|reviewer`，最大深度 2。root 只编排、collect、汇报和申请 transition；series 只拥有一个系列；task/reviewer 只处理 assignment scope。
- delegation 记录不等于物理 Agent。只有 authenticated HostRuntimeAdapter ACK 与 child session claim/attestation 都有效时，delegation 才能进入 running。
- root 只读取 bounded ChildResult、artifact index 和 flow projection；禁止把 child transcript、源码全文或完整系列文档注入 root context。

## 二、迁移边界

- shadow 阶段 Rust 只读比较；同一 workspace 任一时刻只能有一个 writer。切换与回滚必须先 drain，再原子更新 workspace mode。
- Rust sole-writer 生效后，CLI/Hook/daemon 禁止回退到 Python。Python 只允许在 migration test profile 中作为 oracle。
- `apps/ae-sdd-monitor/**` 整体延后；任何 T1-T6 核心任务不得修改、依赖或用 Monitor 行为阻断 cutover。
- generated `dist/`、runtime package 和安装产物只能由 build job 生成，禁止手工修补。

## 三、状态与协作

- project-scoped 请求必须显式携带 project 与 WorkItem；lease、fencing、revision、idempotency 与 confirmation 按 operation registry 的 `requires*` flags 强制。`lease.acquire`/`lease.break` 不得被 active-lease 前置条件锁死；禁止通过“唯一候选”猜 WorkItem。
- watcher、Hook、prompt 和 host ACK 都只是输入事件，不是业务成功证明。只有 committed receipt、fresh GateOutcome 和经验证 artifact 才能推进状态。
- 重复 prompt 不得增加 correction；只有 fresh `FAIL` 是业务失败，`ERROR/TIMEOUT/CANCELLED/STALE` 必须保留各自语义。
- compact request 不等于 compact 完成；只有匹配 session/generation 的宿主 ACK 加 rehydrate 成功后才推进 context generation。
- 所有 slice 修改前必须重读 `git status`/diff；共享 dirty worktree 中不属于当前 owner 的修改一律保留。
