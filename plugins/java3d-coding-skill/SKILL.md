---
name: java3d-coding
description: |
  Java3D 适配器（🔧 v1.4.0，基于 references 学习成果增强：聚合边界判定线 + messagebus 事件设计 +
  @Transactional 传播陷阱 + 分布式锁反模式 + 静态扫描补充）。承载 Java 语言 + icec/life
  项目族的「编码决策知识层」，叠加于共有 coding-skill §3/§4/§8/§11/§12 之上。技术栈锁、Service DDD 5 模块
  落点决策（含防腐层 Facade + 集成/领域事件双线）、BFF 三层落点决策（合并 domain、interfaces/converter、
  数据策略红线）、聚合边界判定线、骨架展开特化、验证姿态特化、命名/错误码决策、静态扫描特化、icec/life 踩坑决策库。
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
| 消息 | 底层 **Kafka**，上层封装为 **casstime messagebus**（`@EnableMessage`+`EventPublisher`+`@EventHandler`） | 🔴 业务代码禁直连 spring-kafka `@KafkaListener`、禁 courier，一律走 messagebus 封装（见 §8 踩坑#2）|
| RPC/容错 | Feign（Client 继承 SPI 接口）+ Hystrix | 跨服务调用必有降级 `fallbackMethod` |
| 配置中心 | panda-spring-boot-starter 1.0.9 + cass-config | 走 panda，禁本地硬编码业务配置 |
| 日志 | casslog-spring-boot-starter 1.5.0 | 🔴 禁直接配 logback/log4j |
| 定时任务 | job-spring-boot-starter 4.0.5（`@JobHandler`） | 🔴 禁 `@Scheduled`（见 §8 踩坑）|
| 工具 | Lombok 1.18.16（scope=provided 每模块显式声明）| MapStruct 已在 pom 声明但 🔴 **禁用其生成**（见 §8 踩坑#3）|
| 工具（🔧 v1.2.0 补） | **Guava** | 集合/缓存/函数式工具优先用 Guava，禁重复造轮子 |
| 异步/响应式（🔧 v1.2.0 补） | **RxJava** | 异步流式编排场景使用；与 Feign/Hystrix 同步调用并存，按场景二选一，不混用同一调用链 |
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
| 分布式锁 | `setIfAbsent`+分离 `expire`（非原子，崩溃间隙丢 TTL=经典坏锁）| Redisson `tryLock`/`trySet`（原子带 TTL）；或注入项目已有锁 wrapper | 非原子→锁泄露/互斥失效（见 §8 踩坑#20）|

---

## §2 DDD 4 层落点决策表（icec/life 特化）

> **决策定位：** 共有 coding-skill §3「分层职责红线」给的是抽象判定口诀（业务规则→Domain / 编排→Application / 存取→Repository / 协议→Interfaces）。本节给 **icec/life 的精确包路径落点** + **object-per-layer 表**，供 CodeAnalysis §6「要素 4 包路径映射」直接查用。
>
> **规则源（不复述）：** life `constraints/project-structure.md §3` + `layered-arch.md` + `life.assets.md §4`（包路径权威映射）

### §2.1 工程模块结构（5 平级模块 + 独立打包，🔧 v1.2.0 按 life-user README 修正）

> **🔴 与旧版差异：** 旧版把 domain/application/infrastructure/interfaces 揪成一个"Service"工程统一命名。实测项目（`icec-cloud-boss-user` 等）是 **5 个平级 Maven 模块**，每层单独一个模块、单独一个 pom、单独打成一个 JAR（README DDD 约定第 3 条），service 模块只是启动壳，不含业务代码。

