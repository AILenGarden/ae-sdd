---
name: project-assets-schema
description: 项目资产目录标准 — Coding-SKILL ④bis CodePlan 必引用的"项目代码世界地图"。规范项目资产的结构、探查 SOP、跨项目复用方式。每个新项目必须先构建一份项目资产再进入 Code Plan 阶段。
---

# Project Assets Schema — 项目资产目录标准

> **本文件是 Coding-SKILL ④bis 阶段的"项目真相源"。** Code Plan 编写前必须先有项目资产；无项目资产 → 走 §9 探查 SOP 构建 → 再进入 Code Plan。

---

## 0. 摘要与使用场景

| 维度 | 内容 |
|------|------|
| 何时需要查 | ④bis 编写 Code Plan / ⑤ Coding 落实施工 / ⑦ Code Review 对照时 |
| 谁负责写 | Story 阶段的 AI/开发者；新项目启动 1 周内完成首版；每月审计一次 |
| 与 `constraints/` 的关系 | constraints/ 是规则层（团队/项目无关的"怎么写代码"），项目资产是事实层（"这个项目里有什么模块/包路径/类名"） |
| 关键不变量 | **本文件不重复定义 rules**，只把 rules 映射到本项目代码；rules 改动在 constraints/，事实改动在 project-assets |

---

## 1. 项目资产元信息

| 字段 | 必填 | 说明 |
|------|------|------|
| projectKey | ✅ | 短横线串，如 `icec-cloud-boss` |
| projectName | ✅ | 全名 |
| gitPath | ✅ | Git 仓库根路径（🆕 2026-06-10 强化为下游定位锚点，详见下方） |
| productLine | ⚠️ | 产品线（如 `boss` / `life`） |
| profile | ⚠️ | Spring Profile 列表 |
| mainClass | ⚠️ | 启动类路径 |
| packaging | ⚠️ | jar / war |
| portRange | ⚠️ | 端口区间（用于规划新模块端口） |
| lastAuditedAt | ✅ | 最后审计时间（YYYY-MM-DD） |
| owner | ✅ | 维护负责人 |

### 1.2 Meta.部署信息（🆕 2026-06-26）

> **🆕 升级背景**：原 §1 只列 portRange，每个工程的部署细节（profile / 数据源 / Redis / 网关 / 镜像仓库 / 私服 / JaCoCo）散落在各工程 bootstrap.yml，靠 grep 拼凑。**从 coder "接手第一个本地服务" 到 "跑通" 平均浪费 1-2 小时**——所以升级为强制结构化记录。

| 字段 | 必填 | 说明 | 抽取命令 |
|------|------|------|---------|
| `profile.active` | ✅ | 当前激活的 Spring Profile（如 `beta-kunlun`）| `grep "spring.profiles.active" */bootstrap.yml` |
| `db.urlTemplate` | ✅ | 数据源 URL 模板（含占位符 `${icec.database.servers}` 等）| `grep "jdbc:mysql" */bootstrap.yml` |
| `db.pool` | ⚠️ | 连接池配置（HikariCP max-active / min-idle / timeout）| `grep -A 3 "hikari" */bootstrap.yml` |
| `redis.address` | ✅ | Redis 地址（生产/测试分开）| `grep "spring.redis" */bootstrap.yml` |
| `redis.password.inConfig` | 🔴 | 是否在配置文件中明文写密码 | `grep -E "redis.*password" */bootstrap.yml` |
| `gateway` | ✅ | 网关/代理 URL（如 `icec.api.agent`）| `grep "icec.api" */bootstrap.yml` |
| `imageRepo` | ✅ | 容器镜像仓库（如 `registry.cn-shenzhen.aliyuncs.com/cassmall/`）| `grep "docker.image` */pom.xml` |
| `nexusRepo` | ⚠️ | 私服仓库地址 | `grep "cass-public" pom.xml` |
| `management.port` | ⚠️ | Actuator 端口（防止本地多服务启动冲突）| `grep "management.port" */bootstrap.yml` |
| `coverageTool` | ⚠️ | 覆盖率插件（JaCoCo 等）| `grep "jacoco" pom.xml` |

**字段判例（boss-user-service）：**

```yaml
deployment:
  profile.active: beta-kunlun
  db.urlTemplate: "jdbc:mysql://${icec.database.servers}/${icec.database.dbname}?useUnicode=true&characterEncoding=utf-8&allowMultiQueries=true&autoReconnect=true"
  db.pool:
    type: HikariCP
    max-active: 20
    min-idle: 1
    timeout: 30s
  redis.address: "redis-...dcs.huaweicloud.com:6379"
  redis.password.inConfig: false  # 🔴 若 true → 安全提示
  gateway: "http://life-hwbeta-api-penglai.intra.casstime.com"
  imageRepo: "registry.cn-shenzhen.aliyuncs.com/cassmall/boss-user-service:1.0-SNAPSHOT"
  nexusRepo: "http://dev.casstime.com/nexus/content/groups/public/"
  management.port: 30000  # 默认值；显式配置时填实际值
  coverageTool: jacoco
```

**🔴 安全提示触发条件**（自动写入 §14）：
- `redis.password.inConfig: true` → "bootstrap.yml 中 Redis 明文密码直接写入配置文件，建议改为占位符外部注入"
- `management.port` 缺省 30000 且无 auth → "Actuator 端点可能外露，需配置 `management.endpoints.web.exposure.include` 白名单"

---

### 1.1 Meta.gitPath 字段强化约束（🆕 2026-06-10）

**下游消费：** `document-storage-skill` §0.5/§0.6 强依赖此字段作为"项目根"事实基线。

**校验规则：**
- **必填：** ✅
- **类型：** string（绝对路径，Windows 兼容反斜杠）
- **路径格式：**
  - 必须是**绝对路径**（不以 `./` 或 `../` 开头）
  - 必须是文件系统可达的目录
  - 必须是 git 仓库根（包含 `.git/` 子目录）
  - 路径分隔符：Windows 用 `\`（如 `d:\Item\icec-cloud-boss`），Linux/Mac 用 `/`（如 `/home/user/projects/icec-cloud-boss`）

**派生用途：**
- 项目级定位：所有"项目根"开头的路径（如 `design/` 前缀、`{项目根}/.auto-engineering/`）从此派生
- 微服务级定位：微服务根 = `{gitPath} + "/" + {microservices[].name}`（拼接约定）
- CodingSkill 写代码时定位工程根（`mvn compile` 等命令的工作目录）

**示例：**
- ✅ `d:\Item\icec-cloud-boss`
- ✅ `d:\\Item\\icec-cloud-boss`（JSON 转义）
- ✅ `/home/user/projects/icec-cloud-boss`
- ❌ `./` 或 `../`（相对路径）
- ❌ `D:\Item\icec-cloud-boss\icec-cloud-boss-user-service`（子工程路径，不是仓库根）

---

## 2. 微服务清单

| 字段 | 必填 | 说明 |
|------|------|------|
| name | ✅ | 模块名 |
| responsibility | ✅ | 业务职责（一句话） |
| port | ⚠️ | server.port（如能从 bootstrap.yml 读出） |
| contextPath | ⚠️ | Servlet 上下文路径 |
| hasBff | ✅ | 是否有 BFF 层 |
| callChain | ✅ | api→bff→spi→service 链中的位置 |
| dependsOnSpi | ⚠️ | 依赖的 SPI 子模块列表 |

**判例：**
```yaml
- name: icec-cloud-boss-user
  responsibility: Boss 用户域核心业务（用户/角色/菜单/菜单图标）
  port: 12004
  contextPath: /boss-user
  hasBff: true
  callChain: service（DDD 四层）+ Feign 调用 icec-cloud-life-cs SPI
  dependsOnSpi:
    - icec-cloud-life-cs-spi
