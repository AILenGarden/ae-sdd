---
name: icec-cloud-life-cs-project-assets
description: icec-cloud-life-cs 工程级项目资产 — 按 schema v3（2026-06-26）工程级粒度拆分 SOP 生成。客服域 Service：工单状态机（WAITING/IN_PROGRESS/RESOLVED/UNRESOLVED/TIMEOUT_CLOSED）+ 超时自动结束 + Copilot 双向同步 + 融云在线状态同步 + 三端通知 + 坐席负载管理 + 语音外呼。本文是 schema §15 + 附录 B 的第 2 份工程级子文件实例。
parent: icec-cloud-life.assets.md
探查时间: 2026-06-26
探查来源: 同事 D:\Item\life\document\life-team-project-docs\knowledge\project\icec-cloud-life-cs.md (110KB / 1510 行) + ae-sdd life.assets.md §4 DDD 落点
---

# icec-cloud-life-cs 工程级项目资产

> **本文是 `icec-cloud-life.assets.md` 的工程级子文件**（schema §15），仅含本工程细节。跨工程信息见主体。
>
> **范本定位**：本文是 schema §15 工程级拆分 + §1.2 部署信息 + §14 安全提示 + §5 核心类方法级 + 可信度三态标注 + **第 2 份实例**（第 1 份是 boss-user）。重点展示客服域特有的"状态机 + 超时 + 异步 + 分布式锁"等复杂业务场景如何落表。

---

## 0. 摘要与使用场景 [已确认]

| 维度 | 内容 |
|------|------|
| 工程名 | `icec-cloud-life-cs` |
| 父工程 | `icec-cloud-life` |
| 探查时间 | 2026-06-26 |
| 工程定位 | 客服域 Service（工单/会话/坐席/状态机/通知编排/Copilot AI 同步/融云在线状态/语音外呼） |
| 关键不变量 | 本工程不重复定义 rules；只把 rules 映射到本工程代码 |
| 最后审计 | 2026-06-26 |
| 关键架构特性 | 🔴 Spring StateMachine（工单+会话两套）+ Redisson 分布式锁（user/ticket 维度）+ @Async + @Retryable + xxl-job 定时任务 + 三端通知（APP 极光/工作台融云/copilot-server）|

---

## 1. 模块元信息 [已确认]

| 字段 | 值 |
|------|---|
| moduleName | `icec-cloud-life-cs` |
| groupId | `com.casstime.cloud` |
| artifactId | `icec-cloud-life-cs` |
| version | `1.0-SNAPSHOT` |
| packaging | `pom（聚合父工程）` |
| 子模块数 | 5 个（domain / application / interfaces / infrastructure / service）|
| 主启动类 | `icec-cloud-life-cs-service/src/main/java/com/casstime/cloud/life/cs/WebApplication.java`（`@SpringBootApplication + @EnableAspectJAutoProxy + @EnableFeignClients + @EnableAsync + @EnableRetry`）|
| profile | `dev, test, prod, beta-kunlun` |
| port | `20092`（HTTP）/ `10092`（xxl-job 执行器）|
| contextPath | — |
| dependsOnSpi | [`icec-cloud-life-cs-spi 1.1`, `icec-cloud-life-im-spi 1.1`] |
| lastAuditedAt | `2026-06-26` |
| owner | 架构组 + life-cs 域负责人 |

### 1.1 部署信息 [已确认]

> 🆕 2026-06-26 schema §1.2 新增节。本节从 `icec-cloud-life-cs-service/src/main/resources/bootstrap.yml` + service pom 抽取。

| 字段 | 值 | 抽取来源 |
|------|---|---------|
| `profile.active` | `beta-kunlun` | `bootstrap.yml` `spring.profiles.active` |
| `db.driverClassName` | `com.mysql.cj.jdbc.Driver` | `bootstrap.yml` |
| `db.type` | `com.zaxxer.hikari.HikariDataSource` | `bootstrap.yml` |
| `db.urlTemplate` | `jdbc:mysql://${icec.database.servers}/${icec.database.dbname}?useUnicode=true&characterEncoding=utf-8&allowMultiQueries=true&autoReconnect=true` | `bootstrap.yml` |
| `db.username/password` | `${icec.database.username}` / `${icec.database.password}`（配置中心占位符）| `bootstrap.yml` |
| `db.pool` | HikariCP max-pool-size=20 / min-idle=1 / connection-timeout=30000ms / idle-timeout=600000ms / max-lifetime=1800000ms / connection-test-query=SELECT 1 | `bootstrap.yml` |
| `redis.database` | `0` | `bootstrap.yml` |
| `redis.address` | `redis-...dcs.huaweicloud.com:6379`（华为云 DCS）| `bootstrap.yml` |
| `redis.password.inConfig` | 🔴 **true — bootstrap.yml 中明文 `****`（高危）** | `bootstrap.yml` `spring.redis.password` |
| `redis.pool` | max-active=50 / max-idle=5 / min-idle=1 / max-wait=10000ms | `bootstrap.yml` |
| `feign.connectTimeoutMillis` | `1000` | `bootstrap.yml` |
| `feign.pool.maxConnTotal` | `1000` | `bootstrap.yml` |
| `feign.pool.maxConnPerRoute` | `100` | `bootstrap.yml` |
| `feign.hystrix.enabled` | `false`（关闭 Hystrix 熔断）| `bootstrap.yml` |
| `panda.config.server-addr` | `http://panda.casstime.com` | `bootstrap.yml` |
| `panda.config.product` | `b2c` | `bootstrap.yml` |
| `panda.config.instance` | `life-cs-service` | `bootstrap.yml` |
| `gateway` | `http://life-hwbeta-api-penglai.intra.casstime.com`（icec.api.agent）| `bootstrap.yml` |
| `imageRepo` | `registry.cn-shenzhen.aliyuncs.com/cassmall/${project.name}` tag=`${project.version}` | `service/pom.xml` `dockerfile-maven-plugin` |
| `nexusRepo` | `http://dev.casstime.com/nexus/content/groups/public/`（cass-public）| `pom.xml` repositories |
| `encrypt.failOnError` | `false`（配置解密失败不中断启动）| `bootstrap.yml` |
| `xxl-job.executor.appname` | `life-cs-executor` | `bootstrap.yml` `job.executor` |
| `xxl-job.executor.port` | `10092` | `bootstrap.yml` |
| `xxl-job.executor.addressList` | `xxl-job 集群地址（hwbeta-kunlun）` | `bootstrap.yml` |
| `management.port` | 未显式配置（默认 30000）| ⚠️ [据推断] |
| `coverageTool` | JaCoCo | `service/pom.xml` |

### 1.2 安全提示 [已确认/待确认]

> 🆕 2026-06-26 schema §14 新增节。

