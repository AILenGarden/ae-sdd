---
docId: EVALUATION-TEST-TOOL-DEMO
docType: evaluation
category: demo
title: ae-sdd 能力测试评分卡 — test-tool 综合能力库
purpose: 四维度评分，量化 ae-sdd 在 RA→DR→Story→CodingPlan→Coding→Test→Review 完整长链路上的执行表现，并量化模型在算法/trait/泛型/serde/API 五个能力维度上的上限
scope: 评估操作员手册，ae-sdd 不应修改本文件
---

# ae-sdd 能力测试评分卡 — test-tool 综合能力库

> 配套需求：[`../../ae-sdd-doc/RA/RA-AE-SDD-CAPABILITY-TEST-TEST-TOOL.md`](../../ae-sdd-doc/RA/RA-AE-SDD-CAPABILITY-TEST-TEST-TOOL.md)
> 指标 schema：[`metrics.schema.json`](./metrics.schema.json)

---

## 0. 使用说明

1. **谁用**：操作员（跑 ae-sdd 测试的人）。
2. **何时用**：一次完整的 ae-sdd 流程跑完后（从 `/ae-sdd` 触发到 Review 完成）。
3. **怎么用**：
   - 按 §2~§5 四维度逐项打分，证据来源必须真实存在（文件路径 + sha256 或命令输出）。
   - 把分数汇总到 §6 总分表。
   - 按 `metrics.schema.json` 落地一份 `metrics-<STORY-ID>-run<N>.json`。
4. **红线**：维度 A 中标 🔴 的检查项，任一违反 → 本次运行判为「不合规」，维度 A 直接 0 分，不再计算其他维度（但仍记录指标供分析）。
5. **分层评分**：维度 C 拆成「必修层（§4.1~§4.4，满分 80）」和「选修层（§4.5，加分项最高 +30）」。选修层加分让维度 C 可超 100，但综合总分以 100 为上限。

---

## 1. 评分权重总览

| 维度 | 权重 | 说明 | 满分 |
| ---- | ---- | ---- | ---- |
| A. 流程合规性 | 35% | 是否真按 ae-sdd 规矩走 | 100 |
| B. 能力项覆盖度 | 25% | 各能力项是否被真实调用 | 100 |
| C. 完成度与质量 | 25% | 必修 AC 全绿 + 选修 AC 加分 + 边界覆盖 + clippy/fmt/无 unsafe | 80 + 30 加分 |
| D. 效率指标 | 15% | 总耗时 / token / 轮次（纯记录 + 时长分档） | 100 |

**综合总分** = `A×0.35 + min(C, 100)×0.25 + B×0.25 + D×0.15`（满分 100）

> 选修层（§4.5）的作用是把维度 C 推到 100+，从而拉高综合总分。它在维度 C 内部超过 100 的部分不计入综合分，但单独记录为「能力上限分」供横向对比模型能力。

---

## 2. 维度 A：流程合规性（权重 35%，满分 100）

> 验证 ae-sdd 是否真的按 RA→DR→Story→CodingPlan→[批准]→Coding→Test→Review 走。
> 每个检查项要给出**证据**（文件路径 + 关键字段值或 sha256），不能凭印象打勾。

### 2.1 检查项表

