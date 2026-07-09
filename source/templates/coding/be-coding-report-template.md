# {WORKITEM-ID}：{标题} - Coding 报告

> **存放路径：** 由 `document-storage.resolve_path(intent="CODING_REPORT", workItemId={WORKITEM-ID}, storyId={STORY-ID}, version={major,minor})` 推导（见 document-storage §1.3）
> 
> 目录结构示例：
> ```
> ae-sdd-doc/Coding/{WORKITEM-ID}/
> └── {WORKITEM-ID}-CodingReport-v{N}-r{M}.md      ← Coding 报告
> ```
> 
> 完整路径示例：`{docWorkspacePath}/ae-sdd-doc/Coding/BUG-LIFE-001/BUG-LIFE-001-CodingReport-v1-r1.md`

## 填写声明表

> 本模板采用"两大段"结构：本表集中声明所有章节的**填写义务**，正文每章节按"骨架 + 示例"两段组织。
> 章节标题不再标注 `必填/选填`；判定依据统一查此表。

| § | 章节 | 填写义务 | 适用条件 |
| --- | --- | --- | --- |
| — | 元信息 | 🔴 必填 | 全部 |
| 1 | Story 任务概述 | 🔴 必填 | 全部 |
| 1.1 | └ 核心功能 | 🔴 必填 | 全部 |
| 1.2 | └ 业务价值 | 🔴 必填 | 全部 |
| 1.3 | └ 实现范围 | 🔴 必填 | 全部 |
| 2 | 分层实现清单 | 🔴 必填 | 全部 |
| 2.1 | └ SPI 层 | 🟡 选填（条件） | 跨服务契约变更时 |
| 2.2 | └ Domain 层 | 🟡 选填（条件） | 本层有文件变更时 |
| 2.3 | └ Application 层 | 🟡 选填（条件） | 本层有文件变更时 |
| 2.4 | └ Infrastructure 层 | 🟡 选填（条件） | 本层有文件变更时 |
| 2.5 | └ Interfaces / BFF 层 | 🟡 选填（条件） | 入口接口变更时 |
| 2.6 | └ Test 层 | 🟡 选填（条件） | 本层有文件变更时 |
| 2.7 | └ 文档 / 配置层 | 🟡 选填（条件） | 有文档或配置变更时 |
| 3 | 关键业务逻辑说明 | 🔴 必填 | 全部 |
| 3.1 | └ 核心方法实现 | 🔴 必填 | 全部 |
| 3.2 | └ 状态机流转逻辑 | 🟡 选填（条件） | 有状态机时 |
| 3.3 | └ 事务边界说明 | 🔴 必填 | 全部 |
| 3.4 | └ 并发控制方案 | 🟡 选填（条件） | 有并发场景时 |
| 4 | 数据库变更 | 🟡 选填（条件） | 有数据库变更时 |
| 4.1 | └ 表结构变更 | 🟡 选填（条件） | 有表结构变更时 |
| 4.2 | └ 索引变更 | 🟡 选填（条件） | 有索引变更时 |
| 4.3 | └ 数据迁移脚本 | 🟡 选填（条件） | 有数据迁移时 |
| 5 | 外部依赖调用 | 🟡 选填（条件） | 有外部调用时 |
| 5.1 | └ 外部服务调用清单 | 🟡 选填（条件） | 有外部调用时 |
| 5.2 | └ 调用参数说明 | 🟡 选填（条件） | 有外部调用时 |
| 6 | 单元测试覆盖 | 🔴 必填 | 全部 |
| 6.1 | └ 测试类清单 | 🔴 必填 | 全部 |
| 6.2 | └ 核心场景覆盖 | 🔴 必填 | 全部 |
| 6.3 | └ Mock 策略说明 | 🔴 必填 | 全部 |
| 7 | 开发问题记录 | 🟡 选填（条件） | 有问题时 |
| 7.1 | └ 已解决问题 | 🟡 选填（条件） | 有问题时 |
| 7.2 | └ 技术债务 | 🟡 选填（条件） | 有技术债务时 |
| 7.3 | └ 待优化项 | 🟡 选填（条件） | 有优化空间时 |
| 8 | 验证与交付 | 🔴 必填 | 全部 |
| 8.1 | └ 编译验证 | 🔴 必填 | 全部 |
| 8.2 | └ 单元测试验证 | 🔴 必填 | 全部 |
| 8.3 | └ 集成测试验证 | 🟡 选填（条件） | 有集成测试时 |
| 8.4 | └ 可交付检查清单 | 🔴 必填 | 全部 |
| 9 | 附录 | 🟡 选填（条件） | 有补充材料时 |
| 9.1 | └ 相关文档 | 🟡 选填（条件） | 有相关文档时 |
| 9.2 | └ 参考资料 | 🟡 选填（条件） | 有参考资料时 |

