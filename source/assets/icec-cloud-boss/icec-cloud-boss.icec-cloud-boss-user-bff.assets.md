---
name: icec-cloud-boss-user-bff-project-assets
description: icec-cloud-boss-user-bff 工程级项目资产 — 按 schema v3（2026-06-26）工程级粒度拆分 SOP 生成。Boss 用户 BFF 聚合层（Backend-For-Frontend）：单模块 jar，聚合 icec-cloud-boss-user + icec-cloud-boss-log 两个下游 SPI，含 4 个 AppService（28 方法）+ 3 个 OperationLoggable（角色/菜单/角色菜单绑定）+ 6 个 Feign Client + GlobalExceptionHandler。本文与 boss-user（多模块 DDD）形成"单模块 BFF vs 多模块 Service"对比范例。
parent: icec-cloud-boss.assets.md
探查时间: 2026-06-26
探查来源: 同事 D:\Item\life\document\life-team-project-docs\knowledge\project\icec-cloud-boss-user-bff.md (72KB / 1232 行) + ae-sdd boss.assets.md §4 DDD 落点
---

# icec-cloud-boss-user-bff 工程级项目资产

> **本文是 `icec-cloud-boss.assets.md` 的工程级子文件**（schema §15），仅含本工程细节。跨工程信息见主体。
>
> **范本定位**：本文是 schema §15 工程级拆分的**第 3 份实例**（前两份：boss-user 多模块 DDD / life-cs 客服域状态机）。重点展示 **BFF 单模块工程**如何落表 — 与 boss-user（5 模块聚合父工程）形成"Service vs BFF"对比范例，演示 BFF 的 Facade 模式 + OperationLog capability + 异常降级 三大典型模式。

---

## 0. 摘要与使用场景 [已确认]

| 维度 | 内容 |
|------|------|
| 工程名 | `icec-cloud-boss-user-bff` |
| 父工程 | `icec-cloud-boss` |
| 探查时间 | 2026-06-26 |
| 工程定位 | Boss 用户 BFF（Backend-For-Frontend）聚合层；面向 Boss 管理后台前端，编排 + 聚合 icec-cloud-boss-user-service + icec-cloud-boss-log-service 两个下游 SPI |
| 工程类型 | 🆕 **单模块 jar**（非聚合父工程）；对比 boss-user 是 5 模块 pom |
| 关键不变量 | 本工程不重复定义 rules；只把 rules 映射到本工程代码 |
| 最后审计 | 2026-06-26 |
| 关键架构特性 | 🆕 BFF 5 层轻量分层（接口/应用/防腐/领域/基础设施）+ Facade 模式（异常降级返回 null/空集合/Result.error）+ OperationLog capability 模式（3 个 Loggable）+ Feign Client 必 extends SPI 接口 + TokenService 必用 |

---

## 1. 模块元信息 [已确认]

| 字段 | 值 |
|------|---|
| moduleName | `icec-cloud-boss-user-bff` |
| groupId | `com.casstime.cloud` |
| artifactId | `boss-user-bff` |
| version | `1.1`（注意：非 1.0-SNAPSHOT，是带版本号的发布版）|
| packaging | `jar`（**单模块**，非聚合父工程）|
| 子模块数 | 0（单模块工程；包结构代替模块结构）|
| 主启动类 | `[待确认]`（boss-user-bff.md §1.6 标注"源码未提供"）|
| profile | `dev, test, prod, beta-kunlun` |
| port | `12003` |
| contextPath | `/boss-user-bff` |
| dependsOnSpi | [`icec-cloud-boss-user-spi 1.0-SNAPSHOT`, `icec-cloud-boss-log-spi 1.0-SNAPSHOT`] |
| lastAuditedAt | `2026-06-26` |
| owner | 架构组 + user 域负责人 |

### 1.1 部署信息 [已确认]

> 🆕 2026-06-26 schema §1.2 新增节。本节从 `bootstrap.yml` + pom.xml 抽取。

| 字段 | 值 | 抽取来源 |
|------|---|---------|
| `application.name` | `boss-user-bff` | `bootstrap.yml` |
| `profile.active` | `beta-kunlun` | `bootstrap.yml` |
| `server.port` | `12003` | `bootstrap.yml` |
| `server.contextPath` | `/boss-user-bff` | `bootstrap.yml` |
| `db.*` | **不直接连 DB**（BFF 聚合层，🔴 BFF 禁直连 DB）| schema §6.1 红线 |
| `redis.*` | 启用 spring-boot-starter-data-redis（用于 JWT 会话存储 / 缓存）[据推断] | pom 依赖 |
| `gateway` | `http://life-hwbeta-api-kunlun.intra.casstime.com`（icec.api.agent）| `bootstrap.yml` |
| `imageRepo` | `swr.cn-south-1.myhuaweicloud.com/cassmall/boss-user-bff:1.1` | `pom.xml` `docker.imageName/imageVersion` |
| `nexusRepo` | `http://dev.casstime.com/nexus/content/groups/public/` | `pom.xml` repositories |
| `feign.connectTimeoutMillis` | `1000` | `bootstrap.yml` |
| `feign.httpclient.enabled` | `true` | `bootstrap.yml` |
| `feign.httpclient.pool.maxConnPerRoute` | `100` | `bootstrap.yml` |
| `feign.httpclient.pool.maxConnTotal` | `1000` | `bootstrap.yml` |
| `feign.hystrix.enabled` | `false`（关闭 Hystrix 熔断）| `bootstrap.yml` |
| `panda.config.server-addr` | `http://panda.casstime.com` | `bootstrap.yml` |
| `panda.config.product` | `b2c` | `bootstrap.yml` |
| `panda.config.instance` | `boss-user-bff` | `bootstrap.yml` |
| `swagger.enabled` | `true` | `bootstrap.yml` |
| `swagger.resourceUrl` | `http://life-hwbeta-api.intra.casstime.com/auth/bff` | `bootstrap.yml` |
| `encrypt.failOnError` | `false`（加密失败不中断启动）| `bootstrap.yml` |
| `management.port` | 未显式配置（默认 30000）| ⚠️ [据推断] |
| `coverageTool` | 未配置 JaCoCo（BFF 工程常见）| ⚠️ [据推断] |

### 1.2 安全提示 [已确认/待确认]

> 🆕 2026-06-26 schema §14 新增节。