| 工程类型 | 模块命名（`{project}` 如 `life-user`） | 职责 | 依赖 |
|---------|------------------------------------|------|------|
| SPI 模块 | `{project}-spi` | 接口契约（interface/DTO/Request/Enum），无 impl | 无 |
| interfaces 模块 | `{project}-interfaces` | 接口层/表现层：外部输入输出协议适配 | application、spi |
| application 模块 | `{project}-application` | 应用层：薄层业务流程编排 | domain、spi |
| domain 模块 | `{project}-domain` | 领域层：业务规则与逻辑，不依赖技术框架 | 不依赖任何其他模块 |
| infrastructure 模块 | `{project}-infrastructure` | 基础设施层：为上层提供技术实现（DB/缓存/MQ/Feign）| domain、application、spi |
| service 模块 | `{project}-service` | 统一主模块，仅含启动类 + bootstrap 配置 | ALL：domain/application/infrastructure/interfaces/spi |
| API 聚合 | `icec-cloud-life-api` | 多 bff-api 聚合成一个 Spring Boot | — |
| 独立 BFF | `icec-cloud-life-{module}-bff` | 单模块 Spring Boot | — |

调用链：`前端 → api → bff → spi(Feign) → service(DDD 五模块)`

### §2.2 Service DDD 五模块落点表（类角色 → 精确包路径，🔧 v1.2.0 按 life-user README 修正）

> **🔴 调用链依赖方向（单向，违反即阻断）：** `interfaces → application → domain`；infrastructure 实现/依赖其余各层抽象，其余各层不可直接调用 infrastructure（README DDD 约定第 2 条）。
>
> **🔴 与旧版差异：** ① application/infrastructure/interfaces 三层去掉 `{subdomain}` 嵌套（仅 domain 层按聚合名嵌套一级，其余 3 层是扁平包结构，见 README 包结构树）；② 新增 Facade（防腐层，接口定义于 domain / 实现于 infrastructure）与 EventPublisher/EventHandler（集成事件 vs 领域事件两条线，见 README DDD 约定第 6/7 条）；③ Mapper 落点补 `dao` 子包 + XML 独立路径；④ 持久化转换器改名 `{Resource}DataConverter`（原 `PersistenceConverter` 为旧命名，弃用）。

| 类角色 | 命名模板 | 落点包路径（life 2c 线，`{domain}`=业务域如 user/cs，`{aggregate}`=聚合名，单聚合服务通常=`{domain}`） | object 持有 |
|--------|---------|----------------------|-----------|
| SPI 实现类（Service 形态）| `{Resource}ServiceImpl implements {Resource}Service` | `...life.{domain}.interfaces.restful` | DTO only |
| BFF Controller 实现 | `{Resource}RestImpl implements {Resource}Rest` | `...life.bff.{domain}.interfaces.restful` | DTO only |
| 消息监听（集成事件订阅）| `{Feature}EventHandler` | `...{domain}.interfaces.eventhandlers` | DTO |
| 异常处理 | `{Domain}ExceptionHandler`（`@RestControllerAdvice`） | `...{domain}.interfaces.config` | — |
| Job 处理 | `{Feature}JobHandler` | `...{domain}.interfaces.jobhandler` | — |
| 应用编排 | `{Resource}AppService`（`@Transactional`） | `...{domain}.application.appservice` | DO, DTO |
| 应用层写入参 | `{Resource}Command` | `...{domain}.application.vo.command` | — |
| 应用层查询入参 | `{Resource}Query` | `...{domain}.application.vo.query` | — |
| 集成事件发布器（接口定义，产生于 application）| `ApplicationEventPublisher` | `...{domain}.application.publisher` | — |
| 应用转换器 | `{Resource}Converter`（`@UtilityClass` 静态，DTO↔DO） | `...{domain}.application.converter` | — |
| 领域实体（充血）| `{Resource}DO`（**注意：DO=领域对象，非 PO**） | `...{domain}.domain.{aggregate}.model.entity` | DO only |
| 领域枚举 | `{Resource}StatusEnum` | `...{domain}.domain.{aggregate}.model.enums` | — |
| 仓储接口 | `{Resource}Repository`（接口） | `...{domain}.domain.{aggregate}.repository` | — |
| 领域服务 | `{Resource}DomainService` | `...{domain}.domain.{aggregate}.service` | DO |
| 领域事件 | `{Resource}{Changed}Event` | `...{domain}.domain.{aggregate}.event` | — |
| 防腐层定义（Facade）| `{Resource}Facade`（接口，定义于 domain；不放 application 是为防止外部服务需求的领域知识泄露到应用层）| `...{domain}.domain.facade` | — |
| 领域事件发布器（接口定义，产生于 domain）| `DomainEventPublisher` | `...{domain}.domain.publisher` | — |
| 仓储实现 | `{Resource}RepositoryImpl extends ServiceImpl<{Resource}Mapper,{Resource}PO> implements {Resource}Repository` | `...{domain}.infrastructure.persistence.repository` | DO, PO, DTO |
| 持久化对象 | `{Resource}PO`（贫血，`@TableName`） | `...{domain}.infrastructure.persistence.entity` | PO |
| 持久化转换器 | `{Resource}DataConverter` | `...{domain}.infrastructure.persistence.converter` | DO↔PO |
| Mapper | `{Resource}Mapper extends BaseMapper<PO>` | `...{domain}.infrastructure.persistence.dao` | — |
| Mapper XML | `{Resource}Mapper.xml` | `...{domain}.infrastructure.persistence.dao.xml` | — |
| 集成事件发布器实现 | `KafkaApplicationEventPublisher` | `...{domain}.infrastructure.messaging.publisher` | — |
| 领域事件发布器实现 | `KafkaDomainEventPublisher` | `...{domain}.infrastructure.messaging.publisher` | — |
| 防腐层实现（Facade）| `{Resource}FacadeImpl implements {Resource}Facade` | `...{domain}.infrastructure.facade` | — |
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

