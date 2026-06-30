---
name: java3d-coding
description: |
  Java3D 适配器（🆕 v3.6.2）。承载 Java 语言 + icec/life 项目族的「编码决策知识层」，
  叠加于共有 coding-skill §3/§4/§8/§11/§12 之上。技术栈锁、DDD 4 层落点决策、骨架展开特化、
  验证姿态特化、命名/错误码决策、静态扫描特化、icec/life 踩坑决策库。
  v3.6.2 增强：§1.1bis 固化 life 三条产品线 base package；§2.4 DO/PO 类型差异判定线。
  本文件不复述项目 constraints/ + assets 的纯规则（指针引用，DRY）；只提供"遇到 X 决策时怎么选"。
  注册 key: coding-adapter-java（type=skill-new，母版 L3）。
  触发：共有 coding-skill §13.1 加载协议按项目技术栈解析本适配器后叠加应用。
---

# Java3D 适配器 — Java 语言 + icec/life 项目编码决策知识层

> **🔴 定位（v3.6.1 适配器机制）：** 本文件是共有 [`coding-skill.md`](../../source/skills/phase2-coding/coding-skill.md) 的**叠加层**，不是独立流程节点。
> - **共有 coding-skill §1-§12** 提供"语言/项目无关"的编码决策（11 维 CodingModel、CodeAnalysis 方法论、通用验证/扫描等）——始终生效。
> - **本适配器** 提供"Java 语言特有 + icec/life 项目特有"的**决策知识**——按 §13 注册加载协议命中后**叠加**到共有 §3/§4/§8/§11/§12。
> - **生效优先级**：本适配器 > 共有（冲突时以本适配器为准）。
>
> **🔴 内容边界（DRY 红线）：** 本文件只承载"决策知识层"——遇到技术选型/分层落点/注解选型/验证姿态时**怎么决策**。
> 项目的**纯规则**（字段命名、DDL 规范、错误码段定义、包路径清单等）不在这里复述，一律**指针引用**：
> - life 项目规则源：`D:\Item\life\document\life-team-ai-standards\constraints\`（9 个约束文件）+ `D:\Item\life\.ae-sdd\assets\life\life.assets.md`（规则→代码映射）
> - 通用规则源：`get_constraints(projectKey)` 返回的 9 项约束
> 新项目接入时：先 `project-assets-update-skill §3` 生成 assets，本适配器的"项目族特有决策"按需在 L1 项目层覆盖。

---

## §1 技术栈锁 + 禁用项决策（Java 语言共性 + icec 项目族）

> **决策定位：** 共有 coding-skill §2「约束文件引用」是抽象清单（列 9 项约束 name）；本节给出 Java + icec 的**具体技术栈事实**与**禁用决策**，供 CodeAnalysis 做 11 维 CodingModel 决策时锁定前提。
>
> **规则源（不复述，按需查）：** life 项目 `constraints/technology-stack.md` + `life.assets.md §6.8`

### §1.1 技术栈事实表（icec/life 项目实测）

| 维度 | 事实 | 决策影响 |
|------|------|---------|
| 语言 | Java 8（source/target=8） | 禁用 Java 9+ 语法（var/record/switch 表达式/ sealed） |
| 框架 | Spring Boot 1.5.7.RELEASE + Spring Cloud Dalston.SR4 | 校验注解包=hibernate-validator 5.x；Feign 注解包=netflix 旧包（非 spring-cloud-openfeign）|
| 持久化 | MyBatis-Plus 3.3.2 + PageHelper 5.2.1 | 禁手写分页 SQL；分页走 PageHelper |
| 数据库 | MySQL 8.0.17 | utf8mb4；每服务独占 DB，禁跨服务连库 |
| 缓存 | 本地=Caffeine，分布式=Redis（不混用） | 选型决策：单机一致→Caffeine；多实例一致→Redis |
| 搜索 | ES 7.10.2（rest-high-level-client） | 复杂检索走 ES，禁 left/full LIKE |
| 消息 | **casstime messagebus**（`@EnableMessage`+`EventPublisher`+`@EventHandler`） | 🔴 **禁** spring-kafka `@KafkaListener`、禁 courier（见 §8 踩坑#2）|
| RPC/容错 | Feign（Client 继承 SPI 接口）+ Hystrix | 跨服务调用必有降级 `fallbackMethod` |
| 配置中心 | panda-spring-boot-starter 1.0.9 + cass-config | 走 panda，禁本地硬编码业务配置 |
| 日志 | casslog-spring-boot-starter 1.5.0 | 🔴 禁直接配 logback/log4j |
| 定时任务 | job-spring-boot-starter 4.0.5（`@JobHandler`） | 🔴 禁 `@Scheduled`（见 §8 踩坑）|
| 工具 | Lombok 1.18.16（scope=provided 每模块显式声明）| MapStruct 已在 pom 声明但 🔴 **禁用其生成**（见 §8 踩坑#3）|
| 公共 | icec-cloud-commons（b2c.1.0，强制每项目） | Result/异常/utils 统一来源 |

### §1.1bis base package 固化（🆕 v3.6.2 — 消除包路径"前缀待确认"缺口）

> **🔴 决策定位：** 测试反馈——§2.2 落点表用 `{domain}.{subdomain}` 模板，但 **life 项目 base package 未固化**，导致 AI 拼包路径时前缀只能标 `{待确认}` 查 assets。本节把 life 三条产品线的 base package **直接固化**在适配器内，使 §2.2 的包路径能**机械拼全**，无需再查 assets 前缀。
>
> **使用方式：** 拼包路径 = `base package` + `.{domain}` + `.{层}.{subdomain}.{子目录}`（§2.2 表）。本节只固化前缀，**字段/类名仍归 assets §5/§4 权威**（DRY，不复述）。

| 产品线 | 代码根 | base package（Java 包根）| 典型域 |
|--------|-------|------------------------|-------|
| **2c（消费端，life 主线）** | `D:\Item\life\2c\` | `com.casstime.cloud.life` | life（=cs）、life-im 等 → `com.casstime.cloud.life.cs.*` / `com.casstime.cloud.life.im.*` |
| **admin（2B 管理）** | `D:\Item\life\admin\` | `com.casstime.cloud.boss` | boss-user、boss-vehicle 等 → `com.casstime.cloud.boss.user.*` |
| **common（公共）** | `D:\Item\life\common\` | `com.casstime.ec.cloud.common` | 跨产品线公共能力 |

**套用示例（cs 工单域，base=`com.casstime.cloud.life` + 域=`cs`）：**
- `CsTicketAppService` → `com.casstime.cloud.life.cs.application.appservice`
- `CsTicketDO` → `com.casstime.cloud.life.cs.domain.ticket.model.entity`
- `CsTicketRepositoryImpl` → `com.casstime.cloud.life.cs.infrastructure.ticket.persistence.repository.mysql`

> **其他 icec 子项目（非 life）：** base package 按各自 assets §4 固化值（如 boss 独立仓走 `com.casstime.cloud.boss`）。本表只覆盖 life 三条主线；项目级前缀差异由 L1 项目层覆盖或查对应 assets §4。

### §1.2 禁用项决策（遇以下选型时一律否决）

| 决策场景 | 禁用 | 应选 | 决策依据 |
|---------|------|------|---------|
| 时间类型 | `LocalDateTime` | `java.util.Date`（全层统一） | Feign+JSON 兼容（见 §8 踩坑）|
| JSON 序列化 | 裸 `new ObjectMapper()` | `com.casstime.commons.utils.JsonUtils` | 统一序列化策略 |
| 线程池 | `Executors.newXxx()` / 裸 `new Thread` | `ThreadPoolExecutor`（命名线程 + 显式参数） | 资源可控（core/max/queue + `CallerRunsPolicy`）|
| 对象映射 | MapStruct 生成代码 | 显式 `@UtilityClass` Converter（cs）/ `public final class`+私有构造（im） | team 约定禁 MapStruct |
| 定时 | `@Scheduled` | job-spring-boot-starter + `@JobHandler` | 调度统一可观测 |
| Kafka 消费 | `@KafkaListener` / courier | messagebus `@EventHandler` | 走统一消息总线 |
| 缓存混用 | 本地+分布式混用 | 二选一（按一致性需求） | 防双写不一致 |
| 跨服务数据 | 直接连别服务 DB | 走该服务 SPI（Feign） | 数据所有权隔离 |

---

## §2 DDD 4 层落点决策表（icec/life 特化）

> **决策定位：** 共有 coding-skill §3「分层职责红线」给的是抽象判定口诀（业务规则→Domain / 编排→Application / 存取→Repository / 协议→Interfaces）。本节给 **icec/life 的精确包路径落点** + **object-per-layer 表**，供 CodeAnalysis §6「要素 4 包路径映射」直接查用。
>
> **规则源（不复述）：** life `constraints/project-structure.md §3` + `layered-arch.md` + `life.assets.md §4`（包路径权威映射）

### §2.1 工程模块结构（4 工程类型）

| 工程类型 | 模块命名 | 职责 |
|---------|---------|------|
| SPI 模块 | `icec-cloud-life-{module}-spi` | 接口契约（interface/DTO/Request/Enum），无 impl |
| Service（DDD 四层）| `icec-cloud-life-{module}` | domain/application/interfaces/infrastructure + service 启动 |
| API 聚合 | `icec-cloud-life-api` | 多 bff-api 聚合成一个 Spring Boot |
| 独立 BFF | `icec-cloud-life-{module}-bff` | 单模块 Spring Boot |

调用链：`前端 → api → bff → spi(Feign) → service(DDD 四层)`

### §2.2 Service DDD 四层落点表（类角色 → 精确包路径）

> **🔴 调用链依赖方向（单向，违反即阻断）：** `service → interfaces → application → domain`；infrastructure 实现侧边抽象（domain 定义接口，infrastructure 实现）。

| 类角色 | 命名模板 | 落点包路径（life 2c 线） | object 持有 |
|--------|---------|----------------------|-----------|
| SPI 实现类（Service 形态）| `{Resource}ServiceImpl implements {Resource}Service` | `...life.{domain}.interfaces.restful` | DTO only |
| BFF Controller 实现 | `{Resource}RestImpl implements {Resource}Rest` | `...life.bff.{domain}.interfaces.restful` | DTO only |
| 异常处理 | `{Domain}ExceptionHandler`（`@RestControllerAdvice`） | `...{domain}.interfaces.config` | — |
| Job 处理 | `{Feature}JobHandler` | `...{domain}.interfaces.jobhandler` | — |
| 应用编排 | `{Resource}AppService`（`@Transactional`） | `...{domain}.application.appservice` | DO, DTO |
| 应用转换器 | `{Resource}Converter`（`@UtilityClass` 静态） | `...{domain}.application.converter` | — |
| 领域实体（充血）| `{Resource}DO`（**注意：DO=领域对象，非 PO**） | `...{domain}.domain.{subdomain}.model.entity` | DO only |
| 仓储接口 | `{Resource}Repository`（接口） | `...{domain}.domain.{subdomain}.repository` | — |
| 领域服务 | `{Resource}DomainService` | `...{domain}.domain.{subdomain}.service` | DO |
| 仓储实现 | `{Resource}RepositoryImpl extends ServiceImpl<Mapper,PO>` | `...{domain}.infrastructure.{subdomain}.persistence.repository.mysql` | DO, PO, DTO |
| 持久化对象 | `{Resource}PO`（贫血，`@TableName`） | `...{domain}.infrastructure.{subdomain}.persistence.entity` | PO |
| 持久化转换器 | `{Resource}PersistenceConverter` | `...{domain}.infrastructure.{subdomain}.persistence.converter` | DO↔PO |
| Mapper | `{Resource}Mapper extends BaseMapper<PO>` | `...{domain}.infrastructure.{subdomain}.persistence.mapper` | — |
| Feign 客户端 | `{Resource}Client` / `{Resource}ServiceClient` | `...{domain}.infrastructure.feign` | DTO |

### §2.3 object-per-layer 决策（每层允许持有的对象类型）

| 层 | 允许持有 | 禁止持有（违反即阻断） |
|----|---------|--------------------|
| Domain | DO only | PO / DTO / SQL / 外部服务编排 |
| Application | DO, DTO | PO / SQL 细节 / 领域规则（下沉 Domain）|
| Interfaces | DTO only | DO / PO |
| Infrastructure | DO, PO, DTO（做转换）| 业务规则 / 状态流转判断 |

> 🔴 **双命名并存事实：** cs 线 Interfaces 命名 `{Resource}ServiceImpl implements SPI {Resource}Service`；im-bff 命名 `{Resource}RestImpl implements api `{Resource}Rest`。两种形态合法，按所在模块类型选。

