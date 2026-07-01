---
name: be-coding-plan-template
description: BE Code Plan 模板 — Coding 阶段 ④bis 产出物。基于"项目资产"分层归类 + 编排 Task 执行顺序 + 输出类骨架（不写完整实现）。含 3 个 Tier 分级（Bug 修复/增量开发/全新模块）和 15 条门禁（含判定 SOP，🆕 v3.4.0 G-CODEPLAN-SRC 源码核对）。
---

# BE Code Plan 模板

> **本质：** Coding 阶段的施工蓝图。**实现细节在 Task 里**，Code Plan 只做 **Task 编排 + 类骨架 + 方法级逻辑说明 + 目录对应**。
>
> **使用：**
> 1. 落地路径由 `document-storage.resolve_path(intent="CODING_PLAN", workItemId={WORKITEM-ID}, storyId={STORY-ID?})` 推导（见 document-storage §1.3），不硬编码
> 2. 选 Tier（见 §0.5）
> 3. 逐节填值（**禁止留空**；无则填 N/A 并说明）
> 4. 通过 10 条门禁后进入 ⑤ Coding

---

## 0. 元信息

| 字段 | 值 |
|------|---|
| STORY-ID | `{STORY-ID}` |
| Story 标题 | `{标题}` |
| Story 版本 | `v{N}` |
| 涉及工程 | `{工程1, 工程2, ...}`（与项目资产 §2 微服务清单对齐） |
| 生成时间 | `{YYYY-MM-DD HH:mm}` |
| 作者 | AI（Claude Code） |
| 前置依赖 | Task 实现方案 ✅ / 测试用例 ✅ / 项目资产 ✅ / 约束文档 ✅ |

### 0.5 Tier 选择（🔴 强制，未选 Tier 视为 Plan 未开始）

| Tier | 适用场景 | 必填章节 | 必省章节 |
|------|---------|---------|---------|
| **Tier 1** | 5 行 Bug 修复 / 单方法调整 | §0, §1, §2, §5, §9, §10 | §3, §4, §6, §7, §8, §11, §12, §13, §14, §15 |
| **Tier 2** | 增量功能（单 Task 或 ≤3 Task） | §0-§10, §11, §15 | §12, §13, §14 |
| **Tier 3** | 全新模块（≥4 Task，含新表/新 SPI/新 BFF） | **全部 16 节** | 无 |

**当前 Plan Tier：** `{Tier 1 / Tier 2 / Tier 3}`

---

## 1. 项目资产引用块（🔴 门禁 0：缺资产 = 整 Plan 打回）

> 本节是 Plan 的"事实基准"。**所有 §2-§14 的事实陈述必须能从本节指向的项目资产章节找到出处**。

| 字段 | 值 |
|------|---|
| 项目资产路径 | `{资产根}/{workspaceKey}.assets.md`（+ 工程级子文件，见 document-storage §2.3）|
| 本次引用的项目资产章节 | `{§3 抽象分层映射, §4 DDD 内部分层落点, §5 命名约定, §6 工程约束, §7 契约入口}` |
| 项目资产已就绪 | ✅ / ❌（❌ 时**必须先**走项目资产 §9 探查 SOP 构建，禁止跳过） |
| 项目资产版本/审计时间 | `{lastAuditedAt}` |

---

## 2. 抽象分层 → 项目分层映射表

> **行 = 抽象 4 层 + 可选 2 类；列 = 本项目对应的工程模块 / 包路径 / 典型类名 / 职责落点。**
> **包路径禁止写"待定/TBD"**；必须能在项目资产 §4 找到对应模式。

| 抽象层 | 本项目工程模块 | 精确包路径 | 典型类名 | 职责落点（本层放什么 / 不放什么） |
|--------|--------------|-----------|---------|----------------------------------|
| 请求处理（Interfaces） | `{module}-interfaces` | `com.casstime.cloud.{xxx}.{yyy}.interfaces.restful` | `{Resource}RestImpl` | 仅协议适配；**不写**业务规则 |
| 业务编排（Application） | `{module}-application` | `com.casstime.cloud.{xxx}.{yyy}.application.appservice` | `{Resource}AppService` | 事务、编排、调 Domain 顺序；**不写**业务规则 |
| 领域逻辑（Domain） | `{module}-domain` | `com.casstime.cloud.{xxx}.{yyy}.domain.{boss}.{xxx}.model.entity` | `{Resource}DO` | 充血、业务方法不以 get/set 开头 |
| 基础能力（Infrastructure） | `{module}-infrastructure` | `com.casstime.cloud.{xxx}.{yyy}.infrastructure.persistence.repository.mysql` | `{Resource}RepositoryImpl` | 仅存取语义：`findByXxx / save / update` |
| 跨模块 SPI（可选） | `{project}-spi/{module}-spi` | `com.casstime.cloud.spi.{user}.service` | `{Resource}Service` | Feign 接口；`ServiceProviderConstants` |
| BFF 入口（可选） | `{module}-bff` | `com.casstime.cloud.bff.{user}.interfaces.restful` | `{Resource}RestImpl` | BFF 控制器；**必须**经 Facade 调 Feign |