> **🔴 决策定位：** 测试反馈——icec 里 DO（领域充血对象）与 PO（持久化贫血对象）类型经常**不一致**（DO 用枚举/领域类型、PO 用 String/Long），这是**合法模式**（由 DataConverter 桥接）。但 DO/PO 类型不一致也可能是**建模错误**（如同一业务字段在两边本应一致却写错）。文档此前没给判定线，导致 §10 异常根因分类时"层4 AI犯蠢"vs"合法差异"边界模糊。本节给明确判定线。

**判定原则：DO 持领域语义类型、PO 持存储原类型，差异由 DataConverter 桥接 = 合法；同语义字段在 DO/PO 间类型不可互转（无桥接语义）= 建模错误。**

| 场景 | DO 类型 | PO 类型 | 判定 | 依据 |
|------|---------|---------|------|------|
| 状态字段 | `TicketStatus`（枚举）| `String`（VARCHAR(32)）| ✅ **合法** | 枚举↔字符串，DataConverter 用 `status.name()`/`TicketStatus.valueOf()` 桥接 |
| 金额字段 | `BigDecimal`（Money 值对象）| `decimal` | ✅ **合法** | 值对象↔数值，Converter 解包/打包 |
| 时间字段 | `java.util.Date` | `datetime` | ✅ **合法**（且必须一致用 Date）| 同类型映射，🔴 严禁 DO 用 LocalDateTime |
| 主键/外键 | `Long` | `bigint` | ✅ **合法** | 同语义；🔴 但若 DO=Long / PO=String(varchar) 则 **🔴 建模错误**（无桥接语义，主键类型应一致） |
| 软删标记 | `Boolean deleted` | `Integer deleted_flag`（tinyint）| ✅ **合法** | 布尔↔0/1，Converter 桥接 |
| 嵌套对象/聚合引用 | `SubEntity sub`（DO 内充血子对象）| 扁平字段 `sub_xxx` / 关联表 | ✅ **合法** | 防腐层做聚合根↔扁平 PO 的组装/拆解（见 §8 踩坑复盘：life STORY-020 扁平 PO 范式）|
| **任意业务字段：DO=Long / PO=String（非主键，无枚举语义）** | Long | String | 🔴 **建模错误** | 无桥接语义，类型本应一致；属层1/2/3 文档错或层4 笔误 |
| **DO 含 PO/SQL/DTO 引用** | — | — | 🔴 **分层违规**（非类型问题）| 共有§3 + §2.3 object-per-layer，Domain 禁持 PO |