**填写义务图例：**
- 🔴 **必填** — 必须填写，不能为空，不能删章节
- 🟡 **选填（条件）** — 仅当"适用条件"成立时必填；条件不成立可整节删除

---

## 元信息

- 文档类型：Coding 报告
- WorkItem ID：{WORKITEM-ID}
- Story ID：{STORY-ID}
- 来源 Story：[{story文件名}]({story相对路径})
- 编码完成时间：{YYYY-MM-DD}
- 编码人员：{姓名}
- 状态：Draft / Review / Approved

---

## 1. Story 任务概述

### 1.1 核心功能

{一句话描述本 Story 实现的核心功能}

> 示例：实现坐席接单功能，支持并发校验和状态机流转

### 1.2 业务价值

{说明实现此功能带来的业务价值}

> 示例：坐席可以主动接入待接待工单，系统自动校验并发数，防止超额接单

### 1.3 实现范围

**包含：**
- {功能点 1}
- {功能点 2}

**不包含：**
- {明确不实现的功能}

> 示例：
> 包含：接单接口、并发校验、状态机流转 waiting→in_progress
> 不包含：自动分配、排队功能

---

## 2. 分层实现清单

> **填写规则：** 按工程调用顺序自上而下列出本轮所有新增/修改/删除/无改动但已核对的文件。表格列固定为：`类型 / 文件路径 / 变更类型 / 说明`。文件路径写工程内相对路径，长路径允许换行；`变更类型` 可填 `新增 / 修改 / 删除 / 无改动 / 仅测试 / 仅文档`。
>
> **分层顺序推荐：** SPI 层 → Domain 层 → Application 层 → Infrastructure 层 → Interfaces/BFF 层 → Test 层 → 文档/配置。Story 不涉及的层可以省略；如某层关键文件经核对但无需改动，可写 `无改动` 并说明原因。

### 2.1 SPI 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| SPI 接口 | `{spi-module}/src/main/java/.../{ServiceName}.java` | 新增/修改/无改动 | {新增方法 / 修改签名 / 已核对无需改动} |
| SPI DTO | `{spi-module}/src/main/java/.../dto/{DtoName}.java` | 新增/修改/无改动 | {字段变化、类型变化、兼容性说明} |

> 示例：
> | 类型 | 文件路径 | 变更类型 | 说明 |
> | SPI 接口 | `icec-cloud-life-spi/icec-cloud-life-im-spi/src/main/java/com/casstime/cloud/life/spi/im/service/session/ImSessionService.java` | 修改 | 新增 `getLatestMessageAt` / `batchGetLatestMessageAt` 方法签名 |
> | SPI DTO | `icec-cloud-life-spi/icec-cloud-life-im-spi/src/main/java/com/casstime/cloud/life/spi/im/dto/MessageDTO.java` | 无改动 | 现有字段满足本 Story，已核对 |

### 2.2 Domain 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| 聚合根/实体/值对象 | `{service-module}-domain/src/main/java/.../{ClassName}.java` | 新增/修改/无改动 | {领域行为、字段、不变量变化} |
| Domain Service | `{service-module}-domain/src/main/java/.../service/{ClassName}.java` | 新增/修改/无改动 | {领域规则或状态机变化} |
| Facade 接口 | `{service-module}-domain/src/main/java/.../facade/{FacadeName}.java` | 新增/修改/无改动 | {对外部领域能力的抽象变化} |
| Repository 接口 | `{service-module}-domain/src/main/java/.../repository/{RepositoryName}.java` | 新增/修改/无改动 | {新增查询/写入能力} |

> 示例：
> | 类型 | 文件路径 | 变更类型 | 说明 |
> | Facade 接口 | `icec-cloud-life-cs/icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/facade/ImSessionServiceFacade.java` | 修改 | CS 防腐层新增 2 个查询方法 |
> | Repository 接口 | `icec-cloud-life-cs/icec-cloud-life-cs-domain/src/main/java/com/casstime/cloud/life/cs/domain/cs/repository/CsTicketRepository.java` | 修改 | 新增 `syncLastMessageAtFromIm(Long ticketId, Date lastMessageAt)` |
> | Repository 接口 | `icec-cloud-life-im/icec-cloud-life-im-domain/src/main/java/com/casstime/cloud/life/im/domain/session/repository/ImSessionRepository.java` | 修改 | 新增 `getLatestMessageAt` / `batchGetLatestMessageAt` 方法签名（数据访问下沉） |

