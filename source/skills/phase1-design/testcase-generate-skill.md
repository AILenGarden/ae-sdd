---
name: testcase-generate
description: TestCase 系列 Step 2 generateSkill。根据已通过 Story Review 的 Story 生成测试用例文档，覆盖 AC、全场景和 L1/L2/L3/L4 分层。
---

# TestCase Generate — 测试用例生成 SKILL

## 与监管器 4 步的关系

本文件只负责 **TestCase 系列 Step 2：generateSkill**。

| 系列步骤 | 执行方 | 本文件职责 |
|---|---|---|
| Step 1 compact + 调用声明 | 主流程监管器 | 无 |
| Step 2 generateSkill | `testcase-generate-skill.md` | 生成 TestCase 文档与生成报告 |
| Step 3 reviewSkill + Loop | `testcase-review-skill.md` + 主流程监管器 | 接收缺陷报告后重生用例 |
| Step 4 人工审核 | 主流程监管器 | 提供审核摘要材料 |

禁止在本文件内另设逐步人工确认流程；确认、循环、暂停统一由主流程监管器和 `review-loop-skill.md` 管理。

## 输入

必须读取：

| 输入 | 路径 / 来源 | 用途 |
|---|---|---|
| Story 主文档 | 用户提供或 `resolve_path(intent="STORY")` | AC、接口、数据、异常路径 |
| 测试策略 | `source/standards/testing/be-testcase-strategy.md` | 三层覆盖策略 |
| 测试约束 | `source/standards/constraints/testing.md` | HTTP/DB/Mock/断言红线 |
| 测试模板 | `source/templates/testcase/be-testcase-template.md` | 输出格式 |
| 项目资产 | `ae-sdd assets read testcase --project <projectKey>` | 测试工具、项目约定 |

PRD / 原型如存在必须读取；不存在时标注“未提供”，禁止编造业务或 UI 场景。

## 生成规则

### 1. Story 类型识别

按测试策略识别类型，可多选叠加：

| 类型 | 判定证据 | 覆盖策略 |
|---|---|---|
| 状态机 | 状态枚举 / transition / 状态流转 | 完整转换矩阵 + 多步序列 |
| CRUD | 增删改查接口 | 每个操作正常 / 边界 / 规则 / 权限 |
| 回调/Webhook | 外部推送 / 签名 / 幂等 | 签名 + 幂等 + 数据一致 + 异常恢复 |
| 定时任务 | 定时触发 / 批处理 | 正常 + 空跑 + 边界 + 并发 + 异常 |
| 集成/编排 | 多服务 / 事务编排 | 全成功 + 逐环节失败 + 回滚 |

每个判定必须引用 Story/PRD/资产证据；不确定则标 `{待确认}`。

### 2. 三层覆盖

| 层 | 必做 |
|---|---|
| 第一层：类型策略 | 按上表覆盖功能路径，实际用例数不得低于策略公式 |
| 第二层：通用维度 | 字段边界、权限鉴权、并发、关联数据不存在、多步序列、字段组合 |
| 第三层：测试分层 | 每个用例标注 L1/L2/L3/L4 与自动化入口 |

L2/L4 接口测试默认真实 HTTP：`SpringBootTest(RANDOM_PORT) + TestRestTemplate`。MockMvc 只能降级，且必须在用例备注写明原因。

### 3. 测试真实性预埋

每条用例必须包含：

- 测试数据来源：Story AC / Task / 接口示例 / 项目资产。
- 真实链路要求：真实 HTTP / 真实 DB / 单元测试 / 外部依赖 Mock。
- Mock 边界：只 Mock 直接外部依赖，且返回具体业务值。
- 负向或失败注入：核心 AC 至少一个负向验证。
- 自动化入口：`src/test/java/...#method`，无法确定则标 `{待确认}`。

## 输出

写入前必须调用 `document-storage-skill.md`，禁止硬编码路径。

| 输出 | API | 通过标准 |
|---|---|---|
| 测试用例文档 | `save_doc(intent="TESTCASE", storyId, version={major,minor})` | 符合 `be-testcase-template.md` |
| TestCase WriterReport | `save_doc(intent="TESTCASE_WRITER_REPORT", storyId, version={r:N})` | 说明输入、覆盖策略、缺口 |
| 合规性校验报告 | `save_doc(intent="TESTCASE_COMPLIANCE_REPORT", storyId, version={r:N})` | 下方 10 项全通过 |

## 合规性校验

| # | 检查项 | 通过标准 |
|---|---|---|
| TC-G1 | AC 覆盖 | 每个 AC 至少 1 条用例 |
| TC-G2 | 全场景覆盖 | 主流程、异常、边界、权限、并发均有用例或豁免理由 |
| TC-G3 | 类型策略 | 适用 Story 类型的策略公式已满足 |
| TC-G4 | L1 | 核心纯逻辑、状态判定、异常分支有单元测试 |
| TC-G5 | L2 | HTTP 参数、状态码、异常映射有真实 HTTP 用例 |
| TC-G6 | L3 | 写库、索引、事务、SQL 有真实 DB 用例 |
| TC-G7 | L4 | 跨 Story / 多组件路径有端到端用例或豁免理由 |
| TC-G8 | 可执行性 | 前置、步骤、断言、清理动作齐全 |
| TC-G9 | 真实性预埋 | 数据来源、真实链路、Mock 边界、失败注入齐全 |
| TC-G10 | 无杜撰 | 所有字段、接口、错误码均来自输入证据或标 `{待确认}` |

任一未通过：生成报告标 `BLOCKED`，把缺口交给主流程监管器进入 Loop，不得伪造通过。

## 禁止事项

| 禁止 | 正确做法 |
|---|---|
| 只覆盖 AC | 覆盖 AC + 全场景 + 分层 |
| 不读策略/模板/约束 | 先读输入表中必读文件 |
| 用例无数据来源 | 每条写明来源或 `{待确认}` |
| 用 MockMvc 冒充真实 HTTP | 仅降级使用并写原因 |
| 全 Mock 核心落库路径 | L3 用真实 DB/H2/Testcontainers 验证 |
| 生成阶段直接改 Story | 缺 AC/接口时输出缺口，交监管器路由 |

## 执行清单

| # | 动作 | 产出 | 门禁 |
|---|---|---|---|
| 1 | 读取输入 | 输入证据清单 | 必读输入齐，缺失已标注 |
| 2 | 识别 Story 类型 | 类型策略表 | 每项有证据 |
| 3 | 生成三层用例 | TestCase 文档草案 | 覆盖公式满足 |
| 4 | 跑合规性校验 | 校验报告 | TC-G1~TC-G10 全通过 |
| 5 | 落地文档 | TestCase + WriterReport + 校验报告 | `save_doc` 成功，G-DOC-STORAGE ✅ |
