# ae-sdd 实现架构说明书

> v4.0.0 · 面向 ae-sdd 维护者。本文档描述代码实现结构、模块边界和设计-实现对齐规则；能力语义仍以 [`ae-sdd-design.md`](ae-sdd-design.md) 为入口。

## 过程状态与文档边界

`document_storage` 对 retired intent 保留 resolve/read 兼容，但 save/finalize 返回 E012；新写入只允许核心文档和显式可选文档。`state.json` 除 `executionPlan` 与 `review` 外，持有 `routeDecision` 与 `requirementSpec`，分别记录路由事实和需求说明书引用。新任务阶段为 `route-selected -> requirement-analyzed`，之后由 `routeDecision.selectedDesign` 选择 DR/Story/CodingPlan；旧 phase 仍兼容读取。evidence manifest 持有测试/交付证据。文档索引写入 `ae-sdd-doc/index.json`，不更新历史 `STORING.md`。

## 当前 Rust Runtime 实现基线

本节是当前实现权威；后续 Python `tools/`/`scripts/` 内容仅用于迁移盘点与 golden oracle，不进入 native release，也不构成 daemon 不可用时的 fallback。

### 进程与边界

| 进程/入口 | 职责 | 禁止 |
| --- | --- | --- |
| `ae-sddd` | 每用户单例 daemon；IPC、actors、FlowRuntime、scheduler、store、supervisor | 读取宿主 prompt 作为可信身份；将 endpoint secret 写日志/SQLite |
| `ae-sdd` | 薄 CLI/Hook/admin/host-adapter client；manifest + handshake + framing + host 输出转换 | 本地执行 Gate/state mutation；daemon 失败时调用脚本 |
| `ae-sdd-build` | native package、compatibility audit、release scan、Hook benchmark、service descriptor generation | 把 migration oracle 打入 release |

### Crate 分层

```text
domain <- protocol <- policy/flow/delegation/context/host
   ^             ^                ^
   |             |                |
store/artifacts/inventory/gates/scanners/operations
                 ^
                 |
runtime (ports + actors + supervisor)
                 ^
                 |
integrations/client/build -> ae-sddd / ae-sdd / ae-sdd-build
```

`ae-sdd-runtime` 只依赖内向 ports，不直接持有平台 adapter。`ae-sdd-integrations` 实现 Named Pipe/UDS、filesystem、SQLite、native service manager、Git/toolchain 与 Host process adapters。

### 请求路径

1. client 原子读取受保护 endpoint manifest，以 token、expected boot/policy 和 protocol range 完成 `runtime.handshake`。
2. Hook/session 请求绑定 workspace、work-item、agent/session、turn 与幂等 identity；业务 request 进入有界 actor mailbox。
3. `FlowRuntime`/operation registry 验证 role、lineage、schema、confirmation、lease/revision/fencing/idempotency；轻 policy 进程内执行，重 Gate 进入有界 scheduler。
4. Gate 返回后重新核对 state/policy/inventory/input freshness。non-PASS 保持原类型且阻断。
5. mutation 先写 project state directory 下的 `mutation-journal/v1` PREPARED entry，再 staged write/atomic replace/fsync，最后 COMMITTED receipt/event；SQLite 仅保存可重建 index/cursor。
6. `FlowSupervisor` 从跨 restart 全局 `eventStoreId + eventSeq` 重放 typed bounded event，持久化 checkpoint/decision 并预计算 role-aware context delta。

### Hook 与宿主合同

```text
ae-sdd hook --method hook.user_prompt --request-json -
ae-sdd hook --method hook.pre_tool --request-json -
ae-sdd hook --method hook.post_tool --request-json -
ae-sdd hook --method hook.stop --request-json -
```

stdin wrapper 为 `{params, engaged, offlineCapability?, nowUnixMs}`。engaged transport failure 仍按宿主协议输出 deny/block，不运行本地 Gate。SessionStart 映射 `session.open`；SubagentStart 必须先 `delegation.accept` 再 `session.open`；SubagentStop 映射 `delegation.report/session.close`；PreCompact 映射 `compact.request`；PostCompact 映射 `context.project`，原生 ACK 独立走 `host.action_ack`。宿主不支持事件时返回 unsupported，不伪造成功。

### Service 与 cutover

跨平台生成合同见 `source/skill-fallbacks/runtime/service-lifecycle-contract.md`；shadow/canary/rollback 与逐文件 legacy 删除门见 `source/skill-fallbacks/runtime/cutover-contract.md` 和 `legacy-runtime-cutover.v1.json`。Windows 使用当前用户 Task Scheduler 定义，macOS 使用 LaunchAgent，Linux 使用 systemd user unit。三者必须通过 actual CI 的 install/start/drain/upgrade/stop/uninstall、ACL 与恢复验证，单机模拟不算跨平台 PASS。

Monitor (`apps/ae-sdd-monitor/**`) 完整排除。

## 1. 文档边界

| 文档 | 职责 | 不承担 |
| --- | --- | --- |
| `source/SKILL.md` | Agent 运行入口、流程编排、门禁与子 SKILL 路由 | 代码模块架构说明 |
| `source/docs/ae-sdd-design.md` | 系统能力设计：能力是什么、为什么存在、当前能力边界 | 每个函数/文件的完整实现地图 |
| `source/docs/ae-sdd-implementation-architecture.md` | 实现架构：代码分层、模块边界、运行时数据流、变更闭环 | 历史方案细节和发版流水账 |
| `source/docs/plans/*.md` | 单次方案、调研、迁移记录 | 当前架构权威状态 |
| `source/CHANGELOG/*.md` | 变更原因、影响范围、验证方式 | 设计正文 |

原则：能力文档写稳定语义，实施细节写实现架构，历史推导写 plans 或 CHANGELOG。

## 2. 实现分层

