# STORY-{number}-{title}

<a id="metadata"></a>
<!-- ae-sdd:story-section id=metadata layer=secondary -->
## 元信息

| 字段 | 值 |
| --- | --- |
| 文档类型 | Story 用户故事 |
| Story ID | {STORY-ID} |
| 来源 PRD | {PRD 文档引用} |
| 来源 RA | {RA 文档引用；无则填“无”} |
| 来源 DR | {DR 文档引用} |
| 功能点 ID | {功能点 ID} |
| 标题 | {标题} |
| 类型 | {类型} |
| 优先级 | P1 / P2 / P3 |
| 规模 | {规模} |
| 状态 | Draft / Ready / In Progress / Done / Superseded |
| 作者 | {作者} |
| 处理人 | {处理人} |
| 负责人 | {负责人} |

> 示例：Story ID: STORY-003-BE，来源 PRD: PRD-001，来源 DR: DR-001-02，优先级: P1

---

**总章节目录**

- 分析： [用户故事](#user-story) · [业务价值](#business-value) · [范围](#scope) · [前置条件](#prerequisites) · [涉及工程](#affected-projects) · [触发入口](#trigger-entry) · [主流程](#main-flow) · [异常流程](#exception-flow) · [依赖与风险](#dependencies-risks)
- 设计： [接口契约](#interface-contract) · [状态流转总览](#state-transition-overview) · [错误码](#error-codes) · [数据模型](#data-model) · [配置变更](#configuration-changes) · [非功能设计](#non-functional-design)
- 实现： [实现设计](#implementation-design) · [验收标准](#acceptance-criteria)
- 补充： [元信息](#metadata) · [用例设计映射](#testcase-mapping) · [实现任务映射](#implementation-task-mapping) · [人工任务](#manual-tasks) · [未决问题](#open-questions)

> 本模板只承载章节结构；填写义务、来源、写法和 Review 口径以 [`story-writing-guide.md`](../../standards/story/story-writing-guide.md) 为准。

**接口目录**

**新增接口**

| 类型 | 编号 | 接口 / 签名 | 详情 |
| --- | --- | --- | --- |
| REST | REST-1 | {METHOD} {/path} | [跳转](#rest-1) |
| SPI | SPI-1 | {接口签名} | [跳转](#spi-1) |

**复用 / 既有接口**

| 类型 | 编号 | 接口 / 签名 | 本次关系 | 现状证据 | 详情 |
| --- | --- | --- | --- | --- | --- |
| REST / SPI | {REST-N / SPI-N} | {接口 / 签名} | 直接复用 / 既有扩展 / 回归验证 | {源码 / 已发布文档 / 契约测试} | [跳转](#{接口锚点}) |

### 核心设计 · 分析

<a id="user-story"></a>
<!-- ae-sdd:story-section id=user-story layer=primary -->
## 用户故事

作为 {角色}，我希望 {能力}，以便 {价值}。

> 示例：作为 `运营人员`，我希望 `修改用户账号状态`，以便 `快速停用违规账号或恢复正常账号`。

<a id="business-value"></a>
<!-- ae-sdd:story-section id=business-value layer=secondary -->
## 业务价值

- {业务价值}

> 示例：运营人员可以实时响应违规行为，无需走审批流程即可停用问题账号。

<a id="scope"></a>
<!-- ae-sdd:story-section id=scope layer=primary -->
## 范围

### 包含

- {范围项}

### 不包含

- {非目标}

> 示例：
> 包含：修改单个用户状态（启用 / 停用）
> 不包含：批量修改状态、状态变更后的消息通知

<a id="prerequisites"></a>
<!-- ae-sdd:story-section id=prerequisites layer=primary -->
## 前置条件

| 前置条件 | 责任方 | 验证方式 |
| --- | --- | --- |
| 数据前置：{条件} | {责任方} | {验证} |
| 权限前置：{条件} | {责任方} | {验证} |
| 系统前置：{条件} | {责任方} | {验证} |

> 分别填写数据前置、权限前置和系统前置；无特殊前置时明确写“无”。
> 示例：
> 数据前置：目标用户已存在
> 权限前置：操作人具有 `user:status:edit` 权限
> 系统前置：无

<a id="affected-projects"></a>
<!-- ae-sdd:story-section id=affected-projects layer=primary -->
## 涉及工程

| 工程 / 服务 | 父工程 | 类型 | 模块 | 变更类型 | 变更说明 / 影响 |
| --- | --- | --- | --- | --- | --- |
| {工程} | {父工程} | BFF / SPI / Service / API | {模块} | {新增/修改/删除} | {变更说明 / 影响} |

> 若工程不存在，需在父工程中创建子模块并注册到父 pom 的 modules 中。

> 示例：
>
> | 工程 | 父工程 | 类型 | 变更说明 |
> | --- | --- | --- | --- |
> | icec-cloud-life-im-bff | — | BFF | 新增 `PUT /user/{id}/status` Controller 及参数校验，实现对外 API 接口 |
> | icec-cloud-life-im | — | Service | 实现 AppService 编排、DomainService 校验、Repository 更新 |
> | icec-cloud-life-user-spi | icec-cloud-life-spi | SPI | 新增 `UpdateUserStatusRequest` / `UserDTO` 及 `UserService` 接口方法 |
> | icec-cloud-life-user-api | icec-cloud-life-api | API | 新增对外接口定义（APP 侧） |
> | icec-cloud-boss-user-api | icec-cloud-boss-api | API | 新增对外接口定义（Boss 侧） |

<a id="trigger-entry"></a>
<!-- ae-sdd:story-section id=trigger-entry layer=primary -->
## 触发入口

| 入口类型 | 入口 | 调用方 | 触发条件 |
| --- | --- | --- | --- |
| {REST/SPI/Event/Job/UI} | {入口} | {调用方} | {条件} |

- REST 接口（`/api/{module}/v1/...`）：
- SPI 接口（服务间调用）：
- 消息事件（Kafka topic）：
- 后台任务（job-spring-boot-starter）：
- 外部系统：

> 示例：
> REST 接口：`PUT /user/{id}/status`
> SPI 接口：`UserService.updateUserStatus`
> 消息事件：无
> 后台任务：无

<a id="main-flow"></a>
<!-- ae-sdd:story-section id=main-flow layer=primary -->
## 主流程

> 只描述正常路径，假设所有前置条件满足、所有校验通过。异常场景在下方“异常流程”中描述。

1. {步骤}
2. {步骤}
3. {结果}

> 示例：
> 1. 运营人员提交用户 ID 和目标状态
> 2. 校验用户是否存在
> 3. 校验目标状态与当前状态不同
> 4. 更新用户状态及 `last_updated_date`
> 5. 返回更新后的用户信息

<a id="exception-flow"></a>
<!-- ae-sdd:story-section id=exception-flow layer=primary -->
## 异常流程

| 编号 | 场景 | 触发条件 | 处理流程 | 最终状态 | 用户提示 / 日志 | AC ID |
| --- | --- | --- | --- | --- | --- | --- |
| EX-1 | {场景} | {条件} | {处理} | {状态} | {结果} | AC-{编号} |

> 示例：
>
> | 编号 | 场景 | 触发条件 | 处理流程 | 最终状态 | 用户提示 / 日志 | AC ID |
> | --- | --- | --- | --- | --- | --- | --- |
> | EX-1 | 用户不存在 | userId 对应用户不存在 | 抛出业务异常 | 未变更 | 用户不存在 | AC-002 |
> | EX-2 | 状态未变更 | 目标状态与当前状态相同 | 抛出业务异常 | 未变更 | 用户状态未发生变化 | AC-003 |

<a id="dependencies-risks"></a>
<!-- ae-sdd:story-section id=dependencies-risks layer=primary -->
## 依赖与风险

| 类型 | 项目 | 上游依赖 | 下游影响 | 风险 / 影响 | 概率 | 缓解措施 | 责任方 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| {依赖/风险} | {项目} | {上游依赖} | {下游影响} | {风险/影响} | {概率} | {缓解方式} | {责任方} |

### 偏离声明

> 只写偏离 constraints 默认行为的特殊情况。默认行为：需要鉴权、AppService 层开启本地事务、禁止在事务中调用 Feign。

| 项目 | 基线 / 契约 | 当前决策或用途 | 原因 / SLA | 风险与降级 | 批准 / 责任方 |
| --- | --- | --- | --- | --- | --- |
| {偏离项；无偏离时填“无”} | {基线/契约} | {决策/用途} | {原因/SLA} | {风险/降级} | {批准方/责任方} |

> 示例：鉴权可明确 `@SkipAuth` 的批准原因；跨服务一致性可明确 Seata AT 等例外及批准方。

### 第三方服务

| 服务名称 | 调用方式 | 开发文档地址 | 负责人/联系方式 |
| --- | --- | --- | --- |
| {服务名称} | REST / SDK / MQ | {文档地址} | {负责人 / 联系方式} |

> 示例：
>
> | 服务名称 | 调用方式 | 开发文档地址 | 负责人/联系方式 |
> | --- | --- | --- | --- |
> | 极光推送 | REST | https://docs.jiguang.cn/jpush/server/push/rest_api_v3_push | @李四 |
> | 融云 IM | SDK | https://docs.rongcloud.cn/platform-chat-api/server-sdk | @负责人 |
> SDK 坐标：`cn.rongcloud.im:server-sdk-java:4.0.2`（[GitHub](https://github.com/rongcloud/server-sdk-java)）

### 核心设计 · 设计

<a id="interface-contract"></a>
<!-- ae-sdd:story-section id=interface-contract layer=primary -->
## 接口契约

<a id="interface-contract-spi"></a>
<a id="interface-contract-rest"></a>

> 先按接口现状分组：当前代码和已发布契约中不存在、本次需要创建的接口归入“新增接口”；已经存在的接口，无论是直接调用、扩展字段还是仅做回归，都归入“复用 / 既有接口”。每个接口只出现一次，组内继续使用独立的 `REST-N` / `SPI-N` 编号。
>
> 复用 / 既有接口不能只列名称：仍须按对应 REST 或 SPI 完整结构填写，并明确“直接复用 / 既有扩展 / 回归验证”、现状证据和本次变更边界，禁止把设计目标写成当前代码事实。

### 新增接口

> 本组只放当前不存在、由本 Story 创建的接口。无新增接口时填“无”，不要把既有扩展写入本组。

<a id="rest-1"></a>
### REST-1：{METHOD} {/path}

#### 1. 基本信息

| 项目 | 内容 |
| --- | --- |
| 名称 | {接口名称} |
| 描述 | {接口用途与业务边界} |
| Method / Path | `{METHOD} {/path}` |
| 版本 | v{版本} |
| 维护人 | {维护人} |
| 本次关系 | 目标新增 |
| 现状证据 | {当前不存在的路由、Controller 与接口定义核对} |
| 本次变更边界 | {新增的请求、响应、行为与调用链} |
| 调用方 | {调用方} |
| Content-Type | {类型} |
| 成功状态 | {状态码} |

#### 2. 鉴权安全

| Token 方式 | 参数位置 | 权限要求 |
| --- | --- | --- |
| {Bearer / API Key / 无} | {Authorization Header / Query / Cookie} | {权限表达式} |

#### 3. 请求定义

##### Path 参数

| 名称 | 类型 | 必填 | 范围 / 约束 | 说明 |
| --- | --- | --- | --- | --- |
| {参数} | {类型} | 是 / 否 | {范围} | {说明} |

##### Query 参数

| 名称 | 类型 | 必填 | 范围 / 约束 | 说明 |
| --- | --- | --- | --- | --- |
| {参数} | {类型} | 是 / 否 | {范围} | {说明} |

##### Header 参数

| 名称 | 类型 | 必填 | 范围 / 约束 | 说明 |
| --- | --- | --- | --- | --- |
| {参数} | {类型} | 是 / 否 | {范围} | {说明} |

##### Body 参数

| 名称 | 类型 | 必填 | 范围 / 约束 | 说明 |
| --- | --- | --- | --- | --- |
| {字段} | {类型} | 是 / 否 | {范围} | {语义} |

##### 请求示例

```json
{请求示例}
```

#### 4. 响应定义

| 字段 | 类型 | 可空 | 来源 | 语义 |
| --- | --- | --- | --- | --- |
| code | integer | 否 | 业务层 | 业务码；成功通常为 `200`，错误码按接口错误码表填写 |
| message | string / null | 是 | 业务层 | 消息；按项目约定可为 `null` |
| data | object / array / null | 是 | 业务层 | 单对象、列表、分页分别对应项目约定的 VO / List / `PagedModels<T>` |
| 分页 | `PagedModels<T>`（位于 `data`） | 条件 | 查询结果 | 分页接口填写 `data` 的 `PagedModels<T>` 字段结构；非分页接口填 `—` |
| traceId | string | 条件 | 网关 / 服务 | 请求追踪标识；未纳入项目统一 envelope 时填 `—` |

> 当前项目 API envelope 的权威形式为 `code` + `message` + `data`；分页统一使用 `PagedModels<T>`。`traceId` 仅在项目或网关实际提供时记录，不得凭空新增响应字段。

##### data / VO 字段

| 字段 | 类型 | 必填 | 范围 / 约束 | 说明 |
| --- | --- | --- | --- | --- |
| {字段} | {类型} | 是 / 否 | {范围、格式或枚举} | {业务语义} |

##### 响应示例（完整）

```json
{
  "code": 200,
  "message": null,
  "data": {响应数据或分页对象}
}
```

> 若项目统一 envelope 实际包含 `traceId`，在示例中按契约增量加入；否则不得加入。

#### 5. 错误码表

| 错误码 | 含义 | 触发场景 | 处理建议 | HTTP 状态码 | 是否重试 |
| --- | --- | --- | --- | --- | --- |
| {错误码} | {含义} | {条件} | {处理建议} | {状态码} | {规则} |

> “处理建议”同时填写用户提示、前端行为、恢复方式或运维动作；不要另建重复错误表。

#### 6. 非功能

| 幂等 | 限流 | 超时 | 重试 |
| --- | --- | --- | --- |
| {幂等键 / 规则} | {QPS / 并发上限 / 响应} | {连接 / 读取超时} | {次数、退避、不可重试条件} |

#### 7. 调用示例

##### cURL

```bash
curl -X {METHOD} '{baseUrl}{/path}' \
  -H 'Authorization: Bearer {token}' \
  -H 'Content-Type: application/json' \
  -d '{请求示例}'
```

##### SDK

```text
{SDK 调用代码或客户端方法}
```

> 按项目实际 SDK 填写调用代码；没有官方 SDK 时明确注明“无官方 SDK”。

#### 8. 版本变更

正文不写变更历史；统一引用 [`CHANGELOG/`](../../CHANGELOG/)。

---

<a id="spi-1"></a>
### SPI-1：{接口签名}

#### 基本信息

| 项目 | 内容 |
| --- | --- |
| 名称 | {接口名称} |
| 接口签名 | `{返回类型 methodName(Request)}` |
| 本次关系 | 目标新增 |
| 现状证据 | {当前不存在的接口、实现与调用点核对} |
| 本次变更边界 | {新增的契约、实现与调用链} |
| 调用方 | {调用方} |
| 提供方 | {提供方} |
| 请求模型 | {类型} |
| 返回模型 | {类型} |
| 调用时机 | {时机} |
| 超时 / 重试 | {契约} |
| 幂等 | {契约} |

#### 错误与降级

| 错误码 / 异常 | 触发条件 | 调用方处理 | 是否重试 |
| --- | --- | --- | --- |
| {错误} | {条件} | {处理} | {规则} |

> SPI 只填写服务间调用协议实际约定的签名、Request / DTO、字段语义、超时、重试、幂等、错误与降级；不适用的传输层字段不填。以下 Request / DTO 表是字段契约的唯一填写源。

##### Request

| 方向 | 字段 | 类型 | 必需性 | 约束 | 业务语义 |
| --- | --- | --- | --- | --- | --- |
| Request | {字段} | {类型} | 必填 / 选填 | {范围、格式、枚举或校验规则} | {说明} |

##### DTO

| 方向 | 字段 | 类型 | 必需性 | 约束 | 业务语义 |
| --- | --- | --- | --- | --- | --- |
| Response | {字段} | {类型} | 必填 / 选填 | {范围、格式或枚举} | {说明} |

> 示例：`UserDTO updateUserStatus(UpdateUserStatusRequest request)`，Request 包含 `userId`、`status`，DTO 返回 `id`、`status` 和 `lastUpdatedDate`。

---

### 复用 / 既有接口

> 本组放当前已经存在的接口，并在每个接口“基本信息”中将“本次关系”改为“直接复用 / 既有扩展 / 回归验证”之一；“现状证据”引用实际源码、已发布接口文档或契约测试，“本次变更边界”明确保持不变与需要扩展的部分。
>
> 按需复制上方对应的完整 REST 或 SPI 接口块到本组；不得用一行接口名或链接代替请求、响应、错误和非功能契约。复制后同步更新接口目录、编号和锚点。无复用 / 既有接口时填“无”。

<a id="state-transition-overview"></a>
<!-- ae-sdd:story-section id=state-transition-overview layer=primary -->
## 状态流转总览

| 当前状态 | 事件 / 条件 | 下一状态 | 副作用 | 非法转换处理 |
| --- | --- | --- | --- | --- |
| {状态} | {事件} | {状态} | {副作用} | {处理} |

<a id="error-codes"></a>
<!-- ae-sdd:story-section id=error-codes layer=primary -->
## 错误码

| 错误码 | 协议状态（HTTP / SPI 按适用填写） | 触发条件 | 对外信息 | 是否重试 |
| --- | --- | --- | --- | --- |
| {错误码} | {状态} | {条件} | {信息} | {规则} |

<a id="data-model"></a>
<!-- ae-sdd:story-section id=data-model layer=primary -->
## 数据模型

### 表与字段变更

> 表操作须遵守：软删除（`delete_flag`）、更新记录必须同时更新 `last_updated_date`。

### 表变更

| 表 | 变更类型 | 说明 |
| --- | --- | --- |
| {表} | 新增 / 修改 / 删除 | {说明} |

### 字段变更

**{表}**

| 表 | 字段 | 类型 | 可空 | 默认值 | 约束 | 变更 | 说明 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| {表} | {字段} | {类型} | {是/否} | {默认值} | {约束} | {变更} | {业务语义} |

### 索引变更

| 表 | 索引 | 字段 | 唯一性（普通 / 唯一） | 目的 / 说明 |
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

> bootstrap.yml / application.yml 与 Nacos 配置分别记录配置项、所属工程或 DataId、变更类型和说明。

### bootstrap.yml / application.yml

| 配置项 | 所属工程 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| {配置项} | {工程} | 新增 / 修改 | {说明} |

### Nacos 配置

| 配置项 | DataId | 变更类型 | 说明 |
| --- | --- | --- | --- |
| {配置项} | {DataId} | 新增 / 修改 | {说明} |

> 示例：bootstrap/application 中记录 `feign.client.user.url` 及所属工程；Nacos 中记录 `user.status.cache.ttl` 和 `icec-cloud-life-im.yaml` DataId。

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

| AC ID | Given | When | Then | 来源 | 测试层级 |
| --- | --- | --- | --- | --- | --- |
| AC-001 | {前置} | {动作} | {结果} | {来源} | 单元 / 接口 / 集成 |

> **AC 最小覆盖维度（编写时逐项检查）：**
> 1. 正向流程；2. 业务规则拒绝；3. 入参边界；4. 异常容错；5. 幂等 / 并发（如涉及）。

> 示例：
>
> | AC ID | Given | When | Then | 来源 | 测试层级 |
> | --- | --- | --- | --- | --- | --- |
> | AC-001 | 用户存在且当前状态为 ACTIVE | 提交 status=INACTIVE | 返回 200，用户状态变为 INACTIVE | 主流程 | 接口 |
> | AC-002 | userId 对应用户不存在 | 提交任意 status | 返回错误码 11001 | 异常流程 EX-1 | 接口 |
> | AC-003 | 用户当前状态为 ACTIVE | 提交 status=ACTIVE | 返回错误码 11002 | 异常流程 EX-2 | 接口 |

### 验证矩阵

| 验证 ID | AC | 边界 | 验证方式 | 预期证据 |
| --- | --- | --- | --- | --- |
| V-001 | AC-001 | {边界} | {命令/人工步骤} | {证据} |

### 补充信息

<a id="testcase-mapping"></a>
<!-- ae-sdd:story-section id=testcase-mapping layer=secondary -->
## 用例设计映射

详见对应 TestCase 文档的覆盖矩阵章节，以 TestCase 文档为唯一维护源。

格式：`详见 [TC 文档名](相对路径) 覆盖矩阵章节。`

> 示例：详见 [2c-im-testcase-003-BE.md](../../../design/testcase/be/2c-im-testcase-003-BE.md) 覆盖矩阵章节。

<a id="implementation-task-mapping"></a>
<!-- ae-sdd:story-section id=implementation-task-mapping layer=secondary -->
## 实现任务映射

| Task / 任务 | 说明 | 覆盖章节 / AC | 涉及工程 / 层 | 依赖 | 验证 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| {任务} | {说明} | {章节/AC} | {工程名} / BFF · SPI · Service · Domain | {依赖} | {验证} | Planned |

> 示例：
>
> | Task / 任务 | 说明 | 覆盖章节 / AC | 涉及工程 / 层 | 依赖 | 验证 | 状态 |
> | --- | --- | --- | --- | --- | --- | --- |
> | 实现 updateUserStatus SPI 接口 | AppService 编排、DomainService 校验、Repository 更新 | AC-001~AC-003 | icec-cloud-life-cs / Service · Domain | 无 | 单元 / 集成测试 | Planned |
> | 实现 PUT /user/{id}/status REST 接口 | Controller 参数校验、AppService 调用 Facade、Facade 调用 Feign | AC-001~AC-003 | icec-cloud-boss-bff / BFF | SPI Task | 真实 HTTP | Planned |

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