| ID | 类型 | 位置 | 风险等级 | 描述 | 建议修复 | 状态 |
|----|------|------|---------|------|---------|------|
| S-201 | BFF 禁直连 DB 检查项 | 工程内 pom.xml | 🟢 低 | 🔴 BFF 严禁直连 DB（schema §6.1 红线）— 需确认本工程无 `mybatis-plus` / `mysql-connector` 显式依赖 | 跑 `grep -E "mybatis-plus\|mysql-connector" pom.xml`，期望输出为空 | 已规则化 |
| S-202 | Feign Client 必 extends SPI 接口 | 工程内 `infrastructure/feign/` | 🟢 低 | 🔴 BFF Feign Client 必须 extends SPI 接口（schema §6.9）— 需确认 6 个 Client 都 extends 对应 SPI | 跑 `grep "@FeignClient" infrastructure/feign/*.java`，验证每个 Client 都有 `extends` 关键字 | 已规则化 |
| S-203 | BFF AppService 必经 Facade 调 Feign | 工程内 `application/appservice/` | 🟢 低 | 🔴 BFF AppService 必须经 Facade 调 Feign（schema §6.9）— 不允许 `@Autowired XxxClient` 直接用 | 跑 `grep "@Autowired.*Client" application/appservice/*.java`，期望输出为空 | 已规则化 |
| S-204 | TokenService 必用 / 禁 AccessUserInfoContext | 工程内 `application/appservice/` | 🟢 低 | 🔴 BFF 必用 `TokenService.getCurUserId()` 取当前用户，**禁用** `AccessUserInfoContext`（schema §6.6）| 跑 `grep "AccessUserInfoContext" -r`，期望输出为空 | 已规则化 |
| S-205 | Feign 异常降级返回 null/空集合/Result.error | Facade 层 | 🟢 低 | 🔴 Facade 异常时返回 null/空集合/Result.error（schema §6.9）| 跑 `grep -A 3 "catch" application/facade/*.java`，验证 catch 块符合规范 | 已规则化 |
| S-206 | management.port 未配置 | `bootstrap.yml` | 🟢 低 | 未显式配置 `management.port`，本地多服务同时启动可能端口冲突 | 显式配置 `management.port: 30003`（与 server.port 错开）| 待修 |

---

## 2. 子模块结构 [已确认]

> 🆕 **BFF 单模块工程** — 不像 boss-user 是 5 模块聚合父工程，本工程用包结构（package）代替模块结构隔离职责。

| 包路径 | 对应 DDD 层 | 职责 | 主要依赖 |
|--------|-----------|------|---------|
| `src/main/java/com/casstime/cloud/boss/bff/user/interfaces/` | 接口层（Interface / Web）| 对外暴露 HTTP 接口；`BossUserManagementRestImpl` / `BossMenuRestImpl` / `BossRoleRestImpl` / `BossMenuIconRestImpl` 实现 `icec-cloud-boss-user-bff-api` 的接口契约 | `icec-cloud-boss-user-bff-api 1.0-SNAPSHOT`、`icec-cloud-base-webapp` |
| `src/main/java/com/casstime/cloud/boss/bff/user/application/` | 应用层（Application）| 编排 Facade 客户端调用 + 模型转换 + 注入 TokenService 取当前用户；包含 `appservice/`（4 个 AppService）+ `converter/`（Request↔VO）+ `facade/`（防腐层接口）+ `facade/impl/`（防腐层实现）| `icec-cloud-boss-user-spi`、`icec-cloud-boss-log-spi`、`icec-cloud-boss-security 2.0.6` |
| `src/main/java/com/casstime/cloud/boss/boss/bff/user/application/facade/` | 防腐层（Anti-Corruption Layer）| 抽象外部服务调用；具体实现经 Feign Client 调到下游 SPI；Facade 异常时返回 null/空集合/Result.error | `icec-cloud-boss-user-spi`、`icec-cloud-boss-log-spi` |
| `src/main/java/com/casstime/cloud/boss/bff/user/domain/` | 领域层（Domain）| BFF 领域层（轻量，含领域模型 / 枚举 / 仓储接口定义）[据推断，BFF 域层相对薄弱] | — |
| `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/` | 基础设施层（Infrastructure）| 持久化（如有）+ Feign Client（6 个）+ OperationLog capability（3 个）+ OperationLog Provider（1 个）+ Config（WebMvcConfig / GlobalExceptionHandler）+ Exception（UnAuthorizeException）+ 占位包（facade/utils/constants）| `spring-cloud-starter-feign`、`icec-cloud-boss-operation-log-starter 1.0-SNAPSHOT`、`icec-cloud-boss-security 2.0.6` |

### 2.1 依赖层次关系 [已确认]

```
boss-user-bff（单模块）
├── interfaces/        → icec-cloud-boss-user-bff-api（接口契约）
├── application/       → icec-cloud-boss-user-spi（用户 SPI）
│   └── facade/impl/   ─┤
├── domain/            → （轻量，仅定义模型 / 枚举 / 接口）
└── infrastructure/    → icec-cloud-boss-user-spi / log-spi
    ├── feign/         ─┤（6 个 Feign Client 必 extends SPI 接口）
    ├── operationlog/  ─┤（3 个 OperationLoggable + 1 个 OperatorInfoProvider）
    └── config/        → spring-web + boss-security
```

> BFF 5 层轻量分层符合 DDD 简化版：facade 层封装下游服务调用，异常降级返回 null/空集合/Result.error；OperationLog capability 通过 `@Aspect` 切面自动埋点。

---

## 3. 完整技术栈版本号 [已确认]

> 🆕 2026-06-26 schema §6.8 新增节。**完整 7 张表见主体 §6.8**，本节只列本工程特有的依赖。

