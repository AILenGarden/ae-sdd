---
name: project-assets-template
description: 项目资产目录空模板（Starter）— 供新项目 cp 后填值用。结构与 project-assets-schema.md 一一对应（15 节 + 附录 A-F + 维护）。
---

# Project Assets Template — 项目资产目录 Starter 模板

> **使用方法：**
> 1. `cp skills/ae-sdd/standards/project-assets/project-assets-template.md {资产根}/{workspaceKey}.assets.md`（`{资产根}` = `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/`，见 document-storage §2.3）
> 2. 把所有 `{占位符}` 替换为你的项目事实
> 3. 走 `project-assets-schema.md §9 + §3.5-3.7` 探查 SOP 填值
> 4. 完成后在 §1 写 `lastAuditedAt` + `owner` + **§1.X 部署信息**
> 5. **🆕 主体 > 30KB 时必须按 §15 拆工程级子文件**
>
> **与 schema 的关系：** 本文件 = schema 15 节 + 附录 A-F 的"空值版本"，schema 是"有数据的示例 + 字段定义"。两份文件结构一一对应。

---

## 0. 摘要与使用场景

| 维度 | 内容 |
|------|------|
| 何时需要查 | {本项目 Code Plan 阶段 / Coding / Code Review} |
| 谁负责写 | {owner name / role} |
| 与 `constraints/` 的关系 | {一句话：规则在 constraints/，本文件只映射事实} |
| 关键不变量 | {一句话：本文件不重复定义 rules} |

---

## 1. 项目资产元信息

| 字段 | 值 |
|------|---|
| projectKey | `{project-key,如 my-project}` |
| projectName | `{项目全名}` |
| gitPath | `{git 仓库根路径}` |
| productLine | `{产品线,如 alpha}` |
| profile | `{dev, test, prod 等}` |
| mainClass | `{com.xxx.Application}` |
| packaging | `{jar / war}` |
| portRange | `{8080-8090}` |
| lastAuditedAt | `{YYYY-MM-DD}` |
| owner | `{架构组 / 域负责人}` |

### 1.X 部署信息（🆕 2026-06-26）

> **🔴 必填**：从 coder "接手第一个本地服务" 到 "跑通" 必须能直接从本节读到所有配置。

```yaml
deployment:
  profile.active: {beta-kunlun / dev / prod / ...}
  db.urlTemplate: "jdbc:mysql://${host}/${dbname}?useUnicode=true&..."
  db.pool:
    type: HikariCP
    max-active: 20
    min-idle: 1
    timeout: 30s
  redis.address: "{redis-...dcs.huaweicloud.com:6379}"
  redis.password.inConfig: false  # 🔴 若 true → 写 §14 安全隐患
  gateway: "{http://...}"
  imageRepo: "{registry..../cassmall/{projectKey}:1.0-SNAPSHOT}"
  nexusRepo: "{http://dev.casstime.com/nexus/...}"
  management.port: {30000 / 显式值}
  coverageTool: jacoco
```

---

## 2. 微服务清单

```yaml
- name: {module-name-1}
  responsibility: {业务职责一句话}
  port: {端口号}
  contextPath: {/context-path}
  hasBff: {true / false}
  callChain: {api→bff→spi→service 中的位置}
  dependsOnSpi:
    - {spi-module-1}
    - {spi-module-2}

- name: {module-name-2}
  responsibility: ...
  ...
```

---

## 3. 抽象分层 → 项目分层映射（粗粒度）

| 抽象层 | 含义 | 本项目对应工程模块 | 备注 |
|--------|------|------------------|------|
| 请求处理（Interfaces） | 控制器/请求入口 | `{module}/{module}-interfaces` | 不含 BFF |
| 业务编排（Application） | AppService/编排 | `{module}/{module}-application` | 事务在 AppService |
| 领域逻辑（Domain） | DO/聚合根 | `{module}/{module}-domain` | 充血 |
| 基础能力（Infrastructure） | PO/Mapper | `{module}/{module}-infrastructure` | 仅存取 |
| 跨模块 SPI（可选） | Feign 接口 | `{project}-spi/{module}-spi` | ServiceProviderConstants |
| BFF 入口（可选） | BFF 控制器 | `{module}-bff` | 仅当 hasBff=true |

> 详细包路径见 §4

---

## 4. DDD 内部分层落点（细粒度）

> 从你项目的 `{module}` 中**逐个填**。**禁止把"待定"留空**——所有路径必须能在代码里 grep 到。