**判定口诀：**
- 业务规则（状态机/不变量/聚合一致性）→ **Domain**
- 协调谁调谁（事务/顺序/跨域）→ **Application**
- 存取数据（findByXxx/save/update）→ **Repository / Infrastructure**
- 接 HTTP / 协议适配 → **Interfaces**
- 跨服务契约 → **SPI**
- BFF 场景 → **BFF 入口**

**边缘案例判定（🔴 补强）：**
- 状态机（业务规则核心）→ **Domain**（写在 `domain/.../service/{Resource}DomainService` 或 `entity/{Resource}DO.transition()`）
- 跨聚合事务（协调多聚合）→ **Application**（写在 `appservice/{Resource}AppService` 的 `@Transactional` 方法）
- 缓存读（带业务策略如"先查缓存再查 DB"）→ **Application**（业务编排的一部分）
- 全局唯一性校验（需查 DB）→ **Domain**（写在 DomainService，因这是聚合不变量）

---

## 3. Task 执行顺序编排

> **不重写 Task 细节。** 从 `{STORY-ID}-Task实现方案.md` 抽出 Task 列表，按依赖关系画执行顺序。

| # | Task ID | 名称 | 涉及工程-层 | 前置依赖 | 状态 |
|---|---------|------|------------|----------|------|
| 1 | Task-0 | 公共依赖说明 | - | - | Planned |
| 2 | Task-1 | `{名称}` | `{module}-domain` | - | Planned |
| 3 | Task-2 | `{名称}` | `{module}-infrastructure` | Task-1 | Planned |
| 4 | Task-3 | `{名称}` | `{module}-application` | Task-1, Task-2 | Planned |
| 5 | Task-4 | `{名称}` | `{module}-interfaces` | Task-3 | Planned |
| 6 | Task-5 | `{名称}` | `{module}-bff` | Task-4 | Planned |
| 7 | Task-6 | `{名称}` | `各 module test` | Task-1 ~ Task-5 | Planned |

**执行顺序原则：**
1. Domain（充血模型）→ Infrastructure（PO/Mapper/Repository）→ Application（AppService + Converter）→ Interfaces（RestImpl）→ BFF → Test
2. 每步可独立 `mvn compile`
3. 优先做 Task-0（公共依赖：枚举、错误码、Facades）

---

## 4. 文件级实现顺序

> 每个文件必须有"前置必须先写"和"完成后验证"。**禁止一次性写完所有文件再编译**。

| # | 文件路径 | 类型 | 前置必须先写 | 完成后必须通过的验证 |
|---|---------|------|------------|-------------------|
| 1 | `{module}-domain/.../{Resource}DO.java` | 新增 | - | `mvn -pl {module}-domain compile` |
| 2 | `{module}-infrastructure/.../{Resource}PO.java` | 新增 | Task-0 枚举 | `mvn -pl {module}-infrastructure compile` |
| 3 | `{module}-infrastructure/.../{Resource}Mapper.java` | 新增 | #2 | `mvn -pl {module}-infrastructure compile` |
| 4 | `{module}-infrastructure/.../{Resource}DataConverter.java` | 新增 | #1, #2 | 单元测试通过 |
| 5 | `{module}-infrastructure/.../{Resource}RepositoryImpl.java` | 新增 | #3, #4 | 仓储方法可被调用 |
| 6 | `{module}-application/.../{Resource}AppService.java` | 新增 | #1, #5 | 单元测试通过（mock Repository） |
| 7 | `{module}-application/.../{Resource}Converter.java` | 新增 | #1 | - |
| 8 | `{module}-interfaces/.../{Resource}RestImpl.java` | 新增 | #6 | `mvn -pl {module}-interfaces compile` |
| 9 | `{module}-bff/.../{Resource}{Action}AppService.java` | 新增 | #8 | - |
| 10 | `{module}-bff/.../{Resource}RestImpl.java` | 新增 | #9 | `@SpringBootTest + TestRestTemplate` 真实 HTTP 集成测试通过 |
| 11 | 各 module test | 新增 | #1-#10 | `mvn test` 全绿 |

---

## 5. 关键类骨架

