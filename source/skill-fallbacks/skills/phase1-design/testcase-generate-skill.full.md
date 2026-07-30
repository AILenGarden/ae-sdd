---
name: testcase-generate
description: TestCase 系列 Step 2 generateSkill。采用有界风险驱动范式：先建立有限风险登记，再按行为等价类和最低充分层级选择用例，以停止条件和预算例外限制无价值边界扩张。
---

# TestCase Generate — 测试设计能力（独立文档按需）

> **v3.12：** 默认把验证矩阵写入 Story，不生成独立 TestCase。仅当独立场景超过 20 条、存在合规交付要求或测试矩阵需要独立所有权时，才显式 `doc save --intent TESTCASE`。不生成 ComplianceReport/TestCaseReview Markdown。

## 与监管器 4 步的关系

本文件只负责 **TestCase 系列 Step 2：generateSkill**。

| 系列步骤 | 执行方 | 本文件职责 |
|---|---|---|
| Step 1 compact + 调用声明 | 主流程监管器 | 无 |
| Step 2 generateSkill | `testcase-generate-skill.md` | 生成 TestCase 文档与生成报告 |
| Step 3 reviewSkill + Loop | `testcase-review-skill.md` + 主流程监管器 | 接收缺陷报告后重生用例 |
| Step 4 人工审核 | 主流程监管器 | 提供审核摘要材料 |

禁止在本文件内另设逐步人工确认流程；确认、循环、暂停统一由主流程监管器和 `review-loop-skill.md` 管理。

## 第零步：TestCase 准入检查

必须读齐：

- Story 主文档（AC/接口/数据/异常路径来源）
- 项目资产（测试工具与项目约定）
- 项目约束（HTTP/DB/Mock/断言红线）
- 测试策略 + 测试约束 + 测试模板（生成范式与输出格式）

任一缺失，禁止进入生成。

**🔴 机械门禁（v3.9.1，对齐 RA/Coding 三合一）：** prose 清单之外，必须跑：

```bash
ae-sdd gates check --only G-TESTCASE-CTX
```

`G-TESTCASE-CTX` 校验 `constraints/assets/Story` 三类上下文已读齐（注册表 `CONTEXT_GATE_REGISTRY`，复用 `document-storage-skill` 的 `get_assets` API + `paths.find_doc`；约束直接读取 `constraints/` 目录，索引 `constraints/README.md`）。未过 → **BLOCK，禁止进入生成**。

**读取入口（v3.9.1 显式化）：**
- Story：`ae-sdd doc resolve --intent STORY --story-id {S}`
- 项目资产：`document-storage-skill.get_assets(projectKey)` + `ae-sdd assets read testcase --project <projectKey>`
- 项目约束：直接读取 `constraints/` 目录（索引 `constraints/README.md`）→ `constraints/*.md`

## 输入

### 场景推导前置

先从 Story/API contract 生成 CapabilityModel：能力、前后状态、独立观察面、变化维度、不变量、扰动轴和失败机制。再生成最小 ScenarioManifest；不得从 CRUD 名称或固定示例复制用例。每条场景必须写 `rationale`、`detects`、独立观察面、可重复 command/isolation/cleanup，并证明它能发现具体缺陷。新 HTTP executionPlan 写 `scenarioPolicyVersion: 1`，每个 `boundary=http` verification 写项目内 `scenarioManifest` 相对路径。

必须读取：

| 输入 | 路径 / 来源 | 用途 |
|---|---|---|
| Story 主文档 | 用户提供或 `ae-sdd doc resolve --intent STORY --story-id {S}` | AC、接口、数据、异常路径 |
| 测试策略 | `source/standards/testing/be-testcase-strategy.md` | 假设驱动范式 + 三层覆盖策略 |
| 测试约束 | `source/standards/constraints/testing.md` | HTTP/DB/Mock/断言红线 |
| 测试模板 | `source/templates/testcase/be-testcase-template.md` | 输出格式（含缺陷假设字段） |
| 项目资产 | `ae-sdd assets read testcase --project <projectKey>` | 测试工具、项目约定 |

**🆕 v3.6.3 缺陷信号源（假设挖掘的项目素材，缺失时标注"未加载"，禁止编造）：**