| ID | 类型 | 位置 | 风险等级 | 描述 | 建议修复 | 状态 |
|----|------|------|---------|------|---------|------|
| S-101 | 🔴 **明文密码** | `icec-cloud-life-cs-service/src/main/resources/bootstrap.yml` `spring.redis.password` | 🟠 高 | bootstrap.yml 中 Redis 密码直接是 `****` 明文（同事 life-cs.md §7.3 原文标注"当前为明文，建议迁移至配置中心/密钥管理 [安全提示]"）| 改为占位符 `${icec.redis.password}` 由配置中心注入；或迁移至密钥管理服务（如 AWS Secrets Manager）| **待修** |
| S-102 | Actuator 端点外露 | `bootstrap.yml` 未显式配置 `management.endpoints.web.exposure.include` | 🟡 中 | 未限制 Actuator 暴露端点；本地多服务同时启动可能端口冲突（management.port 未显式）| 显式配置 `management.endpoints.web.exposure.include: health,info` + `management.port: 30092` | 待修 |
| S-103 | Hystrix 关闭 | `bootstrap.yml` `feign.hystrix.enabled: false` | 🟢 低 | Hystrix 关闭意味着下游故障无熔断保护 | 评估风险后可选择性开启 `@HystrixCommand` 或迁移至 Sentinel/Resilience4j | 已记录 |
| S-104 | icec-cloud-life-cs-spi 依赖已注释 | `interfaces/pom.xml` | 🟢 低 | 同事 life-cs.md §1.2 标注 interfaces 模块已将该 spi 依赖注释屏蔽 | 确认是否需要删除注释或改用其他 SPI | 待确认 |
| S-105 | encrypt.failOnError: false | `bootstrap.yml` | 🟡 中 | 配置解密失败不中断启动，可能导致带敏感信息的配置在解密失败时被错误使用 | 评估后决定是否改为 `true` | 待确认 |

---

## 2. 子模块结构 [已确认]

| 模块 | ArtifactId | 打包 | 职责 | 主要依赖 |
|------|-----------|------|------|---------|
| 领域层 | `icec-cloud-life-cs-domain` | jar | 客服域核心业务模型与规则：工单聚合根（CsTicketDO 含 assignedAt / claimedAt / sessionId / lastMessageAt / claimedAt / startMessageId / endMessageId 等字段）、会话实体（CsConversationDO）、坐席实体（CsUserDO 含 assignmentVersion 乐观锁）、状态枚举（CsTicketStatusEnum: WAITING/IN_PROGRESS/RESOLVED/UNRESOLVED/TIMEOUT_CLOSED）、服务模式（ServiceModeEnum: HUMAN/AI）、通知事件策略表（NotificationEventTypeEnum）、通知负载（CsNotificationPayload）、AI 消息值对象（AiMessage）、**融云在线状态同步值对象（CsUserStatusRongyunSyncDO）**、坐席负载领域服务（CsAgentLoadDomainService）、工单领域服务（CsTicketDomainService）、路由领域服务（CsRoutingDomainService）；定义端口：CsTicket / CsConversation / CsAiSummary / CsUserRepository / **CsUserStatusRongyunSyncRepository** / CsNotificationGateway / ICopilotServerFacade / ImMessageFacade / CopilotMessage / CopilotService / ImSessionServiceFacade 等。不依赖外部业务模块 | `icec-cloud-commons`、`spring-context`、`commons-lang3 3.9` |
| 应用层 | `icec-cloud-life-cs-application` | jar | 用例编排（薄层）：CsConversationAppService（含 getAiSummary / getConversationOwner / getServiceStatus / **receiveAiMessages** C25 用例，委托 CsMessageHandler 处理 Copilot 主动推送）、CsTicketAppService（含 claimTicket / closeTicket / reopenTicket / getUserTickets / getTicketMessageRange，共享 CsTicketCloseOrchestrator 5步结单编排器）、CsTicketTimeoutAppService（STORY-020 IN_PROGRESS 双侧静默判定 + WAITING assignedAt 扫描 + Redis 提醒去重）、CsVoiceCall(Sync)AppService、CsUserAppService（含 getCsUserByUserId / updateOnlineStatus / **upsertRongyunOnlineStatus 单条与批量**，批量带 @Transactional）；编排器 CsTicketCloseOrchestrator（5步结单：状态机流转+字段写入+释放负载+同步会话+通知）；状态机驱动/监听 CsTicketStateMachineDriver / CsTicket/CsConversationStateMachineListener；处理器 CsMessageHandler（IM 消息事件 + copilot 双向同步 + **Copilot 主动推送接收 handleCopilotAiMessageReceive**）；CsAgentNameCache（Guava LoadingCache）；组件 CsDistributedLock（Redisson ticket/user 维度回调式锁） | `cs-domain`、`icec-cloud-life-cs-spi 1.1`、`icec-cloud-life-im-spi 1.1`、`icec-cloud-commons`、`redisson 3.5.7`、`spring-context`、`spring-tx` |
| 用户接口层 | `icec-cloud-life-cs-interfaces` | jar | 对外 HTTP 接口与 SPI 实现；xxl-job 任务处理器 TimeoutAgent/User/WaitingJobHandler（**禁用 `@Scheduled`**）；新增 Copilot AI 消息接收端点 + 融云状态同步端点 | `cs-application`、`icec-cloud-commons(b2c.1.0-SNAPSHOT)`、`spring-web` |
| 基础设施层 | `icec-cloud-life-cs-infrastructure` | jar | 端口实现：持久化（MyBatis-Plus + MySQL + HikariCP，CsTicketRepositoryImpl 含 assignedAt 查询扩展：findWaitingTimeoutTickets / findWaitingReminderTickets / findInProgressTimeoutTickets）、**CsUserStatusRongyunSyncRepositoryImpl**（融云状态快照 UPSERT，对应 cs_user_status_rongyun_sync 表，含 CsUserStatusRongyunSyncMapper / CsUserStatusRongyunSyncPO / CsUserStatusRongyunSyncPersistenceConverter）、**CsUserRepositoryImpl 扩展**（新增 findById 等方法支持融云回调链路）；Feign 客户端 CopilotServerClient / NotificationServiceClient / TqOutboundCallClient；防腐层实现 CopilotMessageFacadeImpl（@Async + @Retryable 3次/5s）、CopilotServiceFacadeImpl（增量游标分页拉取 AI 消息，支持 text/image/block/richtext/markdown 强类型映射，未知类型归档 UNKNOWN）、CopilotServerFacadeImpl（服务模式同步，重试 3次/3s）、ImMessageFacadeImpl、CsNotificationGatewayImpl + CsNotificationAsyncExecutor（@Async 双通道）、AbnormalCallSyncFacadeImpl | `cs-domain`、`cs-application`、`icec-cloud-life-im-spi 1.1`、`icec-cloud-spi-common`、`spring-cloud-starter-feign`、`spring-cloud-netflix-core`、`spring-boot-starter-data-redis`、`redisson 3.5.7`、`HikariCP 2.7.9`、`mysql-connector-java 8.0.17`、`mybatis-plus-boot-starter 3.3.2` |
| 启动/装配层 | `icec-cloud-life-cs-service` | jar | Spring Boot 应用入口（`WebApplication.java`，含 `@EnableAsync + @EnableRetry + @EnableFeignClients + @EnableAspectJAutoProxy`），聚合 interfaces + infrastructure，运行时配置（bootstrap.yml），Docker 打包（spring-boot-maven-plugin 2.7.5 repackage + dockerfile-maven-plugin 1.4.0） | `cs-interfaces`、`cs-infrastructure`、`spring-boot-starter-web/aop/cache`、`spring-cloud-starter-config/feign`、`spring-retry`、`panda 1.0.9 / casslog / cass-config / cassmetrics starter`、`mysql-connector-java 8.0.17` |