```text
source/                         母版文档与方法论 SSOT
  SKILL.md                      Agent 主入口
  skills/                       子 SKILL
  skill-fallbacks/              源 SKILL 瘦身前完整原文，语义 fallback
  docs/                         能力设计、实现架构、方案归档
  standards/                    机器可读或人读标准
  templates/                    产物模板

bins/                           三个原生二进制入口
  ae-sdd-cli/                   薄 CLI/Hook/admin client
  ae-sdd-daemon/                每用户单例 daemon（`ae-sddd`）
  ae-sdd-worker/                隔离 build/test worker
crates/                         业务与基础设施 crate，见上文 Crate 分层
migrations/                     runtime store schema migration
scripts/                        少量非 Rust 辅助脚本（hook 安装、文档迁移）
dist/ae-sdd/                    编译后分发包，构建产物，不手工维护
  SKILL.md                      编译后主入口 bootloader
  skills/**/*.md                编译后子 SKILL bootloader，非原文
  runtime/                      compact runtime、manifest、fallback 原文
    subskills.compact.md        子 SKILL 编译入口索引
    skills/**                   子 SKILL 局部 manifest/boot/outline/fallback
harness/                        派生适配层，不手工改生成物
```

## 3. 运行时数据流

```text
用户/Agent 调用 ae-sdd
  -> ae-sdd-cli 读取受保护 endpoint manifest + runtime.handshake
  -> Named Pipe/UDS 送入 ae-sddd（薄 client 不做业务判断）
  -> RuntimeService 校验 role/lineage/schema/lease/revision/fencing/idempotency
  -> ae-sdd-gates: G-RA 共用单一 RA resolver（state raDocPath 优先，latest formal fallback）
       -> ae-sdd-scanners 提供 7 个扫描器的 authoritative scope 与运行时扫描
  -> ae-sdd-store: mutation journal PREPARED -> atomic replace -> COMMITTED
  -> 结构化 JSON 经同一连接回传，CLI 只做输出转换
```

硬门禁判断以 daemon 返回为准。SKILL 文档可以声明流程纪律，但不能替代 `ae-sdd-gates`、`ae-sdd-policy`、`ae-sdd-store` 的实现；daemon 不可用时 CLI 返回结构化错误，engaged Hook 输出 deny/block，不在本地近似求解。

## 4. 模块职责

| 子系统 | 主要文件 | 职责 | 变更要求 |
| --- | --- | --- | --- |
| CLI 入口 | `bins/ae-sdd-cli/src/main.rs` | 四个子命令（`runtime`/`rpc`/`hook`/`resume-approved-plan`）、manifest 读取、handshake、输出转换 | 保持薄入口；禁止在 CLI 侧做业务判断或本地 fallback |
| daemon 入口 | `bins/ae-sdd-daemon/src/main.rs` | 单例守护、allowed-root 准入、endpoint manifest 发布、actor 装配 | 新 RPC method 必须先定 owner/schema/权限/deadline，见 `constraints/api.md` |
| 协议层 | `crates/ae-sdd-protocol` | frame、handshake、capability token、RpcMethod 注册表、wire DTO | 字段变更须同步 operation schema digest 与 compatibility manifest |
| 传输与 IPC | `crates/ae-sdd-integrations`（`ipc.rs` / `endpoint.rs`） | Named Pipe/UDS、endpoint token、连接准入 | 禁 TCP listener；token 不入日志/SQLite |
| 状态机 | `crates/ae-sdd-flow` + `crates/ae-sdd-policy/src/transition.rs` | phase、route、series 调度、合法 transition 判定 | 改 phase 链需同步 gates/policy/tests |
| StateStore | `crates/ae-sdd-store`（`authority.rs` / `lease.rs` / `journal.rs`） | allowed-root、lease、fencing、revision CAS、idempotency、mutation journal、atomic persistence | 所有 mutation 必须经此层；并发/过期/损坏需 fail-closed 测试 |
| Typed operations | `crates/ae-sdd-operations/src/registry.rs` | 23 个 operation 的 registry、schema、describe/execute | 新 operation 必须有 schema、稳定错误码、测试，并同步 manifest 计数 |
| 门禁 | `crates/ae-sdd-gates`（`registry.rs` / `evaluator.rs` / `scheduler.rs`） | 36 个 Gate 注册表、DAG、有界调度、G-RA 单一 RA resolver | 改门禁需同步 UC-02/UC-03 与 focused tests；G-RA-1~6/FLOW 不得各自猜 RA |
| 扫描器 | `crates/ae-sdd-scanners`（`registry.rs` / `scope.rs`） | 7 个扫描器、authoritative scope、JSON 报告契约 | scope 判定必须由 gates 与全部 RA scanner 共享 |
| 诊断检查 | `crates/ae-sdd-integrations/src/jobs/diagnostics/` | `update-check`（UG 查询 + UC 语义检查）、`iteration-check`（IC-1~4）、doc-storage gate | 见 §11 实现度；新增 UC 必须同步 `update-graph.json` |
| 文档与资源 | `crates/ae-sdd-resources`（`document.rs` / `assets.rs` / `resolver.rs`） | intent 驱动的文档 resolve/read/save/finalize、`.assets.md` 读取、outline/section/query/stats | 只读 intent 禁止写入；禁止 fuzzy Story ID 选择；缓存变更需测试失效路径 |
| 生命周期 | `crates/ae-sdd-lifecycle` + `crates/ae-sdd-integrations/src/lifecycle_authority.rs` | WorkItem/Story/PRD typed 生命周期、projection、校验 | 新 intent 必须有 validation 与 focused test |
| Review | `crates/ae-sdd-review`（`supervisor.rs` / `fingerprint.rs` / `policy.rs`） | Tier、角色独立、batch/round、fingerprint、预算、clean streak、retry/exit | `STALLED` 不得映射为 PASS；平台失败只重试缺失角色 |
| 执行与证据 | `crates/ae-sdd-execution` + `crates/ae-sdd-contracts/src/evidence.rs` | executionPlan、slice、capsule、receipt、evidence manifest | evidence 只登记真实运行结果；禁止手写 PASS |
| 上下文 | `crates/ae-sdd-context`（`projection.rs` / `compact.rs` / `pressure.rs`） | role-aware 投影、delta、LoadedContextProof、pressure 与 compact 周期 | 投影字节数不是 token 遥测；ACK 不得伪造 |
| Hook 层 | `crates/ae-sdd-client/src/hook.rs` + `crates/ae-sdd-policy/src/hook.rs` | 四个 Hook method 的 fail-closed 宿主输出与 HookGuard 裁决 | engaged 传输失败仍须 deny/block；Hook 内不跑扫描或长任务 |
| Methodology | `crates/ae-sdd-methodology`（`catalog.rs` / `compiler.rs` / `verifier.rs`） | Skill Catalog IR、版本/digest、plugin winner、compiled runtime 校验 | `SKILL.md`、`runtime/**`、`skills/**/*.md` 输出必须字节级幂等 |
| 构建分发 | `crates/ae-sdd-build`（`post_commit.rs` / `managed_instructions.rs` / `release.rs`） | compile -> verify -> distribute -> 托管 L2 指令注入、compatibility audit、release scan、Hook benchmark | 见 §8.1；不把手工改动写入 dist；发布链路禁解释器 |
| Host 适配 | `crates/ae-sdd-host` + `crates/ae-sdd-integrations/src/host_supervisor.rs` | spawn/send/wait/cancel/attest/compact 的受约束物理执行与 attestation | 凭证不进 state/manifest/log；不得伪造物理会话 |

