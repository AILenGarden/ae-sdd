---
name: icec-cloud-life-im-project-assets
description: icec-cloud-life-im 工程级项目资产 — 按 schema v3（2026-06-26）工程级粒度拆分 SOP 生成。即时通讯域 Service：会话/消息/融云/身份/CS 子域，集成 RongCloud 第三方 IM 云，含 1 个 16 方法 AppService + 1 个 1 方法 AppService + 融云防腐层（apdapter.rongcloud）+ CS 防腐层。本文与 life-cs 形成"客服 vs IM"两大 Service 域对比；STORY-020 重构（latest 字段改由 ImMessageDomainService.fillLatest 内存推导）。
parent: icec-cloud-life.assets.md
探查时间: 2026-06-26
探查来源: 同事 D:\Item\life\document\life-team-project-docs\knowledge\project\icec-cloud-life-im.md (92KB / 1127 行) + ae-sdd life.assets.md §4 DDD 落点
---

# icec-cloud-life-im 工程级项目资产

> **本文是 `icec-cloud-life.assets.md` 的工程级子文件**（schema §15），仅含本工程细节。跨工程信息见主体。
>
> **范本定位**：本文是 schema §15 工程级拆分的**第 4 份实例**。重点展示：
> 1. **第三方 IM 云集成** — RongCloud（融云）SDK 防腐层（apdapter.rongcloud）
> 2. **多业务子域** — message / session / identity / cs 4 子域共存于一个工程
> 3. **多态消息体** — TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE 6 种类型
> 4. **STORY-020 重构** — latest 字段从 SQL 窗口函数改为 Domain Service 内存推导
> 5. **签名校验** — 融云回调 4 元签名三元组（appKey/nonce/timestamp/signature）

---

## 0. 摘要与使用场景 [已确认]

| 维度 | 内容 |
|------|------|
| 工程名 | `icec-cloud-life-im` |
| 父工程 | `icec-cloud-life` |
| 探查时间 | 2026-06-26 |
| 工程定位 | 🆕 即时通讯域 Service；集成 RongCloud（融云）作为底层 IM 厂商（落库 `vendor=rongcloud`）；提供会话/消息/身份 Token/在线状态回调能力 |
| 关键不变量 | 本工程不重复定义 rules；只把 rules 映射到本工程代码 |
| 最后审计 | 2026-06-26 |
| 关键架构特性 | 🆕 RongCloud 防腐层（apdapter.rongcloud）+ 🆕 4 业务子域（message/session/identity/cs）+ 🆕 多态消息体（6 类型）+ 🆕 签名校验（融云回调 4 元）+ 🆕 STORY-020 重构（latest 内存推导） |

---

## 1. 模块元信息 [已确认]

| 字段 | 值 |
|------|---|
| moduleName | `icec-cloud-life-im` |
| groupId | `com.casstime.cloud` |
| artifactId | `icec-cloud-life-im` |
| version | `1.0`（**注意：非 1.0-SNAPSHOT，是带版本号的发布版**）|
| packaging | `pom（5 模块聚合父工程）` |
| 子模块数 | 5 个（domain / application / interfaces / infrastructure / service）|
| 主启动类 | `icec-cloud-life-im-service/src/main/java/com/casstime/cloud/life/im/` `Bootstrap.java`（[据推断]）|
| profile | `dev, test, prod, beta-kunlun` |
| port | `20093` |
| contextPath | — |
| dependsOnSpi | [`icec-cloud-life-im-spi 1.1`, `icec-cloud-life-cs-spi 1.1`] |
| lastAuditedAt | `2026-06-26` |
| owner | 架构组 + life-im 域负责人 |

### 1.1 部署信息 [已确认/据推断]

> 🆕 2026-06-26 schema §1.2 新增节。本节从 `bootstrap.yml` + pom 抽取（含融云配置）。

| 字段 | 值 | 抽取来源 |
|------|---|---------|
| `application.name` | `life-im-service` | bootstrap.yml（推断）|
| `profile.active` | `beta-kunlun` | bootstrap.yml（推断）|
| `server.port` | `20093` | bootstrap.yml（推断）|
| `db.driverClassName` | `com.mysql.cj.jdbc.Driver` | pom 显式声明 |
| `db.type` | `HikariDataSource` | pom 显式声明 |
| `db.urlTemplate` | `jdbc:mysql://${icec.database.servers}/${icec.database.dbname}?useUnicode=true&characterEncoding=utf-8&allowMultiQueries=true&autoReconnect=true` | [据推断，同事其他工程一致] |
| `db.pool` | HikariCP 2.7.9 | pom 显式声明 |
| `redis.address` | `redis-...dcs.huaweicloud.com:6379`（华为云 DCS）| pom 显式声明 |
| `redis.password.inConfig` | 待复验 | bootstrap.yml（推断）|
| `gateway` | `http://life-hwbeta-api-penglai.intra.casstime.com`（icec.api.agent）| [据推断] |
| `imageRepo` | `registry.cn-shenzhen.aliyuncs.com/cassmall/${project.name}` | pom `dockerfile-maven-plugin` |
| `nexusRepo` | `http://dev.casstime.com/nexus/content/groups/public/` | pom repositories |
| `panda.config.server-addr` | `http://panda.casstime.com` | pom 显式声明 |
| `panda.config.product` | `b2c` | pom 显式声明 |
| `panda.config.instance` | `life-im-service` | pom 显式声明 |
| 🆕 `rongcloud.app-key` | 🔴 **@Value("${rongcloud.app-key}") 注入** | bootstrap.yml `rongcloud.app-key`（[据推断]）|
| 🆕 `rongcloud.app-secret` | 🔴 **@Value("${rongcloud.app-secret}") 注入** | bootstrap.yml `rongcloud.app-secret`（[据推断]）|
| 🆕 `rongcloud.api-url` | 融云 API 地址 | bootstrap.yml `rongcloud.api-url`（[据推断]）|
| 🆕 `rongcloud.server-name` | 系统发送方融云账号名（@Value 注入）| bootstrap.yml `rongcloud.server-name`（[据推断]）|
| `management.port` | 未显式配置（默认 30000）| ⚠️ [据推断] |
| `coverageTool` | 未配置 JaCoCo | ⚠️ [据推断] |

### 1.2 安全提示 [已确认/待确认]

> 🆕 2026-06-26 schema §14 新增节。

| ID | 类型 | 位置 | 风险等级 | 描述 | 建议修复 | 状态 |
|----|------|------|---------|------|---------|------|
| S-301 | 🔴 **融云 app-secret 明文风险** | `bootstrap.yml` `rongcloud.app-secret` | 🟠 高 | 融云 app-secret 由 `@Value` 注入；如直接在配置文件中明文，会被 Git 仓库泄露 → 第三方推送权限被劫持 | 配置中心加密存储；或迁移至密钥管理服务（如 AWS Secrets Manager）| **待复验** |
| S-302 | 🔴 **融云回调签名校验** | `ImMessageAppService#handleCallback` 入口 | 🟢 低 | 必须始终验签（即使 channelType 不支持也先验签）；绕过验签会导致任意伪造消息入库 | 自动化测试覆盖：故意构造错误签名应抛 20011 | 已规则化 |
| S-303 | spring-boot-maven-plugin 版本不一致 | service pom | 🟡 中 | service 模块使用 spring-boot-maven-plugin 2.7.5，与 Spring Boot 1.5.7.RELEASE 不一致 | 升级 spring-boot-maven-plugin 至与框架版本匹配 | 待确认 |
| S-304 | 父 POM description 为占位符 | 父 pom.xml `description` 字段 | 🟢 低 | 同事 life-im.md §1.3 注 2 标注"父 POM description 字段为 'xxx????' 占位符" | 填写正确描述（如 "IM 即时通讯服务"）| 待修 |
| S-305 | hooks/commit-msg 自动复制 | pom `maven-resources-plugin 3.0.1` | 🟢 低 | validate 阶段将 hooks/commit-msg 复制到 `.git/hooks`，强制 Git 提交规范 | 已是合规配置 | OK |
| S-306 | @Scheduled 禁用检查 | 工程内 Java 代码 | 🟢 低 | 必须用 xxl-job / job-spring-boot-starter | 跑 `grep -rn "@Scheduled" --include="*.java"` | 已规则化 |
| S-307 | 事务内禁止远程调用 / MQ | application/appservice | 🟢 低 | 🔴 BFF/Service 通用红线 | 跑 `grep -B 1 "@Transactional" application/appservice/*.java` | 已规则化 |
| S-308 | API 文档 Swagger 必加 | 接口层 | 🟢 低 | Swagger 2.8.0 必加 | 跑 `grep "@ApiOperation" --include="*.java"` | 已规则化 |

