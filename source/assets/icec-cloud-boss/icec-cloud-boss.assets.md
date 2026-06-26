---
name: icec-cloud-boss-project-assets
description: icec-cloud-boss 项目资产实例 — 基于 Explore Agent 2026-06-04 探查，含 22 个微服务、5 个分层精确包路径、7 类命名约定、8 类工程约束、10 个缺口。供本项目所有 Code Plan 引用。
---

# icec-cloud-boss Project Assets — 项目资产实例

> **本文件是 `project-assets-schema.md` 的首份实例**，按 schema 12 节结构 + 附录 JSON 填写。
>
> **相对路径基准：** 本文件位于 `skills/ae-sdd/project-assets/icec-cloud-boss/`，引用同目录下 schema/template 用 `../../strategies/...`。
>
> **探查时间：** 2026-06-04
> **探查 Agent：** Workflow 一轮 Explore Agent
> **本实例可作为后续新项目构建项目资产的"参考填法"**

---

## 0. 摘要与使用场景

| 维度 | 内容 |
|------|------|
| 何时需要查 | ④bis Code Plan / ⑤ Coding / ⑦ Code Review |
| 谁负责写 | 架构组 + 域负责人（boss-user / life-cs / life-im / ...） |
| 与 `constraints/` 的关系 | constraints/ 在 `D:\Item\icec-cloud-boss\document\life-team-ai-standards\constraints\`，本文件把规则映射到本项目代码事实 |
| 关键不变量 | 本文件不重复定义 rules（如"事务在 AppService"不在本文件重写），只把规则映射到具体包路径/类名 |

---

## 1. 项目资产元信息

| 字段 | 值 |
|------|---|
| projectKey | `icec-cloud-boss` |
| projectName | `icec-cloud-boss`（含 life 子产品线） |
| gitPath | `d:\Item\icec-cloud-boss` |
| productLine | `boss` / `life` |
| profile | `dev, test, prod, beta-kunlun` |
| mainClass | 各 Service `-service` 子模块的 `Bootstrap.java`（典型） |
| packaging | `jar` |
| portRange | `12002-12004`（boss）/ `10087-10097`（life BFF） |
| lastAuditedAt | `2026-06-24` |
| owner | 架构组 / 各域负责人 |

---

## 2. 微服务清单

| name | responsibility | port | contextPath | hasBff | callChain | dependsOnSpi |
|------|---------------|------|-------------|--------|-----------|--------------|
| icec-cloud-boss-user | Boss 用户域（用户/角色/菜单/菜单图标/权限/扩展信息）；Feign 调用 icec-cloud-life-cs | 12004 | /boss-user | true | service（DDD 四层）+ Feign cs-spi | icec-cloud-life-cs-spi |
| icec-cloud-boss-user-bff | 面向管理后台前端聚合 Boss 用户/角色/菜单/图标接口，统一 SPI Feign | 12003 | /boss-user-bff | true | api（*-bff-api）→ BFF → SPI → Service | icec-cloud-boss-user-spi, icec-cloud-boss-log-spi | [详见](icec-cloud-boss.icec-cloud-boss-user-bff.assets.md) |
| icec-cloud-boss-auth-bff | 登录鉴权 BFF（密码登录/Token 颁发/刷新/登出，Cookie security_context JWT） | 12002 | /boss-auth-bff | true | BFF（唯一公开 /public/auth/login/password） | - |
| icec-cloud-boss-notification-bff | Boss 端通知 BFF（通知查询/操作日志） | 10091 | /boss-notification-bff | true | api → BFF → SPI → Service | icec-cloud-life-notification-spi, icec-cloud-life-ops-notification-spi |
| icec-cloud-boss-agent-workbench-bff | Agent 工作台 BFF（坐席/工单工作台） | 10097 | /boss-agent-workbench-bff | true | BFF（聚合 life-cs / life-im） | - |
| icec-cloud-boss-security | 安全公共组件（TokenService / @SkipAuth / @RequiresPermissions / @NeedLogin） | - | - | false | 被所有 BFF 与 Service 依赖 | - |
| icec-cloud-boss-abnormal | Boss 异常处理域 Service（DDD 多模块：-application/-domain/-infrastructure/-service/-web/-web-api）；web 暴露 `/abnormal-api` | 6603 | /boss-abnormal + /abnormal-api (web) | false | Service + Web（双启动模块） | - |
| icec-cloud-boss-webagent | WebAgent（网关 / Agent 入口） | 未读到（仅有 application.yml 无 bootstrap.yml） | /boss-webagent | false | 网关层 | - |
| icec-cloud-life-cs | 客服域 Service（工单/会话/坐席/状态机/通知编排） | 20092 | /life-cs | false | Service（DDD 四层）+ cs-spi 暴露 | - |
| icec-cloud-life-im | IM 域 Service（会话/消息/融云/参与者） | 20093 | /life-im | false | Service（DDD 四层）+ im-spi 暴露 | - |
| icec-cloud-life-im-bff | IM BFF（面向工作台/APP 的 IM 接口聚合） | 10096 | /life-im-bff | true | api（life-api/im-bff-api）→ BFF → im-spi → Service | icec-cloud-life-im-spi |
| icec-cloud-life-vehicle | 车辆域 Service（DDD 四层） | 20087 | /life-vehicle | false | Service（DDD） | - |
| icec-cloud-life-vehicle-bff | 车辆 BFF | 10087 | /vehicle-bff | true | api → BFF → vehicle-spi → Service | icec-cloud-life-vehicle-spi |
| icec-cloud-life-user | 2C 端用户域 Service（DDD 多模块：service 6602 + web 6001 + /user-api） | 6602 (service) / 6001 (web) | /life-user + /user-api (web) | false | Service + Web（双启动模块） | - |
| icec-cloud-life-workticket | 工单域 Service（DDD 四层） | 20090 | /life-workticket | false | Service（DDD） | - |
| icec-cloud-life-notification | 通用通知 Service（极光/融云/内部事件）；web 暴露 `/notification-api` | 6605 | /life-notification + /notification-api (web) | false | Service + Web | - |
| icec-cloud-life-ops-notification | 运营通知 Service（DDD 四层） | 20091 | /life-ops-notification | false | Service（DDD） | - |
| icec-cloud-life-spi（聚合 SPI） | SPI 父工程（11 个子模块 SPI；聚合 `icec-cloud-life-spi-service` 端口 6099） | 6099（聚合 service） | - | false | SPI（被 BFF 与 Service Feign 消费） | - |
| icec-cloud-life-api（聚合 API） | 2C 端 API 聚合工程（auth-bff-api / content-feed-bff-api / im-bff-api / user-journey-bff-api / vehicle-bff-api / workticket-bff-api）；`icec-cloud-life-api-service` 启动 | 6100 | - | false | *-bff-api → api-service → SPI → Service | 多个 SPI |
| icec-cloud-boss-api（聚合 API） | Boss 端 API 聚合工程（agent-workbench-bff-api / auth-bff-api / configuration-bff-api / log-bff-api / notification-bff-api / user-bff-api / vehicle-bff-api / workticket-bff-api）；`icec-cloud-boss-api-service` 启动 | 6101 | - | false | *-bff-api → api-service → SPI → Service | 多个 SPI |
| boss-common | 公共工具/基础包（ApiResult / PagedModels / PageRequest） | - | - | false | 基础库（被所有工程依赖） | - |

**微服务总数：22 个**（含 6 个聚合工程 + 1 个公共库 + 1 个安全组件 + 14 个业务 Service/BFF）
**端口段：**
- 6000-6099：基础聚合（6100 life-api / 6101 boss-api / 6099 life-spi 聚合 / 6001 life-user-web / 6602 life-user-service / 6603 boss-abnormal-service / 6605 life-notification-service）
- 10000-10099：BFF（10087 vehicle-bff / 10091 boss-notification-bff / 10096 life-im-bff / 10097 boss-agent-workbench-bff）
- 12000-12099：核心 BFF（12002 boss-auth-bff / 12003 boss-user-bff / 12004 boss-user-service）
- 20000-20099：业务 Service（20087 life-vehicle / 20090 life-workticket / 20091 life-ops-notification / 20092 life-cs / 20093 life-im）

**双工程结构（Service + Web）注意：** boss-abnormal、life-user、life-notification 三个 Service 采用"双启动模块"（`-service` 后端服务 + `-web` Web 暴露层），`-web` 模块通过 `context-path` 直接暴露 REST API（不经过 BFF）。

---

## 3. 抽象分层 → 项目分层映射（粗粒度）

> 行=抽象 4 层+可选 2 类；列=本项目对应的工程模块。详细包路径见 §4。

| 抽象层 | 本项目对应工程模块 | 备注 |
|--------|------------------|------|
| 请求处理（Interfaces） | `{module}/icec-cloud-{product}-{domain}-interfaces` | 不含 BFF 入口 |
| 业务编排（Application） | `{module}/icec-cloud-{product}-{domain}-application` | 事务在 AppService |
| 领域逻辑（Domain） | `{module}/icec-cloud-{product}-{domain}-domain` | 充血模型 |
| 基础能力（Infrastructure） | `{module}/icec-cloud-{product}-{domain}-infrastructure` | 仅存取语义 |
| 跨模块 SPI（可选） | `icec-cloud-life-spi/icec-cloud-{product}-{domain}-spi` | `ServiceProviderConstants` |
| BFF 入口（可选） | `icec-cloud-{product}-{domain}-bff` | 仅当 hasBff=true |

**典型例子（icec-cloud-boss-user）：**
- Interfaces → `icec-cloud-boss-user/icec-cloud-boss-user-interfaces`
- Application → `icec-cloud-boss-user/icec-cloud-boss-user-application`
- Domain → `icec-cloud-boss-user/icec-cloud-boss-user-domain`
- Infrastructure → `icec-cloud-boss-user/icec-cloud-boss-user-infrastructure`

---

## 4. DDD 内部分层落点（细粒度）

> 基于冰山模块 `icec-cloud-boss-user` 抽取。**类角色 → 精确包路径** 是项目资产"最核心的可复用部分"。

| 类角色 | 精确包路径 | 典型类名 | 放什么 / 不放什么 |
|--------|-----------|---------|------------------|
| **Interfaces** | | | |
| Rest 实现类 | `icec-cloud-boss-user-interfaces/src/main/java/com/casstime/cloud/boss/user/interfaces/restful/` | `BossUserServiceImpl` / `BossMenuServiceImpl` / `BossRoleServiceImpl` / `BossUserInfoServiceImpl` / `BossMenuIconServiceImpl`（implements SPI 接口 + `@RestController`） | 仅协议适配，不写业务规则 |
| Event Handler | `icec-cloud-boss-user-interfaces/src/main/java/com/casstime/cloud/boss/user/interfaces/eventhandlers/` | `{Resource}EventHandler` | 事件接入入口 |
| **Application** | | | |
| AppService | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/appservice/` | `BossUserAppService` / `BossMenuAppService` / `BossRoleAppService` / `BossMenuIconAppService` | 事务、编排、调 Domain 顺序 |
| Converter | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/converter/` | `BossUserConverter` / `BossMenuConverter`（`@UtilityClass` + 静态方法 toDTO/toCommand） | DO↔DTO |
| Publisher | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/publisher/` | `ApplicationEventPublisher` | 跨域事件发布 |
| Command VO | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/vo/command/` | `{Resource}Command` | 写命令对象 |
| Query VO | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/vo/query/` | `BossUserQuery` / `BossMenuQuery` / `BossRoleQuery` | 读命令对象 |
| **Domain** | | | |
| Domain Object | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/entity/` | `BossUserDO` / `BossRoleDO` / `BossMenuDO` / `BossMenuIconDO` / `BossUserExtensionDO` / `BossRoleClosureDO` / `BossRoleMenuDO` / `BossUserRoleDO` / `BossUserApiPermsDO` / `BossRoleTreeDO`（充血，`@Data`，业务方法不以 get/set 开头） | 业务逻辑 |
| Value Object | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/value/` | `BossUserQuery` / `BossMenuQuery` / `BossRoleQuery` | 不可变值对象 |
| Enum | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/enums/` | `{Resource}MessageEnum` | (key, value) 双字段 |
| Error Enum | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/enums/error/` | `BossUserErrorCode` (11101-11107) / `BossRoleErrorCode` / `BossMenuErrorCode` / `BossMenuIconErrorCode` / `BossCommonErrorCode` | 错误码枚举（5 位分段 11xxx 用户段） |
| Context | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/context/` | `{Resource}Context` | 上下文对象 |
| Repository 接口 | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/repository/` | `BossUserRepository` / `BossMenuRepository` / `BossRoleRepository` / `BossMenuIconRepository` / `BossUserRoleRepository` / `BossRoleMenuRepository` / `BossRoleClosureRepository` / `BossUserExtensionRepository`（**仅接口**） | 不允许放业务规则 |
| Domain Service | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/service/` | `BossUserDomainService` / `BossRoleDomainService` / `BossMenuDomainService` / `BossMenuIconDomainService` | 跨聚合业务规则 |
| Event | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/event/` | `{Resource}CreatedEvent` | 领域事件定义 |
| Exception | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/exception/` | `BossDomainException` | 领域异常 |
| Facade | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/facade/` | `LoginLockFacade` | 跨域服务抽象接口 |
| **Infrastructure** | | | |
| Config | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/config/` | `MybatisPlusConfig` | Spring 配置 |
| Feign Client | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/feign/` | `CsUserClient` | 调外部 Service |
| Publisher (MQ) | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/messaing/publisher/` | `KafkaDomainEventPublisher` / `KafkaApplicationEventPublisher` | 事件发布 |
| PO | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/entity/` | `BossUserPO`（`@TableName("boss_user")`） / `BossRolePO` / `BossMenuPO` / `BossMenuIconPO` / `BossUserRolePO` / `BossRoleMenuPO` / `BossRoleClosurePO` / `BossUserExtensionPO` | 贫血模型，对应表 |
| Mapper | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/dao/mapper/` | `BossUserMapper`（extends `BaseMapper<BossUserPO>`） | MyBatis 映射 |
| DataConverter | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/converter/` | `BossUserDataConverter` / `BossMenuDataConverter` / `BossMenuIconDataConverter` / `BossUserExtensionDataConverter` | PO↔DO |
| Repository Impl (MySQL) | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/repository/mysql/` | `BossUserRepositoryImpl extends ServiceImpl<BossUserMapper, BossUserPO> implements BossUserRepository`（`@Repository + @RequiredArgsConstructor`） | 仓储方法名：`findByXxx / save / update / updateStatus` |
| Facade Impl | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/facade/` | `LoginLockFacadeImpl` | 异常时返回 null/空集合/Result.error |
| **SPI（被消费方）** | | | |
| SPI Service 接口 | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/service/` | `BossUserService` / `BossUserManagementService` / `BossUserInfoService` | Feign 接口 |
| DTO | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/dto/` | `BossUserDTO` / `BossUserManagementDTO` / `BossUserApiPagedModels` | 跨服务传输 |
| Request | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/request/` | `BossUserManagementRequest` | 请求对象 |
| Constants | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/` | `ServiceProviderConstants`（`BOSS_USER_SERVICE = "boss-user-service"`） | **禁硬编码服务名** |
| **BFF** | | | |
| Rest Impl | `icec-cloud-boss-user-bff/src/main/java/com/casstime/cloud/boss/bff/user/interfaces/restful/` | `BossUserManagementRestImpl` / `BossMenuRestImpl` / `BossRoleRestImpl` / `BossMenuIconRestImpl`（implements *-bff-api 中的 `{Resource}Rest`） | BFF 控制器 |
| Converter (BFF) | `icec-cloud-boss-user-bff/src/main/java/com/casstime/cloud/boss/bff/user/interfaces/converter/` | `BossUserManagementConverter` / `ResultConverter` | Request↔VO |
| AppService (BFF) | `icec-cloud-boss-user-bff/src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/` | `BossUserManagementAppService` / `BossMenuAppService` / `BossRoleAppService` | BFF 编排 |
| Facade (BFF) | `icec-cloud-boss-user-bff/src/main/java/com/casstime/cloud/boss/bff/user/application/facade/` | `{Resource}Facade` | 抽象外部服务 |
| Feign Client (BFF) | `icec-cloud-boss-user-bff/src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/feign/` | `BossUserManagementClient extends BossUserManagementService`（`@FeignClient(name="boss-user-service", url=...)`） | 调 Service |
| OperationLog Capability | `icec-cloud-boss-user-bff/src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/operationlog/capability/` | `RoleMenuOperationLoggable` / `RoleOperationLoggable` / `MenuOperationLoggable` | 操作日志能力 |