> **🔴 重要发现（STORY-020 重构）**：
> - 父 pom `<modules>` 声明 5 个子模块；`icec-cloud-life-cs-spi`（v1.1）和 `icec-cloud-life-im-spi`（v1.1）来源于外部 `icec-cloud-life-spi` 聚合工程，未纳入本父 pom
> - 原单体 `CsTicketStateMachineAppService`（应用层）与 `CsTicketStateMachineService`（领域层）**已删除**；状态机样板下沉为 `statemachine` 包的统一驱动组件 `CsTicketStateMachineDriver` + 两个 `*StateMachineListener`
> - 原 `CsNotificationConstants` 常量类**已删除**；三端通知重构为以 `NotificationEventTypeEnum` 为领域策略表
> - 工单 `lastSenderType` 字段**已移除**；待接待超时/提醒扫描字段由 `created_date` 改为 `assigned_at`（新增）

### 2.1 依赖层次关系 [已确认]

```
life-cs-service                  ← 启动装配（聚合 interfaces + infrastructure）
        ↓
        ├──→ icec-cloud-life-cs-interfaces
        │             ↓
        │             └──→ icec-cloud-life-cs-application
        │                          ↓
        │                          ├──→ icec-cloud-life-cs-domain
        │                          └──→ (external) icec-cloud-life-cs-spi 1.1
        │                                            ↑
        └──→ icec-cloud-life-cs-infrastructure ───┘ (持久化 / 远程 / 缓存 / 防腐层)
                          ↑
                          ├──→ icec-cloud-life-cs-domain (Repository 接口实现)
                          ├──→ icec-cloud-life-cs-application (Facade 接口实现)
                          └──→ (external) icec-cloud-life-im-spi 1.1
```

> 依赖方向符合 DDD：domain 为最内层（无业务模块依赖，仅 commons/Spring 基础包），application 依赖 domain，interfaces 依赖 application，infrastructure 依赖 domain/application，service 装配 interfaces + infrastructure。

---

## 3. 完整技术栈版本号 [已确认]

> 🆕 2026-06-26 schema §6.8 新增节。**完整 7 张表见主体 §6.8**，本节只列本工程特有的依赖。

| 依赖 | 版本 | 用途 | 来源 |
|------|------|------|------|
| `spring-statemachine` | 跟随 Spring Boot | 🔴 工单/会话两套状态机流转 | `icec-cloud-commons` 传递依赖 |
| `redisson` | `3.5.7` | 🔴 分布式锁（CsDistributedLock ticket/user 维度）| application/infrastructure pom |
| `spring-retry` | 跟随 Spring Boot | @Retryable 重试（Copilot 推送 3次/5s；CopilotServerFacade 3次/3s）| service pom |
| `job-spring-boot-starter` | 跟随 | 🔴 xxl-job 执行器（替代 `@Scheduled`，**禁用 `@Scheduled`**）| service pom |
| `guava` | 跟随 | CsAgentNameCache（LoadingCache 坐席姓名缓存）| application pom |
| `assertj-core` | `3.11.1` | 测试断言（fluent API）| application pom |
| `spring-boot-maven-plugin` | `2.7.5` | Fat JAR 打包（repackage 目标）| service pom |
| `dockerfile-maven-plugin` | `1.4.0` | Docker 镜像构建（spotify）| service pom |
| `panda-spring-boot-starter` | `1.0.9` | 配置中心 | service pom |
| `casslog-spring-boot-starter` | 跟随 | 日志组件 | service pom |
| `cass-config-spring-boot-starter` | 跟随 | 公司内部配置组件 | service pom |
| `cassmetrics-spring-boot-v1-starter` | 跟随 | 监控指标采集 | service pom |
| `junit` / `mockito-core` | 4.12 / 1.10.19 | 测试框架 | application pom |
| `JaCoCo` | 跟随 | 覆盖率插件 | service pom |
| `lombok` | `1.18.16` | @Data / @RequiredArgsConstructor / @Slf4j | 各模块 pom |
| `commons-lang3` | `3.9` | 字符串/通用工具 | domain pom |

---

## 4. DDD 内部分层落点 [已确认]

> **详细类角色映射见主体 §4**；本节只列本工程内实际类。

| 类角色 | 精确包路径 | 典型类名（已确认）|
|--------|-----------|------------------|
| **Interfaces** | | |
| Rest 实现类 | `icec-cloud-life-cs-interfaces/src/main/java/com/casstime/cloud/life/cs/interfaces/restful/` | `CsConversationServiceImpl`（含 Copilot AI 消息接收端点）/ `CsUserServiceImpl`（含融云状态同步单条/批量端点）|
| xxl-job Handler | `icec-cloud-life-cs-interfaces/src/main/java/com/casstime/cloud/life/cs/interfaces/job/` | `TimeoutAgentJobHandler` / `TimeoutUserJobHandler` / `TimeoutWaitingJobHandler` |
| GlobalExceptionHandler | `icec-cloud-life-cs-interfaces/src/main/java/com/casstime/cloud/life/cs/interfaces/config/` | `GlobalExceptionHandler` |
| **Application** | | |
| AppService | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/` | `CsConversationAppService`（6 方法）/ `CsTicketAppService`（8 方法）/ `CsTicketTimeoutAppService`（3 方法）/ `CsUserAppService`（6 方法）/ `CsVoiceCallAppService` / `CsVoiceCallSyncAppService` |
| Orchestrator | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/orchestrator/` | `CsTicketCloseOrchestrator`（5步结单编排器）|
| State Machine | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/statemachine/` | `CsTicketStateMachineDriver`（统一驱动）/ `CsTicketStateMachineListener` / `CsConversationStateMachineListener` |
| Handler | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/handler/` | `CsMessageHandler`（🆕 IM 消息事件 + Copilot 双向同步 + 主动推送接收）|
| Component | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/component/` | `CsDistributedLock`（Redisson 回调式锁）/ `CsAgentNameCache`（Guava LoadingCache）|
| Converter | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/converter/` | `CsTicketConverter`（`@UtilityClass`）/ `CopilotMessageConverter` |
| **Domain** | | |
| Domain Object | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/model/entity/` | `CsTicketDO`（工单聚合根）/ `CsConversationDO` / `CsUserDO` / **`CsUserStatusRongyunSyncDO`**（融云状态快照）/ `CsAiSummaryDO` / `VoiceCallRecordDO` |
| Enum | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/model/enums/` | `CsTicketStatusEnum`（WAITING/IN_PROGRESS/RESOLVED/UNRESOLVED/TIMEOUT_CLOSED）/ `ServiceModeEnum`（HUMAN/AI）/ `OnlineStatusEnum` / `NotificationEventTypeEnum`（三端通知策略表）/ `AiMessageTypeEnum`（text/image/block/richtext/markdown/UNKNOWN）|
| Error Code | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/model/enums/error/` | `CsUserErrorCode` / `CsTicketErrorCode`（16000-16999 客服段，详见主体 §6.4）|
| Repository 接口 | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/repository/` | `CsTicketRepository` / `CsConversationRepository` / `CsAiSummaryRepository` / `CsUserRepository` / **`CsUserStatusRongyunSyncRepository`** / `CsTicketSearchIndexRepository` |
| Domain Service | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/service/` | `CsTicketDomainService` / `CsRoutingDomainService`（路由领域服务）/ **`CsAgentLoadDomainService`**（坐席负载管理 BR-02 + assignment_version 乐观锁）|
| Facade（接口） | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/facade/` | `ICopilotServerFacade` / `CopilotMessageFacade`（@Async + @Retryable）/ `CopilotServiceFacade` / `ImMessageFacade` / `ImSessionServiceFacade` / `CsNotificationGateway`（三端通知网关）/ `CsTicketSearchIndexFacade`（ES 索引）/ `ImIdentityFacade` |
| Exception | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/exception/` | `CsUserDomainException` / `CsTicketDomainException` |
| Event | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/event/` | `CsTicketCreatedEvent` / `CsTicketClaimedEvent` / `CsTicketClosedEvent` / `CsTicketReopenedEvent`（[据推断]）|
| **Infrastructure** | | |
| Facade 实现 | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/facade/` | `CopilotMessageFacadeImpl`（@Async + @Retryable 3次/5s）/ `CopilotServiceFacadeImpl`（增量游标分页 + 强类型映射）/ `CopilotServerFacadeImpl`（重试 3次/3s）/ `ImMessageFacadeImpl` / `CsNotificationGatewayImpl` / `CsNotificationAsyncExecutor`（@Async 双通道）/ `AbnormalCallSyncFacadeImpl` |
| Feign Client | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/feign/` | `CopilotServerClient`（POST `/api/copilot/v1/session/service-mode`）/ `NotificationServiceClient` / `TqOutboundCallClient` |
| PO | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/persistence/entity/` | `CsTicketPO`（`@TableName("cs_ticket")` + assignedAt 字段）/ `CsConversationPO` / `CsUserPO`（`@TableName("cs_user")` + IdType.ASSIGN_UUID）/ **`CsUserStatusRongyunSyncPO`** / `CsAiSummaryPO` / `VoiceCallRecordPO` |
| Mapper | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/persistence/dao/mapper/` | `CsTicketMapper` / `CsConversationMapper` / `CsUserMapper` / **`CsUserStatusRongyunSyncMapper`**（含 upsert / batchUpsert）|
| DataConverter | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/persistence/converter/` | `CsTicketPersistenceConverter` / `CsConversationPersistenceConverter` / `CsUserPersistenceConverter` / **`CsUserStatusRongyunSyncPersistenceConverter`** |
| Repository Impl | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/persistence/repository/mysql/` | `CsTicketRepositoryImpl`（**STORY-020 重构**：findWaitingTimeoutTickets / findWaitingReminderTickets / findInProgressTimeoutTickets；移除 lastSenderType）/ `CsConversationRepositoryImpl`（**ACTIVE_STATUSES 由 [WAITING, IN_PROGRESS] 缩减为仅 [IN_PROGRESS]**）/ `CsUserRepositoryImpl`（**新增 findById 支持融云回调**）/ **`CsUserStatusRongyunSyncRepositoryImpl`**（UPSERT 批量）|
| **SPI（被消费方）** | | |
| SPI Service 接口 | `icec-cloud-life-spi/icec-cloud-life-cs-spi/src/main/java/com/casstime/cloud/life/spi/cs/service/` | `CsUserService` / `CsTicketService` / `CsConversationService` / `CsAiSummaryService` |
| DTO | `icec-cloud-life-spi/icec-cloud-life-cs-spi/src/main/java/com/casstime/cloud/life/spi/cs/dto/` | `CsUserDTO` / `CsTicketDTO` / `CsConversationDTO` / `CsAiSummaryDTO` |
| Request | `icec-cloud-life-spi/icec-cloud-life-cs-spi/src/main/java/com/casstime/cloud/life/spi/cs/request/` | `UpsertRongyunOnlineStatusRequest` / `CopilotAiMessageReceiveRequest` |
| Constants | `icec-cloud-life-spi/icec-cloud-life-cs-spi/src/main/java/com/casstime/cloud/life/spi/cs/` | `ServiceProviderConstants`（`LIFE_CS_SERVICE = "life-cs-service"`）|

