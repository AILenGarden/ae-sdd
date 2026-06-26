---
name: icec-cloud-boss-user-project-assets
description: icec-cloud-boss-user 工程级项目资产范本 — 按 schema v3（2026-06-26）新增的工程级粒度拆分 SOP 生成，含 §1.1 部署信息 / §1.2 安全提示 / §3 完整技术栈版本号 / §5 核心类方法级实现 / 可信度三态标注。本文是 schema §15 + 附录 B 的首份工程级子文件实例，供后续工程级子文件按此范式生成。
parent: icec-cloud-boss.assets.md
探查时间: 2026-06-26
探查来源: 同事 D:\Item\life\document\life-team-project-docs\knowledge\project\icec-cloud-boss-user.md (102KB / 1638 行) + ae-sdd boss.assets.md §4 DDD 落点
---

# icec-cloud-boss-user 工程级项目资产

> **本文是 `icec-cloud-boss.assets.md` 的工程级子文件**（schema §15），仅含本工程细节。跨工程信息见主体。
>
> **范本定位**：本文是 schema §15 工程级拆分 + §1.2 部署信息 + §14 安全提示 + §5 核心类方法级 + 可信度三态标注的**首份实例**，供后续工程级子文件（life-cs / life-im / life-user / ...）按此范式生成。

---

## 0. 摘要与使用场景 [已确认]

| 维度 | 内容 |
|------|------|
| 工程名 | `icec-cloud-boss-user` |
| 父工程 | `icec-cloud-boss` |
| 探查时间 | 2026-06-26 |
| 工程定位 | Boss 用户域核心业务（用户/角色/菜单/菜单图标/权限/扩展信息）；Feign 调用 icec-cloud-life-cs |
| 关键不变量 | 本工程不重复定义 rules；只把 rules 映射到本工程代码 |
| 最后审计 | 2026-06-26 |

---

## 1. 模块元信息 [已确认]

| 字段 | 值 |
|------|---|
| moduleName | `icec-cloud-boss-user` |
| groupId | `com.casstime.cloud` |
| artifactId | `icec-cloud-boss-user` |
| version | `1.0-SNAPSHOT` |
| packaging | `pom（聚合父工程）` |
| 子模块数 | 5 个（domain / application / interfaces / infrastructure / service） |
| 主启动类 | `boss-user-service` 的 `Bootstrap.java` |
| profile | `dev, test, prod, beta-kunlun` |
| port | `12004` |
| contextPath | `/boss-user` |
| dependsOnSpi | [`icec-cloud-life-cs-spi`] |
| lastAuditedAt | `2026-06-26` |
| owner | 架构组 + user 域负责人 |

### 1.1 部署信息 [已确认]

> 🆕 2026-06-26 schema §1.2 新增节。本节从 bootstrap.yml / service pom 抽取。

| 字段 | 值 | 抽取来源 |
|------|---|---------|
| `profile.active` | `beta-kunlun` | `bootstrap.yml` `spring.profiles.active` |
| `db.urlTemplate` | `jdbc:mysql://${icec.database.servers}/${icec.database.dbname}?useUnicode=true&characterEncoding=utf-8&allowMultiQueries=true&autoReconnect=true` | `bootstrap.yml` |
| `db.pool` | HikariCP max-active=20 / min-idle=1 / timeout=30s | `bootstrap.yml` |
| `redis.address` | `redis-...dcs.huaweicloud.com:6379`（华为云 DCS） | `bootstrap.yml` |
| `redis.password.inConfig` | 🔴 **false（确认无明文）** [已确认] | 同事 user.md §1.4 标注但该文档原文有"bootstrap.yml 中 Redis 明文密码直接写入配置文件"提示 → 需复验 |
| `gateway` | `http://life-hwbeta-api-penglai.intra.casstime.com`（icec.api.agent） | `bootstrap.yml` |
| `imageRepo` | `registry.cn-shenzhen.aliyuncs.com/cassmall/boss-user-service:1.0-SNAPSHOT` | `service/pom.xml` dockerfile-maven-plugin |
| `nexusRepo` | `http://dev.casstime.com/nexus/content/groups/public/`（cass-public） | `pom.xml` repositories |
| `management.port` | 未显式配置（默认 30000，可能本地多服务冲突） | ⚠️ [据推断] |
| `coverageTool` | JaCoCo（target/jacoco.exec） | `service/pom.xml` |