### §2.4 DO/PO 类型差异判定线（🆕 v3.6.2 — 消除"合法差异 vs 建模错误"判定缺口）

> **🔴 决策定位：** 测试反馈——icec 里 DO（领域充血对象）与 PO（持久化贫血对象）类型经常**不一致**（DO 用枚举/领域类型、PO 用 String/Long），这是**合法模式**（由 PersistenceConverter 桥接）。但 DO/PO 类型不一致也可能是**建模错误**（如同一业务字段在两边本应一致却写错）。文档此前没给判定线，导致 §10 异常根因分类时"层4 AI犯蠢"vs"合法差异"边界模糊。本节给明确判定线。

**判定原则：DO 持领域语义类型、PO 持存储原类型，差异由 PersistenceConverter 桥接 = 合法；同语义字段在 DO/PO 间类型不可互转（无桥接语义）= 建模错误。**

| 场景 | DO 类型 | PO 类型 | 判定 | 依据 |
|------|---------|---------|------|------|
| 状态字段 | `TicketStatus`（枚举）| `String`（VARCHAR(32)）| ✅ **合法** | 枚举↔字符串，PersistenceConverter 用 `status.name()`/`TicketStatus.valueOf()` 桥接 |
| 金额字段 | `BigDecimal`（Money 值对象）| `decimal` | ✅ **合法** | 值对象↔数值，Converter 解包/打包 |
| 时间字段 | `java.util.Date` | `datetime` | ✅ **合法**（且必须一致用 Date）| 同类型映射，🔴 严禁 DO 用 LocalDateTime |
| 主键/外键 | `Long` | `bigint` | ✅ **合法** | 同语义；🔴 但若 DO=Long / PO=String(varchar) 则 **🔴 建模错误**（无桥接语义，主键类型应一致） |
| 软删标记 | `Boolean deleted` | `Integer deleted_flag`（tinyint）| ✅ **合法** | 布尔↔0/1，Converter 桥接 |
| 嵌套对象/聚合引用 | `SubEntity sub`（DO 内充血子对象）| 扁平字段 `sub_xxx` / 关联表 | ✅ **合法** | 防腐层做聚合根↔扁平 PO 的组装/拆解（见 §8 踩坑复盘：life STORY-020 扁平 PO 范式）|
| **任意业务字段：DO=Long / PO=String（非主键，无枚举语义）** | Long | String | 🔴 **建模错误** | 无桥接语义，类型本应一致；属层1/2/3 文档错或层4 笔误 |
| **DO 含 PO/SQL/DTO 引用** | — | — | 🔴 **分层违规**（非类型问题）| 共有§3 + §2.3 object-per-layer，Domain 禁持 PO |

