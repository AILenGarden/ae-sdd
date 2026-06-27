---
name: STORY-003-BE-登录认证
description: 坐席登录与认证 4 个接口的调用链 + 8 个 DTO 字段变更 + 2 个 Redis Key + 4 条关键约束（基于 STORY-003-BE 登录认证）
---

# 坐席登录与认证-BE 接口逻辑速查（STORY-003-BE）

> **来源 Story：** STORY-003-BE（2c-im-story-003-坐席登录与认证-BE）
> **涉及工程：** icec-cloud-boss-api / icec-cloud-boss-auth-bff / icec-cloud-boss-user / icec-cloud-life-spi / icec-cloud-life-cs
> **整体描述：** 复用 boss 平台登录体系，通过 `loginScene=cs` 区分坐席场景；坐席登录后获得含 cs 字段（displayName/profileImgUrl/onlineStatus 等）的 Token 用于 IM 工作台。
> **探查时间：** 2026-05-28
> **最后更新：** 2026-06-27
> **可信度：** 已确认

> **🆕 v3.5.1.1 来源说明**：本专题由同事知识库 `D:\Item\life\document\life-team-project-docs\knowledge\function\登录与认证-BE-接口逻辑速查.md`（10872 字节）迁入并按 `function-topic-template.md` 模板格式化。

---

## 1. 接口清单 [已确认]

| # | 接口 | 方法 | 所属工程 | 变更类型 |
|---|------|------|----------|----------|
| 1 | POST /boss-admin/boss-auth-bff/public/auth/login/password | 坐席登录 | icec-cloud-boss-api (icec-cloud-boss-auth-bff-api) | 扩展 |
| 2 | POST /boss-admin/boss-auth-bff/public/auth/logout | 坐席登出 | icec-cloud-boss-api (icec-cloud-boss-auth-bff-api) | 无变更 |
| 3 | POST /boss-admin/boss-auth-bff/public/auth/refresh-token | 刷新 Token | icec-cloud-boss-api (icec-cloud-boss-auth-bff-api) | 扩展 |
| 4 | GET /boss-admin/boss-user-bff/user/{userId} | 用户详情 | icec-cloud-boss-api (icec-cloud-boss-user-bff-api) | 扩展 |

---

## 1.5 公共数据模型变更一览 [已确认]

| 类 | 所属工程 | 新增/变更字段 | 变更类型 |
|----|----------|--------------|---------|
| `BossUserLoginReq` | icec-cloud-boss-api / icec-cloud-boss-auth-bff-api | loginScene | 已有 |
| `TokenResponse` | icec-cloud-boss-api / icec-cloud-boss-auth-bff-api | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad | 扩展 |
| `BossUserManagementVO` | icec-cloud-boss-api / icec-cloud-boss-user-bff-api | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad | 扩展 |
| `BossVerifyPasswordRequest` | icec-cloud-life-spi / icec-cloud-boss-user-spi | loginScene | 扩展 |
| `BossUserManagementDTO` | icec-cloud-life-spi / icec-cloud-boss-user-spi | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad | 扩展 |
| `CsUserDTO` | icec-cloud-life-spi / icec-cloud-life-cs-spi | userId, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad | 新建 |
| `CsUserService` | icec-cloud-life-spi / icec-cloud-life-cs-spi | 接口定义 | 新建 |
| `JwtLoginUser` | icec-cloud-boss-security | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad | 扩展 |

> **强制规则**：本节横向列出本 Story 涉及的所有 DTO/VO/Req/Resp 字段变更，防止 CodingPlan 阶段漏 DTO。字段来源必须来自 Story、代码、接口文档或已确认专题；据推断时逐项标注。

### cs_user 表（icec-cloud-life-cs 库，新建）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | varchar(36) PK | 主键 |
| boss_user_id | varchar(36) UK | 关联 boss 平台账号 ID |
| display_name | varchar(64) | 昵称 |
| profile_img_url | varchar(64) | 头像 |
| online_status | varchar(16) | ONLINE / AWAY，默认 ONLINE |
| current_load | int | 当前接待数，默认 0 |
| max_concurrency | int | 最大并发接待数，默认 5 |
| assignment_version | int | 乐观锁版本号，路由分配 CAS 使用 |
| is_enabled | tinyint | 1=启用，0=禁用 |
| created_by | varchar(64) | 创建人 |
| created_date | datetime | 创建时间，默认 CURRENT_TIMESTAMP |
| last_updated_by | varchar(64) | 最后更新人 |
| last_updated_date | datetime | 最后更新时间，自动更新 |