## 5. CLI 入口规则

`bins/ae-sdd-cli` 是薄入口，只有四个子命令：`runtime`、`rpc`、`hook`、
`resume-approved-plan`。

- 业务逻辑全部在 daemon 内；CLI 只做 manifest 读取、handshake、framing 和宿主输出转换。
- 每个业务请求都先成功读取受保护 endpoint manifest 并完成 `runtime.handshake`，校验
  `bootId`、`policyDigest`、protocol range、limits 和 capability 公钥。
- daemon 不可用或 endpoint 过期时返回结构化错误；engaged Hook 输出 deny/block。禁止
  本地近似求解、本地 Gate 或本地 state mutation。
- `rpc` 只接受精确注册的 method 名与完整 `RequestParams` 对象；未注册 method 直接拒绝。
- 新增 method 必须先确定 owner、request/response schema、权限、幂等 key、deadline、
  错误码和验证矩阵，禁止只加 CLI 分支。
- PowerShell 或其他 shell 批量编排必须逐命令检查退出码；最终 process exit 只代表最后
  一条命令。不得用后续成功覆盖前序失败。
- Windows 上裸 `ae-sdd` 命中 `%LOCALAPPDATA%\Programs\ae-sdd\ae-sdd.exe`。安装期写入
  `HKCU\Environment\Path` 并广播环境变化；当前父进程仍需重启才能继承。

## 6. Gate 与 Scanner 规则

门禁分两层：

| 层 | 位置 | 说明 |
| --- | --- | --- |
| gate 编排 | `crates/ae-sdd-gates/src/registry.rs` + `evaluator.rs` / `scheduler.rs` | gate ID、名称、强度、DAG 依赖、有界调度与分发 |
| scanner 规则 | `crates/ae-sdd-scanners`（`registry.rs` / `engine.rs` / `scope.rs` / `report.rs`） | 具体静态扫描、authoritative scope 与 findings 生成 |

规则：