---

## 5. 核心类方法级实现（🆕 2026-06-26）

> **🆕 范本节**：客服域作为状态机 + 异步 + 分布式锁的代表域，重点展示 AppService 编排器 + 状态机驱动 + 防腐层 + 仓储扩展的方法级实现（来源：同事 life-cs.md §7.4 应用服务层 + §7.5 领域层新增组件 + §2.2 各层详细说明）。

### 5.1 Converter 层 [已确认]

#### CopilotMessageConverter [🆕]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/converter/CopilotMessageConverter.java` |
| 职责 | 🆕 Copilot 消息转换器；负责 Copilot SPI 请求对象（`CopilotAiMessageReceiveRequest` 等）→ 领域层 / IM SPI 消息 DTO 的双向映射 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| （具体方法待探查）| — | — | [待确认] Copilot 消息格式 ↔ IM 消息格式转换；强类型映射 text/image/block/richtext/markdown，未知类型归档 UNKNOWN |

#### CsTicketConverter [据推断]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/converter/CsTicketConverter.java` |
| 职责 | 工单 DO ↔ DTO 转换；`@UtilityClass` 静态方法 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| （具体方法待探查）| — | — | [据推断] 含 toDTO / toDO / 时间字符串化 |

### 5.2 AppService 层 [已确认]

#### CsConversationAppService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/CsConversationAppService.java` |
| 职责 | 会话域应用服务；编排会话查询、工单流转、AI 摘要、Copilot AI 消息接收 |

**核心字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `csConversationRepository` | `CsConversationRepository` | 会话持久化仓储 |
| `csTicketRepository` | `CsTicketRepository` | 工单持久化仓储 |
| `csUserRepository` | `CsUserRepository` | 坐席用户仓储 |
| `csAiSummaryRepository` | `CsAiSummaryRepository` | AI 摘要仓储 |
| `imIdentityFacade` | `ImIdentityFacade` | IM 身份门面 |
| `imSessionServiceFacade` | `ImSessionServiceFacade` | IM 会话门面 |
| `csTicketSearchIndexFacade` | `CsTicketSearchIndexFacade` | 工单搜索索引门面（ES）|
| `csMessageHandler` | `CsMessageHandler` | 🆕 Copilot AI 消息处理器 |

**核心方法：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `getConversationHeader` | `String sessionId` | `ConversationHeaderDTO` | 获取会话头部轻量信息 |
| `getConversations` | `GetConversationsRequest` | `ConversationPageDTO` | 分页查询会话列表 |
| `getConversationPermission` | `String sessionId, String userId` | `ConversationPermissionDTO` | 查询会话操作权限 |
| `getServiceStatus` | `String userId` | `ServiceStatusDTO` | 查询坐席服务状态 |
| `getAiSummary` | `String sessionId` | `CsAiSummaryDTO` | 查询 AI 摘要（STORY-017）|
| `receiveAiMessages` 🆕 | `CopilotAiMessageReceiveRequest request` | `void` | 🔴 C25 用例：接收 Copilot 主动推送的 AI 阶段消息，委托 `CsMessageHandler#handleCopilotAiMessageReceive` 转换并 UPSERT 写入 IM 消息存储；**失败时异常向上传播由接口层返回 5xx 供 Copilot 重试** |

#### CsUserAppService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/CsUserAppService.java` |
| 职责 | 坐席用户域应用服务；负责坐席查询、在线状态变更、融云在线状态同步写入 |

**核心字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `ONLINE_STATUS`（常量）| `String` | 🆕 值 `"ONLINE"` |
| `csUserRepository` | `CsUserRepository` | 坐席用户仓储 |
| `csUserStatusRongyunSyncRepository` | `CsUserStatusRongyunSyncRepository` | 🆕 融云状态快照仓储 |

