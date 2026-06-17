---
name: icec-cloud-life-project-assets
description: icec-cloud-life 项目资产实例 — 基于 2026-06-17 Explore Agent 探查，含 25+ 个微服务、5 个分层精确包路径（以 life-cs 和 life-im 为冰山模块）、7 类命名约定、8 类工程约束、缺口记录。供本项目所有 Code Plan 引用。
---

# icec-cloud-life Project Assets — 项目资产实例

> **本文件是 icec-cloud-life 项目的首份资产实例**，按 project-assets-schema 结构填写。
>
> **gitPath：** `D:\Item\life`（源码在 `D:\Item\life\2c\`）
> **探查时间：** 2026-06-17
> **探查 Agent：** 多 Agent 并行 Explore

---

## 0. 摘要与使用场景

| 维度 | 内容 |
|------|------|
| 何时需要查 | Code Plan 编写 / Coding 实现 / Code Review |
| 谁负责写 | 架构组 + 域负责人（life-cs / life-im / life-user / ...） |
| 与 `constraints/` 的关系 | 本文件把通用规则映射到本项目具体包路径/类名；通用规则不在本文件重写 |
| 关键不变量 | 本文件不重复 constraints/ 中定义的 rules，只做规则 → 项目代码事实的映射 |

---

## 1. 项目资产元信息

| 字段 | 值 |
|------|---|
| projectKey | `icec-cloud-life` |
| projectName | `icec-cloud-life`（2C 端产品线） |
| gitPath | `D:\Item\life` |
| sourceRoot | `D:\Item\life\2c\` |
| productLine | `life`（2C 端，对应 boss 为 2B 端） |
| profile | `dev, test, prod, beta-kunlun` |
| mainClass | 各 Service `-service` 子模块的 `WebApplication.java` / `Bootstrap.java` |
| packaging | `jar` |
| portRange | `6001-6100`（web/api/spi）/ `10086-10097`（BFF）/ `20081-20093`（business service） |
| lastAuditedAt | `2026-06-17` |
| owner | 架构组 / 各域负责人 |
| isGitRepo | false（工程根目录非 git 仓库） |
| hasMavenRootPom | false（各模块独立，无 root pom） |

---

## 2. 微服务清单

| name | responsibility | port | contextPath | type | callChain | dependsOnSpi |
|------|---------------|------|-------------|------|-----------|--------------|
| `icec-cloud-life-cs` | 客服域 Service（工单/会话/坐席/状态机/语音外呼）；DDD 四层 | 20092 | — | service | Service（DDD）；暴露 cs-spi | — |
| `icec-cloud-life-im` | IM 域 Service（会话/消息/融云/参与者）；DDD 四层 | 20093 | — | service | Service（DDD）；暴露 im-spi | — |
| `icec-cloud-life-im-bff` | IM BFF（面向 APP/工作台的 IM 接口聚合）；单模块 | 10096 | `/life-im-bff` | bff | BFF → im-spi/cs-spi → Service | `icec-cloud-life-im-spi`, `icec-cloud-life-cs-spi` |
| `icec-cloud-life-user` | 2C 用户域（双启动模块：service 6602 + web 6001） | 6602 (service) / 6001 (web) | `/user-api` (web) | service | Service（DDD）+ Web 暴露层 | — |
| `icec-cloud-life-vehicle` | 车辆域 Service；DDD 四层 | 20087 | — | service | Service（DDD）；暴露 vehicle-spi | — |
| `icec-cloud-life-vehicle-bff` | 车辆 BFF；单模块 | 10087 | `/vehicle-bff` | bff | BFF → vehicle-spi → Service | `icec-cloud-life-vehicle-spi` |
| `icec-cloud-life-notification` | 通用通知 Service（极光/融云/内部事件）；双启动模块 | 6605 (service) / 6005 (web) | `/notification-api` (web) | service | Service + Web | — |
| `icec-cloud-life-ops-notification` | 运营通知 Service；DDD 四层 | 20091 | — | service | Service（DDD） | — |
| `icec-cloud-life-workticket` | 工单域 Service；DDD 四层 | 20090 | — | service | Service（DDD）；暴露 workticket-spi | — |
| `icec-cloud-life-content-feed` | 内容流 Service | 20089 | — | service | Service | — |
| `icec-cloud-life-content-feed-bff` | 内容流 BFF | 10095 | `/content-feed-bff` | bff | BFF → Service | — |
| `icec-cloud-life-touchpoint` | 触点/行为 Service；暴露 touchpoint-spi | 20088 | — | service | Service | — |
| `icec-cloud-life-captcha` | 验证码 Service；暴露 captcha-spi | 20086 | — | service | Service | — |
| `icec-cloud-life-configuration` | 配置 Service | 20081 | — | service | Service | — |
| `icec-cloud-life-obs` | 对象存储 Service（双启动模块：service + web /obs-api） | 6602 (service) / 6002 (web) | `/obs-api` (web) | service | Service + Web | — |
| `icec-cloud-life-event-integration` | 集成事件 Service（含 Kafka） | 6606 | — | service | Service（事件驱动） | — |
| `icec-cloud-life-user-journey-bff` | 用户旅程 BFF | 10088 | `/user-journey-bff` | bff | BFF → SPI → Service | — |
| `icec-cloud-life-auth-bff` | 认证 BFF（登录/授权/Token）；单模块 | 10086 | `/auth/bff` | bff | BFF（公开 /public/ 前缀） | — |
| `icec-cloud-life-workticket-bff` | 工单 BFF | 10089 | `/workticket-bff` | bff | BFF → workticket-spi → Service | `icec-cloud-life-workticket-spi` |
| `icec-cloud-life-spi`（聚合 SPI） | SPI 父工程（11 个子模块 SPI；聚合 life-spi-service 端口 6099） | 6099 | — | spi | 被 BFF/Service Feign 消费 | — |
| `icec-cloud-life-api`（聚合 API） | 2C 端 API 聚合工程（auth/im/user-journey/vehicle/content-feed/workticket bff-api）；api-service 端口 6100 | 6100 | — | api | *-bff-api → api-service | — |

**微服务总数：21 个**（含 2 个聚合工程 + 19 个业务 Service/BFF）

**端口段：**
- `6001-6100`：web/api/spi（6001 life-user-web / 6002 life-obs-web / 6005 life-notification-web / 6099 life-spi / 6100 life-api / 6602 life-user-service/obs-service / 6605 life-notification-service / 6606 life-event-integration）
- `10086-10097`：BFF（10086 auth-bff / 10087 vehicle-bff / 10088 user-journey-bff / 10089 workticket-bff / 10095 content-feed-bff / 10096 im-bff）
- `20081-20093`：业务 Service（20081 configuration / 20086 captcha / 20087 vehicle / 20088 touchpoint / 20089 content-feed / 20090 workticket / 20091 ops-notification / 20092 cs / 20093 im）

**双启动模块（Service + Web）说明：** life-user / life-notification / life-obs 采用"双启动模块"（`-service` 后端服务 + `-web` Web 暴露层），`-web` 模块通过 `context-path` 直接暴露 REST API（不经过 BFF）。

---

## 3. 抽象分层 → 项目分层映射（粗粒度）

> 行=抽象 4 层+可选 2 类；列=本项目对应的工程模块。详细包路径见 §4。

| 抽象层 | 本项目对应工程模块 | 备注 |
|--------|------------------|------|
| 请求处理（Interfaces） | `{module}/icec-cloud-life-{domain}-interfaces` | SPI 接口实现层，不含 BFF 入口 |
| 业务编排（Application） | `{module}/icec-cloud-life-{domain}-application` | 事务在 AppService |
| 领域逻辑（Domain） | `{module}/icec-cloud-life-{domain}-domain` | 充血模型 |
| 基础能力（Infrastructure） | `{module}/icec-cloud-life-{domain}-infrastructure` | 仅存取语义 |
| 跨模块 SPI（可选） | `icec-cloud-life-spi/icec-cloud-life-{domain}-spi` | `ServiceProviderConstants` 管服务名 |
| BFF 入口（可选） | `icec-cloud-life-{domain}-bff` 或独立单模块工程 | 仅当 type=bff |

**典型例子（icec-cloud-life-cs）：**
- Interfaces → `icec-cloud-life-cs/icec-cloud-life-cs-interfaces`（包：`com.casstime.cloud.life.cs.interfaces`）
- Application → `icec-cloud-life-cs/icec-cloud-life-cs-application`（包：`com.casstime.cloud.life.cs.application`）
- Domain → `icec-cloud-life-cs/icec-cloud-life-cs-domain`（包：`com.casstime.cloud.life.cs.domain`）
- Infrastructure → `icec-cloud-life-cs/icec-cloud-life-cs-infrastructure`（包：`com.casstime.cloud.life.cs.infrastructure`）
- 启动模块 → `icec-cloud-life-cs/icec-cloud-life-cs-service`

**BFF 单模块例子（icec-cloud-life-im-bff）：**
- 无子模块分层；包路径 `com.casstime.cloud.life.bff.im`
- interfaces/restful → `com.casstime.cloud.life.bff.im.interfaces.restful`
- application/appservice → `com.casstime.cloud.life.bff.im.application.appservice`
- application/facade → `com.casstime.cloud.life.bff.im.application.facade`

---

## 4. DDD 内部分层落点（细粒度）

> 基于冰山模块 `icec-cloud-life-cs` 和 `icec-cloud-life-im` 联合抽取。**类角色 → 精确包路径** 是项目资产"最核心的可复用部分"。

| 类角色 | 精确包路径 | 典型类名 | 放什么 / 不放什么 |
|--------|-----------|---------|------------------|
| **Interfaces 层** | | | |
| SPI 实现类 | `icec-cloud-life-{domain}-interfaces/src/main/java/com/casstime/cloud/life/{domain}/interfaces/restful/` | `CsUserServiceImpl` / `CsConversationServiceImpl` / `CsTicketServiceImpl`（implements SPI 接口 + `@RestController`） | 仅协议适配，不写业务规则 |
| Job Handler | `icec-cloud-life-{domain}-interfaces/src/main/java/com/casstime/cloud/life/{domain}/interfaces/jobhandler/` | `TimeoutWaitingJobHandler` / `TimeoutAgentJobHandler` / `WaitingReminderJobHandler` | 定时任务入口，使用 job-spring-boot-starter |
| Exception Handler | `icec-cloud-life-{domain}-interfaces/src/main/java/com/casstime/cloud/life/{domain}/interfaces/config/` | `ImExceptionHandler`（`@RestControllerAdvice`） | 全局异常处理 |
| **Application 层** | | | |
| AppService | `icec-cloud-life-{domain}-application/src/main/java/com/casstime/cloud/life/{domain}/application/appservice/` | `CsTicketAppService` / `CsConversationAppService` / `CsUserAppService` / `ImTokenAppService` / `ImSessionAppService` / `ImMessageAppService` | 事务、编排、调 Domain 顺序 |
| Converter（Application） | `icec-cloud-life-{domain}-application/src/main/java/com/casstime/cloud/life/{domain}/application/converter/` | `CsTicketConverter` / `CsConversationConverter`（`@UtilityClass` + 静态方法，cs 域已验证） | DO↔DTO 转换 |
| **Domain 层** | | | |
| Domain Object（充血） | `icec-cloud-life-{domain}-domain/src/main/java/com/casstime/cloud/life/{domain}/domain/{subdomain}/model/entity/` | `CsTicketDO` / `CsConversationDO` / `CsUserDO` / `ImSessionDO` / `ImSessionParticipantDO` / `ImMessageDO` | 业务方法不以 get/set 开头；`@Data` |
| Repository 接口 | `icec-cloud-life-{domain}-domain/src/main/java/com/casstime/cloud/life/{domain}/domain/{subdomain}/repository/` | `CsTicketRepository` / `CsConversationRepository` / `ImSessionRepository` / `ImMessageRepository`（**仅接口**） | 不允许放业务规则 |
| Domain Service | `icec-cloud-life-{domain}-domain/src/main/java/com/casstime/cloud/life/{domain}/domain/{subdomain}/service/` | `CsTicketDomainService`（核心领域服务，含状态机逻辑） | 跨聚合业务规则 |
| Message Body DO（im 特有） | `com.casstime.cloud.life.im.domain.message.model.entity` | `TextMessageBodyDO` / `ImageMessageBodyDO` / `FileMessageBodyDO` / `ImMessageBodyDO` / `CompositeMessageBodyDO` | im 域消息体多态 |
| **Infrastructure 层** | | | |
| PO | `icec-cloud-life-{domain}-infrastructure/src/main/java/com/casstime/cloud/life/{domain}/infrastructure/{subdomain}/persistence/entity/` | `CsTicketPO` / `CsConversationPO` / `ImSessionPO` / `ImMessagePO`（`@TableName("cs_ticket")`） | 贫血模型，对应表 |
| Repository Impl | `icec-cloud-life-{domain}-infrastructure/src/main/java/com/casstime/cloud/life/{domain}/infrastructure/{subdomain}/persistence/repository/mysql/` | `CsTicketRepositoryImpl` / `ImSessionRepositoryImpl` extends `ServiceImpl<Mapper, PO>` | 方法名：`findByXxx / save / update` |
| PersistenceConverter（Infra） | `icec-cloud-life-{domain}-infrastructure/src/main/java/com/casstime/cloud/life/{domain}/infrastructure/{subdomain}/persistence/converter/` | `CsTicketPersistenceConverter` / `ImSessionPOConverter` / `ImMessagePOConverter` | PO↔DO 转换 |
| Feign Client | `icec-cloud-life-{domain}-infrastructure/src/main/java/com/casstime/cloud/life/{domain}/infrastructure/feign/` | `ImSessionServiceClient` / `ImMessageServiceClient` / `UserServiceClient` / `NotificationServiceClient` | 调外部 Service；extends SPI 接口 |
| **SPI（被消费方）** | | | |
| SPI 接口 | `icec-cloud-life-spi/icec-cloud-life-{domain}-spi/src/main/java/com/casstime/cloud/life/spi/{domain}/service/` | `CsUserService` / `CsConversationService` / `CsTicketService` / `ImMessageService` / `ImSessionService` / `ImTokenService` | Feign 接口；被 Feign Client extends |
| DTO | `icec-cloud-life-spi/icec-cloud-life-{domain}-spi/src/main/java/com/casstime/cloud/life/spi/{domain}/dto/` | `CsTicketDTO` / `ImSessionDTO` | 跨服务传输 |
| ServiceProviderConstants | `icec-cloud-life-spi/icec-cloud-life-{domain}-spi/src/main/java/com/casstime/cloud/life/spi/{domain}/` | `ServiceProviderConstants`（`LIFE_CS_SERVICE = "life-cs-service"`） | **禁硬编码服务名** |
| **BFF（单模块）** | | | |
| BFF Rest Impl | `icec-cloud-life-{domain}-bff/src/main/java/com/casstime/cloud/life/bff/{domain}/interfaces/restful/` | `ImTokenRestImpl` / `CsConversationRestImpl`（implements `*Rest` 接口） | BFF 控制器 |
| BFF AppService | `icec-cloud-life-{domain}-bff/src/main/java/com/casstime/cloud/life/bff/{domain}/application/appservice/` | `ImTokenAppService` / `CsConversationAppService` | BFF 编排 |
| BFF Facade | `icec-cloud-life-{domain}-bff/src/main/java/com/casstime/cloud/life/bff/{domain}/application/facade/` | `ImTokenFacade` / `CsConversationFacade` | 抽象外部服务 |

---

### 4.5 分层职责硬约束（🔴 违反即阻断）

#### 4.5.1 判定口诀表（项目特定）

| 这段代码是… | 归属层 | 落点 | 关键判定 |
|---|---|---|---|
| 业务规则 / 能不能 / 状态流转 / 不变量 | **Domain** | 实体充血方法 / DomainService | 业务不变量校验 |
| 先做A再做B / 协调谁调谁 / 事务边界 / 转 DTO | **Application** | AppService | 跨域编排、事务边界 |
| 数据存取 / 取出来 / 转 PO↔DO / 查询条件 | **Repository** | RepositoryImpl | 数据访问封装 |
| 参数格式校验（@Valid）/ HTTP 适配 | **Interfaces** | SPI 实现类 | 协议层职责 |

#### 4.5.2 禁止事项（项目特定）

- ❌ 在 Application 写领域规则（下沉到 Domain）
- ❌ 在 Application 写 SQL/持久化细节
- ❌ 在 Application 写纯数据访问封装（字段提取/Map 组装）—— 下沉到 Repository
- ❌ 在 Domain 串多个外部服务的编排
- ❌ 在 Domain 出现 PO/DTO/SQL
- ❌ 在 Repository 写业务规则或状态流转判断
- ❌ BFF 直连 DB（必须经 Feign SPI）
- ❌ BFF 写 `@Transactional`（事务在 Service AppService）

#### 4.5.3 典型反模式（项目踩坑沉淀）

| 反模式 | 真实表现 | 正确做法 |
|--------|---------|---------|
| AppService 做纯读封装 | Application 层做"调 Repository + 转换字段" | 在 Repository 接口声明方法，RepositoryImpl 实现 |
| AppService 内联 SQL 拼接 | "WHERE id=" + id | MyBatis Mapper 配 `#{}` 参数化 |
| Domain Service 调 Feign Client | 领域规则里调外部服务 | 跨服务调用移到 AppService |