> **每个类一张子卡**：类签名 + 所在层 + 包路径（步骤 4 已定） + 核心字段 + 核心方法签名 + 方法伪代码（10-30 行）。
>
> **🆕 v3.4.0 G-CODEPLAN-SRC 源码核对（强制）：** 每个新增/修改的类骨架，必须附**来源标记**之一：
> - `【已读源码：{相对路径}】` — 已核对现有同类源码的建模范式（命名/字段/注解/Converter 写法/PO 映射/测试范式/依赖注入）；标记的文件必须真实存在
> - `【待核实源码】` 或 `【待核实源码：{待核对项}】` — 未核对，须补读现有同类源码后改为【已读源码：】
>
> **判定标准**（"现有同类源码"= 同包同类 / 同职责类 / Converter·PO·DO 同类型）。待核实清单非空 → CodingPlan 视为草案，禁止进 ⑤ Coding。
>
> **方法伪代码分级：**
> - 简单方法（单一职责）≤ 10 行
> - 中等方法（含分支/循环）≤ 20 行
> - 复杂方法（多分支/异常处理重）≤ 40 行（**需 @复杂 标注**，触发用户确认）
>
> **伪代码格式：** "1. 校验入参非空；2. 调 repository.findByXxx；3. 若结果空抛 NotFoundException；4. 调 converter.toDTO；5. 返回"
> 每步以**动词开头**（校验/查询/转换/返回/抛异常/组装/调用）。
> **禁止贴完整方法体**（完整方法体是 ⑤ Coding 的事）。

### 5.1 `{Resource}DO.java`（Domain 层）

| 字段 | 值 |
|------|---|
| 类签名 | `@Data public class {Resource}DO implements Serializable` |
| 所在层 | Domain |
| 包路径 | `com.casstime.cloud.{xxx}.{yyy}.domain.{boss}.{xxx}.model.entity` |
| 核心字段 | `{id: Long, name: String, status: StatusEnum, createdDate: Date, ...}`（一句话说明每个） |
| 充血方法 | `transitionTo(NewStatus) {校验 from→to 是否合法；改 status；记录领域事件}` |
| 🆕 源码核对 | `【已读源码：domain/.../model/entity/ExistingDO.java】` 或 `【待核实源码：DO 建模范式】` |

**伪代码（@简单 ≤ 10 行）：**
```
1. 校验当前 status 是否允许流转到 newStatus（白名单）
2. 校验业务不变量 X
3. 设置 this.status = newStatus
4. 注册领域事件 ResourceTransitionedEvent（不入参）
5. 返回 this
```

### 5.2 `{Resource}AppService.java`（Application 层）

| 字段 | 值 |
|------|---|
| 类签名 | `@Service @RequiredArgsConstructor @Slf4j public class {Resource}AppService` |
| 所在层 | Application |
| 包路径 | `com.casstime.cloud.{xxx}.{yyy}.application.appservice` |
| 核心字段 | `private final {Resource}Repository repository; private final {Resource}Converter converter;` |
| 核心方法 | `public {Resource}DTO create(CreateCommand cmd) {@Transactional}` |

**伪代码（@中等 ≤ 20 行）：**
```
1. 校验入参 cmd 必填字段（@Valid 已在 Controller 跑过，AppService 补业务校验）
2. 调 cmd.toDO() 转换
3. 调 domainService.validateBusinessRules(do)  // 跨聚合校验
4. 调 repository.save(do)
5. 若需发 MQ 事件：applicationEventPublisher.publishEvent(do.pullEvent())  // 事务内不调 Feign，事件发布 OK
6. 调 converter.toDTO(savedDO)
7. 返回 dto
异常：捕获 DomainException → 直接抛；其他异常 → 包装为 AppException(code=...)
```

### 5.3 `{Resource}RestImpl.java`（Interfaces 层）

| 字段 | 值 |
|------|---|
| 类签名 | `@RestController @RequestMapping("/api/v1/{resources}") @Api(tags = "...") public class {Resource}RestImpl implements {SpiInterface}` |
| 所在层 | Interfaces |
| 包路径 | `com.casstime.cloud.{xxx}.{yyy}.interfaces.restful` |
| 核心方法 | `public ApiResult<{Resource}DTO> create(@Valid @RequestBody CreateRequest request)` |

**伪代码（@简单 ≤ 10 行）：**
```
1. 调 appService.create(request.toCommand())
2. 包装 ApiResult.success(dto)
3. 返回
```

### 5.4 `{Resource}RepositoryImpl.java`（Infrastructure 层）