| 信号源 | 读取方式 | 信号形态 |
|---|---|---|
| assets §6.9 隐性约定 | `ae-sdd assets section §6.9` | 踩坑 Story 出处 + 反向链接 |
| assets §10 团队惯用反模式 | `ae-sdd assets section §10` 筛"反模式" | 错误写法 + 文件路径 |
| assets §14 安全隐患登记表 | `ae-sdd assets section §14` | 结构化已发现问题 |
| function/ 同类 Story §1.8 关键约束 | 读 function/{同类Story}.md | 约束 + 出处/反例 |
| config/ 模板 §5 已知踩坑 | 读 config/{serviceKey}.md | 显式坑库 |
| 同类 Story 历史测试报告失败用例 | 读同类 Story 的 *-testcase-report.md | 代码缺陷根因（最高质量信号） |

PRD / 原型如存在必须读取；不存在时标注"未提供"，禁止编造业务或 UI 场景。

## 生成规则

### 0. 有限风险登记与缺陷假设挖掘

> **范式定位：** 先挖“这个 Story 最可能踩什么坑”，但命中的假设只是候选，不自动生成用例。候选必须经过边界测试准入、行为等价类合并、局部数量上限和选择决策。
>
> **为什么必须做：** 三层覆盖矩阵保证的是**结构完整性**，不是**缺陷发现力**。一个填满的 N×(N-1) 状态机矩阵可以覆盖率 100%，却完全错过"两个并发请求在特定时序下导致状态丢失"。缺陷假设挖掘解决"挖不出真实 BUG"。

**双向挖掘机制：**

**① 自上而下：通用假设库匹配**

拿 `be-testcase-strategy.md §通用缺陷假设库`（7 大类：并发/事务/边界/状态机/集成/安全/时序资源）逐条对照当前 Story，形成候选：

```
对每条假设 H-{类}-{N}：
  判定"这条假设在本 Story 的业务上下文中可能触发吗？"
  ├─ 是 → 进入有限风险登记，标注"证据来源=通用库"
  └─ 否 → exclude（说明不适用依据，不生成用例）
```

**② 自下而上：项目坑库挖掘**

读取 §输入 的缺陷信号源（assets §6.9/§10/§14 + function §1.8 + config §5 + 同类 Story 历史测试报告），从中提取项目特有坑：

```
对每个信号源中的踩坑/反模式/缺陷：
  转化为一条风险候选 C-{类}-PROJ-{N}
  标注"信号来源=项目坑:{具体出处}"
  例：assets §6.9"事务中禁止远程调用" → H-TX-PROJ-1"本 Story 的 Feign 调用是否在事务内"
```

**③ Story 业务分析：基于 Story 自身逻辑挖掘**

基于 Story 的数据模型/接口契约/主流程/异常流程，分析业务特有风险：

```
Story 含状态流转 → 挖状态机风险候选
Story 含并发写操作 → 挖并发风险候选
Story 含外部依赖 → 挖集成风险候选
Story 涉及金额 → 挖高风险边界候选（BigDecimal 精度）
... 等
```

**产出：有限风险登记**

| 候选 ID | 类型/风险等级 | 证据来源 | 行为分区 | 独立失败机制 | 最低充分层级 | 选择决策 | 合并至/覆盖用例 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C-CONC-1 | 并发/high | 通用库 + Story 写路径 | 乐观锁冲突 | 丢失更新 | L3 | keep | H-CONC-1 / TC-001 |
| C-BND-2 | 边界/low | 无显式契约 | 与既有 validator 相同 | 无 | L2 | exclude | N/A |

`keep` 候选转为 H-{N} 缺陷假设并必须被用例覆盖；`merge` 指向已有 H/TC；`exclude/defer` 保留理由但不计入待覆盖假设。登记随 TestCase 文档落地，供 testcase-review TC-10 校验选择决策与停止条件。

### 1. Story 类型识别

按测试策略识别类型，可多选叠加：

