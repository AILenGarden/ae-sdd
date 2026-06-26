---
name: be-codereview-template
description: BE CodeReview 报告空白模板 — 配合 code-review-skill.md 使用。9 章节空白 + 七道闸汇总 + UpdatePlan 空白。本模板只定通用结构，工程特化（如分层结构、命名约定）从项目资产 §3/§4 读取。
---

# {STORY-ID} CodeReview 报告 v{N}-r{M}

> **配套 SKILL：** [`code-review-skill.md`](../../code-review-skill.md) — 评审流程 / 6 阶段评审 / 7 道闸 / UpdatePlan 详见该 SKILL。
>
> **🔴 设计原则：** 模板**不写死**分层结构（哪些层、层序、命名）。分层从项目资产 `{projectKey}.assets.md §3` 读取。本模板只定"按什么顺序、含什么列、跑什么闸"。
>
> **🔴 强制：** 出报告前必须过 7 道闸 + 6 阶段评审 + 用户审核点 4 确认。

---

## 报告元信息

| 字段 | 值 |
|------|---|
| **报告时间** | {YYYY-MM-DD HH:mm} |
| **Story ID / 标题** | {STORY-ID} / {标题} |
| **报告版本** | v{N}-r{M}（Story 版本-Coding 轮次） |
| **报告人** | AI（Claude Code） + Reviewer: {name/agent} |
| **项目分层来源** | `{projectKey}.assets.md §3 抽象分层映射表`（必填） |
| **项目分层** | 从 §3 抽取：{例：BFF 入口 / 跨模块 SPI / Interfaces / Application / Domain / Infrastructure / 测试 / 文档} |
| **审核阶段** | Phase 3 ⑦ 节点 |

---

## 零、全切面一致性核查表（🔴 ⑥bis 闸，CodeReview 硬前置）

> **🔴 来源：** `code-review-skill.md §闸 1 ⑥bis 一致性闸`。
> **以当前磁盘代码为锚**，反向核查 DR / Story / Task / 测试用例 / 代码 五方一致。

| DR 章节 | Story 章节 | Task 章节 | 测试用例 ID | 代码文件:行号 | 一致/漂移 | 证据（grep/行号/test method）|
|---------|-----------|-----------|------------|--------------|----------|------|
| | | | | | ✅/🔴 | |

**🔴 漂移项 = 阻断型 CodeReview 问题，按层级回改（代码错改码/设计落后回写/DR 缺陷走异常路径）。**

---

## 一、本次实现业务概览

### 1.1 Story 信息

| 字段 | 内容 |
|------|------|
| Story ID / 标题 | {STORY-ID} / {标题} |
| 涉及工程 | {工程1、工程2}（与项目资产 §2 对齐） |
| 核心业务价值 | {一句话} |
| 涉及 AC 数 | {N}（N=Story §AC 章节的 AC 总数） |

### 1.2 业务理解摘要

> 用 2-3 句话描述本次实现了什么，核心价值是什么。
> 这一节让读者（架构师/技术负责人）5 秒内理解这个 Story 在做什么。

---

## 二、核心业务逻辑详解（🔴 阶段 A 评审产物）

### 2.1 主流程（按调用链路描述）

> **必读：按项目资产 §3 调用层级自上而下填表。** 例：BFF → SPI → Service → Domain → DB。

| 步骤 | 操作 | 入口类 → 方法 | 数据变更 | 异常处理 |
|------|------|-------------|---------|---------|
| 1 | | 类:方法 | | 异常类 → 错误码 |
| 2 | | | | |
| ... | | | | |

### 2.2 AC × 实现 × 测试 对照表（🔴 必填，每个 AC 一行）

| AC_ID | AC 描述 | 实现位置（类:方法）| 测试方法 | 真实 DB/HTTP | 状态 |
|-------|---------|------------------|---------|------------|------|
| AC-001 | {描述} | `ClassName.methodName` | `TestClass.testXxx` | ✅/❌ | ✅/❌ |
| AC-002 | | | | | |
| ... | | | | | |

> **🔴 禁止"AC 已实现"无证据**：每行必须附"实现位置 + 测试方法"两列。

### 2.3 状态机流转规则（如有）