---

### 4.5 分层职责硬约束（🔴 违反即阻断，STORY-021-BE 沉淀）

> **本节定义 icec-cloud-boss 项目当前职责边界**。通用分层原则见 `coding-skill.md` §6.1 分层职责红线；**项目特定的细化职责边界**在本节。
> 编码前必读，编码后必跑全切面核查闸（coding-skill §Step 9）以代码为锚反查。

#### 4.5.1 判定口诀表（项目特定）

| 这段代码是… | 归属层 | 落点 | 关键判定 |
|---|---|---|---|
| 业务规则 / 能不能 / 算什么（状态能否流转、金额怎么算、不变量校验） | **Domain** | 实体充血方法 / DomainService | 业务不变量校验 |
| 先做A再做B / 协调谁调谁 / 事务从哪到哪 / 转 DTO | **Application** | AppService | 跨域编排、事务边界 |
| **读 Repository 返回值 + 简单转换（字段提取 / Map 组装 / DTO 映射）** | **Repository** | **Repository 接口（Domain 层）声明方法 + RepositoryImpl 实现** | **🔴 应用层禁止做此封装——这是 STORY-021-BE 暴露的常见错误** |
| 把数据存进去 / 取出来 / 转 PO↔DO 格式 / 拼查询条件 | **Repository** | RepositoryImpl | 数据访问封装 |
| 参数格式校验（@Valid） | **Interfaces** | Controller/Impl | 协议层职责 |