```

---

## 3. 抽象分层 → 项目分层映射（粗粒度）

> **行 = 抽象 4 层 + 可选 2 类，列 = 本项目对应的工程模块。**
> **与 §4 关系**：本表是粗粒度（抽象层 × 工程模块），§4 是细粒度（DDD 分层 × 精确包路径）。§3 引用 §4 避免重复。

| 抽象层 | 含义 | 本项目对应工程模块 | 备注 |
|--------|------|------------------|------|
| 请求处理（Interfaces） | 控制器/请求入口 | icec-cloud-boss-user/icec-cloud-boss-user-interfaces | 不含 BFF 入口 |
| 业务编排（Application） | AppService/编排逻辑 | icec-cloud-boss-user/icec-cloud-boss-user-application | 事务在 AppService |
| 领域逻辑（Domain） | DO/聚合根/不变量 | icec-cloud-boss-user/icec-cloud-boss-user-domain | 充血模型 |
| 基础能力（Infrastructure） | PO/Mapper/Converter | icec-cloud-boss-user/icec-cloud-boss-user-infrastructure | 仅存取 |
| 跨模块 SPI（可选） | Feign 接口/契约 | icec-cloud-life-spi/icec-cloud-boss-user-spi | ServiceProviderConstants |
| BFF 入口（可选） | BFF 控制器 | icec-cloud-boss-user-bff | 仅当 hasBff=true |

---

## 4. DDD 内部分层落点（细粒度）

> **行 = 类角色，列 = 精确包路径模板。** 这是项目资产"最核心的可复用部分"。

| 类角色 | 精确包路径 | 典型类名 | 放什么 / 不放什么 |
|--------|-----------|---------|------------------|
| **Interfaces** | | | |
| Rest 实现类 | `interfaces/restful/` | `{Resource}RestImpl implements {SpiInterface}` | 仅做协议适配，不写业务规则 |
| Event Handler | `interfaces/eventhandlers/` | `{Resource}EventHandler` | 事件接入入口 |
| **Application** | | | |
| AppService | `application/appservice/` | `{Resource}AppService` | 事务、编排、调 Domain 顺序 |
| Converter | `application/converter/` | `{Resource}Converter` | `@UtilityClass` 静态方法，DO↔DTO |
| Publisher | `application/publisher/` | `{Resource}Publisher` | 跨域事件发布 |
| Command/Query VO | `application/vo/command\|query/` | `{Resource}Command / {Resource}Query` | 写/读命令对象 |
| **Domain** | | | |
| Domain Object | `domain/{业务域}/model/entity/` | `{Resource}DO` | 充血，业务方法不以 get/set 开头 |
| Value Object | `domain/{业务域}/model/value/` | `{Resource}Query / {Resource}Context` | 不可变值对象 |
| Enum | `domain/{业务域}/model/enums/` | `{Resource}MessageEnum` | (key, value) 双字段 |
| Error Enum | `domain/{业务域}/model/enums/error/` | `{Resource}ErrorCode` | 错误码枚举 |
| Repository 接口 | `domain/{业务域}/repository/` | `{Resource}Repository` | 仅接口，**不允许放业务规则** |
| Domain Service | `domain/{业务域}/service/` | `{Resource}DomainService` | 跨聚合业务规则 |
| Event | `domain/{业务域}/event/` | `{Resource}CreatedEvent` | 领域事件定义 |
| Exception | `domain/{业务域}/exception/` | `{Resource}DomainException` | 领域异常 |
| Facade | `domain/{业务域}/facade/` | `{Resource}Facade` | 跨域服务抽象接口 |
| **Infrastructure** | | | |
| Config | `infrastructure/config/` | `MybatisPlusConfig / RedisConfig` | Spring 配置 |
| Feign Client | `infrastructure/feign/` | `{Resource}Client extends {SpiService}` | 调外部服务 |
| PO | `infrastructure/persistence/entity/` | `{Resource}PO @TableName("xxx")` | 贫血，对应表 |
| Mapper | `infrastructure/persistence/dao/mapper/` | `{Resource}Mapper extends BaseMapper<{Resource}PO>` | MyBatis 映射 |
| DataConverter | `infrastructure/persistence/converter/` | `{Resource}DataConverter` | PO↔DO |
| Repository Impl | `infrastructure/persistence/repository/mysql/` | `{Resource}RepositoryImpl extends ServiceImpl<...>` | 实现 domain 接口 |
| Redis Repository | `infrastructure/persistence/repository/redis/` | `{Resource}RedisRepository` | 缓存实现 |
| Facade Impl | `infrastructure/facade/` | `{Resource}FacadeImpl` | 异常时返回 null/空集合/Result.error |
| **SPI** | | | |
| SPI Service 接口 | `spi/{user, cs, im}/service/` | `{Resource}Service / {Resource}ManagementService` | Feign 接口 |
| DTO | `spi/{user, cs, im}/dto/` | `{Resource}DTO` | 跨服务传输 |
| Request | `spi/{user, cs, im}/request/` | `{Resource}Request` | 请求对象 |
| Constants | `spi/{user, cs, im}/` | `ServiceProviderConstants` | Feign 服务名常量 |
| **BFF** | | | |
| Rest Impl | `bff/interfaces/restful/` | `{Resource}RestImpl implements *Rest` | BFF 控制器 |
| AppService | `bff/application/appservice/` | `{Resource}{Action}AppService` | BFF 编排 |
| Feign Client | `bff/infrastructure/feign/` | `{Resource}Client extends *Service` | 调 Service SPI |
| OperationLog Capability | `bff/infrastructure/operationlog/capability/` | `{Resource}OperationLoggable` | 操作日志能力 |

---

## 5. 命名约定

| 对象 | 命名模板 | 例子 | 反例 |
|------|---------|------|------|
| Controller (BFF) | `{Resource}RestImpl` | `BossUserManagementRestImpl` | `UserController`（禁） |
| AppService (BFF) | `{Resource}{Action}AppService` | `BossUserManagementAppService` | `UserService`（歧义） |
| AppService (Service) | `{Resource}AppService` | `BossUserAppService` | `BossUserManager`（动名词禁） |
| Domain Object | `{Resource}DO` | `BossUserDO` | `BossUser`（缺 DO 后缀） |
| Persistent Object | `{Resource}PO` | `BossUserPO` | `BossUserEntity`（禁） |
| Repository | `{Resource}Repository` | `BossUserRepository` | `BossUserDao`（禁） |
| Repository Impl | `{Resource}RepositoryImpl` | `BossUserRepositoryImpl` | — |
| Converter (Application) | `{Resource}Converter` | `BossUserConverter` | — |
| Data Converter (Infra) | `{Resource}DataConverter` | `BossUserDataConverter` | — |
| Feign Client | `{Resource}{Action}Client` | `BossUserManagementClient` | — |
| Error Code | `{Resource}ErrorCode` | `BossUserErrorCode` | — |
| Domain Exception | `{Resource}DomainException` | `BossDomainException` | — |

**反例汇总：**
- ❌ 用 LocalDateTime → ✅ 用 java.util.Date
- ❌ 用 MapStruct → ✅ 显式 Converter
- ❌ 用 `@Scheduled` → ✅ job-spring-boot-starter
- ❌ `BossUserManager` / `BossUserHandler`（动名词/动名词）→ ✅ `BossUserAppService`
- ❌ `private` 字段无 `@Getter` → ✅ `@Data`

---

## 6. 工程约束（继承自 constraints/，按本项目裁剪+补缺）

> **本节不重复定义 rules**，只把 rules 映射到本项目代码。规则原版在 `document/life-team-ai-standards/constraints/`。

### 6.1 分层架构（constraints/layered-arch.md 的本项目落点）

- 外部请求 → API（聚合工程）→ BFF → SPI（Feign）→ Service（DDD 四层）
- BFF **禁止**直接操作 DB/Redis/Kafka
- Service **禁止**直连前端
- Service 间同步调用必须走 SPI（Feign）
- **禁止**跨 Service 直连数据库

### 6.2 工程结构（constraints/project-structure.md 的本项目落点）

- 业务规则 → Domain；协调谁调谁 → Application；存取数据 → Repository
- Repository 方法名仅 `findByXxx / save / update / updateStatus`
- 对象类型：Domain 仅 DO（充血），Interfaces 仅 DTO（无 DO/PO）

### 6.3 代码风格（constraints/code-style.md 的本项目落点）

- 时间统一 `java.util.Date`（禁 LocalDateTime）
- JSON 用 `com.casstime.commons.utils.JsonUtils`
- 枚举统一 `(key, value)` 双字段
- 事务增删改在 AppService 用 `@Transactional`（查询不开）
- **事务内禁止**远程调用/MQ 发送
- Feign Client 必须 extends SPI 接口
- BFF AppService **必须经** Facade 调 Feign，Facade 异常返回 null/空集合/Result.error

### 6.4 接口规范（constraints/api.md 的本项目落点）

- URL 小写连字符，名词单数，公开 `/public/` 前缀
- 分页 POST + RequestBody，统一 `PageRequest<T>` 包装
- 状态变更 PUT，幂等写入用 PUT
- 返回值：BFF 用 `ApiResult<T>`；Service 需分支判断用 `Result<T>`；Controller 内部方法不包装
- 错误码 5 位分段（认证 10000-10999、用户 11000-11999、车辆 12000-12999、工单 13000-13999，**尚未统一划分**）
- 删除优先逻辑删除，物理删除需在 DR 说明
- HTTP 状态码统一 200，错误经 code/message 表达
- Swagger 注解必加

### 6.5 数据库规范（constraints/database.md 的本项目落点）

- 库名 = 服务名；utf8mb4/utf8mb4_0900_ai_ci；Service 独占库
- 必备字段：`id / created_by / created_date / last_updated_by / last_updated_date`
- 主键：`bigint AUTO_INCREMENT` 或 `varchar(32)` 业务单号
- 表名/字段名小写下划线单数
- 单表索引 ≤ 5；varchar 索引指定长度（一般 20）
- 禁外键级联；IN 集合 ≤ 1000；`#{}` 参数化（禁 `${}`）；禁 `SELECT *`
- 审计四字段 + `deleted_flag TINYINT(1) DEFAULT 0` 选型

### 6.6 安全规范（constraints/security.md 的本项目落点）

- JWT 经 Cookie `security_context` 传递
- 默认需鉴权（`@SkipAuth` 显式跳过）
- 方法级权限：`@RequiresPermissions("xxx:xxx")`
- `superManager=1` 跳权限码
- 手机号脱敏：`DesensitizeUtils.handleCellphone`
- 密码 BCrypt 单向哈希：`BCryptUtil.matches`
- 日志禁打密码/手机号/Token

### 6.7 测试规范（constraints/testing.md 的本项目落点）

- Controller 测试真实 HTTP（`@SpringBootTest(webEnvironment = RANDOM_PORT) + TestRestTemplate`），MockMvc 仅框架过老时降级
- Service 单测 JUnit + Mockito，**禁真实** DB/远程
- 集成测试用开发库 + `@Transactional + @Rollback`
- 覆盖率：整体 ≥ 60% / Service 核心 ≥ 70% / Mapper XML 自定义 SQL ≥ 60% / Controller 接口 ≥ 50%
- 断言必须校验业务字段与错误码/错误信息
- 核心落库路径必须真实 DB 验证
- 静态分析 Checkstyle + SpotBugs（P0 阻断）

### 6.8 完整技术栈版本号表（constraints/technology-stack.md 的本项目落点）

> **🆕 2026-06-26 升级**：原 §6.8 "技术栈范围"只列主框架，缺内部 starter / 工具库 / 测试框架版本号 → 升级为"完整技术栈版本号表"。**任何新增/升级依赖必须先更新本表**。

#### 6.8.1 主框架与运行时