| 依赖 | 版本 | 用途 |
|------|------|------|
| `spring-boot-maven-plugin` | `2.3.5.RELEASE`（注意：比 boss-user 的 2.7.5 旧！）| Fat JAR 打包（repackage 目标）|
| `dockerfile-maven-plugin` | `1.4.0`（com.spotify）| Docker 镜像构建 |
| `spring-boot-starter-web` | 跟随 Spring Boot | Web MVC + 嵌入式容器 |
| `spring-boot-starter-aop` | 跟随 Spring Boot | 🔴 AOP 支持（操作日志切面）|
| `spring-boot-starter-data-redis` | 跟随 Spring Boot | Redis 缓存/JWT 会话存储 [据推断] |
| `spring-cloud-starter-feign` | 跟随 Spring Cloud Dalston | 6 个 Feign Client |
| `panda-spring-boot-starter` | `1.0.9` | 配置中心 |
| `casslog-spring-boot-starter` | `1.5.0` | 日志组件（排除 icec-cloud-commons 传递依赖）|
| `cassmetrics-spring-boot-v1-starter` | `1.0.4` | 监控指标采集 |
| `cass-config-spring-boot-starter` | `1.1.2` | 配置加载 |
| `icec-cloud-base-webapp` | `b2c.1.0-SNAPSHOT` | 🆕 Web 应用基础能力封装 |
| `icec-cloud-boss-user-bff-api` | `1.0-SNAPSHOT` | 本服务对外 BFF 接口契约 |
| `icec-cloud-boss-user-spi` | `1.0-SNAPSHOT` | 用户 SPI 下游契约 |
| `icec-cloud-boss-log-spi` | `1.0-SNAPSHOT` | 日志 SPI 下游契约 |
| `icec-cloud-boss-operation-log-starter` | `1.0-SNAPSHOT` | 🔴 操作日志记录组件（含 `@OperationLog` 注解 + 切面）|
| `icec-cloud-boss-security` | `2.0.6`（注意：版本号 2.x，与 boss-user 的传递依赖不同）| Boss 端安全/鉴权（TokenService / @SkipAuth / @RequiresPermissions）|
| `icec-cloud-spi-common` | `b2c.1.0-SNAPSHOT` | SPI 公共契约 |
| `icec-cloud-life-api-common` | `1.0-SNAPSHOT` | 生活服务 API 公共契约 |
| `swagger` | `2.8.0` | 接口文档（必加）|
| `lombok` | `1.16.20`（注意：比 boss-user 的 1.18.16 旧！）| @Data / @RequiredArgsConstructor / @Slf4j |
| `log4j-bom` | `2.17.2` | 日志依赖版本管理（dependencyManagement 首位）|
| `maven-source-plugin` | 跟随 | 编译阶段生成 source jar |
| `maven-compiler-plugin` | source/target 1.8 / UTF-8 | Java 8 编译 |

---

## 4. DDD 内部分层落点 [已确认/据推断]

> **详细类角色映射见主体 §4**；本节只列本工程内实际类。

| 类角色 | 精确包路径 | 典型类名（已确认）|
|--------|-----------|------------------|
| **Interfaces** | | |
| Rest 实现类 | `src/main/java/com/casstime/cloud/boss/bff/user/interfaces/restful/` | `BossUserManagementRestImpl` / `BossMenuRestImpl` / `BossRoleRestImpl` / `BossMenuIconRestImpl`（implements `icec-cloud-boss-user-bff-api` 中的 `{Resource}Rest`）|
| **Application** | | |
| AppService | `src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/` | `BossMenuIconAppService`（5 方法）/ `BossMenuAppService`（10 方法）/ `BossRoleAppService`（13 方法）/ `BossUserManagementAppService`（6 方法 + 1 私有）|
| Facade 接口 | `src/main/java/com/casstime/cloud/boss/bff/user/application/facade/` | （具体接口待确认）[据推断] |
| Facade 实现 | `src/main/java/com/casstime/cloud/boss/bff/user/application/facade/impl/` | （具体实现待确认）[据推断] |
| Converter | `src/main/java/com/casstime/cloud/boss/bff/user/application/converter/` | `BossUserManagementConverter`（request ↔ user page request）|
| **Domain** | | |
| Domain Object | `src/main/java/com/casstime/cloud/boss/bff/user/domain/` | （BFF 域层轻量，具体类待确认）[据推断] |
| **Infrastructure** | | |
| Feign Client | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/feign/` | `BossUserManagementClient`（extends `BossUserManagementService`）/ `BossUserInfoClient`（extends `BossUserInfoService`）/ `BossRoleClient`（extends `BossRoleService`）/ `BossMenuClient`（extends `BossMenuService`）/ `BossMenuIconClient`（extends `BossMenuIconService`）/ `BossOperationLogClient`（extends `OperationLogService`）|
| OperationLoggable | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/operationlog/capability/` | `RoleMenuOperationLoggable`（角色菜单权限绑定）/ `RoleOperationLoggable`（角色 CRUD）/ `MenuOperationLoggable`（菜单 CRUD）|
| OperatorInfoProvider | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/operationlog/` | `BossOperatorInfoProvider`（操作人信息提供者，实现 `OperatorInfoProvider`）|
| Config | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/config/` | `WebMvcConfig`（继承 `GenericWebMvcConfig`）/ `GlobalExceptionHandler`（`@RestControllerAdvice` + 8 个 handler 方法）|
| Exception | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/exception/` | `UnAuthorizeException`（继承 `RuntimeException`，未获取当前用户登录信息时抛）|
| 占位包 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/{facade,utils,constants}/` | 仅有 `package-info.java` 占位 |

---

## 5. 核心类方法级实现（🆕 2026-06-26）

> **🆕 范本节**：BFF 4 个 AppService 共 34 个方法（其中 1 个私有） + 3 个 OperationLoggable + 1 个 OperatorInfoProvider + 6 个 Feign Client + GlobalExceptionHandler 8 个 handler 方法 + UnAuthorizeException。完整覆盖 BFF 编排层的全部能力（来源：同事 boss-user-bff.md §4-§5）。

### 5.1 Converter 层 [已确认/据推断]

#### BossUserManagementConverter [据推断]

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/application/converter/BossUserManagementConverter.java` |
| 职责 | 用户管理 Request ↔ PageRequest 转换 |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `toUserPageRequest` | `PageRequest<BossUserApiPageRequest>` | `UserPageRequest<BossUserPageCondition>` | BFF 入参转换：API 分页请求 → SPI 分页请求 [据推断] |

### 5.2 AppService 层 [已确认]

> 🆕 **BFF AppService 编排模式**：注入 Feign Client + TokenService；TokenService 注入当前用户 ID（createdBy/lastUpdatedBy）；Converter 做模型转换；封装 `ApiResult` 返回。

#### BossMenuIconAppService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/BossMenuIconAppService.java` |
| 职责 | 🆕 菜单图标管理应用服务；编排菜单图标的增删改查，封装 Feign 调用与模型转换 |

**核心字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `bossMenuIconClient` | `BossMenuIconClient` | 菜单图标 Feign 客户端 |

**核心方法（5 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `getMenuIconList` | `BossMenuIconQueryRequest queryRequest` | `ApiResult<PagedModels<BossMenuIconVO>>` | 🔴 分页查询菜单图标列表；转换查询条件 → 调 client → 重组分页 VO |
| `getMenuIconById` | `Long id` | `ApiResult<BossMenuIconVO>` | 根据 ID 查询菜单图标详情 |
| `createMenuIcon` | `BossMenuIconApiRequest apiRequest` | `ApiResult<Long>` | 创建菜单图标，返回新建图标 ID |
| `updateMenuIcon` | `BossMenuIconApiRequest apiRequest` | `ApiResult<Boolean>` | 更新菜单图标 |
| `deleteMenuIcon` | `Long id` | `ApiResult<Boolean>` | 根据 ID 删除菜单图标 |