#### 4.5.2 禁止事项（项目特定，本项目严禁）

- ❌ 在 Application 写领域规则（下沉到 Domain）
- ❌ 在 Application 写 SQL/持久化细节
- ❌ **在 Application 写纯数据访问封装（字段提取/Map 组装）** —— 下沉到 Repository（🔴 STORY-021-BE 错误）
- ❌ 在 Domain 串多个外部服务的编排
- ❌ 在 Domain 出现 PO/DTO/SQL
- ❌ 在 Repository 写业务规则或状态流转判断
- ❌ BFF 跨 Service 直连 DB（必须经 Feign SPI）
- ❌ BFF 写 `@Transactional`（事务在 Service AppService）

#### 4.5.3 典型反模式（项目踩坑沉淀）

| 反模式 | 真实表现 | 正确做法 |
|--------|---------|---------|
| **AppService 加 `findById` / `getLatestMessageAt` 等纯读方法** | Application 层做"调 Repository + 转换字段" | 在 Repository 接口声明方法，RepositoryImpl 实现；AppService 保持原状 |
| **AppService 内联 SQL 字符串拼接** | "WHERE id=" + id | 用 MyBatis Mapper 配 `#{}` 参数化 |
| **Domain Service 调 Feign Client** | 领域规则里调外部服务 | 跨服务调用下沉到 AppService |

---

## 5. 命名约定

| 对象 | 命名模板 | 本项目例子 | 反例 |
|------|---------|----------|------|
| Controller (BFF) | `{Resource}RestImpl` | `BossUserManagementRestImpl` | ❌ `UserController` |
| AppService (BFF) | `{Resource}{Action}AppService` | `BossUserManagementAppService` | ❌ `UserService`（歧义） |
| AppService (Service) | `{Resource}AppService` | `BossUserAppService` | ❌ `BossUserManager`（动名词） |
| Domain Object | `{Resource}DO` | `BossUserDO` | ❌ `BossUser`（缺 DO 后缀） |
| Persistent Object | `{Resource}PO` | `BossUserPO` | ❌ `BossUserEntity` |
| Repository | `{Resource}Repository` | `BossUserRepository` | ❌ `BossUserDao` |
| Repository Impl | `{Resource}RepositoryImpl` | `BossUserRepositoryImpl` | — |
| Converter (Application) | `{Resource}Converter` | `BossUserConverter` | — |
| Data Converter (Infra) | `{Resource}DataConverter` | `BossUserDataConverter` | — |
| Feign Client | `{Resource}{Action}Client` | `BossUserManagementClient` | — |
| Error Code | `{Resource}ErrorCode` | `BossUserErrorCode` | — |
| Domain Exception | `{Resource}DomainException` | `BossDomainException` | — |
| Facade | `{Resource}Facade` / `{Resource}FacadeImpl` | `LoginLockFacade` / `LoginLockFacadeImpl` | — |
| OperationLog Capability | `{Resource}OperationLoggable` | `RoleOperationLoggable` | — |

**反例汇总（本项目常见违规）：**
- ❌ 用 `LocalDateTime` → ✅ 用 `java.util.Date`
- ❌ 用 MapStruct → ✅ 显式 Converter
- ❌ 用 `@Scheduled` → ✅ job-spring-boot-starter
- ❌ `BossUserManager` / `BossUserHandler`（动名词/名词）→ ✅ `BossUserAppService`
- ❌ `private` 字段无 `@Getter` → ✅ `@Data`

---

## 6. 工程约束（继承自 constraints/，按本项目裁剪+补缺）

### 6.1 分层架构（constraints/layered-arch.md）

- 外部请求 → API（聚合工程）→ BFF → SPI（Feign）→ Service（DDD 四层）
- BFF **禁止**直接操作 DB/Redis/Kafka
- Service **禁止**直连前端
- Service 间同步调用必须走 SPI（Feign）
- **禁止**跨 Service 直连数据库

### 6.2 工程结构（constraints/project-structure.md）

- 业务规则 → Domain；协调谁调谁 → Application；存取数据 → Repository
- Repository 方法名仅 `findByXxx / save / update / updateStatus`
- 对象类型：Domain 仅 DO（充血），Interfaces 仅 DTO（无 DO/PO）

### 6.3 代码风格（constraints/code-style.md）

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

### 6.4 接口规范（constraints/api.md）

- URL 小写连字符，名词单数，公开 `/public/` 前缀
- 分页 POST + RequestBody，统一 `PageRequest<T>` 包装
- 状态变更 PUT，幂等写入用 PUT
- 返回值：BFF 用 `ApiResult<T>`；Service 需分支判断用 `Result<T>`；Controller 内部方法不包装
- **错误码 5 位分段（基于冰山模块 boss-user 探查的现状）：**
  - 认证 10000-10999
  - 用户 11000-11999（**已占用 11101-11107**：11101 用户不存在 / 11102 角色不能为空 / 11103 用户名格式无效 / 11104 密码不能为空 / 11105 用户名或密码错误 / 11106 账号已被禁用 / 11107 账号锁定）
  - 车辆 12000-12999
  - 工单 13000-13999
  - 通知 14000-14999（life-notification）
  - 异常 15000-15999（boss-abnormal）
  - 客服 16000-16999（life-cs）
  - IM 17000-17999（life-im）
  - 触点 18000-18999（life-touchpoint）
  - 验证码 19000-19999（life-captcha）
  - 2C 用户 20000-20999（life-user）
  - 工单 21000-21999（life-workticket）
  - 运营通知 22000-22999（life-ops-notification）
  - 车辆域 23000-23999（life-vehicle）
  - **Boss Common 通用码**（如系统级异常，单独定义在 `BossCommonErrorCode`）