| # | 检查项 | 证据来源 | 分值 | 评分规则 |
| - | ------ | -------- | ---- | -------- |
| A1 🔴 | **Route 真发生** | `.auto-engineering/<story>/state.json` 的 `routeDecision{}` 非空 + `history[]` 含 `route-selected` 条目 | 8 | 缺任一 → 维度 A 0 分（红线） |
| A2 | 规模判定为「中」 | `state.json` 的 `scale=="中"` + phase 链含约 10 个节点 | 6 | 误判为小/微扣 50%；误判为大扣 25% |
| A3 | RA 文档生成 | `ae-sdd-doc/RA/<RA-ID>.md` 存在 + 记录 sha256 | 8 | 缺 → 0 分 |
| A4（加分） | DR 文档生成 | `ae-sdd-doc/DR/<DR-ID>.md` 存在 + 含算法选型 + 分派选型决策 | 6（加分项，可超 100） | 缺不扣分但有则加分 |
| A5 | Story 文档生成 | `ae-sdd-doc/Story/<STORY-ID>.md` 存在 + 含接口契约表/字段表/AC | 8 | 缺 → 0 分；缺接口契约表扣 50% |
| A6 🔴 | **第一个 Edit 落在 .md 不落 .rs**（契约变更类红线） | `git log --diff-filter=A --format=%ci -- demo/test-tool/` 首个 `.rs` 文件创建时间 > Story `.md` 创建时间 | 10 | 违反 → 维度 A 0 分（红线） |
| A7 | CodingPlan 写入 `state.executionPlan` | `state.json` 的 `executionPlan{}` 非空 + 含 `changedPaths` / `verification` / `risks` | 8 | 缺 → 0 分；缺任一字段扣 30% |
| A8 🔴 | **用户真的批准了 executionPlan** | `executionPlan.approved==true` + `approvedBy` 以 `user:` 开头 + `history[]` 有 `execution-plan-approved` 且 `by` 以 `user:` 开头 | 10 | 未批准 → 维度 A 0 分（红线） |
| A9 🔴 | **Coding 前三 gate fresh PASS** | `.auto-engineering/<story>/evidence/` 含 `g-codeplan-src` / `g-14` / `g-08` 三 gate 的 fresh PASS 记录 | 10 | 任一缺 → 维度 A 0 分（红线） |
| A10 | Coding 真实性（G-CODE-1） | evidence/ 含 `g-code-1` PASS | 6 | 缺 → 扣 6 分 |
| A11 🔴 | **Test evidence 真实** | `.auto-engineering/<story>/evidence/manifest.json` 存在 + `exitCode==0` + 含 `cargo test` 输出 snapshot（非 `cargo build`） | 10 | 缺/伪造 → 维度 A 0 分（红线） |
| A12 | Review findings 落地 | `state.json` 的 `reviewSession.findings[]` 非空 或 `ae-sdd-doc/CR/<STORY-ID>/<STORY-ID>-CodeReview.md` 存在 + 含问题分级 | 8 | 缺 → 扣 8 分；缺分级扣 50% |
| A13 | 不写禁用文档 | 未新写 Proposal / CodingReport / TestReport / ChangeLog / STORING.md（v3.12 fail closed） | 6 | 每违反一项扣 2 分 |
| A14 | 无流程外 mutation | `state.json` 的 `history[]` 每条 transition 都有合法 `by` 字段（`ae-sdd` / `ae-sdd state write` / `user:...`） | 6 | 每条非法 by 扣 1 分 |

### 2.2 维度 A 小计

```
A_raw = Σ(各项得分)   // 含 A4 加分可超 100
A_total = min(A_raw, 100)
```

---

## 3. 维度 B：能力项覆盖度（权重 25%，满分 100）

> 验证 ae-sdd 各能力项是否真的被调用。每项打勾 = 满分，未触发 = 0 分。

### 3.1 能力项清单