### 2.3 Application 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| AppService | `{service-module}-application/src/main/java/.../appservice/{ClassName}.java` | 新增/修改/无改动 | {业务编排变化} |
| Orchestrator | `{service-module}-application/src/main/java/.../orchestrator/{ClassName}.java` | 新增/修改/无改动 | {跨领域编排变化} |
| DTO/Command | `{service-module}-application/src/main/java/.../dto/{ClassName}.java` | 新增/修改/无改动 | {入参/出参字段变化} |
| Converter | `{service-module}-application/src/main/java/.../converter/{ClassName}.java` | 新增/修改/无改动 | {字段映射变化} |
| 事件处理器 | `{service-module}-application/src/main/java/.../handler/{ClassName}.java` | 新增/修改/无改动 | {消息/事件消费变化} |

> 示例：
> | 类型 | 文件路径 | 变更类型 | 说明 |
> | AppService | `icec-cloud-life-im/icec-cloud-life-im-application/src/main/java/com/casstime/cloud/life/im/application/session/appservice/ImSessionAppService.java` | 无改动 | 不加转发方法（数据访问下沉到 Repository） |
> | Orchestrator | `icec-cloud-life-cs/icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/orchestrator/CsTicketCloseOrchestrator.java` | 修改 | CloseContext 加 `lastMessageAt` 字段并用于关单 race 对齐 |
> | AppService | `icec-cloud-life-cs/icec-cloud-life-cs-application/src/main/java/com/casstime/cloud/life/cs/application/appservice/CsTicketTimeoutAppService.java` | 修改 | 加 `preloadLastMessageAtMap` 并调整超时关单入参 |

### 2.4 Infrastructure 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| Facade 实现 | `{service-module}-infrastructure/src/main/java/.../facade/{ClassName}.java` | 新增/修改/无改动 | {Feign/SPI 调用变化} |
| Repository 实现 | `{service-module}-infrastructure/src/main/java/.../repository/{ClassName}.java` | 新增/修改/无改动 | {Repository 方法实现变化} |
| Mapper/DAO | `{service-module}-infrastructure/src/main/java/.../mapper/{ClassName}.java/xml` | 新增/修改/无改动 | {SQL / 字段映射变化} |
| PO/DO | `{service-module}-infrastructure/src/main/java/.../dataobject/{ClassName}.java` | 新增/修改/无改动 | {持久化字段变化} |
| 外部服务 Gateway | `{service-module}-infrastructure/src/main/java/.../gateway/{ClassName}.java` | 新增/修改/无改动 | {外部服务适配变化} |

> 示例：
> | 类型 | 文件路径 | 变更类型 | 说明 |
> | Facade 实现 | `icec-cloud-life-cs/icec-cloud-life-cs-infrastructure/src/main/java/com/casstime/cloud/life/cs/infrastructure/facade/ImSessionServiceFacadeImpl.java` | 修改 | 实现 2 个方法；Feign 调用 IM 端点，错误降级为 ERROR 日志 |
> | Repository 实现 | `icec-cloud-life-im/icec-cloud-life-im-infrastructure/src/main/java/com/casstime/cloud/life/im/infrastructure/persistence/repository/ImSessionRepositoryImpl.java` | 修改 | 新增最新消息时间查询实现 |
> | Mapper XML | `icec-cloud-life-cs/icec-cloud-life-cs-infrastructure/src/main/resources/mapper/CsTicketMapper.xml` | 修改 | 新增 `syncLastMessageAtFromIm` SQL |

### 2.5 Interfaces / BFF 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| Controller | `{service-module}-interfaces/src/main/java/.../controller/{ClassName}.java` | 新增/修改/无改动 | {HTTP 入口变化} |
| Request/Response | `{service-module}-interfaces/src/main/java/.../dto/{ClassName}.java` | 新增/修改/无改动 | {前端契约字段变化} |
| JobHandler | `{service-module}-interfaces/src/main/java/.../jobhandler/{ClassName}.java` | 新增/修改/无改动 | {定时任务入口变化} |
| BFF API | `{bff-module}/src/main/java/.../{ClassName}.java` | 新增/修改/无改动 | {BFF 聚合入口变化} |

