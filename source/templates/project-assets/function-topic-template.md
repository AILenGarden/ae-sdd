---
name: function-topic-template
description: project-assets function/ 业务场景专题 Starter 模板
---

# {场景名}-BE 接口逻辑排查（{Story 编号}）

> **适用场景：** project-assets `function/` 业务场景专题，按 Story 排查 BE 接口逻辑：接口清单、公共数据模型变更、Redis Key、Feign 调用、关键约束、集成测试范式、逐接口逻辑、关键词反向索引与待确认事项。
> **精简规则：** 必填章节不得留空；章节连续阿拉伯编号；占位符统一 `{花括号}`；表格分隔符统一 `| --- |`；强制规则统一 `> **🔴 强制：**` 引用块。
>
> **来源 Story：** {story-id}
> **涉及工程：** {A} / {B} / {C} / {D} / {E}
> **整体描述：** {一句话讲清这个 Story 在做什么业务场景}
> **探查时间：** {YYYY-MM-DD}
> **最后更新：** {YYYY-MM-DD}
> **可信度：** {已确认 / 据推断 / 待确认}

---

## 1. 接口清单 `必填`

| # | 接口 | 用途 | 接口归属工程 | 备注 |
| --- | --- | --- | --- | --- |
| 1 | `POST {URL}` | {用途} | {归属工程} | {变更/扩展} |
| 2 | ... | ... | ... | ... |

---

## 2. 公共数据模型变更一览 `选填（条件）`

| 类 | 所属工程 | 新增/变更字段 | 变更类型 |
| --- | --- | --- | --- |
| `{XxxReq}` | {api/bff-api 路径} | {field1}, {field2} | 新增/扩展/删除 |
| `{XxxDTO}` | {spi 路径} | {field1}, {field2} | 新增/扩展/删除 |
| `{XxxVO}` | {bff-api 路径} | {field1}, {field2} | 新增/扩展/删除 |

> **🔴 强制：** 本节横向列出本 Story 涉及的所有 DTO/VO/Req/Resp 字段变更，防止 CodingPlan 阶段漏 DTO。字段来源必须来自 Story、代码、接口文档或已确认专题；据推断时逐项标注。

---

## 3. Redis Key 设计规范 `选填（条件）`

| Key 格式 | 用途 | TTL | 所属工程 |
| --- | --- | --- | --- |
| `{prefix}:{biz}:{id}` | {用途} | {N s / 跟随 Token 过期 / 永久} | {工程} |

> **🔴 强制：** 所有新增/修改 Redis Key 必须在本节登记；Key 命名优先遵循 `{业务域}:{功能}:{参数}` 三段式。TTL 来源不明时不得写"永久"，必须标（待确认）。

---

## 4. 跨服务 Feign 调用表 `选填（条件）`

| 调用方 | SPI 接口定义 | 被调用方 | Feign Client | 方法 |
| --- | --- | --- | --- | --- |
| {工程} | {spi 子模块 / 接口} | {被调工程} | {Client} | {method} |

> **🔴 强制：** 调用方列具体工程名（不是"BFF"）；SPI 接口路径精确到子模块，便于跨工程追踪。

---

## 5. 关键约束清单 `🔴 强制`

> **每条约束 1 行**：`约束名 — 描述 — 出处/反例`

1. **{约束名 1}** — {描述} — 出处/反例：`{file:line 或 function 第 N 节}`
2. **{约束名 2}** — {描述} — 出处/反例：`{file:line 或 function 第 N 节}`

> **🔴 强制：** 每条约束必须给出出处或反例。无出处只能标（待确认），不得进入 CodingPlan 的"已确认约束"。

---

## 6. 集成测试范式 `选填（条件）`

> **适用场景：** 本 Story 涉及回调、状态机、多 Service 协作、落库一致性或外部 SDK 回调时填写。

| 维度 | 实现 | 出处 |
| --- | --- | --- |
| 测试框架 | `{JUnit/SpringRunner/...}` | {file:line} |
| 数据库 | `{H2 MySQL 模式 / Testcontainers / 开发库}` | {file:line} |
| 落库门禁 | `{DRIFT-XX / AC-XX / 无}` | {file:line} |
| 回滚 | `{Transactional 自动回滚 / 清理脚本}` | {file:line} |
| Mock 策略 | `{仅 Mock 外部 SDK / 签名校验 / Hook 等}` | {file:line} |

> **🔴 强制：** 本节仅在有集成测试需求时填；Mock 策略必须说明哪些依赖真实注入、哪些依赖被 Mock。若引用 DRIFT-XX，必须能在本工程 §6 安全门禁或 Story AC 中找到定义。

---

## 7. 接口 1：{接口名} `必填`

**入口：** `{XxxRestImpl#{method}}` / `{XxxAppService#{method}}`

**调用链：**

```text
{前端}
  -> [{归属工程 A} / {bff-api}] {XxxRestImpl#{method}}
    -> [{归属工程 B} / {bff}] {XxxAppService#{method}}
      -> [Feign 调用] {XxxClient#{method}}
        -> [{归属工程 C} / {spi}] {XxxService#{method}}
          -> [{归属工程 C} / {interfaces}] {XxxServiceImpl#{method}}
            -> [{归属工程 C} / {application}] {XxxAppService#{method}}
              -> [{归属工程 C} / {domain}] {XxxDomainService#{method}}
                -> [{归属工程 C} / {infrastructure}] {RepositoryImpl}
                -> [{归属工程 C} / {infrastructure}] {FacadeImpl} (Redis)
```

**关键逻辑：**

1. {逻辑点 1}
2. {逻辑点 2}
3. {逻辑点 3}

**影响面：**

- **请求对象：** `{XxxRequest}` 新增 `{字段}`
- **响应对象：** `{XxxResponse}` 新增 `{字段}`
- **SPI 接口：** `{XxxService}` 新增 `{方法签名}`
- **Service 方法：** `{XxxAppService}` 新增 `{方法}`
- **Redis Key：** 新增 `{key 格式}` (TTL {n}s)

**错误码：**

| 错误码 | 含义 |
| --- | --- |
| {11101} | {含义} |

**约束：**

- {约束 1}
- {约束 2}

---

## 8. 接口 2：{接口名} `选填`

（结构同上）

---

## 9. 关键词反向索引 `选填`

| 关键词 | 出现位置 |
| --- | --- |
| {类名} | §2 / §7 |
| {方法名} | §7 |
| {Redis Key} | §3 / §7 |

---

## 10. 待确认事项 `选填`

| ID | 问题 | 影响范围 | 优先级 | 待查 |
| --- | --- | --- | --- | --- |
| F-{NNN} | {问题} | {范围} | {🟠/🟡} | {方法} |