**遇到类型不匹配时的判定 SOP（配合共有 §10 异常根因 4 层）：**
1. 先查差异是否属上表"✅ 合法"行 → 合法则 PersistenceConverter 正确桥接即可，**不算根因**。
2. 不属合法行 → 进共有 §10 逐层判定：层1 Task 数据结构表类型矛盾？层2 Story 字段语义？层3 DR 业务规则？层4 AI 笔误/分层违规。
3. **主键/业务关键字段类型在 DO/PO 间不可互转 → 直接判层4**（除非 Task 明确要求异构存储，需 DR 举证）。

---

## §3 分层红线决策（icec/life 特化 → 叠加共有 §3）

> **决策定位：** 共有 coding-skill §3 给通用分层红线（Repository 禁业务逻辑等）。本节叠加 icec/life 项目**特有红线**（BFF 禁触 DB、事务只在 AppService 等）。

| 🔴 红线（icec/life 特有）| 决策 |
|------------------------|------|
| BFF 直接操作 DB / Redis / Kafka | 禁。BFF 必须通过 Feign SPI 调用 Service |
| BFF 写 `@Transactional` | 禁。事务只在 Service 层 AppService |
| `@Transactional` 方法内调 Feign/MQ/Kafka | 禁。事务内禁网络调用（AGENTS 红线#2）|
| 查询方法开事务 | 禁。查询方法不加 `@Transactional` |
| Application 写领域规则 | 禁。下沉 Domain |
| Application 纯读组装（提字段/拼 Map）| 禁。下沉 Repository（放方法到仓储接口）|
| Application import `infrastructure.persistence` | 禁。静态扫描拦截（见 §7）|
| AppService 内联 SQL（`"WHERE id="+id`）| 禁。走 MyBatis `#{}` 参数化 |
| Domain 串多外部服务编排 | 禁。跨服务调用归 AppService |
| DomainService 调 Feign Client | 禁。跨服务调用归 AppService |
| Domain 引用 PO/DTO/SQL | 禁 |
| Repository 写业务规则/状态流转 | 禁。仓储方法名只能 `findByXxx/save/update/updateStatus` |
| 跨 Service 直接连库 | 禁。走 SPI |

