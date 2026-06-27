---
name: cs-域概览
description: 客服域 (life-cs) 业务边界、状态机、跨域依赖、错误码分段、5-7 条红线（基于工程文档归纳）
---

# 客服域（cs-域概览）

> **业务域：** CS（Customer Service）
> **适用工程：** icec-cloud-life-cs（life-cs-service）/ icec-cloud-life-cs-spi（SPI）
> **最后更新：** 2026-06-27
> **可信度：** 已确认（基于工程文档归纳）

> **🆕 v3.5.1.1 来源说明**：本概览由同事知识库 `D:\Item\life\document\life-team-project-docs\knowledge\project\icec-cloud-life-cs.md` 第 1/2/8/12 章 + `D:\Item\life\document\life-team-project-docs\knowledge\project\.md` 第 89/335/949-1087/1797+ 行归纳整理。**同事知识库 `domain/客服.md` 是 0 字节空文件**，本概览为首份客服域文档化材料。

---

## 1. 域边界

| 边界项 | 说明 | 出处 |
|--------|------|------|
| 核心职责 | 工单全生命周期 + 状态机、客服会话管理、坐席负载管理、超时自动结束、Copilot 双向消息同步、融云在线状态同步、CS-IM 联动 | `icec-cloud-life-cs.md:18-30` |
| 非职责 | 工单内容存储（走 IM 服务）、AI 消息生成（走 copilot-server）、推送下发（走 life-notification-service 中台）、TQ 外呼（仅记录） | `icec-cloud-life-cs.md:16, 26` |
| 上游消费者 | icec-cloud-boss-auth-bff（登录获取坐席信息）、icec-cloud-boss-user-bff（用户详情） | `function/STORY-003-BE-登录认证.md §1.7` |
| 下游依赖 | icec-cloud-life-im（IM 消息收发 + 在线状态查询）、copilot-server（AI 助手消息双向同步）、life-notification-service（推送中台）、TQ 外呼平台 | `icec-cloud-life-cs.md:16, 50` |

---

## 2. 核心实体与状态机

### 2.1 核心实体

| 实体 | 文件路径 | 核心字段 | 说明 |
|------|----------|---------|------|
| `CsTicketDO` | `icec-cloud-life-cs-domain/.../model/entity/CsTicketDO.java` | id, userId, status, sessionId, assignedAt, claimedAt, lastMessageAt, assignmentVersion | 工单聚合根（含状态机） |
| `CsUserDO` | `icec-cloud-life-cs-domain/.../model/entity/CsUserDO.java` | id, bossUserId, displayName, profileImgUrl, onlineStatus, currentLoad, maxConcurrency, assignmentVersion, isEnabled | 坐席聚合根（含乐观锁） |
| `CsConversationDO` | `icec-cloud-life-cs-domain/.../model/entity/CsConversationDO.java` | id, userId, csUserId, status, lastMessageAt | 客服会话聚合根 |
| `CsUserStatusRongyunSyncDO` | `icec-cloud-life-cs-domain/.../model/entity/CsUserStatusRongyunSyncDO.java` | id, userId, status, onlineDeviceCount, lastRongcloudOnlineAt, lastSeenAt, platforms | 融云状态同步值对象 |

### 2.2 工单状态机

```text
WAITING ──[坐席接单 claimTicket]──> IN_PROGRESS
   │                                       │
   ├──[AI 转人工]──> IN_PROGRESS           ├──[结单 closeTicket]──> RESOLVED
   │                                       │
   ├──[30min 未接单]──> TIMEOUT_CLOSED     ├──[15min 双侧静默]──> TIMEOUT_CLOSED
   │                                       │
   └──[5min 二次提醒]──> WAITING (再等)    └──[重开 reopenTicket]──> IN_PROGRESS
```

**状态枚举**（`CsTicketStatusEnum`）：WAITING / IN_PROGRESS / RESOLVED / UNRESOLVED / TIMEOUT_CLOSED

**驱动组件**：
- `CsTicketStateMachineDriver`：统一驱动两套 Spring StateMachine（工单 + 会话）
- `CsTicketStateMachineListener` / `CsConversationStateMachineListener`：监听 `TRANSITION_END` 落库
- `CsTicketCloseOrchestrator`：5 步结单编排（状态机流转 + 字段写入 + 释放负载 + 同步会话 + 通知），坐席主动结单与超时关单共用