**核心方法：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `getCsUserByUserId` | `String userId` | `CsUserDTO` | 按用户 ID 查询坐席信息 |
| `getCsUserListByUserIds` | `List<String> userIds` | `List<CsUserDTO>` | 批量查询坐席信息 |
| `updateOnlineStatus` | `String bossUserId, String status` | `void` | 变更坐席在线状态；内部复用 `validateOnlineStatus` 校验 |
| `upsertRongyunOnlineStatus`（单条）🆕 | `UpsertRongyunOnlineStatusRequest request` | `void` | 🔴 单条融云在线状态同步兼容入口：校验 → 查坐席 → upsert 快照表 → 更新 online_status |
| `upsertRongyunOnlineStatus`（批量）🆕 | `List<UpsertRongyunOnlineStatusRequest> requests` | `void` | 🔴 `@Transactional`；按 `cs_user_id` 去重保留最后一条 → 批量写快照表 → 批量更新 online_status |
| `validateOnlineStatus`（私有）| `String status` | `void` | 校验状态合法性（`OnlineStatusEnum.isValid`）；非法时抛 `CsUserDomainException(INVALID_ONLINE_STATUS)` |

#### CsTicketAppService [已确认（核心方法）]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/CsTicketAppService.java` |
| 职责 | 工单全生命周期用例（创建/路由、接单、结单、重开、超时）收敛；委托 CsTicketCloseOrchestrator 结单编排 |
| 事务 | `@Transactional` 在 claimTicket / reopenTicket / closeTicket 等方法 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `createAndRoute` | `CreateTicketRequest request` | `Long ticketId` | 创建工单并路由分配（user 维度分布式锁）[据推断] |
| `claimTicket` 🔴 | `Long ticketId, String csUserId` | `void` | 🔴 坐席接单（WAITING→IN_PROGRESS，**事务内双状态机流转**，**事务后同步 AI 消息**）|
| `closeTicket` | `Long ticketId, String operatorId, String reason, String note` | `void` | 结单；委托 `CsTicketCloseOrchestrator` 5步编排（状态机流转+字段写入+释放负载+同步会话+通知）|
| `reopenTicket` 🔴 | `Long ticketId` | `void` | 工单重开（user 维度分布式锁）|
| `getUserTickets` | `String userId, PagedRequest page` | `PagedModels<CsTicketDTO>` | 历史工单游标分页 |
| `getTicketMessageRange` | `Long ticketId, Long startMsgId, Long endMsgId` | `List<CsMessageDTO>` | 工单消息范围查询 |
| `syncLastMessageAtFromIm` 🆕 | `Long ticketId` | `void` | 🔴 STORY-020 重构：从 IM 同步最后消息时间；单调写入（WHERE last_message_at < 新值）|
| （其他方法待探查）| — | — | [待确认] |

#### CsTicketTimeoutAppService [已确认（STORY-020 重构）]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/CsTicketTimeoutAppService.java` |
| 职责 | 🔴 超时自动结束（STORY-020 重构核心）；返回 `TimeoutScanResult`（scanned/success/failed）；分布式锁串行化每个工单的关单操作 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `scanInProgressTimeout` | 无 | `TimeoutScanResult` | 🔴 接待中 15min 任一侧静默 → 批量查 IM 双侧末消息时间 → 内存判定"球在谁那"决定责任方 → 关单 |
| `scanWaitingTimeout` | 无 | `TimeoutScanResult` | 🔴 待接待 30min 未接单关闭；基于 `assignedAt` 扫描（SQL 层 `IS NOT NULL` 过滤历史工单）；关单前同步 AI 阶段消息 |
| `scanWaitingReminder` | 无 | `TimeoutScanResult` | 🔴 待接待 5min 二次提醒；基于 `assignedAt` 窗口 + Redis SETNX 去重 → 走 IM 系统消息通道 |

#### CsTicketCloseOrchestrator [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/orchestrator/CsTicketCloseOrchestrator.java` |
| 职责 | 🔴 5步结单编排器；坐席主动结单与超时关单共用 |

| 步骤 | 动作 |
|------|------|
| 1 | 状态机流转（IN_PROGRESS → RESOLVED / UNRESOLVED / TIMEOUT_CLOSED）|
| 2 | 字段写入（finished_at / closed_by / closed_reason / close_note / last_message_at）|
| 3 | 释放坐席负载（`CsAgentLoadDomainService#releaseLoad`，更新 `assignment_version`）|
| 4 | 同步会话状态（`CsConversationRepositoryImpl#updateStatus`）|
| 5 | 三端通知（`CsNotificationGateway#notify`，经 `NotificationEventTypeEnum` 策略表路由到 APP 极光 / 工作台融云 / copilot-server）|

#### CsMessageHandler [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/handler/CsMessageHandler.java` |
| 职责 | 🆕 Copilot AI 消息处理器；接收 IM 消息事件 + Copilot 双向同步 + 主动推送接收 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `handleImMessage` | `ImMessageEvent event` | `void` | 处理 IM 消息事件；维护工单 `last_message_at`；携带 CS_USER 消息时从 `CsAgentNameCache` 补充坐席姓名后推送 copilot [据推断] |
| `syncAiMessagesFromCopilot` | `Long ticketId, String lastSourceEventId` | `void` | 转人工/接单/重开时增量拉取 AI 阶段历史消息；以 IM 中最近 AI 消息的 `sourceEventId` 为游标分页拉取 → 写回 IM [据推断] |
| `handleCopilotAiMessageReceive` 🆕 | `CopilotAiMessageReceiveRequest request` | `void` | 🔴 C25 用例：将 Copilot 主动推送的 AI 阶段消息通过 `CopilotMessageConverter` 转换后写入 IM 消息存储（UPSERT）；**失败异常向上传播供 Copilot 重试** |

### 5.3 Domain 层 [已确认/据推断]

#### CsTicketDO [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/model/entity/CsTicketDO.java` |
| 职责 | 🔴 工单聚合根；STORY-020 重构后已无 `lastSenderType` 字段，新增 `assignedAt` 字段 |
| 类型 | 充血模型（业务方法不以 get/set 开头）|

**核心字段（从 CsTicketPO 映射 + 业务扩展）：**

| 字段名 | 类型 | 说明 [已确认/据推断] |
|--------|------|-------------------|
| `id` | `Long` | 主键 [已确认] |
| `conversationId` / `sessionId` | `Long` | 关联会话 / IM 会话 ID [已确认] |
| `requesterPrincipalType` / `requesterPrincipalId` | `String` | 发起方主体类型 / ID [已确认] |
| `csUserId` / `claimedCsUserId` | `String` | 路由坐席 / 接单坐席 ID [已确认] |
| `status` | `String` | 工单状态（WAITING / IN_PROGRESS / RESOLVED / UNRESOLVED / TIMEOUT_CLOSED）[已确认] |
| `title` / `description` / `extraAttribute` | `String` | 标题 / 描述 / 扩展属性 [已确认] |
| `closedReason` / `closeNote` / `closedBy` | `String` | 结单原因 / 备注 / 操作人 [已确认] |
| `claimedAt` | `Date` | 接单时间（接待中超时扫描窗口起点）[已确认] |
| `assignedAt` 🆕 | `Date` | 🔴 派单时间（待接待超时扫描计时起点；`IS NOT NULL` 过滤历史无此字段工单）|
| `finishedAt` | `Date` | 结单时间 [已确认] |
| `lastMessageAt` | `Date` | 最后消息时间（消息钩子维护，供会话列表排序）[已确认] |
| `startMessageId` / `endMessageId` | `Long` | 本轮服务起止消息 ID [已确认] |
| 审计四字段 | `createdBy / createdDate / lastUpdatedBy / lastUpdatedDate` | 必备 [已确认] |