| 类角色 | 精确包路径 | 典型类名 | 放什么 / 不放什么 |
|--------|-----------|---------|------------------|
| **Interfaces** | | | |
| Rest 实现类 | `interfaces/restful/` | `{Resource}RestImpl implements {SpiInterface}` | 仅协议适配 |
| Event Handler | `interfaces/eventhandlers/` | `{Resource}EventHandler` | 事件接入 |
| **Application** | | | |
| AppService | `application/appservice/` | `{Resource}AppService` | 事务、编排 |
| Converter | `application/converter/` | `{Resource}Converter` | `@UtilityClass` 静态方法 |
| Publisher | `application/publisher/` | `{Resource}Publisher` | 跨域事件发布 |
| **Domain** | | | |
| Domain Object | `domain/{业务域}/model/entity/` | `{Resource}DO` | 充血 |
| Value Object | `domain/{业务域}/model/value/` | `{Resource}Query / {Resource}Context` | 不可变值对象 |
| Enum | `domain/{业务域}/model/enums/` | `{Resource}MessageEnum` | (key, value) 双字段 |
| Error Enum | `domain/{业务域}/model/enums/error/` | `{Resource}ErrorCode` | 错误码 |
| Repository 接口 | `domain/{业务域}/repository/` | `{Resource}Repository` | 仅接口 |
| Domain Service | `domain/{业务域}/service/` | `{Resource}DomainService` | 跨聚合业务 |
| Event | `domain/{业务域}/event/` | `{Resource}CreatedEvent` | 领域事件 |
| Exception | `domain/{业务域}/exception/` | `{Resource}DomainException` | 领域异常 |
| Facade | `domain/{业务域}/facade/` | `{Resource}Facade` | 跨域抽象 |
| **Infrastructure** | | | |
| Config | `infrastructure/config/` | `MybatisPlusConfig` | Spring 配置 |
| Feign Client | `infrastructure/feign/` | `{Resource}Client extends {SpiService}` | 调外部服务 |
| PO | `infrastructure/persistence/entity/` | `{Resource}PO @TableName("xxx")` | 贫血 |
| Mapper | `infrastructure/persistence/dao/mapper/` | `{Resource}Mapper extends BaseMapper<{Resource}PO>` | MyBatis 映射 |
| DataConverter | `infrastructure/persistence/converter/` | `{Resource}DataConverter` | PO↔DO |
| Repository Impl | `infrastructure/persistence/repository/mysql/` | `{Resource}RepositoryImpl` | 实现 domain 接口 |
| Redis Repository | `infrastructure/persistence/repository/redis/` | `{Resource}RedisRepository` | 缓存 |
| Facade Impl | `infrastructure/facade/` | `{Resource}FacadeImpl` | 异常返回 null/空集合 |
| **SPI** | | | |
| SPI Service 接口 | `spi/{user, cs, im}/service/` | `{Resource}Service` | Feign 接口 |
| DTO | `spi/{user, cs, im}/dto/` | `{Resource}DTO` | 跨服务传输 |
| Constants | `spi/{user, cs, im}/` | `ServiceProviderConstants` | Feign 服务名 |
| **BFF** | | | |
| Rest Impl | `bff/interfaces/restful/` | `{Resource}RestImpl implements *Rest` | BFF 控制器 |
| AppService | `bff/application/appservice/` | `{Resource}{Action}AppService` | BFF 编排 |
| Feign Client | `bff/infrastructure/feign/` | `{Resource}Client extends *Service` | 调 Service |
| OperationLog Capability | `bff/infrastructure/operationlog/capability/` | `{Resource}OperationLoggable` | 操作日志 |

---

## 5. 命名约定

| 对象 | 命名模板 | 你的项目例子 | 反例 |
|------|---------|------------|------|
| Controller (BFF) | `{Resource}RestImpl` | `{例}` | `{反例}` |
| AppService (BFF) | `{Resource}{Action}AppService` | `{例}` | `{反例}` |
| AppService (Service) | `{Resource}AppService` | `{例}` | `{反例}` |
| Domain Object | `{Resource}DO` | `{例}` | `{反例}` |
| Persistent Object | `{Resource}PO` | `{例}` | `{反例}` |
| Repository | `{Resource}Repository` | `{例}` | `{反例}` |
| Repository Impl | `{Resource}RepositoryImpl` | `{例}` | `{反例}` |
| Converter (Application) | `{Resource}Converter` | `{例}` | `{反例}` |
| Data Converter (Infra) | `{Resource}DataConverter` | `{例}` | `{反例}` |
| Feign Client | `{Resource}{Action}Client` | `{例}` | `{反例}` |
| Error Code | `{Resource}ErrorCode` | `{例}` | `{反例}` |
| Domain Exception | `{Resource}DomainException` | `{例}` | `{反例}` |