---

## §4 骨架展开特化（icec/life 决策 → 叠加共有 §4）

> **决策定位：** 共有 coding-skill §4 给通用骨架展开规则（伪代码动词→代码）。本节给 icec/life 的**特化展开决策**（注解选型、Converter 形态、事务、对象契约）。

| 伪代码/场景 | icec/life 展开决策 | 决策依据 |
|-----------|------------------|---------|
| Controller 注解 | `@RestController @Slf4j @RequiredArgsConstructor @Validated` | 构造器注入优先于 `@Autowired` |
| 依赖注入 | `@RequiredArgsConstructor`（final 字段）| Lombok 构造器注入 |
| 对象转换 | 调 `{Resource}Converter.toXxx()`（`@UtilityClass` 静态方法）；禁 MapStruct | team 约定 |
| PO↔DO 转换 | 调 `{Resource}PersistenceConverter`（infrastructure 层）| 防腐层 |
| 跨服务调用 | FeignClient 继承 SPI 接口 + `@HystrixCommand(fallbackMethod)` | 容错降级 |
| 仓储实现 | `extends ServiceImpl<{Resource}Mapper, {Resource}PO>` | MyBatis-Plus |
| 分页 | PageHelper（禁手写分页 SQL）| 统一分页 |
| 消息发送 | 事务提交后 `@TransactionalEventListener` 发 messagebus 事件 | 先写库后发消息 |
| 定时任务 | `@JobHandler`（job-spring-boot-starter）| 禁 `@Scheduled` |
| 时间字段 | `java.util.Date`（全层统一）| Feign+JSON 兼容 |
| 异常抛出 | 自定义异常 `extends RuntimeException`（带 code），在 domain `exception` 包 | 禁裸 RuntimeException |
| 返回契约 | BFF→`ApiResult<T>`；分页→`PagedModels<T>`；Service/AppService 内部→业务对象直返（无包装） | 统一响应 |
| 审计字段（created_by 等）| 在 Facade/AppService 赋值，**禁在 Converter 赋值**（会关闭 DB 自动填充）| 见 §8 踩坑 |