> 示例：
> | 类型 | 文件路径 | 变更类型 | 说明 |
> | JobHandler | `icec-cloud-life-cs/icec-cloud-life-cs-interfaces/src/main/java/com/casstime/cloud/life/cs/interfaces/jobhandler/TimeoutAgentJobHandler.java` | 无改动 | JobHandler 仍只做薄入口，业务逻辑在 AppService |
> | Controller | `icec-cloud-life-im-bff/src/main/java/com/casstime/cloud/life/im/bff/controller/ImMessageController.java` | 修改 | 新增查询最近消息时间接口 |

### 2.6 Test 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| 单元测试 | `{module}/src/test/java/.../{ClassName}Test.java` | 新增/修改/无改动 | {覆盖的分支/方法} |
| 集成测试 | `{module}/src/test/java/.../{ClassName}IT.java` | 新增/修改/无改动 | {真实 DB / 真实 HTTP 覆盖} |
| 测试资源 | `{module}/src/test/resources/{file}` | 新增/修改/无改动 | {schema/data/mock 配置变化} |

> 示例：
> | 类型 | 文件路径 | 变更类型 | 说明 |
> | 单元测试 | `icec-cloud-life-cs/icec-cloud-life-cs-application/src/test/java/com/casstime/cloud/life/cs/application/appservice/CsTicketTimeoutAppServiceTest.java` | 修改 | 覆盖预加载 latestMessageAt 与超时关单入参 |
> | Mapper 测试 | `icec-cloud-life-cs/icec-cloud-life-cs-infrastructure/src/test/java/com/casstime/cloud/life/cs/infrastructure/persistence/CsTicketMapperTest.java` | 修改 | 覆盖 `syncLastMessageAtFromIm` SQL |

### 2.7 文档 / 配置层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| YAML/Nacos | `{module}/src/main/resources/{file}.yml` | 新增/修改/无改动 | {配置项变化} |
| DDL/SQL | `{module}/src/main/resources/db/{file}.sql` | 新增/修改/无改动 | {表/字段/索引变化} |
| 项目文档 | `ae-sdd-doc/.../{file}.md` | 仅文档/修改/无改动 | {Story/Task/TestCase/CodingReport 变化} |

> 示例：
> | 类型 | 文件路径 | 变更类型 | 说明 |
> | Coding 报告 | `ae-sdd-doc/Coding/STORY-020-BE/STORY-020-BE-CodingReport-v2-r2.md` | 仅文档 | 记录本轮修复范围、测试结果和残余风险 |
> | 配置 | `icec-cloud-life-cs/icec-cloud-life-cs-interfaces/src/main/resources/bootstrap.yml` | 无改动 | 本 Story 未新增运行时配置 |

---

## 3. 关键业务逻辑说明

### 3.1 核心方法实现

#### 方法 1：{方法名}

**位置：** `{完整类路径}#{方法签名}`

**业务逻辑：**
{详细描述该方法实现的业务逻辑，包括：输入、处理步骤、输出}

**关键代码片段：**
```java
// 展示核心逻辑代码片段（10-20 行）
```

**注意事项：**
- {特殊处理说明}
- {边界条件说明}

> 示例：
> **位置：** `com.casstime.cloud.life.cs.application.ticket.appservice.CsTicketAppService#claimTicket`
> 
> **业务逻辑：**
> 1. 校验工单状态为 WAITING
> 2. 校验坐席并发数未超限
> 3. 调用领域服务执行状态流转
> 4. 更新坐席当前负载 +1
> 5. 触发状态变更通知
> 
> **关键代码片段：**
> ```java
> public void claimTicket(Long ticketId, Long agentId) {
>     CsTicket ticket = ticketRepository.findById(ticketId);
>     ticket.checkStatus(TicketStatus.WAITING);
>     
>     CsUser agent = csUserRepository.findById(agentId);
>     agent.checkConcurrency();
>     
>     stateMachine.transition(ticket, TicketStatus.IN_PROGRESS);
>     agent.incrementLoad();
> }
> ```

### 3.2 状态机流转逻辑

**状态机类：** `{完整类路径}`

**本次新增的状态流转：**

| 起始状态 | 目标状态 | 触发条件 | 前置校验 | 后置动作 |
| --- | --- | --- | --- | --- |
| {状态A} | {状态B} | {触发条件} | {校验逻辑} | {后置动作} |