---

## 5. 命名约定

| 对象 | 命名模板 | 本项目例子 | 反例 |
|------|---------|----------|------|
| Controller（SPI 实现） | `{Resource}ServiceImpl` | `CsUserServiceImpl` / `CsTicketServiceImpl` / `ImMessageServiceImpl` | ❌ `CsUserController` |
| Controller（BFF） | `{Resource}RestImpl` | `ImTokenRestImpl` / `CsConversationRestImpl` | ❌ `ImTokenController` |
| AppService（Service） | `{Resource}AppService` | `CsTicketAppService` / `ImSessionAppService` | ❌ `CsTicketManager` |
| AppService（BFF） | `{Resource}AppService` | `ImTokenAppService` / `CsConversationAppService` | ❌ `ImTokenService`（歧义） |
| Facade（BFF） | `{Resource}Facade` / `{Resource}FacadeImpl` | `ImTokenFacade` / `CsConversationFacade` | — |
| Domain Object | `{Resource}DO` | `CsTicketDO` / `ImSessionDO` | ❌ `CsTicket`（缺 DO 后缀） |
| Persistent Object | `{Resource}PO` | `CsTicketPO` / `ImSessionPO` | ❌ `CsTicketEntity` |
| Repository 接口 | `{Resource}Repository` | `CsTicketRepository` / `ImSessionRepository` | ❌ `CsTicketDao` |
| Repository Impl | `{Resource}RepositoryImpl` | `CsTicketRepositoryImpl` / `ImSessionRepositoryImpl` | — |
| Converter（Application） | `{Resource}Converter` | `CsTicketConverter` / `CsConversationConverter`（`@UtilityClass`） | — |
| PersistenceConverter（Infra） | `{Resource}PersistenceConverter` 或 `{Resource}POConverter` | `CsTicketPersistenceConverter` / `ImSessionPOConverter` | — |
| Feign Client（Infrastructure） | `{Resource}Client` 或 `{Resource}ServiceClient` | `ImSessionServiceClient` / `UserServiceClient` / `NotificationServiceClient` | — |
| Domain Service | `{Resource}DomainService` | `CsTicketDomainService` | — |
| Job Handler | `{Feature}JobHandler` | `TimeoutWaitingJobHandler` / `SyncVoiceCallRecordJobHandler` | — |