| # | 能力项 | 触发证据 | 分值 |
| - | ------ | -------- | ---- |
| B1 | 路由 classify（自动判定 scale） | `state.json` 含 `scale` 字段 + `routeDecision.reason` 体现 classify 推理 | 8 |
| B2 | 约束加载 `get_constraints(projectKey)` | evidence/ 或 state 含 `LoadedContextProof.constraints_ref`；或 ae-sdd 输出提到读取了 `constraints/` 下文件 | 8 |
| B3 | RA 8 维度齐全 | RA 文档含 8 个核心维度（背景/目标/边界/AC/NFR/风险/依赖/优先级 等） | 10 |
| B4 | DR 架构决策（含分派选型） | DR 文档含 ≥2 个架构决策点（算法选型 + 动态/静态分派选型） | 10 |
| B5 | Story 接口契约表 | Story 含字段表（字段名/类型/约束/wire schema） | 10 |
| B6 | AC ↔ TC 追溯矩阵 | Story 或 TestCase 含 AC ID ↔ TC ID 映射表 | 8 |
| B7 | executionPlan 结构完整 | `state.executionPlan` 含 `changedPaths` / `verification` / `risks` / `sourceReads` | 8 |
| B8 | RED-GREEN-REFACTOR | evidence 含先写失败测试（RED）→ 实现（GREEN）→ 重构（REFACTOR）的时序证据 | 8 |
| B9 | `cargo fmt --check` 通过 | 在 `demo/test-tool/` 跑命令通过 | 6 |
| B10 | `cargo clippy -- -D warnings` 零告警 | 在 `demo/test-tool/` 跑命令零告警 | 8 |
| B11 | focused tests + workspace regression | evidence 含 `cargo test -p test-tool` + `cargo test --workspace` 两份输出（或显式说明 demo 不进 workspace） | 8 |
| B12 | evidence snapshot 存在 | `.auto-engineering/<story>/evidence/artifacts/<sha256>-<name>` 存在且 sha256 匹配 | 8 |

### 3.2 维度 B 小计

```
B_total = Σ(各项得分)   // ≤100
```

---

## 4. 维度 C：完成度与质量（权重 25%，必修 80 + 选修加分最高 30）

> 验证交付物的实际质量。直接在 `demo/test-tool/` 跑命令验证。
> 必修层 + 边界 + 契约 + 工程质量 = 80 分；选修 4 组能力压测最高 +30 加分。

### 4.1 必修 AC（共 40 分）

> 在 `demo/test-tool/` 跑 `cargo test`，逐条核对 AC-1~AC-10（AC-10 为加分）。

| AC | 描述 | 星度 | 分值 | 通过 |
| -- | ---- | ---- | ---- | ---- |
| AC-1 | 平地 5×5 BFS 最短跳数（path.len()==9, cost==8000） | ★ | 3 | ☐ |
| AC-2 | 含障碍 Dijkstra 绕行最小代价 | ★★ | 4 | ☐ |
| AC-3 | 沼泽/公路 A\* 代价优于 BFS | ★★★ | 5 | ☐ |
| AC-4 | 不连通返回 `found:false`（非 Err） | ★★ | 4 | ☐ |
| AC-5 | start==goal 返回 `StartEqualsGoal` | ★ | 3 | ☐ |
| AC-6 | start/goal 落 Obstacle 返回对应错误 | ★★ | 4 | ☐ |
| AC-7 | 超 `max_nodes_expanded` 返回带上下文的 `NodeLimitExceeded { expanded, limit }` | ★★★ | 5 | ☐ |
| AC-8 | 八连通 vs 四连通路径不同（或显式 UnsupportedAlgorithm） | ★★★ | 4 | ☐ |
| AC-9 | A\* 展开节点数 ≤ Dijkstra（3 张地图，含严格 <） | ★★★★ | 6 | ☐ |
| AC-10（加分） | 路径确定性（两次 `==` + 字节相等） | ★★★ | 2（加分） | ☐ |

**必修 AC 小计**：`min(Σ AC_必修, 38)` + AC-10 加分（最高 +2，可让此项达 40）

### 4.2 边界情况覆盖（15 分）

> 检查测试代码是否覆盖 RA §6 的 12 条边界。每条 1.25 分。

| 边界 # | 场景 | 覆盖 | 边界 # | 场景 | 覆盖 |
| ------- | ---- | ---- | ------- | ---- | ---- |
| 1 | 1×1 地图 | ☐ | 7 | 全 Obstacle | ☐ |
| 2 | 同坐标不同 instance | ☐ | 8 | 全 Road（零代价环路） | ☐ |
| 3 | cells 空 + width=0 | ☐ | 9 | max_nodes=0（不限） | ☐ |
| 4 | cells.len ≠ w×h | ☐ | 10 | max_nodes=usize::MAX | ☐ |
| 5 | start 越界 | ☐ | 11 | u16 累加溢出 | ☐ |
| 6 | goal 越界 | ☐ | 12 | 八连通穿角策略 | ☐ |