---

## §5 验证判定特化（icec/life 决策 → 叠加共有 §8）

> **决策定位：** 共有 coding-skill §8 通用验证（编译/启动/接口/DB/事务）。本节叠加 icec/life **特化验证姿态**（测试框架、DB 策略、构建流）。
>
> **规则源（不复述）：** life `constraints/testing.md` + `life.assets.md §6.7`

| 验证维度 | icec/life 特化决策 | 与共有 §8 的差异 |
|---------|------------------|---------------|
| 单测框架 | **JUnit 4.12** + Mockito 1.10.19 + AssertJ 2.6.0 | 非 JUnit 5 |
| Service 单测 | mock 全部外部依赖（Repository/Facade/远程）；Mock 状态不跨用例共享 | — |
| 集成测试 DB | **dev MySQL + `@Transactional`+`@Rollback`**（非 H2/TestContainers）| 🔴 与共有 §8.5「H2」不同，以本适配器为准 |
| Mapper 测试 | SpringBootTest + dev DB，验自定义 XML SQL 正确性 | — |
| Controller 测试 | MockMvc（验入参校验/响应结构/异常路径） | — |
| 核心落库路径 | 🔴 必真实 DB，禁全 Mock | 同共有 |
| 断言 | 禁只断 HTTP status；须断响应体业务字段；异常路径断 code+message | — |
| 覆盖率（JaCoCo）| 总体≥60%；Service 核心≥70%（核心方法 100%）；Mapper≥60%；Controller≥50% | — |
| 静态分析 | Checkstyle + SpotBugs，P0 阻断 CI | — |
| 构建（no-root-pom）| 🔴 无根 pom，每模块独立构建；跨模块依赖须先 `mvn install` 到本地仓 | 见 §8 踩坑#1 |
| 编译验证 | 父工程根目录 `mvn compile`（但 icec 无根 pom → 各模块 `mvn -pl {module} compile`）| 叠加：icec 无根 pom 时按模块 |