---

## 1.6 Redis Key 设计规范 [已确认]

| Key 格式 | 用途 | TTL | 所属工程 |
|----------|------|-----|---------|
| `boss_user:login_fail:{loginScene}_{userId}` | 登录失败计数 | 1800s（30 分钟） | icec-cloud-boss-user |
| `{loginScene}_{userId}` | 互踢 session 定位 | 跟随 Token 过期 | icec-cloud-boss-user |

> **强制规则**：所有新增/修改 Redis Key 必须在本节登记；Key 命名优先遵循 `{业务域}:{功能}:{参数}` 三段式。TTL 来源不明时不得写"永久"，必须标 `{待确认}`。

---

## 1.7 跨服务 Feign 调用表 [已确认]

| 调用方 | SPI 接口定义 | 被调用方 | Feign Client | 方法 |
|--------|-------------|----------|--------------|------|
| icec-cloud-boss-auth-bff | icec-cloud-life-spi / icec-cloud-boss-user-spi `BossUserManagementService` | icec-cloud-boss-user | `BossUserManagementClient` | authenticateUser |
| icec-cloud-boss-user | icec-cloud-life-spi / icec-cloud-life-cs-spi `CsUserService` | icec-cloud-life-cs | `CsUserClient` | getCsUserByUserId |
| icec-cloud-boss-user-bff | icec-cloud-life-spi / icec-cloud-life-cs-spi `CsUserService` | icec-cloud-life-cs | `CsUserClient` | getCsUserByUserId |

> **强制规则**：调用方列具体工程名（不是"BFF"）；SPI 接口路径精确到子模块，便于跨工程追踪。

---

## 1.8 关键约束清单（🔴 编码必读）

> **每条约束 1 行**：`约束名 — 描述 — 出处/反例`

1. **事务中禁止远程调用** — `BossUserAppService#authenticateUser` 含 Feign 调用，不开启事务 — 出处：`icec-cloud-boss-user-application/.../BossUserAppService.java:42`
2. **互踢与融云无关** — 基于 Redis session 失效，不依赖融云 SDK 与 STORY-001/002 — 反例：若误依赖融云 logout 回调，融云宕机时坐席无法重新登录
3. **Feign 失败处理差异化** — 登录时 Feign 失败直接抛异常返回错误码；用户详情时 Feign 失败不阻断 — 出处：`function/登录与认证-BE-接口逻辑速查.md:222`
4. **错误码区分账号存在性** — 接受轻微安全代价，"账号不存在"（11101）与"用户名或密码错误"（11105）错误码分离 — 出处：`icec-cloud-boss-user-domain/.../enums/error/BossUserErrorCode.java`

> **强制规则**：每条约束必须给出出处或反例。无出处只能标 `{待确认}`，不得进入 CodingPlan 的"已确认约束"。

---

## 1.9 集成测试范式 [已确认]

> **适用场景**：本 Story 不涉及集成测试（业务为登录认证，主要为单接口功能 + Controller HTTP 测试）。
> 集成测试范式参考 `assets/icec-cloud-boss/icec-cloud-boss.assets.md §11.8.3`（H2 + DRIFT 落库门禁）。

---

## 2. 接口 1：坐席登录 [已确认]

**入口：** `AuthenticationRestImpl#login` → `AuthenticationAppService#login`

**调用链：**