### 4.3 接口契约对齐（15 分）

> 对比 `demo/test-tool/src/contracts.rs`（或等价文件）与 RA §2 的领域模型，逐项核对。

| 检查 | 分值 | 评分 |
| ---- | ---- | ---- |
| 所有 struct/enum 名称与 RA §2 一致（`GridMap` / `Cell` / `TerrainType` / `Position` / `Algorithm` / `Connectivity` / `PathRequest` / `PathResult` / `PathError`） | 5 | |
| 字段名/类型 100% 对齐（`width: u32` / `total_cost_permille: u64` 等） | 5 | 每处偏差扣 1 |
| wire schema 齐全（`#[serde(rename_all="camelCase", deny_unknown_fields)]` + enum `kebab-case`） | 5 | 缺一项扣 2 |

### 4.4 工程质量（10 分）

| 检查 | 命令 | 分值 | 评分 |
| ---- | ---- | ---- | ---- |
| `cargo fmt --check` 通过 | `cd demo/test-tool && cargo fmt --check` | 2 | |
| `cargo clippy -- -D warnings` 零告警 | `cd demo/test-tool && cargo clippy --all-targets -- -D warnings` | 3 | |
| 无 `unsafe` / `unwrap` / `expect` / `todo!` / `unimplemented!` / `panic!`（非 test 代码） | `grep -rnE "unsafe|unwrap|expect|todo!|unimplemented!|panic!" src/` | 2 | 每处扣 0.5 |
| Review findings 无 BLOCKER / MAJOR（仅有 MINOR / INFO） | `state.json` reviewSession 或 CR 文档 | 3 | BLOCKER 每个 −2，MAJOR 每个 −1 |

### 4.5 选修能力压测（加分项，最高 +30）

> 每组选修含 2 条 AC。**完成**（=两条 AC 都 PASS）该组才能拿该组分；只完成一条 = 0 分（防半吊子）。
> 放弃某组要在 Story 显式 `deferred: true` + 理由；不声明又不实现 = 0 分且倒扣该组 1 分（测自我认知）。

#### 4.5.1 trait 抽象 + 多态（最高 +8 分）

| AC | 描述 | 星度 | 分值 | 通过 |
| -- | ---- | ---- | ---- | ---- |
| AC-11 | Pathfinder trait + 三算法 impl + `select()` 工厂 | ★★★ | 3 | ☐ |
| AC-12 | 动态（`Box<dyn>`）vs 静态（泛型）分派语义对等 | ★★★★ | 5 | ☐ |

**计分规则**：AC-11 与 AC-12 都 PASS = +8 分；只 PASS 一个 = 0 分。

#### 4.5.2 泛型设计（最高 +8 分）

| AC | 描述 | 星度 | 分值 | 通过 |
| -- | ---- | ---- | ---- | ---- |
| AC-13 | `GenericGridMap<C>` + `Cost` trait（u16/u32/u64）+ 与定长版行为一致 | ★★★★ | 3 | ☐ |
| AC-14 | 泛型 + trait 正交：`Pathfinder::find` 在 `GenericGridMap<C>` 上工作 | ★★★★★ | 5 | ☐ |

**计分规则**：AC-13 与 AC-14 都 PASS = +8 分；只 PASS 一个 = 0 分。

#### 4.5.3 Builder + IntoIterator（最高 +7 分）

| AC | 描述 | 星度 | 分值 | 通过 |
| -- | ---- | ---- | ---- | ---- |
| AC-15 | `PathRequest::builder()` 链式构造 + `.build()` 校验 | ★★★ | 4 | ☐ |
| AC-16 | `PathResult: IntoIterator` + `iter()` + `total_hops()` | ★★★ | 3 | ☐ |