**遇到类型不匹配时的判定 SOP（配合共有 §10 异常根因 4 层）：**
1. 先查差异是否属上表"✅ 合法"行 → 合法则 DataConverter 正确桥接即可，**不算根因**。
2. 不属合法行 → 进共有 §10 逐层判定：层1 Task 数据结构表类型矛盾？层2 Story 字段语义？层3 DR 业务规则？层4 AI 笔误/分层违规。
3. **主键/业务关键字段类型在 DO/PO 间不可互转 → 直接判层4**（除非 Task 明确要求异构存储，需 DR 举证）。

### §2.5 BFF 三层落点表（🆕 v1.3.0 — BFF 工程形态决策）

> **🔴 决策定位：** BFF（`{project}-bff`）是与 Service 五模块（§2.2）并列的**独立工程形态**，走**完全不同的落点规则**。
> AI 编码前必须先判工程类型：`-bff` 结尾 → 用本表；否则 → 用 §2.2 service 五模块表。**BFF 不得套用 §2.2 上半部分 service 规则。**
>
> **🔴 关键特征：BFF 不设独立 domain 层。** README 原文「application 合并 application 和 domain」——BFF 的 application 层直接承担领域职责，**不生成 `domain/` 包**（区别于 Service 的独立 domain 模块）。
>
> **规则源（不复述）：** life 项目族 14 份 BFF README（2c 6 份 + admin 8 份，全线逐字节一致）：`icec-cloud-life-{module}-bff/readme.md` + `icec-cloud-boss-{module}-bff/README.md`

| 类角色 | 命名模板 | 落点包路径（`{domain}`=业务域） | object 持有 |
|--------|---------|----------------------|-----------|
| Rest 接口实现（实现 api 契约层定义的 Rest 接口）| `{Resource}RestImpl implements {Resource}Rest` | `...{domain}.interfaces.restful` | VO, DTO |
| DTO↔VO 转换器（在 interfaces 层，不在 application）| `{Resource}Converter` | `...{domain}.interfaces.converter` | — |
| 应用服务（合并 domain，命名 `ServiceApp` 非 `AppService`）| `{Resource}ServiceApp` | `...{domain}.application.appservice` | DTO |
| 防腐层定义（在 application 层，非 domain）| `{Resource}Facade` | `...{domain}.application.facade` | — |
| 防腐层实现 | `{Resource}FacadeImpl implements {Resource}Facade` | `...{domain}.infrastructure.facade` | — |
| Feign 客户端（调 SPI）| `{Resource}ServiceClient` | `...{domain}.infrastructure.feign` | DTO |
| Web 配置（如 WebMvcConfig）| `{Resource}Config` | `...{domain}.infrastructure.config` | — |
| 启动类 | `WebApplication` | `...{domain}.`（包根） | — |

> **BFF object 持有约束：** 仅允许 VO/DTO；🔴 禁持有 PO（BFF 不触 DB）、禁持有 DO（无独立 domain 层）。

### §2.6 聚合边界判定线（🆕 v1.4.0 — 补强 §2 DDD 落点表的边界决策缺口）

> **🔴 决策定位：** §2.2 给了"类角色→包路径"落点表，§2.4 给了 DO/PO 类型差异判定线，但都没回答"**什么时候该拆聚合、聚合方法能不能传别的聚合对象**"这类边界判定问题。本节给可操作判定线（AB-1~AB-4），供 CodeAnalysis §6「要素 4 包路径映射」+ Domain 层建模决策时核对。
>
> **来源：** `ddd-architecture-coach/phase3-implementation-spec.md` AB-1~AB-4 + `spring-boot-skills/domain-driven-design` + DDD 蓝皮书/IDDD 交叉印证。

**判定总则：聚合边界 = 必须立即成立的不变量（invariant）的保护范围**，不是对象图的自然连接关系，也不是实体数量。