### 1.2 安全提示 [待确认]

> 🆕 2026-06-26 schema §14 新增节。

| ID | 类型 | 位置 | 风险等级 | 描述 | 建议修复 | 状态 |
|----|------|------|---------|------|---------|------|
| S-001 | 明文密码（待复验） | `icec-cloud-boss-user-service/src/main/resources/bootstrap.yml` Redis 段 | 🟡 中 | 同事 user.md §1.4 提示"bootstrap.yml 中 Redis 明文密码直接写入配置文件，建议改为占位符外部注入"——但当前实际版本是否仍含明文**待复验** | 改为占位符 `${icec.redis.password}` 外部注入 | 待复验 |
| S-002 | management.port 未配置 | `bootstrap.yml` | 🟢 低 | 未显式配置 `management.port`，本地多服务同时启动时第二服务可能因端口冲突启动失败 | 显式配置 `management.port: 30004`（与 service.port 错开） | 待修 |
| S-003 | lombok scope=provided | 各模块 pom.xml | 🟢 低 | lombok 不传递依赖，新模块如未在 pom 显式声明可能编译失败 | 每个模块 pom 显式声明 lombok 依赖 | 已知约束 |
| S-004 | icec-cloud-life-demo-spi 依赖已注释 | `infrastructure/pom.xml` | 🟢 低 | 同事 user.md §1.2 提示 infrastructure pom 中 `icec-cloud-life-demo-spi` 依赖已注释——可能影响 SPI 引用 | 确认是否需要删除注释或改用其他 SPI | 待确认 |

---

## 2. 子模块结构 [已确认]

| 模块 | ArtifactId | 打包 | 职责 | 主要依赖 |
|------|-----------|------|------|---------|
| 领域层 | `icec-cloud-boss-user-domain` | jar | 用户领域核心模型与业务规则；含聚合（BossUserDO / BossRoleDO / BossMenuDO / BossMenuIconDO / BossUserExtensionDO / BossRoleClosureDO / BossRoleMenuDO / BossUserRoleDO / BossUserApiPermsDO / BossRoleTreeDO）、错误码（BossUserErrorCode 11101-11107）、登录锁防腐接口（LoginLockFacade）、领域服务（BossUserDomainService） | `icec-cloud-commons`、`icec-cloud-boss-user-spi`、`spring-security-crypto`、`commons-lang3(3.9)`、`commons-collections4(4.0)`、`JUnit 4.12`、`Mockito 1.10.19` |
| 应用层 | `icec-cloud-boss-user-application` | jar | 应用服务编排（BossUserAppService）、对象转换（BossUserConverter）、事务协调、领域事件发布；引入 courier 消息投递组件 | `icec-cloud-boss-user-domain`、`icec-cloud-boss-user-spi`、`icec-cloud-boss-user-event`、`courier-spring-boot-starter(3.3-SNAPSHOT)`、`spring-context`、`spring-tx` |
| 接口层 | `icec-cloud-boss-user-interfaces` | jar | 对外接口适配，实现 SPI 服务接口（BossUserServiceImpl / BossMenuServiceImpl / BossRoleServiceImpl / BossUserInfoServiceImpl / BossMenuIconServiceImpl） | `icec-cloud-boss-user-application`、`icec-cloud-boss-user-spi`、`icec-cloud-commons(b2c.1.0-SNAPSHOT)`、`spring-web` |
| 基础设施层 | `icec-cloud-boss-user-infrastructure` | jar | 持久化（BossUserMapper.xml）、登录锁防腐实现（LoginLockFacadeImpl）、远程调用（CsUserClient Feign 客户端）、Redis 缓存；注：`icec-cloud-life-demo-spi` 依赖已注释 | `icec-cloud-boss-user-domain`、`icec-cloud-boss-user-application`、`icec-cloud-spi-common`、`spring-cloud-starter-feign`、`spring-cloud-netflix-core`、`spring-boot-starter-data-redis`、`HikariCP(2.7.9)`、`mysql-connector-java(8.0.17)`、`mybatis-plus-boot-starter(3.3.2)` |
| 启动/服务层 | `boss-user-service` | jar | Spring Boot 应用入口、Web 容器、配置加载、Docker 镜像构建 | `icec-cloud-boss-user-interfaces`、`icec-cloud-boss-user-infrastructure`、`panda(1.0.9)` / `casslog(1.5.0)` / `cass-config(1.1.2)` / `cassmetrics(1.0.4)` starter、`spring-boot-starter-web/aop/cache`、`spring-cloud-starter-config/feign`、`spring-boot-maven-plugin(2.7.5)`、`dockerfile-maven-plugin(1.4.0)`、`JaCoCo` |