- `ae-sdd-gates::registry` 是门禁列表的代码权威，当前 36 个 Gate。
- 评估必须覆盖每个注册 gate，不能 stub-pass 掩盖缺口；`compatibility-audit` 校验计数。
- G-PATH 的 SSOT 豁免按 scan root 下的严格相对路径识别，仅覆盖 canonical document-storage source entry、source full fallback 和 compiled runtime fallback；basename 相同但父目录错误的文件不得豁免。
- G-PATH 项目侧只扫描 `.ae-sdd/memory/**/*.md`、顶层 `AGENTS.md`/`CLAUDE.md`/`MEMORY.md` 与 `.harness/memory/**/*.md`；`.ae-sdd/drafts/**/*.md` 属于过程产物，不纳入该 gate。`current_story` 不作为项目路径静默过滤条件。
- scanner 输出 JSON 必须包含项目根 `root`、`status`、`scannedPaths`、顶层统计、同值 `reportStats` 和 `findings[]`；G-CODE-1 对路径安全/唯一性/scope 覆盖、exit/status、finding schema 及全部计数执行 fail-closed attestation。production eligibility 与 scanner 枚举使用同一文本代码边界：Java/Kotlin/XML/YAML/properties 加 `.py/.js/.ts`，生成目录、虚拟环境/site-packages、`__tests__` 和常规 Python/JS/TS test/spec 命名在两侧均排除。scanner 的自身规则常量不参与业务判定；业务代码同 URI 不豁免，真实 pom metadata 由 `ae-sdd-scanners::parser` 的有界 XML 解析确认（拒绝 DTD 与畸形输入）；不提供通用 inline suppression。
- `test-authenticity` scanner 在逐行规则之外执行文件级 HTTP 判定：MockMvc/application-context-bound WebTestClient 产生 `mock-http-boundary`；真实端口 marker 与内部 Service/Repository/Mapper/Application mock bean 共现产生 `http-internal-mock`。外部 Client stub 不被误分类为内部主链，但只可生成 supplemental evidence。
- Coding scanner 仅在 XML 解析确认真实 Maven POM 根元素后豁免标准 `xmlns`/`xsi`/`schemaLocation` 元数据 URL；Java 或 XML 中的实际外部 endpoint 仍按 `hardcoded-external-url` 阻断。
- G-RA-1~6/FLOW 共用 `_resolve_selected_ra()`：合法 `state.raDocPath`/active Story `raDocPath` 优先，否则从 `resolve_ra_scan_scope(root).files` 选择统一 latest formal RA。scanner gate 必须传 `--file <selected>`，details 必须包含 `selected_file`、`selection_source`、`scope_mode`；selected RA 自身有 blocker 时仍 fail closed。
- RA scanner 显式 `--file` 是 caller-authoritative scope，但仍要求项目根 containment、普通 Markdown 文件和存在性；错误以 exit 2 + `INVALID_RA_SCAN_SCOPE` JSON 返回。未传 file 的 root audit 排除 `references/templates/CHANGELOG/dist`、依赖/缓存和 GeneratePlan/Impact/ReverseIssues/Review/Report 等 event sidecar，同时保留 canonical `ae-sdd-doc/RA` 与 legacy `design/**/RA-*`。
- 新增 scanner 必须注册进 `ae-sdd-scanners::registry` 并同步 `compatibility-audit` 计数。
- scanner 在 daemon 进程内以有界方式运行（文件数、事件数、字节数均有上限），不 spawn 子进程 CLI。
- Review Batch、baseline、VerificationPlan 和 evidence 由 `ae-sdd-review`、`ae-sdd-execution` 与 `ae-sdd-contracts::evidence` 提供；所有 fingerprint 使用 canonical JSON，避免依赖 Git/mtime。状态写入保留 `reviewLoop` 兼容投影，门禁优先读取 `reviewSession`/batch v2。
- `verification.plan` 先规范化项目内真实文件，再由 `StateStore` 以 lease + fencing token + revision CAS 原子写入 Work Item 绑定 plan；`dryRun` 不写任何状态文件。计划同时保留兼容 `inputFingerprint` 和专用 `evidenceInputFingerprint`，Evidence 命令不得使用 `planFingerprint`。G-09 与 G-CODE-1 共用 changedPaths containment、plan fingerprint 和 evidence/artifact hash 校验；G-CODE-1 再过滤测试/文档，只保留生产代码，要求 scanner `scannedPaths` 完整覆盖该 scope，空生产 scope 阻断，无 scope 保持全仓结果。evidence artifact 必须是项目内文件，record 复制到内容寻址 immutable snapshot；同 logical key 的旧 active entry 标记 superseded，finalize 只校验 active snapshot，旧 schema manifest 读取不静默改写。HTTP plan 由 G-09 调 `validate_http_acceptance_manifest()`，只接受 active `http-local`/`http-test-env`，校验 current input fingerprint、artifact、loopback/non-loopback URL、同 buildId、AC 覆盖和 local→test-env 顺序；`http-external-supplemental` 不计入完成度。
- LLM 写入口统一由 `crates/ae-sdd-operations::registry` 提供，经 `operation.describe` / `operation.execute` 两个 RPC 暴露。describe 返回 JSON Schema，execute 执行显式 Work Item 的 typed operation；未知 operation、raw state patch、项目根不匹配、路径越界和缺少 lease/revision/idempotency 均 fail closed。
- G-13 的 DR exemption 只由 `entryNode=STORY && scale=中` 触发；实现继续校验 Story 本体与成熟阶段的 Task/CodingReport/CodeReview 存在及引用关系。
- G-07/G-08/G-14/G-CODEPLAN-SRC 共用 `_resolve_codingplan_doc()`，通过 `document_storage.resolve_scoped_artifact()` 按显式 Work Item 优先定位，Story 仅作唯一兼容 fallback。G-08 根据 `state.scale` 选择 full 或 micro profile；G-14 仅对无 Story 的 standalone 微任务返回可审计 N/A。
- G-02/G-14 共用 `_resolve_story_doc()`，底层 `resolve_story_document()` 按 bound path、精确非 glob StoryName、无 StoryName 时 Story-category-only ID 路径解析。Task/Coding 等同名文档不参与；canonical 多候选同样 ambiguity。正式文件反校验 `Story ID` 元数据；歧义、漂移和非法 basename 以稳定错误码阻断。
- 🆕 v3.9.1 注册表模式：同族门禁（如 G-DR-CTX/G-STORY-CTX/G-TESTCASE-CTX/G-TASK-CTX 四个上下文加载准入门禁）用 `CONTEXT_GATE_REGISTRY` 注册表 + 单个 `_check_context_loaded` 函数服务多个 gate_id，避免每门禁重复写 scale 豁免/phase 感知/逐项校验逻辑；4 个薄封装 `check_g_*_ctx` 对齐 `CHECK_FUNCS` 的 `(project_dir, st, current_story)` 签名，内部转发到统一实现。

## 7. 项目侧状态与缓存

项目侧运行状态集中在 `.ae-sdd/`：

| 路径 | 用途 |
| --- | --- |
| `.ae-sdd/config.yaml` | 项目配置 |
| `.ae-sdd/state.json` | 不再作为 active state、mirror 或 fallback；旧项目残留文件只能视为历史数据，不得参与新状态解析 |
| `.ae-sdd/session-context/` | 会话级 work-item 绑定缓存；`UserPromptSubmit` 从当前 prompt/cwd 解析真实 work-item 并写入，`PreToolUse` 只用同一 session key 读取，禁止跨会话共享 |
| `.ae-sdd/.hook-activity/` | session 级 turn token；只记录激活时间、最近时间和来源，不保存 prompt 正文；Stop 成功或普通新 prompt 清理；不参与 Work Item lease |
| `.auto-engineering/{workItemKey}/state.json` | work item 独立状态机；新建入口为 `operation.execute workitem.create`（workspace-scoped；`entryNode` 仅 PRD/DR/STORY；可省略 `workItemId`，由 daemon 铸造 `{entryNode}-{8 位小写 hex}` 业务键，响应 `data.workItemId` 为可寻址业务键、`data.stateMachineId` 为带 UUID 前缀的目录身份），目录名为 R6 顶层名（🆕 v3.10.1 带随机 UUID 前缀，如 `{uuid}-PRD-001`）；v3.11.3 可保存 StoryName/docPath 正文指针 |
| `.ae-sdd/memory/` | 分区 compact 记忆；task(L1) 默认任务级、project(L2) 跨任务复用，UserPromptSubmit 任务优先注入并只用 project 补充 |
| `.ae-sdd/plugins/` | 项目层插件注册 |
| `.ae-sdd/cache/` | 工具链缓存，新增缓存优先放这里 |
| `.ae-sdd/runtime-stats/` | Runtime Stats JSONL，本地观测数据，可清理，不进入版本控制 |
| `.ae-sdd/baselines/` | 用户批准的 gate baseline（默认 G-CODE-1），带 ruleset/content hash | 不自动创建；规则或触碰文件时重新确认 |
| `.ae-sdd/doc-aliases.json` | 旧文档路径到 canonical 正文的 alias registry | 只存指针，不存第二份正文 |
| `.auto-engineering/{story}/evidence/manifest.json` | Story 验证证据索引与复用条件 | 保存 canonical content hash；失败证据、manifest/input/artifact hash 不一致均不可复用 |