#### BossMenuAppService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/BossMenuAppService.java` |
| 职责 | 🆕 菜单管理应用服务；编排菜单及按钮权限的查询、增删改 + 菜单树 / 权限标识获取 |

**核心字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `bossMenuClient` | `BossMenuClient` | 菜单 Feign 客户端 |
| `tokenService` | `TokenService` | 🔴 令牌服务，获取当前用户 ID |

**核心方法（10 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `getMenuList` | `PageRequest<BossMenuQueryRequest> menuRequest` | `ApiResult<PagedModels<BossMenuVO>>` | 分页获取菜单列表；转换分页条件 → 调 client → 重组分页 VO |
| `getMenuById` | `String id` | `ApiResult<BossMenuVO>` | 查看菜单详情 |
| `createMenu` | `BossMenuApiRequest menuRequest` | `ApiResult<String>` | 🆕 新增菜单，**注入当前用户 ID**（createdBy），返回菜单 ID |
| `updateMenu` | `BossMenuApiRequest menuRequest` | `ApiResult<Boolean>` | 🆕 修改菜单，**注入当前用户 ID**（lastUpdatedBy）|
| `deleteMenu` | `String id` | `ApiResult<Boolean>` | 删除菜单 |
| `getChildMenus` | `String menuId` | `ApiResult<List<BossMenuVO>>` | 根据菜单 ID 获取子菜单 |
| `getMenuAll` | 无 | `ApiResult<List<BossMenuVO>>` | 获取所有菜单 |
| `getRoleMenuTree` | 无 | `ApiResult<List<BossMenuVO>>` | 🔴 根据当前用户 ID 获取用户菜单树 |
| `getPermsByUserId` | 无 | `ApiResult<List<String>>` | 🔴 获取当前用户按钮权限标识列表（用于前端按钮级权限控制）|
| `getAllButtons` | 无 | `ApiResult<List<BossMenuVO>>` | 获取所有按钮菜单 |

#### BossRoleAppService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/BossRoleAppService.java` |
| 职责 | 🆕 角色管理应用服务；编排角色增删改查、授权、菜单绑定、角色从属关系 |

**核心字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `bossRoleClient` | `BossRoleClient` | 角色 Feign 客户端 |
| `tokenService` | `TokenService` | 令牌服务 |

**核心方法（13 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `getAllRoles` | 无 | `ApiResult<List<BossRoleVO>>` | 获取所有角色 |
| `getRoleList` | `PageRequest<BossRoleApiPageRequest> pageRequest` | `ApiResult<PagedModels<BossRoleVO>>` | 🆕 分页查询角色列表，**条件中注入当前用户 ID** |
| `getRoleById` | `String id` | `ApiResult<BossRoleVO>` | 🆕 根据 ID 查看角色详情，**携带当前用户 ID** |
| `createRole` | `BossRoleApiRequest roleRequest` | `ApiResult<String>` | 🆕 新增角色，**注入当前用户 ID**，返回角色 ID |
| `updateRole` | `BossRoleApiRequest roleRequest` | `ApiResult<Boolean>` | 🆕 修改角色，**注入当前用户 ID** |
| `deleteRole` | `String id` | `ApiResult<Boolean>` | 🆕 删除角色，**携带当前用户 ID** |
| `getMenusByRoleId` | `String roleId` | `ApiResult<List<BossMenuVO>>` | 获取角色的菜单权限 |
| `getUsersByRoleId` | `String roleId` | `ApiResult<List<BossUserManagementVO>>` | 获取角色下的用户列表 |
| `authRoles` | `BossRoleAuthApiRequest authRoleApiRequest` | `ApiResult<Boolean>` | 🆕 批量授权角色，**注入当前用户 ID** |
| `cancelAuthRoles` | `BossRoleAuthApiRequest authRoleApiRequest` | `ApiResult<Boolean>` | 🆕 批量取消用户角色绑定，**注入当前用户 ID** |
| `getRoleRelationDetails` | `String roleId` | `ApiResult<BossRoleRelationDetailVO>` | 获取角色从属关系配置详情 |
| `queryCandidateRelationRoles` | `BossRoleRelationCandidateQueryRequest request` | `ApiResult<List<BossRoleRelationSimpleVO>>` | 🆕 查询可选角色列表，**仅返回启用（enabled）的角色** |
| `saveRoleRelations` | `String roleId, BossRoleRelationConfigSaveRequest request` | `ApiResult<Boolean>` | 🆕 保存角色从属关系配置，**注入操作人 ID 并记录日志** |
| `getRoleTree` | 无 | `ApiResult<BossRoleRelationPreviewTreeVO>` | 🔴 获取全量角色树（**携带当前用户 ID 的可配置角色树**）|
| `bindMenu` | `BossRoleBindMenuApiRequest request` | `ApiResult<Boolean>` | 🆕 为角色绑定菜单（roleId + menuIdList）|

#### BossUserManagementAppService [已确认]

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/BossUserManagementAppService.java` |
| 职责 | 🆕 用户管理应用服务；编排用户新增/更新/状态变更/删除/详情/分页列表 + 手机号脱敏 |

**核心字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `bossUserManagementClient` | `BossUserManagementClient` | 用户管理 Feign 客户端 |
| `tokenService` | `TokenService` | 令牌服务 |

**核心方法（6 + 1 私有）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `saveOrUpdate` | `BossUserManagementReq request` | `ApiResult<String>` | 🆕 新增或更新用户；通过 `BossUserManagementConverter` 转换请求体，**注入当前用户 ID**，返回用户 ID |
| `updateUserStatus` | `String userId, BossUserStatusReq request` | `ApiResult<BossUserStatusVO>` | 🆕 更新指定用户状态；转换请求 → 调 client → DTO 转 VO |
| `deleteUser` | `String userId` | `ApiResult<BossUserStatusVO>` | 删除指定用户；DTO 转 VO |
| `getUserDetail` | `String userId` | `ApiResult<BossUserManagementVO>` | 获取用户详情；DTO 转 VO |
| `getUserList` | `PageRequest<BossUserApiPageRequest> pageRequest` | `ApiResult<BossUserApiPagedModels>` | 🆕 分页查询用户列表；**转换分页条件为 `UserPageRequest<BossUserPageCondition>`** → 调 client → 转 PagedModels |
| `getUserOptions` | 无 | `ApiResult<List<BossUserOptionVO>>` | 🔴 获取用户下拉列表；以 **page=1, size=9999** 拉全量；手机号经 `maskCellphone` 脱敏；**客户端调用失败或数据为空时返回空列表** |
| `maskCellphone`（私有）| `String cellphone` | `String` | 🔴 手机号脱敏：空值或长度 ≤ 4 时原样返回；否则保留后 4 位，前缀替换为单个 `*` |

### 5.3 Domain 层 [据推断]

> BFF 域层相对薄弱（轻量 BFF 模式）；具体类待探查 [据推断]。

### 5.4 Infrastructure 层 [已确认]

#### 🆕 OperationLoggable 模式（操作日志 capability）

> 🆕 **BFF 独有模式**：操作日志能力通过 `OperationLoggable` 接口实现，由 `icec-cloud-boss-operation-log-starter` 的 `@OperationLog` 注解 + AOP 切面自动埋点。

##### RoleMenuOperationLoggable — 角色菜单权限绑定操作日志

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/operationlog/capability/RoleMenuOperationLoggable.java` |
| 职责 | 🔴 **角色菜单权限绑定的操作日志能力**；对比绑定前后菜单集合，生成「新增权限/移除权限」差异描述 |
| 实现接口 | `OperationLoggable<RoleMenuOperationLoggable.RoleMenuLogData>` |
| 依赖 | `BossRoleAppService`（查询角色名 + 角色菜单列表）|