- 删除优先逻辑删除，物理删除需在 DR 说明
- HTTP 状态码统一 200，错误经 code/message 表达
- Swagger 注解必加
- 统一返回 `ApiResult<T>`，分页用 `PagedModels<T>`

### 6.5 数据库规范（constraints/database.md）

- 库名 = 服务名；utf8mb4/utf8mb4_0900_ai_ci；Service 独占库
- 500 万行/2GB 才分库分表
- 必备字段：`id / created_by / created_date / last_updated_by / last_updated_date`
- 主键：业务 `bigint AUTO_INCREMENT` 或 `varchar(32)` 业务单号
- 按需 `deleted_flag TINYINT(1) DEFAULT 0`
- 表名/字段名小写下划线单数；禁用保留字
- `is_xxx` 用 `tinyint(1)`；金额 `decimal`；时间 `datetime`；JSON 用 `json` 类型
- 单表索引 ≤ 5；varchar 索引指定长度（一般 20）
- 组合索引区分度高的放最左；ORDER BY 字段放组合索引最后
- 超过 3 表禁 JOIN；禁外键级联；IN 集合 ≤ 1000
- `#{}` 参数化（禁 `${}`）；禁 `SELECT *`；分页先判 count

### 6.6 安全规范（constraints/security.md）

- JWT 经 Cookie `security_context` 传递
- 默认需鉴权（`@SkipAuth` 显式跳过）
- 方法级权限：`@RequiresPermissions("xxx:xxx")`
- `superManager=1` 跳权限码
- 手机号脱敏：`DesensitizeUtils.handleCellphone`
- 密码 BCrypt 单向哈希：`BCryptUtil.matches`
- 日志禁打密码/手机号/Token
- 接口响应不返密码
- **BFF 鉴权注解**（当前）：`@SkipAuth` 跳过认证；`@RequiresPermissions` 方法级权限
- BFF 必用 `TokenService.getCurUserId()` 取当前用户，**禁用** `AccessUserInfoContext`

### 6.7 测试规范（constraints/testing.md）

- Controller 测试真实 HTTP（`@SpringBootTest(webEnvironment = RANDOM_PORT) + TestRestTemplate`），MockMvc 仅框架过老时降级并注明原因
- Service 单测 JUnit + Mockito，**禁真实** DB/远程
- 集成测试用开发库 + `@Transactional + @Rollback`
- 覆盖率：整体 ≥ 60% / Service 核心 ≥ 70% / Mapper XML 自定义 SQL ≥ 60% / Controller 接口 ≥ 50%
- 断言必须校验业务字段与错误码/错误信息
- 核心落库路径必须真实 DB 验证（**禁全 mock**）
- 禁 Thread.sleep
- 静态分析 Checkstyle + SpotBugs（P0 阻断）
- 跨 Story 集成测试在 `@SpringBootTest` 启动完整上下文（仅 Mock 外部系统）

### 6.10 工程经验检查清单（🔴 编码完成自审必跑）

> 本节记录 icec-cloud-boss 项目的**工程特定**经验教训——**通用**经验在 `coding-skill.md` 第 15 行附近的"经验检查清单"章节。
> 这些是历史踩坑沉淀，违反会引发线上事故 / Code Review 反复改。

| # | 检查项 | 说明 | 历史教训 / 检测命令 |
|---|--------|------|-------------------|
| 1 | **静态扫描 `java.util.` / `java.sql.` / `java.io.` 全限定名** | import 块外不应出现 | 🔴 STORY-021-BE 暴露 `java.util.Date` 全限定名遗漏；通用规则 |
| 2 | **静态扫描 application 层 import infrastructure.persistence** | application 不应直接 import 基础设施层 | 防"跨层误引用" |
| 3 | **静态扫描 application 层 import 内的 com.casstime.cloud.life.x.y** | 通用：application 调外部 SPI/Feign 走 facade，不直接 import 外部 domain/infrastructure 包 | 防"穿透 facade 直接调外部类" |
| 4 | **新加方法分层归属自查** | 每个新方法用项目资产 §4.5 判定口诀表自查 | 🔴 STORY-021-BE 暴露"在 AppService 写纯数据访问封装" |
| 5 | **BFF AppService 调 Feign 必须经 Facade** | 不可 `@Autowired XxxClient` 直接用 | 见 §6.9 隐性约定 |
| 6 | **Service 之间 Feign Client 命名 `{Resource}Client`** | 放 `infrastructure/feign/` | 见 §6.9 |
| 7 | **pom 依赖是否被注释** | 新工程模板中 SPI 依赖常被注释 | Skill 通用经验 |
| 8 | **lombok 是否显式声明** | scope=provided 不传递 | Skill 通用经验 |
| 9 | **第三方 SDK 实际包路径** | 从 jar 中确认 | Skill 通用经验 |
| 10 | **@NotBlank 来源包** | hibernate-validator 5.x 用 `org.hibernate.validator.constraints.NotBlank` | Skill 通用经验 |
| 11 | **Result.code 类型** | 确认 Integer vs String | Skill 通用经验 |
| 12 | **字段类型与 Task 一致** | 特别注意 ID 字段 | Skill 通用经验 |
| 13 | **ApiResult 完整 import** | 不同工程的 ApiResult 包路径不同 | Skill 通用经验 |
| 14 | **新模块注册到父 pom** | 子模块必须加到父 pom modules | Skill 通用经验 |
| 15 | **BFF Controller 实现 Rest 接口** | 不要自己加 @Api/@GetMapping | Skill 通用经验 |
| 16 | **Feign 注解版本** | Spring Cloud Dalston 用 `org.springframework.cloud.netflix.feign.FeignClient` | Skill 通用经验 |
| 17 | **VO 和 DTO 分离** | bff-api 定义 VO，SPI 定义 DTO | Skill 通用经验 |
| 18 | **事务外执行** | 使用 TransactionSynchronizationManager.afterCommit() | Skill 通用经验 |

**清单使用方式：**
- 编码完成后**逐项验证**：
  ```bash
  # 检查项 1（项目特定全限定名）
  grep -rn "java\.\(util\|sql\|io\)\.\w" --include="*.java" \
    icec-cloud-*/src/main/java/ \
    | grep -v ":import " | grep -v ":package "
  # 期望输出为空
  ```
- 检查项 1-6 是项目特定，1-3 在 §6.11 "工程特定静态扫描"有专门命令
- 检查项 7-18 是 Java/Spring 通用经验，参见 `coding-skill.md` 经验检查清单

### 6.11 工程特定静态扫描清单（🔴 编码完成必跑）

> 通用扫描（任何 Java 项目适用）在 `coding-skill.md` 闸 7。
> **本节是项目特定扫描**——`com.casstime.cloud.life.x.y.` 等包路径检查只对本项目适用。

```bash
# === 项目特定扫描（🔴 每轮编码完成后必跑） ===

# 1. 项目包路径全限定名扫描（除 import 块外不应出现）
#    业务代码不应写 com.casstime.cloud.life.x.y. 的全限定名
grep -rn "com\.casstime\.cloud\.life\.\(domain\|infrastructure\)\.\w\+\." \
  --include="*.java" icec-cloud-life-cs/{application,interfaces} icec-cloud-life-im/{application,interfaces} \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空

# 2. application 层不应直接 import infrastructure.persistence 包
#    必须经 Repository 接口（在 domain 层）
grep -rn "import com\.casstime\.cloud\.life\.cs\.infrastructure\.persistence" \
  --include="*.java" icec-cloud-life-cs-application/src/main/java/
# 期望输出为空

# 3. application 层不应直接 import 外部 SPI domain/infrastructure 包
#    必须经 Facade（在 domain 层）
grep -rn "import com\.casstime\.cloud\.\(life\|boss\)\.[a-z]\+\.\(domain\|infrastructure\)" \
  --include="*.java" \
  icec-cloud-life-cs-application/src/main/java/ \
  icec-cloud-life-cs-interfaces/src/main/java/
# 期望输出为空

# 4. SQL 关键字在 Service/AppService 层的扫描（应仅 Infrastructure 出现）
grep -rn "SELECT\|INSERT\|UPDATE\|DELETE" \
  --include="*AppService.java" --include="*Service.java" \
  icec-cloud-*/src/main/java/ \
  | grep -v "import " | grep -v "//.*\(SELECT\|INSERT\|UPDATE\|DELETE\)"
# 期望仅 Infrastructure 出现

# 5. 状态机流转条件 if/else 直接写在 Controller 的扫描（应在 AppService）
grep -rn "if.*\.getStatus()\|if.*status ==\|switch.*status" \
  --include="*Controller.java" --include="*RestImpl.java" \
  icec-cloud-*/icec-cloud-*-interfaces/
# 期望输出为空
```