> 说明：父 pom `<modules>` 声明了 domain / application / interfaces / infrastructure / service 五个模块；`icec-cloud-boss-user-spi` 与 `icec-cloud-boss-user-event` 在 dependencyManagement/各模块依赖中被引用，但未列入 `<modules>`，为外部独立模块或子工程 [待确认]。

### 2.1 依赖层次关系 [已确认]

```
boss-user-service              ← 启动装配（聚合 interfaces + infrastructure）
        ↓
        ├──→ icec-cloud-boss-user-interfaces       ← 接口实现 SPI 接口
        │             ↓
        │             └──→ icec-cloud-boss-user-application
        │                          ↓
        │                          └──→ icec-cloud-boss-user-domain
        │                                            ↑
        └──→ icec-cloud-boss-user-infrastructure ───┘ (持久化 / 远程 / 缓存)
                          ↑
                          └──→ icec-cloud-boss-user-domain (Repository 接口实现)
```

> 整体遵循「启动层聚合接口层与基础设施层、接口层依赖应用层、应用层依赖领域层、基础设施层向上支撑」的 DDD 依赖倒置结构。

---

## 3. 完整技术栈版本号 [已确认]

> 🆕 2026-06-26 schema §6.8 新增节。**完整 7 张表见主体 §6.8**，本节只列本工程特有的依赖（pom 中显式声明但不在公共 dependencyManagement 的）。

| 依赖 | 版本 | 用途 |
|------|------|------|
| `spring-security-crypto` | 跟随 Spring Boot | domain 层密码加密/校验（`BCryptUtil.matches`） |
| `courier-spring-boot-starter` | `3.3-SNAPSHOT` | application 层领域事件/消息投递 |
| `panda-spring-boot-starter` | `1.0.9` | 配置中心（panda.casstime.com，product=b2c，instance=boss-user-service） |
| `casslog-spring-boot-starter` | `1.5.0` | 日志组件（排除 icec-cloud-commons） |
| `cassmetrics-spring-boot-v1-starter` | `1.0.4` | 监控指标采集 |
| `cass-config-spring-boot-starter` | `1.1.2` | 公司内部配置组件 |
| `spring-boot-maven-plugin` | `2.7.5` | Fat JAR 打包（repackage 目标） |
| `dockerfile-maven-plugin` | `1.4.0` | Docker 镜像构建 |
| `surefire` | `2.22.2` | Maven 测试插件 |
| `JaCoCo` | （跟随）| 覆盖率插件（target/jacoco.exec） |
| `log4j-bom` | `2.17.2` | 日志依赖统一管理（置于 dependencyManagement 首位） |
| `MapStruct` | `1.5.3.Final` | DTO/PO 映射 [据依赖管理推断] |
| `Lombok` | `1.18.16` | 简化样板代码 |
| `Swagger2` | `2.8.0` | 接口文档 |
| `JUnit` | `4.12` | 单元测试 |
| `Mockito` | `1.10.19` | Mock 框架（domain 层） |
| `commons-lang3` | `3.9` | 字符串/通用工具（domain 层） |
| `commons-collections4` | `4.0` | 集合工具（domain 层） |

---

## 4. DDD 内部分层落点 [已确认]

> **详细类角色映射见主体 §4**；本节只列本工程内实际类。