| 组件 | 版本 | 备注 |
|------|------|------|
| Java | 8 | 主版本（禁止 Java 11+ 新特性）|
| Spring Boot | 1.5.7.RELEASE | 应用基础框架 |
| Spring Cloud | Dalston.SR4 | 服务治理 / 配置 / Feign |
| MyBatis-Plus | 3.3.2 | ORM；Mapper XML 路径 `classpath*:com/.../*.xml` |
| PageHelper | 5.2.1 | 物理分页 |
| MySQL Connector/J | 8.0.17 | 驱动（`com.mysql.cj.jdbc.Driver`）|
| HikariCP | 2.7.9 | 连接池（max-active=20 / min-idle=1 / timeout=30s）|
| Redis | spring-boot-starter-data-redis | database 0，pool max-active 50 |
| Elasticsearch | 7.10.2 | 搜索（仅 life 域用）|

#### 6.8.2 工具库与映射

| 组件 | 版本 | 备注 |
|------|------|------|
| MapStruct | 1.5.3.Final | DTO/PO 映射（仅引用，实际用显式 Converter）|
| Lombok | 1.18.16 | `@Data @RequiredArgsConstructor @Slf4j` |
| Swagger2 | 2.8.0 | 接口文档（必加）|
| commons-lang3 | 3.9 | 字符串/通用工具（domain 层）|
| commons-collections4 | 4.0 | 集合工具（domain 层）|
| Caffeine | （跟随）| 本地缓存（与 Redis 配合）|
| JSON | `com.casstime.commons.utils.JsonUtils` | 统一 JSON 序列化 |
| 日期类型 | `java.util.Date` | 禁 LocalDateTime（防序列化踩坑）|

#### 6.8.3 安全与加解密

| 组件 | 版本 | 备注 |
|------|------|------|
| spring-security-crypto | 跟随 Spring Boot | 密码 BCrypt（`BCryptUtil.matches`）|
| DesensitizeUtils | `com.casstime.commons.utils` | 手机号脱敏 |
| JWT 鉴权 | Cookie `security_context` | 默认需鉴权，`@SkipAuth` 显式跳过 |

#### 6.8.4 内部基础组件（公司 starter，🔴 必须用，禁止直配）

| 组件 | 版本 | 用途 | 替代 |
|------|------|------|------|
| panda-spring-boot-starter | 1.0.9 | 配置中心（`panda.casstime.com`）| 禁直连 Nacos |
| casslog-spring-boot-starter | 1.5.0 | 日志组件 | 禁直配 logback/log4j |
| cassmetrics-spring-boot-v1-starter | 1.0.4 | 监控指标 | 禁自造 metrics |
| cass-config-spring-boot-starter | 1.1.2 | 内部配置组件 | — |
| job-spring-boot-starter | （跟随）| 定时任务 | 禁 `@Scheduled` |
| courier-spring-boot-starter | 3.3-SNAPSHOT | 领域事件 / MQ 投递 | Kafka 必须经 courier |

#### 6.8.5 测试框架与覆盖率要求

| 组件 | 版本 | 用途 |
|------|------|------|
| JUnit | 4.12 | 单元测试 |
| Mockito | 1.10.19 | Mock 框架（domain 层）|
| surefire | 2.22.2 | Maven 测试插件 |
| JaCoCo | （跟随）| 覆盖率插件（target/jacoco.exec）|

**覆盖率硬指标（🔴 阻断）：**
- 整体 ≥ 60%
- Service 核心 ≥ 70%
- Mapper XML 自定义 SQL ≥ 60%
- Controller 接口 ≥ 50%

#### 6.8.6 构建与镜像

| 组件 | 版本 | 用途 |
|------|------|------|
| spring-boot-maven-plugin | 2.7.5 | Fat JAR 打包（repackage）|
| dockerfile-maven-plugin | 1.4.0 | Docker 镜像构建 |
| 私服仓库 | `http://dev.casstime.com/nexus/content/groups/public/` | cass-public |
| 镜像仓库 | `registry.cn-shenzhen.aliyuncs.com/cassmall/` | 阿里云 |

#### 6.8.7 静态分析与门禁工具

| 组件 | 用途 | 门禁等级 |
|------|------|---------|
| Checkstyle | 代码风格 | 🔴 P0 阻断 |
| SpotBugs | Bug 模式 | 🔴 P0 阻断 |
| 工程特定扫描 | 见 §6.11 | 🔴 P0 阻断 |

### 6.9 隐性约定（constraints/implicit-constraints.md 的本项目补缺）

> 当前 constraints/implicit-constraints.md 为空（仅占位）。Code Plan 编写时若发现"项目内大家知道但没人写下来"的约定，应主动提议补充到该文件，并在本节列出补充项。

| 约定名 | 描述 | 出处 / 踩坑 Story | 反向链接 |
|--------|------|------------------|---------|
| 互踢与融云无关 | 坐席 session 互踢基于 Redis session 失效，不依赖融云 SDK | `function/STORY-003-BE-登录认证.md §1.8 关键约束 #2` + `function/登录与认证-BE-接口逻辑速查.md:220-224` | §10.7 并发与幂等模式（分布式锁）|
| 事务中禁止远程调用 | `BossUserAppService#authenticateUser` 含 Feign 调用，不开启 `@Transactional` | `icec-cloud-boss-user-application/.../appservice/BossUserAppService.java:42` | §10.3 Application 层模式（无大事务）|
| Feign 失败处理差异化 | 登录时 Feign 失败直接抛异常返回错误码；用户详情时 Feign 失败不阻断 | `function/STORY-003-BE-登录认证.md §1.8 关键约束 #3` + `function/登录与认证-BE-接口逻辑速查.md:222` | §10.4 Infrastructure 层模式（Feign 封装）|
| 错误码区分账号存在性 | 接受轻微安全代价，"账号不存在"（11101）与"用户名或密码错误"（11105）错误码分离 | `icec-cloud-boss-user-domain/.../enums/error/BossUserErrorCode.java` + `function/STORY-003-BE-登录认证.md §1.8 关键约束 #4` | §10.6 异常处理模式（错误码定义）|

> **强制规则**：每条隐性约定必须有 1 个 Story 踩坑出处或代码出处，并至少反向链接到 §10 团队惯用实现方式、§5 核心类方法或 constraints/ 中的具体条款。不得把无出处的经验写成"已确认"。
>
> **🆕 v3.5.1.1 初始沉淀**：上表 4 条为基于 STORY-003-BE 登录认证与同事知识库 `function/登录与认证-BE-接口逻辑速查.md:220-224` 归纳的初始数据；后续探查发现新约定时按相同格式追加。

---

## 7. 跨服务契约入口

### 7.1 关键事实表

| 字段 | 抽取方法 | 本项目示例 |
|------|---------|-----------|
| Feign 服务名 | `grep "ServiceProviderConstants" icec-cloud-life-spi/ -r` | `icec-cloud-boss-user-spi` 中 `ServiceProviderConstants.BOSS_USER_SERVICE = "boss-user-service"` |
| Nacos 实例名 | `grep "spring.application.name" */bootstrap.yml` | `boss-user-service` |
| 错误码分段现状 | 读 `domain/.../enums/error/*ErrorCode.java` 注释 | 11000-11999 用户段，13000-13999 工单段（**未全统一**） |
| Service 间依赖 | `mvn dependency:tree \| grep -E "icec-cloud-.*-spi"` | boss-user → cs-spi, notification-spi |

### 7.2 契约抽取命令清单

```bash
# Feign 服务名常量
grep -rn "ServiceProviderConstants" icec-cloud-life-spi/ --include="*.java" | head -30

# Nacos 应用名
find . -name "bootstrap*.yml" -o -name "application*.yml" | xargs grep -l "spring.application.name" 2>/dev/null

# 错误码分布
find . -path "*/enums/error/*ErrorCode.java" | head -20

# 跨服务 Feign 客户端
grep -rn "@FeignClient" --include="*.java" | head -30
```

---

## 8. Code Plan 输入索引

> Code Plan 的哪些字段引用本项目资产的哪些章节。

| Code Plan 章节 | 引用本项目资产章节 | 说明 |
|---------------|------------------|------|
| §1 项目资产引用块 | §1-§11 全部 | 文首必引 |
| §2 抽象分层 → 项目分层映射 | §3 / §4 | 包路径/类名模板 |
| §5 关键类骨架 | §4 / §5 / **§10** | 类角色 + 命名约定 + **惯用实现方式** |
| §6 DO 字段对齐 | §6.5 数据库规范 | 审计四字段/类型 |
| §7 Mapper / SQL | §6.5 | EXPLAIN 验证步骤 |
| §8 测试对应 | §6.7 / **§10.8** | 真实 DB/HTTP 判定 + **测试模式** |
| §11 约束合规自审 | §6 全章 | 9 类约束 |
| §12 接口契约一致性 | §6.4 | URL/方法/分页/错误码 |
| **§13 惯用模式对齐** 🆕 | **§10 全章** | **新代码是否复用了经验文档中的惯用模式** |

---

## 9. 探查 SOP（如何"研究并构建"一份新的项目资产）

> **适用：** 新项目启动 / 老项目首次构建项目资产 / 月度审计重建

### 步骤 1：读 CLAUDE.md + AGENTS.md + 项目根 README

**产物：项目资产 §1 元信息 + §3/§5/§6 初稿**

三路输入，优先级：CLAUDE.md > README（信息重叠时以 CLAUDE.md 为准，它由 `/init` 扫描全库生成，更新更及时）

| 输入文件 | 抽取内容 | 写入章节 |
|---|---|---|
| `CLAUDE.md`（`/init` 生成） | Tech Stack（框架/Java版本）→ §6 工程约束初稿 | §6 |
| `CLAUDE.md` | Architecture（调用链/分层结构）→ §3 抽象分层映射初稿 | §3 |
| `CLAUDE.md` | Key Conventions / Naming（类名后缀规则）→ §5 命名约定初稿 | §5 |
| `CLAUDE.md` | Key Files（关键路径）→ §4 典型类名初稿 | §4 |
| `AGENTS.md` | 第 2 节"关键子工程清单" → 微服务清单初稿 | §2 |
| `AGENTS.md` | SPI 依赖关系 → dependsOnSpi 初稿 | §2 |
| `README.md` | 端口/Profile/启动类（CLAUDE.md 未覆盖时补充） | §1 |