`PreToolUse` 解析 Work Item 时遵循“显式优先、隐式 fail-closed”：真实 `ae-sdd` Bash 命令携带 `--work-item` 时，先通过 `paths.find_work_item_state_path()` 定位该状态，再执行原有 phase、memory 与 gate 校验；目标不存在时直接拒绝，不得回退到其他候选。未携带显式目标时只使用同 session 绑定，缺失绑定即 fail-closed（constraints/api.md §五：`projectKey/workItemId` 精确匹配，不从唯一候选猜测）；绑定由 `workitem.create` 成功后 daemon 持久写入 `session.current_work_item` 建立，不做默认候选解析。有效的显式目标可写入当前 session 绑定，使同一会话的后续 Write/Edit 继续落在该 Work Item 上。

`PreToolUse` 对 ae-sdd Bash 命令只维护一套 token-aware 前缀解析：接受直接 `ae-sdd ...`、`python C:/.../ae-sdd ...`、`python "C:/.../ae-sdd" ...`，以及带引号的 Python 可执行文件路径；各 state、memory、assets 与 readonly 分支消费同一组规范化参数。解析只确认命令身份，不负责放行；`_CHAIN_RE` 与 `_REDIRECT_RE` 仍在 fast path 外层拒绝链式、命令替换和重定向载荷。引号未闭合、解释器或脚本 basename 不精确匹配时返回“非 ae-sdd 命令”，不得猜测或前缀模糊匹配。

Hook activation 与 Work Item state/lease 解耦：`prompt_inject` 仅在显式 `/ae-sdd` turn 创建 `.ae-sdd/.hook-activity/<session-hash>.json`，普通 prompt 先清理残留 token 后直接返回空 payload；`gate_intercept` 只对 active token 执行 phase/path/memory 门禁，并允许明确的 ae-sdd 写流程入口启动当前 turn；Stop CLI 在 active token 下执行 `stop_check`，成功或 fail-open 释放 token，阻断重试保留 token。旧 `.session-engaged` 文件不再作为激活依据。

`crates/ae-sdd-domain/src/lifecycle.rs` 与 `crates/ae-sdd-flow` 保留状态字段、phase 流程和终态不变量；`crates/ae-sdd-store`（`authority.rs` / `lease.rs` / `journal.rs`）是所有 Work Item mutation 的唯一并发所有者。store 在构造、锁、lease、state 与临时替换文件处校验 resolved path 仍位于 allowed root；独占创建在事务锁内完成，已有 state 不覆盖。mutation 在同一 Work Item 锁内二次读取 lease/state，检查 fencing token 与 revision CAS，先写 `mutation-journal/v1` PREPARED entry，再用临时文件 + fsync + atomic replace 写回，最后落 COMMITTED receipt；lease 过期接管会递增 fencing token，重复 idempotency key 返回原结果。`phase`/`history` 表示生命周期主状态；`currentPhase`、`currentStep`、`completedSteps`、`pendingOutputs`、`codingRound` 是工作流投影字段，不能独立滞留在旧步骤。写入生命周期 phase 时必须级联同步投影，落盘前执行终态不变量校验。Story 正文绑定在嵌套 state 使用 `storyStates[storyId].storyName/docPath`，扁平兼容 state 使用 `storyName/storyDocPath`；绑定经短租约 mutation 写入，重复绑定不增加 revision。

`routeDecision.selectedDesign` 的合法值为 `DR`、`STORY`、`CODING_PLAN`。读取侧必须先折叠 `-`、`_` 和空格再比较（`business.rs::normalize_route` 与 `lifecycle_authority.rs::normalize` 共用同一规则），否则分类器写出的 `CODING_PLAN` 会被判为非法路由。

新增项目侧文件必须说明是否可删、是否进入版本控制、是否参与 gate。

## 8. 构建与分发

构建链路：

```text
source/
  -> ae-sdd-build post-commit: compile     # source/ -> dist/ae-sdd/
  -> ae-sdd-build post-commit: verify      # 只读校验编译产物
  -> dist/ae-sdd/
  -> ae-sdd-build post-commit: distribute  # -> 各 skill 目标目录
  -> Agent skills runtime
```

阶段划分与权威边界见 §8.1，那里是发布链路的当前真相。

规则：

- `dist/ae-sdd/` 是构建产物，不手工维护。
- `source/SKILL.md` 与 `source/skills/**/*.md` 可以是 slim entry，但必须符合 `ae-sdd-source-slim/v2`，完整原文必须在 `source/skill-fallbacks/**`。
- `ae-sdd-build post-commit` 的 verify 阶段必须校验 fallback 哈希、语义 inventory hash、标准/模板路径和模板重渲染一致性。
- runtime compact 文件由编译器生成，不手改。
- `dist/ae-sdd/SKILL.md` 必须是编译后的主入口 bootloader。
- `dist/ae-sdd/skills/**/*.md` 必须是编译后的子 SKILL bootloader，不允许保留 `source/skills/**/*.md` 原文。
- 子 SKILL 原文 fallback 只允许出现在 `runtime/skills/**/fallback/SKILL.full.md`。
- `runtime/manifest.json` 必须记录 `subskills` 与 `extracts.subskill_count`，并与实际 `source/skills/**/*.md` 数量一致。
- `runtime/subskills.compact.md` 是子 SKILL 入口索引，路由到每个子 SKILL 的局部 `manifest.json`、`boot.compact.md`、`outline.compact.md` 和 fallback。
- 新增业务能力放对应 `crates/ae-sdd-*`，并在 `Cargo.toml` workspace members 与 `constraints/project-structure.md` 同步登记。
- scanner 与 gate 的共享 scope 判定放 `ae-sdd-scanners::scope`，禁止各 scanner 自带一份；新增 scanner 必须进 `registry` 并同步 `compatibility-audit` 计数。
- 分发器只能安装编译后 package，不能直接安装 `source/`。

### 8.1 Rust 发布期 post-commit 链路（当前真相）

`.githooks/post-commit` 只调 `ae-sdd-build`，五个阶段顺序固定：