| 字段 | 值 |
|------|---|
| 类签名 | `@Repository @RequiredArgsConstructor public class {Resource}RepositoryImpl extends ServiceImpl<{Resource}Mapper, {Resource}PO> implements {Resource}Repository` |
| 所在层 | Infrastructure |
| 包路径 | `com.casstime.cloud.{xxx}.{yyy}.infrastructure.persistence.repository.mysql` |
| 仓储方法 | `findById(Long id) / save({Resource}DO do) / update({Resource}DO do) / updateStatus(Long id, StatusEnum status)` |

**伪代码（@简单 ≤ 10 行）：**
```
1. 调 mapper.selectById(id) → PO
2. 若 PO == null → 返回 null
3. 调 dataConverter.toDO(po)
4. 返回 do
```

### 5.5 `{Resource}{Action}AppService.java`（BFF 层，可选）

| 字段 | 值 |
|------|---|
| 类签名 | `@Service @RequiredArgsConstructor @Slf4j public class {Resource}{Action}AppService` |
| 所在层 | BFF |
| 包路径 | `com.casstime.cloud.bff.{user}.application.appservice` |
| 核心字段 | `private final {Resource}Facade facade;` |
| 核心方法 | `public ApiResult<{Resource}VO> create(@Valid CreateRequest request)` |

**伪代码（@简单 ≤ 10 行）：**
```
1. 调 request.toCommand() 转换
2. 调 facade.executeCreate(command)  // Facade 封装 Feign 调用，异常返回 null/Result.error
3. 若 facade 返回 null → 抛 BusinessException(USER_NOT_FOUND)
4. 调 converter.toVO(result)
5. 包装 ApiResult.success(vo)
6. 返回
```

---

## 6. 数据结构 / 全链路映射照表（🔴 按项目资产分层结构）

> **🔴 设计原则（2026-06-05 修订）：** 模板**不写死**全链路的"层"（哪些层、层序、命名、命名风格）—— 这些是**项目特化结构**，必须从**项目资产**读取。
>
> **🔴 强制：** 本节内容**必须**先读取项目资产获取分层结构，再按结构逐层填表。
> 引用项目资产：`{projectKey}.assets.md §3 抽象分层 → 项目分层映射表`（项目特定分层） + `§4 DDD 内部分层落点`（精确包路径）。
> **禁止**在模板里写死"5 层"（触入 HTTP / BFF 接收 / SPI DTO / DO / DB 列）或其他具体层数。
>
> **本节目的：** 展示**每个字段跨所有"项目分层"的完整映射**（含命名/类型/特殊处理），按调用链路自上而下溯源。

### 6.0 引用项目资产获取分层结构

```
【本节填写前置动作】

1. 读取项目资产 {projectKey}.assets.md：
   - §3 抽象分层 → 项目分层映射表（行=抽象层，列=本项目对应工程模块）
   - §4 DDD 内部分层落点（行=类角色，列=精确包路径）

2. 从 §3 抽出本项目实际分层（自上而下）：
   例（来自 icec-cloud-boss 项目 §3）：
   ┌──────────────────────────────────────────────────────┐
   │ 抽象层          │ 本项目对应工程模块                  │
   ├──────────────────────────────────────────────────────┤
   │ 用户入口         │ icec-cloud-*-bff                  │
   │ 跨模块契约       │ icec-cloud-life-spi/*-spi         │
   │ 请求处理         │ icec-cloud-*-*-interfaces         │
   │ 业务编排         │ icec-cloud-*-*-application         │
   │ 领域逻辑         │ icec-cloud-*-*-domain             │
   │ 基础能力         │ icec-cloud-*-*-infrastructure     │
   │ 测试             │ 各模块 src/test                   │
   │ 文档             │ docs/, .auto-engineering/         │
   └──────────────────────────────────────────────────────┘
   ⚠️ 其他项目分层可能不同（简单项目可能只有 3 层：Controller/Service/Repository）

3. 用本项目实际分层替换 §6.1 表头和 §6.3 审计字段分层
```

**特殊处理标记（不依赖项目分层，跨项目通用）：**
- `(丢弃)` — 该层不再保留此字段
- `(路由用)` — 该层使用此字段但不持久化
- `(签名验用)` — 仅用于签名验证，不进数据流
- `(透传可选)` — 可选透传
- `(UNIQUE)` — 唯一索引

### 6.1 全链路映射照表（主表）

**🔴 强制：每行一个字段跨本项目所有"层"的完整映射**，格式：

| 第 1 层（按 §3） | 第 2 层（按 §3） | ... | 倒数第 2 层（按 §3） | 最后一层（按 §3） | 备注 |
|---|---|---|---|---|---|
| 字段名 (类型) | 字段名 (类型) | ... | 字段名 (类型) | `snake_case` (类型) | 特殊处理标记 |