**抽取项最小集（§1）：** 项目名/Git 路径/产品线/Profile/启动类/打包方式/端口区间

> **注意：** CLAUDE.md 不存在时回退到 README + 代码扫描；步骤 3-6 变为「验证 + 补充」而非从头抽取。

### 步骤 2：读 constraints/ 目录所有 .md

**产物：项目资产 §6 工程约束初稿**
- 文件名固定 8 个：technology-stack / project-structure / layered-arch / code-style / api / database / security / testing
- 可选：implicit-constraints（默认空，需项目补）
- 抽取项：每文件 ≤ 200 字摘要 + 关键 rule 编号

### 步骤 3：跑 mvn dependency:tree，列模块依赖

**产物：项目资产 §2 微服务清单的"dependsOnSpi"列**
- 过滤规则：只看 `icec-cloud-*-spi` 开头（不关注 transitive）
- 命令：`mvn dependency:tree -pl service-module -Dincludes="com.casstime.cloud:icec-cloud-*-spi" | head -30`
- 输出：每个 Service 依赖的 SPI 子模块列表

### 步骤 4：抽取典型类（每个 DDD 分层 1-2 个）

**产物：项目资产 §4 DDD 内部分层落点的"典型类名"列**
- 判定标准：典型类 = 含 5 类关键方法之一
  - RestImpl：含 `@RestController` + `implements *Service`
  - AppService：含 `@RequiredArgsConstructor` + `@Transactional`
  - DO：含业务方法（非 get/set）
  - PO：含 `@TableName + @TableId`
  - RepositoryImpl：含 `extends ServiceImpl<*,*>` + `implements *Repository`

### 步骤 5：抽取命名约定

**产物：项目资产 §5 命名约定**
- 7 类对象各 5-10 个真实类名 → 抽命名模板
- 至少看 2 个 Service（如 icec-cloud-boss-user 和 icec-cloud-life-cs）+ 1 个 BFF

### 步骤 6：抽取跨服务契约

**产物：项目资产 §7 跨服务契约入口**
- 用 §7.2 命令清单 grep
- 至少 1 个 SPI 子模块（如 icec-cloud-boss-user-spi）的 ServiceProviderConstants 全量

### 步骤 7：填写抽象分层 → 项目分层映射

**产物：项目资产 §3**
- 逐工程模块，对照 §4 写 §3 粗粒度映射
- §3 引用 §4 的精确包路径

### 步骤 8：列 openGaps

**产物：项目资产 §11 缺口**
- 探查中未读出的端口/配置/类
- 隐性约定（项目内"大家都知道但没人写下来"的坑）
- 写入项目资产 §11，并在下次审计时优先补

### 步骤 9：提炼团队惯用实现方式（经验文档）

**产物：项目资产 §10 团队惯用实现方式**

#### 9.1 探查目标

从现有代码中提炼团队**实际在用且符合约束**的惯用实现模式，形成经验文档。

#### 9.2 探查 SOP（6 步）

| 步骤 | 动作 | 命令/方法 | 产物 |
|------|------|---------|------|
| 9.2.1 | 选取样本服务（≥2 个成熟服务） | 优先选"代码行数最多 + 维护时间最久"的服务 | 样本服务清单 |
| 9.2.2 | 逐层扫描典型实现（每层 ≥3 个文件） | 按 §4 DDD 分层，每层 Read 3-5 个完整类文件 | 原始代码片段集 |
| 9.2.3 | 交叉验证（同一模式 ≥2 处使用） | grep 关键方法签名/类结构模式，确认非孤例 | 使用频次统计 |
| 9.2.4 | 约束过滤（逐条对照 constraints/） | 对照 8 个 constraints/ 文件，剔除违规写法 | 合规模式清单 |
| 9.2.5 | 提炼为可复用骨架（去业务化） | 抽象为 ≤30 行代码骨架 + 场景说明 | 经验条目草稿 |
| 9.2.6 | 标注反模式（可选） | 发现的常见错误写法也记录（供 CodeReview 参考） | 反模式清单 |

#### 9.3 必须探查的 9 个分类

| # | 分类 | 最少探查文件数 | 典型 grep 命令 |
|---|------|-------------|---------------|
| 1 | 跨层模式 | 2 个完整调用链（BFF→Service） | 找一个 BFF RestImpl → 追到 SPI → 追到 Service AppService |
| 2 | Domain 层模式 | 3 个 DO 类 | `find . -path "*/domain/*/model/entity/*DO.java"` |
| 3 | Application 层模式 | 3 个 AppService 类 | `grep -rl "@Transactional" --include="*AppService.java"` |
| 4 | Infrastructure 层模式 | 3 个 RepositoryImpl + 2 个 FacadeImpl | `find . -name "*RepositoryImpl.java"` |
| 5 | Interfaces 层模式 | 3 个 RestImpl 类 | `find . -name "*RestImpl.java" -path "*/interfaces/*"` |
| 6 | 异常处理模式 | 所有 ErrorCode 枚举 + GlobalExceptionHandler | `find . -name "*ErrorCode.java"` |
| 7 | 并发与幂等模式 | 含锁/幂等的实现 | `grep -rl "@Version\|RedisLock\|Idempotent" --include="*.java"` |
| 8 | 测试模式 | 3 个 Test 类 | `find . -name "*Test.java" -path "*/test/*"` |
| 9 | 配置与集成模式 | Config 类 + Job 类 | `find . -name "*Config.java" -path "*/config/*"` |

#### 9.4 门禁

- 🔴 **每个分类至少 1 条经验**（9 分类 × ≥1 条 = 最少 9 条经验）
- 🔴 **每条经验必须有 ≥2 个真实出处**（文件路径:行号）
- 🔴 **每条经验必须标注对齐的 constraints/ 条款**
- 🟠 **违反约束的惯用写法必须记录为"反模式"**（不进经验文档正文，放附录）
- 🟡 **探查覆盖 ≥2 个不同服务**（避免单服务偏见）

### 步骤 10：审计与归档

- 在 §1 写 `lastAuditedAt`
- 在文件底部加 "维护" 章节（owner/更新频率/与谁同步）

---

## 10. 团队惯用实现方式（经验文档） 🆕 与 §11 缺口表不可混用

> **🆕 2026-06-26 命名说明**：本节是"团队惯用模式沉淀"，**不要**把项目缺口写到这里；项目缺口一律写 §11。
>
> **本质：** 从项目现有代码中提炼团队**已验证的惯用实现模式**，经约束过滤后沉淀为可复用经验。CodingSkill 在实现新功能时优先参考本节，避免重复设计已有成熟方案。

### 10.1 经验文档结构

| 分类 | 内容 | 提炼来源 | 过滤标准 |
|------|------|---------|---------|
| **跨层模式** | 完整调用链路的惯用组合（如"BFF→SPI→AppService→Repository"的标准写法） | 选取 ≥3 处一致使用的调用模式 | 符合 constraints/layered-arch.md |
| **Domain 层模式** | 聚合根设计、状态机实现、领域事件、充血方法命名 | DO 类中含业务方法（非 getter/setter）的典型写法 | 符合 constraints/project-structure.md |
| **Application 层模式** | 事务边界组织、编排顺序、Converter 使用、审计字段赋值 | AppService 类的惯用结构 | 无大事务、事务内无远程调用 |
| **Infrastructure 层模式** | Repository 实现、Feign 封装、Facade 异常处理、PO↔DO 转换 | RepositoryImpl / FacadeImpl 的典型实现 | 符合 constraints/code-style.md |
| **Interfaces 层模式** | RestImpl 结构、参数校验、统一返回包装 | Controller 的惯用写法 | 符合 constraints/api.md |
| **异常处理模式** | 错误码定义、异常类层次、全局异常捕获、业务异常 vs 系统异常 | ErrorCode 枚举 + GlobalExceptionHandler | 无空 catch、无吞异常 |
| **并发与幂等模式** | 乐观锁、分布式锁、幂等键设计 | 含 `@Version` / Redis Lock / 幂等注解的实现 | 符合 constraints/security.md |
| **测试模式** | 单测组织结构、Mock 策略、测试数据构造、断言惯用写法 | Test 类的惯用结构 | 符合 constraints/testing.md |
| **配置与集成模式** | Nacos 配置组织、Feign Fallback、定时任务注册、消息监听器 | config 类 + job 类 + listener 类 | 符合 constraints/technology-stack.md |

### 10.2 每条经验的记录格式

```markdown
### {分类}.{序号} {模式名称}

**场景：** 什么时候用这个模式
**惯用写法：**（代码骨架，≤30 行，含关键注释）
**出处：** {文件路径}:{行号}（至少 2 个真实出处证明"惯用"）
**约束对齐：** 符合 {constraints/xxx.md} 中的 {具体条款}
**反模式：** 项目中存在的错误写法（如有，标注文件路径，供 CodeReview 参考）
**实战案例：** [{function 专题路径}](../../function/{Story-id}.md#接口-{N})（可选；无案例时写"暂无"）
```

### 10.3 准入门禁（什么样的实现才能进经验文档）