> 示例：
> | 起始状态 | 目标状态 | 触发条件 | 前置校验 | 后置动作 |
> | WAITING | IN_PROGRESS | 坐席接单 | 坐席并发数未超限 | 更新 claimed_at、触发通知 |

### 3.3 事务边界说明

| 事务方法 | 事务范围 | 传播行为 | 说明 |
| --- | --- | --- | --- |
| `{类名}#{方法名}` | {事务包含的操作} | REQUIRED / REQUIRES_NEW | {事务设计说明} |

> 示例：
> | 事务方法 | 事务范围 | 传播行为 | 说明 |
> | `CsTicketAppService#claimTicket` | 更新工单状态 + 更新坐席负载 | REQUIRED | 事务内不包含外部调用，通知在事务外异步执行 |

**事务外操作：**
- {列出事务外执行的操作，如：消息推送、外部接口调用}

### 3.4 并发控制方案

**并发场景：** {描述并发场景}

**控制方案：** 乐观锁 / 悲观锁 / 分布式锁

**实现细节：**
{详细说明并发控制的实现方式}

> 示例：
> **并发场景：** 多个坐席同时接单可能导致并发数超限
> **控制方案：** 乐观锁（基于 assignment_version 字段）
> **实现细节：** 
> - 更新坐席负载时使用 `UPDATE ... WHERE id = ? AND assignment_version = ?`
> - 更新失败时抛出 ConcurrentModificationException
> - 最多重试 3 次

---

## 4. 数据库变更

### 4.1 表结构变更

**变更类型：** 新增表 / 修改表 / 删除表

**表名：** `{table_name}`

**DDL 语句：**
```sql
-- 新增表 / 修改表的 DDL
```

**字段说明：**

| 字段名 | 类型 | 说明 | 变更类型 |
| --- | --- | --- | --- |
| {column_name} | {type} | {说明} | 新增/修改/删除 |

### 4.2 索引变更

| 索引名 | 表名 | 索引字段 | 索引类型 | 变更类型 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `{index_name}` | `{table_name}` | {columns} | UNIQUE / NORMAL | 新增/删除 | {说明} |

> 示例：
> | 索引名 | 表名 | 索引字段 | 索引类型 | 变更类型 | 说明 |
> | `idx_ticket_status_agent` | `cs_ticket` | status, assigned_cs_user_id | NORMAL | 新增 | 优化按状态和坐席查询工单 |

### 4.3 数据迁移脚本

**迁移场景：** {描述需要迁移的数据场景}

**迁移脚本：**
```sql
-- 数据迁移 SQL
```

**回滚脚本：**
```sql
-- 回滚 SQL
```

---

## 5. 外部依赖调用

### 5.1 外部服务调用清单

| 服务名称 | 接口 | 调用位置 | 调用时机 | 超时设置 |
| --- | --- | --- | --- | --- |
| {服务名} | `{接口路径}` | `{类名}#{方法名}` | {调用时机} | {超时时间} |

> 示例：
> | 服务名称 | 接口 | 调用位置 | 调用时机 | 超时设置 |
> | 极光推送 | `POST /v3/push` | `CsNotificationService#notifyApp` | 工单状态变更后 | 3000ms |
> | copilot-server | `POST /api/session/sync-status` | `CsTicketAppService#claimTicket` | 接单成功后 | 5000ms |

### 5.2 调用参数说明

#### 调用 1：{服务名称} - {接口}

**入参：**
```json
{
  "参数示例": "值"
}
```

**出参：**
```json
{
  "返回示例": "值"
}
```

**异常处理策略：**
- {异常场景 1}：{处理方式}
- {异常场景 2}：{处理方式}

> 示例：
> **异常处理策略：**
> - 超时：记录日志，不影响主流程，异步重试 3 次
> - 服务不可用：降级处理，返回默认值

---

## 6. 单元测试覆盖

### 6.1 测试类清单

| 测试类 | 测试目标 | 测试方法数 | 覆盖率 |
| --- | --- | --- | --- |
| `{TestClassName}` | {测试目标类} | {数量} | {覆盖率%} |

> 示例：
> | 测试类 | 测试目标 | 测试方法数 | 覆盖率 |
> | `CsTicketAppServiceTest` | CsTicketAppService | 5 | 85% |
> | `CsTicketStateMachineTest` | CsTicketStateMachine | 8 | 100% |

### 6.2 核心场景覆盖

| 场景 | 测试方法 | 验证点 | 状态 |
| --- | --- | --- | --- |
| {场景描述} | `{TestClass}#{method}` | {验证点} | ✅ Pass / ❌ Fail |