> **表头填写：** 按本项目 §3 实际分层顺序填表头（最右一列固定为 DB 列，因为 DDL 是项目必有的）。

### 6.2 实例：icec-cloud-boss 项目 IM 消息全链路映射（🔴 参考实例）

> **🔴 此节是实例（icec-cloud-boss 项目 IM 消息域），不是模板。**
> 其他项目按本项目 §3 实际分层填写。Icec-cloud-boss 项目有 8 个调用层（§3），IM 消息字段跨这 8 层展示映射规则。

| 触入 HTTP | BFF 接收 | SPI DTO | ImMessageDO | DB 列 | 备注 |
|----------|----------|---------|-------------|-------|------|
| `channelType` (String) | `channelType` (String) | `channelType` (String) | `(路由用，不落库)` | `channel_type` (varchar(32)) | SPI 透传；DO 不存；DB 作路由索引 |
| `fromUserId` (Long) | `fromUserId` (Long) | `senderVendorUserId` (String) | `senderVendorUserId` (String) | `sender_vendor_user_id` (varchar(64)) | SPI 重命名（业务字段） |
| `toUserId` (Long) | `toUserId` (Long) | `recipientVendorUserId` (String) | `(路由用)` | `(路由用)` | DO/DB 都不存（路由参数）|
| `msgTimestamp` (Long) | `msgTimestamp` (Long) | `sentAt` (Long) | `sentAt` (Date) | `sent_at` (datetime) | 类型转换 Long → Date；命名 sentAt |
| `timestamp` (Long) | `timestamp` (Long) | `timestamp` (Long) | `-` | `(签名验用)` | 签名验证专用，不落库 |
| `objectName` (String) | `objectName` (String) | `msgType` (String) | `messageType` (MessageTypeEnum) | `message_type` (varchar(32)) | DO 转为枚举；DB 作分类索引 |
| `content` (String) | `content` (String) | `content` (String) | `contentPayload` (String) | `content_payload` (text) | DO 包装为 contentPayload 业务字段 |
| `msgUID` (String) | `msgUID` (String) | `sourceEventId` (String) | `sourceEventId` (String) | `source_event_id` (varchar(64) **UNIQUE**) | 唯一索引；SPI 重命名 |
| `sensitiveType` (String) | `(丢弃)` | `-` | `-` | `-` | BFF 层丢弃（敏感词过滤结果不入库） |
| `source` (String) | `(丢弃)` | `-` | `-` | `-` | BFF 层丢弃（消息来源类型）|
| `msgRandom` (Long) | `(丢弃)` | `-` | `-` | `-` | BFF 层丢弃（融云/融云随机数）|
| `clientIp` (String) | `(丢弃)` | `-` | `-` | `-` | BFF 层丢弃（仅审计日志用）|
| `appKey/nonce/signature` | `(透传可选)` | `(签名验用)` | `-` | `-` | 仅签名验证；不入数据流 |

### 6.3 审计字段（与本项目分层对齐）

> **按本项目 §3 实际分层填表头**，DB 列遵循项目资产 §6.5（审计四字段 + deleted_flag）。

| 第 1 层 | 第 2 层 | ... | DO | DB 列 | 备注 |
|---|---|---|---|---|---|
| `-` | `-` | ... | `createdBy: String` | `created_by` (varchar(32)) | 审计字段（项目资产 §6.5） |
| `-` | `-` | ... | `createdDate: Date` | `created_date` (datetime DEFAULT CURRENT_TIMESTAMP) | 审计字段 |
| `-` | `-` | ... | `lastUpdatedBy: String` | `last_updated_by` (varchar(32)) | 审计字段 |
| `-` | `-` | ... | `lastUpdatedDate: Date` | `last_updated_date` (datetime ON UPDATE CURRENT_TIMESTAMP) | 审计字段 |
| `-` | `-` | ... | `deletedFlag: Boolean` | `deleted_flag` (tinyint(1) DEFAULT 0) | 逻辑删除（如需） |

### 6.4 字段命名转换规则（与项目资产 §6.5 一致）

| 转换类型 | 规则 | 示例 |
|---------|------|------|
| camelCase → snake_case | 全小写 + 下划线 | `senderVendorUserId` → `sender_vendor_user_id` |
| 类型转换 | 时间戳/枚举/Decimal | `sentAt: Long` → `sent_at: datetime`；`status: StatusEnum` → `status: tinyint` |
| 命名重命名 | 业务字段语义调整 | `fromUserId` → `senderVendorUserId`（语义更准确） |
| 字段丢弃 | `(丢弃)` 标记 | `sensitiveType` 在 BFF 层丢弃 |
| 路由字段 | `(路由用)` 标记 | `channelType` 在 DO 层路由用但不落库 |