---

## §6 命名 / 错误码决策（icec/life 特化）

> **决策定位：** 共有 coding-skill §11.1 经验清单提到"字段类型与 Task 一致"。本节给 icec/life 的**命名/错误码决策事实**。
>
> **规则源（不复述）：** life `constraints/code-style.md` + `api.md` + `database.md §2` + `life.assets.md §5/§6.4`

### §6.1 命名决策表（精简版，全表查 assets §5）

| 对象 | 决策 | 反例 |
|------|------|------|
| 领域对象 | `{Resource}DO`（充血） | ❌ `{Resource}`（无后缀）/`{Resource}Entity` |
| 持久化对象 | `{Resource}PO`（`@TableName`） | ❌ `{Resource}DO`（DO 是领域对象）|
| 常量类 | `class`（非 interface）+ `@NoArgsConstructor(access=PRIVATE)` | interface 常量 |
| 枚举 | `(key,value)` 均 String + `@Getter`，无 MAP/getEnum 静态方法 | — |
| 读写入参 | 写=`{Resource}Command`，读=`{Resource}Query` | — |
| 表名 | `业务名_表的作用`（如 `abnormal_ticket`），小写下划线单数 | `users`（复数）|

### §6.2 错误码决策

- **格式**：5 位 int；成功 200，通用 500，业务错误进常量类（如 `BizCodes`），message 中文
- **life 实际段位**（`life.assets.md §6.4` 权威）：CS 16000-16999 / IM 17000-17999 / 触点 18000-18999 / 验证码 19000-19999 / 2C用户 20000-20999 / 工单 21000-21999 / 运营通知 22000-22999 / 车辆 23000-23999
- **决策**：新增错误码先查所属域段位，禁跨域占用

---

## §7 静态扫描特化（icec/life 决策 → 叠加共有 §12）

> **决策定位：** 共有 coding-skill §12 给通用 grep 规则。本节叠加 icec/life **工程特化扫描**（防分层违规、防框架误用）。
>
> **规则源（不复述）：** `life.assets.md §6.11`（静态扫描清单权威）

```bash
# 1. application 层禁 import infrastructure.persistence（防反向依赖）
grep -rn "import .*\.infrastructure\.\(.*\.\)\?persistence\." \
  --include="*.java" src/main/java/**/application/ 2>/dev/null
# 期望空；非空=反向依赖（🔴 阻断）

# 2. AppService 禁 SQL 关键字（防持久化细节泄漏到 application）
grep -rniE "\b(SELECT|INSERT|UPDATE|DELETE|WHERE|FROM|JOIN)\b" \
  --include="*AppService.java" src/main/java/ 2>/dev/null
# 期望空；非空=AppService 串 SQL（🔴 阻断）

# 3. ServiceImpl/RestImpl 禁状态流转判断（防业务规则塞到接口层）
grep -rnE "(canTransition|transition\(|status\.equals|switch\s*\(.*status)" \
  --include="*ServiceImpl.java" --include="*RestImpl.java" src/main/java/ 2>/dev/null
# 期望空；状态机逻辑应在 Domain（🔴 阻断）

# 4. import 块外禁全限定名（共有 §12 规则1的 icec 复用，扫描全工程）
grep -rn "^[^/*].*\bjava\.\(util\|sql\|io\|time\|math\|net\)\.\w" \
  --include="*.java" src/main/java/ | grep -v ":import " | grep -v ":package "
# 期望空

# 5. 禁 LocalDateTime（icec 强制 Date）
grep -rn "LocalDateTime" --include="*.java" src/main/java/ 2>/dev/null
# 期望空（DTO/DO/PO 全用 Date）

# 6. 禁 @Scheduled（icec 强制 job-spring-boot-starter）
grep -rn "@Scheduled" --include="*.java" src/main/java/ 2>/dev/null
# 期望空

# 7. 禁裸 ObjectMapper（强制 JsonUtils）
grep -rn "new ObjectMapper" --include="*.java" src/main/java/ 2>/dev/null
# 期望空

# 8. 禁 Executors（强制 ThreadPoolExecutor）
grep -rn "Executors\.\(newFixed\|newCached\|newSingle\|newScheduled\)" --include="*.java" src/main/java/ 2>/dev/null
# 期望空
```

