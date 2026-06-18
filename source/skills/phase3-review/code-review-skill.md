---
name: code-review
description: 端到端代码评审 SKILL — Phase 3 ⑦ 节点的环节内具体规则。与 story-review-skill.md 同等地位，覆盖"Code Review 准入/多维评审/6 大闸门/异常路径/多 Agent 编排"。当 Story 编码完成、Coding 报告 + 测试报告 + CodePlan 出炉时触发；当用户说"Code Review 报告"/"出 CR 报告"/"评审代码"/"审核 Story"时触发。
---

# Code Review — 端到端代码评审 Skill

> **核心定位：** 与 `story-review-skill.md` 同等地位 — Story Review 是设计阶段的评审，**Code Review 是实现阶段的评审**。两者结构对齐（目标/总则/整体流程/触发条件/Plan-first/第零步准入/第一步输入/第二步挖掘/结论/判定/补充说明/UpdatePlan/触发/循环/禁止/执行清单/讲解）。
>
> **与现有 SKILL 的分工：**
> - `ae-sdd-skill.md` = 流程编排（Phase 3 ⑦ 在哪）
> - **`code-review-skill.md`（本文件）** = Code Review 的环节内具体规则（怎么审）
> - [`templates/coding/be-codereview-template.md`](../../templates/coding/be-codereview-template.md) = 评审报告空白模板
> - `coding-skill.md` = Coding 阶段规则，**6 大评审闸门从 coding-skill 迁出到本 SKILL §7**

---

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 生成的 CodeReview 报告在写入磁盘前**必须先调用 [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)** 的 API，**不再手写路径**：
> 1. **路径**（§0.6.1 `resolve_path()`）：通过 `intent=CODE_REVIEW` 自动定位到 `ae-sdd-doc/iterations/{YYYY-MM-DD}/CR/{STORY-ID}/`
> 2. **命名 + 版本号**（§0.6.7 `save_doc()`）：**事件类文档带 `v{N}-r{M}`**
> 3. **重入判定**（§0.6.11 `get_latest_version()`）：CodeReview 重入时**新增报告**（r 递增），不修改历史
> 4. **ChangeLog**（§5）：`save_doc()` 自动追加
> 5. **.gitignore**（§0.6.13 `check_and_update_gitignore()`）：首次写入时自动维护

| 输出文档 | API 调用 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| CodeReview 报告 | `save_doc(intent="CODE_REVIEW", storyId, version={v:N,r:M})` | 带 `v{N}-r{M}` | **新增**（r 递增）|
| ⑦bis 追溯矩阵 | `save_doc(intent="TRACE_MATRIX", storyId, version={v:N,r:M})` | 带 `v{N}-r{M}` | **新增** |
| CodeReviewer 报告（如多 reviewer 模式）| `save_doc(intent="CODE_REVIEW", storyId, docType="CodeReview-{BE/AR/QA}", version={r:M})` | 带 reviewer 类型 + r{M} | **新增** |
| CodeReviewUpdatePlan | 嵌入 CodeReview 报告 §十（**2026-06-06 改造：改为 Proposal 指针 → `proposal-skill.md`）** | 走 Proposal | Proposal 替代 |

> 🔴 **关键：** 评审通过后**触发下游 Proposal 流程**（proposal-skill.md），评审发现的问题统一用 Proposal 描述。

---

## 目标

对已实现的 Story 代码进行**架构师级评审**，目标：

- 业务逻辑正确性 100% 匹配 Story AC
- 分层职责零串味（Domain/Application/Repository 不串）
- 数据库逻辑安全（事务边界 / SQL 性能 / 索引 / 约束）
- 测试真实性（8 类禁止 0 命中）
- 项目资产合规（命名 / 包路径 / 调用层级）
- 6 大闸门全过（一致性 / 对称性 / 全文档回扫 / 禁裸 ✅ / 报告-代码对账 / 产出物对账）
- 评审发现问题**进入异常路径**（走 Coding 实时追溯链 / Story Update / Task Update / Project Assets Update）

---

## Code Review 总则（🔴 贯穿全 SKILL，违反 = 结论无效）

### 标尺 1：证据标准（🔴 禁止裸结论）

每个评审结论必须附**客观证据**：

| 评审项 | 必填证据 |
|--------|---------|
| 业务实现正确 | AC_ID + 测试方法名 + Story 章节号 |
| 分层职责归位 | 类:方法 + 文件:行号 + 引用项目资产 §4 |
| SQL 安全 | Mapper 方法名 + WHERE 条件 + 索引名 |
| 测试真实 | 测试类:方法 + 真实 DB 输出 / 真实 HTTP 状态码 |
| 命名合规 | grep 类名 + 项目资产 §5 命名模板匹配 |
| 调用层级合规 | 文件路径 + 项目资产 §3 层级映射 |

> **🔴 禁止**自我声明"通过"。每个 ✅ 必须配 grep/file:line/test method 这三类证据之一。

### 标尺 2：🔴/🟠/🟡/🟢 分级（与 AE 体系一致）

| 等级 | 定义 | 修复时限 |
|------|------|----------|
| 🔴 阻断型 | 阻断型 6 大闸门任一未过 / 业务实现错误 / 真实 DB 落库路径未验证 | 立即修复，禁止提交 |
| 🟠 严重型 | 性能隐患 / 并发风险 / 资源泄漏 / 异常吞噬 / 测试覆盖 < 项目资产 §6.7 标准 | 24 小时内 |
| 🟡 一般型 | 代码规范 / 命名不一致 / 重复代码 / 设计合理性 | 48 小时内 |
| 🟢 建议型 | 优雅性 / JavaDoc 完整性 / Stream API 优化 | 下次迭代 |

### 标尺 3：完整性度量（🔴 "覆盖所有"必须先有穷举清单）

| 维度 | 穷举清单来源 |
|------|-------------|
| 业务 AC | Story §AC 章节（不重不漏） |
| 分层 | 项目资产 §4 DDD 内部分层落点（每层每类角色穷举） |
| DB | Story §数据模型章节（每张表每列穷举） |
| 测试 | Story AC × 项目资产 §6.7 测试分层（L1/L2/L3/L4） |
| 异常 | Story §异常流程章节 + 评审中识别的边界场景 |

### 标尺 4：语言精确性（🔴 禁用纯主观词）

| ❌ 禁用 | ✅ 替换为 |
|--------|----------|
| "代码不错" / "还可以" | 引用具体证据 + 给出"满足/不满足哪条规则" |
| "性能可能有问题" | "**实测** QPS=X / P99=Y / 慢 SQL: <SQL 文本>（详见 EXPLAIN 截图）" |
| "测试不充分" | "Story N 个 AC 中，M 个已被测试覆盖，K 个未覆盖（列出 AC_ID 清单）" |
| "命名不规范" | "类名 X 违反项目资产 §5 命名约定 `{Resource}AppService`（应为 `XxxAppService`）" |

---

## 整体流程

```
触发（Phase 3 ⑦ 节点 / 用户说"出 CR 报告"）
    ↓
第零步：CodeReview 准入检查（🔴 硬门禁）
    ↓
第一步：读取输入（Story/CodePlan/Coding 报告/测试报告/项目资产/变更文件清单/实际代码）
    ↓
第二步：多维评审（6 阶段并行挖掘）
    ├─ 阶段 A：业务逻辑评审
    ├─ 阶段 B：分层职责红线核查
    ├─ 阶段 C：数据库逻辑链评审
    ├─ 阶段 D：测试真实性 + 真实 DB/HTTP 覆盖核查
    ├─ 阶段 E：项目资产合规性核查
    └─ 阶段 F：跨文档引用核查（与 §3 调用层级一致性）
    ↓
Code Review 结论：{STORY-ID}
    ↓
第三步：合理性判定（🔴/🟠/🟡/🟢 分级）
    ↓
第四步：记入补充说明文档
    ↓
第四步 bis：生成 CodeReviewUpdatePlan（🔴 改代码前硬门禁）
    ↓
第五步：触发下游 SKILL
    ├─ 改代码 → 触发 Coding SKILL（按 Coding 实时追溯链：Task → Story → DR）
    ├─ 改 Story → 触发 Story Update SKILL
    ├─ 改 Task → 触发 Task Generate SKILL
    └─ 改项目资产 → 触发 Project Assets Update SKILL
    ↓
第六步：循环判定（连续 1 轮无新增问题才退出）
    ↓
第七步：CodeReview 闸门全集（7 道闸）
    ├─ 7.1 ⑥bis 一致性闸
    ├─ 7.2 ⑦bis 对称性闸
    ├─ 7.3 全文档回扫闸
    ├─ 7.4 禁裸 ✅ 闸
    ├─ 7.5 报告-代码对账闸
    ├─ 7.6 产出物对账闸
    └─ 7.7 真实 DB/HTTP 覆盖核查闸
    ↓
第七步 bis：CodeReview 报告合规性校验
    ↓
完成 → 触发人工审核点 4 → 交付
```