| 方法名 | 入参 | 返回 | 业务含义 [据推断] |
|--------|------|------|---------|
| `canClaim` | `String csUserId` | `boolean` | 校验可接单（当前状态 WAITING + 未超时 + 路由坐席匹配）[据推断] |
| `transitionToInProgress` | `String csUserId` | `void` | 状态机流转：WAITING → IN_PROGRESS；写 claimedAt [据推断] |
| `transitionToResolved` | — | `void` | 状态机流转：IN_PROGRESS → RESOLVED [据推断] |
| `transitionToTimeoutClosed` | — | `void` | 状态机流转：IN_PROGRESS → TIMEOUT_CLOSED；closedBy="system" [据推断] |

#### CsUserStatusRongyunSyncDO [已确认（🆕）]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/model/entity/CsUserStatusRongyunSyncDO.java` |
| 职责 | 🆕 融云在线状态同步领域实体；承载单次融云回调的状态快照数据，用于幂等 UPSERT 写入 |

**核心字段（[据命名/结构推断]）：**

| 字段名 | 类型 | 说明 [已确认/据推断] |
|--------|------|-------------------|
| `csUserId` | `String` | 坐席 ID（`cs_user.id`）[据推断] |
| `csUserOnlineStatus` | `String` | 在线状态（ONLINE / OFFLINE 等）[据推断] |
| `onlineDeviceCount` | `Integer` | 融云上报的在线设备数 [据推断] |
| `lastRongcloudOnlineAt` | `Date` | 最近一次融云上报为 ONLINE 的时间 [据推断] |
| `lastSeenAt` | `Date` | 融云上报的最后活跃时间 [据推断] |
| `platforms` | `String` | 融云上报的在线平台列表（JSON 或逗号分隔）[据推断] |
| 审计四字段 | — | 必备 [已确认] |

#### CsTicketStatusEnum [已确认]

| 错误码 / 枚举值 | 含义 |
|--------|------|
| `WAITING` | 待接待（已路由，等待坐席接单）|
| `IN_PROGRESS` | 接待中（坐席已接单，正在对话）|
| `RESOLVED` | 已解决（结单 + 已解决）|
| `UNRESOLVED` | 未解决（结单 + 未解决）|
| `TIMEOUT_CLOSED` | 超时自动关闭 |

> 错误码段位：16000-16999 客服段（详见主体 §6.4）。具体码值待 `CsTicketErrorCode` 探查。

#### CsAgentLoadDomainService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/service/CsAgentLoadDomainService.java` |
| 职责 | 🔴 坐席负载管理；BR-02 并发上限 + `assignment_version` 乐观锁 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `tryAcquireLoad` | `String csUserId` | `boolean` | 尝试占名额（CAS 更新 `assignment_version` + `current_load`，达到 `max_concurrency` 拒绝）[据推断] |
| `releaseLoad` | `String csUserId` | `void` | 释放名额（CAS 更新）[据推断] |

### 5.4 Infrastructure 层 [已确认]

#### CsTicketRepositoryImpl [已确认（STORY-020 重构）]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/persistence/repository/mysql/CsTicketRepositoryImpl.java` |
| 职责 | 🔴 工单仓储实现；STORY-020 重构扩展 `assignedAt` 查询 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `findWaitingTimeoutTickets` 🆕 | `Date assignedBefore` | `List<CsTicketDO>` | 🔴 待接待超时扫描：`status=WAITING AND assigned_at IS NOT NULL AND assigned_at < ?` |
| `findWaitingReminderTickets` 🆕 | `Date reminderWindowStart, Date reminderWindowEnd` | `List<CsTicketDO>` | 🔴 待接待二次提醒扫描：`status=WAITING AND assigned_at IN (?, ?)` |
| `findInProgressTimeoutTickets` 🆕 | `Date claimedBefore` | `List<CsTicketDO>` | 🔴 接待中超时扫描：`status=IN_PROGRESS AND claimed_at IS NOT NULL AND claimed_at < ?`（2026-06-05 补丁防御 claimed_at 为 null 的异常工单）|
| `findLatestBySessionId` 🆕 | `Long sessionId` | `CsTicketDO` | 🔴 按 sessionId 查最新工单（关单前对齐 last_message_at 用）[据推断] |
| `syncLastMessageAtFromIm` 🆕 | `Long ticketId, Date newLastMessageAt` | `int` | 🔴 单调更新：`UPDATE cs_ticket SET last_message_at=? WHERE id=? AND last_message_at < ?` |
| `findById` | `Long id` | `CsTicketDO` | 按 ID 查 [据推断] |
| `save` | `CsTicketDO ticket` | `void` | 新增 [据推断] |
| `update` | `CsTicketDO ticket` | `void` | 更新 [据推断] |
| `updateStatus` | `Long id, String status, String operatorId` | `void` | 更新状态（事务内）[据推断] |

#### CsUserStatusRongyunSyncRepositoryImpl [已确认（🆕）]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/persistence/repository/mysql/CsUserStatusRongyunSyncRepositoryImpl.java` |
| 职责 | 🆕 融云状态快照仓储实现；UPSERT 批量 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `upsert` | `CsUserStatusRongyunSyncDO sync` | `void` | 🔴 单条 UPSERT：`INSERT ... ON DUPLICATE KEY UPDATE`；ONLINE 时刷新 `last_rongcloud_online_at`，OFFLINE 保留 |
| `batchUpsert` | `List<CsUserStatusRongyunSyncDO> syncs` | `void` | 🔴 批量 UPSERT（`@Transactional` 链路使用）|

#### CopilotServerFacadeImpl [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/facade/CopilotServerFacadeImpl.java` |
| 职责 | Copilot Server 防腐层；封装服务模式同步调用；内置固定间隔重试 |

| 字段 | 类型 | 值 |
|------|------|---|
| `copilotServerClient` | `CopilotServerClient` | Feign Client |
| `MAX_RETRY` | `int` | `3` |
| `RETRY_INTERVAL_MS` | `long` | `3000ms` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `syncServiceMode` | `Long sessionId, String serviceMode` | `void` | 🔴 同步会话服务模式至 Copilot Server；**重试 3 次（间隔 3s），失败记录 warn 日志不上抛** |

#### CopilotMessageFacadeImpl [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/facade/CopilotMessageFacadeImpl.java` |
| 职责 | Copilot Message 防腐层；推送消息到 Copilot；**@Async + @Retryable** |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `pushMessage` 🆕 | `CopilotPushRequest request` | `void` | 🔴 推送消息到 Copilot；**@Async + @Retryable(3次/5s)**；失败异常向上传播 |

#### CopilotServerClient (Feign) [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/feign/CopilotServerClient.java` |
| 目标服务 | `copilot-server`（外部服务，契约以 copilot 团队为准）|

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `syncServiceMode` | `CopilotServiceModeRequest request` | `Map<String,Object>` | 🔴 `POST /api/copilot/v1/session/service-mode`，同步会话服务模式（HUMAN / AI）|

#### CsNotificationGatewayImpl + CsNotificationAsyncExecutor [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/facade/CsNotificationGatewayImpl.java` + `CsNotificationAsyncExecutor.java` |
| 职责 | 🔴 三端通知网关实现；以 `NotificationEventTypeEnum` 为策略表；@Async 双通道 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `notify` | `NotificationEvent event` | `void` | 根据 `NotificationEventTypeEnum` 路由到对应通道：APP 极光（经 life-notification-service 中台）/ 工作台融云系统消息 / copilot-server 服务模式同步 |