---

## 2. 子模块结构 [已确认]

> 🆕 **IM 域特色**：5 模块 DDD，但 domain 层按**业务子域**（message/session/identity/cs）切分子包 — 不是按 DDD 分层。

| 模块 | ArtifactId | 打包 | 职责 | 主要依赖 |
|------|-----------|------|------|---------|
| 领域层（🆕 4 子域）| `icec-cloud-life-im-domain` | jar | 🆕 **4 子域**：message（消息体多态模型 / ImMessageDomainService / ImMessageRepository 端口）/ session（会话模型 / 仓储端口）/ identity（RongCloudFacade 防腐端口 / Token 相关）/ cs（CsUserServiceFacade 防腐端口 / UpsertRongyunOnlineStatusDO）；领域层**不感知厂商细节** | `icec-cloud-commons:b2c.1.0-SNAPSHOT`、`commons-lang3:3.9`、`lombok` |
| 应用层 | `icec-cloud-life-im-application` | jar | 编排：ImMessageAppService（🆕 16 方法 / Bean 名 `imMessageCallbackAppService`）+ ImUserOnlineStatusAppService（1 方法 / Bean 名 `imUserOnlineStatusAppService`）+ ImSessionAppService + ImTokenAppService；Converter：ImMessageConverter / ImSessionConverter / ImTokenConverter | `icec-cloud-life-im-domain`、`icec-cloud-life-im-spi 1.1`、`icec-cloud-commons`、`lombok` |
| 接口层 | `icec-cloud-life-im-interfaces` | jar | RESTful 接口：ImSessionServiceImpl / ImMessageServiceImpl / ImTokenServiceImpl / **ImUserOnlineStatusServiceImpl**（融云在线状态回调 Controller）/ ImExceptionHandler（统一异常处理）；Converter：ImUserOnlineStatusCallbackConverter（融云 query/body → SPI Request） | `icec-cloud-life-im-application`、`icec-cloud-commons`、`spring-web`、`lombok` |
| 基础设施层 | `icec-cloud-life-im-infrastructure` | jar | 🆕 **融云防腐层** `apdapter.rongcloud`（RongCloudConfig 装配 SDK / RongCloudClient 封装注册/Token/发消息 / RongCloudFacadeImpl / ImMessageBodyConverterImpl 经 RongBody 做 6 类型双向转换）；**CS 防腐层**（CsUserServiceFacadeImpl + CsUserServiceClient Feign）；持久化（MyBatis-Plus + MySQL + HikariCP + Redis）；消息推送 Hook（CsMessageNotifyHook） | `icec-cloud-life-im-domain`、`icec-cloud-life-im-application`、`icec-cloud-life-cs-spi 1.1`、`icec-cloud-spi-common`、`spring-cloud-starter-feign`、`spring-cloud-netflix-core`、`spring-boot-starter-data-redis`、`HikariCP 2.7.9`、`mysql-connector-java 8.0.17`、`mybatis-plus-boot-starter 3.3.2` |
| 启动/装配层 | `icec-cloud-life-im-service` | jar | Spring Boot 应用入口；整合各层；装配 MyBatis-Plus mapper（`mybatis-plus.mapper-locations`）；Docker 镜像打包 | `icec-cloud-life-im-interfaces`、`icec-cloud-life-im-infrastructure`、`panda 1.0.9 / casslog / cass-config 1.1.2 / cassmetrics 1.0.4 starter`、`spring-boot-starter-web/aop/cache/test`、`spring-cloud-starter-config/feign`、`mysql-connector-java 8.0.17`、`spring-boot-maven-plugin 2.7.5`、`maven-compiler-plugin 3.8.1` |

> **🔴 关键发现**：
> - 🆕 icec-cloud-life-im-spi 升自 1.0.2 → **1.1**（承接用户在线状态回调新 DTO）
> - 🆕 icec-cloud-life-cs-spi 1.1 也被 infrastructure 引入（用于 CsUserServiceClient Feign）
> - 父 POM description 字段是 "xxx????" 占位符（同事标注）

### 2.1 依赖层次关系 [已确认]

```
life-im-service                       ← 启动装配（聚合 interfaces + infrastructure）
        ↓
        ├──→ icec-cloud-life-im-interfaces
        │             ↓
        │             └──→ icec-cloud-life-im-application
        │                          ↓
        │                          ├──→ icec-cloud-life-im-domain (4 子域)
        │                          └──→ (external) icec-cloud-life-im-spi 1.1
        │                                            ↑
        └──→ icec-cloud-life-im-infrastructure ───┘
                          ↓
                          ├──→ icec-cloud-life-im-domain (Repository/Facade 接口实现)
                          ├──→ icec-cloud-life-im-application (Facade 接口实现)
                          └──→ (external) icec-cloud-life-life-cs-spi 1.1 (CsUserServiceClient Feign)
```

### 2.2 🆕 4 业务子域包结构（domain 层）

```
icec-cloud-life-im-domain/
├── message/                         ← message 子域
│   ├── model/
│   │   ├── entity/MessageDO.java
│   │   ├── enums/MessageTypeEnum.java (TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE)
│   │   ├── value/{MarkdownMessageBodyDO, LatestSideMessageDO}.java
│   │   └── repository/ImMessageRepository.java
│   ├── service/ImMessageDomainService.java        ← fillLatest 三侧锚点推导
│   └── facade/{ImMessageBodyConverter, RongCloudFacade}.java  ← 防腐端口
├── session/                         ← session 子域
│   └── repository/{ImSessionRepository, ImSessionParticipantRepository}.java
├── identity/                        ← identity 子域（Token）
│   ├── model/IdentityRegistry.java
│   └── repository/ImIdentityRegistryRepository.java
└── cs/                              ← cs 子域（融云在线状态）
    ├── model/entity/UpsertRongyunOnlineStatusDO.java
    └── facade/CsUserServiceFacade.java             ← CS 服务防腐端口
```

---

## 3. 完整技术栈版本号 [已确认]

> 🆕 2026-06-26 schema §6.8 新增节。**完整 7 张表见主体 §6.8**，本节只列本工程特有的依赖。

| 依赖 | 版本 | 用途 |
|------|------|------|
| 🆕 `rongcloud server-sdk-java` | （版本待确认 — 各子模块 pom.xml 未显式声明，是否通过传递依赖引入待查）| 融云即时通讯 SDK |
| `spring-boot` | `1.5.7.RELEASE` | 应用框架 |
| `spring-cloud` | `Dalston.SR4` | 微服务框架 |
| `spring-cloud-starter-feign` / `spring-cloud-netflix-core` | 跟随 | Feign 远程调用（CsUserServiceClient / ImMessageEventNotifyClient）|
| `mybatis-plus` | `3.3.2` | ORM；`mybatis-plus.mapper-locations` 加载 mapper XML |
| `mysql-connector-java` | `8.0.17` | MySQL 8 |
| `HikariCP` | `2.7.9` | 连接池 |
| `spring-boot-starter-data-redis` | 跟随 | Redis 缓存（华为云 DCS）|
| `MapStruct` | `1.5.3.Final`（据 properties 推断）| 对象映射 |
| `lombok` | `1.18.16` | @Data / @RequiredArgsConstructor / @Slf4j |
| `commons-lang3` | `3.9` | 字符串/通用工具 |
| `icec-cloud-commons` | `b2c.1.0-SNAPSHOT` | 公共组件 |
| `icec-cloud-spi-common` | `b2c.1.0-SNAPSHOT` | SPI 公共契约（仅 infrastructure 显式声明）|
| 🆕 `icec-cloud-life-im-spi` | `1.1`（升自 1.0.2）| IM SPI 契约（含用户在线状态回调新 DTO）|
| 🆕 `icec-cloud-life-cs-spi` | `1.1` | CS SPI 契约（仅 infrastructure 显式声明）|
| `panda-spring-boot-starter` | `1.0.9` | 配置中心 |
| `casslog-spring-boot-starter` | `1.5.0` | 日志组件 |
| `cass-config-spring-boot-starter` | `1.1.2` | 配置组件 |
| `cassmetrics-spring-boot-v1-starter` | `1.0.4` | 监控指标 |
| `log4j-bom` | `2.17.2` | Log4Shell 修复 |
| `Swagger2` | `2.8.0` | API 文档 |
| `spring-boot-maven-plugin` | `2.7.5`（**与 Spring Boot 1.5.7 不一致，待确认**）| Fat JAR 打包 |
| `maven-compiler-plugin` | `3.8.1`（service 模块显式配置 source/target 1.8）| Java 编译 |
| `dockerfile-maven-plugin` | `1.4.0`（spotify）| Docker 镜像构建 |
| `maven-resources-plugin` | `3.0.1` | 复制 hooks/commit-msg 到 .git/hooks |