**关键原则：** 每个步骤必须产出中间结果，用户确认后方可进入下一步。未确认禁止继续。

---

## 触发条件

| 触发场景 | 触发方式 |
|---------|---------|
| Phase 3 ⑦ 节点 | auto-engineering-skill 编排层自动触发 |
| 用户手动 | "出 CR 报告" / "Code Review 报告" / "评审代码" / "审核 Story" |

---

## Plan-first 更新原则（🔴 CodeReview 反馈修改前硬前置）

> **🔴 强制：** Code Review 发现的任何问题，**禁止**直接修改。**必须先有 CodeReviewUpdatePlan**，按 Plan 执行修改。

**Why：** Code Review 阶段涉及多文档同步（Code 改、Story 可能要改、Task 可能要改、项目资产可能要改）。如果没有 Plan，**容易遗漏联动修改**（如改了 Task 但忘了同步 Story 接口契约）。

**Plan 内容（详见 §第四步 bis）：**
- 问题清单（按 🔴/🟠/🟡/🟢 分级）
- 存疑项（评审时不确定的事项）
- 更新计划（每个问题对应修改哪个文档/代码）
- 字段链路影响分析（如改 DO 字段需同步 BFF / SPI / DB 列）
- 执行后验收（修改后如何验证）

---

## 第零步：CodeReview 准入检查（🔴 硬门禁，未通过禁止进入 Review）

**🔴 必读清单（4 个文件）：**

| # | 文件 | 读取要求 |
|---|------|---------|
| 1 | Story 主文档 | 全部章节（含 AC / 接口契约 / 数据模型 / 异常流程 / 验收标准） |
| 2 | 统一版 `{STORY-ID}-CodingPlan.md` | 全部 16 节（用户已确认） |
| 3 | Coding 报告 | `{STORY-ID}-CodingReport-v{N}-r{M}.md`（本轮变更文件清单 + 编译/测试结果） |
| 4 | 测试报告 | `{STORY-ID}-Report-v{N}-r{M}.md`（含 L1/L2/L3/L4 用例结果） |
| 5 | 项目资产 | 调用 `project-assets-update-skill.assets.forCodeReview(projectKey)` 返回 §6 + §C（字段索引）+ §D（组件索引）+ §3 + §4 + §5（**禁止**直接全文读取 assets.md）|

**门禁判定：**
- ✅ 5 个文件全部读取 + 用户确认 → 进入第一步
- ❌ 任一文件未读取 / 未确认 → **停止，补充读取 + 用户确认后再继续**
- 🔴 缺统一版 CodePlan 或项目资产 → **禁止**继续 Review（先让用户跑 task-generate-skill §6 汇总）

**第零步 产出：准入检查记录**

```markdown
## 准入检查记录 - {STORY-ID} - {YYYY-MM-DD HH:mm}

| # | 文件 | 状态 | 备注 |
|---|------|------|------|
| 1 | Story 主文档 | ✅ 已读 / ❌ 未读 | |
| 2 | 统一版 CodePlan | ✅ 已读 / ❌ 未读 | |
| 3 | Coding 报告 | ✅ 已读 / ❌ 未读 | |
| 4 | 测试报告 | ✅ 已读 / ❌ 未读 | |
| 5 | 项目资产 | ✅ 已读 / ❌ 未读 | |

**用户确认：** ☐ 通过 ☐ 补充后再继续
```

---

## 第一步：读取输入

### 1.1 提取 Story 信息

| 信息项 | 来源 | 提取要点 |
|--------|------|---------|
| Story ID / 标题 | Story 元信息 | — |
| 验收标准 AC | Story §AC 章节 | AC_ID + 描述 |
| 接口契约 | Story §接口契约 | URL / 方法 / Request / Response / 错误码 |
| 数据模型 | Story §数据模型 | 表名 / 字段 / 类型 / 索引 |
| 异常流程 | Story §异常流程 | 错误码 / 异常路径 |
| 主流程步骤 | Story §主流程 | 业务步骤 |
| 实现任务映射 | Story §Task 列表 | Task_ID / 名称 / 涉及工程层 |
| 前端契约 | Story §前端契约 | URL / 错误码 / 边界场景（①bis 已审过） |

### 1.2 提取统一版 CodePlan 信息

| 信息项 | 提取要点 |
|--------|---------|
| Tier 选择 | Tier 1 / 2 / 3 |
| §2 抽象分层映射 | 涉及哪些层 |
| §3 Task 执行顺序 | Task 0 公共依赖 / Task 1..N |
| §4 文件级顺序 | 哪些文件 |
| §5 类骨架 | 哪些类 / 哪些方法 / 哪些层 |
| §6 全链路映射 | 字段跨 5 层映射 |
| §7 Mapper/SQL | 关键 SQL / WHERE 条件 |
| §8 测试对应 | AC × 测试方法 |
| §9 验证点 | 编译/启动/接口/DB/事务各项验证 |
| §10 调试回滚 | 失败类型与回滚 |
| §15 10 条门禁 | 全过？ |

### 1.3 提取 Coding 报告信息

| 信息项 | 提取要点 |
|--------|---------|
| 本轮变更文件清单 | 按项目资产 §3 层级（不是变更类型） |
| 编译结果 | BUILD SUCCESS / FAIL |
| 测试结果 | 通过/失败数量 + 失败用例 ID |
| 已知问题 | AI 自报的问题 |

### 1.4 提取测试报告信息

| 信息项 | 提取要点 |
|--------|---------|
| 用例总数 | L1 / L2 / L3 / L4 各多少 |
| 通过率 | 各层通过率 |
| 失败用例 | 失败 ID + 原因 + 严重性 |
| 覆盖率 | 整体 / Service 核心 / Mapper XML / Controller 接口 |
| 测试真实性 | 是否含 8 类禁止（@Disabled / assertTrue(true) / catch 吞噬 / 全 Mock / 期望值=实际值 / Thread.sleep / 凑覆盖率） |

### 1.5 调用项目资产服务

> 🔴 **强制：** 禁止直接读取 `{projectKey}.assets.md` 全文。
> 调用 `project-assets-update-skill.assets.forCodeReview(projectKey)` 返回：

| 返回章节 | 用于 Code Review 的检查维度 |
|---------|--------------------------|
| §6 工程约束 | 9 类约束逐项对照（分层/命名/数据库/安全/测试等）|
| §C 字段索引 | DB 字段对齐核查（E4 阶段）|
| §D 组件索引 | 复用核查：是否重复造轮子（E5 阶段）|
| §3 抽象分层 | 调用层级合规核查 |
| §4 DDD 落点 | 包路径合规核查 |
| §5 命名约定 | 7 类命名合规核查 |
| §7 契约入口 | Feign / SPI 调用核查 |

**精准查询（需要时）：**
- 验证某字段存在：`assets.table(projectKey, tableName)`
- 查组件是否已有复用：`assets.component(projectKey, componentName)`
- 查跨服务 API：`assets.api(projectKey, method)`

### 1.6 读取实际代码（🔴 必须读代码本身，不读报告代劳）