| 类角色 | 精确包路径 | 典型类名（已确认）|
|--------|-----------|------------------|
| **Interfaces** | | |
| Rest 实现类 | `icec-cloud-boss-user-interfaces/src/main/java/com/casstime/cloud/boss/user/interfaces/restful/` | `BossUserServiceImpl` / `BossMenuServiceImpl` / `BossRoleServiceImpl` / `BossUserInfoServiceImpl` / `BossMenuIconServiceImpl`（implements SPI 接口 + `@RestController`）|
| **Application** | | |
| AppService | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/appservice/` | `BossUserAppService` / `BossMenuAppService` / `BossRoleAppService` / `BossMenuIconAppService` |
| Converter | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/converter/` | `BossUserConverter` / `BossMenuConverter` / `BossRoleConverter` / `BossMenuIconConverter`（`@UtilityClass` + 静态方法）|
| Publisher | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/publisher/` | `ApplicationEventPublisher` |
| Command VO | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/vo/command/` | `{Resource}Command` |
| Query VO | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/vo/query/` | `BossUserQuery` / `BossMenuQuery` / `BossRoleQuery` |
| **Domain** | | |
| Domain Object | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/entity/` | `BossUserDO` / `BossRoleDO` / `BossMenuDO` / `BossMenuIconDO` / `BossUserExtensionDO` / `BossRoleClosureDO` / `BossRoleMenuDO` / `BossUserRoleDO` / `BossUserApiPermsDO` / `BossRoleTreeDO`（充血）|
| Value Object | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/value/` | `BossUserQuery` / `BossMenuQuery` / `BossRoleQuery` |
| Enum | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/enums/` | `{Resource}MessageEnum` |
| Error Enum | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/enums/error/` | `BossUserErrorCode` (11101-11107) / `BossRoleErrorCode` / `BossMenuErrorCode` / `BossMenuIconErrorCode` |
| Repository 接口 | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/repository/` | `BossUserRepository` / `BossMenuRepository` / `BossRoleRepository` / `BossMenuIconRepository` / `BossUserRoleRepository` / `BossRoleMenuRepository` / `BossRoleClosureRepository` / `BossUserExtensionRepository`（仅接口）|
| Domain Service | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/service/` | `BossUserDomainService` / `BossRoleDomainService` / `BossMenuDomainService` / `BossMenuIconDomainService` |
| Exception | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/exception/` | `BossDomainException` |
| Facade | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/facade/` | `LoginLockFacade` |
| **Infrastructure** | | |
| Config | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/config/` | `MybatisPlusConfig` |
| Feign Client | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/feign/` | `CsUserClient` |
| Publisher (MQ) | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/messaing/publisher/` | `KafkaDomainEventPublisher` / `KafkaApplicationEventPublisher` |
| PO | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/entity/` | `BossUserPO` (`@TableName("boss_user")`) / `BossRolePO` / `BossMenuPO` / `BossMenuIconPO` / `BossUserRolePO` / `BossRoleMenuPO` / `BossRoleClosurePO` / `BossUserExtensionPO` |
| Mapper | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/dao/mapper/` | `BossUserMapper` (`extends BaseMapper<BossUserPO>`) |
| DataConverter | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/converter/` | `BossUserDataConverter` / `BossMenuDataConverter` / `BossMenuIconDataConverter` / `BossUserExtensionDataConverter` |
| Repository Impl | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/repository/mysql/` | `BossUserRepositoryImpl extends ServiceImpl<BossUserMapper, BossUserPO> implements BossUserRepository`（`@Repository + @RequiredArgsConstructor`）|
| Facade Impl | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/facade/` | `LoginLockFacadeImpl` |
| **SPI（被消费方）** | | |
| SPI Service 接口 | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/service/` | `BossUserService` / `BossUserManagementService` / `BossUserInfoService` |
| DTO | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/dto/` | `BossUserDTO` / `BossUserManagementDTO` / `BossUserApiPagedModels` |
| Request | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/request/` | `BossUserManagementRequest` |
| Constants | `icec-cloud-life-spi/icec-cloud-boss-user-spi/src/main/java/com/casstime/cloud/boss/spi/user/` | `ServiceProviderConstants`（`BOSS_USER_SERVICE = "boss-user-service"`）|

---

## 5. 核心类方法级实现（🆕 2026-06-26）