**反例汇总（本项目常见违规）：**
- ❌ 用 `LocalDateTime` → ✅ 用 `java.util.Date`
- ❌ 用 MapStruct → ✅ 显式 Converter（`@UtilityClass` 静态方法）
- ❌ 用 `@Scheduled` → ✅ job-spring-boot-starter + JobHandler
- ❌ `CsTicketManager` / `CsTicketHandler`（动名词/名词）→ ✅ `CsTicketAppService`
- ❌ BFF 中 AppService 直接 `@Autowired` Feign Client → ✅ 经 Facade 调用

**注意：** cs 服务的 Interfaces 层命名为 `XxxServiceImpl implements XxxService`（即实现的是 SPI 接口），而 im-bff 的 BFF 控制器命名为 `XxxRestImpl implements XxxRest`（实现的是 api 模块接口）。两类命名在本项目中并存。

---

## 6. 工程约束（继承自 constraints/，按本项目裁剪+补缺）

### 6.1 分层架构

- 外部请求 → API（聚合工程）→ BFF → SPI（Feign）→ Service（DDD 四层）
- BFF **禁止**直接操作 DB/Redis/Kafka
- Service **禁止**直连前端
- Service 间同步调用必须走 SPI（Feign）
- **禁止**跨 Service 直连数据库