| 编号 | 判定线 | 决策 | 依据 |
|------|--------|------|------|
| **AB-1** | 聚合的不变量描述文本只能引用**本聚合自己的字段/方法名** | "An Aggregate must never query another Aggregate's internal state"——不变量不得跨聚合引用 | 跨聚合引用=耦合，破坏聚合独立性 |
| **AB-2** | 聚合方法参数**禁传其他聚合对象**，只能传 ID（或值对象） | 跨聚合操作在 AppService 层分别 `repository.findById(id)` 加载两个聚合后协调，**不**把聚合 A 作为参数塞进聚合 B 的方法 | 单事务内只持有一个聚合根的引用，防聚合越界修改 |
| **AB-3** | 单聚合内子实体（值对象/子实体）**超过 3-4 个** → 考虑拆分聚合 | 启发式数字：>4 子实体通常暗示不变量边界划得过宽，应拆为多个小聚合 | 小聚合=高并发+低锁竞争（DDD 蓝皮书推荐小聚合）|
| **AB-4** | 允许"故意违反单聚合单事务"（一个事务改多个聚合） | 🔴 但**必须显式 callout**：①理由（为何不能用最终一致性/领域事件）②并发风险（多聚合同时被改时的竞态）③补偿方案 | 例外须可追溯，禁止默认行为 |

**遇到"要不要拆聚合"时的判定 SOP：**
1. 先问"这些实体之间有没有**必须同时成立**的不变量？" → 有→同聚合；无→拆。
2. 同事务修改频率高 + 不变量弱 → 拆聚合 + 领域事件最终一致（AB-4 例外才同事务）。
3. 聚合方法需要别聚合的数据 → 走 Facade（§2.2 domain 定义/infrastructure 实现）取 ID，**不**直接传聚合对象（AB-2）。

> **与 §2.2 落点表的衔接：** AB-2 的"AppService 分别加载两个聚合后协调"对应 §2.2 `{Resource}AppService`（应用编排）持 DO + DTO 的职责；Facade 落 `domain.facade`（接口）/`infrastructure.facade`（实现）。本节只给"边界判定"，落点仍查 §2.2。

---

## §3 分层红线决策（icec/life 特化 → 叠加共有 §3）

> **决策定位：** 共有 coding-skill §3 给通用分层红线（Repository 禁业务逻辑等）。本节叠加 icec/life 项目**特有红线**（BFF 禁触 DB、事务只在 AppService 等）。

| 🔴 红线（icec/life 特有）| 决策 |
|------------------------|------|
| BFF 直接操作 DB | 禁。BFF 必须通过 Feign SPI 调用 Service |
| BFF 操作 Redis / Kafka | 禁（🔧 v1.3.0 精确化）。BFF 无状态，不触分布式中间件 |
| BFF 使用分布式缓存 | 禁；可用本地缓存（Caffeine）（🔧 v1.3.0，README「万不得已可用本地缓存而非分布式缓存/数据库」）|
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
| PO↔DO 转换 | 调 `{Resource}DataConverter`（infrastructure 层）| 防腐层 |
| 跨模块调用（接口型）| 调 `{Resource}Facade`（domain 定义接口）/ `{Resource}FacadeImpl`（infrastructure 实现）| 防腐层，隔离外部服务知识 |
| 跨模块调用（消息型）| 集成事件：`ApplicationEventPublisher` 发布（application 定义/infrastructure 实现），对应 `EventHandler` 在 interfaces 层订阅 | 解耦模块间强依赖 |
| 跨服务调用 | FeignClient 继承 SPI 接口 + `@HystrixCommand(fallbackMethod)` | 容错降级 |
| 仓储实现 | `extends ServiceImpl<{Resource}Mapper, {Resource}PO>` | MyBatis-Plus |
| 分页 | PageHelper（禁手写分页 SQL）| 统一分页 |
| 消息发送（触发时机） | 事务提交后 `@TransactionalEventListener(phase=AFTER_COMMIT)` 发 messagebus 事件 | 先写库后发消息；🔴 注意 AFTER_COMMIT 在原事务**之外**运行（见 §8 踩坑#17）|
| 消息发送（事件信封，🆕 v1.4.0）| messagebus `Message` 信封**缺** eventId/schemaVersion/correlationId 标准字段（仅有 id/primaryKey/topic/timeStamp）→ 业务 payload **必须自带**：`eventId`(UUID)/`occurredAt`/`schemaVersion`/可选 `correlationId`（跨服务追踪）| 核实结论：框架不提供标准信封，业务侧补齐（见 §8 踩坑#15）|
| 消息消费（幂等，🆕 v1.4.0）| messagebus 是 at-least-once 投递 + broker 侧重投（`ConsumeFailMessage.needRepush`），**框架不做去重** → 业务 EventHandler 必须自实现幂等：①去重表（`ProcessedEvent(event_id unique)` 与业务同事务，冲突即 no-op）②自然幂等（按业务键 upsert / 状态守卫忽略重放）。**判定线：涉及邮件/支付/下游事件等不可重复副作用 → 必须用去重表，不能只依赖自然幂等** | 核实结论：框架不覆盖幂等（见 §8 踩坑#15）|
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
| 构建（有根 pom，🔧 v1.2.0 修正）| 根 `pom.xml` 只做依赖管理（`dependencyManagement`）+ 插件管理 + `<modules>` 声明，**不直接定义 dependencies**；各层子模块各自 pom 声明自身依赖并独立打包成 JAR | 见 §8 踩坑#1（原"无根 pom"表述已修正）|
| 编译验证 | 父工程根目录 `mvn compile`（走根 pom 的 `<modules>` 聚合构建，依赖版本仲裁由根 pom `dependencyManagement` 统一）| — |

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