```text
前端
  -> [icec-cloud-boss-api / icec-cloud-boss-auth-bff-api] AuthenticationRestImpl#login
    -> [icec-cloud-boss-auth-bff] AuthenticationAppService#login
      ├─ BossUserManagementClient#authenticateUser ──Feign──→
      │    [icec-cloud-life-spi / icec-cloud-boss-user-spi] BossUserManagementService#authenticateUser
      │    [icec-cloud-boss-user / interfaces] BossUserServiceImpl#authenticateUser
      │      -> [icec-cloud-boss-user / application] BossUserAppService#authenticateUser
      │           ├─ [icec-cloud-boss-user / domain] BossUserDomainService#verifyPassword
      │           │    ├─ LoginLockFacade#isLocked (Redis 锁定检查)
      │           │    ├─ BossUserRepository#findValidUserByName (DB)
      │           │    └─ LoginLockFacade#incrementFailCount / clearFailCount
      │           │    [icec-cloud-boss-user / infrastructure] LoginLockFacadeImpl (Redis)
      │           └─ CsUserClient#getCsUserByUserId ──Feign──→ (仅 loginScene=cs)
      │                [icec-cloud-life-spi / icec-cloud-life-cs-spi] CsUserService#getCsUserByUserId
      │                [icec-cloud-life-cs / interfaces] CsUserServiceImpl
      │                  -> [icec-cloud-life-cs / infrastructure] CsMapper (DB: cs_user)
      ├─ kickOldSession (Redis session 互踢, 仅 loginScene=cs)
      └─ tokenService#createAndRefreshToken (生成 JWT)
```

**关键逻辑：**

1. `loginScene` 为空时默认 `boss`
2. 调用 `BossUserDomainService#verifyPassword(username, password, loginScene)`：
   - 检查 Redis 锁定（key: `boss_user:login_fail:{loginScene}_{userId}`）
   - 查询 boss_user 验证账号存在性和状态
   - BCrypt 校验密码，失败则 Redis 计数+1，达 5 次锁定 30 分钟
   - 成功则清除失败计数
3. `loginScene=cs` 时，Feign 调用 `CsUserClient#getCsUserByUserId` 获取客服信息
4. 互踢：`loginScene=cs` 时删除旧 Redis session（key: `{loginScene}_{userId}`），失败仅 warn 不阻断
5. 生成 JWT，cs 字段写入 `JwtLoginUser` 存入 Redis

**影响面：**

- **请求对象：** `BossUserLoginReq` 增加 `loginScene` 字段（已有）
- **响应对象：** `TokenResponse` 增加 `loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad` 字段
- **SPI 接口：** `BossUserManagementService#authenticateUser` 入参 `BossVerifyPasswordRequest` 增加 `loginScene`
- **Service 方法：** `BossUserAppService#authenticateUser` 增加 CsUserClient 调用
- **Redis Key：** 新增 `boss_user:login_fail:{loginScene}_{userId}` (TTL 1800s) + `{loginScene}_{userId}`（互踢，TTL 跟随 Token 过期）

**错误码：**

| 错误码 | 触发条件 |
|--------|----------|
| 11101 | username 在 boss_user 中无记录 |
| 11105 | 密码不匹配（未达 5 次） |
| 11106 | boss_user.status=0（禁用） |
| 11107 | 连续错误 5 次，锁定 30 分钟 |
| 21002 | loginScene=cs 但 bossUserId 在 cs_user 无记录 |

**约束：**

- `BossUserAppService#authenticateUser` 不开启事务（含 Feign 远程调用）
- 互踢失败仅 warn 日志，不阻断登录

---

## 3. 接口 2：坐席登出 [已确认]

**入口：** `AuthenticationRestImpl#logout` → `AuthenticationAppService#logout`

**调用链：**

```text
前端
  -> [icec-cloud-boss-api / icec-cloud-boss-auth-bff-api] AuthenticationRestImpl#logout
    -> [icec-cloud-boss-auth-bff] AuthenticationAppService#logout
      -> tokenService#delLoginUser (清除 Redis session)
```

**关键逻辑：**

1. 从 JWT Token Cookie（`security_context`）获取用户身份
2. 清除 Redis 中的 session 数据
3. 返回成功

**影响面：** 无。现有实现已满足需求。

**变更点：** 无。现有实现已满足需求。

---

## 4. 接口 3：刷新 Token [已确认]

**入口：** `AuthenticationRestImpl#refreshToken` → `AuthenticationAppService#refreshToken`

**调用链：**

```text
前端
  -> [icec-cloud-boss-api / icec-cloud-boss-auth-bff-api] AuthenticationRestImpl#refreshToken
    -> [icec-cloud-boss-auth-bff] AuthenticationAppService#refreshToken
      ├─ tokenService#refreshToken (生成新 Token)
      └─ tokenService#getLoginUser (从 Redis 获取 JwtLoginUser)
```