> 示例：
> | 场景 | 测试方法 | 验证点 | 状态 |
> | 正常接单 | `CsTicketAppServiceTest#testClaimTicket_Success` | 状态流转、负载更新、通知触发 | ✅ Pass |
> | 并发数超限 | `CsTicketAppServiceTest#testClaimTicket_ConcurrencyExceeded` | 抛出 ConcurrencyExceededException | ✅ Pass |
> | 工单状态非法 | `CsTicketAppServiceTest#testClaimTicket_InvalidStatus` | 抛出 IllegalStateException | ✅ Pass |

### 6.3 Mock 策略说明

**Mock 对象：**
- `{依赖类名}`：{Mock 原因和策略}

> 示例：
> - `CsTicketRepository`：Mock 数据库操作，使用 Mockito 模拟返回
> - `NotificationService`：Mock 外部推送服务，验证调用次数和参数
> - `CopilotClient`：Mock 外部接口调用，避免真实网络请求

**未 Mock 的真实调用：**
- `{类名}`：{不 Mock 的原因}

> 示例：
> - `CsTicketStateMachine`：领域逻辑核心，使用真实对象验证状态流转

---

## 7. 开发问题记录

### 7.1 已解决问题

| # | 问题描述 | 影响范围 | 解决方案 | 解决时间 |
| --- | --- | --- | --- | --- |
| 1 | {问题描述} | {影响范围} | {解决方案} | {YYYY-MM-DD} |

> 示例：
> | # | 问题描述 | 影响范围 | 解决方案 | 解决时间 |
> | 1 | 乐观锁更新失败未重试 | 高并发场景接单失败 | 增加重试机制，最多重试 3 次 | 2026-05-22 |
> | 2 | 事务内调用外部接口导致超时 | 接单响应慢 | 将外部调用移到事务外异步执行 | 2026-05-22 |

### 7.2 技术债务

| # | 债务描述 | 影响 | 计划处理时间 | 责任人 |
| --- | --- | --- | --- | --- |
| 1 | {债务描述} | {影响说明} | {计划时间} | {责任人} |

> 示例：
> | # | 债务描述 | 影响 | 计划处理时间 | 责任人 |
> | 1 | 坐席负载更新未使用缓存 | 高并发下数据库压力大 | V2.0 优化 | 张三 |

### 7.3 待优化项

| # | 优化项 | 当前方案 | 优化方案 | 优先级 |
| --- | --- | --- | --- | --- |
| 1 | {优化项} | {当前方案} | {优化方案} | P1/P2/P3 |

> 示例：
> | # | 优化项 | 当前方案 | 优化方案 | 优先级 |
> | 1 | 状态流转日志 | 只记录结果 | 增加流转前后状态对比 | P2 |

---

## 8. 验证与交付

### 8.1 编译验证

- [ ] `mvn clean compile` 通过
- [ ] 无编译警告
- [ ] 无代码规范检查错误

### 8.2 单元测试验证

- [ ] 所有单元测试通过
- [ ] 核心业务逻辑覆盖率 ≥ 80%
- [ ] 无测试用例失败

### 8.3 集成测试验证

- [ ] 接口测试通过
- [ ] 端到端场景验证通过

### 8.4 可交付检查清单

- [ ] 代码已提交到分支：`{分支名}`
- [ ] 代码已通过 Code Review（如需要）
- [ ] 数据库变更脚本已提供
- [ ] 接口文档已更新（如有新接口）
- [ ] 部署说明已提供（如有特殊配置）

---

## 9. 附录

### 9.1 相关文档

- Story 文档：[{story文件名}]({story相对路径})
- DR 文档：[{dr文件名}]({dr相对路径})
- 测试用例：[{testcase文件名}]({testcase相对路径})

### 9.2 参考资料

- {参考资料 1}
- {参考资料 2}

---

## 填写说明

1. **必填项**：在《填写声明表》中填写义务为 🔴 **必填** 的章节必须填写，不能留空
2. **选填项**：在《填写声明表》中填写义务为 🟡 **选填（条件）** 的章节根据适用条件填写，无相关内容可删除该章节
3. **示例内容**：所有 `> 示例：` 开头的内容仅作参考，实际填写时删除
4. **代码片段**：关键代码片段控制在 10-20 行，突出核心逻辑即可
5. **表格填写**：表格中的占位符 `{xxx}` 需替换为实际内容
6. **路径规范**：所有文件路径使用相对路径，从模块根目录开始