**核心方法（8 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `before` | `Object[] args, OperationLogTypeEnum operationType` | `RoleMenuLogData` | 解析 roleId，查询绑定前菜单快照；roleId 为空返回 null |
| `after` | `Object[] args, Object result, OperationLogTypeEnum operationType` | `RoleMenuLogData` | 解析 roleId，查询绑定后菜单快照 |
| `buildContent` | `RoleMenuLogData beforeData, RoleMenuLogData afterData, OperationLogTypeEnum operationType` | `String` | 🔴 对比前后菜单名集合，输出「**新增权限:[…]，移除权限:[…]**」或「权限无变化」；失败时输出失败提示 |
| `getTarget` | `Object[] args, RoleMenuLogData beforeData, RoleMenuLogData afterData` | `OperationTarget` | 以 roleId 为目标 ID，查询角色名作为目标名称（查询失败降级用 roleId）|
| `onFailure` | `Object[] args, Throwable ex` | `RoleMenuLogData` | 标记 `failed=true`，记录 roleId |
| `resolveRoleId` | `Object[] args` | `String` | 从参数中提取 `BossRoleBindMenuApiRequest` 的 roleId（private）|
| `queryMenus` | `String roleId` | `List<BossMenuVO>` | 调用 AppService 查询角色菜单；**异常降级返回空集合**（private）|
| `toMenuNameSet` | `List<BossMenuVO> menus` | `Set<String>` | 提取菜单名去重为有序集合（private）|

**内部数据载体 `RoleMenuLogData`（static class）字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `roleId` | `String` | 角色 ID |
| `menus` | `List<BossMenuVO>` | 菜单快照列表 |
| `failed` | `boolean` | 操作是否失败标记 |

##### RoleOperationLoggable — 角色 CRUD 操作日志

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/operationlog/capability/RoleOperationLoggable.java` |
| 职责 | 🔴 角色新增/修改/删除（含从属关系）的操作日志能力 |
| 实现接口 | `OperationLoggable<RoleOperationLoggable.RoleLogData>` |
| 依赖 | `BossRoleAppService`（按 ID 查询角色）|

**核心方法（8 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `before` | `Object[] args, OperationLogTypeEnum operationType` | `RoleLogData` | CREATE 跳过；其余解析 roleId 查询旧值快照 |
| `after` | `Object[] args, Object result, OperationLogTypeEnum operationType` | `RoleLogData` | DELETE 跳过；从 `BossRoleApiRequest` 构建新值快照（新增时从 result 取生成 ID）|
| `buildContent` | `RoleLogData beforeData, RoleLogData afterData, OperationLogTypeEnum operationType` | `String` | 按类型输出「**新增/修改/删除角色[名称]**」，默认用 `operationType.getDescription()` |
| `getTarget` | `Object[] args, RoleLogData beforeData, RoleLogData afterData` | `OperationTarget` | 优先取 after，否则 before，以 id + roleName 构建目标 |
| `fromRequest` | `BossRoleApiRequest req, Object result` | `RoleLogData` | 从请求构建快照；ID 为空时从 `ApiResult.data` 取（private）|
| `resolveRoleId` | `Object[] args` | `String` | 从 `BossRoleApiRequest.id` 或 String 参数提取（private）|
| `queryRole` | `String roleId` | `RoleLogData` | 查询角色并转为快照，**异常降级 null**（private）|
| `getArg` | `Object[] args, Class<T> clazz` | `T` | 泛型参数提取工具（private）|

**内部数据载体 `RoleLogData` 字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `id` | `String` | 角色 ID |
| `roleName` | `String` | 角色名称 |
| `roleCode` | `String` | 角色编码 |
| `status` | `Boolean` | 启用状态 |

##### MenuOperationLoggable — 菜单 CRUD 操作日志

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/operationlog/capability/MenuOperationLoggable.java` |
| 职责 | 🔴 菜单新增/修改/删除的操作日志能力 |
| 实现接口 | `OperationLoggable<MenuOperationLoggable.MenuLogData>` |
| 依赖 | `BossMenuAppService`（按 ID 查询菜单）|

**核心方法（7 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `before` | `Object[] args, OperationLogTypeEnum operationType` | `MenuLogData` | 解析 menuId 查询操作前菜单快照 |
| `after` | `Object[] args, Object result, OperationLogTypeEnum operationType` | `MenuLogData` | 优先从 result 取生成 ID，否则从参数取，查询操作后菜单快照 |
| `buildContent` | `MenuLogData beforeData, MenuLogData afterData, OperationLogTypeEnum operationType` | `String` | 按类型输出「**新增/修改/删除菜单[名称]**」 |
| `getTarget` | `Object[] args, MenuLogData beforeData, MenuLogData afterData` | `OperationTarget` | 优先取 after，否则 before，以 id + menuName 构建目标 |
| `resolveMenuId` | `Object[] args` | `String` | 从 `BossMenuApiRequest.id` 或 String 参数提取（private）|
| `resolveResultId` | `Object result` | `String` | 从 `ApiResult.data`（String）提取生成 ID（private）|
| `queryMenu` | `String menuId` | `BossMenuVO` | 查询菜单，**异常降级 null**（private）|
| `toLogData` | `BossMenuVO menu` | `MenuLogData` | VO 转日志快照（private）|