### 5.5 Interfaces 层 [据推断]

#### CsConversationServiceImpl

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-interfaces/src/main/java/com/casstime/cloud/life/cs/interfaces/restful/CsConversationServiceImpl.java` |
| 实现 SPI | `icec-cloud-life-cs-spi` 的 `CsConversationService` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `receiveAiMessages` 🆕 | `CopilotAiMessageReceiveRequest request` | `ApiResult<Void>` | 🔴 C25 用例入口；委托 `CsConversationAppService#receiveAiMessages`；**失败时返回 5xx 供 Copilot 重试** |
| （其他方法）| — | — | [据推断] |

#### CsUserServiceImpl

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-cs-interfaces/src/main/java/com/casstime/cloud/life/cs/interfaces/restful/CsUserServiceImpl.java` |
| 实现 SPI | `icec-cloud-life-cs-spi` 的 `CsUserService` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `upsertRongyunOnlineStatus`（单条/批量）🆕 | `UpsertRongyunOnlineStatusRequest(s) request(s)` | `ApiResult<Void>` | 融云状态同步接口（单条/批量）[据推断] |
| `getCsUserByUserId` | `String userId` | `ApiResult<CsUserDTO>` | 坐席查询 |
| `updateOnlineStatus` | `String bossUserId, String status` | `ApiResult<Void>` | 在线状态变更 |

### 5.6 xxl-job Handler 层 [据推断]

| Handler | 触发 | 业务 |
|---------|------|------|
| `TimeoutAgentJobHandler` | xxl-job cron | 🔴 接待中超时扫描（`CsTicketTimeoutAppService#scanInProgressTimeout`）|
| `TimeoutUserJobHandler` | xxl-job cron | 🔴 待接待超时扫描（`CsTicketTimeoutAppService#scanWaitingTimeout`）|
| `TimeoutWaitingJobHandler` | xxl-job cron | 🔴 待接待二次提醒（`CsTicketTimeoutAppService#scanWaitingReminder`）|

---

## 6. 工程特定约束 [已确认]

### 6.1 工程特定静态扫描（🔴 编码完成必跑）

```bash
# 1. 全限定名扫描（除 import 块外不应出现）
grep -rn "com\.casstime\.cloud\.life\.cs\.\(domain\|infrastructure\)\.\w\+\." \
  --include="*.java" icec-cloud-life-cs/{application,interfaces}/src/main/java/ \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空

# 2. application 层不应直接 import infrastructure.persistence
grep -rn "import com\.casstime\.cloud\.life\.cs\.infrastructure\.persistence" \
  --include="*.java" icec-cloud-life-cs-application/src/main/java/
# 期望输出为空

# 3. application 层不应直接 import 外部 SPI domain/infrastructure
grep -rn "import com\.casstime\.cloud\.\(life\|boss\)\.[a-z]\+\.\(domain\|infrastructure\)" \
  --include="*.java" \
  icec-cloud-life-cs-application/src/main/java/ \
  icec-cloud-life-cs-interfaces/src/main/java/
# 期望输出为空

# 4. SQL 关键字在 Service/AppService 层的扫描（应仅 Infrastructure 出现）
grep -rn "SELECT\|INSERT\|UPDATE\|DELETE" \
  --include="*AppService.java" --include="*Service.java" \
  icec-cloud-life-cs/*/src/main/java/ \
  | grep -v "import " | grep -v "//.*\(SELECT\|INSERT\|UPDATE\|DELETE\)"
# 期望仅 Infrastructure 出现

# 5. 🔴 STATE-MACHINE 禁止事项
# 状态机流转不能写在 AppService 里（必须经 CsTicketStateMachineDriver）
grep -rn "statemachine\.sendEvent\|StateMachine.*sendEvent" \
  --include="*AppService.java" icec-cloud-life-cs-application/src/main/java/
# 期望仅 CsTicketStateMachineDriver 内部出现

# 6. 🔴 @Scheduled 禁用（必须用 xxl-job）
grep -rn "@Scheduled" --include="*.java" icec-cloud-life-cs/
# 期望输出为空

# 7. 🔴 事务内禁止远程调用 / MQ
grep -rn -B 1 "@Transactional" icec-cloud-life-cs-application/src/main/java/ \
  | grep -E "Feign|@Async|@Retryable|Client\."
# 期望输出为空
```

### 6.2 🔴 客服域特有约束（🔴 编码必读）

| # | 约束 | 描述 |
|---|------|------|
| 1 | **状态机流转统一驱动** | 所有工单/会话状态流转必须经 `CsTicketStateMachineDriver`；**禁止 AppService 内直接 `stateMachine.sendEvent()`** |
| 2 | **超时字段语义** | `assigned_at` 用于待接待超时扫描（30min / 5min 提醒）；`claimed_at` 用于接待中超时扫描（15min 双侧静默）|
| 3 | **`claimed_at` 防御 null** | `findInProgressTimeoutTickets` 显式 `AND claimed_at IS NOT NULL`（2026-06-05 补丁）|
| 4 | **会话活跃状态收窄** | `CsConversationRepositoryImpl.ACTIVE_STATUSES = [IN_PROGRESS]`（不含 WAITING）|
| 5 | **结单 5 步必须全部执行** | 状态机 → 字段 → 释放负载 → 同步会话 → 通知；任何一步失败回滚 |
| 6 | **三端通知异步** | `CsNotificationGatewayImpl` 必须经 `CsNotificationAsyncExecutor` 双通道异步发送 |
| 7 | **融云 ONLINE 时间刷新条件** | `cs_user_status_rongyun_sync.last_rongcloud_online_at` 仅在状态 ONLINE 时刷新；OFFLINE 保留 |
| 8 | **C25 用例失败必须向上传播** | `receiveAiMessages` / `handleCopilotAiMessageReceive` 失败时异常向上传播供 Copilot 重试，**禁止吞异常** |
| 9 | **坐席负载乐观锁** | `assignment_version` 字段 + CAS 更新；禁止直接 `update` |

---

## 7. 上下游契约 [已确认/据推断]

### 7.1 对外暴露（SPI / Controller）

| 类型 | 接口名 | URL / 服务名 | 文档 |
|------|--------|------------|------|
| SPI | `CsUserService` | 服务名 `life-cs-service` | `icec-cloud-life-spi/icec-cloud-life-cs-spi/` |
| SPI | `CsTicketService` | 服务名 `life-cs-service` | 同上 |
| SPI | `CsConversationService` | 服务名 `life-cs-service` | 同上 |
| SPI | `CsAiSummaryService` | 服务名 `life-cs-service` | 同上 |
| Controller | `CsConversationServiceImpl` | `/life-cs/...` | interfaces 层 |
| Controller | `CsUserServiceImpl` | `/life-cs/...`（含融云状态同步端点）| interfaces 层 |

### 7.2 对内消费（Feign Client）

| SPI | 服务 | 方法 | 本工程 Feign Client |
|-----|------|------|------------------|
| `ImMessageService` | `life-im-service` | （多个）| `ImMessageFacadeImpl`（防腐层封装）|
| `ImSessionService` | `life-im-service` | （多个）| `ImSessionServiceFacade` |
| `NotificationService` | `life-notification-service` | 推送 | `NotificationServiceClient` |
| `TqOutboundCallService` | TQ 外呼平台 | 外呼 | `TqOutboundCallClient` |
| `CopilotServer`（外部）| `copilot-server` | `syncServiceMode` | `CopilotServerClient`（POST `/api/copilot/v1/session/service-mode`）|
| `CopilotService`（外部）| `copilot-server` | `fetchMessages` | `CopilotServerClient`（分页拉取 AI 消息）|