**判定规则：**
- 任一扫描命中 → 视为"裸眼自审漏检" → 修复 → 重跑全部扫描
- 所有扫描通过 + 编译通过 + 测试通过 + Step 9 一致性闸通过 = 编码真正完成

**门禁：**
- 🔴 写完代码后未跑工程特定扫描 → 视为"未完成编码"
- 🔴 扫描发现的问题未修复就提交 → 按"伪造测试"同等级处置


### 6.8 技术栈范围（constraints/technology-stack.md）

- **Java 8 + Spring Boot 1.5.7 + Spring Cloud Dalston.SR4**（**注意：与 MEMORY.md 提到的 Java 17 + Spring Boot 3 不一致，以 constraints/ 为准**）
- MyBatis-Plus 3.3.2 + Lombok 1.18.16 + MapStruct 1.5.3.Final
- Swagger 2.8.0 + Feign + Hystrix
- MySQL 8.0.17 / ES 7.10.2
- 本地缓存 Caffeine + Redis
- Kafka 必须经 courier 组件
- 定时任务用 job-spring-boot-starter（禁 `@Scheduled`）
- 禁直配 logback/log4j（用 casslog）
- 基础组件：panda/casslog/cassmetrics/job-spring-boot-starter

### 6.9 隐性约定（constraints/implicit-constraints.md）

> 当前 constraints/implicit-constraints.md 为空（仅占位）。Code Plan 编写时若发现"项目内大家知道但没人写下来"的约定，应主动提议补充到该文件。

**已发现的隐性约定（本项目探查时）：**
- BFF 与 Service 共享 `icec-cloud-boss-security`（TokenService / @SkipAuth / @RequiresPermissions）
- `d--Item-icec-cloud-boss/memory/coding-guide.md`（69 天前）记录了 8 阶段流程、10 条红线，作为本项目 memory 但未下沉到 constraints/
- BFF 层 Controller **必须**经 Facade 调 Feign（**不允许** BFF AppService 直接 `@Autowired` Feign Client）
- BFF Feign Client **必须** extends SPI 接口（**不允许** 写新的 `@FeignClient` 不接 SPI）
- Service 之间 Feign Client（如 boss-user 调 icec-cloud-life-cs）走 `infrastructure/feign/`，命名 `{Resource}Client`（如 `CsUserClient`）
- 操作日志能力放在 `bff/infrastructure/operationlog/capability/`，命名 `{Resource}OperationLoggable`（如 `RoleMenuOperationLoggable`）

---

## 7. 跨服务契约入口

### 7.1 关键事实表

| 字段 | 本项目值 | 抽取命令 |
|------|---------|---------|
| Feign 服务名 | 详见下方 **11 个 SPI 服务名清单** | `grep -rn "ServiceProviderConstants" icec-cloud-life-spi/ --include="*.java"` |
| Nacos 实例名 | 同上（与 `ServiceProviderConstants` 一一对应） | `find . -name "bootstrap*.yml" -not -path "*/target/*" \| xargs grep "spring.application.name"` |
| 错误码分段 | 详见 §6.4（含 11101-11107 真实占用） | `find . -path "*/enums/error/*ErrorCode.java" -not -path "*/target/*"` |
| Service 间依赖 | boss-user → cs-spi, notification-spi | `mvn dependency:tree -pl icec-cloud-boss-user -Dincludes="com.casstime.cloud:icec-cloud-*-spi"` |

**11 个 SPI 服务名清单（2026-06-05 探查全量）：**

| SPI 子模块 | ServiceProviderConstants 字段 | Nacos 服务名 | 业务域 |
|------------|----------------------------|------------|-------|
| `icec-cloud-boss-abnormal-spi` | `ABNORMAL_ABNORMAL_SERVICE` | `boss-abnormal-service` | 异常处理（boss 侧） |
| `icec-cloud-boss-user-spi` | `BOSS_USER_SERVICE` | `boss-user-service` | 用户域（boss 侧） |
| `icec-cloud-life-captcha-spi` | `CAPTCHA_SERVICE` | `life-captcha-service` | 验证码 |
| `icec-cloud-life-cs-spi` | `LIFE_CS_SERVICE` | `life-cs-service` | 客服域 |
| `icec-cloud-life-im-spi` | `LIFE_IM_SERVICE` | `life-im-service` | IM 域 |
| `icec-cloud-life-notification-spi` | `LIFE_NOTIFICATION_SERVICE` | `life-notification-service` | 通用通知 |
| `icec-cloud-life-ops-notification-spi` | `OPS_NOTIFICATION_SERVICE` | `life-ops-notification-service` | 运营通知 |
| `icec-cloud-life-touchpoint-spi` | `TOUCHPOINT_SERVICE` | `life-touchpoint-service` | 触点 |
| `icec-cloud-life-user-spi` | `LIFE_USER_SERVICE` | `life-user-service` | 用户域（2C 侧） |
| `icec-cloud-life-vehicle-spi` | `VEHICLE_SERVICE` | `life-vehicle-service` | 车辆域 |
| `icec-cloud-life-workticket-spi` | `LIFE_WORK_TICKET_SERVICE` | `life-workticket-service` | 工单域 |

> **未列入清单的工程（缺 SPI 子模块）：** `icec-cloud-boss-security` / `icec-cloud-boss-webagent` / `icec-cloud-boss-auth-bff` / `icec-cloud-boss-agent-workbench-bff` — 这些是工具/网关/BFF 类型，不通过 SPI 暴露接口。

### 7.2 契约抽取命令清单

```bash
# Feign 服务名常量（过滤 target/，避免编译产物干扰）
grep -rn "ServiceProviderConstants" icec-cloud-life-spi/ --include="*.java" | grep -v "/target/" | head -30

# Nacos 应用名（过滤 target/）
find . -name "bootstrap*.yml" -not -path "*/target/*" | xargs grep -l "spring.application.name" 2>/dev/null

# 错误码分布（过滤 target/）
find . -path "*/enums/error/*ErrorCode.java" -not -path "*/target/*" | head -20

# 跨服务 Feign 客户端
grep -rn "@FeignClient" --include="*.java" --include="*.java" | grep -v "/target/" | head -30

# 端口（过滤 target/）
find . -name "bootstrap*.yml" -not -path "*/target/*" -exec grep -H "server.port\|contextPath\|context-path" {} \;
```

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
| §11 约束合规自审 | §6 全章 | 9 类约束 |
| §12 接口契约一致性 | §6.4 | URL/方法/分页/错误码 |

---

## 9. 探查 SOP（如何"研究并构建"一份新的项目资产）

> 完整 9 步见 [`../../strategies/project-assets-schema.md` §9](../../strategies/project-assets-schema.md)。本项目已应用此 SOP 完成首版。

---

## 10. 项目资产缺口与待补充