```bash
# 按 §1.2 提取的 §3 Task 列表逐个读：
cd {module}
git diff {base_commit}..{head_commit} --name-only  # 变更文件清单
for f in $(git diff --name-only); do
  echo "=== $f ==="
  cat $f
done
```

> **🔴 关键：** AI 必须**自己读代码**，不依赖 Coding 报告的描述。报告中说"实现了 X"必须 grep 验证。

### 1.7 产出：输入清单

```
【第一步产出物：输入清单】

Story ID：{STORY-ID}
项目：{projectKey}

已读取：
- [x] Story 主文档（{N} 章 / {M} AC）
- [x] 统一版 CodePlan（{N} 节 / Tier {T}）
- [x] Coding 报告（变更文件 {N} 个 / 编译 {状态} / 测试 {状态}）
- [x] 测试报告（用例 {N} 个 / 通过率 {M}%）
- [x] 项目资产 {projectKey}.assets.md（5 节核心）
- [x] 实际代码（{N} 个文件）

【请确认：以上输入是否完整？】
```

**门禁：用户确认输入完整后，方可进入第二步。**

---

## 第二步：多维评审（六阶段并行挖掘）

> **🔴 关键：** 六个阶段**并行执行**（不是顺序），每个阶段独立产出评审意见，最后汇总。

### 阶段 A：业务逻辑评审

**目的：** 验证业务实现与 Story AC 100% 一致。

**输入：** Story §AC / §主流程 / §异常流程 + 实际代码

**输出：** AC × 实现的对照表

**判定维度：**

| 维度 | 必查项 |
|------|--------|
| 主流程步骤 | 每个主流程步骤是否都有对应代码实现（步骤 × 类:方法） |
| 异常流程 | 每个异常流程是否都正确抛出对应异常（异常 × 类:方法 × 错误码） |
| 业务规则 | Story §业务规则（如果有）是否都体现（grep 关键字） |
| 边界场景 | Story §边界场景是否覆盖（空值/极值/并发/超时） |
| AC 实现 | 每个 AC 都有对应测试方法（AC_ID × TestClass.testXxx） |

**必审项：**
- 每个 AC 都有 `TestClass.testXxx` 方法 + 通过（带证据）
- 异常错误码与 Story §错误码表 100% 一致
- 业务逻辑落点正确（业务规则 → Domain / 协调 → Application / 存取 → Repository）

**反模式：**
- ❌ "AC-001 已实现" — 无 TestClass.testXxx 证据
- ❌ "异常处理完整" — 无具体错误码
- ❌ "覆盖了边界场景" — 无具体场景列表

### 阶段 B：分层职责红线核查

**目的：** 验证 Domain/Application/Repository/Interfaces 各层职责不串味。

**输入：** 项目资产 §4 DDD 内部分层落点 + 实际代码

**输出：** 4 大红线核查表

**🔴 4 大红线（任一 🔴 = 整 Review 不通过）：**

| # | 红线 | 判定 | 证据要求 |
|---|------|------|---------|
| B1 | Repository 实现类的每个方法都是存取语义 | 仓储方法名仅 `findByXxx / save / update / updateStatus` | grep RepositoryImpl + 列出每个方法 |
| B2 | 领域逻辑（状态流转/不变量/业务计算）写在 Domain | 实体充血方法/DomainService | 类:方法 + 引用项目资产 §4 |
| B3 | Application 只做编排 | `@Transactional` 边界 / 调 Domain 顺序 | 类:方法 + 文件:行号 |
| B4 | Domain 无 SQL/PO/DTO | grep `import javax.persistence\|@TableName\|@Data` | 文件:行号 |

**判定口诀：**
- 业务规则（状态机/不变量/聚合一致性）→ **Domain**
- 协调谁调谁（事务/顺序/跨域）→ **Application**
- 存取数据（findByXxx/save/update）→ **Repository / Infrastructure**
- 接 HTTP / 协议适配 → **Interfaces**
- 跨服务契约 → **SPI**
- BFF 场景 → **BFF 入口**

**🔴 边缘案例判定（补充 §6 阶段 3 步骤 3 已有）：**
- 状态机（业务规则核心）→ **Domain**（`domain/.../service/{Resource}DomainService` 或 `entity/{Resource}DO.transition()`）
- 跨聚合事务（协调多聚合）→ **Application**（`appservice/{Resource}AppService` 的 `@Transactional` 方法）
- 缓存读（带业务策略如"先查缓存再查 DB"）→ **Application**（业务编排的一部分）
- 全局唯一性校验（需查 DB）→ **Domain**（写在 DomainService，因这是聚合不变量）

### 阶段 C：数据库逻辑链评审

**目的：** 验证数据库设计、SQL 性能、事务边界符合项目资产 §6.5。

**输入：** 项目资产 §6.5 数据库规范 + Story §数据模型 + Mapper XML + 实际 SQL

**输出：** DB 评审表

**必审项：**

| 维度 | 必查项 | 反模式 |
|------|--------|--------|
| DDL 规范 | 审计四字段 (id/created_by/created_date/last_updated_by/last_updated_date) | 缺任一字段 |
| 命名规范 | snake_case / 禁用保留字 / 单数 | `is_xxx` 字段类型必须是 `tinyint(1)` |
| 索引 | 单表 ≤ 5 / varchar 索引指定长度（一般 20） | 组合索引区分度高的不放在最左 |
| 主键 | `bigint AUTO_INCREMENT` 或 `varchar(32)` 业务单号 | 混用其他类型 |
| 逻辑删除 | `deleted_flag TINYINT(1) DEFAULT 0` | 物理删除需在 DR 说明 |
| 事务 | `@Transactional` 边界 = 包含哪些操作 | 事务内远程调用/MQ 发送 |
| SQL WHERE | UPDATE/DELETE 必含 WHERE 条件 | `WHERE id = #{id} AND status = #{oldStatus}` 乐观锁 |
| EXPLAIN | 复杂查询 EXPLAIN 验证 | 缺 EXPLAIN 输出 |

**反模式（必查 5 类）：**
- ❌ 禁外键级联
- ❌ 禁 `SELECT *`（分页先判 count）
- ❌ 禁超过 3 表 JOIN
- ❌ IN 集合 > 1000
- ❌ 禁 `${}` 拼接 SQL（必须 `#{}` 参数化）

### 阶段 D：测试真实性 + 真实 DB/HTTP 覆盖核查

**目的：** 扫描 8 类禁止伪造手段 + 真实 DB/HTTP 覆盖核查。

**输入：** 测试代码 + 测试报告 + 项目资产 §6.7 测试规范

**输出：** 测试真实性核查表

**🔴 8 类禁止手段扫描（任一命中 = 测试无效）：**

| # | 禁止手段 | 扫描方法 | 命中示例 |
|---|---------|---------|---------|
| D1 | `@Disabled` / `@Ignore` 跳过失败 | `grep -rn "@Disabled\|@Ignore" src/test` | `class FooTest { @Disabled void testX() {...} }` |
| D2 | `assertTrue(true)` 永真 | grep | `assertTrue(true);` |
| D3 | `catch (Exception e) {}` 吞噬 | grep | `try { ... } catch (Exception e) { /* ignore */ }` |
| D4 | 全 Mock 替代（mock 掉所有 Repository/Service） | 人工 spot check | 5 个 @MockBean + 0 个真实 DB |
| D5 | 期望值=实际值（先跑代码得 actual 再写 expected） | 人工 spot check | `assertEquals(actual, actual);` |
| D6 | 无效测试数据（userId=1L 等假设值） | grep | 期望值写死为 null/空集合 |
| D7 | `Thread.sleep` 绕过 | grep | `Thread.sleep(1000);` |
| D8 | 凑覆盖率（assertTrue(true) / catch 吞噬） | 同 D2 / D3 | — |

**🔴 真实 DB/HTTP 覆盖核查（5 类强制真实）：**