| 类型 | 判定证据 | 覆盖策略 |
|---|---|---|
| 状态机 | 状态枚举 / transition / 状态流转 | 按 guard、副作用和失败机制分区 |
| CRUD | 增删改查接口 | 按业务规则、约束和权限风险分区 |
| 回调/Webhook | 外部推送 / 签名 / 幂等 | 按签名、幂等、一致性和恢复机制分区 |
| 定时任务 | 定时触发 / 批处理 | 仅适用的阈值、锁、部分失败和资源风险 |
| 集成/编排 | 多服务 / 事务编排 | 按回滚、补偿、重试和降级语义分区 |

每个判定必须引用 Story/PRD/资产证据；不确定则标 `{待确认}`。

### 2. 用例生成（风险驱动 + 有界查漏）

**🆕 v3.6.3 三类用例分级：**

| 类型 | 生成方式 | 是否需假设 | 价值 |
|------|---------|-----------|------|
| **证伪用例**（主价值） | 覆盖一个或多个 `keep` 风险，一个用例可证伪多个假设 | ✅ 必填 H-{N} | 高 — 直接针对独立失败机制 |
| **AC 代表用例**（基础价值） | 以最少用例覆盖 AC 正常契约；一条可覆盖多个 AC | 可无 H，必标 AC | 合理 — 验证约定行为 |
| **查漏用例**（条件价值） | 三层候选矩阵发现且通过准入的新风险 | 可无 H，必标证据和独立机制 | 仅在增加新证据时保留 |
| **无增量价值用例** | 无来源，或与现有用例同分区/机制/断言 | — | 🔴 merge/exclude |

**覆盖兜底矩阵（查漏工具，详细规则见 `be-testcase-strategy.md`）：**

| 层 | 用法 |
|---|---|
| 第一层：类型策略 | 发现该类型的风险候选，不设数量下限 |
| 第二层：通用维度 | 仅适用时发现边界、权限、并发、关联、多步和组合候选 |
| 第三层：测试分层 | 为 `keep` 风险选择最低充分层级；跨层重复需独立机制 |

生成结束前执行停止条件：AC、`keep` 风险、改动分支和历史回归均已映射，剩余候选不增加新的失败机制、控制流、契约、协议、断言或层级证据。超出局部数量上限必须记录预算例外的新增价值、执行/维护成本、不可合并原因和确认人。

L2/L4 接口 AC 固定真实 HTTP 双阶段：`boundary=http`、`stages=[local,test-env]`、`internalMocksAllowed=false`。本地 `RANDOM_PORT + HTTP client` 走完整内部链，随后以同一 buildId 请求测试环境；MockMvc、直接 Controller 调用和内部 MockBean/SpyBean 都不能关闭 AC。

### 3. 测试真实性预埋

每条用例必须包含：

- 测试数据来源：Story AC / Task / 接口示例 / 项目资产。
- 真实链路要求：HTTP AC 同时规划 `http-local` 与 `http-test-env` evidence；核心 DB 真实验证。
- Test double 边界：只用于非接口单测或外部 supplemental 故障注入，不得替换内部 Service/Repository/Mapper/Application 主链。
- 负向或失败注入：核心 AC 存在可追溯的独立失败机制时保留代表用例；不得按 AC 机械追加。
- 自动化入口：`src/test/java/...#method`，无法确定则标 `{待确认}`。

## 输出

写入前必须调用 `document-storage-skill.md`，禁止硬编码路径。

| 输出 | API | 通过标准 |
|---|---|---|
| 测试用例文档 | `ae-sdd doc save --intent TESTCASE --work-item {W} --story-id {S?} --content-file 草稿.md` | 符合 `be-testcase-template.md` |
| TestCase Review 报告 | `ae-sdd doc save --intent TESTCASE_REVIEW --work-item {W} --story-id {S?} --content-file 草稿.md` | 带 r{N} |
| 合规性校验报告 | `ae-sdd doc save --intent TESTCASE_COMPLIANCE_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md` | 带 r{N} |

## 合规性校验