| # | 缺口 | 优先级 | 状态 | 计划补齐时间 |
|---|------|-------|------|------------|
| 1 | `d:/Item/document/life-team-project-docs/knowledge/` 完整文档未读 | 🟠 P1 | 待补 | 2026-07 |
| 2 | boss-abnormal/webagent/life-cs/im/vehicle/user/workticket/notification/ops-notification 的 server.port 未读 | 🟡 P2 | ✅ 已补 2026-06-05（11 个端口已抽，仅 webagent 仍缺）| - |
| 3 | icec-cloud-life-api / icec-cloud-boss-api 聚合工程的 server.port 未读 | 🟡 P2 | ✅ 已补 2026-06-05（life-api 6100 / boss-api 6101）| - |
| 4 | 错误码分段未统一 | 🟠 P1 | 部分已补 2026-06-05（11101-11107 真实数据已记录到 §6.4；其他 9 个 SPI 域错误码段位仅列出范围，具体值未探查）| 2026-07 |
| 5 | implicit-constraints.md 为空 | 🟠 P1 | 已记 §6.9 | 持续补 |
| 6 | icec-cloud-life-cs-spi 等其它 SPI 子模块的 ServiceProviderConstants 未全抽 | 🟡 P2 | ✅ 已补 2026-06-05（11 个 SPI 全量清单已记 §7.1）| - |
| 7 | ops/ 目录（部署/健康检查/迁移）未读 | 🟢 P3 | 待补 | 2026-Q4 |
| 8 | 其他 Service 工程的 DDD 内部分层是否与 icec-cloud-boss-user 完全一致未逐一验证 | 🟡 P2 | 待补（life-cs/life-im/life-vehicle 等仅探查到模块名，DDD 子模块是否四层完整未逐一验证）| 2026-08 |
| **9** | **🔴 新增**：icec-cloud-boss-webagent 的 server.port 仍未读到（仅有 application.yml 无 bootstrap.yml）| 🟡 P2 | 待补 | 2026-07 |
| **10** | **🔴 新增**：life-cs / life-im / life-vehicle / life-workticket / life-notification 等域的 5 位错误码段位仅占位，具体使用码值未探查 | 🟡 P2 | 待补 | 2026-07 |
| **11** | **✅ 已补 2026-06-24**：§A-§G 索引层此前完全缺失（G-00 靠子串匹配侥幸通过），本次补齐（§B 9 行 / §C 11+7 行 / §D 12 行 / §E 11 行 / §F 25 词 / §G 脚本化），并由 `ae-sdd assets query` 脚本提供倒排索引+BM25 查询 | 🟢 P3 | ✅ 已补 | - |
| **12** | **🆕 2026-06-26**：按 schema v3 §15 拆分首批工程级子文件 — boss-user（多模块 DDD Service）/ boss-user-bff（BFF 单模块）/ life-cs（客服域状态机）；其余 39 个工程按范式分批迁移 | 🟢 P3 | ✅ 已补（3 个）| 持续 |

---

## 12. 横切专题文件索引（🆕 2026-06-26）

> 🆕 schema v3 §12 新增节。单 Story 跨 ≥3 工程时，业务场景应入 `function/`，环境配置入 `config/`，业务域概览入 `domain/`——而非塞进主体。
>
> **当前状态**：三类专题目录尚未建立。下次 STORY-002/003/007/009/010/011/020 完成时同步产出。

### 12.1 function/ 业务场景专题索引

| 文件 | 涉及工程 | 摘要 | 最后更新 |
|------|---------|------|---------|
| _（暂无）_ | — | — | — |

### 12.2 config/ 环境配置专题索引

| 文件 | 适用范围 | 摘要 | 最后更新 |
|------|---------|------|---------|
| _（暂无）_ | — | — | — |

### 12.3 domain/ 业务域概览专题索引

| 文件 | 业务域 | 摘要 | 最后更新 |
|------|--------|------|---------|
| _（暂无）_ | — | — | — |

---

## 15. 工程级粒度拆分记录（🆕 2026-06-26）

> 🆕 schema v3 §15 新增节。主体文件 > 30KB 时按规则拆工程级子文件，本节列出本项目所有子文件。

| 工程名 | 子文件路径 | 大小 | 最后更新 | 状态 |
|--------|----------|------|---------|------|
| `icec-cloud-boss-user` | [`icec-cloud-boss.icec-cloud-boss-user.assets.md`](icec-cloud-boss.icec-cloud-boss-user.assets.md) | ~50KB | 2026-06-26 | ✅ 已生成（首个多模块 DDD Service 范本）|
| `icec-cloud-boss-user-bff` | [`icec-cloud-boss.icec-cloud-boss-user-bff.assets.md`](icec-cloud-boss.icec-cloud-boss-user-bff.assets.md) | ~50KB | 2026-06-26 | ✅ 已生成（首个 BFF 单模块范本）|
| `icec-cloud-boss-auth-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-notification-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-agent-workbench-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-configuration-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-log-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-log` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-vehicle-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-workticket-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-security` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-abnormal` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-webagent` | — | — | — | ⏳ 待拆 |
| `icec-cloud-boss-operation-log-starter` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-cs` | [`icec-cloud-life.icec-cloud-life-cs.assets.md`](../icec-cloud-life/icec-cloud-life.icec-cloud-life-cs.assets.md) | ~50KB | 2026-06-26 | ✅ 已生成（首个客服域状态机范本）|
| `icec-cloud-life-im` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-im-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-user` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-vehicle` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-vehicle-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-notification` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-ops-notification` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-workticket` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-content-feed` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-content-feed-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-touchpoint` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-captcha` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-configuration` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-obs` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-event-integration` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-user-journey-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-auth-bff` | — | — | — | ⏳ 待拆 |
| `icec-cloud-life-workticket-bff` | — | — | — | ⏳ 待拆 |
| `boss-common` | — | — | — | ⏳ 待拆 |
| `boss-passport` | — | — | — | ⏳ 待拆 |
| `life-passport` | — | — | — | ⏳ 待拆 |

**拆分规则：** 主体 ≤ 30KB；每个工程一份子文件；主体 §2 微服务清单每行加 `[详见]({子文件路径})` 链接。

**已生成 3 个范本对比：**

| 工程 | 类型 | 工程数 | 重点展示 |
|------|------|--------|---------|
| `boss-user` | 多模块 DDD Service（5 模块 pom）| 1 | DDD 4 层精确包路径 + 充血模型 + Converter 方法级 11 个 |
| `boss-user-bff` | 🆕 单模块 BFF（jar）| 1 | Facade 模式 + OperationLog capability + GlobalExceptionHandler 8 handler + 34 AppService 方法 |
| `life-cs` | 多模块 DDD Service（5 模块 pom）| 1 | 🔴 Spring StateMachine + Redisson 分布式锁 + @Async + @Retryable + xxl-job |

---

## §A 资产大纲（Outline）

> **调用：** `ae-sdd assets outline --project icec-cloud-boss` — 拉取本总览
> **用途：** 5 秒了解项目范围 + 一级目录速查

### A.1 项目速览

| 维度 | 值 |
|------|---|
| 微服务数 | 22（含 6 聚合工程 + 1 公共库 + 1 安全组件 + 14 业务 Service/BFF） |
| 主表数 | 9（boss_user / boss_role / boss_menu / boss_menu_icon / boss_user_role / boss_role_menu / boss_role_closure / boss_user_extension / boss_user_api_perms） |
| 公共组件数 | 12（ApiResult / PagedModels / PageRequest / TokenService / @SkipAuth / @RequiresPermissions / JsonUtils / DesensitizeUtils / BCryptUtil / MybatisPlusConfig / BaseMapper / KafkaDomainEventPublisher） |
| 跨服务 API 数 | 11 个 SPI 子模块 |
| 业务域 | boss / life |
| 上次审计 | 2026-06-24 |
| 索引关键词数 | 25（见 §F） |

### A.2 一级目录速查