| # | 场景 | 必真实 | 判定 |
|---|------|--------|------|
| T1 | 核心落库（涉及资金/状态/权限的 INSERT/UPDATE/DELETE） | DB | grep `@SpringBootTest + @Transactional` + 看 @MockBean 数量 |
| T2 | 事务回滚 | DB | 同 T1 + `@Rollback` |
| T3 | 分布式锁（如 Redisson） | Redis | grep `RedissonClient` + 真实 Redis 连接 |
| T4 | Feign 调用链路 | HTTP | `@SpringBootTest(RANDOM_PORT) + TestRestTemplate` |
| T5 | Redis 缓存失效 | Redis | 同 T3 |

**🔴 MockMvc 降级（仅限框架过老时）：**
- 必须在测试方法 Javadoc 注明"框架过老，降级使用 MockMvc，原因 X"
- 否则视为违反"能走真实 HTTP 的接口测试必须走真实 HTTP"原则

**覆盖率核查：**
- 整体 ≥ 60%
- Service 核心 ≥ 70%
- Mapper XML 自定义 SQL ≥ 60%
- Controller 接口 ≥ 50%

### 阶段 E：项目资产合规性核查

**目的：** 验证代码与项目资产 §3/§4/§5/§6 完全合规。

**输入：** `project-assets-update-skill.assets.forCodeReview(projectKey)` 返回内容 + 实际代码

**输出：** 5 项合规性核查表

**核查项：**

| # | 维度 | 必查项 | 证据要求 |
|---|------|--------|---------|
| E1 | 分层映射（§3） | 涉及哪些层 / 与 §3 表格的"工程模块"列对照 | 文件路径 ↔ §3 表格行 |
| E2 | 包路径（§4） | 类所在包 ↔ §4 包路径模板 | 类:方法 ↔ §4 行 |
| E3 | 命名约定（§5） | 7 类对象命名与 §5 模板匹配 | grep 类名 ↔ §5 命名模板 |
| E4 | 工程约束（§6） | 9 类约束逐项对照 | 类:方法 ↔ §6 约束条目 |
| E5 | 调用层级（§3） | 文件清单按 §3 层级排序 | 文件 ↔ §3 层级 |

**反模式：**
- ❌ 类名违反 §5 命名约定（如 `UserManager` 而非 `UserAppService`）
- ❌ 包路径与 §4 不符（如写到 `com.casstime.cloud.boss.user.application` 而非 §4 规定的精确路径）
- ❌ 用了 §6.8 禁止的库（如 LocalDateTime 而非 Date）

### 阶段 F：跨文档引用核查（与 §3 调用层级一致性）

**目的：** 验证 CodePlan / 模板 / 报告 / 实际代码四者之间**调用层级引用一致**。

**输入：** 统一版 CodePlan §3 + 模板 + Coding 报告 + 项目资产 §3 + 实际代码

**输出：** 5 方调用层级对照表

**🔴 必查 5 方一致性：**

| 抽象层 | CodePlan §2 描述 | 模板章节 | Coding 报告文件清单 | 项目资产 §3 | 实际代码 |
|--------|-----------------|---------|------------------|------------|---------|
| 用户入口（BFF）| {N} 类 | §五 X.X | {N} 文件 | 抽象层 1 | 实际目录 |
| 跨模块 SPI | {N} 类 | §五 X.X | {N} 文件 | 抽象层 2 | 实际目录 |
| Interfaces | {N} 类 | §五 X.X | {N} 文件 | 抽象层 3 | 实际目录 |
| Application | {N} 类 | §五 X.X | {N} 文件 | 抽象层 4 | 实际目录 |
| Domain | {N} 类 | §五 X.X | {N} 文件 | 抽象层 5 | 实际目录 |
| Infrastructure | {N} 类 | §五 X.X | {N} 文件 | 抽象层 6 | 实际目录 |
| 测试 | {N} 类 | §五 X.X | {N} 文件 | 抽象层 7 | 实际目录 |
| 文档 | {N} 文档 | §五 X.X | {N} 文件 | 抽象层 8 | 实际目录 |

**任一行"5 方不一致" = 🔴 一致性闸未过。**

### 第二步产出：Code Review 结论初稿

```markdown
## Code Review 结论初稿 - {STORY-ID} - {YYYY-MM-DD HH:mm}

| 阶段 | 评审员 | 发现问题数（🔴/🟠/🟡/🟢）| 状态 |
|------|--------|--------------------------------|------|
| A 业务逻辑 | 主 reviewer | {N} / {N} / {N} / {N} | ✅/⚠️/❌ |
| B 分层职责 | 架构 reviewer | {N} / {N} / {N} / {N} | ✅/⚠️/❌ |
| C 数据库逻辑 | 架构 reviewer | {N} / {N} / {N} / {N} | ✅/⚠️/❌ |
| D 测试真实性 | 测试 reviewer | {N} / {N} / {N} / {N} | ✅/⚠️/❌ |
| E 项目资产合规 | 规范 reviewer | {N} / {N} / {N} / {N} | ✅/⚠️/❌ |
| F 跨文档引用 | 集成 reviewer | {N} / {N} / {N} / {N} | ✅/⚠️/❌ |
| **合计** | — | **{N} / {N} / {N} / {N}** | **{✅/⚠️/❌}** |

**通过条件：** 🔴 = 0 + 6 阶段都 ✅
```

---

## 第三步：合理性判定

### 3.1 判定流程

```
对每个发现的缺陷：
    ├─ 是否 🔴 阻断型（影响核心业务/数据安全/系统稳定性）？
    │   ├─ 是 → 🔴 阻断型（24h 修复，禁止提交）
    │   └─ 否 → 继续判断
    ├─ 是否 🟠 严重型（影响性能/并发/可维护性）？
    │   ├─ 是 → 🟠 严重型（24h 修复）
    │   └─ 否 → 继续判断
    ├─ 是否 🟡 一般型（影响代码规范/复用性）？
    │   ├─ 是 → 🟡 一般型（48h 修复）
    │   └─ 否 → 🟢 建议型（下次迭代）
    └─ 是否 🟢 建议型（仅是体验/优雅性）？
        └─ 是 → 🟢 建议型
```

### 3.2 判定结果分类（🔴 必须带归属标签）

| 缺陷归属 | 修复路径 |
|---------|---------|
| 🔴 CR 阻断型 | 走 Code Review Update Plan（§第四步 bis） |
| 业务实现缺陷 | 改代码 + 触发 Coding SKILL |
| Story 设计缺陷 | 触发 Story Update SKILL |
| Task 设计缺陷 | 触发 Task Generate SKILL |
| 项目资产漂移 | 触发 Project Assets Update SKILL |
| 测试真实性问题 | 修测试（不得"修复测试代替修复代码"） |
| 命名/分层问题 | 改代码 + 更新项目资产（如果项目资产过时） |

---

## 第四步：记入补充说明文档

将 Code Review 结果追加到 `{story-prefix}-Supplement.md`：

```markdown
## {N}、Code Review 第 {轮次} 轮 - {YYYY-MM-DD HH:mm}

### Reviewer
- 主 reviewer: {reviewer 名称 / Agent ID}
- 架构 reviewer: {名称}
- 测试 reviewer: {名称}
- 规范 reviewer: {名称}

### 问题汇总
| 缺陷 ID | 阶段 | 等级 | 缺陷描述 | 修复建议 | 归属 |
|---------|------|------|---------|---------|------|
| CR-001 | A | 🔴 | ... | ... | 改代码 |
| CR-002 | B | 🟠 | ... | ... | 改项目资产 |
| CR-003 | D | 🔴 | ... | ... | 修测试 |
| ... |

### 闸门状态
| 闸门 | 状态 | 备注 |
|------|------|------|
| 7.1 ⑥bis 一致性 | ✅/❌ | |
| 7.2 ⑦bis 对称性 | ✅/❌ | |
| 7.3 全文档回扫 | ✅/❌ | |
| 7.4 禁裸 ✅ | ✅/❌ | |
| 7.5 报告-代码对账 | ✅/❌ | |
| 7.6 产出物对账 | ✅/❌ | |
| 7.7 真实 DB/HTTP 覆盖 | ✅/❌ | |
```

---

## 第四步 bis：🔴 触发的 Proposal（改代码前硬门禁）