| 当前状态 | 操作 | 目标状态 | 触发条件 | 实现位置 |
|---------|------|---------|---------|---------|
| | | | | `domain/.../service/XxxDomainService.transitionTo()` |

### 2.4 核心业务规则清单

| # | 规则描述 | 规则来源 | 实现位置 | 校验时机 |
|---|---------|---------|---------|---------|
| BR-01 | | DR/Story AC | 类:方法 | |

### 2.5 阶段 A 评审结论

| 评审维度 | 发现问题数（🔴/🟠/🟡/🟢）| 状态 |
|---------|-------------------------|------|
| 主流程步骤 | / / / | ✅/⚠️/❌ |
| AC 实现 | / / / | ✅/⚠️/❌ |
| 业务规则 | / / / | ✅/⚠️/❌ |
| 异常流程 | / / / | ✅/⚠️/❌ |
| 边界场景 | / / / | ✅/⚠️/❌ |
| **小计** | **/ / /** | |

---

## 三、数据库逻辑链（🔴 阶段 C 评审产物）

### 3.0 DB 行为全量清单（必查，禁止遗漏）

| # | 操作类型 | 目标表 | 具体行为 | 实现类 → 方法 | Mapper 方法 | 事务边界 | 并发控制 |
|---|---------|-------|---------|-------------|-----------|---------|---------|
| 1 | | | | | | | |

### 3.1 涉及的表结构（必符合项目资产 §6.5）

| 表名 | 用途 | 主键 | 索引 | 本 Story 涉及字段 |
|------|------|------|------|----------------|
| | | | | |

**🔴 必填审计四字段**：id / created_by / created_date / last_updated_by / last_updated_date

### 3.2 写操作链路（写明 SQL + WHERE + 乐观锁）

```sql
-- 例:
UPDATE boss_user
SET status = #{newStatus}, last_updated_date = NOW()
WHERE id = #{id}
  AND status = #{oldStatus}  -- 乐观锁
```

### 3.3 阶段 C 评审结论

| 评审维度 | 状态 | 备注 |
|---------|------|------|
| DDL 规范 | ✅/❌ | 审计四字段齐全 |
| 索引 | ✅/❌ | 单表 ≤ 5 / varchar 索引指定长度 |
| 事务边界 | ✅/❌ | `@Transactional` 范围 / 事务内禁远程 |
| SQL WHERE | ✅/❌ | UPDATE/DELETE 必含 WHERE |
| EXPLAIN | ✅/❌ | 复杂查询 EXPLAIN 验证 |
| 乐观锁 | ✅/❌ | 高频更新场景必须 |
| **小计** | | |

---

## 四、接口与外部依赖

### 4.1 对外 SPI 接口清单

| 接口 | 类路径 | 方法 | 入参 | 出参 | 幂等 | 调用方 |
|------|-------|------|------|------|------|-------|
| | | | | | | |

### 4.2 外部服务调用

| # | 服务名 | 接口 | 调用时机 | 参数 | 异常策略 |
|---|-------|------|---------|------|---------|
| | | | | | |

### 4.3 被调用接口（调用方是本 Story）

| 接口 | 类路径 | 调用场景 |
|------|-------|---------|
| | | |

---

## 五、分层实现清单（🔴 阶段 B 评审产物）

> **必读：按项目资产 §3 实际分层填表（不是变更类型）。** 模板不写死 BFF/SPI/Interfaces 等层级，由具体项目 §3 决定。

### 5.X {项目第 1 层}（按 §3 实际填写）

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| | | | | |

### 5.X+1 {项目第 2 层}
（同结构）

### 5.X+N {项目第 N 层}
（同结构）

### 5.分层职责红线核查（🔴 4 大红线，违反 = 阻断型）

> **🔴 来源：** `code-review-skill.md §阶段 B 4 大红线核查`。

| 核查项 | 判定 | 证据（类:方法 / 文件:行号） |
|--------|------|---------------------------|
| B1 Repository 方法都是存取语义（findByXxx/save/update/updateStatus）| ✅ / 🔴 | |
| B2 领域逻辑写在 Domain（状态流转/不变量/业务计算）| ✅ / 🔴 | |
| B3 Application 只做编排（@Transactional 边界/调 Domain 顺序）| ✅ / 🔴 | |
| B4 Domain 无 SQL/PO/DTO | ✅ / 🔴 | |
| **小计** | | 0 🔴 才通过 |

### 5.阶段 E 评审结论（项目资产合规性）

> **🔴 来源：** `code-review-skill.md §阶段 E 项目资产合规性核查`。

| 维度 | 必查项 | 状态 | 证据 |
|------|--------|------|------|
| E1 分层映射（§3）| 文件 ↔ §3 表格行 | ✅/❌ | |
| E2 包路径（§4）| 类 ↔ §4 包路径模板 | ✅/❌ | |
| E3 命名约定（§5）| 7 类对象 ↔ §5 命名模板 | ✅/❌ | |
| E4 工程约束（§6）| 9 类约束逐项对照 | ✅/❌ | |
| E5 调用层级（§3）| 文件清单按 §3 排序 | ✅/❌ | |
| **小计** | | |

---

## 六、关键设计决策说明

### 6.1 为什么这样分层？

> {说明架构选型理由}

### 6.2 并发控制方案

| 问题 | 方案 | 选择理由 | 实现位置 |
|------|------|---------|---------|
| | | | |

### 6.3 事务边界

| 方法 | 事务范围 | 事务外操作 | 失败策略 |
|------|---------|---------|---------|
| | | | |

### 6.4 与约束规范的对照（项目资产 §6）

| 约束项 | 规范要求 | 实现情况 | 是否合规 |
|--------|---------|---------|---------|
| §6.1 分层架构 | | | ✅/❌ |
| §6.2 工程结构 | | | ✅/❌ |
| §6.3 代码风格 | | | ✅/❌ |
| §6.4 接口规范 | | | ✅/❌ |
| §6.5 数据库规范 | | | ✅/❌ |
| §6.6 安全规范 | | | ✅/❌ |
| §6.7 测试规范 | | | ✅/❌ |
| §6.8 技术栈范围 | | | ✅/❌ |
| §6.9 隐性约定 | | | ✅/❌ |

---

## 七、CodeReview 自审结论

### 7.1 问题清单（🔴 阻断型 / 🟠 严重 / 🟡 一般 / 🟢 建议）

| # | 检查项 | 等级 | 问题描述 | 修复建议 | 状态 |
|---|--------|------|---------|---------|------|
| 1 | | 🔴/🟠/🟡/🟢 | | | Open/Fixed |

### 7.2 七道闸结果汇总（🔴 全部 ✅ 才能出报告）

> **🔴 来源：** `code-review-skill.md §第七步 7 道闸`。

| 闸 | 名称 | 等级 | 状态 | 备注 |
|----|------|------|------|------|
| 1 | ⑥bis 一致性闸 | 🔴 阻断 | ✅/❌ | 见"零、§全切面一致性核查表" |
| 2 | ⑦bis 对称性闸 | 🔴 阻断 | ✅/❌ | 见 §闸 2 追溯矩阵（追加到本报告或独立 md）|
| 3 | 全文档回扫闸 | 🔴 阻断 | ✅/❌ | 回扫关键字清单 + 残留判定 |
| 4 | 禁裸 ✅ 闸 | 🔴 阻断 | ✅/❌ | 每个 ✅ 必附证据 |
| 5 | 报告-代码对账闸 | 🔴 阻断 | ✅/❌ | 见 §9 产出物对账 |
| 6 | 产出物对账闸 | 🔴 阻断 | ✅/❌ | 见 §9 |
| 7 | 真实 DB/HTTP 覆盖核查闸 | 🔴 阻断 | ✅/❌ | 5 类必真实场景清单 |

**🔴 任一闸 ❌ = 整报告不通过。**

### 7.3 阶段 D 评审：测试真实性 + 真实覆盖

> **🔴 来源：** `code-review-skill.md §阶段 D 8 类禁止扫描 + 5 类必真实场景`。

**8 类禁止扫描结果：**

| # | 禁止手段 | 扫描方法 | 命中数 | 命中位置 |
|---|---------|---------|-------|---------|
| D1 | `@Disabled` / `@Ignore` | grep | | |
| D2 | `assertTrue(true)` 永真 | grep | | |
| D3 | `catch (Exception e) {}` 吞噬 | grep | | |
| D4 | 全 Mock 替代 | spot check | | |
| D5 | 期望值=实际值 | spot check | | |
| D6 | 无效测试数据 | grep | | |
| D7 | `Thread.sleep` 绕过 | grep | | |
| D8 | 凑覆盖率 | 同 D2/D3 | | |

**5 类必真实场景核查：**

| # | 场景 | 必真实 | 实际 | 证据 |
|---|------|--------|------|------|
| 1 | 核心落库 | DB | ✅/❌ | |
| 2 | 事务回滚 | DB | ✅/❌ | |
| 3 | 分布式锁 | Redis | ✅/❌ | |
| 4 | Feign 调用 | HTTP | ✅/❌ | |
| 5 | Redis 缓存失效 | Redis | ✅/❌ | |

### 7.4 阶段 F 评审：跨文档引用核查

> **🔴 来源：** `code-review-skill.md §阶段 F 5 方调用层级一致性`。

| 抽象层 | CodePlan §2 | 模板章节 | Coding 报告文件清单 | 项目资产 §3 | 实际代码 |
|--------|------------|---------|------------------|------------|---------|
| {第 1 层} | | | | | |
| {第 2 层} | | | | | |
| ... | | | | | |

**🔴 任一行"5 方不一致" = ⑥bis 一致性闸未过。**

### 7.5 ⑦bis 对称性闸（追溯矩阵）

> **追溯 ID | DR 条款 | Story 章节 | Task | 代码实现(文件:行号) | 测试用例 ID | 对称性结论**
>
> 🔴 禁止裸 ✅：每行"对称性结论"必须能点到具体章节号/Task 编号/文件:行号/测试用例 ID。

| 追溯 ID | DR 条款 | Story 章节 | Task | 代码实现 | 测试用例 ID | 对称性结论 |
|---------|--------|-----------|------|---------|------------|----------|
| | | | | | | |

**核查范围：🔴 全量覆盖 DR 中本 Story 涉及的全部业务规则，不只本轮改动。**

### 7.6 整改方案（仅当有 🔴/🟠 问题时填写）

| 问题 | 整改方案 | 负责 | 完成标准 |
|------|---------|------|---------|
| | | | |

### 7.7 自审结论汇总

| 检查项 | 结论 | 证据 |
|--------|------|------|
| 业务逻辑正确性 | ✅/⚠️ | 见 §2 |
| 事务边界合理性 | ✅/⚠️ | 见 §3.3 / §6.3 |
| 并发安全 | ✅/⚠️ | 见 §6.2 |
| 异常处理完整性 | ✅/⚠️ | 见 §2.1 |
| 安全性（注入/权限） | ✅/⚠️ | 见 §6.4 §6.6 |
| 性能隐患（慢 SQL/循环调用） | ✅/⚠️ | 见 §3.2 EXPLAIN |
| 代码规范合规 | ✅/⚠️ | 见 §5 §6.4 |
| DR-Story-Task 一致性 | ✅/⚠️ | 见 §零 + §7.5 |
| 项目资产合规 | ✅/⚠️ | 见 §5.阶段 E |
| **七道闸** | ✅/⚠️ | 见 §7.2 |

---

## 八、本次提交 Git 文件清单（按项目资产分层结构排序）

> **🔴 模板不写死**调用层级。具体分层从 `{projectKey}.assets.md §3` 读取。
> **填写前置：** 1. 读项目资产 §3 → 2. 抽本项目调用层级链 → 3. 按层级自上而下填表
> 表格列：# / 文件路径 / 变更类型 / 所属工程 / 关键改动 / 涉及行号
> 用户按此顺序**自上而下 diff 校验**。

### 8.1 统计摘要

```
工程根路径：{workspace_root}
Story ID：{STORY-ID}
报告版本：v{N}-r{M}
项目分层来源：{projectKey}.assets.md §3
合计：X 个（按本项目分层数统计）
```

### 8.2+ 各层级分节（按项目资产 §3 实际分层填写）

> **每节标题为本项目实际层名**（如"BFF 入口层"），不涉及节标 `N/A — {原因}`。

#### 8.X 模板格式（每层用同一结构）

```
**8.{N} {层名}**（如不涉及：`N/A — {原因}`）

| # | 文件路径 | 变更类型 | 所属工程 | 关键改动 | 涉及行号 |
|---|---------|---------|---------|---------|---------|
|   |         | 新增/修改/删除 |   |         |         |
```

**8.实例：icec-cloud-boss 项目分层填写参考（🔴 标"实例"）**

| § | 层级（来自项目资产 §3）| 本 Story 是否涉及 |
|---|----------------------|:---------------:|
| 8.2 | BFF 入口（`icec-cloud-*-bff`）| {✅/❌} |
| 8.3 | 跨模块 SPI（`icec-cloud-life-spi/*-spi`）| {✅/❌} |
| 8.4 | Interfaces（`icec-cloud-*-*-interfaces`）| {✅/❌} |
| 8.5 | Application（`icec-cloud-*-*-application`）| {✅/❌} |
| 8.6 | Domain（`icec-cloud-*-*-domain`）| {✅/❌} |
| 8.7 | Infrastructure（`icec-cloud-*-*-infrastructure`）| {✅/❌} |
| 8.8 | 测试（各模块 `src/test`）| {✅/❌} |
| 8.9 | 文档（`docs/`、`.auto-engineering/`）| {✅/❌} |

> 🔴 上表是 **icec-cloud-boss 项目实例**，**不**是通用模板。

### 8.变更类型汇总

| 变更类型 | 计数 | 节内位置 |
|---------|:---:|---------|
| 新增 | {N} | |
| 修改 | {N} | |
| 删除 | {N} | |
| **合计** | **{N}** | — |

### 8.Git 提交命令

```bash
# 按本项目分层顺序 add（与 §8.2-§8.X 排序一致）
git add {第1层_files}
git add {第2层_files}
...
git add {第N层_files}
git commit -m "[ST-{id}] {story_title}"
```

---

## 九、产出物对账（🔴 闸 6）

> AI 必须验证以下每个产出物真实存在，并与报告描述一致。

| 产出物 | 实际路径 | 是否存在 | 与报告一致 |
|--------|---------|---------|----------|
| Story 文档 | `design/story/be/{STORY-ID}.md` | □ | □ |
| 统一版 CodePlan | `design/story/be/coding/{STORY-ID}/{STORY-ID}-CodingPlan.md` | □ | □ |
| Coding 报告 | `design/story/be/coding/{STORY-ID}/{STORY-ID}-CodingReport-v{N}-r{M}.md` | □ | □ |
| 测试用例 | `design/testcase/be/{STORY-ID}/{STORY-ID}-testcase.md` | □ | □ |
| 测试报告 | `design/testcase/be/{STORY-ID}/{STORY-ID}-Report-v{N}-r{M}.md` | □ | □ |
| 源代码 | 工作目录对应工程 | □ | □ |
| **CodeReview 报告** | `design/story/be/coding/{STORY-ID}/{STORY-ID}-CodeReview-v{N}-r{M}.md` | □ | □ |
| 项目资产（如有更新）| `skills/ae-sdd/project-assets/{projectKey}/` | □ | □ |

> 🔴 任何产出物不存在或不一致 → 必须修正报告或补充产出物，不得跳过。

---

## 十、CodeReviewUpdatePlan（🔴 改代码前硬前置）

> **🔴 强制：** 任何修改动作（含改代码、改 Story、改 Task、改项目资产）必须先有 Plan，按 Plan 执行。
> **🔴 Plan 未确认前禁止触发任何下游 SKILL。**

### 10.1 问题清单（按 🔴/🟠/🟡/🟢 分级）

- 🔴 CR-001: {缺陷描述}（阶段 A）→ 改代码
- 🟠 CR-002: {缺陷描述}（阶段 B）→ 改项目资产
- ...

### 10.2 存疑项（评审时不确定的事项）

- {X} 不确定是否合理 → 询问用户
- {Y} 不确定是否符合项目资产 §6 → 询问用户

### 10.3 更新计划（按修复路径分组）

- **改代码组**（CR-001, CR-003）：涉及 N 个类 / M 个方法
  - 类 1：{修改内容}
  - 类 2：{修改内容}
- **改项目资产组**（CR-002）：涉及 §X.Y 章节
- **改 Story 组**（CR-005）：涉及 Story §X.Y 章节
- **改 Task 组**（CR-006）：涉及 Task-X §X.Y 章节

### 10.4 字段链路影响分析（如涉及 §6 全链路映射）

- 字段 X 改动：触入 HTTP ✓ / BFF ✓ / SPI ✓ / DO ✓ / DB ✓（5 层都要同步）
- 字段 Y 改动：仅 DB 层（无需同步）

### 10.5 触发路径

- 改代码 → 触发 Coding SKILL（按 Coding 实时追溯链：Task → Story → DR）
- 改项目资产 → 触发 Project Assets Update SKILL
- 改 Story → 触发 Story Update SKILL
- 改 Task → 触发 Task Generate SKILL

### 10.6 执行后验收

- 7 道闸重新过一遍
- 6 阶段评审重新跑
- 连续 3 轮无新增问题才退出循环（3 轮仍有 🔴 → 升级用户）

### 10.7 用户确认

- ☐ 通过，按 Plan 执行
- ☐ 需要调整（说明）

---

## 报告结束

> 报告生成后：
> 1. 跑 7 道闸最终校验（§7.2）
> 2. 跑 10 项报告合规性校验（见 `code-review-skill.md §第七步 bis`）
> 3. **全 ✅** 后进入人工审核点 4
> 4. 用户必须明确"确认 CodeReview 报告"才能进入 ⑨ 完成判定

---

## 附录 A：填示例（🔴 仅作参考，填写时按本 Story 实际数据替换）

> **🔴 此节是冰山模块 `icec-cloud-boss-user` 的 CodeReview 报告填示例**，供后续 Story 填写时参考。
> **实际填写必须按本 Story 所在项目的 §3 实际分层、§6 实际约束。**

### A.1 报告元信息示例

```
报告时间：2026-06-05 14:30
Story ID / 标题：STORY-001-BE / Boss 用户列表查询接口
报告版本：v1-r1
报告人：AI（Claude Code） + Reviewer: reviewer-BE + reviewer-AR
项目分层来源：icec-cloud-boss.assets.md §3
项目分层：BFF 入口 / 跨模块 SPI / Interfaces / Application / Domain / Infrastructure / 测试 / 文档
审核阶段：Phase 3 ⑦
```

### A.2 §一 1.1 Story 信息示例

| 字段 | 内容 |
|------|------|
| Story ID / 标题 | STORY-001-BE / Boss 用户列表查询接口 |
| 涉及工程 | icec-cloud-boss-user, icec-cloud-boss-user-bff |
| 核心业务价值 | 支持后台管理员分页查询用户列表（含角色过滤、状态过滤）|
| 涉及 AC 数 | 5（AC-001 ~ AC-005）|

### A.3 §二 2.2 AC×实现×测试 对照表示例

| AC_ID | AC 描述 | 实现位置 | 测试方法 | 真实 DB/HTTP | 状态 |
|-------|---------|---------|---------|------------|------|
| AC-001 | 管理员能分页查询所有用户 | `BossUserAppService.list(BossUserQuery): PagedModels<BossUserDTO>` | `BossUserAppServiceIT.testList_Success` | ✅（@SpringBootTest + TestRestTemplate）| ✅ |
| AC-002 | 按角色 ID 过滤 | `BossUserAppService.list` 内 `query.roleId` 分支 | `BossUserAppServiceIT.testList_FilterByRoleId` | ✅ | ✅ |
| AC-003 | 按状态过滤（启用/禁用）| `BossUserAppService.list` 内 `query.status` 分支 | `BossUserAppServiceIT.testList_FilterByStatus` | ✅ | ✅ |
| AC-004 | 空结果返回空列表（不抛异常）| `BossUserAppService.list` 空集合返回 | `BossUserAppServiceIT.testList_EmptyResult` | ✅ | ✅ |
| AC-005 | 越权（superManager=0 调该接口）拒绝 | `BossUserRestImpl` `@RequiresPermissions` | `BossUserControllerIT.testList_Forbidden` | ✅ | ✅ |

### A.4 §五 5.X 分层实现清单示例（按 icec-cloud-boss §3 实际分层）

#### 5.1 BFF 入口层

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| `BossUserManagementRestImpl` | BFF 端用户列表查询 | `list(BossUserManagementRequest): ApiResult<PagedModels<BossUserVO>>` | icec-cloud-boss-user-bff | `interfaces/restful/BossUserManagementRestImpl.java:42` |

#### 5.2 跨模块 SPI 层

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| `BossUserManagementService` | 跨服务 Feign 接口 | `list(BossUserManagementRequest): ApiResult<PagedModels<BossUserDTO>>` | icec-cloud-life-spi/icec-cloud-boss-user-spi | `service/BossUserManagementService.java:18` |

#### 5.3 Interfaces 层

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| `BossUserServiceImpl` | 本模块 HTTP 入口 | `listPage(BossUserRequest): ApiResult<PagedModels<BossUserDTO>>` | icec-cloud-boss-user/icec-cloud-boss-user-interfaces | `restful/BossUserServiceImpl.java:65` |

#### 5.4 Application 层

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| `BossUserAppService` | 业务编排 + 事务 | `list(BossUserQuery): PagedModels<BossUserDTO>` | icec-cloud-boss-user/icec-cloud-boss-user-application | `appservice/BossUserAppService.java:88` |

#### 5.5 Domain 层

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| `BossUserRepository` | 仓储接口 | `findByQuery(BossUserQuery): PagedModels<BossUserDO>` | icec-cloud-boss-user/icec-cloud-boss-user-domain | `repository/BossUserRepository.java:25` |
| `BossUserQuery` | 查询值对象 | （充血，含 roleId/status 等字段）| icec-cloud-boss-user/icec-cloud-boss-user-domain | `model/value/BossUserQuery.java:12` |

#### 5.6 Infrastructure 层

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| `BossUserRepositoryImpl` | MySQL 仓储实现 | `findByQuery(BossUserQuery): PagedModels<BossUserDO>` | icec-cloud-boss-user/icec-cloud-boss-user-infrastructure | `persistence/repository/mysql/BossUserRepositoryImpl.java:52` |
| `BossUserMapper` | MyBatis Mapper | `selectPagedByQuery(@Param("query") BossUserQuery): List<BossUserPO>` | icec-cloud-boss-user/icec-cloud-boss-user-infrastructure | `persistence/dao/mapper/BossUserMapper.java:18` |

#### 5.7 测试层

| 类/接口 | 职责 | 核心方法 | 所在工程 | 文件:行号 |
|---------|------|---------|---------|---------|
| `BossUserAppServiceIT` | Application 层集成测试 | `testList_Success / testList_FilterByRoleId / ...` | icec-cloud-boss-user/icec-cloud-boss-user-application | `src/test/.../BossUserAppServiceIT.java:35` |
| `BossUserControllerIT` | 真实 HTTP 集成测试 | `testList_Success_HTTP / testList_Forbidden` | icec-cloud-boss-user-bff | `src/test/.../BossUserControllerIT.java:48` |

### A.5 §七 7.2 七道闸结果汇总示例

| 闸 | 名称 | 等级 | 状态 | 备注 |
|----|------|------|------|------|
| 1 | ⑥bis 一致性闸 | 🔴 阻断 | ✅ | 见"零、"全切面核查表（5 行 DR↔Story↔Task↔用例↔代码）全一致 |
| 2 | ⑦bis 对称性闸 | 🔴 阻断 | ✅ | 追溯矩阵 3 行（AC-001/002/003）全贯通 |
| 3 | 全文档回扫闸 | 🔴 阻断 | ✅ | 回扫关键字：`BossUser` / `list` / `roleId` / `status` — 0 残留 |
| 4 | 禁裸 ✅ 闸 | 🔴 阻断 | ✅ | 32 个 ✅ 全附 grep/file:line 证据（详见各小节）|
| 5 | 报告-代码对账闸 | 🔴 阻断 | ✅ | 报告能力声明 vs 代码：5/5 一致（grep 验证）|
| 6 | 产出物对账闸 | 🔴 阻断 | ✅ | 7 个产出物全存在 + 与报告一致 |
| 7 | 真实 DB/HTTP 覆盖核查闸 | 🔴 阻断 | ✅ | 5 类必真实：核心落库/事务回滚/分布式锁（不适用）/Feign HTTP/Redis（不适用）全 ✅ |

### A.6 §七 7.3 8 类禁止扫描结果示例

| # | 禁止手段 | 扫描方法 | 命中数 | 命中位置 |
|---|---------|---------|-------|---------|
| D1 | `@Disabled` / `@Ignore` | `grep -rn "@Disabled\|@Ignore" src/test` | 0 | — |
| D2 | `assertTrue(true)` 永真 | `grep -rn "assertTrue(true)" src/test` | 0 | — |
| D3 | `catch (Exception e) {}` 吞噬 | `grep -rn "catch.*Exception.*{}" src/test` | 0 | — |
| D4 | 全 Mock 替代 | 人工 spot check | 0 | — |
| D5 | 期望值=实际值 | 人工 spot check | 0 | — |
| D6 | 无效测试数据 | `grep "userId=1L" src/test` | 0 | — |
| D7 | `Thread.sleep` | `grep "Thread.sleep" src/test` | 0 | — |
| D8 | 凑覆盖率 | 同 D2/D3 | 0 | — |

### A.7 §八 Git 文件清单示例（按项目 §3 层级自上而下）

```
# 8.1 统计摘要
工程根路径：d:\Item\icec-cloud-boss
Story ID：STORY-001-BE
报告版本：v1-r1
项目分层来源：icec-cloud-boss.assets.md §3
合计：7 个（按本项目分层数统计）

# 8.2 BFF 入口层（如不涉及：`N/A`）
8.2 BossUserManagementRestImpl.java | 新增 | icec-cloud-boss-user-bff | 新建 BFF 端用户列表接口 | interfaces/restful/BossUserManagementRestImpl.java:1-100

# 8.3 跨模块 SPI 层（如不涉及：`N/A`）
8.3 BossUserManagementRequest.java | 新增 | icec-cloud-boss-user-spi | 新增请求 DTO | dto/BossUserManagementRequest.java:1-25

# 8.4 Interfaces 层（如不涉及：`N/A`）
8.4 BossUserServiceImpl.java | 新增 | icec-cloud-boss-user-interfaces | 新增本模块 HTTP 入口 | restful/BossUserServiceImpl.java:1-120

# 8.5 Application 层
8.5 BossUserAppService.java | 新增 | icec-cloud-boss-user-application | 新增业务编排 | appservice/BossUserAppService.java:1-150

# 8.6 Domain 层
8.6 BossUserRepository.java | 新增 | icec-cloud-boss-user-domain | 新增仓储接口 | repository/BossUserRepository.java:1-50
8.6 BossUserQuery.java | 新增 | icec-cloud-boss-user-domain | 新增查询值对象 | model/value/BossUserQuery.java:1-30

# 8.7 Infrastructure 层
8.7 BossUserRepositoryImpl.java | 新增 | icec-cloud-boss-user-infrastructure | 新增 MySQL 仓储实现 | persistence/repository/mysql/BossUserRepositoryImpl.java:1-100
8.7 BossUserMapper.java | 新增 | icec-cloud-boss-user-infrastructure | 新增 MyBatis Mapper | persistence/dao/mapper/BossUserMapper.java:1-30
8.7 BossUserDataConverter.java | 新增 | icec-cloud-boss-user-infrastructure | 新增 PO↔DO 转换 | persistence/converter/BossUserDataConverter.java:1-40

# 8.8 测试层
8.8 BossUserAppServiceIT.java | 新增 | icec-cloud-boss-user-application | 新增 AppService 集成测试 | src/test/.../BossUserAppServiceIT.java:1-100
8.8 BossUserControllerIT.java | 新增 | icec-cloud-boss-user-bff | 新增 BFF 真实 HTTP 集成测试 | src/test/.../BossUserControllerIT.java:1-80

# 8.9 文档层（如不涉及：`N/A`）

# 变更类型汇总
新增：12 | 修改：0 | 删除：0 | 合计：12
```