### 6.2 工程结构

- 业务规则 → Domain；协调谁调谁 → Application；存取数据 → Repository
- Repository 方法名仅 `findByXxx / save / update / updateStatus`
- 对象类型：Domain 仅 DO（充血），Interfaces 仅接收/响应 DTO（无 DO/PO）
- 无 root pom，各模块独立 pom，依赖通过本地 mvn install 解决

### 6.3 代码风格

- 时间统一 `java.util.Date`（禁 LocalDateTime）
- JSON 用 `com.casstime.commons.utils.JsonUtils`
- 枚举统一 `(key, value)` 双字段
- 事务增删改在 AppService 用 `@Transactional`（查询不开）
- **事务内禁止**远程调用/MQ 发送
- Feign Client 必须 extends SPI 接口
- BFF AppService **必须经** Facade 调 Feign，Facade 异常返回 null/空集合/Result.error
- 构造器注入用 `@RequiredArgsConstructor`
- 日志用 `@Slf4j`
- 禁魔法值（常量化）

### 6.4 接口规范

- URL 小写连字符，名词单数，公开接口加 `/public/` 前缀
- 分页 POST + RequestBody，统一 `PageRequest<T>` 包装
- 返回值：BFF 用 `ApiResult<T>`；分页用 `PagedModels<T>`
- 删除优先逻辑删除，物理删除需在 DR 说明
- HTTP 状态码统一 200，错误经 code/message 表达
- Swagger 注解必加
- **错误码 5 位分段（life 域）：**
  - 客服 CS：16000-16999
  - IM：17000-17999
  - 触点：18000-18999
  - 验证码：19000-19999
  - 2C 用户：20000-20999
  - 工单：21000-21999
  - 运营通知：22000-22999
  - 车辆域：23000-23999

### 6.5 数据库规范

- 库名 = 服务名；utf8mb4；Service 独占库
- 必备字段：`id / created_by / created_date / last_updated_by / last_updated_date`
- 主键：业务 `bigint AUTO_INCREMENT` 或 `varchar(32)` 业务单号
- 按需 `deleted_flag TINYINT(1) DEFAULT 0`
- 表名/字段名小写下划线单数；禁用保留字
- `is_xxx` 用 `tinyint(1)`；金额 `decimal`；时间 `datetime`
- 单表索引 ≤ 5；varchar 索引指定长度
- 超过 3 表禁 JOIN；禁外键级联；IN 集合 ≤ 1000
- `#{}` 参数化（禁 `${}`）；禁 `SELECT *`；分页先判 count

### 6.6 安全规范