---

## 4. DDD 内部分层落点 [已确认/据推断]

> **详细类角色映射见主体 §4**；本节只列本工程内实际类。

| 类角色 | 精确包路径 | 典型类名（已确认）|
|--------|-----------|------------------|
| **🆕 4 子域（domain 层）** | | |
| Message 子域 — Domain Object | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/message/model/entity/` | `MessageDO` / `ImMessageDO`（推断）|
| Message 子域 — Enum | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/message/model/enums/` | `MessageTypeEnum`（🆕 **TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE 6 类型**）|
| Message 子域 — Value Object | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/message/model/value/` | `MarkdownMessageBodyDO` / `LatestSideMessageDO` |
| Message 子域 — Repository | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/message/repository/` | `ImMessageRepository` |
| Message 子域 — Domain Service | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/message/service/` | 🆕 `ImMessageDomainService`（**fillLatest 三侧锚点推导**）|
| Message 子域 — Facade | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/message/facade/` | `ImMessageBodyConverter`（领域消息体 ↔ 融云体端口）/ `RongCloudFacade`（融云防腐端口）|
| Session 子域 — Repository | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/session/repository/` | `ImSessionRepository` / `ImSessionParticipantRepository` |
| Session 子域 — Domain Service | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/session/service/` | `ImSessionDomainService`（私聊双边唯一活跃会话定位）|
| Identity 子域 — Repository | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/identity/repository/` | `ImIdentityRegistryRepository`（按 vendorUserId 批量反查主体身份）|
| CS 子域 — Domain Object | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/cs/model/entity/` | 🆕 `UpsertRongyunOnlineStatusDO`（融云在线状态回调领域实体）|
| CS 子域 — Facade | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/cs/facade/` | 🆕 `CsUserServiceFacade`（CS 服务防腐端口）|
| **Application** | | |
| AppService — message | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/appservice/` | 🆕 `ImMessageAppService`（`@Service("imMessageCallbackAppService")` + 16 方法）|
| AppService — 融云在线状态 | 同上 | 🆕 `ImUserOnlineStatusAppService`（`@Service("imUserOnlineStatusAppService")` + 1 方法）|
| AppService — session | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/session/appservice/` | `ImSessionAppService`（会话创建/批量查询/参与者管理/未读数编排）|
| AppService — identity | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/identity/appservice/` | `ImTokenAppService`（融云 Token 获取与账号注册编排）|
| Converter | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/{message,session,identity}/converter/` | `ImMessageConverter`（DO ↔ DTO / 6 类型消息体）/ `ImSessionConverter` / `ImTokenConverter` |
| Publisher | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/publisher/` | `ApplicationEventPublisher`（空接口占位）|
| **Interfaces** | | |
| Rest Controller | `icec-cloud-life-im-interfaces/src/main/java/com/casstime/cloud/life/im/interfaces/restful/` | `ImSessionServiceImpl` / `ImMessageServiceImpl` / `ImTokenServiceImpl` / 🆕 `ImUserOnlineStatusServiceImpl`（融云在线状态回调 Controller）|
| Converter | `icec-cloud-life-im-interfaces/src/main/java/com/casstime/cloud/life/im/interfaces/converter/` | 🆕 `ImUserOnlineStatusCallbackConverter`（融云 query/body → SPI Request）|
| Exception Handler | `icec-cloud-life-im-interfaces/src/main/java/com/casstime/cloud/life/im/interfaces/config/` | `ImExceptionHandler`（统一异常处理）|
| **🆕 Infrastructure — 融云防腐层（apdapter.rongcloud）** | | |
| 融云 Config | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/apdapter/rongcloud/config/` | `RongCloudConfig`（装配 SDK）|
| 融云 Client | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/apdapter/rongcloud/client/` | `RongCloudClient`（封装注册 / Token / 发消息）|
| 融云 Facade Impl | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/apdapter/rongcloud/facade/` | `RongCloudFacadeImpl`（实现领域防腐端口）|
| 融云 Converter Impl | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/apdapter/rongcloud/convert/` | 🆕 `ImMessageBodyConverterImpl`（经 RongBody 做 6 类型双向转换：TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE）|
| 融云 Model | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/apdapter/rongcloud/model/` | `RongBody`（`@JsonInclude(NON_NULL)`）|
| **Infrastructure — CS 防腐层** | | |
| CS Facade Impl | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/cs/facade/` | 🆕 `CsUserServiceFacadeImpl`（通过 CsUserServiceClient Feign 调用 CS SPI 更新融云在线状态）|
| CS Feign Client | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/feign/` | 🆕 `CsUserServiceClient`（extends `icec-cloud-life-cs-spi` 的 `CsUserService`）|
| **Infrastructure — 持久化（message）** | | |
| Repository Impl | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/repository/mysql/` | 🆕 `ImMessageRepositoryImpl`（**STORY-020 重构：findLatestSideMessagesAfterStartAt + batchUpsert + findConversationMessagesByCursor + 13 个核心方法**）|
| Mapper | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/dao/mapper/` | `ImMessageMapper`（extends MyBatis-Plus `BaseMapper<ImMessagePO>` + 11 个自定义方法）|
| Mapper XML | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/dao/xml/` | `ImMessageMapper.xml`（含 11 个 SQL 定义，含 STORY-020 重构的标量子查询）|
| PO | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/entity/` | `ImMessagePO` / `LatestSideMessagePO`（STORY-020 移除 latest 列）|
| Persistence Converter | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/converter/` | `ImMessagePOConverter` / `LatestSideMessagePOConverter` |
| **Infrastructure — Hook** | | |
| Hook | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/cs/hook/` | 🆕 `CsMessageNotifyHook`（向 CS 推送消息事件，按 sessionBizType 过滤）|
| **SPI（被消费方）** | | |
| SPI Service 接口 | `icec-cloud-life-spi/icec-cloud-life-im-spi/src/main/java/com/casstime/cloud/life/spi/im/service/` | `ImSessionService` / `ImMessageService` / `ImTokenService` / 🆕 `ImUserOnlineStatusService`（融云在线状态回调）|
| SPI DTO | `icec-cloud-life-spi/icec-cloud-life-im-spi/src/main/java/com/casstime/cloud/life/spi/im/dto/` | `ImSessionDTO` / `ImMessageDTO` / `ImMessageBodyDTO` / `ImUserOnlineStatusCallbackRequest` / `ImUserOnlineStatusCallbackItemDTO` |
| SPI Constants | `icec-cloud-life-spi/icec-cloud-life-im-spi/src/main/java/com/casstime/cloud/life/spi/im/` | `ServiceProviderConstants`（`LIFE_IM_SERVICE = "life-im-service"`）|

---

## 5. 核心类方法级实现（🆕 2026-06-26）

> **🆕 范本节**：IM 域作为多子域 + 第三方集成 + 状态机 + 推送的代表域，重点展示 AppService 编排器 + 多态消息体转换 + 防腐层 + 仓储扩展的方法级实现（来源：同事 life-im.md §1.4 + §5 + §4）。

### 5.1 Converter 层 [已确认]

#### 🆕 ImMessageConverter [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/message/converter/ImMessageConverter.java` |
| 职责 | 🆕 消息上下文转换器；`ImMessageDO` ↔ `ImMessageDTO` / `ImMessagePageItemDTO`；`LatestSideMessageDO` → `LatestSideMessageDTO`；领域消息体 `ImMessageBodyDO` ↔ SPI `ImMessageBodyDTO`（**支持 6 类型多态**）|

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `toDTO` | `ImMessageDO` | `ImMessageDTO` | DO 转 DTO [据推断] |
| `toDTOList` | `List<ImMessageDO>` | `List<ImMessageDTO>` | 批量 DTO [据推断] |
| `toPageItemDTOList` | `List<ImMessageDO>` | `List<ImMessagePageItemDTO>` | 转分页项 DTO [据推断] |
| `toLatestSideDTOList` | `List<LatestSideMessageDO>` | `List<LatestSideMessageDTO>` | 转最新侧边消息 DTO（含 userMessageId/userSentAt/csMessageId/csSentAt/latestMessageId/latestSentAt）[据推断] |
| 🆕 `toBodyDO` | `ImMessageBodyDTO` | `ImMessageBodyDO` | SPI 消息体转领域消息体（**支持 6 类型多态：TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE**）|
| 🆕 `toBodyDTO` | `ImMessageBodyDO` | `ImMessageBodyDTO` | 领域消息体转 SPI 消息体 |

