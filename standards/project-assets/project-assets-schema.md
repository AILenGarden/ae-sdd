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

### 6.8 技术栈范围（constraints/technology-stack.md 的本项目落点）

- Java 8 + Spring Boot 1.5.7 + Spring Cloud Dalston.SR4
- MyBatis-Plus 3.3.2 + PageHelper 5.2.1 + JUnit 4.12 + Mockito
- MySQL 8.0.17 / ES 7.10.2
- 本地缓存 Caffeine + Redis
- Kafka 必须经 courier 组件
- 定时任务用 job-spring-boot-starter（禁 `@Scheduled`）
- 禁直配 logback/log4j（用 casslog）
- 基础组件：panda/casslog/cassmetrics/job-spring-boot-starter

### 6.9 隐性约定（constraints/implicit-constraints.md 的本项目补缺）

> 当前 constraints/implicit-constraints.md 为空（仅占位）。Code Plan 编写时若发现"项目内大家知道但没人写下来"的约定，应主动提议补充到该文件，并在本节列出补充项。

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
| §1 项目资产引用块 | §1-§10 全部 | 文首必引 |
| §2 抽象分层 → 项目分层映射 | §3 / §4 | 包路径/类名模板 |
| §5 关键类骨架 | §4 / §5 | 类角色 + 命名约定 |
| §6 DO 字段对齐 | §6.5 数据库规范 | 审计四字段/类型 |
| §7 Mapper / SQL | §6.5 | EXPLAIN 验证步骤 |
| §8 测试对应 | §6.7 | 真实 DB/HTTP 判定 |
| §11 约束合规自审 | §6 全章 | 9 类约束 |
| §12 接口契约一致性 | §6.4 | URL/方法/分页/错误码 |

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

**产物：项目资产 §10 缺口**
- 探查中未读出的端口/配置/类
- 隐性约定（项目内"大家都知道但没人写下来"的坑）
- 写入项目资产 §10，并在下次审计时优先补

### 步骤 9：审计与归档

- 在 §1 写 `lastAuditedAt`
- 在文件底部加 "维护" 章节（owner/更新频率/与谁同步）

---

## 10. 项目资产缺口与待补充

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

## 附录 A：JSON Schema（机器可读，供 Code Plan 自动生成器消费）

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Project Assets",
  "type": "object",
  "required": ["meta", "microservices", "abstractToProjectMapping", "dddInternalLayerMapping", "namingConventions", "constraints", "codePlanInputIndex", "openGaps"],
  "properties": {
    "meta": { "$ref": "#/definitions/Meta" },
    "microservices": { "type": "array", "items": { "$ref": "#/definitions/Microservice" } },
    "abstractToProjectMapping": { "$ref": "#/definitions/AbstractToProjectMapping" },
    "dddInternalLayerMapping": { "$ref": "#/definitions/DddInternalLayerMapping" },
    "namingConventions": { "$ref": "#/definitions/NamingConventions" },
    "constraints": { "$ref": "#/definitions/Constraints" },
    "serviceContracts": { "$ref": "#/definitions/ServiceContracts" },
    "codePlanInputIndex": { "type": "object" },
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
