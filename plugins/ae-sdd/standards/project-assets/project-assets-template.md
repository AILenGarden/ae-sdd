---
name: project-assets-template
description: 项目资产目录空模板（Starter）— 供新项目 cp 后填值用。结构与 project-assets-schema.md 一一对应（12 节 + 附录 JSON Schema + 维护）。
---

# Project Assets Template — 项目资产目录 Starter 模板

> **使用方法：**
> 1. `cp skills/ae-sdd/strategies/project-assets-template.md skills/ae-sdd/project-assets/{your-project-key}/{your-project-key}.assets.md`
> 2. 把所有 `{占位符}` 替换为你的项目事实
> 3. 走 `project-assets-schema.md §9 探查 SOP` 9 步填值
> 4. 完成后在 §1 写 `lastAuditedAt` + `owner`
>
> **与 schema 的关系：** 本文件 = schema 12 节 + 附录的"空值版本"，schema 是"有数据的示例 + 字段定义"。两份文件结构一一对应。

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

### 6.8 技术栈范围

- {本项目 Java/Spring Boot 版本}
- {本项目 ORM 框架}
- {本项目禁止引入的库}

### 6.9 隐性约定

> {本项目隐性约定 — 探查中发现的"大家都知道但没人写下来"的坑}

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
| §1 项目资产引用块 | §1-§10 全部 | 文首必引 |
| §2 抽象分层 → 项目分层映射 | §3 / §4 | 包路径/类名 |
| §5 关键类骨架 | §4 / §5 | 类角色 + 命名 |
| §6 DO 字段对齐 | §6.5 | 审计四字段 |
| §7 Mapper / SQL | §6.5 | EXPLAIN 验证 |
| §8 测试对应 | §6.7 | 真实 DB/HTTP |
| §11 约束合规自审 | §6 全章 | 9 类约束 |
| §12 接口契约一致性 | §6.4 | URL/方法/分页 |

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

## 附录 A：JSON Schema 占位

> 完整 JSON Schema 见 `project-assets-schema.md 附录 A`。本项目填写时按 schema 同步生成 JSON 实例，存为 `project-assets.json` 供 Code Plan 自动生成器消费。

---

## 维护

- **维护人：** `{owner name / role}`
- **更新频率：** `{每月审计 / 新增微服务时立即更新 / ...}`
- **同步对象：** `{本项目所有 Story 编写者 / 跨项目模板 / ...}`
- **双源一致性审计：** `{每月跑对照脚本检查 §6 是否引用了 constraints/ 所有 8 个文件名}`