#### 🆕 ImUserOnlineStatusCallbackConverter [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-interfaces/src/main/java/com/casstime/cloud/life/im/interfaces/converter/ImUserOnlineStatusCallbackConverter.java` |
| 职责 | 🆕 融云用户在线状态回调字段转换器（私有构造 + 纯静态）；融云原生 query 参数（appKey/nonce/timestamp/signature）+ body 参数（items）→ SPI 请求对象 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| 🆕 `toRequest`（static）| `String appKey, String nonce, String timestamp, String signature, List<ImUserOnlineStatusCallbackItemDTO> items` | `ImUserOnlineStatusCallbackRequest` | 组装 SPI 用户在线状态回调请求对象 |

### 5.2 AppService 层 [已确认]

#### 🆕 ImMessageAppService（IM 消息应用服务 — 16 方法）[已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/appservice/ImMessageAppService.java` |
| 类注解 | `@Service("imMessageCallbackAppService")` + `@Slf4j` + `@RequiredArgsConstructor`（**采用"直接实现"方式，不经接口抽象**）|
| 职责 | 🆕 编排融云消息回调用例（**签名校验 → channelType 守卫 → objectName 映射 → 幂等落库 → 收发双方身份解析 → 会话状态/未读数维护 → Hook 触发**）+ 消息列表查询 / 游标分页 / 完整对话 / 发送 / 批量同步 / 工单范围 / 按 sessionId 批量取最新 / 按 IDs / 按 sessionId 取最老 / STORY-020 接待中超时分侧 |

**核心字段（19 个）：**

| 字段名 | 类型 | 值/说明 |
|--------|------|------|
| 🆕 `DEFAULT_DIRECTION` | `String`（常量）| `"ASC"`，`resolveDirection` 对 null/非法值回填 |
| 🆕 `DIRECTION_DESC` | `String`（常量）| `"DESC"`，合法方向 |
| 🆕 `DEFAULT_LIMIT` | `int`（常量）| `20`，`resolveListLimit` 缺省 |
| 🆕 `MAX_LIST_LIMIT` | `int`（常量）| `200`，`listMessages` 上限截断 |
| 🆕 `DEFAULT_SENDER_ID` | `String`（常量）| `"copilot"`，批量同步默认发送者 |
| 🆕 `DEFAULT_PAGE_SIZE` | `int`（常量）| `20` |
| 🆕 `MAX_PAGE_SIZE` | `int`（常量）| `50` |
| 🆕 `MAX_DAYS` | `int`（常量）| `7`，历史消息回溯天数 |
| 🆕 `VENDOR_RONGCLOUD` | `String`（常量）| `"rongcloud"`，实时回调落库 `im_message.vendor` |
| 🆕 `DEFAULT_VENDOR` | `String`（常量）| `"copilot"`，批量同步 vendor 兜底 |
| 🆕 `AI_VENDOR_USER_ID_PLACEHOLDER` | `String`（常量）| `"copilot"`，AI 阶段 sync senderVendorUserId 占位 |
| `messageRepository` | `ImMessageRepository` | 消息仓储 |
| `sessionRepository` | `ImSessionRepository` | 会话仓储 |
| `participantRepository` | `ImSessionParticipantRepository` | 参与者仓储 |
| `identityRegistryRepository` | `ImIdentityRegistryRepository` | 身份注册仓储 |
| `hookRegistry` | `MessageHookRegistry` | 消息 Hook 注册中心（事务外触发）|
| `rongCloudFacade` | `RongCloudFacade` | 融云防腐层门面 |
| `transactionTemplate` | `TransactionTemplate` | 编程式事务模板 |
| `signatureValidator` | `RongCloudSignatureDomainService` | 融云回调签名校验领域服务 |
| `imSessionDomainService` | `ImSessionDomainService` | 会话领域服务 |
| `messageBodyConverter` | `ImMessageBodyConverter` | 领域消息体 ↔ 融云体端口 |
| `imMessageDomainService` | `ImMessageDomainService` | 🆕 消息领域服务（**fillLatest** 三侧锚点推导）|
| `rongCloudAppSecret` | `String` | `@Value("${rongcloud.app-secret}")` |
| `rongCloudServerName` | `String` | `@Value("${rongcloud.server-name}")` |
| 🆕 `systemAccountRegistered` | `volatile boolean` | 系统账号融云注册结果缓存（进程内只注册一次）|

