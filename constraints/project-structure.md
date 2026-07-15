# 工程结构规范

## 摘要

本文件定义 BFF、Service、SPI 工程的标准目录结构和模块职责。新建工程或新增模块时必须遵循此结构。
适用场景：新建工程、评审工程结构合理性时。

---

## 一、工程类型总览

项目中存在四种工程类型：

| 工程类型 | 命名规则 | 职责 |
| --- | --- | --- |
| SPI 模块 | `icec-cloud-life-{module}-spi` | 定义服务间调用的接口契约（接口、DTO、Request、Enum），不含实现 |
| Service 工程 | `icec-cloud-life-{module}` | 实现业务逻辑，提供 SPI 接口的具体实现，DDD 四层结构 |
| API 工程 | `icec-cloud-{system}-api` | 将多个 BFF 模块聚合为一个 Spring Boot 应用，面向同一端前端 |
| 独立 BFF 工程 | `icec-cloud-{system}-{module}-bff` | 独立部署的 BFF，面向特定前端场景 |

**API 工程 vs 独立 BFF 工程：**

| 维度 | API 工程 | 独立 BFF 工程 |
| --- | --- | --- |
| 部署方式 | 多个 BFF 模块聚合为一个 Spring Boot 应用 | 单独一个 Spring Boot 应用 |
| 适用场景 | 同一端的多个业务域（如 APP 端的认证、车型、工单） | 独立业务场景（如客服工作台） |
| 接口定义 | `*-bff-api` 子模块中的 `*Rest` 接口 | 工程内部的 `interfaces/restful` |
| 接口实现 | `*-api-service` 子模块中的 `*RestImpl` | 工程内部的 Controller |

**调用链路：**

```
前端
 └──▶ 独立 BFF 工程 ──────────────▶ SPI ──▶ Service 工程
```

**面向前端 vs 服务间接口的区别：**

| 维度 | API 工程（面向前端） | SPI（服务间） |
| --- | --- | --- |
| 接口命名 | `*Rest` | `*Service` |
| 响应格式 | `ApiResult<T>` | 直接返回业务对象；调用方需按错误码分支时使用 `Result<T>` |
| HTTP 注解 | `@PostMapping` / `@GetMapping` | `@RequestMapping(method=, value=)` |
| 数据模型 | Request / VO | Request / DTO |
| 认证 | 需要（Token 校验） | 内部调用，无需认证 |

---

## 二、独立 BFF 工程结构

独立 BFF 工程为单模块 Spring Boot 项目，面向特定前端场景独立部署。

```
icec-cloud-{system}-{module}-bff/
└── src/main/java/com/casstime/cloud/{system}/bff/{module}/
    ├── application/
    │   ├── appservice/     # 应用服务，编排多个 Service 调用
    │   └── facade/         # 防腐层接口
    ├── infrastructure/
    │   ├── feign/          # Feign 客户端，调用 Service SPI
    │   ├── config/         # 配置
    │   ├── exception/      # 异常处理
    │   └── operationlog/   # 操作日志
    └── interfaces/
        ├── restful/        # REST 控制器，对外 API
        └── converter/      # DTO 转换
```

**规则：**
- 不写核心业务逻辑，只做聚合和编排
- 调用 Service 必须通过 SPI 接口，通过 Feign 客户端调用
- 禁止直接操作数据库、Redis、Kafka
- Request / VO 定义在对应的 `*-bff-api` 子模块的 `request/` 和 `vo/` 包下

---

## 三、Service 工程结构

Service 工程为多模块 Maven 项目，采用 DDD 四层结构。

```
icec-cloud-xxx/
├── icec-cloud-xxx-domain/          # 领域层
├── icec-cloud-xxx-application/     # 应用层
├── icec-cloud-xxx-interfaces/      # 接口层
├── icec-cloud-xxx-infrastructure/  # 基础设施层
└── icec-cloud-xxx-service/         # 启动模块
```

**模块依赖方向（单向）：**

```
Service → Interfaces → Application → Domain
        ↘ Infrastructure ↗
```

### Domain 层

核心业务规则，不依赖任何其他模块，与技术无关。

```
domain/
└── {聚合名}/
    ├── model/
    │   ├── entity/       # 领域实体（DO，充血模型）
    │   ├── value/        # 值对象（查询条件等）
    │   ├── enums/        # 枚举（含错误码）
    │   └── context/      # 上下文对象
    ├── repository/       # 仓储接口定义
    ├── service/          # 领域服务
    ├── event/            # 领域事件定义
    └── exception/        # 领域异常
├── facade/               # 防腐层接口定义（实现在 Infrastructure）
└── publisher/            # 领域事件发布器接口
```