- JWT 经 Cookie `security_context` 传递（**禁止** `Authorization: Bearer` 方式）
- BFF 层获取当前用户**必须**用 `TokenService.getCurUserId()`，**禁用** `AccessUserInfoContext`
- 手机号脱敏：`DesensitizeUtils.handleCellphone`
- 密码 BCrypt 单向哈希
- 日志禁打密码/手机号/Token
- 接口响应不返密码字段

### 6.7 测试规范

- Controller 测试真实 HTTP（`@SpringBootTest(webEnvironment = RANDOM_PORT) + TestRestTemplate`）
- Service 单测 JUnit + Mockito，**禁真实** DB/远程
- 集成测试用开发库 + `@Transactional + @Rollback`
- 核心落库路径必须真实 DB 验证（**禁全 mock**）
- 覆盖率：核心业务 ≥ 80% / 核心方法 100%

### 6.8 技术栈范围

- **Java 8 + Spring Boot 1.5.7 + Spring Cloud Dalston.SR4**
- MyBatis-Plus 3.3.2 + Lombok 1.18.16 + MapStruct 1.5.3.Final（实际探查：cs/im Converter 用 `@UtilityClass` 显式转换，不用 MapStruct 生成器）
- Swagger 2.8.0 + Feign + Hystrix
- MySQL + Redis + Kafka（经 courier 组件）
- 定时任务用 job-spring-boot-starter + `@JobHandler`（禁 `@Scheduled`）
- 禁直配 logback/log4j（用 casslog）
- 基础组件：panda / casslog / cassmetrics / job-spring-boot-starter

### 6.9 隐性约定

> 代码探查中发现的"大家都知道但没写下来"的约定。

- **无 root pom**：所有工程独立构建，依赖必须先 `mvn install` 到本地 maven 仓库
- **cs 服务 Converter 使用 `@UtilityClass`**（已验证 `CsTicketConverter`）；im 服务 Converter 使用 `public final class` + 私有构造（代码注释说明原因）
- **CS 异步通知线程池**：`csNotificationExecutor` core=4/max=8/queue=200 + `CallerRunsPolicy`（生产验证参数）
- **IM 回调 CS 方向**：`ImMessageEventNotifyClient` (`@FeignClient(LIFE_CS_SERVICE)`) 在 im-infrastructure/feign 层
- **cs 服务无 `@AggregateRoot` 注解**：`CsTicketDO` 是业务语义聚合根，无框架标注
- **im-bff 同时消费 im-spi 和 cs-spi**：BFF 跨两个域 SPI

### 6.10 工程经验检查清单（🔴 编码完成自审必跑）

| # | 检查项 | 说明 | 来源 |
|---|--------|------|------|
| 1 | **静态扫描全限定名** | import 块外不应出现 `java.util.` / `java.sql.` 全限定名 | 通用 |
| 2 | **application 层禁 import infrastructure.persistence** | 必须经 Repository 接口（domain 层）| 通用 |
| 3 | **新加方法分层归属自查** | 用 §4.5 判定口诀表自查每个新方法 | 通用 |
| 4 | **BFF AppService 调 Feign 必须经 Facade** | 不可直接 `@Autowired XxxClient` | 本项目 §6.9 |
| 5 | **Service 间 Feign Client 放 infrastructure/feign/，命名 `{Resource}Client`** | 遵循分层 | 本项目 §6.9 |
| 6 | **pom 依赖是否被注释** | 新工程模板 SPI 依赖常被注释 | 通用 |
| 7 | **lombok scope=provided** | 不传递，必须显式声明 | 通用 |
| 8 | **第三方 SDK 实际包路径** | 从 jar 确认，不靠训练数据猜测 | 通用 |
| 9 | **@NotBlank 来源包** | hibernate-validator 5.x 用 `org.hibernate.validator.constraints.NotBlank` | 通用 |
| 10 | **新模块注册到父 pom modules** | 子模块必须加到父 pom | 通用 |
| 11 | **BFF Controller 实现 Rest 接口** | 不自己加 @Api/@GetMapping，接口定义在 bff-api | 通用 |
| 12 | **Feign 注解版本** | Spring Cloud Dalston 用 `org.springframework.cloud.netflix.feign.FeignClient` | 本项目 |
| 13 | **VO 和 DTO 分离** | bff-api 定义 VO，SPI 定义 DTO | 通用 |
| 14 | **无 root pom 时** | 跨模块依赖需先 `mvn install` | 本项目 §6.9 |

### 6.11 工程特定静态扫描清单（🔴 编码完成必跑）

```bash
# 1. 全限定名扫描（除 import 块外不应出现）
grep -rn "com\.casstime\.cloud\.life\.\(domain\|infrastructure\)\.\w\+\." \
  --include="*.java" \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空

# 2. application 层禁直 import infrastructure.persistence
grep -rn "import com\.casstime\.cloud\.life\.cs\.infrastructure\.persistence" \
  --include="*.java" \
  icec-cloud-life-cs-application/src/main/java/
# 期望输出为空

# 3. SQL 关键字不应在 AppService 层出现
grep -rn "SELECT\|INSERT\|UPDATE\|DELETE" \
  --include="*AppService.java" \
  icec-cloud-*/src/main/java/ \
  | grep -v "import " | grep -v "//.*"
# 期望仅 Infrastructure 出现

# 4. 状态机流转条件不应在 Controller/ServiceImpl 层出现
grep -rn "if.*\.getStatus()\|switch.*status" \
  --include="*ServiceImpl.java" --include="*RestImpl.java" \
  icec-cloud-*/icec-cloud-*-interfaces/
# 期望输出为空
```

---

## 7. 跨服务契约入口

### 7.1 关键事实表

| 字段 | 本项目值 | 抽取命令 |
|------|---------|---------|
| Feign 服务名常量 | 见下方 SPI 清单 | `grep -rn "ServiceProviderConstants" icec-cloud-life-spi/ --include="*.java"` |
| Nacos 实例名 | 同下方 Nacos 服务名列 | bootstrap.yml `spring.application.name` |
| 错误码分段 | 见 §6.4 | `find . -path "*/enums/error/*ErrorCode.java"` |