```text
git commit（功能性变更）
  -> ae-sdd-build harness          # 生成 .harness/agent.md
  -> ae-sdd-build post-commit
       1 compile                   # source/ -> dist/ae-sdd/（含 L2-DISCIPLINE.md）
       2 verify                    # 只读校验编译产物
       3 distribute                # dist/ae-sdd/ -> 5 个 skill 目标目录
       4 managed L2 instructions   # 锚点区间替换 3 个全局指令文件
```

L2 注入权威边界：

| 角色 | 归属 |
| --- | --- |
| 正文 SSOT | `source/L2-DISCIPLINE.md`（发布期读编译产物 `dist/ae-sdd/L2-DISCIPLINE.md`） |
| 发布期执行者 | `ae-sdd-build`（`crates/ae-sdd-build/src/managed_instructions.rs` 纯渲染 + `post_commit.rs` 编排） |
| 落盘者 | 既有 native `Admin` job，entrypoint `post-commit.managed-instructions`，每 host 一次独立事务 |

托管目标映射由 CLI 显式给出，禁止从 skill 目标目录字符串推断：

| host | 语言 | 目标 | 注入 |
| --- | --- | --- | --- |
| codex | en | `$USER_HOME/.codex/AGENTS.md` | 是 |
| claude | zh | `$USER_HOME/.claude/CLAUDE.md` | 是 |
| zcode | zh | `$USER_HOME/.zcode/AGENTS.md` | 是 |
| harness / hermes | — | — | 否，仅包分发 |

不变量：

- 只替换 `ae-sdd-l2-ssot` 锚点区间，区外字节与原行尾逐字节保留。
- 目标文件缺失或无完整锚点报 `missing-target` / `missing-anchor` 跳过，绝不创建文件、绝不自动 bootstrap。
- 锚点或 SSOT 标记畸形、越 allowed root、symlink、落盘失败一律 fail closed，进程非零，目标文件不变。
- 审计头不含 wall clock，revision 由 `PostCommitRequest.commit_id` 注入，保证重放确定性。
- 托管注入排在包分发之后：任何托管跳过或失败都不回滚已完成的 skill 包分发。
- 跨 host 非原子：逐 host 事务保证单文件原子与回滚，所有 host 结果必须上报。
- 变更 L2 正文、托管映射或渲染器时必须跑 `managed_instruction_sync`、`compatibility_routes`、`migration_oracle`（详见 `source/standards/update-graph.json` UG-32）。

Runtime 编译数据流：

```text
source/SKILL.md
  -> source/skill-fallbacks/SKILL.full.md               # 源瘦身前完整语义
  -> dist/ae-sdd/SKILL.md
  -> runtime/{boot,route,gates,flow,macros}.compact.md
  -> runtime/fallback/SKILL.full.md

source/skills/**/*.md
  -> source/skill-fallbacks/skills/**/*.full.md         # 源瘦身前完整语义
  -> dist/ae-sdd/skills/**/*.md
  -> runtime/subskills.compact.md
  -> runtime/skills/**/{manifest.json,boot.compact.md,outline.compact.md,fallback/SKILL.full.md}
```

实现边界：

- 编译器只读取母版与工具注册表，不解释业务流程，不替代 `ae-sdd gates check`。
- 源瘦身器负责修改 `source/SKILL.md` 与 `source/skills/**/*.md`，runtime 编译器不负责瘦身源文件。
- runtime 编译器发现 `source_slimmed: true` 时，必须从 `source_fallback` 读取完整原文作为 runtime fallback 和 outline 抽取输入。
- `ae-sdd-build post-commit` 的 compile 阶段把母版复制为 dist 并生成 runtime。
- `crates/ae-sdd-methodology/src/verifier.rs` 负责校验 installed package 是否为完整 compiled runtime；如果存在 source child SKILL 而缺少 compiled 子入口，必须报错。
- `ae-sdd runtime verify` 必须把 `SKILL.md`、`runtime/**`、`skills/**/*.md` 都纳入幂等快照。

## 9. Runtime Stats 架构

运行时统计 P0 已落地；性能优化的阶段性方案归档在 [`plans/2026-07-02-runtime-stats-performance-plan.md`](plans/2026-07-02-runtime-stats-performance-plan.md)。

> 🆕 2026-07-03(B2/B3)：
> - **P1 lazy import 未实施**：plan 文档 P1 声称将 CLI 顶层 import 改 lazy import 以降 bootstrap 固定成本（实测 ~186ms），至今未落地。`perf doctor` 在 avg>150ms 时会提示该挂账。源 SKILL 瘦身（降 Agent token，已落地）与本项（降进程 ms，未落地）是两套不同成本层，勿混。
> - **scale 维度已加入**：runtime_stats 事件现记录 `scale` 字段（从项目 state.json 探测），`summarize_events` 输出 `byScale` 分桶与 `scaleRatios`（微/小/中 vs 大 的平均开销比），用于诊断"微任务 vs 大任务开销比例失调"。

| 模块 | 职责 |
| --- | --- |
| `crates/ae-sdd-integrations/src/jobs/perf.rs` | `perf.report` / `perf.doctor` / `perf.clear` 三个 job entrypoint：读取 JSONL、慢点汇总、`byScale` 分桶与 `scaleRatios` |
| `crates/ae-sdd-runtime/src/service_jobs.rs` | job 提交、授权与有界调度 |
| `crates/ae-sdd-gates/src/evaluator.rs` | 为每个 gate 记录 span，输出 `durationMs` 与 `slowest` |
| `crates/ae-sdd-build/src/benchmark.rs` | release profile 下的 Hook 延迟基准（p50/p95/p99/max/CPU/RSS） |

统计存储与输出规则：