> **🔴 【2026-06-06 重大重构】** 章节标题变更 + 内容替换为 Proposal 指针。
> **原 "CodeReviewUpdatePlan" 已废弃**，统一用 **`proposal-skill.md`** 替代。
>
> **当前规则：**
> - Code Review 评审发现 🔴 缺陷 → **触发 `proposal-skill.md §第二步`** 生成 Proposal
> - Proposal 渠道标识 = 1（Code Review）
> - Proposal 文档路径：`documentStorage.resolve_path(intent="PROPOSAL", storyId={STORY-ID}, version={N}, title={标题})`（按 `document-storage-skill.md §2.6 路径模板` 🆕 2026-06-17 修复 P1-3）
> - 走 5 步流程（proposal-skill.md §第五步）：改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test
> - 不直接生成 CodeReviewUpdatePlan
>
> **详见 [`proposal-skill.md` §多渠道接入设计 - 渠道 1](../cross-cutting/proposal-skill.md)。**

> **🔴 强制：** 任何修复动作（含修代码、改 Story、改 Task、改项目资产）**必须**先有 Plan，按 Plan 执行。
>
> **与 story-review 的差异：** Story Review 的 UpdatePlan 触发 Story Update（链短）；Code Review 的 UpdatePlan 可能触发 4 个下游 SKILL（链长）。

### CodeReviewUpdatePlan 模板

```markdown
## CodeReviewUpdatePlan - {STORY-ID} - {YYYY-MM-DD HH:mm}

### 1. 问题清单（按 🔴/🟠/🟡/🟢 分级）
- 🔴 CR-001: {缺陷描述}（阶段 A）→ 改代码
- 🔴 CR-002: {缺陷描述}（阶段 B）→ 改项目资产
- 🟠 CR-003: {缺陷描述}（阶段 C）→ 改代码
- ...

### 2. 存疑项（评审时不确定的事项）
- {X} 不确定是否合理 → 询问用户
- {Y} 不确定是否符合项目资产 §6 → 询问用户

### 3. 更新计划（按修复路径分组）
- **改代码组**（CR-001, CR-003）：涉及 N 个类 / M 个方法
  - 类 1：{修改内容}
  - 类 2：{修改内容}
- **改项目资产组**（CR-002）：涉及 §X.Y 章节
  - {章节}：{修改内容}
- **改 Story 组**（CR-005）：涉及 Story §X.Y 章节
- **改 Task 组**（CR-006）：涉及 Task-X §X.Y 章节

### 4. 字段链路影响分析（如涉及 §6 全链路映射）
- 字段 X 改动：触入 HTTP ✓ / BFF ✓ / SPI ✓ / DO ✓ / DB ✓（5 层都要同步）
- 字段 Y 改动：仅 DB 层（无需同步）

### 5. 触发路径
- 改代码 → 触发 Coding SKILL（按 Coding 实时追溯链：Task → Story → DR）
- 改项目资产 → 触发 Project Assets Update SKILL
- 改 Story → 触发 Story Update SKILL
- 改 Task → 触发 Task Generate SKILL

### 6. 执行后验收（修改后如何验证）
- 闸门 7.1-7.7 重新过一遍
- 重新跑 §2 六阶段评审
- 连续 1 轮无新增问题才退出循环（§第六步）

### 7. 用户确认
- ☐ 通过，按 Plan 执行
- ☐ 需要调整（说明）
```

### 触发各下游 SKILL

```
Code Review 发现问题
    ↓
生成 CodeReviewUpdatePlan（§第四步 bis）
    ↓
按 Plan 触发下游 SKILL
    ├─ 改代码 → 触发 Coding SKILL（按 Coding 实时追溯链）
    │   追溯层 1：先读 Task（修 Task 文档 + CodePlan）
    │   追溯层 2：再读 Story
    │   追溯层 3：再读 DR
    │   追溯层 4：AI 犯蠢直接改
    ├─ 改项目资产 → 触发 Project Assets Update SKILL
    │   （按其 §4 动作 2 更新 5 步 SOP）
    ├─ 改 Story → 触发 Story Update SKILL
    │   （按其 Plan-first 原则）
    └─ 改 Task → 触发 Task Generate SKILL
        （按其 §5bis 全局 Task Review 5 步 SOP）
    ↓
所有下游修改完成 → 回到本 SKILL §第六步循环判定
```

---

## 第五步：触发下游 SKILL

> **🔴 必读：** Plan 执行前必须先有 Plan（§第四步 bis）。Plan 未确认前禁止触发任何下游 SKILL。

| 缺陷类型 | 触发 SKILL | 引用章节 |
|---------|-----------|---------|
| 改代码 | `coding-skill.md` | §异常路径：实时追溯链（Task→Story→DR→AI 犯蠢） |
| 改项目资产 | `project-assets-update-skill.md` | §4 动作 2 更新 5 步 SOP |
| 改 Story | `story-update-skill.md` | Plan-first 执行 |
| 改 Task | `task-generate-skill.md` | §5bis 全局 Task Review 闭环 |
| 修测试 | `coding-skill.md` | §"测试真实性强制规范"（已下沉到本 SKILL §7） |

---

## 第六步：循环判定

```
所有下游修改完成
    ↓
重新跑本 SKILL §第二步 六阶段评审
    ↓
跑本 SKILL §第七步 7 道闸
    ↓
判定：
    ├─ 无新增问题 → 退出循环（✅ 通过）
    └─ 有新增问题 → 回到 §第四步 bis 生成新 Plan → 循环
```

**退出条件：** 连续 1 轮无新增问题。

**循环上限：** 3 轮。3 轮仍有 🔴 阻断型 → 升级用户决策。

---

## 第七步：CodeReview 闸门全集（7 道闸，从 coding-skill 迁出）

> **🔴 来源（2026-06-05 重大重构）：** 原 `coding-skill.md` §实战闸沉淀 + §📋 ⑥bis/⑦bis + §📋 Coding 问题分层排查 + §📋 测试真实性强制规范 全部迁出到本 SKILL §7。
> coding-skill.md 改为指针 → "闸门定义见 code-review-skill.md §7"。

### 闸 1：⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 硬前置）

> **🔴 关键：** 必须在 §2 六阶段评审前过，否则"评审锚点"是漂移的代码，无法保证评审正确性。

**目的：** 以代码为锚反向核查 DR / Story / Task / 测试用例 / 代码 五方一致。

**4 步核查：**

```
1. 选定锚点：当前磁盘代码（不是 Coding 报告描述）
2. 反向核查：每个代码文件 / 类 / 方法对应哪个 DR 条款？Story 章节？Task？测试用例？
3. 判定：每行 = 一致 / 漂移（🔴 漂移 = 阻断型问题）
4. 证据：每行必须附 grep / 文件:行号 / 测试方法名
```

**核查表（嵌入 CodeReview 报告"零、"章节）：**

| DR 章节 | Story 章节 | Task 章节 | 测试用例 ID | 代码文件:行号 | 一致/漂移 | 证据 |
|---------|-----------|-----------|------------|--------------|----------|------|

**🔴 漂移项 = 阻断型 CodeReview 问题，必须按层级回改。**

**判定层级：**
- 代码错就改码
- 设计落后就回写 Story/Task/用例
- DR 缺陷走异常路径（触发 DR Update SKILL）

### 闸 2：⑦bis 全链路对称性核查闸（🔴 流程收尾强制）

> **🔴 关键：** 人工审核点 4 前最后一道闸，**阻断型闸**。

**目的：** 5 层（DR → Story → Task → 实现 → 测试用例）一一对应、双向追溯无漏无多。

**5 步核查：**

```
1. 从 DR 业务规则出发 → 逐条追踪到 Story 章节
2. Story 章节 → Task 章节
3. Task 章节 → 代码实现（文件:行号）
4. 代码实现 → 测试用例 ID
5. 反向：从测试用例 → 实现 → Task → Story → DR
```

**追溯矩阵（追加到 CodeReview 报告或独立 `{STORY-ID}-追溯矩阵.md`）：**

| 追溯 ID | DR 条款 | Story 章节 | Task | 代码实现(文件:行号) | 测试用例 ID | 对称性结论 |
|---------|--------|-----------|------|-------------------|------------|----------|