---

## §8 icec/life 踩坑决策库（高价值"人人知但未写"的陷阱）

> **决策定位：** 这些是项目特有陷阱的**决策规避**，源自 `life.assets.md §6.9/§6.10/§10` 与历史复盘。CodeAnalysis 遇相关场景须主动核对。

| # | 陷阱 | 决策规避 |
|---|------|---------|
| 1 | **无根 pom**：所有模块独立构建，跨模块依赖需先 `mvn install` | 跨模块引用前先 `mvn install` 被依赖模块到本地仓；CI 流水线按依赖序构建 |
| 2 | **messagebus ≠ courier**：constraints 文档写"Kafka via courier"，但生产实际用 casstime 自建 messagebus（`@EnableMessage`+`EventPublisher`+`KafkaApplicationEventPublisher`） | 消息发送一律走 messagebus；禁 `@KafkaListener`、禁 courier；外部 Kafka PUSH 经 handwritten KafkaConsumer → messagebus 桥接 |
| 3 | **MapStruct 声明却禁用**：pom 有 `mapstruct.version=1.5.3.Final`，但 code-style/project-structure 禁用其生成 | 一律显式 Converter（cs=`@UtilityClass`，im=`public final class`+私有构造）；pom 的 MapStruct 声明是历史遗留，勿启用 |
| 4 | **@NotBlank 包版本**：Spring Cloud Dalston（hibernate-validator 5.x）用 `org.hibernate.validator.constraints.NotBlank`（非 javax.validation）| 校验注解按 Boot 版本确认来源包 |
| 5 | **Feign 注解版本**：Dalston 用 `org.springframework.cloud.netflix.feign.FeignClient`（非 openfeign）| Feign 注解按 Cloud 版本确认包 |
| 6 | **Lombok scope=provided**：不传递，每模块须显式声明 | 新模块 pom 必须显式加 lombok（scope=provided）|
| 7 | **新模块须注册父 pom `<modules>`**；SPI 脚手架依赖常被注释 | 新建模块后注册到父 pom modules；检查 SPI 依赖是否被注释 |
| 8 | **VO vs DTO 分离**：bff-api 定义 VO，SPI 定义 DTO，Controller 做转换；BFF Controller 实现 bff-api 定义的 Rest 接口，不自加 `@Api`/`@GetMapping` | BFF Controller 继承 Rest 接口，不自造注解 |
| 9 | **BFF 鉴权**：只用 `TokenService.getCurUserId()`；🔴 禁 `AccessUserInfoContext`；JWT 经 Cookie `security_context`，禁 `Authorization: Bearer` | BFF 取用户身份固定用 TokenService |
| 10 | **审计字段赋值**：在 Converter 里赋 created_by 等会关闭 DB 自动填充（`DEFAULT CURRENT_TIMESTAMP`）| 审计字段在 Facade/AppService 赋值，Converter 不碰 |
| 11 | **状态机**：cs 用 spring-statemachine，`CsTicketDomainService` 持状态机逻辑；`CsTicketDO` 是语义聚合根但无 `@AggregateRoot` 注解；cs_ticket 超时从 `assigned_at` 算，非 `created_date` | 状态机逻辑归 Domain；超时基准按业务字段非创建时间 |
| 12 | **im-bff 跨域消费**：im-bff 同时消费 im-spi 和 cs-spi（跨域）| im-bff 编排时承认跨域依赖 |
| 13 | **boss-vechcle 拼写**：`boss-vehicle-bff` 包名历史拼成 `vechcle`（应为 vehicle），留原状兼容 | 勿"修正"包名，破坏兼容 |
| 14 | **安全债警示**：历史发现 12 处明文密钥（Redis/MySQL 密码、appKey/appSecret、RSA 私钥 in `boss-common JwtEncoderUtil`）| 🔴 编码时绝不可硬编码密钥/Token/身份证（AGENTS 红线#4）|