# 9. Domain 层禁框架注解污染（🆕 v1.4.0 — 防 AI 把 Spring 语法写进领域层）
grep -rn "@Service\|@Component\|@Autowired\|@Repository" \
  --include="*.java" src/main/java/**/domain/ 2>/dev/null
# 期望空；非空 = 领域层被框架污染（🔴 阻断）。依据：§2.1 domain 模块"不依赖任何其他模块"+ §3"Domain 引用 PO/DTO/SQL 禁"
```

---

## §8 icec/life 踩坑决策库（高价值"人人知但未写"的陷阱）

> **决策定位：** 这些是项目特有陷阱的**决策规避**，源自 `life.assets.md §6.9/§6.10/§10` 与历史复盘。CodeAnalysis 遇相关场景须主动核对。

| # | 陷阱 | 决策规避 |
|---|------|---------|
| 1 | **根 pom 不管依赖内容**（🔧 v1.2.0 修正；🔧 v1.4.0 补检测手段）：根 pom 只做 `dependencyManagement`+插件+`<modules>` 聚合，不声明 `dependencies`；单独在某子模块目录下跑 `mvn compile`（不走根 reactor）时，其依赖的兄弟模块须已 `mvn install` 到本地仓 | 优先走根目录 `mvn compile`（reactor 按 `<modules>` 顺序自动构建）；仅需单模块编译时，先 `mvn install` 被依赖模块到本地仓。**依赖冲突检测（🔧 v1.4.0）：** Maven `nearest wins`——最近声明胜出（非最近版本）；同一 artifact 不同版本在依赖树多路径出现时，用 `mvn dependency:tree -Dverbose` 查 `omitted for duplicate`/`managed from`；CI 加 `maven-enforcer-plugin` 的 `dependencyConvergence` 规则，冲突即 fail（禁止靠"碰巧选到能跑的版本"蒙混）|
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
| 15 | **messagebus 不保证幂等/无标准信封/无 Outbox**（🆕 v1.4.0，核实结论）：courier/messagebus 提供的是 at-least-once 投递 + broker 侧重投（`ConsumeFailMessage.needRpush`），**使重复更可能而非更少**；`Message` 信封仅 `id/primaryKey/topic/timeStamp`，缺 `eventId/schemaVersion/occurredAt/correlationId`；无 Outbox 表（`EventPublisher.publish` 同步交 broker，与业务事务不原子）。现网 `WorkTicketEventHandler`/`AbnormalTicketEventAppService.handle` 无去重且显式不重试 | ①事件信封：业务 payload 自带 `eventId`(UUID)/`occurredAt`/`schemaVersion`/可选 `correlationId`。②幂等消费：EventHandler 必须自实现——去重表（`ProcessedEvent(event_id unique)` 与业务同事务，冲突即 no-op）或自然幂等（业务键 upsert/状态守卫）；**涉及邮件/支付/下游事件等不可重复副作用 → 必须去重表**。③先写库后发消息：用 `@TransactionalEventListener(AFTER_COMMIT)` 但注意边界（见 #17）。详见 §4 消息发送行 |
| 16 | **`@Transactional` 自调用陷阱**（🆕 v1.4.0）：同类内 `this.processSingle()` 调用绕过 Spring AOP 代理，被调方法上的 `@Transactional(REQUIRES_NEW)` 等 propagation **静默失效**（不报错，事务语义直接丢失）| 自调用跨事务方法时：①注入自身代理（`@Autowired private XxxService self; self.processSingle()`）②或把方法抽到独立 bean。🔴 禁用 `AopContext.currentProxy()`（需开 `exposeProxy=true`，易遗漏）。与 Spring Boot 版本无关，1.5.7 同样适用 |
| 17 | **`@TransactionalEventListener(AFTER_COMMIT)` 在原事务之外**（🆕 v1.4.0）：AFTER_COMMIT 阶段运行在原事务提交**之后**、但**不在原事务上下文内**——若 listener 自己要写库，必须新开 `@Transactional(REQUIRES_NEW)`，不能假设还在原事务里。icec 现有"事务提交后 `@TransactionalEventListener` 发 messagebus 事件"规则正好撞此边界 | listener 内：①只发消息（无写库）→ 安全，无需额外事务。②有写库 → listener 方法加 `@Transactional(REQUIRES_NEW)` 开新事务。③listener 内禁直接查原事务未提交的数据（已提交才能查到）。与 §4"消息发送（触发时机）"行呼应 |
| 18 | **checked 异常默认不回滚**（🆕 v1.4.0）：`@Transactional` 默认只在 `RuntimeException`/`Error` 时回滚，checked 异常（如 `IOException`/自定义 checked）**不回滚**，导致部分成功数据残留 | 业务抛 checked 异常需回滚时，显式声明 `@Transactional(rollbackFor = Exception.class)`；或业务异常一律继承 `RuntimeException`（§4 已规定"自定义异常 extends RuntimeException"）|
| 19 | **容错超时预算不一致**（🆕 v1.4.0，工具无关原则）：调用方超时 < 下游超时 × 重试次数之和 → 调用方提前超时但下游仍在重试，造成"调用方已失败、下游仍在消耗资源"的资源泄漏；icec 用 Hystrix 非 Resilience4j，但此原则语言/工具无关 | 排查 Feign+Hystrix 调用链时核对：`caller_timeout > Σ(downstream_timeout × downstream_retries)`；违反即视为 bug。连接池经验公式：`pool_size ≈ cores × 2`（仅参考，按实际 QPS/RT 调）|
| 20 | **分布式锁非原子坏锁**（🆕 v1.4.0，核实发现现网反模式）：`redisTemplate.opsForValue().setIfAbsent(key,value)` + 分离 `expire(key,ttl)` 两步非原子——若 setIfAbsent 成功后进程崩溃未及 expire，key 永不过期=锁永久泄露。现网 `SmsPushRuleAppService.java:142` 即此模式 | 用 Redisson `tryLock`/`trySet`（原子 SET + TTL，单命令 `SET key val NX PX ttl`）；或注入项目已有 wrapper：life-user `RedissonDistributedLock`（短租约 caller-managed）/ life-cs `CsDistributedLock`（callback 长租约，`finally`+`isHeldByCurrentThread` 释放）。🔴 禁裸 `setIfAbsent`+分离`expire`。key 命名/雪崩防护等纯规则归 life constraints（非 java3d 职责）|

---

## §9 与共有 coding-skill 的映射（叠加覆盖声明）

> **🔴 本表是 §13 注册加载协议要求的"必含章节"**。AI 叠加应用时按本表把本适配器决策叠加到共有章节，**冲突时以本适配器为准**。

| 共有 coding-skill 章节 | 本适配器叠加章节 | 叠加内容 | 优先级 |
|----------------------|---------------|---------|--------|
| §3 分层职责红线 | §2（落点表，🔧 v1.2.0 补 Facade/事件发布器落点）+ §2.4（DO/PO 判定线）+ §2.5（BFF 三层落点表，🆕 v1.3.0）+ §2.6（聚合边界判定线 AB-1~AB-4，🆕 v1.4.0）+ §3（特化红线，BFF 数据策略拆 3 条，🔧 v1.3.0）| icec/life 精确包路径（含 §1.1bis 固化 base package）+ 防腐层 Facade（domain 定义/infrastructure 实现）+ 集成/领域事件双线 + DO/PO 合法差异 vs 建模错误判定 + 聚合边界判定（不变量引用/禁传聚合对象/拆分启发式/单聚合单事务例外）+ BFF 三层落点（合并 domain、interfaces/converter、application/facade）+ BFF禁触DB/Redis/Kafka/分布式缓存（可本地缓存）/事务只在AppService 等特化红线 | adapter > 共有 |
| §4 骨架展开规则 | §4（骨架特化，🔧 v1.4.0 补消息发送三行：触发时机/事件信封/幂等消费）| 注解选型/DataConverter形态/Facade/事件发布/Feign+Hystrix/事务/对象契约 + messagebus 事件信封缺失字段补齐 + at-least-once 幂等消费决策 | adapter > 共有 |
| §8 验证判定标准 | §5（验证特化）| JUnit4+Mockito / dev-DB+@Transactional / 根pom聚合构建（dependencyManagement，🔧 v1.2.0 修正） | adapter > 共有（§8.5 H2 被 dev-DB 覆盖）|
| §11 经验检查清单 | §6（命名/错误码）+ §8（踩坑库）+ §1.2（分布式锁禁用，🆕 v1.4.0）| icec/life 命名表 + 错误码段 + 20 项踩坑决策（🆕 v1.4.0 增 messagebus/#15、@Transactional三坑/#16-18、容错超时/#19、分布式锁坏锁/#20、充实 Maven 治理/#1）+ 分布式锁 API 选型（禁非原子 setIfAbsent+expire，应 Redisson） | adapter > 共有（§11.1 第3/4/7/10/11项被 §8 踩坑库取代）|
| §12 静态扫描规则 | §7（静态扫描特化，🔧 v1.4.0 补规则#9）| 9 条 icec 工程特化 grep（防分层/防框架误用/防 domain 层框架注解污染）| adapter > 共有（叠加在通用 §12 之上）|
| §1 CodingModel 决策 | §1（技术栈锁）+ §1.1bis（base package）| 11 维决策时锁定 Java8/SB1.5.7/messagebus 等技术前提 + 固化包路径前缀 | adapter 补充（不取消共有）|
| §2 约束文件引用 | §1（技术栈事实）| 给出 9 项约束的具体 Java+icec 事实 | adapter 补充 |
| §10 异常根因 4 层 | §2.4（DO/PO 判定线，🆕 v3.6.2）+ §2.6（聚合边界判定线，🆕 v1.4.0）| 层1/层4 判定时区分"DO/PO 合法差异"与"建模错误"；聚合越界/多聚合同事务问题用 §2.6 AB-1~AB-4 判定归属 | adapter 辅助（不覆盖共有§10 方法论）|

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
version: 1.4.0
description: Java 语言 + icec/life 项目编码决策知识层（叠加于共有 coding-skill；🆕 v1.4.0 基于参考文献增强：聚合边界判定线 + messagebus 事件设计 + @Transactional 传播陷阱 + 分布式锁反模式 + 静态扫描补充）
```

**查加载路径：** `ae-sdd plugin trace coding-adapter-java` → 应命中 L3-master: java3d-coding-skill → resolved: .../plugins/java3d-coding-skill/SKILL.md

**项目层覆盖示例（life 若要定制）：** 在 `D:\Item\life\.ae-sdd\plugins\registry.yaml` 注册同名 provides（coding-adapter-java）指向 life 自定义适配器，L1 优先级覆盖母版 L3。