**核心方法（16 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `listMessages` | `ListMessagesRequest request` | `List<ImMessageDTO>` | 按 sessionId + afterMessageId/sinceTime + 方向 + senderType + 条数查询；方向归一化（null/非法→ASC）；条数归一化（null/<1→20，上限 200 截断）|
| 🆕 `resolveDirection`（私有）| `String direction` | `String` | 归一化排序方向：null→ASC；忽略大小写，仅 ASC/DESC 合法；非法值归一化为 ASC 并记 warn |
| 🆕 `resolveListLimit`（私有）| `Integer limit` | `int` | 归一化返回条数：null/<1→20；超过 200 截断 |
| 🔴 `handleCallback` | `ImMessageCallbackRequest request` | `void` | 🆕 **融云消息回调核心路径**：①始终验签；②channelType 守卫（仅支持 PERSON 私聊）；③objectName→messageType 映射（不支持类型抛 20011）；④事务内幂等查重 / 私聊双边定位活跃会话（不存在抛 20020）/ 收发双方 vendorUserId 批量反查身份（发送方缺失抛 20021，接收方缺失不阻断）/ 落库 ImMessageDO（vendor=rongcloud/thirdPartyMessageId/messageStatus=NORMAL/sessionBizType 取自 session.getBizType()）/ 更新会话 latestMessageAt / 对非发送方自增未读数 / 构建 ImMessageContext；⑤**事务外触发 hookRegistry** |
| 🔴 `sendMessage` | `SendImMessageRequest request` | `void` | 校验发送方/接收方均为会话活跃参与者（缺失抛 20022/20023）；取双方 vendorUserId 后经 `rongCloudFacade.sendPrivateMessage` 发送 |
| 🆕 `sendSystemMessage` | `SendSystemMessageRequest request` | `void` | 系统身份给会话内某参与者发系统通知；仅校验接收方（缺失抛 20023）；发送方融云账号由 `resolveSystemVendorUserId` 解析；**系统消息只经融云下发、不落库** |
| 🆕 `resolveSystemVendorUserId`（私有）| — | `String` | 解析系统发送方融云账号；以 `server-name` 为 userId；首次发送时双重检查锁 + `volatile` 标志注册（进程内只注册一次）|
| 🆕 `batchSyncMessages` | `BatchSyncMessagesRequest request` | `void` | 🔴 **`@Transactional`**；批量同步 AI 阶段消息；session 不存在抛 `ImSyncSessionNotFoundException`；消息列表空直接返回；**Push 路径**（`updateOnDuplicate=true`）UPSERT 覆盖更新；**Pull 路径**幂等忽略 |
| 🆕 `buildImMessageDO`（私有）| `Long sessionId, SyncMessageDTO dto` | `ImMessageDO` | DTO→DO 映射：vendor 取 dto.vendor 缺省回退 copilot；senderId 缺省回填 copilot；senderVendorUserId 固定 copilot 占位；sentAt 缺失时回退 `new Date()`；contentPayload 经 `resolveContentPayload` |
| 🆕 `resolveContentPayload`（私有）| `SyncMessageDTO dto` | `String` | 解析 content_payload：经 `ImMessageConverter.toBodyDO` 转领域消息体，再由 `messageBodyConverter.toRongContentPayload` 反向序列化；null 时兜底 originalContentPayload |
| `getMessages` | `GetMessagesRequest request` | `MessagePageDTO` | 游标分页（向上加载更早消息）；解析页大小（1~50，缺省 20）；计算 7 天回溯起始时间；多取一条判断 hasMore；裁剪页数据并设置 nextCursor |
| 🆕 `getConversationMessages` | `GetConversationMessagesRequest request` | `MessagePageDTO` | 🆕 完整对话记录（STORY-017）：sender_type IN (USER, AI, CS_USER) 排除 SYSTEM；按 id 倒序游标分页；不截取时间；多取一条判断 hasMore；`excludeCsUserId` 非空时额外排除当前查看客服自己的 CS_USER 消息 |
| `getOldestMessageIdBySession` | `Long sessionId` | `Long` | 按 sessionId 查最老消息 ID；无消息返回 null；委托 `messageRepository.findOldestMessageIdBySessionId` |
| `getLatestMessageBySessionIds` | `List<Long> sessionIds` | `List<ImMessageDTO>` | 按 sessionId 列表批量查询每会话最新一条；空集合返回空列表 |
| `getMessagesByIds` | `List<Long> ids` | `List<ImMessageDTO>` | 按消息 ID 列表批量查询；空列表返回空；某 id 不存在则不出现在结果中 |
| 🔴 `batchLatestSideMessagesAfterStartAt` | `BatchLatestSideMessagesRequest request` | `List<LatestSideMessageDTO>` | 🆕 **STORY-020 接待中超时分侧计时 + end_message_id**；①过滤 null 及 sessionId/startAt 缺失项，构建 `LatestSideMessageQueryCriteria` 列表委托 `messageRepository.findLatestSideMessagesAfterStartAt` **纯取数**（1 次 DB 往返，DO 的 latest 字段此时为 null）；②对每条 DO 调用 `imMessageDomainService::fillLatest` **内存推导 latest**（业务规则在 Domain Service）；③经 `ImMessageConverter.toLatestSideDTOList` 转 DTO 列表；**CS 编排层做分侧计时与 end_message_id 计算** |
| `findByTicketRange` | `FindByTicketRangeRequest request` | `List<ImMessageDTO>` | 按工单消息范围（startMessageId~endMessageId）+ 游标分页查询 |
| `resolveSize`（私有）| `Integer size` | `int` | 页大小解析：null/<1→20，否则 min(size, 50) |
| `resolveSinceTime`（私有）| — | `Date` | 计算 7 天前起始时间（系统时区）|
| `toPageItemDTOList`（私有）| `List<ImMessageDO>` | `List<ImMessagePageItemDTO>` | 转分页项 DTO |

#### 🆕 ImUserOnlineStatusAppService（用户在线状态应用服务 — 1 方法）[已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/appservice/ImUserOnlineStatusAppService.java` |
| 类注解 | `@Service("imUserOnlineStatusAppService")` + `@Slf4j` + `@RequiredArgsConstructor` |
| 职责 | 🆕 处理融云用户在线状态回调；调用链路：融云回调 → interfaces converter → 本服务 → CsUserServiceFacade → CS SPI；本层负责**签名校验 + 回调明细校验 + 批内按客服用户聚合** |

**核心字段（4 个）：**

| 字段名 | 类型 | 值/说明 |
|--------|------|------|
| 🆕 `RONGCLOUD_STATUS_OFFLINE` | `int`（常量）| `0` |
| 🆕 `RONGCLOUD_STATUS_ONLINE` | `int`（常量）| `1` |
| 🆕 `CS_USER_FLAG` | `String`（常量）| `"CS_USER"`；`handleCallback` 遍历时先用此判断 userid 是否包含，不含则跳过 |
| `signatureValidator` | `RongCloudSignatureDomainService` | 融云回调签名校验领域服务 |
| `csUserServiceFacade` | `CsUserServiceFacade` | CS 用户服务防腐层 |
| `rongCloudAppSecret` | `String` | `@Value("${rongcloud.app-secret}")` |

**核心方法（1 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| 🔴 `handleCallback` | `ImUserOnlineStatusCallbackRequest request` | `void` | 🆕 **融云在线状态回调核心路径**：①签名校验；②遍历回调明细守卫：a) userid 空则 warn 跳过；b) **userid 不包含 CS_USER_FLAG 则 warn 跳过（非客服用户）**；c) `UpsertRongyunOnlineStatusDO.generateCsUserId` 解析客服业务 ID（格式 `CS_USER_{id}`，取最后一个 `_` 后的数字串），解析为空则 warn 跳过；③按客服用户 ID 聚合批内多条状态（**同一客服只保留一条 + 在线设备数 > 0 则认为在线 + 最新事件时间作为 lastSeenAt + 最新在线事件时间作为 lastRongcloudOnlineAt**）；④调 `csUserServiceFacade.upsertRongyunOnlineStatus` 批量写入聚合结果 |

#### ImSessionAppService / ImTokenAppService [据推断]

> 同事 life-im.md §5 包结构概览列出两个 AppService，方法级待探查 [据推断]。

### 5.3 Domain 层 [已确认/据推断]

#### 🆕 ImMessageDomainService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/message/service/ImMessageDomainService.java` |
| 职责 | 🔴 **IM 消息领域服务**；封装跨实体的消息业务规则；**STORY-020 三侧锚点推导全局末条**；语义前提：im_message 仅落 USER/CS_USER/AI，SYSTEM 消息不落库，故全局末条必在三侧之中，可在内存安全推导 |

**核心方法（1 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| 🔴 `fillLatest` | `LatestSideMessageDO bySender` | `void` | **从 user/cs/ai 三侧锚点中选 sent_at 最大（相同则 id 最大）者，原地填充 DO 的 latestMessageId / latestSentAt 字段；入参为 null 时直接返回** |

#### 🆕 CsUserServiceFacade（CS 子域防腐端口）[已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/cs/facade/CsUserServiceFacade.java` |
| 职责 | 🔴 CS 子域防腐层端口（ACL）；定义领域层与 CS 服务的交互契约，**屏蔽外部服务协议**，由 infrastructure 层 `CsUserServiceFacadeImpl` 实现 |

**核心方法（1 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| 🔴 `upsertRongyunOnlineStatus` | `List<UpsertRongyunOnlineStatusDO> statusDOList` | `void` | 🆕 批量写入融云在线状态到 CS 服务；调用方（应用层）已按客服用户聚合；**本端口保持一 DO 对一请求的映射** |

#### 🆕 UpsertRongyunOnlineStatusDO [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/cs/model/entity/UpsertRongyunOnlineStatusDO.java` |
| 职责 | 🆕 CS 子域在线状态领域实体；承载应用层按客服用户聚合后的单条融云在线状态 |

**核心字段（9 个）：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `userid` | `String` | 融云用户 ID（格式 csUserId-deviceId），保留原始值方便排查 |
| 🆕 `csUserId` | `String` | 从融云用户 ID 解析的客服用户 ID，CS 表业务主键 |
| `status` | `Integer` | 聚合在线状态：1 在线，0 离线 |
| `onlineDeviceCount` | `Integer` | 在线设备数，同批多端已聚合 |
| `os` | `String` | 在线客户端平台集合，英文逗号分隔 |
| `time` | `Long` | 本批最新事件时间戳（毫秒）|
| 🆕 `lastRongcloudOnlineAt` | `Date` | 最近一次融云在线事件时间；仅有在线设备时有值 |
| `clientIp` | `String` | 客户端 IP；CS SPI 暂未消费 |
| `sessionId` | `String` | 连接 ID；CS SPI 暂未消费 |