---

## §9 与共有 coding-skill 的映射（叠加覆盖声明）

> **🔴 本表是 §13 注册加载协议要求的"必含章节"**。AI 叠加应用时按本表把本适配器决策叠加到共有章节，**冲突时以本适配器为准**。

| 共有 coding-skill 章节 | 本适配器叠加章节 | 叠加内容 | 优先级 |
|----------------------|---------------|---------|--------|
| §3 分层职责红线 | §2（落点表）+ §2.4（DO/PO 判定线）+ §3（特化红线）| icec/life 精确包路径（含 §1.1bis 固化 base package）+ DO/PO 合法差异 vs 建模错误判定 + BFF禁触DB/事务只在AppService 等特化红线 | adapter > 共有 |
| §4 骨架展开规则 | §4（骨架特化）| 注解选型/Converter形态/Feign+Hystrix/事务/对象契约 | adapter > 共有 |
| §8 验证判定标准 | §5（验证特化）| JUnit4+Mockito / dev-DB+@Transactional / no-root-pom 构建 | adapter > 共有（§8.5 H2 被 dev-DB 覆盖）|
| §11 经验检查清单 | §6（命名/错误码）+ §8（踩坑库）| icec/life 命名表 + 错误码段 + 14 项踩坑决策 | adapter > 共有（§11.1 第3/4/7/10/11项被 §8 踩坑库取代）|
| §12 静态扫描规则 | §7（静态扫描特化）| 8 条 icec 工程特化 grep（防分层/防框架误用）| adapter > 共有（叠加在通用 §12 之上）|
| §1 CodingModel 决策 | §1（技术栈锁）+ §1.1bis（base package）| 11 维决策时锁定 Java8/SB1.5.7/messagebus 等技术前提 + 固化包路径前缀 | adapter 补充（不取消共有）|
| §2 约束文件引用 | §1（技术栈事实）| 给出 9 项约束的具体 Java+icec 事实 | adapter 补充 |
| §10 异常根因 4 层 | §2.4（DO/PO 判定线，🆕 v3.6.2）| 层1/层4 判定时区分"DO/PO 合法差异"与"建模错误" | adapter 辅助（不覆盖共有§10 方法论）|

> **未被覆盖的共有章节（仍按共有生效）：** §5 CodeAnalysis ④bis 全套、§6 ④bis 实战 SOP、§7 G-CODEPLAN-SRC、§9 漂移核查、§13 适配器注册加载协议本身（含 §13.1bis 叠加视图速查表，🆕 v3.6.2）——这些是语言/项目无关的方法论，本适配器不覆盖。

---

## 注册信息（供 plugin_loader.validate / trace 核对）

```yaml
# 注册位置：母版 L3 plugins/registry.yaml（项目可在 .ae-sdd/plugins/registry.yaml 用 L1 覆盖）
# 适配器文件：plugins/java3d-coding-skill/SKILL.md（L3 插件工件，path 相对注册表目录）
name: java3d-coding-skill
type: skill-new
provides: coding-adapter-java
path: ./java3d-coding-skill/SKILL.md
version: 1.1.0
description: Java 语言 + icec/life 项目编码决策知识层（叠加于共有 coding-skill）
```

**查加载路径：** `ae-sdd plugin trace coding-adapter-java` → 应命中 L3-master: java3d-coding-skill → resolved: .../plugins/java3d-coding-skill/SKILL.md

**项目层覆盖示例（life 若要定制）：** 在 `D:\Item\life\.ae-sdd\plugins\registry.yaml` 注册同名 provides（coding-adapter-java）指向 life 自定义适配器，L1 优先级覆盖母版 L3。
