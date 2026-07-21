---
name: http-scenario-strategy
description: 从接口能力、状态模型、独立观察面、不变量和失败机制推导最小有效 HTTP 场景，不提供固定 CRUD 用例清单。
---

# HTTP 场景推导标准

## 目标

HTTP 测试证明使用者可观察到的行为和状态关系，而不是证明某个内部方法被调用。`create→read`、`update→read` 等只是能力模型命中后的实例，不是通用模板。

## 推导链

每个接口/API AC 必须形成可审计记录：

`能力契约 → 前后状态 → 独立观察面 → 变化维度/不变量 → 扰动轴 → 独立失败机制 → 最小场景集合`

1. 从契约识别 `command`、`query`、`state-machine`、`batch`、`async`、`file`、`auth`、`idempotent`、`concurrent` 等能力；不能因 HTTP method 是 POST/PUT 就自动套 CRUD。
2. 声明可达前态、合法后态、禁止后态、变化维度和保持不变量。
3. 观察面必须与动作实现路径独立，优先公开查询、列表、任务状态、事件、下载内容或只读持久化观察。
4. 从字段语义、身份/租户、顺序、重复、并发、时间、边界和依赖失败中选择能触发独立失败机制的扰动。
5. 每条场景写明 `rationale` 和 `detects`；相同失败机制合并，无独立检错价值的场景删除。

## 能力到场景的典型映射

| 能力 | 重点证明 | 可能的独立观察 |
| --- | --- | --- |
| query | 过滤、排序、分页、投影和跨视图关系 | list/detail/聚合/计数 |
| state-machine | 合法/非法迁移、守恒、不变量、重复和并发 | 状态查询、事件、审计记录 |
| batch | 部分失败、原子性、数量/金额守恒 | 批状态、逐项结果、只读汇总 |
| async | 推进、最终一致性、重试、乱序和重复消费 | task status、事件、最终查询 |
| auth | actor、资源归属、租户隔离和拒绝后无副作用 | 另一 actor/tenant 的公开读取 |
| idempotent | 重放、超时重试、key 冲突和单次副作用 | 结果查询、计数、审计事件 |
| file | 内容、长度、摘要、元数据和重复上传 | 下载内容、checksum、metadata |

## G-HTTP-1 阻断

阻断缺少推导链、能力未覆盖、无独立观察面、只有状态码/非空断言、无 `rationale`/`detects`、相同失败机制重复、无 command/isolation/cleanup，或内部 mock 代替 HTTP/持久化主链的场景。

新 HTTP executionPlan 必须声明 `scenarioPolicyVersion: 1`，每个 `boundary=http` verification 必须用项目内相对路径引用 `scenarioManifest`。无版本字段只用于读取历史计划，不是新计划的豁免入口。

## 单元测试边界

纯逻辑、格式转换和明确外部故障注入可使用少量 mock；不能关闭接口、数据库、事务、状态迁移、权限或持久化 AC。