### 7.3 域间事件

| 事件 | 事件源 | 事件消费者 |
|------|--------|----------|
| `CsTicketCreatedEvent` | `CsTicketAppService#createAndRoute` | （待确认）|
| `CsTicketClaimedEvent` | `CsTicketAppService#claimTicket`（事务后）| IM 服务 / AI 上下文 |
| `CsTicketClosedEvent` | `CsTicketCloseOrchestrator` step 5 | 三端通知 / Copilot 同步 |
| `CsTicketReopenedEvent` | `CsTicketAppService#reopenTicket` | IM 服务 |
| `CsConversationCreatedEvent` | （IM 服务反向）| `CsConversationAppService` |

---

## 8. 数据库表清单 [已确认]

| 表名 | 关键字段 | 业务含义 |
|------|---------|---------|
| `cs_user` | id / boss_user_id / display_name / profile_img_url / online_status / current_load / max_concurrency / assignment_version / is_enabled | 坐席基础信息 + 负载控制 |
| `cs_ticket` 🔴 | id / conversation_id / session_id / requester_principal_type / requester_principal_id / cs_user_id / claimed_cs_user_id / status / title / description / extra_attribute / closed_reason / close_note / claimed_at / **assigned_at 🆕** / finished_at / closed_by / last_message_at / start_message_id / end_message_id | 工单全生命周期 |
| `cs_conversation` | id / session_id / requester_principal_type / requester_principal_id / cs_user_id / status / started_at / ended_at / start_message_id / end_message_id | 会话信息 |
| `cs_ai_summary` | id / ticket_id / core_issue / extra_attributes | AI 工单摘要 |
| `voice_call_record` | id / conversation_id / ticket_id / session_id / biz_type / customer_id / caller_principal_type / caller_principal_id / boss_user_id / tq_uin / virtual_number / call_status / started_at / answered_at / ended_at / duration_seconds / recording_url / vendor / vendor_call_id / is_synced | 虚拟外呼通话记录 |
| `cs_user_status_rongyun_sync` 🆕 | id / cs_user_id（唯一约束）/ cs_user_online_status / online_device_count / last_rongcloud_online_at / last_seen_at / platforms | 融云在线状态同步快照 |

> **🔴 STORY-020 重构变更**：
> - `cs_ticket.last_sender_type` 字段已移除
> - 原复合索引 `idx_last_message_at(status, last_sender_type, last_message_at)` 已删除
> - `cs_ticket.assigned_at` 字段新增（派单时间）
> - `cs_ticket.claimed_at` IS NOT NULL 约束（防御异常工单）

---

## 9. 本工程缺口与待补充

| # | 缺口 | 优先级 | 状态 |
|---|------|-------|------|
| 1 | `CsTicketConverter` 完整方法列表 | 🟡 P2 | 待补 |
| 2 | `CsTicketAppService` 完整方法列表（仅推断 claimTicket / closeTicket / reopenTicket / getUserTickets / getTicketMessageRange）| 🟠 P1 | 待补 |
| 3 | `CsTicketDomainService` 完整方法列表 | 🟡 P2 | 待补 |
| 4 | `CsRoutingDomainService` 路由算法（基于负载 / 在线状态 / 历史分配）| 🟠 P1 | 待补 |
| 5 | `CsTicketErrorCode` 完整错误码（16000-16999 段位已确认，具体码值待探查）| 🟠 P1 | 待补 |
| 6 | 状态机流转事件定义（CsTicketStateEvent / CsConversationStateEvent）| 🟡 P2 | 待补 |
| 7 | xxl-job 三类 Handler 的 cron 配置（每分钟？每 5 分钟？）| 🟡 P2 | 待补 |
| 8 | ES 索引（CsTicketSearchIndexFacade 详情）| 🟡 P2 | 待补 |
| 9 | CsAgentNameCache Guava LoadingCache 配置（size / expire）| 🟢 P3 | 待补 |
| 10 | 域事件消费者清单（每个事件的实际消费者）| 🟡 P2 | 待补 |

---

## §A 关键词反向索引 [已确认/据推断]

| 关键词 | 出现位置 |
|--------|---------|
| `CsConversationAppService` | §4 / §5.2 |
| `CsUserAppService` | §4 / §5.2 |
| `CsTicketAppService` | §4 / §5.2 |
| `CsTicketTimeoutAppService` | §5.2（STORY-020 核心）|
| `CsTicketCloseOrchestrator` | §5.2（5步结单）|
| `CsTicketStateMachineDriver` | §4 / §5.2 / §6.2 |
| `CsMessageHandler` | §4 / §5.2（C25 用例）|
| `CsTicketStatusEnum` | §4 / §5.3（WAITING/IN_PROGRESS/RESOLVED/UNRESOLVED/TIMEOUT_CLOSED）|
| `CsUserStatusRongyunSyncDO` | §4 / §5.3 |
| `CsUserStatusRongyunSyncRepository` | §4 / §5.4 |
| `assigned_at` | §4 / §5.3 / §5.4 / §8（STORY-020 新增字段）|
| `claimed_at` | §5.3 / §5.4 / §6.2 |
| `findWaitingTimeoutTickets` | §5.4 |
| `findWaitingReminderTickets` | §5.4 |
| `findInProgressTimeoutTickets` | §5.4（2026-06-05 补丁）|
| `syncLastMessageAtFromIm` | §5.4 |
| `last_sender_type` | §6.2 / §8（已移除）|
| `AssignmentVersion` / `assignment_version` | §4 / §5.3 / §6.2（乐观锁）|
| `notificationGateway` / `CsNotificationGatewayImpl` | §4 / §5.4（三端通知）|
| `CopilotServerFacadeImpl` | §4 / §5.4（重试 3次/3s）|
| `CopilotMessageFacadeImpl` | §4 / §5.4（@Async + @Retryable 3次/5s）|
| `receiveAiMessages` / `handleCopilotAiMessageReceive` | §5.2 / §5.4（C25 用例）|
| `CopilotServerClient.syncServiceMode` | §5.4（POST `/api/copilot/v1/session/service-mode`）|
| `CsAgentNameCache` | §4 / §5.2（Guava LoadingCache）|
| `CsDistributedLock` | §4 / §5.2（Redisson ticket/user 维度）|
| `CsAgentLoadDomainService` | §4 / §5.3（BR-02）|
| `spring.redis.password` | §1.1（明文 🟠 安全提示）|
| `xxl-job.executor.appname` = `life-cs-executor` | §1.1 |
| `panda.config.product` = `b2c` | §1.1 |

---

## §B 本工程更新日志（合并到主体 update-log）

> 详见主体 `icec-cloud-life.update-log.md`；本节列本工程特有的变更摘要。

| 日期 | 变更摘要 |
|------|---------|
| 2026-06-26 | 🆕 按 schema v3 §15 首次生成工程级子文件；来源：同事 life-cs.md（110KB / 1510 行）+ ae-sdd life.assets.md §2 |
| 2026-06-26 | 🔴 安全提示 S-101 标记：bootstrap.yml `spring.redis.password` 明文（高危，待修）|
| 2026-06-05 | 🔴 `findInProgressTimeoutTickets` 增加 `AND claimed_at IS NOT NULL` 防御 |