**计分规则**：AC-15 与 AC-16 都 PASS = +7 分；只 PASS 一个 = 0 分。

#### 4.5.4 复杂序列化（最高 +7 分）

| AC | 描述 | 星度 | 分值 | 通过 |
| -- | ---- | ---- | ---- | ---- |
| AC-17 | `PathResult` flatten 序列化 + round-trip + deny_unknown_fields | ★★★★ | 3 | ☐ |
| AC-18 | `Position` 序列化为 `[row, col]` 数组 + round-trip + 错误形态拒绝 | ★★★★★ | 4 | ☐ |

**计分规则**：AC-17 与 AC-18 都 PASS = +7 分；只 PASS 一个 = 0 分。

### 4.6 维度 C 小计

```
C_required = AC必修(38+2加分) + 边界(15) + 契约(15) + 工程质量(10)   // ≤80
C_elective = trait(8) + generic(8) + builder(7) + serde(7)            // ≤30
C_raw = C_required + C_elective                                       // ≤110
C_total = min(C_raw, 100)                                             // 综合分用
C_capability_ceiling = C_elective                                     // 单独记录，模型能力上限指标
```

> **能力上限分（C_capability_ceiling）**：选修层得分，0-30 分。这是横向对比模型能力的关键指标。
> - 0-7：基础模型（只能做算法实现，无抽象/泛型/serde 高级能力）
> - 8-15：中级模型（能做 trait 或 builder，但泛型/serde 困难）
> - 16-23：高级模型（trait + 泛型 + serde 基本都能）
> - 24-30：顶级模型（5 星 AC 全达成）

---

## 5. 维度 D：效率与资源指标（权重 15%，满分 100）

> 纯记录，按时长分档给分。其余指标不参与分档，仅作统计与对比用。
> 指标分 5 组：时间 / Token 与成本 / 代码产出 / 过程质量 / 派生比率。
> 派生比率由操作员或脚本自动计算（公式见 §5.6）。

### 5.1 时间指标（必填）

| 指标 | 来源 | 值 |
| ---- | ---- | -- |
| `totalDurationMinutes` | 操作员记录起止时间戳 | _____ |
| `startedAt` | ISO 8601 时间戳 | _____ |
| `finishedAt` | ISO 8601 时间戳 | _____ |
| `phaseDurations.route` | Route 阶段耗时（分钟） | _____ |
| `phaseDurations.ra` | RA 阶段耗时 | _____ |
| `phaseDurations.dr` | DR 阶段耗时（中型可选；未走填 0） | _____ |
| `phaseDurations.story` | Story 阶段耗时 | _____ |
| `phaseDurations.codingPlan` | CodingPlan 阶段耗时 | _____ |
| `phaseDurations.coding` | Coding 阶段耗时 | _____ |
| `phaseDurations.test` | Test 阶段耗时 | _____ |
| `phaseDurations.review` | Review 阶段耗时 | _____ |
| `designPhaseMinutes` | 派生：route+ra+dr+story+codingPlan（设计阶段合计） | _____ |
| `executionPhaseMinutes` | 派生：coding+test+review（执行阶段合计） | _____ |
| `designExecutionRatio` | 派生：designPhaseMinutes / executionPhaseMinutes | _____ |

### 5.2 Token 与成本指标（必填）