**核心方法（1 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| 🔴 `generateCsUserId`（static）| `String userid` | `String` | 从融云用户 ID（csUserId-deviceId 格式）截取客服用户 ID；**无分隔符时原值返回，避免异常数据致整批失败** |

### 5.4 Infrastructure 层 [已确认]

#### 🆕 ImMessageRepositoryImpl（13 个核心方法 + STORY-020 重构）[已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/repository/mysql/ImMessageRepositoryImpl.java` |
| 职责 | 🔴 消息仓储的 MySQL 实现；实现 `domain.message.repository.ImMessageRepository`；依赖 `ImMessageMapper` + `ImMessagePOConverter` / `LatestSideMessagePOConverter` |
| 类注解 | `@Repository` |

**核心字段（1 个）：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `imMessageMapper` | `ImMessageMapper` | `@Autowired` 注入 MyBatis Mapper |

**核心方法（13 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `listMessages` | `Long sessionId, Long afterMessageId, Date sinceTime, String direction, String senderType, int limit` | `List<ImMessageDO>` | 按会话查询；支持 afterMessageId / sinceTime / senderType 过滤；方向 + 条数限制 |
| `findByCursor` | `Long sessionId, Date sinceTime, Long cursor, int size` | `List<ImMessageDO>` | 🆕 游标分页（向上加载更早消息）：`sent_at >= sinceTime`，`cursor` 非空附加 `id < cursor`，按 id 倒序 |
| `existsBySourceEventId` | `String sourceEventId` | `boolean` | 基于 `LambdaQueryWrapper` 按 `source_event_id` 调用 `selectCount`；`count > 0` 判断存在 |
| `save` | `ImMessageDO message` | `void` | DO 转 PO 后 `insert`；**自增主键回写**（`message.setId(po.getId())`）|
| 🆕 `findExistingSourceEventIds` | `Collection<String> sourceEventIds` | `List<String>` | 批量幂等校验；null/空返回空列表；一次性查询已存在集合 |
| 🆕 `saveBatch` | `List<ImMessageDO> messages` | `void` | 🔴 **Pull 路径**批量保存（幂等忽略）；null/空直接返回；批量转 PO 后 `batchInsert`（**`ON DUPLICATE KEY UPDATE id=id`**）|
| 🔴 `batchUpsert` | `List<ImMessageDO> messages` | `void` | 🔴 **Push 路径**批量 UPSERT（**内容覆盖**）；`ON DUPLICATE KEY UPDATE` **仅覆盖** content_payload / original_content_payload / message_type / last_updated_date；**发送方身份字段（sender_type / sender_id 等）以首次写入为准不覆盖** |
| 🆕 `findLatestBySessionIds` | `List<Long> sessionIds` | `List<ImMessageDO>` | 按 sessionId 列表批量取每会话最新一条；用**关联标量子查询**取 `sent_at`/`id` 最大的一条 |
| `findOldestMessageIdBySessionId` | `Long sessionId` | `Long` | 查询指定会话最老消息 ID；`sessionId` null 返回 null；无消息 Mapper 返回 null |
| `findByTicketRange` | `Long startMessageId, Long endMessageId, Long cursor, int size` | `List<ImMessageDO>` | 按工单消息范围游标分页查询（id 升序）；endMessageId / cursor 动态条件 |
| 🆕 `findConversationMessagesByCursor` | `Long sessionId, Long cursor, int size, String excludeCsUserId` | `List<ImMessageDO>` | 🆕 **STORY-017** 完整对话记录游标分页：sender_type IN (USER, AI, CS_USER) 排除 SYSTEM；excludeCsUserId 非空时附加排除；cursor 非空附加 `id < cursor`；按 id 倒序 |
| 🔴 `findLatestSideMessagesAfterStartAt` | `List<LatestSideMessageQueryCriteria> queries` | `List<LatestSideMessageDO>` | 🆕 **STORY-020 重构**：按 (sessionId, startAt) 列表批量取每会话 USER/CS_USER/AI 各末条；null/空返回空列表；Mapper 标量相关子查询取三侧末条；结果经过滤后转 DO；**DO 的 latestMessageId/latestSentAt 此时为 null，需调用方执行 `ImMessageDomainService.fillLatest` 填充** |
| `findByIds` | `List<Long> ids` | `List<ImMessageDO>` | 按 ID 列表批量查询 |

#### 🆕 ImMessageMapper（11 个自定义方法）[已确认]

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `listMessages` | `@Param sessionId, afterMessageId, sinceTime, direction, senderType, limit` | `List<ImMessagePO>` | 按会话条件查询；XML `listMessages` |
| `findByCursor` | `@Param sessionId, sinceTime, cursor, size` | `List<ImMessagePO>` | 游标分页查询更早消息；XML `findByCursor` |
| `findExistingSourceEventIds` | `@Param("sourceEventIds") Collection<String>` | `List<String>` | 批量查询已存在 source_event_id；XML 同名 |
| 🆕 `batchInsert` | `@Param("list") List<ImMessagePO>` | `int` | 🆕 Pull 路径批量插入（`ON DUPLICATE KEY UPDATE id=id`）|
| 🆕 `batchUpsert` | `@Param("list") List<ImMessagePO>` | `int` | 🆕 Push 路径批量 UPSERT（**仅覆盖内容字段**）|
| `findLatestBySessionIds` | `@Param("sessionIds") List<Long>` | `List<ImMessagePO>` | 按 session_id 分组取每会话最新一条（关联标量子查询）|
| `findOldestMessageIdBySessionId` | `@Param("sessionId") Long` | `Long` | 取指定会话最老消息 ID（`ORDER BY id ASC LIMIT 1`）|
| `findByTicketRange` | `@Param startMessageId, endMessageId, cursor, size` | `List<ImMessagePO>` | 工单消息范围游标分页（id 升序）|
| 🆕 `findConversationMessagesByCursor` | `@Param sessionId, cursor, size, excludeCsUserId` | `List<ImMessagePO>` | 🆕 STORY-017 完整对话记录游标分页 |
| 🔴 `findLatestSideMessagesAfterStartAt` | `@Param("queries") List<LatestSideMessageQueryCriteria>` | `List<LatestSideMessagePO>` | 🔴 **STORY-020 重构** 批量取每会话 USER/CS_USER/AI 各末条（**标量相关子查询**）；**不再输出 latest_message_id / latest_sent_at**（latest 改由 `ImMessageDomainService.fillLatest` 内存推导）；`queries` 为空时派生表 `w` 仅含种子行，整体返回 0 行 |
| `findByIds` | `@Param("ids") List<Long>` | `List<ImMessagePO>` | 按 ID 列表批量查询 |

#### 🆕 CsUserServiceFacadeImpl [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/cs/facade/CsUserServiceFacadeImpl.java` |
| 职责 | 🆕 CS 用户服务防腐层实现；通过 `CsUserServiceClient` Feign 调用 CS SPI 更新融云在线状态 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `upsertRongyunOnlineStatus` | `List<UpsertRongyunOnlineStatusDO> statusDOList` | `void` | 批量写入；调用 `csUserServiceClient.upsertRongyunOnlineStatus`（Feign）|

#### 🆕 CsUserServiceClient [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/feign/CsUserServiceClient.java` |
| 目标 SPI | `icec-cloud-life-cs-spi` 的 `CsUserService` |
| 调用服务名 | `life-cs-service` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `upsertRongyunOnlineStatus` | `List<UpsertRongyunOnlineStatusRequest> requests` | `ApiResult<Void>` | 批量更新融云在线状态到 CS（被 CsUserServiceFacadeImpl 调用）|

### 5.5 Interfaces 层 [已确认/据推断]

#### 🆕 ImUserOnlineStatusServiceImpl [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-life-im-interfaces/src/main/java/com/casstime/cloud/life/im/interfaces/restful/ImUserOnlineStatusServiceImpl.java` |
| 实现 SPI | `icec-cloud-life-im-spi` 的 `ImUserOnlineStatusService` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| 🔴 `handleCallback` | `@RequestParam String appKey, String nonce, String timestamp, String signature, @RequestBody List<ImUserOnlineStatusCallbackItemDTO> items` | `ApiResult<Void>` | 🆕 **融云在线状态回调 Controller 入口**；用 `ImUserOnlineStatusCallbackConverter.toRequest` 装请求对象；委托 `ImUserOnlineStatusAppService#handleCallback` |