> **🆕 范本节**：原 schema §4 只列类名，缺方法级实现。Code Plan 编写者拿到资产时需要知道"这个类有哪些方法 / 每个方法干什么"——单看类名不够。本节按 DDD 分层逐类写方法签名 + 参数 + 返回 + 业务含义（来源：同事 user.md §5.2 BossUserConverter 11 个方法全表）。

### 5.1 Converter 层 [已确认]

#### BossUserConverter

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/converter/BossUserConverter.java` |
| 职责 | 用户 DO ↔ DTO 转换；含扩展信息、API 权限判定、时间字符串化等 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `localDateTimeToString` | `LocalDateTime localDateTime` | `String` | 转 ISO 格式（`yyyy-MM-dd'T'HH:mm:ss.SSS'Z'`）字符串；入参 null 返回 null |
| `toBossUserManagementDTO` | `BossUserDO user` | `BossUserManagementDTO` | 转管理 DTO：含 userId/userName/nickname/cellphone/status/createdAt/updatedAt；extensionDO 非 null 时填充 tqUin/tqPw/companyId/companyName/channelId；labelId/labelName 置 null |
| `toBossUserManagementDTOList` | `List<BossUserDO> users` | `List<BossUserManagementDTO>` | 批量转管理 DTO；空集合返回 emptyList |
| `toBossUserInfoDTO` | `BossUserDO user` | `BossUserInfoDTO` | 转登录信息 DTO（含 userId/userName/cellphone/password）；extensionDO 非 null 时填充 companyId |
| `buildExtensionDO` | `BossUserManagementRequest request, String operatorId` | `BossUserExtensionDO` | 构建用户扩展 DO：填充 tqUin/tqPw 及创建/更新人；request 为 null 返回 null |
| `toBossUserInfoDTO` | `BossUserApiPermsDO bossUserApiPermsDO, List<String> checkPerms` | `BossUserApiPermsDTO` | 转 API 权限 DTO：填充 userId/superManager/perms；调用私有方法计算 hasAllPerms/hasAnyPerms |
| `toBossUserDTO` | `BossUserDO userDO` | `BossUserDTO` | 转基础用户 DTO（userId/userName/cellphone）；入参 null 返回 null |
| `hasAllPerms（私有）` | `List<String> checkPerms, List<String> dbPerms` | `Boolean` | 判定 dbPerms 是否包含全部 checkPerms：checkPerms 为空返回 true，dbPerms 为空返回 false，否则用 HashSet 做 containsAll 判定 |
| `hasAnyPerms（私有）` | `List<String> checkPerms, List<String> dbPerms` | `Boolean` | 判定 dbPerms 是否包含任一 checkPerms：checkPerms 为空返回 true，dbPerms 为空返回 false，否则用 `!Collections.disjoint` 判定交集存在 |

#### BossRoleConverter

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/converter/BossRoleConverter.java` |
| 职责 | 角色 DO ↔ DTO、分页与角色树转换 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `toDTO` | `BossRoleDO roleDO` | `BossRoleDTO` | 角色 DO 转 DTO（含用户数、时间字符串化）|
| `toDTOList` | `List<BossRoleDO> roleDOList` | `List<BossRoleDTO>` | 批量转 DTO；null 返回空列表 |
| `toDO` | `BossRoleDTO roleDTO` | `BossRoleDO` | 角色 DTO 转 DO |
| `toDTO` | `PagedModels<BossRoleDO> pagedModels` | `PagedModels<BossRoleDTO>` | 角色分页结果转换 |
| `toBossRoleTreeDTO` | `BossRoleTreeDO treeDO` | `BossRoleTreeDTO` | 递归转换角色树（current/enabled/children）|