### 6.5 门禁

- 🔴 **表头必须与本项目 §3 实际分层一致**（禁止模板默认 N 层）
- 🔴 **每行"项目分层数"个字段齐全**（或显式标 `(丢弃)` / `-` / `(路由用)` 等）
- 🔴 **业务字段不允许跳过** — 至少 1 行完整的"第 1 层 → DB 列"映射
- 🔴 **审计字段单独成节**（§6.3），不与业务字段混在一起
- 🔴 **类型转换必须有显式说明**（如 `sentAt: Long` → `sentAt: Date`）
- 🔴 **DB 列必须符合项目资产 §6.5**（审计四字段、deleted_flag、主键类型、索引）
- 🔴 **§6.1 字段数 ≥ 业务实际字段数**（与 Story/PRD 数据模型章节对齐）

---

## 7. Mapper / Repository 关键 SQL

> **关键 UPDATE 必须明确 WHERE 条件。** 复杂 SQL 给出 EXPLAIN 验证步骤。

| 操作 | Mapper 方法 | 关键 SQL 与 WHERE 条件 | 乐观锁 | 备注 |
|------|------------|---------------------|-------|------|
| INSERT | `insert(PO)` | MyBatis-Plus `default` | - | 自动填充 created_by/created_date |
| SELECT_BY_ID | `selectById(id)` | `WHERE id = #{id} AND deleted_flag = 0` | - | 逻辑删除过滤 |
| UPDATE_STATUS | `update(PO, Wrapper)` | `SET status = #{status} WHERE id = #{id} AND status = #{oldStatus}` | ✅ `status = oldStatus` | 乐观锁防并发 |
| DELETE | `deleteById(id)` | 逻辑删除：`UPDATE SET deleted_flag = 1 WHERE id = #{id}` | - | 优先逻辑删除 |

**EXPLAIN 验证步骤（如复杂查询）：**
```sql
EXPLAIN SELECT ... FROM {table} WHERE {复杂条件};
-- 确认走索引、避免全表扫描
-- 索引覆盖度检查
```

---

## 8. 测试用例对应

> **测试数据必须可追溯到 Story/Task 章节**（"假设 userId=1L 就能跑" = 门禁不通过）。
> **真实 DB/HTTP 判定：** 核心落库 / 事务回滚 / 分布式锁 / Feign 调用 / Redis 缓存失效 这 5 类**必须**真实 DB 或真实 HTTP。

| AC ID | 测试类 | 测试方法 | 测试数据来源 | Mock 范围 | 真实 DB | 真实 HTTP | 核心标识 |
|-------|--------|---------|------------|----------|---------|-----------|---------|
| AC-001 | `{Resource}AppServiceTest` | `create_Success` | Story §X.Y 行 N | Repository mock | ❌ | ❌ | - |
| AC-001 | `{Resource}IT` | `create_Success_RealDB` | Story §X.Y 行 N | 无 | ✅ | ❌ | 🔴 核心落库 |
| AC-002 | `{Resource}ControllerIT` | `create_Success_HTTP` | Story §X.Y 行 N | Feign mock | ✅ | ✅ | 🔴 核心接口 |
| AC-003 | `{Resource}AppServiceTest` | `create_StatusInvalid_ThrowsDomainException` | Task §Y.M | Repository mock | ❌ | ❌ | - |

**强制真实场景清单（🔴 必标）：**
1. 核心落库（涉及资金/状态/权限的 INSERT/UPDATE/DELETE）→ ✅ 真实 DB
2. 事务回滚（验证 @Transactional 边界）→ ✅ 真实 DB
3. 分布式锁（如 Redisson）→ ✅ 真实 Redis
4. Feign 调用链路 → ✅ 真实 HTTP（@SpringBootTest + TestRestTemplate）
5. Redis 缓存失效 → ✅ 真实 Redis

---

## 9. 编译与测试验证点

> **至少 5 档验证：** 单文件 / 单层 / 单 Task / 全量 / 真实 HTTP。

| 阶段 | 触发时机 | 验证命令 | 通过标准 |
|------|---------|---------|---------|
| 单文件编译 | 写完一个 .java 文件 | `mvn -pl {module} compile` | exit 0 |
| 单层编译 | 写完一个 DDD 分层（如 -domain） | `mvn -pl {module}-{layer} compile` | exit 0 |
| 单 Task | 完成一个 Task 的所有文件 | `mvn -pl {module} test -Dtest={Resource}Test` | 全绿 |
| 全量编译 | ⑤ Coding 末 | `mvn clean install -DskipTests` | exit 0 |
| 全量测试 | ⑤ Coding 末 | `mvn test` | 全绿 |
| 真实 HTTP | BFF 完成时 | `mvn -pl {bff-module} test -Dtest={Resource}IT` | 真实 HTTP 集成测试通过 |
| 服务启动 | 全部写完后 | `java -jar {bff-module}/target/*.jar` | 启动成功 + 注册到 Nacos |