**11 个 SPI 子模块清单（icec-cloud-life-spi 下）：**

| SPI 子模块 | 推测常量字段 | Nacos 服务名 | 业务域 |
|------------|------------|------------|-------|
| `icec-cloud-life-captcha-spi` | `CAPTCHA_SERVICE` | `life-captcha-service` | 验证码 |
| `icec-cloud-life-cs-spi` | `LIFE_CS_SERVICE` | `life-cs-service` | 客服域 |
| `icec-cloud-life-im-spi` | `LIFE_IM_SERVICE` | `life-im-service` | IM 域 |
| `icec-cloud-life-notification-spi` | `LIFE_NOTIFICATION_SERVICE` | `life-notification-service` | 通用通知 |
| `icec-cloud-life-ops-notification-spi` | `OPS_NOTIFICATION_SERVICE` | `life-ops-notification-service` | 运营通知 |
| `icec-cloud-life-passport-spi` | `PASSPORT_SERVICE` | `life-passport-service` | 登录/授权 |
| `icec-cloud-life-touchpoint-spi` | `TOUCHPOINT_SERVICE` | `life-touchpoint-service` | 触点/行为 |
| `icec-cloud-life-user-spi` | `LIFE_USER_SERVICE` | `life-user-service` | 用户域（2C） |
| `icec-cloud-life-vehicle-spi` | `VEHICLE_SERVICE` | `life-vehicle-service` | 车辆域 |
| `icec-cloud-life-integration-event` | — | — | 集成事件（非 SPI） |
| `icec-cloud-boss-abnormal-spi` | `ABNORMAL_ABNORMAL_SERVICE` | `boss-abnormal-service` | Boss 异常（跨产品线） |

### 7.2 关键 Feign 消费关系

| 消费方 | 消费的 SPI 服务名 | 典型 Client 类 | 包路径 |
|--------|-----------------|---------------|--------|
| `icec-cloud-life-cs` | `life-im-service` | `ImSessionServiceClient` / `ImMessageServiceClient` / `ImTokenServiceClient` | `com.casstime.cloud.life.cs.infrastructure.feign` |
| `icec-cloud-life-cs` | `life-user-service` | `UserServiceClient` / `UserBindingVehicleClient` | `com.casstime.cloud.life.cs.infrastructure.feign` |
| `icec-cloud-life-cs` | `life-notification-service` | `NotificationServiceClient` | `com.casstime.cloud.life.cs.infrastructure.feign` |
| `icec-cloud-life-im` | `life-cs-service` | `ImMessageEventNotifyClient` | `com.casstime.cloud.life.im.infrastructure.feign` |
| `icec-cloud-life-im-bff` | `life-im-service` / `life-cs-service` | Feign Client（bff 层） | `com.casstime.cloud.life.bff.im.infrastructure.feign` |

### 7.3 契约抽取命令清单

