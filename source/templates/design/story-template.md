# STORY-{number}-{title}

**总章节目录**

- 分析： [用户故事](#user-story) · [范围](#scope) · [前置条件](#prerequisites) · [涉及工程](#affected-projects) · [触发入口](#trigger-entry) · [主流程](#main-flow) · [异常流程](#exception-flow) · [依赖与风险](#dependencies-risks)
- 设计： [接口契约-SPI](#interface-contract-spi) · [接口契约-REST](#interface-contract-rest) · [状态流转总览](#state-transition-overview) · [错误码](#error-codes) · [数据模型](#data-model) · [配置变更](#configuration-changes) · [非功能设计](#non-functional-design)
- 实现： [实现设计](#implementation-design) · [验收标准](#acceptance-criteria)
- 补充： [元信息](#metadata) · [实现任务映射](#implementation-task-mapping) · [人工任务](#manual-tasks) · [未决问题](#open-questions)

**接口目录**

| 类型 | 编号 | 接口 / 签名 | 详情 |
| --- | --- | --- | --- |
| SPI | SPI-1 | {接口签名} | [跳转](#spi-1) |
| REST | REST-1 | {METHOD} {/path} | [跳转](#rest-1) |

<a id="metadata"></a>
<!-- ae-sdd:story-section id=metadata layer=secondary -->
## 元信息

| 字段 | 值 |
| --- | --- |
| Story ID | {STORY-ID} |
| 标题 | {标题} |
| 类型 | {类型} |
| 规模 | {规模} |
| 关联 PRD / RA / DR | {文档引用} |
| 负责人 | {负责人} |

### 核心设计 · 分析

<a id="user-story"></a>
<!-- ae-sdd:story-section id=user-story layer=primary -->
## 用户故事

作为 {角色}，我希望 {能力}，以便 {价值}。

<a id="scope"></a>
<!-- ae-sdd:story-section id=scope layer=primary -->
## 范围

### 包含

- {范围项}

### 不包含

- {非目标}

<a id="prerequisites"></a>
<!-- ae-sdd:story-section id=prerequisites layer=primary -->
## 前置条件

| 前置条件 | 责任方 | 验证方式 |
| --- | --- | --- |
| {条件} | {责任方} | {验证} |

<a id="affected-projects"></a>
<!-- ae-sdd:story-section id=affected-projects layer=primary -->
## 涉及工程

| 工程 / 服务 | 模块 | 变更类型 | 影响 |
| --- | --- | --- | --- |
| {工程} | {模块} | {新增/修改/删除} | {影响} |

<a id="trigger-entry"></a>
<!-- ae-sdd:story-section id=trigger-entry layer=primary -->
## 触发入口

| 入口类型 | 入口 | 调用方 | 触发条件 |
| --- | --- | --- | --- |
| {REST/SPI/Event/Job/UI} | {入口} | {调用方} | {条件} |

<a id="main-flow"></a>
<!-- ae-sdd:story-section id=main-flow layer=primary -->
## 主流程

1. {步骤}
2. {步骤}
3. {结果}

<a id="exception-flow"></a>
<!-- ae-sdd:story-section id=exception-flow layer=primary -->
## 异常流程

| 编号 | 触发条件 | 处理流程 | 最终状态 | 用户可见结果 |
| --- | --- | --- | --- | --- |
| EX-1 | {条件} | {处理} | {状态} | {结果} |

<a id="dependencies-risks"></a>
<!-- ae-sdd:story-section id=dependencies-risks layer=primary -->
## 依赖与风险

| 类型 | 项目 | 影响 | 概率 | 缓解措施 | 责任方 |
| --- | --- | --- | --- | --- | --- |
| {依赖/风险} | {项目} | {影响} | {概率} | {措施} | {责任方} |

### 偏离与第三方依赖

| 项目 | 基线 / 契约 | 当前决策或用途 | 原因 / SLA | 风险与降级 | 批准 / 责任方 |
| --- | --- | --- | --- | --- | --- |
| {偏离/第三方} | {基线/契约} | {决策/用途} | {原因/SLA} | {风险/降级} | {批准方/责任方} |

### 核心设计 · 设计

<a id="interface-contract-spi"></a>
<!-- ae-sdd:story-section id=interface-contract-spi layer=primary -->
## 接口契约-SPI

### SPI 接口清单

| 编号 | 接口 | 调用方 | 提供方 | 调用时机 | 失败降级 |
| --- | --- | --- | --- | --- | --- |
| SPI-1 | {接口签名} | {调用方} | {提供方} | {时机} | {降级} |

---

<a id="spi-1"></a>
### SPI-1：{接口签名}

#### 基本信息

| 项目 | 内容 |
| --- | --- |
| 请求模型 | {类型} |
| 返回模型 | {类型} |
| 调用时机 | {时机} |
| 超时 / 重试 | {契约} |
| 幂等 | {契约} |

#### 请求与响应字段

| 方向 | 字段 | 类型 | 必需性 | 约束 | 业务语义 |
| --- | --- | --- | --- | --- | --- |
| Request/Response | {字段} | {类型} | {规则} | {约束} | {语义} |

#### 错误与降级

| 错误码 / 异常 | 触发条件 | 调用方处理 | 是否重试 |
| --- | --- | --- | --- |
| {错误} | {条件} | {处理} | {规则} |

<a id="interface-contract-rest"></a>
<!-- ae-sdd:story-section id=interface-contract-rest layer=primary -->
## 接口契约-REST

### REST 接口清单

| 编号 | 方法 | 路径 | 调用方 | 用途 |
| --- | --- | --- | --- | --- |
| REST-1 | {METHOD} | {/path} | {调用方} | {用途} |

---

<a id="rest-1"></a>
### REST-1：{METHOD} {/path}

#### 基本信息

| 项目 | 内容 |
| --- | --- |
| Method / Path | `{METHOD} {/path}` |
| 鉴权 | {鉴权方式} |
| Content-Type | {类型} |
| 成功状态 | {状态码} |
| 幂等 / 重试 | {契约} |

#### 请求字段

| 字段 | 位置 | 类型 | 必需性 | 约束 | 语义 |
| --- | --- | --- | --- | --- | --- |
| {字段} | {path/query/header/body} | {类型} | {规则} | {约束} | {语义} |

#### 请求示例

```json
{请求示例}
```

#### 成功响应

| 字段 | 类型 | 可空 | 来源 | 语义 |
| --- | --- | --- | --- | --- |
| {字段} | {类型} | {是/否} | {来源} | {语义} |

#### 错误响应与前端行为

| HTTP 状态码 | 业务错误码 | 触发条件 | 用户提示 / 前端行为 | 是否重试 |
| --- | --- | --- | --- | --- |
| {状态码} | {错误码} | {条件} | {提示、禁用、重试或恢复} | {规则} |

#### 响应示例

```json
{响应示例}
```

---

{复制本接口块以增加 REST-2、REST-3；同步更新接口目录、编号和锚点。}

<a id="state-transition-overview"></a>
<!-- ae-sdd:story-section id=state-transition-overview layer=primary -->
## 状态流转总览

| 当前状态 | 事件 / 条件 | 下一状态 | 副作用 | 非法转换处理 |
| --- | --- | --- | --- | --- |
| {状态} | {事件} | {状态} | {副作用} | {处理} |

<a id="error-codes"></a>
<!-- ae-sdd:story-section id=error-codes layer=primary -->
## 错误码

| 错误码 | HTTP / SPI 状态 | 触发条件 | 对外信息 | 是否重试 |
| --- | --- | --- | --- | --- |
| {错误码} | {状态} | {条件} | {信息} | {规则} |

<a id="data-model"></a>
<!-- ae-sdd:story-section id=data-model layer=primary -->
## 数据模型

### 表与字段变更

| 表 | 字段 | 类型 | 可空 | 默认值 | 约束 | 变更 |
| --- | --- | --- | --- | --- | --- | --- |
| {表} | {字段} | {类型} | {规则} | {默认值} | {约束} | {变更} |

### 索引变更

| 表 | 索引 | 字段 | 唯一性 | 目的 |
| --- | --- | --- | --- | --- |
| {表} | {索引} | {字段} | {规则} | {目的} |

### DDL

```sql
{DDL}
```

### 字段链路映射

| 入口字段 | DTO / Command | Domain | Persistence | 输出字段 |
| --- | --- | --- | --- | --- |
| {字段} | {字段} | {字段} | {字段} | {字段} |

### 枚举定义映射

| 枚举 | 值 | 业务语义 | 存储值 | 对外值 |
| --- | --- | --- | --- | --- |
| {枚举} | {值} | {语义} | {存储值} | {对外值} |

<a id="configuration-changes"></a>
<!-- ae-sdd:story-section id=configuration-changes layer=primary -->
## 配置变更

| 配置载体 | Key | 默认值 | 环境差异 | 敏感性 | 生效 / 回滚方式 |
| --- | --- | --- | --- | --- | --- |
| {载体} | {key} | {默认值} | {差异} | {等级} | {方式} |

<a id="non-functional-design"></a>
<!-- ae-sdd:story-section id=non-functional-design layer=primary -->
## 非功能设计

| 维度 | 设计 | 指标 / 阈值 | 失败处理 | 验证方式 |
| --- | --- | --- | --- | --- |
| 幂等 | {设计} | {指标} | {处理} | {验证} |
| 一致性与补偿 | {设计} | {指标} | {处理} | {验证} |
| 性能 | {设计} | {指标} | {处理} | {验证} |
| 安全与权限 | {设计} | {指标} | {处理} | {验证} |
| 可观测性 | {设计} | {指标} | {处理} | {验证} |
| 灰度与回滚 | {设计} | {指标} | {处理} | {验证} |

### 核心设计 · 实现

<a id="implementation-design"></a>
<!-- ae-sdd:story-section id=implementation-design layer=primary -->
## 实现设计

### 复用与决策

| 实现点 | 决策 | 依据 | 复用对象 | 归属层 |
| --- | --- | --- | --- | --- |
| {实现点} | {决策} | {依据} | {对象} | {层} |

### 关键伪代码

```text
{关键实现伪代码：调用顺序、事务/锁边界、关键分支、错误传播}
```

<a id="acceptance-criteria"></a>
<!-- ae-sdd:story-section id=acceptance-criteria layer=primary -->
## 验收标准

| AC ID | Given | When | Then | 来源 |
| --- | --- | --- | --- | --- |
| AC-001 | {前置} | {动作} | {结果} | {来源} |

### 验证矩阵

| 验证 ID | AC | 边界 | 验证方式 | 预期证据 |
| --- | --- | --- | --- | --- |
| V-001 | AC-001 | {边界} | {命令/人工步骤} | {证据} |

### 补充信息

<a id="implementation-task-mapping"></a>
<!-- ae-sdd:story-section id=implementation-task-mapping layer=secondary -->
## 实现任务映射

| 任务 | 覆盖章节 / AC | 工程 | 依赖 | 验证 |
| --- | --- | --- | --- | --- |
| {任务} | {章节/AC} | {工程} | {依赖} | {验证} |

<a id="manual-tasks"></a>
<!-- ae-sdd:story-section id=manual-tasks layer=secondary -->
## 人工任务

| 任务 | 责任方 | 前置 | 完成标准 | 状态 |
| --- | --- | --- | --- | --- |
| {任务} | {责任方} | {前置} | {标准} | {状态} |

<a id="open-questions"></a>
<!-- ae-sdd:story-section id=open-questions layer=secondary -->
## 未决问题

| 问题 | 影响章节 / AC | 决策人 | 截止时间 | 状态 |
| --- | --- | --- | --- | --- |
| {问题} | {影响} | {决策人} | {时间} | {状态} |