**内部数据载体 `MenuLogData` 字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `id` | `String` | 菜单 ID |
| `menuName` | `String` | 菜单名称 |
| `parentId` | `String` | 父菜单 ID |
| `path` | `String` | 菜单路径 |
| `perms` | `String` | 权限标识 |
| `status` | `Boolean` | 启用状态 |

#### 🆕 BossOperatorInfoProvider — 操作人信息提供者

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/operationlog/BossOperatorInfoProvider.java` |
| 职责 | 🔴 实现操作日志 SPI 的操作人信息提供；通过 `TokenService` 获取当前用户 ID，再按 userId 查询最新用户名 |
| 实现接口 | `OperatorInfoProvider` |
| 依赖 | `TokenService`（当前用户 ID）+ `BossUserInfoClient`（用户信息查询）|

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `getOperator` | 无 | `OperatorInfo` | 🔴 取当前用户 ID（为空返回 null）→ 查询用户名构建 `OperatorInfo`；查询失败降级仅含 userId；整体异常返回 null |

#### 🆕 Feign Client（6 个，必 extends SPI 接口）

| 类名 | 服务名（`@FeignClient name`）| 继承 SPI 接口 | 职责 |
|------|----------------------------|-----------|------|
| `BossUserManagementClient` | `boss-user-service` | `BossUserManagementService` | 用户管理服务调用 |
| `BossUserInfoClient` | `boss-user-service` | `BossUserInfoService` | 用户信息查询服务调用 |
| `BossRoleClient` | `boss-user-service` | `BossRoleService` | 角色管理服务调用 |
| `BossMenuClient` | `boss-user-service` | `BossMenuService` | 菜单管理服务调用 |
| `BossMenuIconClient` | `boss-user-service` | `BossMenuIconService` | 菜单图标服务调用 |
| `BossOperationLogClient` | `boss-log-service` | `OperationLogService` | 操作日志服务调用 |

> 🔴 **必检**：每个 Client 都有 `extends` 关键字（schema §6.9 红线 — BFF Feign Client 必须 extends SPI 接口，不允许写新的 `@FeignClient` 不接 SPI）

#### 🆕 GlobalExceptionHandler — 全局异常处理器

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/config/GlobalExceptionHandler.java` |
| 职责 | 🔴 全局异常拦截；将各类异常转换为统一 `ApiResult` 响应（HTTP 200），含参数校验 / 业务异常 / 兜底处理 |
| 注解 | `@RestControllerAdvice` + `@Slf4j` |

**核心字段：**

| 字段名 | 类型 | 说明 |
|--------|------|------|
| `MESSAGE_PARAM_INVALID` | `String`（常量）| 参数校验失败统一提示文案「参数验证失败」|

**核心方法（8 个）：**

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `handleException` | `Exception e, HttpServletRequest request` | `ApiResult<Object>` | 🔴 统一入口（`@ExceptionHandler(Exception.class)` + `@ResponseStatus(OK)`），按异常类型分发 |
| `httpMessageExceptionHandler` | `HttpMessageException exception` | `ApiResult<Object>` | 处理 HTTP 消息异常，**透传 code + message**（private）|
| `businessExceptionHandler` | `BusinessException exception` | `ApiResult<Object>` | 处理业务异常，**透传 code + message**（private）|
| `codeMessageExceptionHandler` | `CodeMessageException exception` | `ApiResult<Object>` | 处理带 code 的消息异常（private）|
| `handleMethodArgumentNotValid` | `MethodArgumentNotValidException e` | `ApiResult<Object>` | 处理 `@RequestBody + @Valid` 校验失败，**收集字段错误**（private）|
| `handleConstraintViolation` | `ConstraintViolationException e` | `ApiResult<Object>` | 处理 `@PathVariable/@RequestParam + @Validated` 校验失败，**简化字段名**（private）|
| `handleBindException` | `BindException e` | `ApiResult<Object>` | 处理表单参数（非 JSON）校验失败（private）|
| `handleOtherException` | `Exception e, HttpServletRequest request` | `ApiResult<Object>` | 🆕 兜底处理，**记录错误日志并返回失败响应**（private）|

#### WebMvcConfig

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/config/WebMvcConfig.java` |
| 职责 | 继承 `GenericWebMvcConfig`，扩展 Web MVC 配置（资源处理器注册）|
| 注解 | `@Configuration` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `addResourceHandlers` | `ResourceHandlerRegistry registry` | `void` | 调用父类默认资源处理器注册逻辑 |

#### 🆕 UnAuthorizeException

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/exception/UnAuthorizeException.java` |
| 职责 | 🔴 未获取当前用户登录信息时抛出的运行时异常 |
| 继承 | `RuntimeException` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `UnAuthorizeException`（构造器）| `String message` | — | 构造异常，传入错误描述消息 |

### 5.5 Interfaces 层 [据推断]

#### BossUserManagementRestImpl

| 项 | 内容 |
|---|---|
| 文件路径 | `src/main/java/com/casstime/cloud/boss/bff/user/interfaces/restful/BossUserManagementRestImpl.java` |
| 实现 SPI | `icec-cloud-boss-user-bff-api` 的 `BossUserManagementRest` |

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|---------|
| `saveOrUpdate` | `@RequestBody BossUserManagementReq request` | `ApiResult<String>` | 委派 `BossUserManagementAppService#saveOrUpdate` [据推断] |
| `updateUserStatus` | `@PathVariable String userId, @RequestBody BossUserStatusReq request` | `ApiResult<BossUserStatusVO>` | 委派 [据推断] |
| `deleteUser` | `@PathVariable String userId` | `ApiResult<BossUserStatusVO>` | 委派 [据推断] |
| `getUserDetail` | `@PathVariable String userId` | `ApiResult<BossUserManagementVO>` | 委派 [据推断] |
| `getUserList` | `@RequestBody PageRequest<BossUserApiPageRequest> pageRequest` | `ApiResult<BossUserApiPagedModels>` | 委派 [据推断] |
| `getUserOptions` | 无 | `ApiResult<List<BossUserOptionVO>>` | 委派 [据推断] |

#### BossMenuRestImpl / BossRoleRestImpl / BossMenuIconRestImpl

> 结构与 BossUserManagementRestImpl 类似；分别委派给 BossMenuAppService / BossRoleAppService / BossMenuIconAppService。具体方法待探查 [据推断]。

---

## 6. BFF 特有约束（🔴 编码必读）[已确认]