**关键逻辑：**

1. 验证 refreshToken 有效性
2. 从 Redis 获取 `JwtLoginUser`（含 cs 字段）
3. 生成新 accessToken + refreshToken
4. cs 字段从 `JwtLoginUser` 透传到 `TokenResponse`

**影响面：**

- **Service 方法：** `AuthenticationAppService#refreshToken` 方法中增加 7 行赋值：从 `loginUser` 读取 `loginScene`、`csUserId`、`displayName`、`profileImgUrl`、`onlineStatus`、`maxConcurrency`、`currentLoad` 填入 `TokenResponse`

**错误码：**

| 错误码 | 触发条件 |
|--------|----------|
| 10002 | refreshToken 过期或不存在 |

---

## 5. 接口 4：用户详情 [已确认]

**入口：** `BossUserManagementRest#getUserDetail`

**调用链：**

```text
前端
  -> [icec-cloud-boss-api / icec-cloud-boss-user-bff-api] BossUserManagementRest#getUserDetail
    -> [icec-cloud-boss-user-bff] 现有逻辑获取 BossUserManagementVO
      -> CsUserClient#getCsUserByUserId ──Feign──→ (新增)
        [icec-cloud-life-spi / icec-cloud-life-cs-spi] CsUserService#getCsUserByUserId
        [icec-cloud-life-cs / interfaces] CsUserServiceImpl
          -> [icec-cloud-life-cs / infrastructure] CsMapper (DB: cs_user)
```

**关键逻辑：**

1. 现有逻辑获取用户基本信息 → `BossUserManagementVO`
2. 新增：调用 `CsUserClient#getCsUserByUserId(userId)` 查询客服信息
3. 若 cs_user 存在，填充 cs 字段；不存在则 cs 字段保持 null（不报错）
4. CsUserClient 调用失败不阻断主流程，仅 warn 日志

**影响面：**

- **响应对象：** `BossUserManagementVO` 增加 cs 字段（loginScene/csUserId/displayName/profileImgUrl/onlineStatus/maxConcurrency/currentLoad）
- **Service 方法：** `getUserDetail` 实现中增加 CsUserClient 调用逻辑

---

## 6. 关键词反向索引

| 关键词 | 出现位置 |
|--------|---------|
| loginScene | §1.5 / §1.6 / §2 / §3 / §4 / §5 |
| BossUserManagementDTO | §1.5 / §2 / §5 |
| TokenResponse | §1.5 / §2 / §4 |
| JwtLoginUser | §1.5 / §2 / §4 |
| CsUserService | §1.5 / §1.7 / §2 / §5 |
| CsUserClient | §1.7 / §2 / §5 |
| cs_user | §1.5 / §2 / §5 |
| kickOldSession | §1.6 / §2 |
| boss_user:login_fail | §1.6 / §2 |

---

## 7. 待确认事项

| ID | 问题 | 影响范围 | 优先级 | 待查 |
|----|------|---------|--------|------|
| F-001 | `BossUserManagementClient#authenticateUser` Feign 调用的 Fallback 类是否已实现？登录失败时是否需要降级到 mock 用户？ | §2 接口 1 | 🟡 P2 | 查 `icec-cloud-boss-auth-bff` Fallback 配置 |
| F-002 | `cs_user.is_enabled=0` 的坐席登录时是按"账号禁用"还是按"cs_user 不存在"返回错误码？ | §2 接口 1 错误码 21002 | 🟡 P2 | 查 `icec-cloud-life-cs-domain` `CsUserErrorCode` |
| F-003 | 互踢 Redis Key 在多 BFF 节点（auth-bff + agent-workbench-bff）下的清理时序：删除旧 session 后新 session 才写入？还是有并发风险？ | §2 接口 1 互踢 | 🟠 P1 | 跨 BFF 并发场景压测 |
| F-004 | refreshToken 刷新时 cs 字段是否会因 Redis 抖动而丢失？是否需要 cs 字段兜底重查？ | §4 接口 3 | 🟡 P2 | 查 `icec-cloud-boss-security` `TokenService#refreshToken` 实现 |