**🔴 核查范围：全量覆盖 DR 中本 Story 涉及的全部业务规则**，不只本轮改动。

**🔴 禁止裸 ✅**：每行"对称性结论"必须能点到具体的 Story 章节号 / Task 编号 / 文件:行号 / 测试用例 ID，点不到的不得标"贯通"。

### 闸 3：全文档回扫闸（🔴 CodeReview 必跑）

> **🔴 来源（d--Item-document 项目踩坑）：** STORY-002 历轮 CodeReview 都只核了当轮新增范围就标"三方一致 ✅"，实为虚报。导致 A 类"文档落后于代码"债持续累积。

**🔴 强制：** 出 CodeReview 标"DR-Story-Task 一致 ✅"前，**必须对 Story 主文档全章节回扫**，不能只核对当轮改动范围（diff）。

**4 步执行：**

```
1. 用关键字回扫 Story 主文档全文
   - 旧端点 / 旧错误码 / 已删除的类名 / 已改类型的字段名 / 已废弃的字段
2. 改主流程/契约后，必须联动检查所有引用它的章节
   - 异常流程表 / AC 验收表 / 错误码表 / 索引说明 / 偏离声明 / 未决问题
3. 核心落库路径不能只靠 mock 单测
   - 测试 schema/DDL 必须与设计 DDL 的 NOT NULL 等约束对齐
   - 补真实 DB 约束回归测试（mock 会掩盖约束类缺陷）
4. "三方一致 ✅"是可证伪的结论
   - 写之前要能列出回扫了哪些关键字、哪些章节
   - 否则不准写
```

### 闸 4：禁裸 ✅ 闸（🔴 任何检查项的 ✅ 必须附客观证据）

> **🔴 来源（d--Item-document 项目踩坑）：** 一个只需要"宣称"就能通过的检查项，在压力下必然被宣称通过。

**🔴 强制规则：** 关键检查项（一致性 / 契约对齐 / 落库正确 / 回扫完成等）打 ✅ 时，**必须附客观证据**。

**附证据清单：**

| 检查项 | 必附证据类型 |
|--------|------------|
| 外部契约一致 | 真实报文 / 官方文档 ↔ 字段逐字段对照表 |
| 真实落库正确 | 真实 DB 集成测试的执行输出（不是 mock 全绿） |
| DR-Story-Task-代码一致 | 每个章节的对照结论 + 代码文件:行号（不是 diff 范围） |
| 联动修改完成 | "引用该处的章节清单" + 每章同步状态 |

**反模式：**
- ❌ "DR-Story-Task 一致 ✅" — 无具体证据
- ❌ "测试通过 ✅" — 无测试方法名
- ❌ "无问题 ✅" — 无证据，仅自我声明

### 闸 5：报告-代码对账闸（🔴 任何报告必须验证报告项在代码中真实存在）

> **🔴 来源（d--Item-document 项目踩坑）：** 报告与代码可能漂移：报告说"实现了 X"、实际代码没写 / 写错位置 / 写错类。

**🔴 强制规则：** Coding/测试/CodeReview 报告完成后必须新增产出物对账环节，验证报告声明项在代码中真实存在。

**附证据命令：**

| 报告项 | 验证命令 |
|--------|---------|
| 报告中的每个能力声明 | `grep -r "方法名" --include="*.java"` / IDE "Find Usages" |
| 报告中的每个文件路径 | `ls {路径}` 验证存在 |
| 报告中的每个测试方法 | `grep "@Test" {测试类}` 验证存在 |
| 报告中的每条业务规则 | `grep "关键字" --include="*.java"` 验证实现 |

**🔴 报告与代码不一致 → 必须修正报告或补充代码，二选一，不得跳过。**

### 闸 6：产出物对账闸（🔴 报告完成后必须验证）

**🔴 强制：** AI 必须验证以下每个产出物真实存在，并与报告描述一致。

| 产出物 | 实际路径（由 `documentStorage.resolve_path()` 自动定位） | 是否存在 | 与报告一致 |
|--------|---------|---------|----------|
| Story 文档 | `documentStorage.resolve_path(intent="STORY", storyId)` | □ | □ |
| 统一版 CodePlan | `documentStorage.resolve_path(intent="CODING_PLAN", storyId)` | □ | □ |
| Coding 报告 | `documentStorage.resolve_path(intent="CODING_REPORT", storyId, version={v:N,r:M})` | □ | □ |
| 测试用例 | `documentStorage.resolve_path(intent="TESTCASE", storyId)` | □ | □ |
| 测试报告 | `documentStorage.resolve_path(intent="TEST_REPORT", storyId, version={v:N,r:M})` | □ | □ |
| 源代码 | 工作目录对应工程 | □ | □ |
| CodeReview 报告 | `documentStorage.resolve_path(intent="CODE_REVIEW", storyId, version={v:N,r:M})` | □ | □ |

> **🔴 任何产出物不存在或不一致 → 必须修正报告或补充产出物，不得跳过。**

### 闸 7：真实 DB/HTTP 覆盖核查闸（🔴 5 类场景必须真实）

> **🔴 来源（d--Item-document 项目踩坑）：** mock 测试会掩盖真实约束类缺陷。STORY-002 落库漏 NOT NULL 字段是被 mock 测试掩盖的。

**🔴 强制：** 核心落库路径必须有真实 DB 集成测试覆盖 NOT NULL / 唯一约束等。

**5 类必真实场景清单：**

| # | 场景 | 必真实 | 验证方法 |
|---|------|--------|---------|
| 1 | 核心落库（涉及资金/状态/权限的 INSERT/UPDATE/DELETE） | DB | grep `@SpringBootTest + @Transactional` |
| 2 | 事务回滚（验证 @Transactional 边界） | DB | grep `@Rollback` + 同 1 |
| 3 | 分布式锁（如 Redisson） | Redis | grep `RedissonClient` + 真实 Redis 连接 |
| 4 | Feign 调用链路 | HTTP | `@SpringBootTest(RANDOM_PORT) + TestRestTemplate` |
| 5 | Redis 缓存失效 | Redis | 同 3 |

**🔴 MockMvc 降级（仅限框架过老时）：**
- 必须在测试方法 Javadoc 注明"框架过老，降级使用 MockMvc，原因 X"
- 否则视为违反"能走真实 HTTP 的接口测试必须走真实 HTTP"原则

### 闸门汇总

| 闸 | 名称 | 等级 | 触发时机 |
|----|------|------|---------|
| 1 | ⑥bis 一致性闸 | 🔴 阻断 | §2 六阶段评审前 |
| 2 | ⑦bis 对称性闸 | 🔴 阻断 | 人工审核点 4 前 |
| 3 | 全文档回扫闸 | 🔴 阻断 | §2 阶段 F 跨文档引用核查前 |
| 4 | 禁裸 ✅ 闸 | 🔴 阻断 | 每个 ✅ 判定时 |
| 5 | 报告-代码对账闸 | 🔴 阻断 | §2 阶段 D / 报告完成时 |
| 6 | 产出物对账闸 | 🔴 阻断 | 报告完成时 |
| 7 | 真实 DB/HTTP 覆盖核查闸 | 🔴 阻断 | §2 阶段 D 时 |

**任一闸门 🔴 未过 = 整 Code Review 不通过。**

---

## 第七步 bis：CodeReview 报告合规性校验

> **🔴 强制：** CodeReview 报告生成后，跑 7 道闸的最终校验。

**校验项：**

| # | 校验项 | 必填 | 判定 |
|---|--------|------|------|
| 1 | 报告含 7 道闸的状态 | ✅ | ✅/❌ |
| 2 | 报告含 §2 六阶段评审结果 | ✅ | ✅/❌ |
| 3 | 报告含 §3 合理性判定 | ✅ | ✅/❌ |
| 4 | 报告含 §第四步 bis CodeReviewUpdatePlan | ✅ | ✅/❌ |
| 5 | 报告含 §第六步 循环判定结果 | ✅ | ✅/❌ |
| 6 | 报告含 §第七步 7 道闸结果 | ✅ | ✅/❌ |
| 7 | 报告含问题清单 + 等级 + 修复建议 | ✅ | ✅/❌ |
| 8 | 报告含证据（每个 ✅ 都附） | ✅ | ✅/❌ |
| 9 | 报告无自我声明 ✅ | ✅ | ✅/❌ |
| 10 | 报告含产出物对账（§闸 6） | ✅ | ✅/❌ |