#### ImSessionServiceImpl / ImMessageServiceImpl / ImTokenServiceImpl [据推断]

> 结构与 ImUserOnlineStatusServiceImpl 类似；具体方法待探查 [据推断]。

#### ImExceptionHandler [据推断]

> 统一异常处理；具体 handler 方法待探查。

---

## 6. IM 域特有约束（🔴 编码必读）[已确认]

### 6.1 🔴 IM 域 8 条红线（违反即阻断）

| # | 红线 | 描述 | 静态扫描命令 |
|---|------|------|------------|
| 1 | **🔴 融云回调始终验签** | `handleCallback` 入口必须始终做签名校验；即使 channelType 不支持也先验签；绕过会导致任意伪造消息入库 | `grep "signatureValidator.validate" ImMessageAppService.java` |
| 2 | **🔴 ChannelType 守卫** | 本期仅支持 PERSON 私聊；非 PERSON 丢弃返回 200 | `grep "channelType" ImMessageAppService.java` |
| 3 | **🔴 事务内禁止远程调用 / MQ** | `batchSyncMessages` `@Transactional` 内不得调外部 Feign | `grep -B 1 "@Transactional" application/appservice/*.java` |
| 4 | **🔴 事务外触发 Hook** | 消息回调事务**提交后**才触发 `hookRegistry`，禁止事务内 | `grep "hookRegistry" ImMessageAppService.java` |
| 5 | **🔴 batchUpsert 仅覆盖内容字段** | Push 路径 UPSERT 不得覆盖发送方身份（sender_type / sender_id / vendor）；XML 注释明确说明 | 人工 review `ImMessageMapper.xml#batchUpsert` |
| 6 | **🔴 CS_USER 标识守卫** | `handleCallback` 必须用 `CS_USER_FLAG` 守卫，非客服 userid warn 跳过 | `grep "CS_USER_FLAG" ImUserOnlineStatusAppService.java` |
| 7 | **🔴 generateCsUserId 容错** | 无分隔符时原值返回，避免异常数据致整批失败 | `grep "generateCsUserId" UpsertRongyunOnlineStatusDO.java` |
| 8 | **🔴 STORY-020 内存推导 latest** | 不得用 SQL 窗口函数推导 latest；统一由 `ImMessageDomainService.fillLatest` 内存推导 | 跑 SQL review：`grep -i "window\\|rank\\|row_number" ImMessageMapper.xml` 应为空 |

### 6.2 🆕 6 种消息体类型多态约束

| # | 类型 | 说明 |
|---|------|------|
| 1 | `TEXT` | 文本消息 |
| 2 | `IMAGE` | 图片消息 |
| 3 | `FILE` | 文件消息 |
| 4 | `RICH_TEXT` | 富文本消息 |
| 5 | `MARKDOWN` | Markdown 消息 |
| 6 | `COMPOSITE` | 复合消息 |

> **🆕 关键约束**：`ImMessageConverter.toBodyDO` / `ImMessageBodyConverter.toRongContentPayload` 必须覆盖**全部 6 种类型**，缺失会抛运行时异常。

### 6.3 IM 域工程特定静态扫描（🔴 编码完成必跑）

```bash
# 1. 始终验签（🔴 STORY 冰山 ST-003 必查）
grep -n "signatureValidator.validate" icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/appservice/ImMessageAppService.java
# 期望：handleCallback 入口首行

# 2. channelType 守卫
grep -n "channelType" icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/appservice/ImMessageAppService.java
# 期望：仅 PERSON 合法

# 3. 事务外触发 Hook（不在事务内调 hookRegistry）
grep -B 5 "hookRegistry" icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/appservice/ImMessageAppService.java
# 期望：hookRegistry 调用在 transactionTemplate.execute 之外

# 4. batchUpsert 仅覆盖内容字段（人工 review SQL）
grep -A 5 "batchUpsert" icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/dao/xml/ImMessageMapper.xml
# 期望：ON DUPLICATE KEY UPDATE 子句仅含 content_payload / original_content_payload / message_type / last_updated_date

# 5. CS_USER 守卫（ImUserOnlineStatusAppService）
grep -n "CS_USER_FLAG" icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/appservice/ImUserOnlineStatusAppService.java
# 期望：handleCallback 中有 userid 包含 CS_USER_FLAG 的判断

# 6. STORY-020 latest 内存推导（禁止 SQL 窗口函数）
grep -i "window\\|rank\\|row_number" icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/dao/xml/ImMessageMapper.xml
# 期望输出为空

# 7. 消息体 6 类型覆盖（人工 review Converter）
grep -E "TEXT|IMAGE|FILE|RICH_TEXT|MARKDOWN|COMPOSITE" icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/message/converter/ImMessageConverter.java
# 期望：6 种类型都在 switch / if-else 分支

# 8. 消息发送方身份不得被 UPSERT 覆盖
grep -A 10 "batchUpsert" icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/message/persistence/dao/xml/ImMessageMapper.xml | grep "ON DUPLICATE KEY UPDATE"
# 期望：仅含 content_payload / original_content_payload / message_type / last_updated_date；不含 sender_type / sender_id / vendor
```

---

## 7. 上下游契约 [已确认/据推断]

### 7.1 对外暴露（SPI / Controller）

| 类型 | 接口名 | URL / 服务名 |
|------|--------|------------|
| SPI | `ImSessionService` | 服务名 `life-im-service` |
| SPI | `ImMessageService` | 服务名 `life-im-service` |
| SPI | `ImTokenService` | 服务名 `life-im-service` |
| 🆕 SPI | `ImUserOnlineStatusService` | 服务名 `life-im-service`（融云在线状态回调）|
| Controller | `ImSessionServiceImpl` | `/life-im/...` |
| Controller | `ImMessageServiceImpl` | `/life-im/...` |
| Controller | `ImTokenServiceImpl` | `/life-im/...` |
| 🆕 Controller | `ImUserOnlineStatusServiceImpl` | `/life-im/...`（融云在线状态回调入口）|

### 7.2 对内消费（Feign Client + 防腐层）

| SPI / 服务 | 方法 | 本工程 Feign Client / 防腐层 |
|------------|------|----------------------------|
| `CsUserService`（life-cs-spi）| `upsertRongyunOnlineStatus` | 🆕 `CsUserServiceClient` + `CsUserServiceFacadeImpl` |
| 🆕 RongCloud SDK | 注册 / Token / 发消息 | 🆕 `RongCloudClient` + `RongCloudFacadeImpl`（apdapter.rongcloud 防腐层）|
| `life-cs-service` | 接收消息事件（Hook 路径）| `CsMessageNotifyHook` |

### 7.3 🆕 融云回调签名校验流程

```
融云 HTTP 回调 (POST /im/user-online-status/callback)
  → query 参数: appKey + nonce + timestamp + signature
  → body: items (JSON 数组)
  → ImUserOnlineStatusServiceImpl#handleCallback
    → ImUserOnlineStatusCallbackConverter.toRequest
      → ImUserOnlineStatusCallbackRequest (SPI DTO)
    → ImUserOnlineStatusAppService#handleCallback
      → RongCloudSignatureDomainService.validate (签名校验)
      → 遍历 items 守卫: userid 空 / 非 CS_USER / generateCsUserId 失败 → skip
      → 按 csUserId 聚合: 同一客服只保留一条; 在线设备数 > 0 → online; 最新事件时间
      → CsUserServiceFacade.upsertRongyunOnlineStatus (聚合后的 List<DO>)
        → CsUserServiceFacadeImpl → CsUserServiceClient (Feign)
          → life-cs-service: CsUserService.upsertRongyunOnlineStatus
            → CsUserAppService.upsertRongyunOnlineStatus (单条/批量)
              → CsUserRepositoryImpl.findById + CsUserStatusRongyunSyncRepositoryImpl.batchUpsert
```

---

## 8. 关键数据库表清单 [已确认/据推断]

> 🆕 IM 域核心表（仅列已确认的；详细字段在 §4 实体类中）