---

## 10. 调试与回滚方案

> **至少覆盖 5 种失败类型：** 编译失败 / 单测失败 / 集成测试失败 / 真实 HTTP 失败 / 性能问题。

| 失败类型 | 定位方法 | 回滚策略 |
|---------|---------|---------|
| 编译失败 | IDE 跳转 / mvn 报错行 | 修正代码（无回滚） |
| 单测失败 | `mvn test -Dtest={X}` + 报错 stack | 修正测试数据/Mock 范围；不得"修复测试"代替"修复代码" |
| 集成测试失败 | `mvn test -Dtest={X}IT` + 真实 DB 输出 | 修正数据初始化 SQL 或回滚 DDL |
| 真实 HTTP 失败 | curl 调试 + Nacos 检查 | 检查 Feign 服务名 / URL / 鉴权头 |
| 性能问题（慢 SQL） | `EXPLAIN` + 慢查询日志 | 加索引 / 改写 SQL / 分页优化 |
| 事务回滚异常 | `@Transactional` 边界检查 | 调整事务粒度；**事务内禁远程调用** |

---

## 11. 约束合规自审（🔴 必填，10 条门禁的合规对照）

> 逐条对照项目资产 §6 工程约束 9 类。

| 约束项 | 规范要求 | 实现方式 | 是否合规 | 证据 |
|--------|---------|---------|---------|------|
| 分层架构 | BFF 禁直连 DB | BFF AppService 调 Facade → Feign | ✅ | `BFF/.../FacadeImpl.java:行号` |
| 分层架构 | Service 间禁直连 | 走 SPI Feign | ✅ | `Service/.../feign/CsUserClient.java:行号` |
| 工程结构 | 业务规则在 Domain | `{Resource}DO.transitionTo` | ✅ | `Domain/.../{Resource}DO.java:行号` |
| 工程结构 | Repository 只存取 | `findByXxx / save / update` | ✅ | `Infra/.../RepositoryImpl.java:行号` |
| 代码风格 | 时间字段用 Date | `private Date createdDate` | ✅ | `PO.java:行号` |
| 代码风格 | 事务在 AppService | `@Transactional` | ✅ | `AppService.java:行号` |
| 代码风格 | 事务内禁远程 | 业务编排无 Feign 调用 | ✅ | `AppService.java:行号` |
| 接口规范 | URL 小写连字符 | `/api/v1/{resources}` | ✅ | `RestImpl.java:行号` |
| 接口规范 | 返回 ApiResult | `ApiResult.success(dto)` | ✅ | `RestImpl.java:行号` |
| 数据库规范 | 审计四字段 | `id/created_by/created_date/last_updated_by/last_updated_date` | ✅ | DDL 行号 |
| 数据库规范 | 表名小写下划线 | `@TableName("boss_user")` | ✅ | `PO.java:行号` |
| 安全规范 | 密码 BCrypt | `BCryptUtil.matches(raw, hashed)` | ✅ | `AppService.java:行号` |
| 测试规范 | 核心落库真实 DB | `{Resource}IT` with real DB | ✅ | `IT.java:行号` |

---

## 12. 与 Story 接口契约一致性确认

| 接口 | Story 定义签名 | Task 实现签名 | 是否一致 |
|------|---------------|--------------|---------|
| POST /api/v1/{resources} | `create(@RequestBody CreateRequest): ApiResult<{Resource}DTO>` | `create(@Valid @RequestBody CreateRequest): ApiResult<{Resource}DTO>` | ✅ |
| GET /api/v1/{resources}/{id} | `getById(@PathVariable Long id): ApiResult<{Resource}DTO>` | `getById(@PathVariable Long id): ApiResult<{Resource}DTO>` | ✅ |

---

## 13. Task 间依赖与调用关系

```mermaid
Task-0 公共依赖
    ↓
Task-1 Domain ({Resource}DO + 充血方法)
    ↓
Task-2 Infrastructure (PO + Mapper + DataConverter + RepositoryImpl)
    ↓
Task-3 Application (AppService + Converter)
    ↓
Task-4 Interfaces (RestImpl)
    ↓
Task-5 BFF (BFF AppService + BFF RestImpl)
    ↓
Task-6 Test (单测 + 集成测试 + 真实 HTTP)
```

