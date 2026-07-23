# 技术栈规范

## 摘要

本文件定义 ae-sdd 核心运行时、CLI、Hook adapter 和工程工具的技术栈。所有 released execution path 必须使用 Rust；Markdown 方法论与模板仍作为数据输入。
适用场景：技术选型、依赖引入、DR 评审、构建与发行审计。

---

## 一、语言与工具链

| 项目 | 约束 |
| --- | --- |
| 语言 | Rust 1.97.1 stable，edition 2024 |
| 工具链锁定 | 根目录 `rust-toolchain.toml` 必须固定 `1.97.1`；禁止只写浮动 `stable` |
| MSRV | 首版 MSRV 为 Rust 1.97；降低或提升 MSRV 必须有独立兼容性验证和 DR 记录 |
| 组件 | `rustfmt`、`clippy` 必装；交叉平台 target 由 CI matrix 显式安装 |
| 依赖版本 | 工作区统一在 `[workspace.dependencies]` 管理；`Cargo.lock` 必须提交并用于所有 release/CI 构建 |

T1 已从 Rust 官方发布渠道安装并选定 `rustc 1.97.1` / `cargo 1.97.1`；仍必须把 `rustc --version --verbose`、`cargo --version` 与 lockfile digest 记录为 T1 evidence。

## 二、核心库与默认选型

| 能力 | 默认技术 | 强制边界 |
| --- | --- | --- |
| 异步 runtime | Tokio 1.x | I/O 使用 async；阻塞文件、SQLite 和外部进程等待必须进入有界 blocking executor |
| CLI | clap 4.x | CLI 只解析参数、渲染结果和连接 daemon，不持有 Gate、policy 或 state mutation 逻辑 |
| 序列化 | serde + serde_json | wire schema 使用显式 DTO；参与 digest 的 map 使用确定性顺序，禁止直接 hash 任意 JSON object |
| 错误 | thiserror | library 返回 typed error；binary 边界映射为稳定错误码，禁止把内部堆栈作为协议 |
| 日志/追踪 | tracing + tracing-subscriber | 结构化字段；禁止记录 prompt、transcript、token、claim 或 secret 正文 |
| ID/时间 | uuid + UTC RFC 3339 | daemon 生成内部 ID；duration/deadline 使用整数毫秒 |
| digest/MAC/signature | SHA-256 + HMAC-SHA-256 + Ed25519（`ed25519-dalek` 2.x） | protocol/artifact digest 统一 64 位小写 hex；secret 比较 constant-time；离线只读 capability verification 必须非对称，boot private key 不落盘 |
| runtime metadata | SQLite WAL，默认 `rusqlite` bundled | SQLite 只保存可恢复运行元数据；项目文件仍是业务真相 |
| 文件监听 | `notify` 8.2.x 或经 DR 批准的等价跨平台 adapter | watcher 事件只用于失效提示，mutation 前必须重算内容 fingerprint；禁止选 9.x pre-release 进入首版 |
| 平台 API | Tokio Named Pipe/UDS；Windows DACL/handle 边界使用最小 feature 的 `windows-sys` | 平台代码只存在于 integrations/client adapter，不得泄漏进 domain/policy；`unsafe` 限于单一审计模块 |

引入替代库必须在 DR 说明：为何默认选型不满足、额外 transitive dependency、平台支持、license、advisory、crash/取消语义和回滚方式。

## 三、本地通信与进程模型

- 每个 OS 用户、每个 protocol major 只运行一个 `ae-sddd`。
- Windows 使用 Named Pipe；macOS/Linux 使用 Unix Domain Socket；默认禁止 TCP/HTTP listener。
- wire protocol 使用 4-byte big-endian 长度前缀 + UTF-8 JSON-RPC 2.0；默认 frame 上限 16 MiB。
- `ae-sdd` CLI 和 Hook adapter 必须是薄 client；daemon 不可用时不得执行 Python 或本地业务 fallback。
- 外部 `git`、编译器、测试 runner 和宿主命令必须通过 integrations adapter 以参数数组启动，禁止拼接 shell 字符串。

## 四、持久化与文件系统

- WorkItem state、lease、文档、memory、evidence 和可审计 artifact 继续保存在项目目录中。
- daemon SQLite 保存 workspace/session/turn/delegation/host-action/job/event/cache/receipt 等运行元数据，不得成为 phase、executionPlan 或 review 的唯一存储。
- lease-protected 项目 mutation 必须使用跨进程锁、lease、revision CAS、单调 fencing、idempotency receipt、PREPARED/COMMITTED journal、同目录临时文件、fsync 和 atomic replace；`lease.acquire`/`lease.break` 等 registry 明示的无 lease 控制写仍必须在跨进程锁内维护单调 fencing、tombstone 与审计。
- 路径操作必须 canonicalize 并验证 allowed-root、symlink、junction 和 reparse-point containment。

## 五、依赖与供应链

- CI/release 必须运行 `cargo fmt --check`、`cargo clippy --workspace --all-targets --all-features -- -D warnings`、`cargo test --workspace --all-features`、`cargo audit` 和 `cargo deny check`。
- 默认允许 MIT、Apache-2.0、BSD-2-Clause、BSD-3-Clause、0BSD、ISC、Unicode-3.0、Zlib、CC0-1.0；0BSD 仅因 `interprocess` 2.4.x 的 `doctest-file`/`recvmsg` 传递依赖用于 Named Pipe/UDS，CC0-1.0 仅因 `notify` 8.2.x 的已审用途纳入，其他 license 必须经显式审查后加入 allowlist。
- 默认只允许 crates.io；git dependency 必须固定完整 commit revision并在 DR 说明原因，禁止 branch/tag 浮动依赖。
- 禁止 wildcard dependency；禁止未说明的同一 crate 多 major 版本；禁止把 yanked 或有未接受 advisory 的 crate 带入 release。
- `unsafe` 默认禁止；仅平台 adapter 在没有安全 API 可用时可使用，并必须有局部 `SAFETY` 证明、测试和独立 review finding=0。

YAML parser 尚未默认授权。plugin registry 实现前必须完成 Rust parser spike，证明 duplicate-key、alias expansion、depth、document byte size 与 scalar length 限制，并记录维护状态/license；已停止维护的 `serde_yaml` 不得进入 release dependency graph。

## 六、明确排除

- released core runtime 禁止调用 Python worker、Python fallback 或 Python scanner；Python 仅可存在于迁移测试 profile 作为只读 oracle。
- 禁止引入 Spring、JVM、HTTP server、MySQL、Redis、Kafka、ElasticSearch 或云 SDK 作为核心 daemon 前置依赖。
- `apps/ae-sdd-monitor/**` 不属于本轮 Rust daemon 迁移，也不作为核心 cutover 的构建或测试依赖。
- 禁止手工修改 generated `dist/`；必须由 Rust build job 从 source 输入重建。
