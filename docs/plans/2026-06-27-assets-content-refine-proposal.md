> **⚠️ 归档副本标注（2026-06-27）**：本文档为 v1.0 草案，修订方案已落地 v3.5.1（2026-06-27 首次实施）+ v3.5.1.1（2026-06-27 补做实施）两次实施。
>
> - v3.5.1 实施范围（首次）：§3.1 §10.2 实战案例字段 + §3.2 附录 C 5 实战维度 + §3.6 §6.9 表格化 + 3 个 starter 模板新增
> - v3.5.1.1 实施范围（补做）：§3.3 §11.8 扩 4 条 + §3.4 §12 横切专题首批落地 3 篇 + §3.5 §5 增"变更点"列 + §3.6 §6.9 填 4 条初始数据
> - 详见 `D:\Item\ae-sdd\source\CHANGELOG\2026-06-27-v3.5.1-assets-content-refine.md` 与对应修改文件。
>
> 本归档副本不再用于评审，请查阅 CHANGELOG 与 v3.5.1.1 实际落地文件。

---
name: assets-content-refine-proposal
description: 参考同事 life-team-project-docs/knowledge/ 库的 7 处优质之处，针对 ae-sdd 项目资产（schema/template/实例）的内容侧修订方案。重点修订 §5/§11/§12/附录 C，不是路径/目录侧。
version: 1.0
date: 2026-06-27
status: ✅ 已落地（v3.5.1 + v3.5.1.1 两次实施完成）
authors: Mavis (mvs_f8e47fab87ca4c678c5f5ed77be0b448)
related:
  - standards/project-assets/project-assets-schema.md
  - standards/project-assets/project-assets-template.md
  - source/CHANGELOG/2026-06-22-discipline-hardening-plan.md
  - source/CHANGELOG/2026-06-26-asset-path-governance-design.md
  - source/CHANGELOG/2026-06-27-v3.5.1-assets-content-refine.md
evidence-source: D:\Item\life\document\life-team-project-docs\knowledge\ (life-team-project-docs 同事知识库)

# ae-sdd 项目资产内容优化修订方案 v1.0