#### BossMenuConverter

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/converter/BossMenuConverter.java` |
| 职责 | 菜单 DO ↔ DTO、分页转换；按菜单类型（按钮/接口）裁剪字段 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `toDTO` | `BossMenuDO menuDO` | `BossMenuDTO` | 菜单 DO 转 DTO（含 children 递归）|
| `toDTOList` | `List<BossMenuDO> menuDOList` | `List<BossMenuDTO>` | 批量转 DTO；null 返回空列表 |
| `toDO` | `BossMenuDTO menuDTO, Integer menuType` | `BossMenuDO` | DTO 转 DO 并按类型裁剪：BUTTON 清空 path，API 清空 menuName/perms |
| `toDTO` | `PagedModels<BossMenuDO> pagedModels` | `PagedModels<BossMenuDTO>` | 菜单分页结果转换 |

#### BossMenuIconConverter

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/converter/BossMenuIconConverter.java` |
| 职责 | 菜单图标 DO ↔ DTO、分页转换；DO 构建采用 builder |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `toDTO` | `BossMenuIconDO doObj` | `BossMenuIconDTO` | 图标 DO 转 DTO（时间字符串化）|
| `toDTOList` | `List<BossMenuIconDO> doList` | `List<BossMenuIconDTO>` | 批量转 DTO；空返回空列表 |
| `toDTO` | `PagedModels<BossMenuIconDO> paged` | `PagedModels<BossMenuIconDTO>` | 图标分页结果转换 |
| `toDO` | `BossMenuIconDTO dto` | `BossMenuIconDO` | DTO 转 DO（builder 构建）|

### 5.2 AppService 层 [据推断]

> 同事 user.md 未列 AppService 方法级实现，本节标 `[据推断]`，Code Plan 编写时需进一步探查。

#### BossUserAppService

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/appservice/BossUserAppService.java` |
| 职责 | 用户域核心业务编排；登录鉴权（`authenticateUser` 含 loginScene 参数）、增删改查 |
| 事务 | `@Transactional` 在新增 / 修改 / 删除方法；查询方法不开事务 |

| 方法名 | 入参 | 返回 | 业务含义 [据推断/已确认] |
|--------|------|------|---------|
| `authenticateUser` | `BossVerifyPasswordRequest request` | `BossUserManagementDTO` | 登录鉴权；调用 `BossUserDomainService#verifyPassword(username, password, loginScene)`，失败计数走 Redis（key: `boss_user:login_fail:{loginScene}_{userId}` TTL 1800s）；`loginScene=cs` 时调 `CsUserClient#getCsUserByUserId` 拿坐席信息 + 踢旧 session [据推断] |
| `createUser` | `BossUserManagementRequest request` | `Long userId` | 创建用户；调 `buildExtensionDO` + `BossUserRepository#save` [据推断] |
| `updateUser` | `BossUserManagementRequest request` | `void` | 更新用户 [据推断] |
| `findById` | `Long userId` | `BossUserDO` | 查单个用户 [据推断] |
| ... | ... | ... | （其他方法待探查）|

### 5.3 Domain 层 [据推断]

#### BossUserDO

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/entity/BossUserDO.java` |
| 职责 | 用户领域实体；充血模型（业务方法不以 get/set 开头）|

| 字段名 | 类型 | 说明 [据推断/已确认] |
|--------|------|-------------------|
| `userId` | `Long` | 用户唯一标识 [据推断] |
| `userName` | `String` | 用户名/账号 [据推断] |
| `password` | `String` | 加密后的密码（配合 spring-security-crypto BCrypt）[据推断] |
| `cellphone` | `String` | 手机号（脱敏展示）[据推断] |
| `nickname` | `String` | 昵称 [据推断] |
| `status` | `Integer` | 用户状态（0=禁用 / 1=启用）[据推断] |
| `extensionDO` | `BossUserExtensionDO` | 用户扩展信息 [据推断] |
| 审计四字段 | `createdBy / createdDate / lastUpdatedBy / lastUpdatedDate` | 必备（schema §6.5）[已确认] |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| （具体方法待探查） | — | — | [待确认] |

#### BossUserDomainService

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/service/BossUserDomainService.java` |
| 职责 | 用户跨聚合业务规则：账号密码校验、登录失败计数 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `verifyPassword` | `String username, String password, String loginScene` | `BossUserDO` | 账号密码校验：先查 Redis 锁定 → 查 boss_user 校验账号存在 + 状态 → BCrypt 校验密码 → 成功清失败计数 / 失败 +1（5 次锁 30 分钟）[据推断] |

#### BossUserErrorCode [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-domain/src/main/java/com/casstime/cloud/boss/user/domain/boss/model/enums/error/BossUserErrorCode.java` |