### 2.3 坐席负载与并发控制

- `currentLoad` ≤ `maxConcurrency`（默认 5）：接单时 `+1`，结单/超时释放 `-1`
- `assignmentVersion`：乐观锁版本号，工单接单时 CAS 更新
- `CsAgentLoadDomainService`：负责接单占名额与释放名额的核心业务规则
- `CsDistributedLock`（Redisson）：user 维度回调式锁，串行化"创建工单 + 路由分配"、"重开工单"等并发敏感操作

---

## 3. 跨域依赖

| 依赖方向 | 依赖对象 | 调用方式 | 约束 |
|----------|----------|----------|------|
| `cs → boss-user` | `BossUserManagementService#authenticateUser` | Feign（`BossUserManagementClient`） | 登录链路；事务中禁止远程调用 |
| `cs → life-im` | `ImMessageService`（消息收发）+ `ImUserOnlineStatusService`（在线状态查询） | Feign（`CopilotServerClient` + `ImUserOnlineStatusServiceClient`） | 状态同步走 xxl-job 定时，消息同步走 CsMessageHandler 实时 |
| `cs → copilot-server` | AI 消息推送 + 服务模式同步 | Feign（`CopilotServerClient`），含 `@Async + @Retryable(3次/5s)` | `@Async` 异步推送，失败重试 3 次 |
| `cs → life-notification` | APP 极光推送 + 工作台融云系统消息 | Feign（`NotificationServiceClient`），含 `CsNotificationAsyncExecutor` | 走 `NotificationEventTypeEnum` 策略表内聚 |
| `cs → TQ 外呼` | TQ 外呼记录同步 | Feign（`TqOutboundCallClient`） | 保留能力，非本次变更焦点 |

---

## 4. 错误码与红线

### 4.1 错误码分段

| 错误码段 | 含义 | 备注 |
|----------|------|------|
| `21000-21099` | 坐席域 | `21002` loginScene=cs 但 bossUserId 在 cs_user 无记录（见 `function/STORY-003-BE-登录认证.md §2`）|
| `21100-21199` | 工单域 | `21110` / `21130` 等具体错误码见 `icec-cloud-life-cs-domain/.../enums/error/CsTicketErrorCode.java` |
| `21035` / `21040` / `21070` 等 | 应用层触发场景 | 散落在 `CsTicketAppService` / `CsConversationAppService` 异常抛出处 |

> **错误码分段尚未完全统一**（见 `icec-cloud-boss.assets.md §10 缺口 4`），部分错误码可能在 `21000-21999` 段位内有重叠，需后续梳理。

### 4.2 红线（🔴 编码必读）

| # | 红线 | 描述 | 出处 |
|---|------|------|------|
| 1 | 事务中禁止远程调用 | `CsTicketAppService#claimTicket`（事务内）只做本地双状态机流转，IM/Copilot/通知推送均在事务外或 `CsTicketCloseOrchestrator` 第 5 步事务外执行 | `icec-cloud-life-cs.md:21-22, 30` + `function/STORY-003-BE-登录认证.md §1.8 关键约束 #1` |
| 2 | 工单状态机流转必须经 `CsTicketStateMachineDriver` | 不允许直接 `csTicket.setStatus(...)` 绕过状态机 | `icec-cloud-life-cs.md:21` |
| 3 | 接单必须 CAS 更新 `assignment_version` | 高并发下避免两个坐席同时接单同一工单 | `icec-cloud-life-cs.md:28` + `icec-cloud-life-cs.md §2.3` |
| 4 | 超时关单必须先获取 user 维度分布式锁 | `CsTicketTimeoutAppService#closeTicket` 入口处 `CsDistributedLock.lock(userId)` | `icec-cloud-life-cs.md:22` |
| 5 | Copilot 消息同步失败不阻断主流程 | 推送失败仅 warn 日志，由 `@Retryable(3次/5s)` 兜底 | `icec-cloud-life-cs.md:24, 50` |
| 6 | 互踢与融云无关 | 坐席 session 互踢基于 Redis session 失效，不依赖融云 SDK | `function/STORY-003-BE-登录认证.md §1.8 关键约束 #2` |
| 7 | 禁 `@Scheduled`，统一用 xxl-job | `SyncImUserOnlineStatusJobHandler` 等定时任务经 xxl-job 执行器驱动（`life-cs-executor`，端口 10092） | `icec-cloud-life-cs.md:16, 52` + `assets/icec-cloud-boss.assets.md §11.9.1` |