- 项目内运行写入 `.ae-sdd/runtime-stats/YYYY-MM-DD.jsonl`；无项目 `.ae-sdd/` 时写入系统临时目录 `ae-sdd/runtime-stats/`。
- 测试和临时环境可用 `AE_SDD_STATS_DIR=<dir>` 改写存储目录；`AE_SDD_STATS=0` 可关闭统计。
- 统计不得污染业务 stdout；`--json` 业务输出保持可解析。查询统计必须显式调用 `ae-sdd perf report --json`。
- `perf clear` 清理当前统计文件并抑制自身 command event，避免刚清理又写入一条 clear 记录。
- 子进程调用统一走 `ae-sdd-integrations::command`，强制 UTF-8 解码、deadline、输出字节上限、环境变量 allowlist 与 process-tree 清理。
- CLI 入口在导入业务模块前将 stdout/stderr `reconfigure(encoding="utf-8")`；回归测试会在父环境声明 `PYTHONIOENCODING=gbk` 时严格按 UTF-8 解码 gate 输出，防止 Windows 代码页回退。
- 🆕 2026-07-03(B3)：`scale` 字段由 `start_command` 内部 `_detect_scale()` 从项目 state.json 读取（无则 null，不阻断业务），写入事件顶层；`summarize_events` 的 `byScale`/`scaleRatios` 用于 `perf doctor` 比例失调诊断。

边界：Runtime Stats 只记录命令名、脱敏 argv、耗时、退出码、span 属性和 scale（任务规模），不记录业务文档正文；它用于定位慢点与比例失调，不作为硬门禁。

## 9.5 分发器注册表架构（🆕 2026-07-03 注册表模式）

分发目标由外部 JSON 注册表 + 协议枚举驱动，支持注册/注销/扫描，不在代码里硬编码目标列表。

### 注册表文件

`~/.ae-sdd/distributors.json`（用户环境态，与 plugins/ 同级）。首次运行无文件时用种子初始化（含 claude/codex/zcode/hermes/harness，harness 默认 `enabled:false` 反映无 daemon 环境）。

每条目字段：`name` / `protocol`(copytree|harness_mount) / `target_path` / `detect`(always|path_exists|cli_exists) / `detect_cli` / `enabled` / `registered_at` / `notes`。

### 协议模板（内置，数据填参构造实例）

| 协议 | 枚举值 | 适用 | 原生实现状态 |
| --- | --- | --- | --- |
| `copytree` | `DistributorProtocol::Copytree` | claude/codex/zcode/hermes 及同类 | 有。备份→复制→校验→清旧 .bak |
| `harness_mount` | `DistributorProtocol::HarnessMount` | harness 及同类 | **无原生实现**。分发时报 `UnsupportedProtocol`，必须先 disable 该条目或补原生支持 |

注册一个 Agent = 选协议 + 填 target_path/detect 参数；注销 = 注册表除名。协议为枚举值，schema 版本不匹配、重复 host、空 target、`l2GlobalFile` 缺 `l2Language` 等均 fail closed。

### CLI 管理

`ae-sdd distributor list|register|unregister|enable|disable|scan`。注销 harness：`ae-sdd distributor disable harness`（软注销，保留条目可恢复）或 `unregister harness`（硬注销，删条目）。`scan` 扫描 `~/.*/skills/` 识别已安装 Agent 并建议注册命令，不越权委托 Agent 安装。

### 数据流

```
~/.ae-sdd/distributors.json (enabled + detect 过滤)
  → ae-sdd-build::distributor_registry 解析并校验条目
  → ae-sdd-build post-commit: distribute 遍历已解析 host
  → copytree: 复制编译后 dist 到 target_path
  → harness_mount: 无原生实现，报 UnsupportedProtocol
```

六个 CLI 动词由 `crates/ae-sdd-build/src/offline/distributor.rs` 以 offline kernel 实现（`list`/`scan`/`register`/`unregister`/`set_enabled`），不经 daemon job——注册表是用户环境态，不属于项目 state。

### 边界

注册表只管"分发到哪、用什么协议"，不管编译（编译在 `ae-sdd-build post-commit` 的 compile 阶段，分发前的硬约束保留）。注册表是用户环境态，不进 git；母版不预置注册表，首次运行种子生成。

## 10. ae-sdd Monitor 架构

Monitor 是本仓库下的独立桌面应用，位置为 `apps/ae-sdd-monitor/`。它读取项目侧 ae-sdd 状态文件并做 UI 投影，不进入 `dist/ae-sdd/` runtime 编译链，也不作为 Agent skill 分发内容。

| 模块 | 职责 |
| --- | --- |
| `apps/ae-sdd-monitor/src/main.js` | Electron 主进程、窗口生命周期、目录选择、路径打开 IPC、UI 偏好读写、父目录文件 watcher |
| `apps/ae-sdd-monitor/src/preload.js` | 只暴露受控 `monitorApi` 与 watcher 事件订阅，隔离 renderer 与 Node 能力 |
| `apps/ae-sdd-monitor/src/workspace.js` | 扫描父目录、识别 `.ae-sdd/` 工作区、读取 state/config/runtime-stats/memory、派生展示状态、阶段轴、workItemKey 身份、任务列表和活跃任务 |
| `apps/ae-sdd-monitor/renderer/src/App.tsx` | React + TypeScript renderer；左侧项目/任务两级 keyed 列表、筛选、右侧详情 Tab、本地 UI 状态、响应式静默刷新、任务级局部更新、目录选择反馈、交互动效触发和偏好恢复 |
| `apps/ae-sdd-monitor/renderer/src/main.tsx` / `renderer/index.html` | Vite renderer 入口，构建到 `dist/renderer/` 后由 Electron 主进程加载 |
| `apps/ae-sdd-monitor/src/styles.css` | 黑白圆角类 Mac 外观、iOS 风格轻量交互动效、折叠/切换/按压反馈和 reduced-motion 降级 |
| `apps/ae-sdd-monitor/test/workspace.test.js` | 扫描、YAML 读取、work item、Memory、Runtime Stats 聚合的契约测试 |
| `apps/ae-sdd-monitor/scripts/package-win.ps1` | Windows 本地打包、安装 zip、自解压 setup 生成 |
| `apps/ae-sdd-monitor/scripts/package-mac.sh` | macOS 本地打包入口，调用 electron-builder 生成 dmg/zip |
| `apps/ae-sdd-monitor/scripts/package-mac-unsigned.ps1` | 跨平台生成未签名 macOS `.app.zip`，基于 Electron darwin runtime 注入 app 资源 |
| Electron userData `preferences.json` | 保存上次父目录、选中工作区、选中任务、自动刷新开关和主题；不写项目侧 `.ae-sdd/` |