### Application 层

薄层，只做业务流程编排，不写业务规则。依赖 Domain 和 SPI。

```
application/
├── appservice/           # 应用服务，编排领域服务调用
├── converter/            # DTO 转换
├── publisher/            # 应用事件发布器接口
└── vo/
    ├── command/          # 写操作入参
    └── query/            # 读操作入参
```

### Interfaces 层

处理外部请求，协议适配。依赖 Application 和 SPI。

- 参数校验在此层完成，使用 `@Valid` + Bean Validation 注解，禁止在 Application / Domain 层重复校验
- SPI 实现类命名为 `{SpiInterfaceName}Impl`（如 `ImSessionServiceImpl`），使用 `@RestController` + `implements XxxService`
- 推荐使用 `@RequiredArgsConstructor` 构造器注入

```
interfaces/
├── restful/              # SPI 实现类（XxxServiceImpl），实现 SPI 接口
└── eventhandlers/        # 消息事件监听器
```

### Infrastructure 层

技术实现，依赖 Domain、Application 和 SPI。

```
infrastructure/
├── config/               # 配置类
├── facade/               # 防腐层实现（隔离外部服务变化）
├── feign/                # Feign 客户端（调用其他 Service SPI）
├── messaing/
│   └── publisher/        # Kafka 事件发布实现
└── persistence/
    ├── converter/        # PO ↔ DO 转换（Converter 类，显式转换）
    ├── dao/
    │   ├── mapper/       # MyBatis-Plus Mapper
    │   └── xml/          # MyBatis XML（复杂 SQL）
    ├── entity/           # 持久化对象（PO，贫血模型）
    └── repository/
        ├── mysql/        # 仓储 MySQL 实现
        └── redis/        # 仓储 Redis 实现
```

### Service 层（启动模块）

```
service/
└── src/main/
    ├── java/             # Spring Boot 启动类
    └── resources/
        └── bootstrap.yml
```

### 🔴 分层职责红线（架构腐化高发区，违反即阻断）

> **一句话总纲：Domain 写领域逻辑，Application 写业务编排，Repository 只做数据存取——三者职责不可串味。**
> 这是反复出问题的地方：领域逻辑漏进 Repository、业务编排和领域规则混在一起。每次 Story 设计、Task 拆分、Coding、CodeReview 都必须按本节核对。

**各层"必须做 / 禁止做"清单：**

| 层 | 必须做（职责） | 🔴 禁止做 |
|----|--------------|----------|
| **Domain（领域层）** | 领域逻辑：状态流转规则、业务不变量、聚合内一致性校验、领域计算。充血模型，行为写在实体/领域服务里 | 禁止依赖 Repository 实现细节、禁止写编排（不调用多个外部服务串流程）、禁止出现 PO/DTO/SQL |
| **Application（应用层）** | 业务编排：调用领域服务/聚合、组织事务边界、协调多个 Repository 与 SPI、DTO↔DO 转换、发布应用事件 | 🔴 禁止写领域规则（状态能否流转、金额怎么算属于 Domain）、禁止写 SQL/持久化细节、禁止写参数格式校验（在 Interfaces 层） |
| **Repository（仓储，实现在 Infrastructure）** | 纯数据存取：增删改查、PO↔DO 转换、拼装查询条件、缓存读写 | 🔴 **禁止写任何业务逻辑或领域逻辑**：不做状态流转判断、不做业务规则校验、不做跨聚合编排、不在存取里塞 if-业务分支。仓储方法语义只能是"存/取某个聚合"，不能是"处理某项业务" |
| **Interfaces（接口层）** | 协议适配、参数校验（@Valid）、调用 Application | 禁止写业务编排和领域规则 |

**判定口诀（写代码/审代码时自问）：**
- 这段代码是"业务规则/能不能/算什么"→ 属于 **Domain**。
- 这段代码是"先做A再做B、协调谁调谁、事务从哪到哪"→ 属于 **Application**。
- 这段代码是"把数据存进去/取出来/转个格式"→ 属于 **Repository**。
- 一个仓储方法名若像 `handleXxx`/`processXxx`/`doBusinessXxx` → 八成放错层了，仓储应是 `findByXxx`/`save`/`updateStatus` 这类存取语义。

**正例 / 反例：**