**反例汇总：**
- ❌ `{项目内常见错误命名}` → ✅ `{正确命名}`

---

## 6. 工程约束（继承自 constraints/，按本项目裁剪+补缺）

> **本节不重复定义 rules**，只把 rules 映射到本项目代码。

### 6.1 分层架构

- {本项目分层调用链}
- {本项目禁止事项}

### 6.2 工程结构

- {本项目分层职责红线}
- {本项目判定口诀}

### 6.3 代码风格

- {本项目时间字段类型}
- {本项目 JSON 工具}
- {本项目枚举规范}
- {本项目事务规则}
- {本项目 Feign/Facade 规则}

### 6.4 接口规范

- {本项目 URL 规范}
- {本项目分页规范}
- {本项目错误码分段}
- {本项目 HTTP 状态码}

### 6.5 数据库规范

- {本项目库名规则}
- {本项目审计字段}
- {本项目主键类型}
- {本项目索引规则}

### 6.6 安全规范

- {本项目鉴权方式}
- {本项目权限注解}
- {本项目脱敏规则}

### 6.7 测试规范

- {本项目 Controller 测试方式}
- {本项目 Service 测试方式}
- {本项目覆盖率要求}

### 6.8 完整技术栈版本号表（🆕 2026-06-26）

> **🔴 必填**：从 schema §6.8 复制完整 7 张表（主框架 / 工具库 / 安全 / 内部 starter / 测试 / 构建 / 静态分析），按本项目实际情况填实际版本号 + 备注。
>
> 任何新增/升级依赖**必须先更新本表**，否则 Code Review 不通过。

#### 6.8.1 主框架与运行时

| 组件 | 版本 | 备注 |
|------|------|------|
| Java | {8} | |
| Spring Boot | {1.5.7.RELEASE} | |
| Spring Cloud | {Dalston.SR4} | |
| MyBatis-Plus | {3.3.2} | |
| ... | | |

#### 6.8.2 工具库与映射

| 组件 | 版本 | 备注 |
|------|------|------|
| MapStruct | {1.5.3.Final} | 仅引用，实际用显式 Converter |
| Lombok | {1.18.16} | |
| Swagger2 | {2.8.0} | |
| ... | | |

#### 6.8.3 安全与加解密
#### 6.8.4 内部基础组件（🔴 必填）
#### 6.8.5 测试框架与覆盖率要求
#### 6.8.6 构建与镜像
#### 6.8.7 静态分析与门禁工具

### 6.9 隐性约定

| 约定名 | 描述 | 出处 / 踩坑 Story | 反向链接 |
|--------|------|------------------|---------|
| {约定名} | {项目内隐性规则，必须可执行} | {Story / function 专题 / file:line} | {§11 经验 / §5 核心类 / constraints 条款} |

> **填写规则**：每条隐性约定必须有 1 个 Story 踩坑出处或代码出处，并至少反向链接到 §11 团队惯用实现方式、§5 核心类方法或 constraints/ 中的具体条款。无出处时标 `{待确认}`。

---

## 7. 跨服务契约入口

### 7.1 关键事实表

| 字段 | 你的项目值 | 抽取命令 |
|------|----------|---------|
| Feign 服务名 | `{例: boss-user-service}` | `grep -rn "ServiceProviderConstants" {spi-path}/ --include="*.java"` |
| Nacos 实例名 | `{例: boss-user-service}` | `find . -name "bootstrap*.yml" \| xargs grep "spring.application.name"` |
| 错误码分段 | `{例: 11000-11999 用户段}` | `find . -path "*/enums/error/*ErrorCode.java"` |
| Service 间依赖 | `{例: boss-user → cs-spi, notification-spi}` | `mvn dependency:tree -pl {service} -Dincludes="*:icec-cloud-*-spi"` |

### 7.2 契约抽取命令清单