> 🆕 BFF 与 Service 工程的约束**完全不同**。主体 §6 是项目级约束；本节只列本工程**特有**的约束（与 boss-user 多模块 Service 工程对比）。

### 6.1 🔴 BFF 5 条红线（违反即阻断）

| # | 红线 | 描述 | 静态扫描命令 |
|---|------|------|------------|
| 1 | **BFF 禁直连 DB/Redis/Kafka** | BFF 是聚合层，所有数据操作走下游 SPI Service | `grep -E "mybatis-plus\|mysql-connector\|@Repository\|@Mapper" -r --include="*.java"` → 期望输出为空 |
| 2 | **BFF Feign Client 必 extends SPI 接口** | 6 个 Feign Client 必须 extends 对应 SPI 服务接口 | `grep "@FeignClient" infrastructure/feign/*.java` → 每个 Client 都有 `extends` |
| 3 | **BFF AppService 必经 Facade 调 Feign** | 不允许 `@Autowired XxxClient` 直接用 | `grep "@Autowired.*Client" application/appservice/*.java` → 期望输出为空 |
| 4 | **BFF 必用 TokenService 禁 AccessUserInfoContext** | 取当前用户必用 `TokenService.getCurUserId()` | `grep "AccessUserInfoContext" -r` → 期望输出为空 |
| 5 | **Facade 异常降级返回 null/空集合/Result.error** | Facade 实现异常时**不允许抛**给 AppService | 人工 review Facade 实现 catch 块 |

### 6.2 BFF 工程特定静态扫描（🔴 编码完成必跑）

```bash
# 1. BFF 禁直连 DB
grep -rn "mybatis-plus\|mysql-connector\|@Repository\|@Mapper" \
  --include="*.java" src/main/java/ | grep -v "feign/"
# 期望输出为空

# 2. BFF Feign Client 必 extends SPI
for f in src/main/java/com/casstime/cloud/boss/bff/user/infrastructure/feign/*.java; do
  if ! grep -q "extends" "$f"; then
    echo "❌ $f 缺少 extends 关键字"
  fi
done
# 期望所有 .java 都有 extends

# 3. BFF AppService 必经 Facade 调 Feign（不允许直接注入 Client）
grep -rn "@Autowired.*Client" --include="*.java" src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/
# 期望输出为空

# 4. BFF 禁用 AccessUserInfoContext
grep -rn "AccessUserInfoContext" --include="*.java" src/main/java/
# 期望输出为空

# 5. BFF 必用 TokenService
grep -rn "TokenService" --include="*.java" src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/
# 期望至少 4 个 AppService 都注入 TokenService

# 6. OperationLog 注解使用检查（操作日志埋点必加）
grep -rn "@OperationLog" --include="*.java" src/main/java/com/casstime/cloud/boss/bff/user/interfaces/
# 期望 createMenu / updateMenu / deleteMenu / createRole / updateRole / deleteRole / bindMenu 等写操作都有 @OperationLog

# 7. BFF AppService 全部返回 ApiResult<T>
grep -rn "public.*ApiResult<" --include="*.java" src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/
# 期望所有方法返回 ApiResult

# 8. lombok @Data / @RequiredArgsConstructor 检查
grep -rn "@Data\|@RequiredArgsConstructor" --include="*.java" src/main/java/com/casstime/cloud/boss/bff/user/application/appservice/
# 期望所有 AppService 类都有 @RequiredArgsConstructor
```

### 6.3 与 boss-user Service 工程的差异（重点对比）

| 维度 | BFF（boss-user-bff）| Service（boss-user）|
|------|-------------------|---------------------|
| 工程结构 | 🆕 单模块 jar | 5 模块 pom 聚合 |
| 分层 | 🆕 BFF 5 层轻量（接口/应用/防腐/领域/基础设施）| 完整 DDD 4 层 + 启动层 |
| 持久化 | ❌ 无（禁直连 DB）| ✅ MyBatis-Plus + HikariCP + Redis |
| 数据源 | ❌ 无 | ✅ MySQL 8.0.17 + HikariCP |
| Feign Client | 🆕 6 个（必 extends SPI）| 1 个（CsUserClient）|
| 操作日志 | 🆕 3 个 OperationLoggable + 1 个 OperatorInfoProvider（冰山特色）| 无 |
| 异常处理 | 🆕 GlobalExceptionHandler（8 个 handler）| 通过下游 Service 返回 |
| AppService 数量 | 4 个（34 方法）| 1 个（推断 ≥6 方法）|
| 主启动类 | [待确认] | `Bootstrap.java` |
| 版本号 | `1.1`（发布版）| `1.0-SNAPSHOT`（开发版）|
| spring-boot-maven-plugin | `2.3.5.RELEASE`（旧）| `2.7.5`（新）|
| lombok | `1.16.20`（旧）| `1.18.16`（新）|
| icec-cloud-boss-security | `2.0.6`（直接依赖）| 传递依赖 |

---

## 7. 上下游契约 [已确认/据推断]

### 7.1 对外暴露（Controller）

| 接口名 | URL | 文档 |
|--------|-----|------|
| `BossUserManagementRest` | `/boss-user-bff/user/...` | `icec-cloud-boss-user-bff-api/` |
| `BossMenuRest` | `/boss-user-bff/menu/...` | 同上 |
| `BossRoleRest` | `/boss-user-bff/role/...` | 同上 |
| `BossMenuIconRest` | `/boss-user-bff/menu-icon/...` | 同上 |

### 7.2 对内消费（Feign Client 调下游 SPI）

| Feign Client | 下游 SPI | 服务 | 业务 |
|--------------|---------|------|------|
| `BossUserManagementClient` | `BossUserManagementService` | `boss-user-service` | 用户管理增删改查 |
| `BossUserInfoClient` | `BossUserInfoService` | `boss-user-service` | 用户信息查询 |
| `BossRoleClient` | `BossRoleService` | `boss-user-service` | 角色管理 |
| `BossMenuClient` | `BossMenuService` | `boss-user-service` | 菜单管理 |
| `BossMenuIconClient` | `BossMenuIconService` | `boss-user-service` | 菜单图标 |
| `BossOperationLogClient` | `OperationLogService` | `boss-log-service` | 操作日志写入 |

### 7.3 操作日志触发点（🆕 BFF 独有）