| 指标 | 来源 | 值 |
| ---- | ---- | -- |
| `tokens.input` | ZCode UI 或客户端日志（累计 input token） | _____ |
| `tokens.output` | 同上（累计 output token） | _____ |
| `tokens.total` | 派生：input + output | _____ |
| `tokens.cached` | 若宿主报告 prompt caching 命中（无则填 0） | _____ |
| `tokens.contextWindowPeak` | 单轮最大上下文占用（从 UI 读，无则填 0） | _____ |
| `tokens.inputOutputRatio` | 派生：output / input（反映"产出密度"） | _____ |
| `cost.inputUsd` | input token × 模型单价（USD） | _____ |
| `cost.outputUsd` | output token × 模型单价 | _____ |
| `cost.totalUsd` | 派生：inputUsd + outputUsd | _____ |
| `cost.modelName` | 本次跑用的 LLM 模型（如 glm-4.5/claude-sonnet-4） | _____ |
| `cost.inputPricePerMillion` | 模型 input 单价（USD / 1M token） | _____ |
| `cost.outputPricePerMillion` | 模型 output 单价（USD / 1M token） | _____ |

### 5.3 代码产出指标（必填，跑完后用 `tokei`/`cloc`/`git ls-files` 统计）

| 指标 | 来源 | 值 |
| ---- | ---- | -- |
| `code.filesTotal` | `git ls-files demo/test-tool/src/ \| wc -l` | _____ |
| `code.filesContracts` | contracts.rs 等 DTO/wire 文件数 | _____ |
| `code.filesAlgorithm` | algorithm/ 下文件数 | _____ |
| `code.filesElective` | pathfinder.rs/generic.rs/builder.rs/serde_impl.rs（选修产物） | _____ |
| `code.filesTests` | tests/ 下测试文件数 | _____ |
| `code.locSrc` | src/ 非空非注释行数（用 `tokei` 或 `cloc`） | _____ |
| `code.locTests` | tests/ 非空非注释行数 | _____ |
| `code.locTotal` | 派生：locSrc + locTests | _____ |
| `code.testToCodeRatio` | 派生：locTests / locSrc（理想 ≥ 1.0） | _____ |
| `code.testCasesTotal` | `cargo test -- --list` 统计的测试用例总数 | _____ |
| `code.testCasesPassed` | `cargo test` 实际通过的用例数 | _____ |
| `code.testCasesFailed` | 失败的用例数 | _____ |
| `code.testPassRate` | 派生：testCasesPassed / testCasesTotal（%） | _____ |
| `code.unsafeBlocks` | `grep -rc "unsafe" src/` 非零计数 | _____ |
| `code.unwrapCount` | `grep -rEc "\.unwrap\(\)" src/`（非 test） | _____ |
| `code.todoCount` | `grep -rc "todo!\|unimplemented!" src/` | _____ |

### 5.4 过程质量指标（必填）

| 指标 | 来源 | 值 |
| ---- | ---- | -- |
| `turnCount` | 操作员计数（一轮 user+assistant 算一轮） | _____ |
| `cliInvocations` | `.ae-sdd/runtime-stats/<date>.jsonl` 中本 story 相关行数 | _____ |
| `stateRevisionDelta` | 跑后 `state.json` revision − 跑前 revision | _____ |
| `gateBlocks.count` | runtime-stats spans 中 `allowed:false` 计数 | _____ |
| `gateBlocks.gateIds` | BLOCK 涉及的 gate ID 列表（去重） | _____ |
| `gateBlocks.avgRemediationMinutes` | 平均每次 BLOCK 到下次 PASS 的耗时（无 BLOCK 填 0） | _____ |
| `gateBlocks.totalRemediationMinutes` | 所有 BLOCK 累计补救耗时 | _____ |
| `review.attempts` | `state.json` reviewSession.counters.attempts | _____ |
| `review.validBatches` | reviewSession.counters.validBatches | _____ |
| `review.remediations` | reviewSession.counters.remediations（修复轮数） | _____ |
| `review.findingsOpened` | findings 中 status 非初始 CLOSED 的总数 | _____ |
| `review.findingsClosed` | findings 中 status=CLOSED 的总数 | _____ |
| `review.blockerCount` | findings category=BLOCKER 总数 | _____ |
| `review.majorCount` | findings category=MAJOR 总数 | _____ |
| `review.minorCount` | findings category=MINOR 总数 | _____ |
| `review.infoCount` | findings category=INFO 总数 | _____ |
| `phasesSkipped` | 实际未走的 phase 数（如中型无 DR 则 dr=1） | _____ |
| `electiveDeferred` | 选修层显式声明放弃的组数（0-4） | _____ |
| `electiveHalfDone` | 选修层"只完成一条 AC"的组数（半吊子） | _____ |