```bash
# Feign 服务名常量
grep -rn "ServiceProviderConstants" D:\Item\life\2c\icec-cloud-life-spi\ --include="*.java"

# Nacos 应用名
find D:\Item\life\2c\ -name "bootstrap*.yml" -not -path "*/target/*" -exec grep -H "spring.application.name" {} \;

# 所有 @FeignClient 注解
grep -rn "@FeignClient" D:\Item\life\2c\ --include="*.java" | grep -v "/target/"

# 端口汇总
find D:\Item\life\2c\ -name "bootstrap*.yml" -not -path "*/target/*" -exec grep -H "server.port\|contextPath" {} \;

---

## 8. Code Plan 输入索引

| Code Plan 章节 | 引用本项目资产章节 | 说明 |
|---------------|------------------|------|
| §1 项目资产引用块 | §1-§10 全部 | 文首必引 |
| §2 抽象分层 → 项目分层映射 | §3 / §4 | 包路径/类名 |
| §5 关键类骨架 | §4 / §5 | 类角色 + 命名 |
| §6 DO 字段对齐 | §6.5 | 审计四字段 |
| §7 Mapper / SQL | §6.5 | EXPLAIN 验证 |
| §8 测试对应 | §6.7 | 真实 DB/HTTP |
| §11 约束合规自审 | §6 全章 | 8 类约束 |
| §12 接口契约一致性 | §6.4 | URL/方法/分页/错误码 |

---

## 9. 探查 SOP

> 完整 9 步见 `strategies/project-assets-schema.md §9`。本项目已应用此 SOP 完成首版（2026-06-17 多 Agent 并行探查）。

---

## 10. 项目资产缺口与待补充

| # | 缺口 | 优先级 | 状态 | 计划补齐时间 |
|---|------|-------|------|------------|
| 1 | life-user / life-vehicle / life-workticket / life-notification 等域的 DDD 内部分层包路径未逐一验证（仅 cs + im 为冰山模块） | 🟠 P1 | 待补 | 2026-07 |
| 2 | ServiceProviderConstants 各字段具体值未全量扫描（11 个 SPI 清单中部分为推测值） | 🟠 P1 | 待补 | 2026-07 |
| 3 | life-obs / life-obs-service 端口疑似与 life-user-service 共用 6602，需核实 | 🟠 P1 | 待核实 | 2026-07 |
| 4 | 错误码具体占用值（各域 16xxx-23xxx 下已使用的码值）未探查 | 🟡 P2 | 待补 | 2026-08 |
| 5 | life-user / life-vehicle 等服务的 Converter 风格（是否 @UtilityClass）未验证 | 🟡 P2 | 待补 | 2026-08 |
| 6 | boss 相关模块（boss-common / boss-security / boss-user-bff 等）位于另一工程（d:\Item\icec-cloud-boss），本文件不覆盖 | 🟢 P3 | 设计如此 | — |
| 7 | life-event-integration 的 Kafka Topic 命名规则未探查 | 🟡 P2 | 待补 | 2026-08 |
| 8 | icec-cloud-life-auth-bff 的认证流程（Token 颁发/刷新/Cookie 写入）细节未探查 | 🟠 P1 | 待补 | 2026-07 |
| 9 | icec-cloud-life-configuration 的配置中心集成方式（Nacos/Panda）未探查 | 🟢 P3 | 待补 | 2026-Q4 |
| 10 | 各 BFF 工程的 Feign Client 注册方式（是否所有 BFF 都有 Facade 层）未全量验证 | 🟡 P2 | 待补 | 2026-07 |

---

## §A 资产大纲（索引层）

本文件共 10 节 + 7 个索引层，覆盖以下维度：

| 章节 | 内容摘要 | 读者 |
|------|---------|------|
| §0 | 使用场景 / constraints/ 关系 | 所有开发者 |
| §1 | 元信息（projectKey / gitPath / portRange）| 工具 / CI |
| §2 | 21 个微服务清单（端口 / ContextPath / 类型）| Code Plan 编写者 |
| §3 | 抽象层 → 工程模块映射（粗粒度）| 新开发者入门 |
| §4 | DDD 内部分层落点（精确包路径 + 典型类名）| Code Plan 必引 |
| §5 | 命名约定（14 类对象 + 反例）| Code Review |
| §6 | 工程约束（分层/代码风格/接口/DB/安全/测试/技术栈/隐性约定/扫描）| 编码自审 |
| §7 | 跨服务契约（11 个 SPI + Feign 消费关系）| 跨服务开发 |
| §8 | Code Plan 输入索引 | Code Plan 编写者 |
| §9 | 探查 SOP | 资产维护者 |
| §10 | 缺口与待补充 | 架构组 |

---

## §B 模块索引

| module | 概述 | 基础包 | 类型 | 入口 Controller | 关键 AppService | 文档 |
|--------|------|--------|------|----------------|----------------|------|
| `icec-cloud-life-cs` | 客服域（工单/会话/坐席/状态机/语音）| `com.casstime.cloud.life.cs` | service | `CsTicketServiceImpl` / `CsConversationServiceImpl` | `CsTicketAppService` / `CsConversationAppService` | §4 |
| `icec-cloud-life-im` | IM 域（会话/消息/融云/参与者）| `com.casstime.cloud.life.im` | service | `ImSessionServiceImpl` / `ImMessageServiceImpl` | `ImSessionAppService` / `ImMessageAppService` | §4 |
| `icec-cloud-life-im-bff` | IM BFF（APP/工作台聚合层）| `com.casstime.cloud.life.bff.im` | bff | `ImTokenRestImpl` / `CsConversationRestImpl` | `ImTokenAppService` / `CsConversationAppService` | §4 BFF 节 |
| `icec-cloud-life-user` | 2C 用户域（service+web 双模块）| `com.casstime.cloud.life.user` | service | `PersonalController` / `UserBindingVehicleController` | `UserAppService` / `PersonalAppService` | §2 |
| `icec-cloud-life-vehicle` | 车辆域 | `com.casstime.cloud.life.vehicle` | service | — | — | §2 |
| `icec-cloud-life-vehicle-bff` | 车辆 BFF | `com.casstime.cloud.life.bff.vehicle` | bff | — | — | §2 |
| `icec-cloud-life-notification` | 通用通知（极光/融云/内部事件）| `com.casstime.cloud.life.notification` | service | — | — | §2 |
| `icec-cloud-life-spi` | SPI 聚合父工程（11 子模块）| `com.casstime.cloud.life.spi` | spi | — | — | §7 |
| `icec-cloud-life-api` | API 聚合父工程（6 BFF-api 子模块）| `com.casstime.cloud.life.api` | api | — | — | §7 |

---

## §C 字段索引（核心表关键字段）

### CS 域核心表

**cs_ticket（工单表）**
- `id`, `created_by`, `created_date`, `last_updated_by`, `last_updated_date`
- `status`（工单状态，状态机驱动）
- `assigned_at`（坐席分配时间；**注意：** 超时计时起点从此字段开始，不是 created_date）
- `conversation_id`, `session_id`

**cs_conversation（会话表）**
- `id`, `ticket_id`, `status`, `created_date`

**cs_user（坐席用户表）**
- `id`, `user_id`, `status`（坐席在线/离线状态）

### IM 域核心表

**im_session（会话表）**
- `id`, `created_date`, `last_message_at`

**im_session_participant（会话参与者表）**
- `id`, `session_id`, `user_id`, `role`

**im_message（消息表）**
- `id`, `session_id`, `sender_id`, `message_type`, `body`, `sent_at`

---

## §D 组件索引（公共组件）

| 组件 | 位置 | 用途 | 使用方 |
|------|------|------|--------|
| `ApiResult<T>` | `icec-cloud-life-api/icec-cloud-life-api-common` | 统一返回包装 | 所有 BFF/Service |
| `PagedModels<T>` | `icec-cloud-life-api/icec-cloud-life-api-common` | 分页返回包装 | BFF 列表接口 |
| `ServiceProviderConstants` | 各 `icec-cloud-life-{domain}-spi` 子模块 | 服务名常量 | Feign Client @FeignClient(name=...) |
| `TokenService` | `icec-cloud-boss-security`（注：安全组件在 boss 工程）| 获取当前用户 ID | BFF 层 |
| `JsonUtils` | `com.casstime.commons.utils` | JSON 序列化 | 所有模块 |

---

## §E API 索引（关键跨服务 Feign/SPI）

| 服务 | SPI 接口 | 包路径 | 消费方 |
|------|---------|--------|-------|
| `life-cs-service` | `CsTicketService` | `com.casstime.cloud.life.spi.cs.service` | boss-agent-workbench-bff |
| `life-cs-service` | `CsConversationService` | `com.casstime.cloud.life.spi.cs.service` | im-bff |
| `life-im-service` | `ImMessageService` | `com.casstime.cloud.life.spi.im.service.message` | cs, im-bff |
| `life-im-service` | `ImSessionService` | `com.casstime.cloud.life.spi.im.service.session` | cs, im-bff |
| `life-im-service` | `ImTokenService` | `com.casstime.cloud.life.spi.im.service.token` | cs, im-bff |
| `life-user-service` | `UserBindingVehicleService`（推测）| `com.casstime.cloud.life.spi.user.service` | cs |
| `life-notification-service` | `NotificationService`（推测）| `com.casstime.cloud.life.spi.notification.service` | cs |

---

## §F 关键词反向索引

| 关键词 | 查哪里 | 说明 |
|--------|--------|------|
| 工单状态机 | §4 Domain Service 节 / §C | `CsTicketDomainService` |
| 融云回调 | §4 Interfaces 层 / §6.9 | `ImMessageEventNotifyServiceImpl` in im; `ImMessageEventNotifyClient` feign to cs |
| 超时任务 | §4 Interfaces JobHandler / §C | `TimeoutWaitingJobHandler` / `TimeoutAgentJobHandler` |
| BFF 认证 | §6.6 | Cookie `security_context`; `TokenService.getCurUserId()` |
| 分页 | §6.4 | `PageRequest<T>` + `PagedModels<T>` |
| 错误码 | §6.4 | life 域 16000-23999 |
| Feign 服务名 | §7.1 | `ServiceProviderConstants` |
| 异步通知线程池 | §6.9 | `csNotificationExecutor` core=4/max=8 |
| @UtilityClass | §4 Converter 节 / §5 | cs 域 Converter 已确认使用 |

---

## §G 资产读取 API（调用方式说明）

```text
# 读取整份资产（Code Plan 文首引用）
assets.outline()     → §A 大纲（各章速查）
assets.module(name)  → §B 模块索引中对应行 + §4 该域包路径
assets.table(name)   → §C 字段索引中对应表的关键字段
assets.spi(domain)   → §7.1 SPI 清单中该域的行
assets.naming(role)  → §5 命名约定中该类角色的行
assets.port(service) → §2 微服务清单中的 port/contextPath