| 表名 | 关键字段 | 业务含义 |
|------|---------|---------|
| 🆕 `im_message` | id / session_id / source_event_id (unique) / sender_type (USER/CS_USER/AI/SYSTEM) / sender_id / sender_vendor_user_id / message_type (TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE) / message_status (NORMAL) / content_payload / original_content_payload / sent_at / session_biz_type / third_party_message_id | IM 消息主体（落库仅 USER/CS_USER/AI；SYSTEM 不落库）|
| `im_session` | id / session_id / biz_type / latest_message_at / participants | 会话 |
| `im_session_participant` | session_id / principal_type / principal_id / unread_count / status | 会话参与者 + 未读数 |
| `im_identity_registry` | vendor / vendor_user_id (unique) / principal_type / principal_id | 身份注册（vendorUserId 反查主体身份）|
| 🆕 `latest_side_message` (PO, 不落库) | session_id / start_at / user_message_id / user_sent_at / cs_message_id / cs_sent_at / ai_message_id / ai_sent_at / (🆕 STORY-020 移除 latest_message_id / latest_sent_at 列) | 每会话三侧末条（STORY-020；latest 改由 Domain Service 内存推导）|

---

## 9. 本工程缺口与待补充

| # | 缺口 | 优先级 | 状态 |
|---|------|-------|------|
| 1 | `icec-cloud-life-im-spi` 升级至 1.1 后新增 DTO 完整清单（仅推断含 `ImUserOnlineStatusCallbackRequest`）| 🟠 P1 | 待补 |
| 2 | 融云 app-key / app-secret / api-url 实际注入位置（@Value 或其他方式）| 🟠 P1 | 待补 |
| 3 | 融云 server-sdk-java 依赖的实际引入方式（传递依赖？哪个 starter？）| 🟠 P1 | 待补 |
| 4 | `ImSessionAppService` 完整方法列表（会话创建/批量查询/参与者管理/未读数编排）| 🟡 P2 | 待补 |
| 5 | `ImTokenAppService` 完整方法列表（融云 Token 获取与账号注册）| 🟡 P2 | 待补 |
| 6 | `ImExceptionHandler` 完整 handler 方法 | 🟡 P2 | 待补 |
| 7 | `ImSessionDomainService` 完整方法列表（私聊双边唯一活跃会话定位）| 🟡 P2 | 待补 |
| 8 | `RongCloudConfig` / `RongCloudClient` / `RongCloudFacadeImpl` / `ImMessageBodyConverterImpl` 完整方法 | 🟠 P1 | 待补 |
| 9 | `RongCloudSignatureDomainService` 签名校验算法（HMAC-SHA1？）| 🟡 P2 | 待补 |
| 10 | 父 POM description 占位符 "xxx????" 待填 | 🟢 P3 | 待修 |
| 11 | 消息体 6 种类型的具体 schema（TEXT/IMAGE/FILE 的 content_payload JSON 格式）| 🟡 P2 | 待补 |
| 12 | `CsMessageNotifyHook` 实现细节（按 sessionBizType 过滤逻辑）| 🟡 P2 | 待补 |
| 13 | 错误码（20011 objectName 不支持 / 20020 私聊双边会话不存在 / 20021 发送方身份缺失 / 20022 发送方非活跃参与者 / 20023 接收方非活跃参与者）| 🟠 P1 | 待补 |
| 14 | STORY-020 latest 内存推导在 CS 编排层的具体使用方式（分侧计时 + end_message_id 计算）| 🟠 P1 | 待补 |
| 15 | spring-boot-maven-plugin 2.7.5 与 Spring Boot 1.5.7 版本不一致是否需调整 | 🟡 P2 | 待确认 |

---

## §A 关键词反向索引 [已确认/据推断]

| 关键词 | 出现位置 |
|--------|---------|
| `ImMessageAppService` | §4 / §5.2（16 方法 + 19 字段常量）|
| `ImUserOnlineStatusAppService` | §4 / §5.2（1 方法）|
| `ImMessageDomainService.fillLatest` | §4 / §5.3（**STORY-020 内存推导 latest**）|
| `CsUserServiceFacade` | §4 / §5.3（CS 防腐端口）|
| `CsUserServiceFacadeImpl` | §4 / §5.4 |
| `UpsertRongyunOnlineStatusDO` | §4 / §5.3（9 字段 + generateCsUserId 静态方法）|
| `MessageTypeEnum` | §4（🆕 TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE 6 类型）|
| `MarkdownMessageBodyDO` / `LatestSideMessageDO` | §4 |
| `ImMessageRepositoryImpl` | §4 / §5.4（13 方法 + STORY-020 重构）|
| `batchUpsert`（**仅覆盖内容字段**）| §5.4（🔴 不覆盖 sender_type/sender_id/vendor）|
| `findLatestSideMessagesAfterStartAt` | §5.4（**STORY-020 标量相关子查询**）|
| `findConversationMessagesByCursor` | §5.4（**STORY-017 完整对话记录**）|
| `ImMessageMapper.xml` | §4（11 个 SQL + STORY-020 重构）|
| `apdapter/rongcloud` | §4（🆕 融云防腐层包结构）|
| `RongCloudConfig` / `RongCloudClient` / `RongCloudFacadeImpl` / `ImMessageBodyConverterImpl` | §4 / §5.4 |
| `RongBody` | §4（`@JsonInclude(NON_NULL)`）|
| `rongcloud.app-key` / `rongcloud.app-secret` / `rongcloud.api-url` / `rongcloud.server-name` | §1.1（@Value 注入）|
| `rongcloud.server-sdk-java` | §3（版本待确认）|
| `icec-cloud-life-im-spi 1.1` | §3 / §4 |
| `icec-cloud-life-cs-spi 1.1` | §3 / §4 |
| `ImUserOnlineStatusServiceImpl` | §4 / §5.5（融云在线状态回调 Controller）|
| `ImUserOnlineStatusCallbackConverter` | §5.1（融云 query/body → SPI Request）|
| `CsMessageNotifyHook` | §4 / §7.2（向 CS 推送消息事件）|
| `CsUserServiceClient`（Feign）| §4 / §5.4 |
| `VENDOR_RONGCLOUD` / `DEFAULT_VENDOR` | §5.2（vendor 常量）|
| `CS_USER_FLAG` | §5.2（客服用户守卫）|
| `systemAccountRegistered`（`volatile boolean`）| §5.2（系统账号融云注册缓存）|
| `signatureValidator` / `RongCloudSignatureDomainService` | §5.2（签名校验）|
| `hookRegistry` | §5.2（事务外触发）|
| `transactionTemplate` | §5.2（编程式事务）|
| `MessageHookRegistry` | §5.2 |
| `STORY-020` | §5.3 / §5.4 / §6.1 / §8（多处）|
| `STORY-017` | §5.4（完整对话记录）|
| 错误码 20011/20020/20021/20022/20023 | §9（待补）|

---

## §B 本工程更新日志（合并到主体 update-log）

> 详见主体 `icec-cloud-life.update-log.md`；本节列本工程特有的变更摘要。

| 日期 | 变更摘要 |
|------|---------|
| 2026-06-26 | 🆕 按 schema v3 §15 首次生成工程级子文件；来源：同事 life-im.md（92KB / 1127 行）+ ae-sdd life.assets.md §2 |
| 2026-06-24 | 🔴 SPI 升级 1.0.2 → 1.1：新增 `ImUserOnlineStatusCallbackRequest` + `ImUserOnlineStatusCallbackItemDTO` |
| 2026-06-24 | 🔴 STORY-020 重构：`LatestSideMessagePO` 移除 latest 列；`findLatestSideMessagesAfterStartAt` SQL 改标量相关子查询；latest 改由 `ImMessageDomainService.fillLatest` 内存推导 |
| 2026-06-24 | 🆕 新增 `CsUserServiceFacadeImpl` + `CsUserServiceClient` + `CsMessageNotifyHook`；infrastructure 引入 `icec-cloud-life-cs-spi 1.1` |
| 2026-06-24 | 🆕 新增 `ImUserOnlineStatusAppService` + `ImUserOnlineStatusServiceImpl`（融云在线状态回调 Controller）+ `ImUserOnlineStatusCallbackConverter` |