事务边界：
- Task-3 AppService.create() `@Transactional` 包含：Repository.save + ApplicationEventPublisher.publishEvent
- 事务外：MQ 发送 / 通知推送 / 缓存失效

---

## 14. 实现注意事项

| # | 注意事项 | 风险等级 | 说明 |
|---|---------|---------|------|
| 1 | {Resource}DO 充血方法不能直接调 Repository | 🟠 | DO 是领域对象，不应感知基础设施 |
| 2 | 事务内禁 Feign 调用 | 🟠 | 违反导致事务长持有 |
| 3 | 状态机校验放在 Domain | 🟡 | 业务规则核心，不应放 AppService |
| 4 | ... | | |

---

## 15. 门禁自检（🔴 15 条全部 ✅ 才允许进入 `CodingSkill.Execute`）

> **每条门禁附判定 SOP，避免"自我声明 ✅"通过。**  
> 任一门禁未通过，禁止进入 `CodingSkill.Execute`，必须回到 `CodingSkill.Plan` 阶段修补。
> 🆕 v3.4.0 第 15 条 G-CODEPLAN-SRC 源码核对：新增/修改类建模范式须核对现有同类源码。

| # | 门禁 | 判定 SOP | ✅/❌ |
|---|------|---------|------|
| 1 | 7 章（§1-§7）全填 | grep Plan 文档：`grep "^## " ... \| wc -l` ≥ 7；N/A 必须有说明 | ☐ |
| 2 | 文件顺序可独立编译 | §4 每行"完成后必须通过的验证"必须有 mvn 命令 | ☐ |
| 3 | 类骨架覆盖所有层核心类 | §5 至少有 1 个 DO + 1 个 AppService + 1 个 RestImpl + 1 个 RepositoryImpl | ☐ |
| 4 | DO 字段与 Story 一致 | §6 表 = Story 数据模型章节的字段集合；不允许 Plan 多字段 | ☐ |
| 5 | SQL WHERE 明确 | §7 每行 UPDATE/DELETE 必须有 WHERE 条件；`grep "WHERE" ...` 命中 | ☐ |
| 6 | 测试数据可追溯 | §8 每行"测试数据来源"必须含 Story 章节号 + 行号；grep Story 命中 | ☐ |
| 7 | 核心落库标真实 DB | §8 "核心标识"列含 🔴 的必须有 ✅ 真实 DB | ☐ |
| 8 | 核心接口标真实 HTTP | §8 "核心标识"列含 🔴 的必须有 ✅ 真实 HTTP | ☐ |
| 9 | 验证点覆盖每 Task | §9 每个 Task ID 在"触发时机"列有对应验证命令 | ☐ |
| 10 | 调试回滚 ≥ 5 类 | §10 失败类型行数 ≥ 5 | ☐ |
| 11 | CodingModel 决策记录完整 | 每个 Task 文档的 `## CodingModel 决策记录` 中 11 维均有明确结论；无空值、无"不知道" | ☐ |
| 12 | 核心链路保护已标注 | 回调/Webhook/支付/状态更新/消息落库/通知类 Task，"核心链路保护"表格已填写，无空值 | ☐ |
| 13 | 资源隔离已证明 | 涉及异步/批量/外部通知时，线程池/队列/连接池/DB 共享情况已说明；共享的必须有隔离方案或容量证据 | ☐ |
| 14 | 混合压测场景已覆盖 | 涉及核心链路 + 批量任务时，测试映射中必须包含"批量任务满载 + 核心接口并发"混合场景 | ☐ |
| 15 | 🆕 G-CODEPLAN-SRC 源码核对通过 | §5 每个类骨架附【已读源码：】标记（文件真实存在）或【待核实源码】；待核实清单为空才 ✅ | ☐ |

**Code Plan 失败回退机制：**
- 门禁 1-10 不通过 → **只修补对应章节**，其他章节复用
- 门禁 11-14 不通过 → **回到各 Task 文档**，由 `CodingSkill.Plan(task-level)` 补充缺失内容，再重新汇总
- 门禁 15 不通过 → **补读现有同类源码**，把【待核实源码】改为【已读源码：】；或确认文件存在
- 修补后重跑门禁；全 ✅ 后进入 `CodingSkill.Execute`

---

## 维护

- **维护人：** 架构组 + Coding 阶段作者
- **更新频率：** 每次 ④bis 阶段使用本模板
- **同步对象：** ① 与 `project-assets-schema.md` 配套使用 ② 引用本模板的所有 CodingPlan
- **Tier 选择演进：** Tier 1/2/3 分类是经验值，后续根据 Story 复杂度调整