| 章节 | 标题 | 一句话说明 |
|------|------|-----------|
| §0 | 摘要与使用场景 | 何时查 / 谁负责 / 与 constraints 关系 |
| §1 | 项目资产元信息 | projectKey / gitPath / 端口段 |
| §2 | 微服务清单 | 22 个微服务的职责/端口/SPI 依赖 |
| §3 | 抽象分层映射 | 4 层 DDD → 本项目工程模块 |
| §4 | DDD 内部分层落点 | 类角色 → 精确包路径 |
| §5 | 命名约定 | 13 类命名模板 + 反例 |
| §6 | 工程约束 | 8 类 constraints 映射 |
| §7 | 跨服务契约入口 | 11 个 SPI 服务名清单 |
| §8 | Code Plan 输入索引 | CodePlan 章节 → 资产章节引用 |
| §9 | 探查 SOP | 9 步研究流程 |
| §10 | 项目资产缺口 | 待补充项 |
| §A | 资产大纲（本节）| 总览 |
| §B | 模块索引 | 9 行微服务索引 |
| §C | 字段索引 | 主表关键字段索引 |
| §D | 组件索引 | 12 行公共组件索引 |
| §E | API 索引 | 11 行跨服务契约索引 |
| §F | 关键词反向索引 | 25 行关键词定位 |
| §G | 资产读取 API | 调用协议（已由 ae-sdd assets 脚本实现） |

---

## §B 模块索引（Module Index）

> **调用：** `ae-sdd assets query "<name>" --project icec-cloud-boss` — 关键词反查定位

| module | 概述 | 基础包 | 类型 | 入口 Controller | 关键 AppService | 文档 |
|--------|------|--------|------|----------------|----------------|------|
| `icec-cloud-boss-user` | Boss 用户域（用户/角色/菜单/权限） | `com.casstime.cloud.boss.user` | service | `BossUserServiceImpl` / `BossMenuServiceImpl` / `BossRoleServiceImpl` | `BossUserAppService` / `BossMenuAppService` / `BossRoleAppService` | §4 |
| `icec-cloud-boss-user-bff` | Boss 用户 BFF 聚合层 | `com.casstime.cloud.boss.bff.user` | bff | `BossUserManagementRestImpl` / `BossMenuRestImpl` | `BossUserManagementAppService` / `BossMenuAppService` | [工程级子文件](icec-cloud-boss.icec-cloud-boss-user-bff.assets.md) |
| `icec-cloud-boss-auth-bff` | 登录鉴权 BFF（Token/Cookie） | `com.casstime.cloud.boss.bff.auth` | bff | `BossAuthRestImpl` | `BossAuthAppService` | §2 |
| `icec-cloud-boss-security` | 安全公共组件（TokenService/@SkipAuth） | `com.casstime.cloud.boss.security` | lib | — | — | §6.6 |
| `icec-cloud-boss-abnormal` | Boss 异常处理域（双启动模块） | `com.casstime.cloud.boss.abnormal` | service | `BossAbnormalServiceImpl` | `BossAbnormalAppService` | §2 |
| `icec-cloud-life-cs` | 客服域 Service（工单/会话/状态机） | `com.casstime.cloud.life.cs` | service | `CsTicketServiceImpl` | `CsTicketAppService` | §2 |
| `icec-cloud-life-im` | IM 域 Service（会话/消息/融云） | `com.casstime.cloud.life.im` | service | `ImSessionServiceImpl` | `ImSessionAppService` | §2 |
| `icec-cloud-life-spi` | SPI 聚合父工程（11 子模块） | `com.casstime.cloud.life.spi` | spi | — | — | §7.1 |
| `icec-cloud-boss-api` | Boss API 聚合工程（8 BFF-api） | `com.casstime.cloud.boss.api` | api | — | — | §7 |

> 其余 13 个微服务（vehicle/workticket/notification/ops-notification/webagent/content-feed/touchpoint/captcha/configuration/obs/event-integration/user-journey-bff/agent-workbench-bff/notification-bff）见 §2 微服务清单。

---

## §C 字段索引（Table/Field Index）

> **调用：** `ae-sdd assets query "<table>" --project icec-cloud-boss` — 表名/字段反查

### C.1 主表关键字段（boss_user 域，基于冰山模块探查）

| 表名 | 字段 | 类型 | 业务含义 | 关联模块 |
|------|------|------|---------|---------|
| `boss_user` | `id` | bigint | 主键 | boss-user |
| `boss_user` | `user_name` | varchar(64) | 用户名 | boss-user |
| `boss_user` | `password` | varchar(128) | BCrypt 哈希密码 | boss-user |
| `boss_user` | `cellphone` | varchar(20) | 手机号（脱敏） | boss-user |
| `boss_user` | `status` | tinyint(1) | 0=禁用 1=启用 | boss-user |
| `boss_user` | `created_by` / `created_date` / `last_updated_by` / `last_updated_date` | — | 审计四字段 | common |
| `boss_role` | `id` / `role_name` / `status` | — | 角色表 | boss-user |
| `boss_menu` | `id` / `menu_name` / `parent_id` / `sort` | — | 菜单表（树形） | boss-user |
| `boss_user_role` | `user_id` / `role_id` | bigint | 用户-角色关系 | boss-user |
| `boss_role_menu` | `role_id` / `menu_id` | bigint | 角色-菜单关系 | boss-user |
| `boss_role_closure` | `ancestor` / `descendant` / `depth` | bigint | 角色闭包表（树形权限） | boss-user |

### C.2 错误码占用值（boss-user 域，真实探查）

| 错误码 | 含义 | 来源类 |
|--------|------|--------|
| 11101 | 用户不存在 | `BossUserErrorCode` |
| 11102 | 角色不能为空 | `BossUserErrorCode` |
| 11103 | 用户名格式无效 | `BossUserErrorCode` |
| 11104 | 密码不能为空 | `BossUserErrorCode` |
| 11105 | 用户名或密码错误 | `BossUserErrorCode` |
| 11106 | 账号已被禁用 | `BossUserErrorCode` |
| 11107 | 账号锁定 | `BossUserErrorCode` |

> 其余 9 个 SPI 域错误码段位见 §6.4（仅范围，具体值待补，见 §10 缺口 10）。

---

## §D 组件索引（Component Index）

> **调用：** `ae-sdd assets query "<component>" --project icec-cloud-boss` — 组件复用查询

| 组件名 | 功能 | 路径 | 调用方 |
|--------|------|------|--------|
| `TokenService` | JWT Token 创建/解析/刷新/删除；`getCurUserId()` 取当前用户 | `icec-cloud-boss-security/.../service/TokenService.java` | 所有 BFF（禁用 AccessUserInfoContext） |
| `@SkipAuth` | 跳过认证注解 | `icec-cloud-boss-security/.../annotation/SkipAuth.java` | 公开接口 Controller（如 /public/auth/login） |
| `@RequiresPermissions` | 方法级权限校验 | `icec-cloud-boss-security/.../annotation/RequiresPermissions.java` | 所有需鉴权 BFF Controller |
| `@NeedLogin` | 强制登录注解 | `icec-cloud-boss-security/.../annotation/NeedLogin.java` | BFF Controller |
| `ApiResult<T>` | 统一返回包装 | `boss-common/.../result/ApiResult.java` | 所有 Controller |
| `PagedModels<T>` | 分页返回 | `boss-common/.../result/PagedModels.java` | 所有分页接口 |
| `PageRequest<T>` | 分页请求包装 | `boss-common/.../request/PageRequest.java` | 所有分页接口 |
| `JsonUtils` | JSON 序列化 | `com.casstime.commons.utils.JsonUtils` | 所有 Service |
| `DesensitizeUtils.handleCellphone` | 手机号脱敏 | `com.casstime.commons.utils.DesensitizeUtils` | 所有展示层 |
| `BCryptUtil.matches` | 密码 BCrypt 校验 | `com.casstime.commons.utils.BCryptUtil` | 登录服务 |
| `MybatisPlusConfig` | MyBatis-Plus 配置 | `icec-cloud-boss-user-infrastructure/.../config/MybatisPlusConfig.java` | 所有 Service |
| `KafkaDomainEventPublisher` | 领域事件发布（MQ） | `icec-cloud-boss-user-infrastructure/.../messaing/publisher/KafkaDomainEventPublisher.java` | boss-user 事件 |

---

## §E API 索引（API Index）

> **调用：** `ae-sdd assets query "<method>" --project icec-cloud-boss` — 跨服务契约查询