**门禁：** 10 项校验全 ✅ → 进入人工审核点 4。任一 ❌ → 报告不通过，回到 §第六步 循环。

---

## 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止自我声明 ✅ 通过 | 虚报 → 漏问题 | 每个 ✅ 附证据 |
| 2 | 禁止只核当轮 diff 就标"一致" | 文档债积累 | 全文档回扫闸（§闸 3） |
| 3 | 禁止"修复测试"代替"修复代码" | 测试造假 | 走 Coding 实时追溯链 |
| 4 | 禁止评审员没读过代码就给报告 | 评审失真 | §1.6 强制读代码 |
| 5 | 禁止未过 7 道闸就出报告 | 报告失真 | §第七步 闸门全过 |
| 6 | 禁止 6 阶段只做其中 1-2 个 | 评审不完整 | 6 阶段全跑 |
| 7 | 禁止直接修改代码不先有 Plan | 遗漏联动修改 | §第四步 bis CodeReviewUpdatePlan |
| 8 | 禁止把项目资产写死的层当模板 | 跨项目失效 | 引用项目资产 §3 |
| 9 | 禁止评审发现"无具体证据" | 主观评判 | 引用 grep/file:line/test method |
| 10 | 禁止循环 3 轮仍有 🔴 未修 | 拖延 | 升级用户决策 |

---

## 执行清单（逐项执行，禁止跳过）

> AI 启动本 SKILL 时，必须用 TodoWrite 1:1 映射此表。

| # | 动作 | 产出物 | 门禁 |
|---|------|--------|------|
| 1 | 读取 5 个输入文件 + 用户确认 | 输入清单 + 准入检查记录 | 5 文件全读 ✅ |
| 2 | 提取 Story / CodePlan / Coding 报告 / 测试报告 / 项目资产信息 | §1.7 输入清单 | 用户确认 |
| 3 | §2 阶段 A：业务逻辑评审（AC × 实现对照表） | 业务评审表 | AC × 实现 100% 对照 |
| 4 | §2 阶段 B：分层职责红线核查（4 大红线） | 分层核查表 | 4 大红线 0 🔴 |
| 5 | §2 阶段 C：数据库逻辑链评审 | DB 评审表 | DDL/SQL/事务/索引 0 🔴 |
| 6 | §2 阶段 D：测试真实性 + 真实 DB/HTTP 覆盖 | 测试核查表 | 8 类禁止 0 命中 + 5 类真实 ✅ |
| 7 | §2 阶段 E：项目资产合规性核查 | 合规核查表 | §3/§4/§5/§6 0 🔴 |
| 8 | §2 阶段 F：跨文档引用核查 | 5 方对照表 | 5 方一致 0 🔴 |
| 9 | §闸 1：⑥bis 一致性闸 | 一致性核查表 | 0 🔴 漂移 |
| 10 | §闸 2：⑦bis 对称性闸 | 追溯矩阵 | 全量覆盖 DR 业务规则 |
| 11 | §闸 3：全文档回扫闸 | 回扫关键字清单 | 0 残留 |
| 12 | §闸 4：禁裸 ✅ 闸 | 证据清单 | 每个 ✅ 附证据 |
| 13 | §闸 5：报告-代码对账闸 | 对账清单 | 0 不一致 |
| 14 | §闸 6：产出物对账闸 | 产出物清单 | 7 个产出物全 ✅ |
| 15 | §闸 7：真实 DB/HTTP 覆盖核查闸 | 真实场景清单 | 5 类强制真实 ✅ |
| 16 | §第三步 合理性判定 | 缺陷分级表 | 🔴 阻断型必须 0 |
| 17 | §第四步 记入补充说明 | Supplement.md | 写入成功 |
| 18 | §第四步 bis 生成 CodeReviewUpdatePlan | UpdatePlan | 用户确认 |
| 19 | §第五步 触发下游 SKILL | 各 SKILL 触发 | 按 Plan 执行 |
| 20 | §第六步 循环判定 | 循环结论 | 连续 1 轮无新增 → 退出 |
| 21 | §第七步 bis 报告合规性校验 | 校验报告 | 10 项全 ✅ |
| 22 | 进入人工审核点 4 | — | 用户确认 |

---

## 📖 人工审核主动讲解规范 — Code 节点

> **🔴 强制：** 人工审核点 4 之前必须按本节讲解。

**AI 必须主动讲解的内容：**

| 维度 | 必须讲清楚 |
|------|-----------|
| 代码实现故事 | 实际代码是怎么把 Story 落到位的？用 walkthrough 带用户走一遍主流程调用链 |
| 分层 walkthrough | Domain/Application/Infrastructure/Interfaces 各层核心类做什么、关键方法签名是什么、关键代码在第几行 |
| 状态机实现 | canTransition() 实际怎么写？状态流转代码长什么样？条件校验在哪一行？ |
| 事务实现 | `@Transactional` 边界在哪个方法？事务传播行为是什么？回滚规则是什么？ |
| 异常处理 | 核心异常在代码里怎么抛？怎么捕获？错误码怎么映射？HTTP 状态码是什么？ |
| 测试覆盖 | 哪些 AC 已被测试覆盖？测试方法名是什么？覆盖率如何？ |
| CodeReview 发现 | 阻断型/严重型问题有哪些？整改方案是什么？ |

**输出模板：**

```
📖 【Code 讲解 - {STORY-ID} v{N}-r{M}】

【代码实现故事】
本轮 Coding 实现了 {Task-1, Task-2, ...} 共 {N} 个 Task。
让我们按调用链路走一遍：用户{操作} → {Controller} → {AppService} → {DomainService} → {Repository} → DB

【Domain 层 walkthrough】
- {XxxAggregate}：{职责}，核心方法 {method1, method2, ...}
- 关键代码：{文件}:{行号} → {一段代码片段或逻辑描述}
- 状态流转：{canTransition 实际逻辑}（在 {文件}:{行号}）

【Application 层 walkthrough】
- {XxxAppService}：{职责}，编排逻辑：{调用链}

【Infrastructure 层 walkthrough】
- {XxxRepositoryImpl}：{落库方式}，关键 SQL：{SQL 描述}

【事务边界】
- {XxxAppService.transition()}：@Transactional 边界 = {包含哪些操作}
- 事务外：{哪些操作在事务外}（如：通知发送）
- 回滚规则：{触发条件}

【异常处理】
- {XxxException}：{抛出条件} → 错误码 {code} → HTTP {status}
- {XxxDomainException}：{抛出条件} → 错误码 {code}

【测试覆盖】
- AC-001：测试方法 {TestClass.testXxx} ✓
- AC-002：测试方法 {TestClass.testXxx} ✓
- 异常路径：测试方法 {TestClass.testXxx} ✓
- 覆盖率：{数字}%

【CodeReview 关键发现】
- 🔴 阻断：{数量} 个 → 整改方案：{方案}
- 🟠 严重：{数量} 个 → 整改方案：{方案}
- 🟡 一般：{数量} 个 → 建议优化
- 🟢 建议：{数量} 个

【讲解结束 - 请审阅】
1. 代码实现是否正确？
2. 事务边界是否合理？
3. 异常处理是否完整？
4. 测试覆盖是否充分？
5. CodeReview 问题是否接受？
```

**反模式：**
- ❌ 只说"测试通过"不讲解实现细节
- ❌ 分层 walkthrough 跳过具体类:方法
- ❌ 关键代码不给文件:行号

---

## 📋 多 Agent 评审编排