| 操作 | 触发 OperationLoggable | 日志内容 |
|------|----------------------|---------|
| 角色绑定菜单（`BossRoleAppService#bindMenu`）| `RoleMenuOperationLoggable` | 🔴 「**新增权限:[…]，移除权限:[…]**」或「权限无变化」 |
| 角色 CRUD（`BossRoleAppService#createRole/updateRole/deleteRole`）| `RoleOperationLoggable` | 「新增/修改/删除角色[名称]」 |
| 角色从属关系保存（`BossRoleAppService#saveRoleRelations`）| `RoleOperationLoggable` | 「保存角色从属关系」 |
| 菜单 CRUD（`BossMenuAppService#createMenu/updateMenu/deleteMenu`）| `MenuOperationLoggable` | 「新增/修改/删除菜单[名称]」 |

---

## 8. 关键 API 端点清单 [据推断]

| 端点 | 方法 | 用途 | 委派 AppService |
|------|------|------|---------------|
| `/boss-user-bff/user/page` | POST | 分页查询用户列表 | `BossUserManagementAppService#getUserList` |
| `/boss-user-bff/user/detail/{userId}` | GET | 获取用户详情 | `BossUserManagementAppService#getUserDetail` |
| `/boss-user-bff/user/save` | POST | 新增/更新用户 | `BossUserManagementAppService#saveOrUpdate` |
| `/boss-user-bff/user/status/{userId}` | PUT | 更新用户状态 | `BossUserManagementAppService#updateUserStatus` |
| `/boss-user-bff/user/delete/{userId}` | DELETE | 删除用户 | `BossUserManagementAppService#deleteUser` |
| `/boss-user-bff/user/options` | GET | 用户下拉列表（手机号脱敏）| `BossUserManagementAppService#getUserOptions` |
| `/boss-user-bff/menu/page` | POST | 分页查询菜单 | `BossMenuAppService#getMenuList` |
| `/boss-user-bff/menu/all` | GET | 所有菜单 | `BossMenuAppService#getMenuAll` |
| `/boss-user-bff/menu/tree` | GET | 当前用户菜单树 | `BossMenuAppService#getRoleMenuTree` |
| `/boss-user-bff/menu/perms` | GET | 当前用户权限标识 | `BossMenuAppService#getPermsByUserId` |
| `/boss-user-bff/role/page` | POST | 分页查询角色 | `BossRoleAppService#getRoleList` |
| `/boss-user-bff/role/{id}` | GET | 角色详情 | `BossRoleAppService#getRoleById` |
| `/boss-user-bff/role/bind-menu` | POST | 角色绑定菜单 | `BossRoleAppService#bindMenu` |
| `/boss-user-bff/role/save-relations/{roleId}` | POST | 保存角色从属关系 | `BossRoleAppService#saveRoleRelations` |

---

## 9. 本工程缺口与待补充

| # | 缺口 | 优先级 | 状态 |
|---|------|-------|------|
| 1 | 主启动类（`Bootstrap.java` / `WebApplication.java`）实际位置 | 🟠 P1 | 待补 |
| 2 | `application/facade/` 下的具体接口与实现（4 个 AppService 各对应一个 Facade）| 🟠 P1 | 待补 |
| 3 | `Converter` 完整方法列表（仅推断 `BossUserManagementConverter#toUserPageRequest`）| 🟡 P2 | 待补 |
| 4 | 4 个 RestImpl 完整方法级实现（仅推断 BossUserManagementRestImpl）| 🟠 P1 | 待补 |
| 5 | Domain 层实际内容（BFF 域层薄弱，是否包含 VO / Enum）| 🟡 P2 | 待补 |
| 6 | 管理端口（`management.port`）实际配置 | 🟢 P3 | 待补 |
| 7 | 单元测试覆盖率（同事 boss-user-bff.md §1.6 标注未配置 JaCoCo）| 🟡 P2 | 待补 |
| 8 | `@OperationLog` 注解在 RestImpl 上的实际触发点 | 🟡 P2 | 待补 |
| 9 | Redis 实际使用场景（JWT 会话？缓存？）| 🟡 P2 | 待补 |
| 10 | Facade 层异常降级实际写法（catch 块怎么写）| 🟡 P2 | 待补 |

---

## §A 关键词反向索引 [已确认/据推断]

| 关键词 | 出现位置 |
|--------|---------|
| `BossMenuIconAppService` | §4 / §5.2（5 方法）|
| `BossMenuAppService` | §4 / §5.2（10 方法）|
| `BossRoleAppService` | §4 / §5.2（13 方法 + `bindMenu`）|
| `BossUserManagementAppService` | §4 / §5.2（6 方法 + `maskCellphone`）|
| `BossUserManagementConverter` | §5.1 |
| `RoleMenuOperationLoggable` | §4 / §5.4（"新增权限:[…]，移除权限:[…]"）|
| `RoleOperationLoggable` | §4 / §5.4（角色 CRUD）|
| `MenuOperationLoggable` | §4 / §5.4（菜单 CRUD）|
| `BossOperatorInfoProvider` | §4 / §5.4（TokenService → userId → 用户名）|
| `BossUserManagementClient` | §4 / §5.4（extends SPI）|
| `BossUserInfoClient` | §4 / §5.4（extends SPI）|
| `BossRoleClient` / `BossMenuClient` / `BossMenuIconClient` / `BossOperationLogClient` | §4 / §5.4（extends SPI）|
| `GlobalExceptionHandler` | §4 / §5.4（8 个 handler 方法）|
| `UnAuthorizeException` | §4 / §5.4 |
| `TokenService.getCurUserId` | §5.2（4 个 AppService 都注入）|
| `maskCellphone` | §5.2（手机号脱敏：保留后 4 位）|
| `getRoleMenuTree` / `getPermsByUserId` | §5.2（基于当前用户 ID）|
| `RoleMenuLogData` / `RoleLogData` / `MenuLogData` | §5.4（内部数据载体）|
| `OperationLoggable` / `OperatorInfoProvider` | §5.4（接口）|
| `OperationLogTypeEnum` | §5.4（CREATE/UPDATE/DELETE）|
| `@OperationLog`（注解）| §6.2 / §7.3 |
| `Feign Client 必 extends SPI` | §5.4 / §6.1 |
| `BFF 禁直连 DB` | §1.2 / §6.1 / §6.2 |
| `icec-cloud-boss-security 2.0.6` | §3 |
| `spring-boot-maven-plugin 2.3.5.RELEASE` | §3（旧版）|
| `lombok 1.16.20` | §3（旧版）|

---

## §B 本工程更新日志（合并到主体 update-log）

> 详见主体 `icec-cloud-boss.update-log.md`；本节列本工程特有的变更摘要。

| 日期 | 变更摘要 |
|------|---------|
| 2026-06-26 | 🆕 按 schema v3 §15 首次生成工程级子文件；来源：同事 boss-user-bff.md（72KB / 1232 行）+ ae-sdd boss.assets.md §4 |