数据流：

```text
用户选择父目录
  -> Electron dialog 返回 rootPath
  -> main.js 保存/读取 userData/preferences.json
  -> workspace.js 递归扫描包含 .ae-sdd/ 的目录
  -> 读取 .ae-sdd/config.yaml / .ae-sdd/state.json
  -> 读取 .auto-engineering/{workItemKey}/state.json
  -> 读取 .ae-sdd/memory/**/*.jsonl / .ae-sdd/memory/.stage/*.json
  -> 读取 .ae-sdd/runtime-stats/*.jsonl
  -> workspace.js 派生 phaseTimeline / activeWorkItems / tasks / memory
  -> React renderer 展示项目/任务两级列表、阶段轴、Memory、事件流、活跃任务和详情
  -> main.js 监听 .ae-sdd/ 与 .auto-engineering/ 文件变化并通过 preload 通知 renderer
  -> React renderer 依靠稳定 key 和 props diff 更新对应组件；同一项目任务切换只更新右侧数据片段和侧边栏选中态
  -> styles.css 提供只作用于 UI 的折叠、切换、按压和悬浮动效
```

边界：

- Monitor 全程只读；扫描、刷新、切换 Tab 不得写项目文件。
- Monitor 的状态枚举是 UI 派生值，不新增 ae-sdd state schema。
- Monitor 的偏好文件只保存用户界面上下文，不保存 ae-sdd 业务状态。
- 响应式刷新采用 main 侧 `fs.watch` + renderer 侧 debounce：`.ae-sdd/` 与 `.auto-engineering/` 变化触发静默刷新；低频轮询只作为 watcher 漏事件兜底；不使用会改变项目状态的命令。
- renderer 不得在任务切换或静默刷新时先把 `detail` 置空再整页重画；React 组件不得用整块 `innerHTML` 替换侧边栏/详情页，只有首次加载或无详情态才显示空态。
- 交互动效只在 renderer/CSS 层表达本地 UI 反馈；不得触发 ae-sdd 命令、不得写 `.ae-sdd/`、不得成为状态权威。
- `PHASE_FLOWS`、state 字段、Memory JSONL/stage 字段、Runtime Stats JSONL 字段变化时，必须同步 [`ae-sdd-monitor-design.md`](ae-sdd-monitor-design.md)、`workspace.js` 和测试。
- Monitor 不运行 `ae-sdd gates check`，不替代 CLI/gate 的硬判断；最多展示已有 state/runtime 线索。
- Mac `.dmg`/签名最终构建必须在 macOS runner 上完成；Windows runner 可生成 Windows setup exe/zip 和未签名 macOS `.app.zip`。
- `source/standards/update-graph.json:UG-22` 负责把 ae-sdd 设计/实现/state/runtime 变化级联到 Monitor 文档、解析器、测试和 README。

## 11. 设计-实现对齐闭环

| 防线 | 工具 | 覆盖 | 当前实现度 |
| --- | --- | --- | --- |
| 快速同步检查 | `ae-sdd update-check` | `update-graph.json` 的 UG 依赖规则查询与 UC 语义检查 | **部分**。`--affected` 查询与 UC-01（版本一致性）可用；其余 UC 返回 `pass:false` + "not yet registered"，故不带 `--only` 的全量运行恒为 FAIL |
| 迭代检查 | `ae-sdd iteration-check` | IC-1~4：过时描述、路由清单缺失、未接入模块粗筛 | 完整 |
| Runtime 校验 | `ae-sdd runtime verify` | compiled runtime manifest、load_order、全量子 SKILL compiled entry、`SKILL.md` + `runtime/**` + `skills/**/*.md` 幂等输出 | 完整 |
| 兼容盘点审计 | `ae-sdd-build compatibility-audit` | 113 command / 23 operation / 36 Gate / 7 scanner 三源计数与 owner/evidence 映射 | 完整 |
| 单元与契约测试 | `cargo test --workspace` | 代码契约 | 完整 |

原 `alignment_audit.py`（UC-08~13 深度对齐）随 Python 树一并删除，其审计对象（Python 门禁承诺、argparse 命令注册）已不存在。这部分覆盖面目前是空缺，不是由上表任一工具承接。

重大实现变更流程：

```text
1. 查 ae-sdd-design.md 确认能力语义
2. 查本文件确认代码层落点
3. ae-sdd update-check --affected <files>     # UG 依赖查询，可用
4. 修改代码/文档/测试
5. ae-sdd update-check --only UC-01           # 全量运行恒 FAIL，见上表
6. cargo test -p <受影响 crate>
7. cargo fmt --all --check && cargo clippy --workspace -- -D warnings
8. 写 source/CHANGELOG
```

## 12. 新实现设计写入规则

每个系统级设计必须先在主设计文档的 Design Ledger 记录“要解决的问题、核心决策、预期价值、验证证据和版本状态”。实现架构文档只记录当前模块边界和数据流，不重复叙述设计动机。任何时候都不写 changelog；维护者通过 `ae-sdd update-check --affected` 查询 UG-28，并运行 UC-20，机器测试与运行时验证提供迭代证据。

| 内容 | 写入位置 |
| --- | --- |
| 能力为什么存在、用户语义、流程边界 | `source/docs/ae-sdd-design.md` 对应章节 + §0 Design Ledger |
| 设计 ID、问题、预期价值、验证证据、版本状态 | `source/docs/ae-sdd-design.md` §0 Design Ledger |
| 模块分层、文件职责、数据流、缓存、子进程、hook、build/distribute | 本文件 |
| 单次技术方案、阶段性取舍、性能基线 | `source/docs/plans/*.md` |
| 当前设计事实与预期价值 | `source/docs/ae-sdd-design.md` Design Ledger |
| 模块边界与数据流 | `source/docs/ae-sdd-implementation-architecture.md` |
| 可执行验证结果 | 测试输出、evidence manifest、runtime verify |
| Agent 执行入口和路由 | `source/SKILL.md` |
| 阶段内具体规则 | 对应 `source/skills/**/**-skill.md` |
| 机器可读依赖闭环 | `source/standards/update-graph.json` |