### 5.5 综合分数回填（与 §6 一致，便于横向对比）

| 指标 | 来源 | 值 |
| ---- | ---- | -- |
| `scores.A` | 维度 A 总分（红线违反为 0） | _____ |
| `scores.B` | 维度 B 总分 | _____ |
| `scores.C_required` | 维度 C 必修层得分（满分 80） | _____ |
| `scores.C_elective` | 维度 C 选修加分（满分 30）= capabilityCeiling | _____ |
| `scores.C_total` | min(C_required + C_elective, 100) | _____ |
| `scores.D` | 维度 D 时长分档（100/80/60/40） | _____ |
| `scores.total` | 综合总分（满分 100） | _____ |
| `scores.grade` | A/B/C/D/F | _____ |
| `scores.capabilityTier` | basic/intermediate/advanced/top | _____ |

### 5.6 派生比率（自动计算，便于横向对比）

| 派生指标 | 公式 | 含义 |
| -------- | ---- | ---- |
| `speed.totalScorePerMinute` | total / totalDurationMinutes | 每分钟产出综合分（越高越快） |
| `speed.requiredScorePerMinute` | C_required / totalDurationMinutes | 每分钟产出必修分 |
| `speed.locPerMinute` | locTotal / totalDurationMinutes | 每分钟代码产出（LOC） |
| `speed.acPassedPerMinute` | (AC 必修通过数) / totalDurationMinutes | 每分钟 AC 完成数 |
| `efficiency.totalScorePerMillionTokens` | total / (tokens.total / 1_000_000) | 每 100 万 token 产出综合分 |
| `efficiency.locPerMillionTokens` | locTotal / (tokens.total / 1_000_000) | 每 100 万 token 产出 LOC |
| `efficiency.requiredScorePerUsd` | C_required / cost.totalUsd | 每美元产出必修分 |
| `efficiency.totalScorePerUsd` | total / cost.totalUsd | 每美元产出综合分 |
| `efficiency.testPassRate` | testCasesPassed / testCasesTotal × 100% | 测试通过率 |
| `efficiency.blockerRatio` | review.blockerCount / max(review.findingsOpened, 1) | BLOCKER 占 findings 比例（越低越好） |
| `efficiency.remediationPerFinding` | review.remediations / max(review.findingsOpened, 1) | 每个 finding 平均修复轮数 |
| `efficiency.cachedTokenRate` | tokens.cached / tokens.total × 100% | 缓存命中率（无缓存填 0） |

### 5.7 维度 D 评分（按时长分档）

| 总耗时 | 分数 |
| ------ | ---- |
| ≤ 60 分钟 | 100 |
| 60 ~ 120 分钟 | 80 |
| 120 ~ 180 分钟 | 60 |
| > 180 分钟 | 40 |

> 注：选修层多，模型若选择全做，时长自然变长。但维度 D 的分档不变——这是设计取舍：选修分高 + 时长分低 vs 选修分低 + 时长分高，由模型自己权衡。

```
D_total = 时长分档分数
```

---

## 6. 综合总分表

```
total_score = A_total × 0.35 + B_total × 0.25 + min(C_raw, 100) × 0.25 + D_total × 0.15
```

| 维度 | 得分 | 权重 | 加权 |
| ---- | ---- | ---- | ---- |
| A 流程合规性 | _____ / 100 | 0.35 | _____ |
| B 能力项覆盖度 | _____ / 100 | 0.25 | _____ |
| C 完成度与质量（必修） | _____ / 80 |   |   |
| C 完成度与质量（选修加分） | _____ / 30 |   |   |
| C 小计（综合分用） | _____ / 100 | 0.25 | _____ |
| **C_capability_ceiling（能力上限分，单独记录）** | **_____ / 30** |   |   |
| D 效率指标 | _____ / 100 | 0.15 | _____ |
| **总分** |   |   | **_____ / 100** |