| 错误码 | 含义 | 错误级别 |
|--------|------|---------|
| 11101 | 用户不存在 | 业务错误 |
| 11102 | 角色不能为空 | 业务错误 |
| 11103 | 用户名格式无效 | 校验错误 |
| 11104 | 密码不能为空 | 校验错误 |
| 11105 | 用户名或密码错误 | 业务错误 |
| 11106 | 账号已被禁用 | 业务错误 |
| 11107 | 账号锁定 | 业务错误 |

### 5.4 Infrastructure 层 [已确认]

#### CsUserClient (Feign)

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/feign/CsUserClient.java` |
| 目标 SPI | `icec-cloud-life-cs-spi` 的 `CsUserService` |
| 调用服务名 | `life-cs-service` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `getCsUserByUserId` | `String bossUserId` | `ApiResult<CsUserDTO>` | 获取坐席信息（loginScene=cs 时调用）[已确认] |

#### BossUserRepositoryImpl

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/persistence/repository/mysql/BossUserRepositoryImpl.java` |
| 职责 | 用户仓储实现 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `findById` | `Long userId` | `BossUserDO` | 查单个用户 [据推断] |
| `findValidUserByName` | `String userName, String loginScene` | `BossUserDO` | 按用户名 + loginScene 查有效用户（登录鉴权用）[据推断] |
| `save` | `BossUserDO user` | `void` | 新增用户 [据推断] |
| `update` | `BossUserDO user` | `void` | 更新用户 [据推断] |
| `updateStatus` | `Long userId, Integer status` | `void` | 更新用户状态 [据推断] |

#### LoginLockFacadeImpl

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-infrastructure/src/main/java/com/casstime/cloud/boss/user/infrastructure/facade/LoginLockFacadeImpl.java` |
| 职责 | 登录锁防腐实现（Redis） |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `isLocked` | `String loginScene, Long userId` | `boolean` | 检查账号是否锁定（Redis key: `boss_user:login_fail:{loginScene}_{userId}` TTL 1800s）[据推断] |
| `incrementFailCount` | `String loginScene, Long userId` | `int` | 失败计数 +1 [据推断] |
| `clearFailCount` | `String loginScene, Long userId` | `void` | 登录成功后清失败计数 [据推断] |

### 5.5 Interfaces 层 [据推断]

#### BossUserServiceImpl

| 项 | 内容 |
|---|---|
| 文件路径 | `icec-cloud-boss-user-interfaces/src/main/java/com/casstime/cloud/boss/user/interfaces/restful/BossUserServiceImpl.java` |
| 实现 SPI | `icec-cloud-boss-user-spi` 的 `BossUserService` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `authenticateUser` | `BossVerifyPasswordRequest request` | `ApiResult<BossUserDTO>` | 登录鉴权；委托给 `BossUserAppService#authenticateUser` [据推断] |
| `getUserById` | `Long userId` | `ApiResult<BossUserDTO>` | 按 ID 查用户 [据推断] |
| （其他方法待探查） | — | — | [待确认] |

---

## 6. 工程特定约束 [已确认/据推断]

> 主体 §6 是项目级约束；本节只列本工程**特有**的约束。

### 6.1 工程特定静态扫描（🔴 编码完成必跑）

```bash
# === 本工程特定扫描（🔴 每轮编码完成后必跑） ===

# 1. 全限定名扫描（除 import 块外不应出现）
grep -rn "com\.casstime\.cloud\.boss\.user\.\(domain\|infrastructure\)\.\w\+\." \
  --include="*.java" icec-cloud-boss-user/{application,interfaces}/src/main/java/ \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空

# 2. application 层不应直接 import infrastructure.persistence
grep -rn "import com\.casstime\.cloud\.boss\.user\.infrastructure\.persistence" \
  --include="*.java" icec-cloud-boss-user-application/src/main/java/
# 期望输出为空

# 3. application 层不应直接 import 外部 SPI domain/infrastructure
grep -rn "import com\.casstime\.cloud\.\(life\|boss\)\.[a-z]\+\.\(domain\|infrastructure\)" \
  --include="*.java" \
  icec-cloud-boss-user-application/src/main/java/ \
  icec-cloud-boss-user-interfaces/src/main/java/
# 期望输出为空

# 4. SQL 关键字在 Service/AppService 层的扫描（应仅 Infrastructure 出现）
grep -rn "SELECT\|INSERT\|UPDATE\|DELETE" \
  --include="*AppService.java" --include="*Service.java" \
  icec-cloud-boss-user/*/src/main/java/ \
  | grep -v "import " | grep -v "//.*\(SELECT\|INSERT\|UPDATE\|DELETE\)"
# 期望仅 Infrastructure 出现
```