| # | 检查项 | 通过标准 |
|---|---|---|
| TC-G1 | AC 覆盖 | 每个 AC 至少被一个用例覆盖；允许一条用例覆盖多个 AC |
| TC-G2 | 风险覆盖 | 每个 `keep` 风险已映射；高影响风险未被预算静默排除 |
| TC-G3 | 有界选择 | 候选均有选择决策，行为等价项已合并，停止条件已满足 |
| TC-G4 | L1 | 适合纯逻辑层暴露的独立失败机制在 L1 验证 |
| TC-G5 | L2/L4 | 接口 AC 规划本地完整内部链 + 同 buildId 测试环境 HTTP；缺任一阶段为 BLOCKED |
| TC-G6 | L3 | 仅 DB 约束、事务、SQL 的独立风险使用真实 DB |
| TC-G7 | L4 | 仅跨 Story / 多组件的独立链路风险使用端到端测试 |
| TC-G8 | 可执行性 | 前置、步骤、断言、清理动作齐全 |
| TC-G9 | 真实性预埋 | 数据来源、真实链路、Mock 边界、失败注入齐全 |
| TC-G10 | 无杜撰 | 所有字段、接口、错误码均来自输入证据或标 `{待确认}` |
| **TC-G11** | **测试组合价值与有界性** | 见下方专项校验 |

任一未通过：生成报告标 `BLOCKED`，把缺口交给主流程监管器进入 Loop，不得伪造通过。

### TC-G11 测试组合价值与有界性专项校验

1. **登记完整性**：每个候选都有证据来源、风险等级、行为分区、独立失败机制、最低充分层级和选择决策。
2. **双向追溯**：每个 `keep` 风险至少被一个用例覆盖；每个用例能追溯到 AC 或 `keep` 风险；一个用例可覆盖多个来源。
3. **去重与局部上限**：同一 validator/错误分支一个代表；组合无笛卡尔积；状态按 guard/副作用分区；跨层重复有层级特有机制。
4. **高风险保护**：安全、权限、金额、数据丢失、事务、并发、幂等和不可逆状态命中适用条件时不得因预算排除。
5. **停止条件**：剩余候选不再增加新失败机制、控制流、契约、协议、断言或层级证据。
6. **预算例外**：超出局部数量上限时，新增价值、执行成本、维护成本、不可合并原因和确认人齐全。

任一项不满足则 `BLOCKED`。禁止通过增加低价值用例数量来修复；应补证据、合并重复项、修正选择决策或取得预算例外确认。

## 禁止事项

| 禁止 | 正确做法 |
|---|---|
| 只覆盖 AC | 在 AC 之外补充有证据且通过准入的独立风险 |
| 不读策略/模板/约束 | 先读输入表中必读文件 |
| 用例无数据来源 | 每条写明来源或 `{待确认}` |
| 用 MockMvc、方法调用或内部 MockBean 冒充真实 HTTP | 改为真实端口完整内部链；MockMvc 不属于接口验收 |
| 全 Mock 核心落库路径 | L3 用真实 DB/H2/Testcontainers 验证 |
| 生成阶段直接改 Story | 缺 AC/接口时输出缺口，交监管器路由 |
| 跳过有限风险登记直接填矩阵 | 先登记候选，再执行准入、合并和选择决策 |
| 生成无独立失败机制的用例 | merge/exclude，并指向已有覆盖或说明理由 |
| 缺陷信号源缺失时编造坑内容 | 标注"未加载"，降低置信度，不用数量配额补偿 |
| 为满足层级完整而复制同一场景 | 选择最低充分层级；独立层级风险另行准入 |
| 停止条件满足后继续扩张 | 立即停止；确需扩展时走预算例外 |

## 执行清单

| # | 动作 | 产出 | 门禁 |
|---|---|---|---|
| 1 | 读取输入（含缺陷信号源） | 输入证据清单 | 必读输入齐，缺失已标注，不编造 |
| 1.5 | 有限风险登记与选择 | 候选表 + H-{N} | 每条有证据、分区、机制、层级、选择决策和覆盖关系 |
| 2 | 识别 Story 类型 | 类型策略表 | 每项有证据 |
| 3 | 生成最小充分用例组合 | TestCase 文档草案 | AC 与 `keep` 风险已映射，停止条件满足 |
| 4 | 跑合规性校验（含 TC-G11） | 校验报告 | TC-G1~TC-G11 全通过，无无界扩张 |
| 5 | 落地文档 | TestCase + WriterReport + 校验报告 | `save_doc` 成功，G-DOC-STORAGE ✅ |