```bash
# Feign 服务名常量
grep -rn "ServiceProviderConstants" {spi-path}/ --include="*.java" | head -30

# Nacos 应用名
find . -name "bootstrap*.yml" -o -name "application*.yml" | xargs grep -l "spring.application.name" 2>/dev/null

# 错误码分布
find . -path "*/enums/error/*ErrorCode.java" | head -20

# 跨服务 Feign 客户端
grep -rn "@FeignClient" --include="*.java" | head -30
```

---

## 8. Code Plan 输入索引

| Code Plan 章节 | 引用本项目资产章节 | 说明 |
|---------------|------------------|------|
| §1 项目资产引用块 | §1-§12 全部 | 文首必引 |
| §2 抽象分层 → 项目分层映射 | §3 / §4 | 包路径/类名 |
| §5 关键类骨架 | §4 / §5 / §11 / function 专题 | 类角色 + 命名 + 团队惯用模式 + 实战案例 |
| §6 DO 字段对齐 | §6.5 | 审计四字段 |
| §7 Mapper / SQL | §6.5 | EXPLAIN 验证 |
| §8 测试对应 | §6.7 / §11.8 / function 专题 §1.9 | 真实 DB/HTTP + 集成测试范式 |
| §11 约束合规自审 | §6 全章 | 9 类约束 |
| §12 接口契约一致性 | §6.4 | URL/方法/分页 |
| §13 惯用模式对齐 | §11 全章 | 新代码是否复用团队惯用方式 |

---

## 9. 探查 SOP（如何"研究并构建"一份新的项目资产）

> 完整 9 步见 `project-assets-schema.md §9`。本节仅列**首版快速通道**（≤ 2 小时完成）：

1. 读 AGENTS.md / README → §1
2. 读 constraints/ 8 个 .md → §6 初稿
3. `mvn dependency:tree` 列 SPI 依赖 → §2 dependsOnSpi
4. 抽典型类（每层 1-2 个）→ §4 典型类名
5. 抽命名（7 类各 5 个类名）→ §5
6. 抽契约（§7.2 命令）→ §7
7. 写 §3 粗粒度映射
8. 填 §10 缺口
9. 审计 + 写 §1 lastAuditedAt

---

## 10. 项目资产缺口与待补充

| # | 缺口 | 优先级 | 负责人 | 计划补齐时间 |
|---|------|-------|-------|------------|
| 1 | {缺口 1} | {🟠/🟡/🟢} | | |
| 2 | {缺口 2} | | | |
| ... | | | | |

---

## 11. 团队惯用实现方式（经验文档）

> **🔴 必填 ≥ 9 条经验**：按 schema §10.1 九大类，每类 ≥ 1 条。每条 ≥ 2 个真实出处（文件路径:行号）+ 约束对齐。
>
> **本节是 §10 缺口的"沉淀池"**——探查中发现的惯用模式必须沉淀到本节，下次复用。

### 11.1 跨层模式（≥1 条）

#### 11.1.1 {模式名}

**场景：** {什么时候用}
**惯用写法：**（代码骨架，≤30 行）
**出处：** `{file:line}` × 2
**约束对齐：** 符合 `constraints/layered-arch.md` §{X}
**反模式：** {如有}
**实战案例：** `function/{Story}.md#接口-{N}`（没有则写"暂无"）

### 11.2 Domain 层模式（≥1 条）
### 11.3 Application 层模式（≥1 条）
### 11.4 Infrastructure 层模式（≥1 条）
### 11.5 Interfaces 层模式（≥1 条）
### 11.6 异常处理模式（≥1 条）
### 11.7 并发与幂等模式（≥1 条）
### 11.8 测试模式（≥1 条）

#### 11.8.1 {测试模式名}

**场景：** {什么时候用}
**惯用写法：** {单测 / HTTP 集成测试 / H2 + 落库门禁 / Testcontainers 等}
**出处：** `{file:line}` × 2
**约束对齐：** 符合 `constraints/testing.md` §{X}
**反模式：** {如有，例如全 Mock Repository 导致落库不真实}
**实战案例：** `function/{Story}.md#19-集成测试范式`（没有则写"暂无"）

### 11.9 配置与集成模式（≥1 条）

---

## 12. 横切专题文件索引（🆕 2026-06-26）

> **🆕 升级背景**：单 Story 跨 ≥3 工程时，业务场景应入 `function/`，环境配置入 `config/`，业务域概览入 `domain/`——而非塞进主体。
>
> **🔴 必填**：本节列出本项目所有 `function/` `config/` `domain/` 文件，每个一行摘要。

### 12.1 function/ 业务场景专题索引