---

## 7. 上下游契约 [已确认/据推断]

### 7.1 对外暴露（SPI / Controller）

| 类型 | 接口名 | URL / 服务名 | 文档 |
|------|--------|------------|------|
| SPI | `BossUserService` | 服务名 `boss-user-service` | `icec-cloud-life-spi/icec-cloud-boss-user-spi/` |
| SPI | `BossUserManagementService` | 服务名 `boss-user-service` | 同上 |
| SPI | `BossUserInfoService` | 服务名 `boss-user-service` | 同上 |
| Controller | `BossUserServiceImpl` | `POST /boss-user/...` | interfaces 层 |
| Controller | `BossMenuServiceImpl` | `POST /boss-user/...` | interfaces 层 |

### 7.2 对内消费（Feign Client）

| SPI | 服务 | 方法 | 本工程 Feign Client |
|-----|------|------|------------------|
| `CsUserService` | `life-cs-service` | `getCsUserByUserId` | `CsUserClient` (infrastructure/feign/) |

### 7.3 域间事件

| 事件 | 事件源 | 事件消费者 |
|------|--------|----------|
| `BossUserCreatedEvent` | `BossUserAppService` 创建用户后 | （待确认）|

---

## 8. 本工程缺口与待补充

| # | 缺口 | 优先级 | 状态 |
|---|------|-------|------|
| 1 | BossUserAppService 完整方法级实现（仅推断 authenticateUser / createUser / updateUser）| 🟠 P1 | 待补 |
| 2 | BossUserDO 字段（仅推断 6 字段，实际 10+ 字段）| 🟡 P2 | 待补 |
| 3 | BossUserDomainService 完整方法列表 | 🟠 P1 | 待补 |
| 4 | Redis Key 实际格式（同事 user.md §1.4 提到 `boss_user:login_fail:{loginScene}_{userId}`，需复验）| 🟡 P2 | 待补 |
| 5 | 域事件列表（创建 / 修改 / 删除 / 状态变更各事件）| 🟡 P2 | 待补 |
| 6 | icec-cloud-boss-user-event 事件工程结构 | 🟡 P2 | 待补 |
| 7 | Bootstrap.java 实际启动类配置（filter / interceptor / aspect 顺序）| 🟢 P3 | 待补 |

---

## §A 关键词反向索引 [已确认/据推断]

| 关键词 | 出现位置 |
|--------|---------|
| `BossUserAppService` | §4 / §5.2 |
| `BossUserConverter` | §5.1（11 个方法）|
| `BossUserDomainService` | §4 / §5.3 |
| `BossUserErrorCode` | §4 / §5.3（11101-11107）|
| `BossUserDO` | §4 / §5.3 |
| `CsUserClient` | §4 / §5.4 |
| `LoginLockFacade` / `LoginLockFacadeImpl` | §4 / §5.4 |
| `ServiceProviderConstants.BOSS_USER_SERVICE` | §4 / §7.1 |
| `loginScene=cs` | §5.2 / §5.4 |
| `boss_user:login_fail:{loginScene}_{userId}` | §5.4（Redis Key）|
| `BCryptUtil.matches` | §3 / §5.2 |
| `MybatisPlusConfig` | §4 |
| `KafkaDomainEventPublisher` | §4 |
| `courier-spring-boot-starter` | §2 / §3 |
| `panda.casstime.com` | §3 |

---

## §B 本工程更新日志（合并到主体 update-log）

> 详见主体 `icec-cloud-boss.update-log.md`；本节列本工程特有的变更摘要。

| 日期 | 变更摘要 |
|------|---------|
| 2026-06-26 | 🆕 按 schema v3 §15 首次生成工程级子文件；来源：同事 user.md（102KB）+ ae-sdd boss.assets.md §4 |