# 典型 Code Plan 使用模式：
1. 引用 §1 确认 projectKey / gitPath
2. 引用 §3-§4 确认分层包路径
3. 引用 §5 确认类命名
4. 引用 §6 做约束合规自审
5. 引用 §7 确认跨服务 Feign 服务名
```

---

## 附录 A：本项目 JSON 实例（机器可读）

```json
{
  "meta": {
    "projectKey": "icec-cloud-life",
    "projectName": "icec-cloud-life",
    "gitPath": "D:\\Item\\life",
    "sourceRoot": "D:\\Item\\life\\2c\\",
    "productLine": ["life"],
    "profile": ["dev", "test", "prod", "beta-kunlun"],
    "packaging": "jar",
    "portRange": "6001-6100 (web/api/spi) / 10086-10097 (BFF) / 20081-20093 (service)",
    "lastAuditedAt": "2026-06-17",
    "isGitRepo": false,
    "hasMavenRootPom": false
  },
  "microservices": [
    { "name": "icec-cloud-life-cs", "responsibility": "客服域", "port": 20092, "contextPath": null, "type": "service", "basePackage": "com.casstime.cloud.life.cs", "submodules": ["domain","application","interfaces","infrastructure","service"] },
    { "name": "icec-cloud-life-im", "responsibility": "IM 域", "port": 20093, "contextPath": null, "type": "service", "basePackage": "com.casstime.cloud.life.im", "submodules": ["domain","application","interfaces","infrastructure","service"] },
    { "name": "icec-cloud-life-im-bff", "responsibility": "IM BFF", "port": 10096, "contextPath": "/life-im-bff", "type": "bff", "basePackage": "com.casstime.cloud.life.bff.im", "submodules": ["single-module"] },
    { "name": "icec-cloud-life-user", "responsibility": "2C 用户域", "port": [6602, 6001], "contextPath": [null, "/user-api"], "type": "service", "basePackage": "com.casstime.cloud.life.user", "submodules": ["domain","application","infrastructure","web-api","web","service"] },
    { "name": "icec-cloud-life-vehicle", "responsibility": "车辆域", "port": 20087, "type": "service", "basePackage": "com.casstime.cloud.life.vehicle" },
    { "name": "icec-cloud-life-vehicle-bff", "responsibility": "车辆 BFF", "port": 10087, "contextPath": "/vehicle-bff", "type": "bff", "basePackage": "com.casstime.cloud.life.bff.vehicle" },
    { "name": "icec-cloud-life-notification", "responsibility": "通用通知", "port": [6605, 6005], "contextPath": [null, "/notification-api"], "type": "service", "basePackage": "com.casstime.cloud.life.notification" },
    { "name": "icec-cloud-life-spi", "responsibility": "SPI 聚合", "port": 6099, "type": "spi", "submodules": ["cs-spi","im-spi","user-spi","vehicle-spi","notification-spi","ops-notification-spi","passport-spi","touchpoint-spi","captcha-spi","boss-abnormal-spi","integration-event"] },
    { "name": "icec-cloud-life-api", "responsibility": "API 聚合", "port": 6100, "type": "api" }
  ],
  "openGaps": [
    "life-obs 端口 6602 与 life-user-service 疑似冲突，待核实",
    "各域 SPI ServiceProviderConstants 字段值部分为推测，待验证",
    "life-auth-bff 认证流程细节未探查",
    "life-user/vehicle/workticket 等域的 DDD 包路径未逐一验证"
  ]
}
```

---

## 维护

- **维护人：** 架构组 + 各域负责人（life-cs：{owner-cs} / life-im：{owner-im} / life-user：{owner-user} / ...）
- **更新频率：** 每月审计一次；新增微服务/分层调整时立即更新
- **同步对象：** ① 本项目所有 Story/Code Plan 编写者（强制引用本文件）② 与 icec-cloud-boss.assets.md 中的 life 域章节保持一致
- **探查历史：**
  - `2026-06-17`：首版，多 Agent 并行探查（冰山模块：cs + im；其余域元信息级别）
```