### 6.1 等级划分

| 总分 | 等级 | 含义 |
| ---- | ---- | ---- |
| ≥ 90 | A | 优秀：流程严谨、能力齐全、质量过硬、效率高 |
| 80~89 | B | 良好：基本合规，个别项有改进空间 |
| 70~79 | C | 合格：跑通主流程，但合规或质量有明显短板 |
| 60~69 | D | 不合格：存在红线违反或大面积能力缺失 |
| < 60 | F | 失败：未完成或严重违规 |

### 6.2 能力上限标签（基于 C_capability_ceiling）

| 选修分 | 标签 | 含义 |
| ------ | ---- | ---- |
| 0-7 | 基础模型 | 只能做算法实现 |
| 8-15 | 中级模型 | 能做 trait 或 builder |
| 16-23 | 高级模型 | trait + 泛型 + serde 基本都能 |
| 24-30 | 顶级模型 | 5 星 AC 全达成 |

---

## 7. 评语模板（可选）

操作员可在评分后填写主观评语：

- **亮点**：本次运行表现最好的能力项是 _____
- **短板**：本次运行最需要改进的是 _____
- **意外**：本次运行出现预期外的 _____（如 gate 反复 BLOCK、路由误判、契约漂移、选修层半吊子等）
- **能力上限判断**：本次选修分 _____，对应 _____ 模型水平。判断依据 _____
- **建议**：下次运行建议 _____
- **重跑对比**（如有）：相比 run<N-1>，本次 _____（提升/退化）了 _____

---

## 8. 多次运行对比表

| run | STORY-ID | 总分 | A | B | C | C_ceiling | D | 总耗时 | input tok | output tok | LOC 总 | 测试通过率 | 成本(USD) | 主要差异 |
| --- | -------- | ---- | -- | -- | -- | --------- | -- | ------ | --------- | ---------- | ------- | ---------- | --------- | -------- |
| 1 | STORY-DEMO-TEST-TOOL-001 |   |   |   |   |           |   |        |           |            |         |            |           | 初次基线 |
| 2 | STORY-DEMO-TEST-TOOL-002 |   |   |   |   |           |   |        |           |            |         |            |           |          |
| 3 | STORY-DEMO-TEST-TOOL-003 |   |   |   |   |           |   |        |           |            |         |            |           |          |

> 用途：判断 ae-sdd 在同一需求上的执行稳定性与改进趋势，同时横向对比不同模型/版本的能力上限（看 C_ceiling）与单位资源产出（看 input/output tok、LOC、成本三列）。

### 8.1 关键派生指标对比表

> 上述多次 run 的派生比率横向对比（公式见 §5.6）。理想情况下同一模型/版本多次 run 的派生指标方差应较小。

| run | 分/分钟 | 分/百万 tok | 分/美元 | LOC/分钟 | AC 通过/分钟 | 测试通过率 | BLOCKER 占比 | 缓存命中率 |
| --- | ------- | ----------- | ------- | -------- | ------------ | ---------- | ------------ | ---------- |
| 1 |   |   |   |   |   |   |   |   |
| 2 |   |   |   |   |   |   |   |   |
| 3 |   |   |   |   |   |   |   |   |

> 派生指标的含义：
> - **分/分钟 / 分/百万 tok / 分/美元**：单位资源产出的综合分，越高越高效。
> - **LOC/分钟**：代码产出速度。
> - **AC 通过/分钟**：有效交付速度（比 LOC/分钟更真实，避免堆代码）。
> - **BLOCKER 占比**：Review findings 中 BLOCKER 比例，越低越好。
> - **缓存命中率**：prompt cache 命中 token 占总 token 比例（无缓存填 0）。