```
❌ 反例（领域逻辑漏进 Repository）：
   TicketRepositoryImpl.closeIfTimeout(id) {
       Ticket t = mapper.selectById(id);
       if (t.getStatus() == DOING && isTimeout(t)) {  // ← 业务规则写在仓储！
           t.setStatus(CLOSED);
           mapper.updateById(t);
       }
   }

✅ 正例（职责归位）：
   // Domain：领域规则
   Ticket.canTimeoutClose() { return status == DOING && isTimeout(); }
   Ticket.timeoutClose()   { this.status = CLOSED; }
   // Application：编排 + 事务边界
   TicketAppService.closeTimeout(id) {
       Ticket t = ticketRepository.findById(id);   // 取
       if (t.canTimeoutClose()) { t.timeoutClose(); // 领域判断+变更
           ticketRepository.save(t); }              // 存
   }
   // Repository：只存取
   TicketRepositoryImpl.findById / save  // 无任何 if-业务分支
```

### DDD 分层约定

**各层可出现的对象：**

| 层 | 允许出现的对象 | 说明 |
| --- | --- | --- |
| Domain | DO | 充血模型，禁止出现 PO / DTO |
| Application | DO、DTO | 无专属核心对象 |
| Interfaces | DTO | 禁止出现 DO / PO |
| Infrastructure | DO、PO、DTO | 负责各层对象之间的转换 |

**对象命名约定：**

| 对象 | 后缀 | 说明 |
| --- | --- | --- |
| 领域对象 | DO | 充血模型，含业务行为 |
| 持久化对象 | PO | 贫血模型，与数据库表对应 |
| 数据传输对象 | DTO | 用于 interfaces 层和 SPI |
| 写操作入参 | Command | application 层入参 |
| 读操作入参 | Query | application 层入参 |

**对象转换规约：**
- 禁止使用 MapStruct 做对象转换
- 统一使用 Converter 类显式转换，命名为 `{Entity}Converter`，方法为静态方法
- DO ↔ DTO 转换：在 Application 层的 `converter/` 包下定义
- PO ↔ DO 转换：在 Infrastructure 层的 `persistence/converter/` 包下定义
- 示例：
  ```java
  @NoArgsConstructor(access = AccessLevel.PRIVATE)
  public class AbnormalRuleConverter {
      public static AbnormalRuleDTO toDTO(AbnormalRule domain) { ... }
      public static List<AbnormalRuleDTO> toDTOs(List<AbnormalRule> domains) { ... }
  }
  ```

**层间调用规则：**
- 严格单向依赖：interfaces → application → domain，不可跨层调用，同层不可互相调用
- infrastructure 层实现各层抽象，各层不可直接调用 infrastructure 层
- 聚合间通信：通过领域事件解耦，不直接调用

**防腐层（ACL）：**
- 定义在 domain 层（领域知识不应泄露到 application 层）
- 实现在 infrastructure 层

**事件规范：**

领域事件（内部事件）：

| 环节 | 所在层 |
| --- | --- |
| 定义 | domain 层 |
| 产生 | domain 层 |
| 发送接口 | domain 层 |
| 发送实现 | infrastructure 层 |
| 订阅 | interfaces 层 |

集成事件（应用事件）：

| 环节 | 所在层 |
| --- | --- |
| 定义 | SPI 的 event 包 |
| 产生 | application 层 |
| 发送接口 | application 层 |
| 发送实现 | infrastructure 层 |
| 订阅 | interfaces 层 |

---

## 四、API 工程结构

API 工程将多个 BFF 模块聚合为一个 Spring Boot 应用，面向同一端前端提供统一入口。统一定义在 `icec-cloud-{system}-api` 工程中。

**包含：**
- `api-common`：统一响应 `ApiResult<T>`、全局异常处理、加解密、工具类
- `{biz}-bff-api`：各业务域的接口定义（`*Rest`）、Request、VO，不含实现
- `api-service`：实现所有 `*Rest` 接口、Feign 客户端、Spring Boot 启动类

**规则：**
- bff-api 子模块只定义接口和数据模型，不含任何实现
- api-service 通过 Feign + SPI 调用 Service，不直接连数据库
- 启动类排除 `DataSourceAutoConfiguration`，`@EnableFeignClients` 必须指定 `basePackages`

---

## 五、SPI 工程结构

SPI 统一定义在 `icec-cloud-life-spi` 工程中，是 Service 对外暴露的接口契约。

**包含：**
- Service 对外接口定义（供 BFF 或其他 Service 通过 Feign 调用）
- DTO 定义
- 领域事件 / 集成事件定义

**规则：**
- BFF 调用 Service 必须通过 SPI 接口，不得直接依赖 Service 内部模块
- Service 间同步调用必须通过对方的 SPI 接口