> **TL;DR** — 同事知识库 `D:\Item\life\document\life-team-project-docs\knowledge\` 的 `function/` 与 `project/` 里有 7 处实战元素（ASCII 调用链图 / DTO 字段级变更点 / Redis Key 设计表 / 跨服务 Feign 调用表 / 关键约束清单 / H2 + DRIFT 集成测试模式 / 横切专题结构），ae-sdd 项目资产 schema 已经把"骨架与门禁"搭得很扎实（§11 经验 23 条、§13 可信度三态、§14 安全登记、§15 拆分规则），但**实战元素几乎为空**。本方案针对 schema/template/附录 C 提出 5 项修订，目标让 ae-sdd 项目资产在 CodingPlan 阶段提供"骨架 + 实战 + 门禁"三位一体的真相源。
>
> **适用读者**：ae-sdd-update-skill 执行人 / 架构组 / 各域负责人
> **作用域**：仅修改 `ae-sdd/source/standards/project-assets/*` 与 `templates/project-assets/*`；不修改 `source/SKILL.md` 主体；不修改 `source/assets/{boss,life}/*` 已有实例（实例迁移走另一份文档）

---

## 0. 背景与目标

### 0.1 背景

2026-06-22 ~ 2026-06-26，ae-sdd 已经完成 v3.5.0 路径治理（`source/CHANGELOG/2026-06-26-asset-path-governance-design.md`）+ 学科硬化（`2026-06-22-discipline-hardening-plan.md`），并在 `assets/` 下首批生成 3 份工程级子资产（boss-user / boss-user-bff / life-cs）。

但 2026-06-27 跟同事知识库（`D:\Item\life\document\life-team-project-docs\knowledge\`）硬对比后发现：

| 维度 | ae-sdd | 同事知识库 |
|---|---|---|
| 骨架完整度 | ✅ schema 15 节 + 6 附录 + JSON Schema | ❌ 41 篇文档无统一骨架（章节命名/顺序各不一样）|
| 门禁严格度 | ✅ §13 可信度三态 + §14 安全登记 + §15 拆分 | ❌ 无任何门禁 |
| 经验沉淀 | ⚠️ §11 共 23 条经验（仅文本骨架）| ❌ 0 条 |
| **实战元素** | 🔴 **缺**：调用链图 / 变更点表 / Redis Key 表 / DTO 字段级变更一览 / 集成测试范式 | ✅ 7 处优质实战元素 |

**关键事实**：ae-sdd 项目资产写新代码时，AI 只能看到"事务在 AppService"这种**抽象经验**，看不到"boss-user 实际怎么改 7 个 DTO 字段加 loginScene"这种**实战案例**——而后者才是 CodingPlan 阶段最缺的。

### 0.2 目标

让 ae-sdd 项目资产在 CodingPlan 阶段能输出**"抽象经验 + 实战案例 + 门禁"**三位一体的真相源：

1. **抽象经验**（已有）：§11 23 条经验继续保留
2. **实战案例**（本方案新增）：§11 关联"跨工程调用链 case study"，附录 C function-topic-template 增加 5 个实战维度
3. **门禁**（已有）：§13/§14/§15 不变

### 0.3 不在范围

- 不修改 SKILL.md 主体（`source/SKILL.md`）
- 不修改 G-00/G-RA/G-CODEPLAN-SRC/G-DOC-STORAGE/G-14 门禁
- 不修改 `assets/{boss,life}/*` 已生成的 6 份资产实例（实例迁移方案另出 `2026-06-27-assets-instance-migration-plan.md`）
- 不修改 constraints/*（约束规则层与项目资产层正交）

---

## 1. 同事知识库的 7 处优质之处（基于硬证据）

> 完整证据清单见**附录 A**。本节只列要点 + path:line。

### 1.1 ✅ 实战调用链 ASCII 图（最稀缺）

**出处**：`D:\Item\life\document\life-team-project-docs\knowledge\function\登录与认证-BE-接口逻辑速查.md:25-45, 91-96, 113-120, 147-153`

**形态**：ASCII 树形图跨工程调用链，每一行带类名 + 行号语义：

```
前端 → [icec-cloud-boss-api / icec-cloud-boss-auth-bff-api]
         AuthenticationRestImpl#login
           → [icec-cloud-boss-auth-bff]
              AuthenticationAppService#login
                ├─ BossUserManagementClient#authenticateUser ──Feign──→
                │    [icec-cloud-life-spi / icec-cloud-boss-user-spi] BossUserManagementService#authenticateUser
                │    [icec-cloud-boss-user / interfaces] BossUserServiceImpl#authenticateUser
                │      → [icec-cloud-boss-user / application] BossUserAppService#authenticateUser
                │           ├─ [icec-cloud-boss-user / domain] BossUserDomainService#verifyPassword
                │           │    ├─ LoginLockFacade#isLocked (Redis 锁定检查)
                │           │    ├─ BossUserRepository#findValidUserByName (DB)
                │           │    └─ LoginLockFacade#incrementFailCount/clearFailCount
                │           │    [icec-cloud-boss-user / infrastructure] LoginLockFacadeImpl (Redis)
                │           └─ CsUserClient#getCsUserByUserId ──Feign──→ (仅 loginScene=cs)
                │                [icec-cloud-life-spi / icec-cloud-life-cs-spi] CsUserService#getCsUserByUserId
                │                [icec-cloud-life-cs / interfaces] CsUserServiceImpl
                │                  → [icec-cloud-life-cs / infrastructure] CsMapper (DB: cs_user)
                ├─ kickOldSession (Redis session 互踢, 仅 loginScene=cs)
                └─ tokenService#createAndRefreshToken (生成 JWT)
```

**对 ae-sdd 资产的价值**：当前 §11.1.1「BFF→SPI→AppService→Repository 标准调用链」（`assets/icec-cloud-boss/icec-cloud-boss.assets.md:637-646`）是**纯文字描述**，没有 ASCII 图，AI 写到 CodingPlan §2 时只能复述文字，没有视觉锚点。

### 1.2 ✅ 变更点追踪表（DTO 字段级）

**出处**：`function/登录与认证-BE-接口逻辑速查.md:60-67, 129-131, 162-164` + `project/icec-cloud-boss-user.md:1580`（§7.5）

**形态**：每段接口"变更点"段精确到 DTO 字段级：

> **变更点**：
> - `BossUserLoginReq` 增加 `loginScene` 字段（已有）
> - `BossVerifyPasswordRequest` 增加 `loginScene`
> - `BossUserManagementDTO` 增加 cs 字段（loginScene/csUserId/displayName/profileImgUrl/onlineStatus/maxConcurrency/currentLoad）
> - `JwtLoginUser` 增加 cs 字段
> - `TokenResponse` 增加 cs 字段
> - `BossUserDomainService#verifyPassword` 增加 loginScene 参数 + 锁定逻辑
> - `BossUserAppService#authenticateUser` 增加 CsUserClient 调用
> - `AuthenticationAppService#login` 增加互踢 + cs 字段赋值

**对 ae-sdd 资产的价值**：ae-sdd §5 核心类方法表格只有"方法名 / 入参 / 返回 / 业务含义"4 列（`assets/icec-cloud-life/icec-cloud-life.icec-cloud-life-cs.assets.md:288-306`），没有"变更点"维度。AI 写 CodingPlan 时只能看到方法存在，看不到"这个方法在 STORY-XXX 加了什么字段、为什么加"。

### 1.3 ✅ 公共数据模型变更一览（横向对比表）

**出处**：`function/登录与认证-BE-接口逻辑速查.md:188-199`

**形态**：一张表把同一个 Story 涉及的所有 DTO 横向列出来：

| 类 | 所属工程 | 新增字段 |
|----|----------|----------|
| `BossUserLoginReq` | icec-cloud-boss-api / icec-cloud-boss-auth-bff-api | loginScene（已有） |
| `TokenResponse` | icec-cloud-boss-api / icec-cloud-boss-auth-bff-api | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad |
| `BossUserManagementVO` | icec-cloud-boss-api / icec-cloud-boss-user-bff-api | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad |
| `BossVerifyPasswordRequest` | icec-cloud-life-spi / icec-cloud-boss-user-spi | loginScene |
| `BossUserManagementDTO` | icec-cloud-life-spi / icec-cloud-boss-user-spi | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad |
| `CsUserDTO`（新建） | icec-cloud-life-spi / icec-cloud-life-cs-spi | userId, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad |
| `CsUserService`（新建） | icec-cloud-life-spi / icec-cloud-life-cs-spi | 接口定义 |
| `JwtLoginUser` | icec-cloud-boss-security | loginScene, csUserId, displayName, profileImgUrl, onlineStatus, maxConcurrency, currentLoad |

**对 ae-sdd 资产的价值**：ae-sdd §5.1 Converter / §7 上下游契约各管一摊，没有"**同一个 Story 涉及的 8 个 DTO 字段横向对比**"这种全景视图。AI 写 CodingPlan 时容易漏 DTO。

### 1.4 ✅ Redis Key 设计规范表

**出处**：`function/登录与认证-BE-接口逻辑速查.md:201-206`

**形态**：

| Key 格式 | 用途 | TTL |
|----------|------|-----|
| `boss_user:login_fail:{loginScene}_{userId}` | 登录失败计数 | 1800s（30 分钟） |
| `{loginScene}_{userId}` | 互踢 session 定位 | 跟随 Token 过期 |

**对 ae-sdd 资产的价值**：ae-sdd §11.7.1「Redis 分布式锁」经验（`assets/icec-cloud-boss/icec-cloud-boss.assets.md:744-749`）只说"用 RedissonClient.tryLock"，**没有 Redis Key 命名规范 + TTL 设计**。AI 写 CodingPlan 时随便拼 Key 名（`userLock:{id}` / `lock:user:{id}` / `redis_lock_user_{id}`）。

### 1.5 ✅ 跨服务 Feign 调用表（5 列结构化）

**出处**：`function/登录与认证-BE-接口逻辑速查.md:210-216`

**形态**：

| 调用方 | SPI 接口定义 | 被调用方 | Feign Client | 方法 |
|--------|-------------|----------|--------------|------|
| icec-cloud-boss-auth-bff | icec-cloud-life-spi / icec-cloud-boss-user-spi `BossUserManagementService` | icec-cloud-boss-user | BossUserManagementClient | authenticateUser |
| icec-cloud-boss-user | icec-cloud-life-spi / icec-cloud-life-cs-spi `CsUserService` | icec-cloud-life-cs | CsUserClient | getCsUserByUserId |
| icec-cloud-boss-user-bff | icec-cloud-life-spi / icec-cloud-life-cs-spi `CsUserService` | icec-cloud-life-cs | CsUserClient | getCsUserByUserId |

**对 ae-sdd 资产的价值**：ae-sdd §7.2「对内消费（Feign Client）」（`assets/icec-cloud-life/icec-cloud-life.icec-cloud-life-cs.assets.md:621-630`）是 4 列「SPI / 服务 / 方法 / 本工程 Feign Client」，缺"调用方工程"列。AI 不知道"哪个工程调的哪个 Client"。

### 1.6 ✅ 关键约束清单（每条一行可执行）

**出处**：`function/登录与认证-BE-接口逻辑速查.md:220-224`

**形态**：

> **关键约束**
> 1. **事务中禁止远程调用**：`BossUserAppService#authenticateUser` 含 Feign 调用，不开启事务
> 2. **互踢与融云无关**：基于 Redis session 失效，不依赖 STORY-001/002
> 3. **Feign 失败处理**：登录时 Feign 失败直接抛异常返回错误码；用户详情时 Feign 失败不阻断
> 4. **密码错误暴露账号存在性**：接受轻微安全代价，区分"账号不存在"和"密码错误"错误码

**对 ae-sdd 资产的价值**：ae-sdd §6 工程约束是"通用层规则"（`standards/project-assets/project-assets-schema.md:222-373`，8 大类），§6.2 客服域特有约束（`assets/icec-cloud-life/icec-cloud-life.icec-cloud-life-cs.assets.md:592-604`）是"业务域红线"——**没有"特定 Story 的关键约束"段**。特定 Story 的约束（如"互踢不依赖融云"）属于一次性踩坑沉淀，需要单独段落承载。

### 1.7 ✅ 集成测试范式（H2 + DRIFT 门禁）

**出处**：`D:\Item\life\document\life-team-project-docs\knowledge\project\.md:2516`

**形态**：

> | 集成测试规范 | `icec-cloud-life-im-infrastructure/…/integration/ImMessageCallbackIntegrationTest.java` | 真实 H2（MySQL 模式）+ 真实 4 个 RepositoryImpl 全链路集成测试（DRIFT-01/02/03/07/09 落库硬门禁证据） | `@RunWith(SpringRunner.class)`，`@TestPropertySource` 注入 H2 datasource，`@Transactional` 自动回滚，仅 Mock 签名校验 / Hook / RongCloud 三个非落库协作者 |

**对 ae-sdd 资产的价值**：ae-sdd §11.8 测试模式只有 2 条（Controller HTTP + Service 单测，`assets/icec-cloud-boss/icec-cloud-boss.assets.md:751-765`），**没有"集成测试 + H2 MySQL 模式 + DRIFT 落库门禁 + Mock 策略"这一整套实战范式**。这套范式是 life 项目 IM 域 STORY 反复用到的核心测试范本。

### 1.8 ✅ 横切专题结构（function/ + config/ + domain/）

**出处**：同事知识库根目录结构

```
D:\Item\life\document\life-team-project-docs\knowledge\
├── function/                                  # 业务场景专题
│   └── 登录与认证-BE-接口逻辑速查.md          (10.8KB)
├── config/
│   └── test/
│       └── api-test-env.md                    (3KB，5 个工程的本地/测试环境配置)
├── domain/
│   └── 客服.md                                (0KB，空文件——骨架在)
└── project/                                   # 工程级百科（38 篇）
```

**对 ae-sdd 资产的价值**：ae-sdd schema §12 横切专题文件索引（`standards/project-assets/project-assets-schema.md:786-808`）+ 附录 C/D/E 模板（schema.md:1179-1457）已经把"function/ config/ domain/ 三类专题"骨架搭好，但 `assets/icec-cloud-boss/icec-cloud-boss.assets.md:786-808` 三类目录**当前都标"暂无"**。同事知识库已经先跑出了 2 篇高质量 function + config 范本，ae-sdd 应该直接吸收。

---

## 2. ae-sdd 项目资产当前不足

> 完整对照表见**附录 C**。本节列具体不足 + 证据。

### 2.1 🔴 §11 经验沉淀缺实战元素（23 条全是"骨架与命名约定"，无"实战案例"）

**证据**：`assets/icec-cloud-boss/icec-cloud-boss.assets.md:629-781`（§11.1-§11.9 共 23 条）

每条经验 = 场景 + 惯用写法（≤30 行）+ ≥2 出处 + 约束对齐 + 反模式。**但没有一条带 ASCII 调用链图、没有一条带 DTO 字段级变更点、没有一条带 Redis Key 规范**。

**问题**：AI 写 CodingPlan §5「关键类骨架」时只能看到"要建 `BossUserManagementAppService` + `BossUserManagementClient` + `BossUserManagementFacade`"，看不到"还要在 8 个 DTO 里加 cs 字段 + 在 Redis 加 2 个 Key + 加 1 个新 SPI 服务 CsUserService"。

### 2.2 🔴 §12 横切专题三类目录"暂无"

**证据**：`assets/icec-cloud-boss/icec-cloud-boss.assets.md:792-808`：

```
12.1 function/ 业务场景专题索引: _（暂无）_
12.2 config/ 环境配置专题索引: _（暂无）_
12.3 domain/ 业务域概览专题索引: _（暂无）_
```

但 schema 附录 C（`standards/project-assets/project-assets-schema.md:1179-1289`）已定义 function-topic-template，附录 D（schema.md:1293+）已定义 config-topic-template，附录 E 定义 domain-topic-template。**模板到位，落地为零**。

### 2.3 🟠 §5 核心类方法表格缺"变更点"列

**证据**：`assets/icec-cloud-life/icec-cloud-life.icec-cloud-life-cs.assets.md:288-306`（`CsTicketAppService` 段）：

| 方法名 | 入参 | 返回 | 业务含义 |
|--------|------|------|----------|
| `claimTicket` | `ClaimTicketRequest` | `Result<ClaimTicketResponse>` | 坐席接单（`@Transactional`）：工单状态机 WAITING→IN_PROGRESS... |
| `closeTicket` | `CloseTicketRequest` | `Result<Void>` | 工单结单... |

**问题**：4 列缺"变更点"列。AI 不知道这个方法是 STORY-020 新增的还是重构的，改了什么字段、为什么改。

### 2.4 🟠 §11.8 测试模式只 2 条（缺集成测试范式）

**证据**：`assets/icec-cloud-boss/icec-cloud-boss.assets.md:751-765`：

- §11.8.1 Controller 测试真实 HTTP（TestRestTemplate）
- §11.8.2 Service 单测禁真实 DB / 远程

**问题**：life-cs/life-im 域大量使用 `集成测试 + H2 + DRIFT-01/02/03/07/09 落库门禁` 范式（`project/.md:2516`），ae-sdd 没有沉淀。

### 2.5 🟠 §6.9 隐性约定空缺

**证据**：`standards/project-assets/project-assets-schema.md:373-375`：

> ### 6.9 隐性约定（constraints/implicit-constraints.md 的本项目补缺）
>
> > 当前 constraints/implicit-constraints.md 为空（仅占位）。Code Plan 编写时若发现"项目内大家知道但没人写下来"的约定，应主动提议补充到该文件，并在本节列出补充项。

**问题**：boss 主体 §6.9（`assets/icec-cloud-boss/icec-cloud-boss.assets.md` 同位置）也是空。"互踢与融云无关"这种隐性约定（`function/登录与认证-BE-接口逻辑速查.md:222`）没有沉淀路径。

### 2.6 🟡 function-topic-template 缺实战维度

**证据**：`standards/project-assets/project-assets-schema.md:1179-1289`（附录 C）

模板章节：来源 Story / 接口清单 / 各接口调用链 / 跨工程 Feign 依赖表 / 关键约束 / 关键词反向索引 / 待确认事项

**缺**：DTO 字段级变更一览表（§1.3）/ Redis Key 设计规范表（§1.4）/ 关键约束清单可执行化（§1.6）。

---

## 3. 修订方案（具体到章节 / 段落 / 模板）

### 3.1 🔴 P0（必做，本周）— §11 增加「实战 case study」挂钩机制

**目标**：保留现有 §11 23 条经验，新增「实战 case study 挂钩」字段，让每条经验能反向引用 `function/` 目录下的实战文档。

**改动文件**：`standards/project-assets/project-assets-schema.md:546-588`（§10.2 每条经验记录格式）

**改动 diff**：

```markdown
### 10.2 每条经验的记录格式

```markdown
### {分类}.{序号} {模式名称}

**场景：** 什么时候用这个模式
**惯用写法：**（代码骨架，≤30 行，含关键注释）
**出处：** {文件路径}:{行号}（至少 2 个真实出处证明"惯用"）
**约束对齐：** 符合 {constraints/xxx.md} 中的 {具体条款}
**反模式：** 项目中存在的错误写法（如有，标注文件路径，供 CodeReview 参考）
**🆕 实战案例：** [{函数案例路径}](../../function/{Story-id}.md#§接口-{N})
  （可选；若 function/ 下有该模式的实战案例文档，附链接；没则省）
```
```

**对应 schema.md:567-577 旧格式**——新增一行 `实战案例`。

**同步修改模板**：`standards/project-assets/project-assets-template.md:332-340`（§11.1.1 跨层模式案例）补一行：

```markdown
**实战案例：** [`function/STORY-003-BE-登录认证.md` §接口 1](../../function/STORY-003-BE-登录认证.md#接口1)
```

**预计工时**：1 小时（仅 schema + template）

### 3.2 🔴 P0（必做，本周）— 附录 C 增加 5 个实战维度

**目标**：在 function-topic-template（`schema.md:1179-1289`）中增加 5 个实战维度，让 function/ 文档能直接产出"实战 + 案例 + 约束"三位一体内容。

**改动文件**：`standards/project-assets/project-assets-schema.md:1179-1289`（附录 C）

**改动 diff**：在附录 C `function-topic-template` 的 §1 接口清单后追加以下 5 节：

```markdown
## 1.5 公共数据模型变更一览 [已确认/据推断]

| 类 | 所属工程 | 新增字段 | 变更类型 |
|----|----------|---------|---------|
| `{XxxReq}` | {api 路径} | {field1}, {field2} | 新增/扩展 |
| `{XxxDTO}` | {spi 路径} | ... | 新增/扩展 |
| `{XxxVO}` | {bff-api 路径} | ... | 新增/扩展 |
| ... | | | |

> **强制规则**：本节横向列出本 Story 涉及的所有 DTO 字段变更，
> 防止 CodingPlan 阶段漏 DTO（实测案例：STORY-003-BE 涉及 8 个 DTO）

## 1.6 Redis Key 设计规范 [已确认/据推断]

| Key 格式 | 用途 | TTL | 所属工程 |
|----------|------|-----|---------|
| `{prefix}:{biz}:{id}` | {用途} | {N s / 跟随 Token 过期} | {工程} |
| ... | | | |

> **强制规则**：所有新增/修改 Redis Key 必须在本节登记；
> Key 命名遵循 `{业务域}:{功能}:{参数}` 三段式（参考 STAGE 7 沉淀）

## 1.7 跨服务 Feign 调用表 [已确认]

| 调用方 | SPI 接口定义 | 被调用方 | Feign Client | 方法 |
|--------|-------------|----------|--------------|------|
| {工程} | {spi 接口} | {被调工程} | {Client} | {method} |
| ... | | | | |

> **强制规则**：调用方列具体工程名（不是"BFF"），便于跨工程追踪；
> SPI 接口路径精确到子模块（`icec-cloud-life-spi/icec-cloud-boss-user-spi/...`）

## 1.8 关键约束清单（🔴 编码必读）

> **每条约束 1 行**：`约束名 — 描述 — 反例链接`

1. **{约束名 1}** — {描述} — 反例：`{file:line}`
2. **{约束名 2}** — {描述} — 反例：`{file:line}`
3. ...

> **强制规则**：每条约束必须给出反例（来自历史 Story 踩坑）；
> 反例标注 file:line 便于 CodeReview 阶段对照

## 1.9 集成测试范式 [已确认/据推断]

> **适用场景**：本 Story 涉及的集成测试（如回调 / 状态机 / 多 Service 协作）

| 维度 | 实现 | 出处 |
|------|------|------|
| 测试框架 | `@RunWith(SpringRunner.class)` | {file:line} |
| 数据库 | H2（MySQL 模式） | {file:line} |
| 落库门禁 | DRIFT-01/02/03/07/09 | {file:line} |
| 回滚 | `@Transactional` 自动回滚 | {file:line} |
| Mock 策略 | 仅 Mock 签名校验 / Hook / 外部 SDK | {file:line} |

> **强制规则**：本节仅当本 Story 有集成测试需求时填；
> 字段名（DRIFT-XX）必须在本工程 §6 安全门禁中已定义
```

**预计工时**：2 小时（template + 1 份样板同步改造）

### 3.3 🟠 P1（重要，本月）— §11.8 测试模式扩到 4 条（含集成测试）

**目标**：在 `assets/icec-cloud-boss/icec-cloud-boss.assets.md:751-765` §11.8 后追加 2 条集成测试范式。

**改动文件**：`assets/icec-cloud-boss/icec-cloud-boss.assets.md:765`

**改动 diff**：

```markdown
### 11.8.3 集成测试 H2 + DRIFT 落库门禁 [已确认]

**场景：** 涉及回调 / 状态机 / 多 Service 协作的集成测试
**惯用写法：**
```java
@RunWith(SpringRunner.class)
@SpringBootTest
@TestPropertySource(properties = {
    "spring.datasource.url=jdbc:h2:mem:test;MODE=MySQL;DATABASE_TO_LOWER=TRUE",
    "spring.datasource.driver-class-name=org.h2.Driver"
})
@Transactional  // 自动回滚
public class ImMessageCallbackIntegrationTest {
    @Autowired CsTicketRepositoryImpl repo;  // 真实注入 4 个 RepositoryImpl
    @MockBean RongCloudSignatureVerifier;    // 仅 Mock 非落库协作者

    @Test
    public void testHandleCallback_DRIFT_01() {
        // DRIFT-01: 私聊消息已读数累计正确
        service.handleCallback(req);
        assertEquals(1, repo.findById(...).getReadCount());
    }
}
```
**出处：** `icec-cloud-life-im-infrastructure/src/test/java/.../ImMessageCallbackIntegrationTest.java:18`
**约束对齐：** 符合 `constraints/testing.md` §7.3（集成测试）
**反模式：** ❌ 集成测试用真实 MySQL + 多个微服务启动 → 启动慢 + 难回滚

### 11.8.4 Mock 策略：仅 Mock 非落库协作者 [已确认]

**场景：** 集成测试中决定 Mock 哪些依赖
**惯用写法：** Mock 3 类：① 外部 SDK（融云 / 短信）② 签名校验 ③ 事件 Hook；不 Mock 4 个 RepositoryImpl（保证落库真实）
**出处：** `icec-cloud-life-im-infrastructure/src/test/java/.../ImMessageCallbackIntegrationTest.java:88`
**约束对齐：** 符合 `constraints/testing.md` §7.3
**反模式：** ❌ 全 Mock RepositoryImpl → 失去"落库真实"的集成测试意义
```

**预计工时**：1 小时

### 3.4 🟠 P1（重要，本月）— §12 横切专题首批落地（3 类各 1 篇）

**目标**：把同事知识库的 2 篇高质量专题（`function/登录与认证-BE-接口逻辑速查.md` + `config/test/api-test-env.md`）迁入 `assets/icec-cloud-boss/function/` 与 `assets/icec-cloud-boss/config/`，并新建 1 篇 `domain/客服.md`。

**改动文件**：
- 新建 `assets/icec-cloud-boss/function/STORY-003-BE-登录认证.md`（从同事知识库迁入）
- 新建 `assets/icec-cloud-boss/config/test/api-test-env.md`（从同事知识库迁入）
- 新建 `assets/icec-cloud-boss/domain/cs-域概览.md`（基于 §11.6.2 错误码分段 + §6.2 客服域特有约束重写）

**改动 diff**：`assets/icec-cloud-boss/icec-cloud-boss.assets.md:786-808`（§12 三类目录）填入：

```markdown
12.1 function/ 业务场景专题索引
| 文件 | 涉及工程 | 摘要 | 最后更新 |
|------|---------|------|---------|
| `function/STORY-003-BE-登录认证.md` | boss-api / boss-auth-bff / boss-user / life-cs | 坐席登录与认证 4 个接口的调用链 + 8 个 DTO 字段变更 + 2 个 Redis Key | 2026-06-27 |
12.2 config/ 环境配置专题索引
| 文件 | 适用范围 | 摘要 | 最后更新 |
|------|---------|------|---------|
| `config/test/api-test-env.md` | 5 个 BFF/Service 工程 | 本地 + 测试环境的登录/Token/Base URL/management port | 2026-06-27 |
12.3 domain/ 业务域概览专题索引
| 文件 | 业务域 | 摘要 | 最后更新 |
|------|--------|------|---------|
| `domain/cs-域概览.md` | 客服域 (life-cs) | 工单状态机 + 错误码分段 + 9 条红线 | 2026-06-27 |
```

**预计工时**：3 小时（迁入 2 篇 + 新建 1 篇 + 更新 §12 索引）

### 3.5 🟡 P2（增强，下季度）— §5 核心类方法表格增「变更点」列

**目标**：让 §5 表格多 1 列"变更点"，AI 写 CodingPlan 时能看到"这个方法是哪个 Story 加的 / 改的"。

**改动文件**：`standards/project-assets/project-assets-schema.md` 附录 B 工程级子文件模板（schema.md:1000-1175）—— 在 §5 核心类方法表格格式中加列。

**改动 diff**：

```markdown
| 方法名 | 入参 | 返回 | 业务含义 | 🆕 变更点 |
|--------|------|------|---------|----------|
| {method} | {params} | {return} | {含义} | {STORY-id 或 "无"} |
```

**预计工时**：1 小时（schema + template）+ 现有 6 份实例同步改造 2 小时（实例改造在另一份 migration-plan 里）

### 3.6 🟡 P2（增强，下季度）— §6.9 隐性约定沉淀路径打通

**目标**：让"特定 Story 踩坑的隐性约定"有地方沉淀，反向链接到 §11 经验或 function/ 实战案例。

**改动文件**：`standards/project-assets/project-assets-schema.md:373-375`（§6.9）

**改动 diff**：

```markdown
### 6.9 隐性约定（constraints/implicit-constraints.md 的本项目补缺）

| 约定名 | 描述 | 出处 / 踩坑 Story | 反向链接 |
|--------|------|------------------|---------|
| 互踢与融云无关 | session 互踢基于 Redis，不依赖融云 | STORY-003-BE（function/登录认证 §关键约束 #2）| §11.7.1 分布式锁 |
| ... | | | |

> **强制规则**：每条隐性约定必须有 1 个 Story 踩坑出处 + 至少 1 个反向链接到 §11 经验或 §5 核心类
```

**预计工时**：1 小时（schema + 4-6 条初始沉淀）

---

## 4. 优先级与时间估算

| # | 修订项 | 优先级 | 工时 | 风险 | 依赖 |
|---|--------|-------|------|------|------|
| 3.1 | §11 实战案例挂钩字段 | 🔴 P0 | 1h | 低（schema 增字段，旧文档兼容）| 无 |
| 3.2 | 附录 C 实战 5 维度 | 🔴 P0 | 2h | 中（template 结构扩展，需要 1 份样板同步验证）| 3.1 |
| 3.3 | §11.8 测试模式扩 4 条 | 🟠 P1 | 1h | 低（仅 boss 主体增量）| 无 |
| 3.4 | §12 横切专题首批落地 | 🟠 P1 | 3h | 中（迁入 + 重写，需用户 review 内容）| 3.2 |
| 3.5 | §5 变更点列 | 🟡 P2 | 1h+2h（实例改造另出文档）| 中（实例改造量大，建议分批）| 3.2 |
| 3.6 | §6.9 隐性约定沉淀 | 🟡 P2 | 1h | 低 | 3.4 |

**总工时**：P0 共 3h / P1 共 4h / P2 共 4h（不含实例迁移）

**推荐执行顺序**：3.1 → 3.2 → 3.4 → 3.3 → 3.5 → 3.6

---

## 5. 验证标准

### 5.1 修订前/后对比

| 维度 | 修订前（当前） | 修订后（目标） |
|------|---------------|---------------|
| §11 每条经验字段数 | 5（场景/写法/出处/约束/反模式）| 6（+ 实战案例挂钩）|
| 附录 C function-topic 实战维度 | 4（接口清单/调用链/Feign 表/约束）| 9（+ DTO 变更一览/Redis Key/Feign 5 列/约束可执行/集成测试）|
| §12 横切专题落地 | 0 / 0 / 0 | 1 / 1 / 1（首批 3 篇）|
| §11.8 测试模式 | 2 | 4（+ H2 + DRIFT 集成测试 + Mock 策略）|

### 5.2 抽样审查（3 工程）

挑 3 份现有资产（`assets/icec-cloud-boss/icec-cloud-boss-user.assets.md` / `icec-cloud-boss-user-bff.assets.md` / `icec-cloud-life/icec-cloud-life-cs.assets.md`），逐节验证：
- §11 每条经验都有"实战案例"字段（即使是"无"也要标）
- §12 索引表的 3 篇专题能在文件系统里查到
- §11.8 能看到 4 条测试模式

### 5.3 AI CodingPlan 阶段对比（回归测试）

**方法**：挑 1 个已完成的 Story（建议 STORY-003-BE 登录认证，因为同事知识库已有高质量 function 范本），让 AI 用修订前/后的项目资产各出 1 份 CodingPlan，对比：
- 修订前：DTO 字段级变更识别 3-4/8（漏 DTO）
- 修订后：DTO 字段级变更识别 7-8/8（基本不漏）
- 修订前：Redis Key 命名随机
- 修订后：Redis Key 命名遵循 `boss_user:login_fail:{loginScene}_{userId}` 规范

### 5.4 G-00 门禁校验

修订后必须保持 G-00（项目资产存在 + 7 层索引齐备 + lastAuditedAt ≤ 30 天）门禁通过。具体执行命令：

```bash
ae-sdd gates check --only G-00
# 返回: { exists: true, last_audited_at: 2026-06-27, missing_sections: [], stale: false }
```

---

## 6. 风险与依赖

### 6.1 对现有资产实例的影响

- **3.1/3.2/3.3/3.6**：仅 schema/template 改动，**现有 6 份资产实例不受影响**（字段是新增，旧文档继续兼容）。
- **3.5**：schema 字段扩展，**现有 6 份资产实例需要同步加列**——但改造量大，建议在 `2026-06-27-assets-instance-migration-plan.md`（待写）里分批处理，不在本方案同步。
- **3.4**：新增 3 篇专题文件，**boss 主体 §12 索引表需要更新**——已包含在 3.4 工时内。

### 6.2 对 SKILL.md G-00 门禁的影响

- G-00 要求"项目资产 7 层索引齐备（§A-§G）"——本方案不修改 §A-§G 索引层，**门禁通过率不变**。
- G-CODEPLAN-SRC 要求"CodingPlan 关键类骨架章节每个新增/修改类必须附【已读源码：】"——本方案**强化**这一门禁（通过 §11 实战案例挂钩让 AI 更容易找到源码）。

### 6.3 与 update-skill 工作流的对接

- 本方案是 ae-sdd-update-skill（v3.0 自身维护）的一部分，遵循 SKILL.md §1.5 自更新识别路径。
- 执行步骤：① 立项（本文件）→ ② 评审（架构组 + code-reviewer）→ ③ 同步实施（按 §3 优先级）→ ④ 验证（§5 标准）→ ⑤ 写 update-log（`source/CHANGELOG/`）。
- 关联变更日志：本方案完成后写 `source/CHANGELOG/2026-06-27-assets-content-refine.md`，引用本文件。

### 6.4 对 life 项目的影响

- life 项目根未生成过 ae-sdd 资产——本方案**不影响** life 项目侧；
- life 项目侧迁入（建议在另一份 `2026-06-27-life-assets-migration-plan.md` 中处理）。

---

## 附录 A：同事知识库 7 处优质素材引用清单

| # | 优质之处 | 文件 path:line | 形态 |
|---|---------|---------------|------|
| 1.1 | 实战调用链 ASCII 图 | `D:\Item\life\document\life-team-project-docs\knowledge\function\登录与认证-BE-接口逻辑速查.md:25-45, 91-96, 113-120, 147-153` | ASCII 树形图跨工程调用链 |
| 1.2 | 变更点追踪表（DTO 字段级）| 同上 :60-67, 129-131, 162-164 | 每段接口的 7-8 条变更点 |
| 1.3 | 公共数据模型变更一览 | 同上 :188-199 | 8 个 DTO 横向对比表 |
| 1.4 | Redis Key 设计规范表 | 同上 :201-206 | 2 行 Key 格式 + 用途 + TTL |
| 1.5 | 跨服务 Feign 调用表 | 同上 :210-216 | 5 列结构化（调用方/SPI/被调/Client/方法）|
| 1.6 | 关键约束清单 | 同上 :220-224 | 4 条每条一行可执行 |
| 1.7 | 集成测试范式（H2 + DRIFT）| `D:\Item\life\document\life-team-project-docs\knowledge\project\.md:2516` | 5 字段测试范式 |
| 1.8 | 横切专题结构 | `D:\Item\life\document\life-team-project-docs\knowledge\` 根目录 | function/ config/ domain/ 三类专题 |

---

## 附录 B：ae-sdd 项目资产待修改文件清单

| 优先级 | 文件 path | 改动类型 | 行数估算 |
|-------|----------|---------|---------|
| 🔴 P0 | `ae-sdd/source/standards/project-assets/project-assets-schema.md` | §10.2 增 1 字段 + 附录 C 增 5 节 | +120 行 |
| 🔴 P0 | `ae-sdd/source/standards/project-assets/project-assets-template.md` | §11.1.1 示例同步 + 新增附录 C 模板示例 | +80 行 |
| 🟠 P1 | `ae-sdd/source/assets/icec-cloud-boss/icec-cloud-boss.assets.md` | §11.8 增 2 条 + §12 填 3 篇索引 | +150 行 |
| 🟠 P1 | `ae-sdd/source/assets/icec-cloud-boss/function/STORY-003-BE-登录认证.md`（新建）| 从同事知识库迁入 | +250 行（迁入 225 行 + 模板化扩展） |
| 🟠 P1 | `ae-sdd/source/assets/icec-cloud-boss/config/test/api-test-env.md`（新建）| 从同事知识库迁入 | +100 行 |
| 🟠 P1 | `ae-sdd/source/assets/icec-cloud-boss/domain/cs-域概览.md`（新建）| 新建 | +150 行 |
| 🟡 P2 | `ae-sdd/source/standards/project-assets/project-assets-schema.md` | §6.9 隐性约定沉淀路径 | +30 行 |
| 🟡 P2 | 6 份现有资产实例 | §5 表格增列 | +每份 30-50 行（另出 migration-plan） |

---

## 附录 C：修订前后对比样例（以 §11.1.1「BFF→SPI→AppService→Repository」为例）

### 修订前（当前 `assets/icec-cloud-boss/icec-cloud-boss.assets.md:637-646`）

```markdown
#### 11.1.1 BFF→SPI→AppService→Repository 标准调用链 [已确认]

**场景：** 任何 BFF / Service 编写新接口时的默认调用路径
**惯用写法：**
```
[前端] → [BFF / RestImpl] → [BFF / AppService] → [Facade] → [Feign Client extends SPI]
       → [Service / AppService] → [Domain Service] → [Repository] → [DB]
```
**出处：** `icec-cloud-boss-user-bff/.../appservice/BossUserManagementAppService.java:42` + `icec-cloud-life-cs/.../appservice/CsTicketAppService.java:88`
**约束对齐：** 符合 `constraints/layered-arch.md` §1.1（外部请求 → API → BFF → SPI → Service）
```

### 修订后（本方案落地后）

```markdown
#### 11.1.1 BFF→SPI→AppService→Repository 标准调用链 [已确认]

**场景：** 任何 BFF / Service 编写新接口时的默认调用路径
**惯用写法：**（ASCII 调用链图，跨工程可读）
```
[前端] → [BFF / RestImpl] → [BFF / AppService] → [Facade] → [Feign Client extends SPI]
       → [Service / AppService] → [Domain Service] → [Repository] → [DB]
```
**出处：** `icec-cloud-boss-user-bff/.../appservice/BossUserManagementAppService.java:42` + `icec-cloud-life-cs/.../appservice/CsTicketAppService.java:88`
**约束对齐：** 符合 `constraints/layered-arch.md` §1.1
**🆕 实战案例：** [`function/STORY-003-BE-登录认证.md` §接口 1](../../function/STORY-003-BE-登录认证.md#接口1)
  - 真实跨工程调用：`boss-auth-bff → boss-user → life-cs`
  - 7 处变更点：8 个 DTO 加 cs 字段、2 个 Redis Key
```

**改动 diff**：仅 +1 行「实战案例」字段（带 1 行说明 + 1 行真实跨工程示例）。

---

## 附录 D：执行 checklist（给 update-skill 执行人用）

- [ ] 立项评审：架构组 + code-reviewer review 本方案
- [ ] 3.1 schema §10.2 增「实战案例」字段（1h）
- [ ] 3.1 template §11.1.1 示例同步（0.5h）
- [ ] 3.2 schema 附录 C 增 5 个实战维度（1.5h）
- [ ] 3.2 template 附录 C 同步（0.5h）
- [ ] 3.4 迁入 `function/STORY-003-BE-登录认证.md`（1h）
- [ ] 3.4 迁入 `config/test/api-test-env.md`（0.5h）
- [ ] 3.4 新建 `domain/cs-域概览.md`（1h）
- [ ] 3.4 boss 主体 §12 索引更新（0.5h）
- [ ] 3.3 boss 主体 §11.8 增 2 条测试模式（1h）
- [ ] 5.2 抽样审查 3 份现有资产（1h）
- [ ] 5.3 AI CodingPlan 回归测试（1.5h）
- [ ] 5.4 G-00 门禁校验（0.5h）
- [ ] 写 `source/CHANGELOG/2026-06-27-assets-content-refine.md`（0.5h）

**总计**：约 12-13 小时（按 P0 + P1 全做估算）

---

**END of 修订方案 v1.0**