| Feign/SPI | 服务 | 方法 | 入参 | 出参 |
|----------|------|------|------|------|
| `BossUserService` | `boss-user-service` | `getUserById` | `Long userId` | `ApiResult<BossUserDTO>` |
| `BossUserManagementService` | `boss-user-service` | 用户管理（增删改查） | `BossUserManagementRequest` | `ApiResult<BossUserManagementDTO>` |
| `BossUserInfoService` | `boss-user-service` | 用户扩展信息 | — | `ApiResult<BossUserDTO>` |
| `CsTicketService` | `life-cs-service` | 工单查询/操作 | — | `ApiResult<CsTicketDTO>` |
| `ImMessageService` | `life-im-service` | 消息发送/查询 | `ImMessageRequest` | `ApiResult<ImMessageDTO>` |
| `ImSessionService` | `life-im-service` | 会话管理 | — | `ApiResult<ImSessionDTO>` |
| `NotificationService` | `life-notification-service` | 通用通知发送 | — | `ApiResult<...>` |
| `OpsNotificationService` | `life-ops-notification-service` | 运营通知 | — | `ApiResult<...>` |
| `VehicleService` | `life-vehicle-service` | 车辆域 | — | `ApiResult<...>` |
| `CaptchaService` | `life-captcha-service` | 验证码 | — | `ApiResult<...>` |
| `TouchpointService` | `life-touchpoint-service` | 触点/行为 | — | `ApiResult<...>` |

> SPI 接口包路径见 §4 "SPI（被消费方）"节；服务名常量见 §7.1 ServiceProviderConstants。

---

## §F 关键词反向索引（Reverse Index）

> **调用：** `ae-sdd assets query "<keyword>" --project icec-cloud-boss` — 关键词跨章节定位
> 位置精确到 §X.Y（脚本运行时可进一步精确到行号：`ae-sdd assets query` 返回 line 字段）

| 关键词 | 出现位置 |
|--------|---------|
| `AppService` | §4 / §5 / §6.3 / §B |
| `@Transactional` | §4 / §6.3 / §4.5.2 |
| `Facade` | §4 / §6.9 / §4.5.3 |
| `FeignClient` | §4 / §6.9 / §7 |
| `Converter` | §4 / §5 |
| `BossUser` | §2 / §4 / §5 / §6.4 / §7 / §C |
| `ServiceProviderConstants` | §4 / §7.1 / §7.2 |
| `security_context` | §6.6 / §6.9 |
| `@SkipAuth` | §4 / §6.6 / §6.9 / §D |
| `@RequiresPermissions` | §4 / §6.6 / §D |
| `ApiResult` | §4 / §6.3 / §6.4 / §D |
| `PagedModels` | §4 / §6.4 / §D |
| `PageRequest` | §4 / §6.4 / §D |
| `BCrypt` | §6.6 / §D |
| `TokenService` | §4 / §6.6 / §6.9 / §D |
| `deleted_flag` | §6.5 |
| `cellphone` | §6.6 / §C |
| `错误码` | §6.4 / §C.2 / §10 |
| `闭包表` | §C / §4（BossRoleClosureDO） |
| `双启动模块` | §2 / §6.9（boss-abnormal / life-user / life-notification） |
| `操作日志` | §4 / §6.9（{Resource}OperationLoggable） |
| `LocalDateTime` | §5 / §6.3（禁用，用 java.util.Date） |
| `job-spring-boot-starter` | §6.8（禁 @Scheduled） |
| `casslog` | §6.8（禁直配 logback） |
| `AccessUserInfoContext` | §6.6（BFF 禁用） |

---

## §G 资产读取 API（🆕 已由 ae-sdd assets 脚本实现）

> **本节原为"自然语言协议"（调用 SKILL 通过 Read/Grep 组合读取），2026-06-24 起由
> `ae-sdd assets` 子命令组真正实现（倒排索引 + 分词 + BM25 评分）。**

### G.1 场景化 API（推荐调用）

| API | CLI 命令 | 适用阶段 | 返回 |
|-----|---------|---------|------|
| `forRequirementAnalysis` | `ae-sdd assets outline` + `assets query "<module>"` | 需求分析 | §A 大纲 + §B 模块 + §C 字段 |
| `forDrGenerate` | `ae-sdd assets section 3` + `assets section 5` + `assets section 7` | DR 设计 | §3 分层 + §5 命名 + §7 契约 |
| `forStoryGenerate` | `ae-sdd assets section 4` + `assets section 5` | Story 拆解 | §4 包路径 + §5 命名 |
| `forCoding` | `ae-sdd assets query "<类名>"` + `assets section 6` | 编码 | §4 落点 + §5 命名 + §6 约束 |
| `forCodeReview` | `ae-sdd assets section 6` + `assets query "<field>"` | CodeReview | §6 约束 + §C 字段 + §D 组件 |

### G.2 底层 API（精准查询）

| API | CLI 命令 | 说明 |
|-----|---------|------|
| `outline()` | `ae-sdd assets outline --project <key>` | §A 大纲 + 索引统计 |
| `module(name)` | `ae-sdd assets query "<name>"` | 关键词 → BM25 top-N 命中 |
| `table(name)` | `ae-sdd assets query "<table>"` | 表名/字段反查 |
| `sections(section)` | `ae-sdd assets section <name>` | 取整章原文 |
| `search(keyword)` | `ae-sdd assets query "<keyword>"` | 倒排索引查询（核心） |
| `stats()` | `ae-sdd assets stats --project <key>` | 索引统计 + 缓存状态 |

### G.3 调用示例

```bash
# 查 "AppService" 在项目里所有出现位置（BM25 排序）
ae-sdd assets query "AppService" --project icec-cloud-boss --top 10

# 取 §4 DDD 落点整章
ae-sdd assets section 4 --project icec-cloud-boss

# JSON 输出（pipeline 友好）
ae-sdd assets query "融云" --project icec-cloud-boss --json

# 直接指定资产文件（绕过 .ae-sdd 定位）
ae-sdd assets query "TokenService" --asset-file path/to/assets.md
```

---

## 附录 A：本项目 JSON 实例（机器可读）

```json
{
  "meta": {
    "projectKey": "icec-cloud-boss",
    "projectName": "icec-cloud-boss",
    "gitPath": "d:\\Item\\icec-cloud-boss",
    "productLine": ["boss", "life"],
    "profile": ["dev", "test", "prod", "beta-kunlun"],
    "packaging": "jar",
    "portRange": "12002-12004 (boss) / 10087-10097 (life BFF)",
    "lastAuditedAt": "2026-06-24",
    "owner": "架构组"
  },
  "microservices": [
    { "name": "icec-cloud-boss-user", "responsibility": "Boss 用户域", "port": 12004, "hasBff": true, "callChain": "service + Feign cs-spi", "dependsOnSpi": ["icec-cloud-life-cs-spi"] },
    { "name": "icec-cloud-boss-user-bff", "responsibility": "Boss 用户 BFF", "port": 12003, "hasBff": true, "callChain": "BFF → SPI → Service", "dependsOnSpi": ["icec-cloud-boss-user-spi"] }
  ],
  "openGaps": [
    "boss-abnormal/life-cs 等 9 个 Service 端口未读",
    "icec-cloud-life-api/boss-api 聚合工程端口未读",
    "implicit-constraints.md 为空"
  ]
}
```

---

## 维护

- **维护人：** 架构组 + 各域负责人（boss-user：{owner1} / life-cs：{owner2} / life-im：{owner3} / ...）
- **更新频率：** 每月审计一次；新增微服务/分层调整时立即更新
- **同步对象：** ① 本项目所有 Story 编写者（强制引用本文件）② 跨项目模板对齐 `ae-sdd-update-skill.md` 边界判定
- **双源一致性审计：** 每月跑对照脚本检查 `§6 工程约束` 是否引用了 `constraints/` 所有 8 个文件名
- **探查历史：** 2026-06-04 Workflow 一轮 Explore Agent 首版探查