---

## 5. 术语表

| 术语 | 定义 | 反例 / 易混点 |
|------|------|--------------|
| 坐席 | 通过 `cs_user` 关联 `boss_user` 的客服账号，1 个 boss 账号最多对应 1 个 cs 账号 | "客服" = "坐席" = "cs_user"；不要混用"agent" |
| 工单 | 用户进线后由系统自动创建的服务单，1 个会话可能产生多个工单 | "工单" ≠ "会话"；工单是"事件"维度，会话是"上下文"维度 |
| 会话 | 用户与坐席的连续对话上下文，由 sessionId 标识 | "会话" ≠ "工单" |
| Copilot | AI 助手，AI 阶段消息（生成/转人工过渡）由 copilot-server 同步到 IM | "Copilot" ≠ "IM"；Copilot 是 AI 消息源，IM 是消息存储 |
| WAITING | 工单状态：待接待（已路由未接单） | 不等于"未创建" |
| IN_PROGRESS | 工单状态：接待中（坐席已接单） | 不等于"会话进行中"（会话可能在 IN_PROGRESS 前就开启）|
| TIMEOUT_CLOSED | 工单状态：超时关闭（30min 未接单 / 15min 双侧静默） | 区分于 `RESOLVED`（坐席主动结单）|
| loginScene | 登录场景标识，区分 `boss` / `cs` / 其他业务 | 留空默认 `boss` |
| assignment_version | 乐观锁版本号，CAS 用于工单接单与坐席负载更新 | 不要用 `currentLoad` 做乐观锁 |
| DRIFT-01/02/03/07/09 | IM 域落库硬门禁编号，见 `assets/icec-cloud-boss.assets.md §11.8.3` | cs 域不直接引用 DRIFT 编号，cs 域 AC 用 `AC-XX` 格式 |

---

## 6. 待确认事项

| ID | 问题 | 影响范围 | 优先级 | 待查 |
|----|------|---------|--------|------|
| D-001 | 工单状态机是否包含 `REOPENED` 状态？文档提到"重开工单（reopenTicket）"但状态枚举只有 5 个（WAITING/IN_PROGRESS/RESOLVED/UNRESOLVED/TIMEOUT_CLOSED） | §2.2 状态机 | 🟠 P1 | 查 `CsTicketStatusEnum` 完整定义 |
| D-002 | 错误码分段 `21000-21099` / `21100-21199` 是否已统一？散落的具体错误码（21035/21040/21070/21110/21130）需要按段位重排 | §4.1 错误码段 | 🟠 P1 | 查全部 `*ErrorCode.java` 汇总 |
| D-003 | 客服域是否使用 DRIFT 落库门禁？文档只提到 IM 域用 DRIFT-XX，cs 域是否另有门禁编号？ | §4.2 红线 | 🟡 P2 | 查 cs 域 `*IntegrationTest.java` |
| D-004 | `CsTicketCloseOrchestrator` 5 步编排中第 3 步"释放负载"和第 4 步"同步会话"的事务边界？是否在同一事务？ | §2.2 / §4.2 红线 #1 | 🟠 P1 | 查 `CsTicketCloseOrchestrator` 源码 |
| D-005 | 客服域对外暴露的 SPI（`icec-cloud-life-cs-spi`）完整接口清单未在此概览中，需补登 §7 SPI 清单 | §3 跨域依赖 | 🟡 P2 | 查 `icec-cloud-life-cs-spi` 全部 `*Service.java` |

---

**🆕 v3.5.1.1 元说明**：本概览是 cs 域的首份结构化文档化材料。后续 STORY-002/007/009/010/011/020 等 Story 完成后，应回头补充 §2.2 状态机流转细节、§4.1 错误码分段表、§4.2 红线实例化反例。