| 文件 | 涉及工程 | 摘要 | 最后更新 |
|------|---------|------|---------|
| `function/{Story}.md` | {A} / {B} | {一句话} | {YYYY-MM-DD} |

### 12.2 config/ 环境配置专题索引

| 文件 | 适用范围 | 摘要 | 最后更新 |
|------|---------|------|---------|
| `config/test/api-test-env.md` | {N 个工程} | {一句话} | {YYYY-MM-DD} |

### 12.3 domain/ 业务域概览专题索引

| 文件 | 业务域 | 摘要 | 最后更新 |
|------|--------|------|---------|
| `domain/{域Key}.md` | {域} | {一句话} | {YYYY-MM-DD} |

### 12.4 横切专题 ↔ 主体章节映射

| 横切专题 | 主体引用章节 |
|---------|------------|
| `function/{Story}.md` | §E.1 + §F |
| `config/test/*.md` | §D.1 + §1.X |
| `domain/{域}.md` | §0 + §2 |

---

## 13. 信息可信度三态标注（🆕 2026-06-26）

> **🔴 必填**：本节登记本项目主体的可信度分布。

| 可信度 | 章节覆盖 | 占比 |
|--------|---------|------|
| `[已确认]` | §1 / §2 / §6.1-6.5 | {X%} |
| `[据推断]` | §4 / §5 | {Y%} |
| `[待确认]` | §10 + `{pending-questions.md}` | {Z%} |

**门禁：** 主体中 `[据推断]` 占比应 ≤ 30%；如超阈值，触发额外探查 SOP。

---

## 14. 安全隐患登记（🆕 2026-06-26）

> **🔴 必填**：按 schema §14.1 四条扫描命令跑过；命中项记入本表。

| ID | 类型 | 位置 | 风险等级 | 描述 | 建议修复 | 状态 |
|----|------|------|---------|------|---------|------|
| S-001 | {明文密码/Actuator外露/...} | {file:line} | {🟠/🟡} | {描述} | {建议} | {待修} |

**门禁：** 🟠 高级风险 → 24h 内通知架构组 + 工程 owner。

---

## 15. 工程级粒度拆分记录（🆕 2026-06-26）

> **🔴 必填**：主体文件 > 30KB 时按 schema §15 拆工程级子文件，本节列出本项目所有子文件。

| 工程名 | 子文件路径 | 大小 | 最后更新 | 状态 |
|--------|----------|------|---------|------|
| {xxx} | `{projectKey}.{xxx}.assets.md` | {KB} | {YYYY-MM-DD} | ✅ 已生成 |

**拆分规则：** 主体 ≤ 30KB；每个工程一份子文件；主体 §2 微服务清单每行加 `[详见]({子文件路径})` 链接。

---

## 附录 A：JSON Schema 占位

> 完整 JSON Schema 见 `project-assets-schema.md 附录 A`。本项目填写时按 schema 同步生成 JSON 实例，存为 `project-assets.json` 供 Code Plan 自动生成器消费。

---

## 附录 B-F：横切专题与工程级子文件模板

> **完整模板见**：
> - 附录 B：工程级子文件 Starter → `project-assets-schema.md 附录 B`
> - 附录 C：function/ 业务场景专题 → `project-assets-schema.md 附录 C`
> - 附录 D：config/ 环境配置专题 → `project-assets-schema.md 附录 D`
> - 附录 E：domain/ 业务域概览专题 → `project-assets-schema.md 附录 E`
> - 附录 F：横切专题 ↔ 主体映射表 → `project-assets-schema.md 附录 F`
>
> **🔴 配套 starter 模板路径**（如需 cp 后填值）：
> - `skills/ae-sdd/templates/project-assets/module-assets-template.md`
> - `skills/ae-sdd/templates/project-assets/function-topic-template.md`
> - `skills/ae-sdd/templates/project-assets/config-topic-template.md`
> - `skills/ae-sdd/templates/project-assets/domain-topic-template.md`

---

## 维护

- **维护人：** `{owner name / role}`
- **更新频率：** `{每月审计 / 新增微服务时立即更新 / ...}`
- **同步对象：** `{本项目所有 Story 编写者 / 跨项目模板 / ...}`
- **双源一致性审计：** `{每月跑对照脚本检查 §6 是否引用了 constraints/ 所有 8 个文件名}`
- **🆕 横切专题审计：** `{每月审计 function/ config/ domain/ 三个目录的文件是否齐全 + 最后更新时间}`
