# 需求规格说明书：{需求名称}

> 本模板是 RA 的**唯一产物**——一份随需求规模自适应的 SRS。
> 同一模板覆盖 micro/small/medium/large：条件章节只由 §3 的 applicability 激活；
> 规模是分析输出（§7），不是模板 profile，不改变文档种类，也不引入按规模计算的字数/表格数/章节数门槛。
>
> **填写原则：** 只展开 `applicable` 章节；`not_applicable` 只在 §3 留有依据的判定，不生成空章节；
> 关键 `unknown` 必须通过 GAP 关闭，否则 `analysisState` 保持 `draft`。
> 用户批准由 daemon 的 confirmation receipt 持有，**不回写 SRS 正文**。

## 0. 文档与需求身份

| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-{ID} |
| Work Item | {WORK-ITEM-ID} |
| Revision | {N} |
| Analysis state | draft / complete |
| Scale | micro / small / medium / large |
| Scale confidence | 0-100 |

### 0.1 来源与实际使用的上下文

| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | {PRD/Issue/对话/现有系统事实} | {引用或摘要} | {digest/版本/行号} | {用途} |

> 上下文全局选填；当 SRS 声称某个现有系统事实时，对应 REF 必需。

## 1. 问题、目标与非目标

{问题：当前痛点是什么}
{目标：要达成什么}
{非目标：明确不做什么}

## 2. 范围

- **In Scope**：{...}
- **Out of Scope**：{...}

## 3. 适用性判定

> 对七个条件维度逐项判定。只有 `applicable` 生成对应条件章节（§8.x）；
> `not_applicable` 只在此表留有依据的判定，不生成空章节；
> `unknown` 若影响范围/验收/风险/规模，必须创建阻断性 GAP-*。

| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | applicable / not_applicable / unknown | {依据} | §8.1 / reason / GAP-* |
| scenarios | applicable / not_applicable / unknown | {依据} | §8.2 / reason / GAP-* |
| state_lifecycle | applicable / not_applicable / unknown | {依据} | §8.3 / reason / GAP-* |
| data_semantics | applicable / not_applicable / unknown | {依据} | §8.4 / reason / GAP-* |
| external_contracts | applicable / not_applicable / unknown | {依据} | §8.5 / reason / GAP-* |
| quality_security_compliance | applicable / not_applicable / unknown | {依据} | §8.6 / reason / GAP-* |
| compatibility_migration_operations | applicable / not_applicable / unknown | {依据} | §8.7 / reason / GAP-* |

## 4. 需求清单

> 规范性需求使用 REQ-*，每条至少绑定一个 REF-*（来源可追溯）。

| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | {规范性需求} | P0/P1/P2 | REF-001 | {依赖/冲突} |

## 5. 验收与追溯

> 每条 REQ-* 至少被一个 AC-* 覆盖。AC 类型允许 example / property / invariant / compatibility / operational，不强制 Given-When-Then。

| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | {example/property/invariant/compatibility/operational} | {可执行/可观察判定} |

## 6. 约束、假设、冲突、风险与未决

| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| C-001 | 约束 | {内容} | 高/中/低 | {处置} |
| A-001 | 假设 | {内容} | 高/中/低 | {验证方式} |
| R-001 | 风险 | {内容} | 高/中/低 | {缓解} |
| GAP-001 | gap | {阻断性未决} | 🔴 阻断 | {关闭方式} |

> 影响范围/验收/风险/规模的阻断性 GAP 必须关闭，否则 analysisState 保持 draft。

## 7. 规模裁定

> 纯需求六维评分，最终 scale 取**最高分**。评分不得引用文件/类数量、预计人天、数据库表、中间件选型或测试实现层级。
> 证据不足时降低 confidence，不得把 unknown 当作"无影响"。

| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | {1/2/3/4} | {证据} |
| 参与方、权限或业务域广度 | {1/2/3/4} | {证据} |
| 状态、数据语义与不变量复杂度 | {1/2/3/4} | {证据} |
| 外部契约与协调范围 | {1/2/3/4} | {证据} |
| 性能、安全、合规、可用性等质量风险 | {1/2/3/4} | {证据} |
| 兼容、迁移、回滚和运行影响 | {1/2/3/4} | {证据} |

定义：1=micro、2=small、3=medium、4=large。

最高分 = {max} -> Scale = {micro/small/medium/large}。

> §0 header 的 Scale 必须等于此最高分对应档位（G-RA-4 校验一致性）。

---

## 条件章节（仅 applicable 生成）

> 以下章节只在 §3 对应维度为 applicable 时生成；不生成空 N/A 章节。

### 8.1 参与方、权限与职责

（仅 participants=applicable 时生成）

### 8.2 场景与交互

（仅 scenarios=applicable 时生成）

### 8.3 状态、生命周期与不变量

（仅 state_lifecycle=applicable 时生成）

### 8.4 数据与信息语义

（仅 data_semantics=applicable 时生成）

### 8.5 外部行为契约与依赖

（仅 external_contracts=applicable 时生成）

### 8.6 质量属性、安全与合规

（仅 quality_security_compliance=applicable 时生成）

### 8.7 兼容、迁移与运行约束

（仅 compatibility_migration_operations=applicable 时生成）

---

> **下一步：** `analysisState=complete` 且 RA SeriesReceipt 已收集后，进入 RequirementAnalyzed [G-RA-1 -> G-RA-2 -> G-RA-3 -> G-RA-4]；RouteEngine 生成 route candidate；用户一次批准后冻结 EngineeringRoute。