| # | 准入条件 | 验证方式 |
|---|---------|---------|
| 1 | **≥2 处一致使用**（非孤例） | grep 类名/方法签名，确认至少 2 个不同业务域使用 |
| 2 | **符合 constraints/**（不违反任何红线） | 逐条对照 constraints/ 对应文件 |
| 3 | **无已知 BUG/技术债**（不沉淀带病模式） | 检查是否有对应的 TODO/FIXME/已知问题 |
| 4 | **可脱离具体业务复用**（抽象到模式级别） | 去掉业务字段后仍能说清"什么时候用、怎么用" |

### 10.4 排除规则（什么不应进经验文档）

- ❌ 只有 1 处使用的"个人偏好"写法
- ❌ 违反 constraints/ 任一条款的写法（即使团队多人使用）
- ❌ 已被标记为技术债/待重构的代码模式
- ❌ 框架自动生成的样板代码（无学习价值）
- ❌ 过度设计的模式（违反 KISS，引入不必要复杂度）

### 10.5 与其他章节的关系

| 引用方 | 引用方式 |
|--------|---------|
| ④bis CodingPlan §5 关键类骨架 | "按项目资产 §10 经验文档的 {分类}.{序号} 模式实现" |
| ⑤ Coding 实施 | 新代码优先复用经验文档中的模式；无对应模式时先评估是否应沉淀为新模式 |
| ⑦ CodeReview | 对照经验文档检查新代码是否偏离团队惯用方式（偏离需说明理由） |

---

## 11. 项目资产缺口与待补充

| # | 缺口 | 优先级 | 负责人 | 计划补齐时间 |
|---|------|-------|-------|------------|
| 1 | `d:/Item/document/life-team-project-docs/knowledge/` 完整文档未读 | 🟠 P1 | | |
| 2 | boss-abnormal/webagent/life-cs/im/vehicle/user/workticket/notification/ops-notification 的 server.port 未读 | 🟡 P2 | | |
| 3 | icec-cloud-life-api / icec-cloud-boss-api 聚合工程的 server.port 未读 | 🟡 P2 | | |
| 4 | 错误码分段未统一 | 🟠 P1 | | |
| 5 | implicit-constraints.md 为空 | 🟠 P1 | | |
| 6 | icec-cloud-life-cs-spi 等其它 SPI 子模块的 ServiceProviderConstants 未全抽 | 🟡 P2 | | |
| 7 | ops/ 目录（部署/健康检查/迁移）未读 | 🟢 P3 | | |
| 8 | 其他 Service 工程的 DDD 内部分层是否与 icec-cloud-boss-user 完全一致未逐一验证 | 🟡 P2 | | |

---

## 12. 横切专题文件索引（🆕 2026-06-26）

> **🆕 升级背景**：原 schema 只有"项目主体 + 工程级"两层，缺"跨工程的横切专题"维度。一个 Story 通常跨 5+ 工程（参见同事 `function/登录鉴权-BE-接口逻辑排查.md` 跨 5 工程），强行塞进主体文件会导致主体膨胀到 100KB+ 无法维护。
>
> **新增三类横切专题**：业务场景专题（function/）+ 环境/部署/测试配置专题（config/）+ 业务域概览专题（domain/）。

### 12.1 三类横切专题边界

> 🆕 v4.1：下表"目录"列用相对基准 `{资产根}/` 表示，完整路径见 [document-storage §2.3](../../skills/cross-cutting/document-storage-skill.md)（`{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/`）。

| 类型 | 目录 | 用途 | 典型场景 | 触发时机 |
|------|------|------|---------|---------|
| **function/** | `{资产根}/function/` | **跨工程业务场景专题** | 单 Story 跨 ≥3 工程的完整调用链 + 影响面 + 错误码 + Redis Key + DTO 字段 | 每个 Story 完成后必产 1 篇（STORY-002-BE / STORY-003-BE / ...）|
| **config/** | `{资产根}/config/` | **环境/部署/测试/运维配置** | 本地端口表 + 测试环境 URL + Feign URL 注入 + management port 分配 + Docker 镜像标签 + 数据迁移 | 部署相关 PR / 新环境接入 / 配置变更时更新 |
| **domain/** | `{资产根}/domain/` | **业务域概览（业务全景图）** | 业务域边界 + 域间依赖 + 域事件流 + 关键术语表 | 项目启动时建首版；域变更时更新 |

### 12.2 命名规范

| 类型 | 文件名模板 | 反例 |
|------|-----------|------|
| function/ | `{Story编号或场景标识}-{端标识}.md` 或 `{场景中文名}-BE-接口逻辑排查.md` | ❌ `function.md` / `function-boss.md`（太泛）|
| config/ | `{环境或场景}.md`，如 `test/api-test-env.md` / `prod/部署清单.md` / `local/本地调试指南.md` | ❌ `config.md` / `config-all.md` |
| domain/ | `{业务域Key}.md`，如 `cs/客服域.md` / `im/IM域.md` / `user/用户域.md` | ❌ `domain.md` |

### 12.3 与主体文件的关系（内容结构 — 🆕 v4.1 路径改为引用）

> **🆕 v4.1（2026-06-27，SKILL 边界修复）：** 本节只定义资产的**内容结构**（装什么子目录、各装什么），**存放路径不再在本文件定义**——路径单一权威源是 [`document-storage-skill.md §2.3 资产类路径模板`](../../skills/cross-cutting/document-storage-skill.md)。此前本节曾硬编码 `skills/ae-sdd/assets/{projectKey}/` 路径，与 document-storage/代码三处漂移，是"路径偏移"的制度性根因。

**资产内容结构（schema 该管的"装什么"）：**

```
{workspaceKey}/                                 # 路径根见 document-storage §2.3，此处只看内容组织
├── {workspaceKey}.assets.md                    # 主体/工作区级索引（核心映射+索引，≤30KB）
├── {workspaceKey}.update-log.md                # 主体变更日志
├── {workspaceKey}.pending-questions.md         # 主体待确认问题
├── function/                                   # 跨工程业务场景专题（按 Story 分）—— 内容结构
├── config/                                     # 环境/部署/测试/运维配置 —— 内容结构
├── domain/                                     # 业务域概览 —— 内容结构
├── {line}/                                     # 🆕 v4.1 业务线分组（多业务线项目；单业务线项目无此层）
│   └── {工程名}/{工程名}.assets.md             # 工程级子文件（一个工程一个）
└── {工程名}/{工程名}.assets.md                 # 工程级子文件（单业务线扁平）
```

**存放路径（引用，不在本文件重定义）：**
- 工作区级索引 / 工程级子文件 / 日志的**完整路径模板** → 见 [`document-storage-skill.md §2.3`](../../skills/cross-cutting/document-storage-skill.md)
- 工程级子文件**业务线分组**（line）规则 → 见 [`document-storage-skill.md §0.5.3`](../../skills/cross-cutting/document-storage-skill.md)
- 代码层自动发现（line 分组 + 单层 + 扁平三路共存）→ `paths.find_module_asset_files()`

**工程级子文件命名（schema 该管的"怎么叫"）：** `{工程名}.assets.md`（详见 §15.2 命名规范）

> ⚠️ **向后兼容：** 历史 v4.0 的 `{docWorkspacePath}/assets/{key}/{module}/` 单层、`.ae-sdd/assets/{key}.*.assets.md` 扁平仍被代码自动发现，不强制迁移。

**主体文件 §A-§G 索引需含三类专题引用：**
- §D.1 config/ 索引（环境/部署组件）
- §E.1 function/ 业务场景索引（Story ID + 一句话摘要）
- §F 反向索引补 `function/` / `config/` / `domain/` 三个关键词

### 12.4 门禁

- 🔴 **每个 Story 完成后必产 1 篇 `function/`**（否则下次同类需求无人参考）
- 🔴 **新工程接入必产 1 篇 `config/test/api-test-env.md`**（否则下一个接手的 coder 浪费 1-2 小时）
- 🟠 **业务域变更（新增域/域拆分）必产 1 篇 `domain/`**

---

## 13. 信息可信度三态标注规范（🆕 2026-06-26）

> **🆕 升级背景**：原 schema 所有内容一视同仁，**不知道哪些是 explore agent 真实读到的、哪些是推断的、哪些是没确认的**——下游消费方（Code Plan / Coding）无法判断可信度。同事知识库用 `[据命名/结构推断]` `[待确认]` 区分清楚了，值得吸收。

### 13.1 三态定义

| 标记 | 含义 | 下游消费规则 | 示例 |
|------|------|------------|------|
| `[已确认]` | 有源码 / 配置文件 / pom 等明确证据 | 直接采纳 | `[已确认] BossUserAppService 在 boss-user-application/.../appservice/BossUserAppService.java:42` |
| `[据推断]` | 由命名约定 / 包结构 / 类似代码模式推断（≥2 处一致）| 谨慎采纳，Code Plan 阶段需进一步验证 | `[据推断] 状态机 if-else 应放 AppService 而非 Controller` |
| `[待确认]` | 探索过程发现但无法验证的事项 | **写入 §10 pending-questions，不进 §0-§9 正文** | `[待确认] boss-abnormal 的 server.port` |

### 13.2 在资产中的标注位置

- **章节标题前缀**：`## [已确认] 4. DDD 内部分层落点`（整章都是确认的）
- **表格单元后缀**：`| 11101 | 用户不存在 [已确认] | BossUserErrorCode |`
- **行内标注**：`redisTemplate [据推断：与 BossUserExtensionPO 同包推断字段名]`

### 13.3 自动分流规则（🔴 探查 SOP 必须遵守）

1. 探查 Agent 抽取内容时**默认全部 `[据推断]`**
2. 凡能贴出 `文件路径:行号` 或 `bootstrap.yml` 行号 → 升级 `[已确认]`
3. 凡"未读到 / 配置缺失 / 端口冲突" → **不得写进主体**，必须写入 `{projectKey}.pending-questions.md`
4. 探查完成后跑一次 `pending-questions.md → §10 缺口表` 同步脚本（待实现）

### 13.4 探查输出物可信度审计（每月审计时跑）

```bash
# 主体中所有"推断"内容占比应 ≤ 30%
grep -c "\[据推断\]" {projectKey}.assets.md
grep -c "\[已确认\]" {projectKey}.assets.md

# pending-questions 中问题数（应逐步减少）
grep -c "^| Q-" {projectKey}.pending-questions.md
```

---

## 14. 安全隐患记录 SOP（🆕 2026-06-26）

> **🆕 升级背景**：同事 user.md §1.4 末尾主动写了"bootstrap.yml 中 Redis 明文密码直接写入配置文件，建议改为占位符外部注入 [待确认]"——这种**探查过程发现的安全隐患**值得结构化记录。

### 14.1 强制扫描清单（探查 SOP §9.11 必跑）

```bash
# 1. 配置文件明文密码扫描（🔴 P0）
grep -rEn "password\s*[:=]\s*[\"']?[A-Za-z0-9_-]{6,}" \
  --include="*.yml" --include="*.yaml" --include="*.properties" \
  {module-path}/src/main/resources/

# 2. 硬编码 API Key / Secret 扫描（🔴 P0）
grep -rEn "(api[_-]?key|secret|token)\s*[:=]\s*[\"']?[A-Za-z0-9_-]{16,}" \
  --include="*.java" --include="*.yml" \
  {module-path}/src/

# 3. Actuator 端点外露扫描（🔴 P0）
grep -rEn "management\.endpoints\.web\.exposure" \
  --include="*.yml" {module-path}/src/main/resources/
# 若 exposure.include=* 且无 spring.security 配置 → 警告

# 4. 数据库连接串明文账号扫描（🔴 P0）
grep -rEn "jdbc:mysql://[^?]*:[^@]*@" \
  --include="*.yml" {module-path}/src/main/resources/
```

### 14.2 写入位置与格式

发现安全隐患后，**写入主体 §14.3 "安全隐患登记表"**，格式：

```markdown
| ID | 类型 | 位置 | 风险等级 | 描述 | 建议修复 | 状态 |
|----|------|------|---------|------|---------|------|
| S-001 | 明文密码 | bootstrap.yml:42 | 🟠 高 | Redis password 直接写在配置文件 | 改为占位符外部注入 | 待修 |
| S-002 | Actuator 外露 | bootstrap.yml:15 | 🟡 中 | exposure.include=* 未配置鉴权 | 加 spring.security 配置或 include 收紧 | 待修 |
```

**禁止**把安全隐患直接写进 §1-§9 正文（污染主体内容）。

### 14.3 门禁

- 🔴 **每次探查/审计必跑 14.1 扫描**，未跑视为探查未完成
- 🔴 任何 🟠 高级风险**必须 24h 内**通知架构组 + 工程 owner
- 🟡 🟡 中级风险随下次 PR 修复
- 🟢 🟢 低级风险累积到月底审计统一处理

---

## 15. 工程级粒度拆分 SOP（🆕 2026-06-26）

> **🆕 升级背景**：原 schema/template 强制"项目级单一文件"，导致 boss.assets.md 单文件 54KB 装 22 工程（冰山与浅滩混在一起）。同事知识库按工程拆 30+ 文件（每个 50-110KB），改一处只 review 一个文件，**变更影响面可控**。值得吸收。

### 15.1 拆分粒度判定（🔴 硬规则）

| 主体规模 | 处理 |
|---------|------|
| ≤ 10 个工程 | **不拆**，单一 `{projectKey}.assets.md` 即可 |
| 11-30 个工程 | **主体 + 工程级子文件**，主体 ≤ 30KB（只装核心映射+索引），每个工程一个 `{module-name}.assets.md` |
| > 30 个工程 | **主体 + 多工程聚合**（按 productLine/team 聚合）+ 每个工程一个文件 |

### 15.2 工程级子文件命名规范

| 类型 | 文件名模板 | 包含内容 |
|------|-----------|---------|
| 工程级子文件 | `{projectKey}.{module-name}.assets.md` | §1 模块元信息 + §2 子模块结构 + §4 DDD 落点 + §5 核心类（方法级）+ §6.1 工程特定约束 + §7 上下游契约 + §10 缺口 |
| 工程级 BFF 子文件 | `{projectKey}.{module-name}.assets.md` | 同上 + BFF 专用节（BFF Controller 列表 / Feign Client 列表 / 操作日志 capability 列表）|

**反例：** ❌ `{module-name}-notes.md` / `{module-name}.md` / `{module-name}-info.md`（后缀必须 `.assets.md` 保持一致）

### 15.3 工程级子文件 starter 模板

详见 [附录 B：工程级子文件 Starter 模板](#附录-b工程级子文件-starter-模板)

### 15.4 主体 vs 子文件分工

| 内容 | 主体 | 子文件 |
|------|------|--------|
| 项目元信息（gitPath / productLine / portRange）| ✅ | ❌ |
| 微服务清单（22 个工程一行）| ✅ | ❌ |
| §B 模块索引 | ✅ | — |
| §C 字段索引（主表 + 通用组件）| ✅ | — |
| §D 组件索引（项目级公共组件）| ✅ | — |
| §E API 索引（项目级跨服务契约）| ✅ | — |
| §F 反向索引 | ✅ | — |
| §G 读取 API | ✅ | — |
| **某工程 DDD 完整落点** | ❌ | ✅ |
| **某工程核心类方法级实现** | ❌ | ✅ |
| **某工程部署信息（数据源/Redis/镜像）** | 概要 | ✅ 详细 |
| **某工程上下游契约** | 概要 | ✅ 详细 |

### 15.5 主体与子文件引用规则

主体文件中，提到某工程时：
```markdown
- `icec-cloud-boss-user`：[详见](icec-cloud-boss-user.assets.md)
```

子文件文首：
```markdown
> **本文是 `<projectKey>.assets.md` 的工程级子文件**，仅含本工程细节。跨工程信息见主体。
```

### 15.6 门禁

- 🔴 **主体文件 > 30KB 时必须拆**（探查/审计脚本检查文件大小）
- 🔴 **任何工程新增必须同步新增对应 `{module-name}.assets.md`**（不是塞进主体）
- 🟠 **每个工程级子文件 ≥ 1 个核心类方法级实现**（防"骨架工程级文件"）

---

## 附录 A：JSON Schema（机器可读，供 Code Plan 自动生成器消费）

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Project Assets",
  "type": "object",
  "required": ["meta", "microservices", "abstractToProjectMapping", "dddInternalLayerMapping", "namingConventions", "constraints", "codePlanInputIndex", "teamPatterns", "openGaps"],
  "properties": {
    "meta": { "$ref": "#/definitions/Meta" },
    "microservices": { "type": "array", "items": { "$ref": "#/definitions/Microservice" } },
    "abstractToProjectMapping": { "$ref": "#/definitions/AbstractToProjectMapping" },
    "dddInternalLayerMapping": { "$ref": "#/definitions/DddInternalLayerMapping" },
    "namingConventions": { "$ref": "#/definitions/NamingConventions" },
    "constraints": { "$ref": "#/definitions/Constraints" },
    "serviceContracts": { "$ref": "#/definitions/ServiceContracts" },
    "codePlanInputIndex": { "type": "object" },
    "teamPatterns": { "$ref": "#/definitions/TeamPatterns" },
    "auditSOP": { "$ref": "#/definitions/AuditSOP" },
    "openGaps": { "type": "array", "items": { "type": "string" } }
  },
  "definitions": {
    "Meta": {
      "type": "object",
      "required": ["projectKey", "projectName", "gitPath", "lastAuditedAt", "owner"],
      "properties": {
        "projectKey": { "type": "string" },
        "projectName": { "type": "string" },
        "gitPath": { "type": "string" },
        "productLine": { "type": "string" },
        "profile": { "type": "array", "items": { "type": "string" } },
        "mainClass": { "type": "string" },
        "packaging": { "type": "string" },
        "portRange": { "type": "string" },
        "lastAuditedAt": { "type": "string", "format": "date" },
        "owner": { "type": "string" }
      }
    },
    "Microservice": {
      "type": "object",
      "required": ["name", "responsibility", "hasBff", "callChain"],
      "properties": {
        "name": { "type": "string" },
        "responsibility": { "type": "string" },
        "port": { "type": "number" },
        "contextPath": { "type": "string" },
        "hasBff": { "type": "boolean" },
        "callChain": { "type": "string" },
        "dependsOnSpi": { "type": "array", "items": { "type": "string" } }
      }
    }
  }
}
```

---

## 维护

- **维护人：** 架构组 / 域负责人
- **更新频率：** 每月审计一次；新增微服务/分层调整时立即更新
- **同步对象：** ① 本项目所有 Story 编写者（强制引用本文件）② 跨项目模板需对齐 `ae-sdd-update-skill.md` 边界判定
- **双源一致性审计：** 每月跑对照脚本检查 `§6 工程约束` 是否引用了 `constraints/` 所有 8 个文件名

---

## 附录 B：工程级子文件 Starter 模板（🆕 2026-06-26）

> **使用方式：** `cp skills/ae-sdd/templates/project-assets/module-assets-template.md {资产根}/[{line}/]{module-name}/{module-name}.assets.md`（多业务线项目加 `{line}/` 层；`{资产根}` 见 document-storage §2.3）

```markdown
---
name: {projectKey}-{module-name}-project-assets
description: {module-name} 工程级项目资产 — 探查时间 {YYYY-MM-DD}，含 {N} 个子模块 + {M} 个核心类 + 上下游契约。供本工程所有 Code Plan 引用。
parent: {projectKey}.assets.md
---

# {module-name} 工程级项目资产

> **本文是 `<parent>` 的工程级子文件**，仅含本工程细节。跨工程信息见主体。

---

## 0. 摘要与使用场景 [已确认/据推断/待确认]

| 维度 | 内容 |
|------|------|
| 工程名 | `{module-name}` |
| 父工程 | `<parent>` |
| 探查时间 | `{YYYY-MM-DD}` |
| 工程定位 | {一句话} |
| 关键不变量 | {本工程不重复定义 rules；只把 rules 映射到本工程代码} |

---

## 1. 模块元信息 [已确认/据推断]

| 字段 | 值 |
|------|---|
| moduleName | `{module-name}` |
| groupId | `com.casstime.cloud` |
| artifactId | `{module-name}` |
| version | `{1.0-SNAPSHOT}` |
| packaging | `pom（聚合父工程）` / `jar` |
| 子模块数 | `{N 个}` |
| 主启动类 | `{Bootstrap.java / WebApplication.java}` |
| profile | `{dev, test, prod, beta-kunlun}` |
| port | `{port 号}` |
| contextPath | `/{context-path}` |
| dependsOnSpi | [`spi 模块列表`] |
| lastAuditedAt | `{YYYY-MM-DD}` |

### 1.1 部署信息 [已确认/据推断]

| 字段 | 值 |
|------|---|
| profile.active | `{beta-kunlun}` |
| db.urlTemplate | `{jdbc:mysql://${...}/${...}?...}` |
| db.pool | `{HikariCP max-active=20 / min-idle=1 / timeout=30s}` |
| redis.address | `{redis-...dcs.huaweicloud.com:6379}` |
| redis.password.inConfig | `{true / false}` |
| gateway | `{http://...}` |
| imageRepo | `{registry..../cassmall/{module-name}:1.0-SNAPSHOT}` |
| nexusRepo | `{http://dev.casstime.com/nexus/...}` |
| management.port | `{30000 / 显式值}` |
| coverageTool | `{jacoco}` |

### 1.2 安全提示（如有）[待确认]

| ID | 类型 | 位置 | 风险等级 | 描述 | 建议修复 |
|----|------|------|---------|------|---------|
| S-{NNN} | {明文密码/外露} | {file:line} | {🟠/🟡} | {描述} | {修复建议} |

---

## 2. 子模块结构 [已确认/据推断]

| 模块 | ArtifactId | 打包 | 职责 | 主要依赖 |
|------|-----------|------|------|---------|
| 领域层 | {xxx-domain} | jar | {职责} | {依赖} |
| 应用层 | {xxx-application} | jar | {职责} | {依赖} |
| 接口层 | {xxx-interfaces} | jar | {职责} | {依赖} |
| 基础设施层 | {xxx-infrastructure} | jar | {职责} | {依赖} |
| 启动/服务层 | {xxx-service} | jar | {职责} | {依赖} |
| {其他} | ... | ... | ... | ... |

### 2.1 依赖层次关系 [已确认]

```
service → interfaces → application → domain
            ↓             ↓
        infrastructure ←─┘
```

---

## 3. 完整技术栈版本号 [已确认]

> **完整表见主体 §6.8**；本节只列本工程特有的依赖（pom 中显式声明但不在公共 dependencyManagement 的）。

| 依赖 | 版本 | 用途 |
|------|------|------|
| {xxx} | {x.x.x} | {用途} |

---

## 4. DDD 内部分层落点 [已确认/据推断]

> **详细类角色映射见主体 §4**；本节只列本工程内实际类。

| 类角色 | 精确包路径 | 典型类名（已确认） |
|--------|-----------|------------------|
| Rest 实现类 | `{interfaces/restful/}` | `{XxxServiceImpl implements SpiInterface}` |
| AppService | `{application/appservice/}` | `{XxxAppService}` |
| Domain Object | `{domain/{业务域}/model/entity/}` | `{XxxDO}` |
| ... | ... | ... |

---

## 5. 核心类方法级实现（🆕 2026-06-26）

> **🆕 升级**：原 schema §4 只列类名，缺方法级实现。**Code Plan 编写者拿到资产时需要知道 "这个类有哪些方法 / 每个方法干什么"**——单看类名不够。本节按 DDD 分层逐类写方法签名+参数+返回+业务含义。

### 5.1 Converter 层 [已确认/据推断]

#### {XxxConverter}

| 项 | 内容 |
|---|---|
| 文件路径 | `.../application/converter/{XxxConverter}.java` |
| 职责 | {一句话} |

| 方法名 | 入参 | 返回 | 业务含义 | 变更点 |
|--------|------|------|---------|---------|
| {methodName} | {params} | {return} | {含义} | {STORY-id 或 "无"} |
| ... | ... | ... | ... | ... |

> **强制规则（🆕 v3.5.1.1）**：「变更点」列用于追溯方法的演进来源，便于 CodingPlan 阶段定位"这个方法在 STORY-XXX 加了什么字段、为什么加"。新增方法时填入对应 Story ID；无对应 Story 的存量方法填"无"。

### 5.2 AppService 层 [已确认/据推断]

#### {XxxAppService}

| 项 | 内容 |
|---|---|
| 文件路径 | `.../application/appservice/{XxxAppService}.java` |
| 职责 | {一句话} |
| 事务 | `@Transactional` 在哪些方法 |

| 方法名 | 入参 | 返回 | 业务含义 | 变更点 |
|--------|------|------|---------|---------|
| {methodName} | {params} | {return} | {含义} | {STORY-id 或 "无"} |

### 5.3 Domain 层 [已确认/据推断]

#### {XxxDO}

| 项 | 内容 |
|---|---|
| 文件路径 | `.../domain/{业务域}/model/entity/{XxxDO}.java` |
| 职责 | {一句话} |
| 类型 | 充血模型（业务方法不以 get/set 开头）|

| 字段名 | 类型 | 说明 [已确认/据推断] |
|--------|------|-------------------|
| {field} | {type} | {说明} |

| 方法名 | 入参 | 返回 | 业务含义 | 变更点 |
|--------|------|------|---------|---------|
| {method} | {params} | {return} | {含义} | {STORY-id 或 "无"} |

#### {XxxDomainService}

| 项 | 内容 |
|---|---|
| 文件路径 | `.../domain/{业务域}/service/{XxxDomainService}.java` |
| 职责 | {跨聚合业务规则} |

| 方法名 | 入参 | 返回 | 业务含义 | 变更点 |
|--------|------|------|---------|---------|
| {method} | {params} | {return} | {含义} | {STORY-id 或 "无"} |

### 5.4 Infrastructure 层 [已确认/据推断]

#### {XxxRepositoryImpl}

| 项 | 内容 |
|---|---|
| 文件路径 | `.../infrastructure/persistence/repository/mysql/{XxxRepositoryImpl}.java` |
| 职责 | {仓储实现} |

| 方法名 | 入参 | 返回 | 业务含义 | 变更点 |
|--------|------|------|---------|---------|
| {method} | {params} | {return} | {含义} | {STORY-id 或 "无"} |

#### {XxxClient} (Feign)

| 项 | 内容 |
|---|---|
| 文件路径 | `.../infrastructure/feign/{XxxClient}.java` |
| 目标 SPI | `{xxx-spi}` |
| 调用服务名 | `{xxx-service}` |

| 方法名 | 入参 | 返回 | 业务含义 | 变更点 |
|--------|------|------|---------|---------|
| {method} | {params} | {return} | {含义} | {STORY-id 或 "无"} |

### 5.5 Interfaces 层 [已确认/据推断]

#### {XxxServiceImpl}

| 项 | 内容 |
|---|---|
| 文件路径 | `.../interfaces/restful/{XxxServiceImpl}.java` |
| 实现 SPI | `{XxxService extends ...}` |

| 方法名 | 入参 | 返回 | 业务含义 | 变更点 |
|--------|------|------|---------|---------|
| {method} | {params} | {return} | {含义} | {STORY-id 或 "无"} |

---

## 6. 工程特定约束 [已确认/据推断]

> 主体 §6 是项目级约束；本节只列本工程**特有**的约束。

### 6.1 工程特定静态扫描（🔴 编码完成必跑）

```bash
# 本工程特定扫描
grep -rn "com\.casstime\.cloud\.{product}\.{domain}\.\(domain\|infrastructure\)\.\w\+\." \
  --include="*.java" {module-path}/src/main/java/ \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空
```

---

## 7. 上下游契约 [已确认/据推断]

### 7.1 对外暴露（SPI / Controller）

| 类型 | 接口名 | URL / 服务名 | 文档 |
|------|--------|------------|------|
| SPI | `{XxxService}` | 服务名 `{xxx-service}` | {指向 spi 文档} |
| Controller | `{XxxServiceImpl}` | `/{context-path}/...` | {方法列表} |

### 7.2 对内消费（Feign Client）

| SPI | 服务 | 方法 | 本工程 Feign Client |
|-----|------|------|------------------|
| {XxxService} | {xxx-service} | {method} | {XxxClient} |

---

## 8. 本工程缺口与待补充

| # | 缺口 | 优先级 | 状态 |
|---|------|-------|------|
| 1 | {本工程 X 类的方法未读完} | 🟡 P2 | 待补 |
| 2 | ... | ... | ... |

---

## §A 关键词反向索引（🆕 2026-06-26）

| 关键词 | 出现位置 |
|--------|---------|
| {className} | §4 / §5.X |
| {methodName} | §5.X |
| {redisKey} | §5.X |
| ... | ... |

---

## §B 本工程更新日志（合并到主体 update-log）

> 详见主体 `{projectKey}.update-log.md`；本节列本工程特有的变更摘要。

| 日期 | 变更摘要 |
|------|---------|
| {YYYY-MM-DD} | {一句话} |
```

---

## 附录 C：function/ 业务场景专题 Starter 模板（🆕 2026-06-26）

> **使用方式：** `cp skills/ae-sdd/templates/project-assets/function-topic-template.md {资产根}/function/{Story编号或场景标识}.md`（`{资产根}` 见 document-storage §2.3）

```markdown
# {场景名}-BE 接口逻辑排查（{Story 编号}）

> **来源 Story：** {story-id}
> **涉及工程：** {A} / {B} / {C} / {D} / {E}
> **整体描述：** {一句话讲清这个 Story 在做什么业务场景}
> **探查时间：** {YYYY-MM-DD}
> **最后更新：** {YYYY-MM-DD}
> **可信度：** {已确认 / 据推断 / 待确认}

---

## 1. 接口清单 [已确认]

| # | 接口 | 用途 | 接口归属工程 | 备注 |
|---|------|------|------------|------|
| 1 | `POST {URL}` | {用途} | {归属工程} | {变更/扩展} |
| 2 | ... | ... | ... | ... |

---

## 1.5 公共数据模型变更一览 [已确认/据推断]

| 类 | 所属工程 | 新增/变更字段 | 变更类型 |
|----|----------|--------------|---------|
| `{XxxReq}` | {api/bff-api 路径} | {field1}, {field2} | 新增/扩展/删除 |
| `{XxxDTO}` | {spi 路径} | {field1}, {field2} | 新增/扩展/删除 |
| `{XxxVO}` | {bff-api 路径} | {field1}, {field2} | 新增/扩展/删除 |

> **强制规则**：本节横向列出本 Story 涉及的所有 DTO/VO/Req/Resp 字段变更，防止 CodingPlan 阶段漏 DTO。字段来源必须来自 Story、代码、接口文档或已确认专题；据推断时逐项标注。

---

## 1.6 Redis Key 设计规范 [已确认/据推断]

| Key 格式 | 用途 | TTL | 所属工程 |
|----------|------|-----|---------|
| `{prefix}:{biz}:{id}` | {用途} | {N s / 跟随 Token 过期 / 永久} | {工程} |

> **强制规则**：所有新增/修改 Redis Key 必须在本节登记；Key 命名优先遵循 `{业务域}:{功能}:{参数}` 三段式。TTL 来源不明时不得写"永久"，必须标 `{待确认}`。

---

## 1.7 跨服务 Feign 调用表 [已确认]

| 调用方 | SPI 接口定义 | 被调用方 | Feign Client | 方法 |
|--------|-------------|----------|--------------|------|
| {工程} | {spi 子模块 / 接口} | {被调工程} | {Client} | {method} |

> **强制规则**：调用方列具体工程名（不是"BFF"）；SPI 接口路径精确到子模块，便于跨工程追踪。

---

## 1.8 关键约束清单（🔴 编码必读）

> **每条约束 1 行**：`约束名 — 描述 — 出处/反例`

1. **{约束名 1}** — {描述} — 出处/反例：`{file:line 或 function §}`
2. **{约束名 2}** — {描述} — 出处/反例：`{file:line 或 function §}`

> **强制规则**：每条约束必须给出出处或反例。无出处只能标 `{待确认}`，不得进入 CodingPlan 的"已确认约束"。

---

## 1.9 集成测试范式 [已确认/据推断]

> **适用场景**：本 Story 涉及回调、状态机、多 Service 协作、落库一致性或外部 SDK 回调时填写。

| 维度 | 实现 | 出处 |
|------|------|------|
| 测试框架 | `{JUnit/SpringRunner/...}` | {file:line} |
| 数据库 | `{H2 MySQL 模式 / Testcontainers / 开发库}` | {file:line} |
| 落库门禁 | `{DRIFT-XX / AC-XX / 无}` | {file:line} |
| 回滚 | `{Transactional 自动回滚 / 清理脚本}` | {file:line} |
| Mock 策略 | `{仅 Mock 外部 SDK / 签名校验 / Hook 等}` | {file:line} |

> **强制规则**：本节仅在有集成测试需求时填；Mock 策略必须说明哪些依赖真实注入、哪些依赖被 Mock。若引用 DRIFT-XX，必须能在本工程 §6 安全门禁或 Story AC 中找到定义。

---

## 2. 接口 1：{接口名} [已确认/据推断]

**入口：** `{XxxRestImpl#{method}}` / `{XxxAppService#{method}}`

**调用链：**

```
{前端} 
  → [{归属工程 A} / {bff-api}] {XxxRestImpl#{method}
    → [{归属工程 B} / {bff}] {XxxAppService#{method}
      → [Feign 调用] {XxxClient#{method}
        → [{归属工程 C} / {spi}] {XxxService#{method}
          → [{归属工程 C} / {interfaces}] {XxxServiceImpl#{method}
            → [{归属工程 C} / {application}] {XxxAppService#{method}
              → [{归属工程 C} / {domain}] {XxxDomainService#{method}
                → [{归属工程 C} / {infrastructure}] {RepositoryImpl}
                → [{归属工程 C} / {infrastructure}] {FacadeImpl} (Redis)
```

**关键逻辑：**

1. {逻辑点 1}
2. {逻辑点 2}
3. {逻辑点 3}

**影响面：**

- **请求对象：** `{XxxRequest}` 新增 `{字段}`
- **响应对象：** `{XxxResponse}` 新增 `{字段}`
- **SPI 接口：** `{XxxService}` 新增 `{方法签名}`
- **Service 方法：** `{XxxAppService}` 新增 `{方法}`
- **Redis Key：** 新增 `{key 格式}` (TTL {n}s)

**错误码：**

| 错误码 | 含义 |
|--------|------|
| {11101} | {含义} |
| ... | ... |

**约束：**

- {约束 1}
- {约束 2}

---

## 3. 接口 2：{接口名} [已确认/据推断]

（结构同上）

---

## 4. 关键词反向索引

| 关键词 | 出现位置 |
|--------|---------|
| {类名} | §1.5 / §2 |
| {方法名} | §2 |
| {Redis Key} | §1.6 / §2 |
| ... | ... |

---

## 5. 待确认事项

| ID | 问题 | 影响范围 | 优先级 | 待查 |
|----|------|---------|--------|------|
| F-{NNN} | {问题} | {范围} | {🟠/🟡} | {方法} |
```

---

## 附录 D：config/ 环境配置专题 Starter 模板（🆕 2026-06-26）

> **使用方式：** `cp skills/ae-sdd/templates/project-assets/config-topic-template.md {资产根}/config/{环境或场景}.md`（`{资产根}` 见 document-storage §2.3）

```markdown
# {环境或场景} 配置（如 接口集成测试环境配置）

> **适用范围：** {哪些工程 / 哪些环境}
> **最后更新：** {YYYY-MM-DD}
> **可信度：** {已确认 / 据推断}

---

## 1. 工程信息

| 工程 | 路径 | 用途 |
| --- | --- | --- |
| {工程 A} | `{绝对路径}` | {用途} |
| {工程 B} | `{绝对路径}` | {用途} |

---

## 2. {场景 1（如 登录获取 Token）}

**接口：** `{METHOD} {URL}`

**说明：** {描述}

**请求体：**

```json
{...}
```

**响应字段：** `{accessToken}` 在 `{Cookie}` 里怎么传

**使用方式：** `{Cookie: security_context=<accessToken>}`

---

## 3. 本地测试前置检查

### 3.1 FeignClient 指定本地 URL

测试时需要给 FeignClient 加 `url` 指向本地服务地址，**测试完成后必须还原**。

示例：

```java
@FeignClient(name = "boss-user-service", url = "http://localhost:12004")
public interface BossUserInfoClient extends BossUserInfoService
```

### 3.2 management port 不能重复

| 工程 | management.port |
| --- | --- |
| {工程 A} | {port} |
| {工程 B} | {port} |

---

## 4. 测试接口 Base URL

### 4.1 本地服务

| 工程 | Base URL | 端口 |
| --- | --- | --- |
| {工程 A} | `http://localhost:{port}/{context-path}` | {port} |
| ... | ... | ... |

### 4.2 测试环境

| 工程 | Base URL | 备注 |
| --- | --- | --- |
| {工程 A} | `https://{host}/{prefix}` | {备注} |
| ... | ... | ... |

---

## 5. 已知踩坑（🔴 必读）

1. **{坑标题}** — {描述 + 解决方案}
2. ...

---

## 6. 关键词反向索引

| 关键词 | 出现位置 |
|--------|---------|
| {URL 前缀} | §2 / §4 |
| {端口} | §3.2 / §4 |
| ... | ... |
```

---

## 附录 E：domain/ 业务域概览专题 Starter 模板（🆕 2026-06-26）

> **使用方式：** `cp skills/ae-sdd/templates/project-assets/domain-topic-template.md {资产根}/domain/{业务域 Key}.md`（`{资产根}` 见 document-storage §2.3）

```markdown
# {业务域名}（{英文 Key}）

> **业务定位：** {一句话讲清这域做什么业务}
> **核心实体：** {聚合根列表}
> **核心流程：** {主流程列表}
> **最后更新：** {YYYY-MM-DD}

---

## 1. 业务范围

{讲清这域的边界，include 什么 exclude 什么}

## 2. 域内微服务

| 工程 | 端口 | 职责 |
|------|------|------|
| {xxx-service} | {port} | {职责} |
| {xxx-bff} | {port} | {职责} |

## 3. 核心实体与聚合根

| 实体 | 聚合根 | 关键字段 |
|------|--------|---------|
| {XxxDO} | ✅ / ❌ | {关键字段} |

## 4. 域间依赖

| 上游域 | 下游域 | 依赖方式 | 用途 |
|--------|--------|---------|------|
| {本域} | {下游域} | {Feign / 事件} | {用途} |

## 5. 域事件流

```
{事件源} --{事件名}--> {事件消费者}
```

## 6. 关键术语表

| 术语 | 含义 |
|------|------|
| {术语} | {含义} |

## 7. 业务规则

- {规则 1}
- {规则 2}
```

---

## 附录 F：横切专题 ↔ 主体章节映射表（🆕 2026-06-26）

| 横切专题 | 触发时机 | 主体章节引用 | 必产条件 |
|---------|---------|------------|---------|
| `function/{Story}.md` | 每个 Story 完成后 | 主体 §E.1 + §F 反向 | Story 涉及 ≥3 个工程 |
| `config/test/api-test-env.md` | 新工程接入测试 | 主体 §D.1 + §1.2 | 任意 BFF 工程 |
| `config/{env}/部署清单.md` | 生产环境部署变更 | 主体 §1.2 + §14 | 部署 PR |
| `domain/{域}.md` | 新建业务域 / 域拆分 | 主体 §0 + §2 | 业务域变更 |
| `{module}.assets.md` | 新工程接入 | 主体 §2 + §B | 主体文件 > 30KB 或工程数 > 10 |
```