> 📍 **多 Agent 编排执行遵循 [`agent-orchestration-skill.md`](../cross-cutting/agent-orchestration-skill.md)**（任务分配协议、派活卡模板、汇总流程、冲突处理规则详见该 SKILL）。
> 本节只描述 **Code Review 场景专属配置**：何时启用多 Agent、各 reviewer 角色职责、评审维度划分。

> **🔴 来源（角色 7 CodeReviewer）：** ae-sdd-skill.md 角色 7 的 prompt 模板从 AE-skill 迁入本 SKILL。
>
> **🔴 本节深度补全（2026-06-06）：** 增加 3 种多 Agent 模式（A/B/C）、交叉对比算法、3 套 prompt 模板（业务/架构/测试）、不一致项处理决策树。

### 角色 7：CodeReview Agent（`code-reviewer`）

| 项 | 内容 |
|----|------|
| 输入 | Coding 报告 + 测试报告 + Story + 实际代码 + 项目资产 |
| 输出 | CodeReview 报告（含 §2 六阶段评审 + §3 合理性判定 + §第四步 bis UpdatePlan + §第七步 7 道闸） |
| 标准 | 见本 SKILL §第二/三/四/六/七步 |
| 报告格式 | `{STORY-ID}-CodeReview-v{N}-r{M}.md` |
| 适用阶段 | Phase 3 ⑦ |
| 数量建议 | 见下方 3 种模式（按 Story 复杂度选择 1-3 个 reviewer） |

### 3 种多 Agent 模式触发条件（Code Review 场景专属）

| 模式 | 触发条件 | reviewer 数 |
|------|---------|------------|
| A 单 Reviewer | Bug 修复 / 单方法调整 / Tier 1 编码 / ≤5 行改动 / 无状态机/事务 | 1（`reviewer-general`） |
| B 双 Reviewer | 增量功能（≤3 Task）/ 含 AC 验收 / 涉及新接口/新表 / 含状态机/事务/接口契约等关键决策点 | 2（`reviewer-BE` + `reviewer-AR`） |
| C 三 Reviewer | 全新微服务 / 全新表 / 全新 SPI / 涉及资金/状态/权限 / 跨 4 Task+ / Tier 3 编码 | 3（`reviewer-BE` + `reviewer-AR` + `reviewer-QA`） |

### reviewer 角色分工（Code Review 场景专属）

| 角色 | 评审重点 | 覆盖 §2 阶段 |
|------|---------|------------|
| `reviewer-general` | 完整 6 阶段全跑（适用模式 A） | A + B + C + D + E + F |
| `reviewer-BE` | 业务实现 + 分层职责 | A + B + F 重点，其余基础核查 |
| `reviewer-AR` | 架构 + 规范（DDL/SQL 性能、命名约定、包路径、调用层级） | C + E + F 重点，⑤⑥ 道闸侧重 |
| `reviewer-QA` | 测试真实性 + 数据库逻辑链（仅模式 C） | D + C + F 重点，⑦ 道闸侧重 |

> 📍 派活卡模板 / 报告回传协议 / 汇总流程 / 冲突处理规则 → 见 [`agent-orchestration-skill.md §任务分配协议`](../cross-cutting/agent-orchestration-skill.md)，此处不再重复。

### 交叉对比算法（root agent 执行）

**算法 5 步：**

```
1. 收集所有 reviewer 报告（1-3 份）
2. 建立"问题-评级-位置"三维表
   行 = 缺陷 ID（按位置聚合）
   列 = reviewer 名称
   值 = 该 reviewer 对该缺陷的评级（🔴/🟠/🟡/🟢/无）
3. 分类判定：
   ├─ 所有 reviewer 一致（同一评级）→ 接受
   ├─ reviewer 评级不同但有 1 名 🔴 → 升级 🔴 阻断型
   ├─ reviewer 评级不同但都 ≤ 🟠 → 取最高评级（🟠）
   └─ 某 reviewer 发现某缺陷但其他未发现 → 列为"存疑项"，询问用户
4. 一致性核查：每个 🔴/🟠 缺陷在 7 道闸上有对应证据
5. 生成最终 CodeReview 报告（按聚合表生成）
```

**不一致项处理决策树：**

```
情况 1：reviewer-BE 给 🔴 / reviewer-AR 给 ✅
  → 视为 🔴 阻断型（业务实现错是核心）
  → 触发 Coding 实时追溯链

情况 2：reviewer-BE 给 ✅ / reviewer-AR 给 🔴
  → 视为 🔴 阻断型（架构错会影响所有 Story）
  → 触发 Coding 实时追溯链 + 评估是否需更新项目资产

情况 3：reviewer-BE 给 🟠 / reviewer-AR 给 🟡
  → 提升到 🟠 严重型
  → 写入 CodeReviewUpdatePlan

情况 4：reviewer-BE 未发现某缺陷 / reviewer-AR 发现
  → 列入"存疑项"
  → 询问用户：是否纳入最终报告

情况 5：所有 reviewer 评级一致
  → 接受，无不一致
```

**root agent 职责清单：**

| # | 职责 | 产出 |
|---|------|------|
| 1 | 评估 Story 复杂度 → 选模式 A/B/C | 模式决策日志 |
| 2 | 创建 N 个 sub-agent 任务描述（含 prompt 模板） | N 个任务卡 |
| 3 | 等待全部完成（或超时） | — |
| 4 | 收集 N 份 CodeReview 报告 | 报告清单 |
| 5 | 跑交叉对比算法 | 聚合表 |
| 6 | 跑不一致项处理决策树 | 决策日志 |
| 7 | 生成最终 CodeReview 报告（模板套用） | 最终报告 |
| 8 | 触发人工审核点 4（用户确认） | — |

### 各 reviewer 评审重点补充（Code Review 场景专属）

| reviewer | §2 阶段侧重 | 7 道闸侧重 | 输出文件名 |
|----------|------------|----------|----------|
| `reviewer-BE` | A（业务逻辑）+ B（分层职责红线）+ F（跨文档） | ①-④ | `{STORY-ID}-CodeReview-BE-r{M}.md` |
| `reviewer-AR` | C（数据库逻辑链）+ E（项目资产合规性）+ F（跨文档） | ⑤-⑥ | `{STORY-ID}-CodeReview-AR-r{M}.md` |
| `reviewer-QA` | D（测试真实性）+ C（数据库逻辑链） | ⑦ | `{STORY-ID}-CodeReview-QA-r{M}.md` |

各 reviewer 的输入清单（Coding 报告 + 测试报告 + Story + 项目资产 + 实际代码）、deadline 字段、report_back 协议
→ 按 [`agent-orchestration-skill.md §任务分配协议`](../cross-cutting/agent-orchestration-skill.md) 中派活卡模板填写，此处不再重复。

### 与子 SKILL 的协同

| 子 SKILL | 协同点 |
|---------|--------|
| `story-review-skill.md` | 评审 Story 缺陷（Plan-first 原则） |
| `task-generate-skill.md` | 评审 Task 缺陷（含任务级 CodePlan） |
| `coding-skill.md` | 评审 Coding 实现（§实时追溯链） |
| `project-assets-update-skill.md` | 评审项目资产漂移 |
| `testcase-generate-skill.md` | 评审测试用例与 AC 映射 |
| `ae-sdd-skill.md` | AE 编排层"由谁做"（角色 7 指针） |
| `ae-sdd-update-skill.md` | SKILL 边界维护规范 |

---

## 维护

- **维护人：** 架构组 + 各项目 owner
- **更新频率：** 每次 Phase 3 ⑦ 节点 / 每次用户说"出 CR 报告"
- **同步对象：**
  - 与 `story-review-skill.md` 对齐质量（同等地位）
  - 与 `coding-skill.md` 协调（6 大闸门已迁出，coding-skill 改为指针）
  - 与 `templates/coding/be-codereview-template.md` 配套（模板只定通用结构，工程特化从项目资产 §3/§4 读取）
- **关键变化（2026-06-05 重大重构）：**
  - 新建独立 SKILL（之前只是模板 + auto-engineering 角色 7）
  - 6 大闸门从 coding-skill 迁出到本 SKILL §第七步
  - 与 story-review-skill 同等地位
  - 模板按项目资产 §3/§4 重构
