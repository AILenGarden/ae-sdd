---
name: proposal
description: 建议书（Proposal）SKILL — 统一所有"问题描述 + 解决方案"载体。覆盖多渠道（Code Review / Story Review / Coding 异常 / Project Assets 漂移 / 用户反馈 / 生产事故 / Test 发现），用统一的 4 段结构（原本是怎么样/要做什么/怎么做/涉及范围）走流程：改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test。🆕 2026-06-06 新建，解决"问题描述模式可以很多种但最终执行流程不变"的核心洞察。
---

# Proposal — 建议书 SKILL（统一问题描述 + 解决方案载体）

> **🔴 核心洞察（2026-06-06）：**
> - **问题描述模式可以有很多种**（从 Code Review / Story Review / 用户反馈 / 生产事故 / Test 发现 等多种渠道来）
> - **但最终的执行流程是不变的**：**修改 Story → TestCase → Task → Coding → Test**
> - 既然流程固定，就**需要统一的"问题载体"** — 不分渠道，所有问题先写成"建议书"再走流程
> - **建议书（Proposal）** = 单一权威载体，对应"原状/目标/方案/影响"4 段式
>
> **与现有 SKILL 的关系：**
> - `ae-sdd-skill.md` = 流程编排（**Proposal 是新流程的"统一入口"**）
> - **`proposal-skill.md`（本文件）** = 建议书的环节内具体规则（怎么写、怎么走流程）
> - [`templates/proposal/proposal-template.md`](../../templates/proposal/proposal-template.md) = 建议书空白模板
> - 替代（不重复）各 SKILL 内置的 UpdatePlan：
>   - `code-review-skill.md` 的 §第四步 bis **CodeReviewUpdatePlan** → 改用 Proposal
>   - `story-review-skill.md` 的 §第四步 bis **StoryReviewUpdatePlan** → 改用 Proposal
>   - `coding-skill.md` 的 **实时追溯链** → 改用 Proposal
>   - `project-assets-update-skill.md` 的 **Update 动作** → 改用 Proposal

---

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 生成的 Proposal 在写入磁盘前**必须先调用 [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)** 确定：
> 1. **路径**（§2.6 路径模板）：
>    - 单 Story：`documentStorage.resolve_path(intent="PROPOSAL", storyId={STORY-ID}, version={N}, title={标题})`
>    - 跨 Story：`documentStorage.resolve_path(intent="PROPOSAL", storyId="cross-story", version={N}, title={标题})`
> 2. **命名**（§3.1/3.2 命名规则）：**事件类文档按 N 编号，不带版本号**
> 3. **重入判定**（§4 重入 SOP）：Proposal **永不修改**（一次性写完，多处引用）；多源合并 = 新 Proposal

| 输出文档 | 路径模板 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| Proposal（单 Story）| `documentStorage.resolve_path(intent="PROPOSAL", storyId={STORY-ID}, version={N}, title={标题})` | 不带版本号（按 N 编号）| **永不修改** |
| Proposal（跨 Story）| `documentStorage.resolve_path(intent="PROPOSAL", storyId="cross-story", version={N}, title={标题})` | 不带版本号 | **永不修改** |
| Proposal（项目级）| `documentStorage.resolve_path(intent="PROPOSAL", storyId="project", version={N}, title={标题})` | 不带版本号 | **永不修改** |
| Proposal 归档 | `documentStorage.resolve_path(intent="PROPOSAL_ARCHIVE", storyId={STORY-ID or 类别}, version={date})` | — | 归档而非删除 |

> 🔴 **关键：** Proposal 是"事件" + "单一权威源"，**永不修改历史**。多源合并 = 新 Proposal（同 N 编号 + 合并 §1/§2/§3 块）。
>
> 🆕 **2026-06-17 修复 P1-3：** 本 SKILL 内全部 16 处 `design/proposal/` 硬编码已统一替换为 `documentStorage.resolve_path(intent="PROPOSAL", ...)` API 调用形式。集中示例见上表，单 Story / 跨 Story / 项目级 / 归档 4 种场景的调用模板见上。

---

## 0. 目标

- **统一问题描述载体**：不论问题来自哪个渠道，都先用 Proposal 描述清楚"原本是怎么样/要做什么/怎么做/涉及范围"
- **统一执行流程**：所有 Proposal 都走相同的"改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test"流程
- **可追溯**：一次写完 Proposal，多个下游 SKILL（Story Update / TestCase Update / Task Generate / Coding / Test）引用
- **避免"散落 Plan 模板"**：替代各 SKILL 内置的 UpdatePlan，统一入口
- **跨问题聚合**：多个相关问题可合并为一份 Proposal 走一次流程

---

## Proposal 总则（🔴 贯穿全 SKILL，违反 = 结论无效）

### 标尺 1：完整性（🔴 4 段必填）

| 段 | 必填 | 说明 |
|----|------|------|
| §1 原本是怎么样 | ✅ | 现状描述（含文件/行号/版本号/证据）|
| §2 要做什么 | ✅ | 目标/新需求（含验收标准）|
| §3 怎么做 | ✅ | 实施方案/影响分析/字段链路 |
| §4 涉及范围 | ✅ | Story/TestCase/Task/Coding/Test 哪些需改 |

### 标尺 2：单一权威源（🔴 一次写完，多处引用）

- Proposal 是问题的"单一权威源"
- 下游 SKILL（Story Update / TestCase Update / Task Generate / Coding）**引用** Proposal 而非重新生成 UpdatePlan
- 避免"上游 Story Review 出一份 UpdatePlan → 下游 Story Update 又重新分析"的双重成本

### 标尺 3：可执行性（🔴 §3 必须有具体步骤）

- §3 怎么做必须可拆解为"Story 改 §X 章节 / TestCase 加 §X 用例 / Task 改 §X 文件 / Coding 改 §X 文件"
- 不可执行 = 🔴 阻断型

### 标尺 4：影响范围明示（🔴 §4 必须列下游动作）

- §4 涉及范围必填 4 类下游：Story / TestCase / Task / Coding/Test
- 不涉及的标 N/A
- 涉及的需要明确"修改 vs 新增 vs 归档"

---

## 整体流程

```
发现问题（任意渠道）
    ├─ 渠道 1：Code Review 评审发现 → 自动生成 Proposal
    ├─ 渠道 2：Story Review 评审发现 → 自动生成 Proposal
    ├─ 渠道 3：Coding 异常追溯链 → 触发 Proposal
    ├─ 渠道 4：Project Assets 漂移 → 触发 Proposal
    ├─ 渠道 5：用户反馈 → 手动写 Proposal
    ├─ 渠道 6：生产事故 → 手动写 Proposal
    └─ 渠道 7：Test 发现（性能/兼容/边界）→ 手动写 Proposal
    ↓
第一步：识别渠道（4 段必填）
    ↓
第二步：填写 Proposal 4 段
    ├─ §1 原本是怎么样（现状 + 证据）
    ├─ §2 要做什么（目标 + AC）
    ├─ §3 怎么做（方案 + 字段链路 + 步骤拆解）
    └─ §4 涉及范围（下游 5 类动作）
    ↓
第三步：合理性自检（4 维度）
    ↓
第四步：写入 Proposal 文档
    ↓
第五步：用 Proposal 走流程
    ├─ 改 Story → 触发 story-update-skill.md（携带 Proposal）
    ├─ 改 TestCase → 触发 testcase-generate-skill.md（携带 Proposal）
    ├─ 改 Task → 触发 task-generate-skill.md（携带 Proposal）
    ├─ 改 Coding → 触发 coding-skill.md（携带 Proposal）
    └─ 改 Test → 触发测试验证（携带 Proposal）
    ↓
第六步：循环判定（所有下游完成后 → 重新跑原评审）
    ↓
完成 → 归档 Proposal
```

---

## 触发条件

| 触发场景 | 触发方式 | 渠道 |
|---------|---------|------|
| Code Review 评审发现 🔴 问题 | `code-review-skill.md §第四步 bis` 自动生成 | 1 |
| Story Review 评审发现 🔴 问题 | `story-review-skill.md §第四步 bis` 自动生成 | 2 |
| Coding 异常追溯链命中 | `coding-skill.md §异常路径` 触发 | 3 |
| Project Assets 漂移 | `project-assets-update-skill.md` 触发 | 4 |
| 用户说"修一下 XX" / "发现 XX 问题" | 手动写 Proposal | 5 |
| 生产事故 / 监控告警 / 客户反馈 | 手动写 Proposal | 6 |
| Test 发现（性能/兼容/边界/数据问题）| 手动写 Proposal | 7 |
| Story Update / TestCase Update / Task Update 触发上游评审 | 链式触发 | 任意 |

---

## 第一步：识别渠道

| 渠道 | 来源 SKILL | 自动/手动 | 关键输入 |
|------|----------|----------|---------|
| 1 Code Review | `code-review-skill.md` | **自动** | 6 阶段评审问题清单 + 7 道闸结果 |
| 2 Story Review | `story-review-skill.md` | **自动** | 5 阶段挖掘缺陷清单 + 1bis 6 维度 |
| 3 Coding 异常 | `coding-skill.md §异常路径` | **自动** | 实时追溯链的判定（Task/Story/DR/AI 犯蠢）|
| 4 Project Assets 漂移 | `project-assets-update-skill.md` | **自动** | 漂移的章节 + 缺口项 |
| 5 用户反馈 | 手动 | 手动 | 用户原话 + 复现步骤 |
| 6 生产事故 | 手动 | 手动 | 事故描述 + 影响范围 + 监控数据 |
| 7 Test 发现 | `coding-report-skill.md §四` | **自动** | 失败用例 + EXPLAIN 输出 |

**渠道识别产出：**
```
渠道：{1/2/3/4/5/6/7}
来源：{Story ID / Test Report / 监控告警 / 用户原话}
时间：{YYYY-MM-DD HH:mm}
```

---

## 第二步：填写 Proposal 4 段

> **🔴 4 段必填，缺一段 = Proposal 不通过。** 模板见 `templates/proposal/proposal-template.md`。

### §1 原本是怎么样（现状）

**必填：**
- 涉及模块（按项目资产 §3 层级）
- 当前实现/描述（引用文件:行号 / 文档章节号）
- 问题/不足（具体表现 + 证据）
- 触发场景（什么时候/什么条件下出现）

**🔴 证据要求：** 每个描述必附 grep/行号/test method/文档锚点。

**示例：**
> **涉及模块：** Application 层（`BossUserAppService.list`）
> **当前实现：** `BossUserAppService.list(BossUserQuery)`（`appservice/BossUserAppService.java:88`）使用 `query.roleId` 和 `query.status` 做简单过滤
> **问题表现：** 当 `query.roleId` 传 0（默认值）时，返回所有角色的用户，**未做 0 = 全部的特殊语义处理**
> **触发场景：** 前端用户列表查询界面中，角色过滤下拉框默认选"全部"（roleId=0），导致管理员看到非自己管辖角色的用户
> **证据：**
> - `appservice/BossUserAppService.java:88-110` — `if (query.getRoleId() != null) { ... } else { ... }`（无 0 特殊处理）
> - 复现：调用 `GET /boss-user-bff/api/v1/users/list?roleId=0` 返回 100 个用户，预期应返回 100 个用户（一致，但产品期望 roleId=0 = 全部时返回 0 个用户，因为管理员不管理 0=未分配角色）

### §2 要做什么（目标/新需求）

**必填：**
- 新行为（修改后应该怎样）
- 验收标准（Given-When-Then 至少 1 条）
- 不需要做的（边界，明确"不在此 Proposal 范围"）

**示例：**
> **新行为：** `BossUserAppService.list` 必须支持 `query.roleId = 0` = "全部" 的语义
> - roleId > 0 → 仅返回该角色用户
> - roleId = 0 → 返回所有角色用户（即"全部"）
> - roleId < 0 → 400 错误 + 错误码 10001（参数非法）
> - roleId = null → 当前行为不变（保持向后兼容）
>
> **验收标准：**
> - AC-1: Given roleId=0, When 调 list, Then 返回所有角色用户（含全部 100 个）
> - AC-2: Given roleId=-1, When 调 list, Then 返回 400 + 错误码 10001
> - AC-3: Given roleId=null, When 调 list, Then 行为不变（向后兼容）
>
> **不在此 Proposal 范围：**
> - 角色权限管理（单独的 Story）
> - 管理员角色范围（与角色层级相关，单独的 Story）

### §3 怎么做（方案/影响分析/步骤拆解）

**必填：**
- 实施步骤（按"改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test"5 段式拆解）
- 字段链路影响分析（与项目资产 §3/§4 对齐）
- 风险评估（改动对其他 Story/Task 的影响）
- 替代方案（如有，至少 2 个备选 + 推荐理由）

**示例：**
> **实施步骤：**
>
> 1. **改 Story**（`STORY-001-BE.md` §AC 验收标准）
>    - 原 AC-004 改写：增加"roleId=0 = 全部"语义
>    - 新增 AC-005：roleId=-1 错误码 10001
> 2. **改 TestCase**（`STORY-001-BE-testcase.md`）
>    - 原 TC-004 改：roleId=0 期望 100 个用户
>    - 新增 TC-005：roleId=-1 期望 400
> 3. **改 Task**（`task-5-BossUserAppServiceList.md`）
>    - 在"主流程"步骤加"roleId=0 特殊处理"分支
>    - 在"边界场景"加"roleId=-1 错误码"
> 4. **改 Coding**（`appservice/BossUserAppService.java`）
>    - 在 `list` 方法加：`if (query.getRoleId() != null && query.getRoleId() == 0) { /* 不过滤 */ } else if (query.getRoleId() != null && query.getRoleId() > 0) { /* 过滤 */ } else if (query.getRoleId() != null && query.getRoleId() < 0) { throw new BusinessException(10001); }`
> 5. **改 Test**（`BossUserAppServiceIT`）
>    - 新增 `testList_RoleIdZero_ReturnAll`
>    - 新增 `testList_RoleIdNegative_Return400`
>
> **字段链路影响分析：**
> - 触入 HTTP: 无需改（前端传 roleId=0）
> - BFF 接收: 无需改（透传）
> - SPI DTO: 无需改
> - Domain: 无需改（DO 不存 roleId）
> - DB 列: 无需改
> **结论：5 层链路中 4 层无需改，仅 Application 层 + Story/TestCase/Task 需改。**
>
> **风险评估：**
> - 风险 1：roleId=0 改为"全部"可能影响依赖此语义的旧前端 → 中等（需协调前端）
> - 风险 2：roleId=0 改为"全部"可能让管理员看到非自己管辖角色 → 🔴 高（需权限校验）
> - 缓解：风险 2 的权限校验由 `@RequiresPermissions` 注解保证，与 Proposal 范围正交
>
> **替代方案：**
> - 方案 A（推荐）：roleId=0 = 全部（语义对齐前端"全部"选项）
> - 方案 B：roleId=0 = 错误（参数非法）
> - 选择 A 的理由：与产品 PRD 期望一致；前端已按此语义设计

### §4 涉及范围（下游 5 类动作）

**🔴 必填：下游 5 类动作（Story / TestCase / Task / Coding / Test）必须逐项判定"修改/新增/归档/N/A"。**

| 下游 | 动作 | 涉及章节/文件 | 命名前缀 |
|------|------|--------------|---------|
| Story 文档 | 修改 / 新增 / 归档 / **N/A** | `STORY-001-BE.md §AC 验收标准` | 原名（设计类不带版本号）|
| TestCase 文档 | 修改 / 新增 / 归档 / **N/A** | `STORY-001-BE-testcase.md` | 原名 |
| Task 文档 | 修改 / 新增 / 归档 / **N/A** | `task-5-BossUserAppServiceList.md` | 原名 |
| Coding 报告 | 修改 / 新增 / 归档 / **N/A** | `STORY-001-BE-CodingReport-v1-r1.md` | r 递增 |
| Test 报告 | 修改 / 新增 / 归档 / **N/A** | `STORY-001-BE-Report-v1-r1.md` | r 递增 |
| CodeReview 报告 | 修改 / 新增 / 归档 / **N/A** | `STORY-001-BE-CodeReview-v1-r1.md` | r 递增 |

**🔴 门禁：** 5 类动作必须**逐项判定**，不可统一填 N/A（除非真的都不涉及）。

---

## 第三步：合理性自检

| 维度 | 必查项 | 状态 |
|------|--------|------|
| 完整性 | 4 段必填（§1/§2/§3/§4）| ✅/❌ |
| 单一权威源 | 不与其他 UpdatePlan 重复 | ✅/❌ |
| 可执行性 | §3 步骤可拆解为下游 5 类动作 | ✅/❌ |
| 影响范围 | §4 5 类下游逐项判定 | ✅/❌ |

---

## 第四步：写入 Proposal 文档

按 [`templates/proposal/proposal-template.md`](../../templates/proposal/proposal-template.md) 模板汇总 4 段内容，写入文档。

**🔴 强制：** 写入前先打印初稿（用 Read 显示），用户确认后再写入。

**文件路径（按 `document-storage-skill.md §2 路径模板`）：**

| Proposal 类型 | 路径 |
|------------|------|
| 单 Story 范围 | `documentStorage.resolve_path(intent="PROPOSAL", storyId={STORY-ID}, version={N}, title={一句话标题})` |
| 跨 Story 范围 | `documentStorage.resolve_path(intent="PROPOSAL", storyId="cross-story", version={N}, title={一句话标题})` |
| 项目级范围 | `documentStorage.resolve_path(intent="PROPOSAL", storyId="project", version={N}, title={一句话标题})` |

**落地存储（🔴 强制，用户确认初稿后执行）：**

```text
documentStorage.resolve_path(intent="PROPOSAL", storyId, version={N}, title={标题})
→ save_doc(intent="PROPOSAL", storyId, version={N})
```

落地成功（G-DOC-STORAGE ✅）后才能进入第五步走流程。未落地禁止触发任何下游 SKILL。

**命名规则：**
- `{N}` = Proposal 编号（自增，跨 Story 递增；如 STORY-001 第一份 Proposal = `-001-`）
- `{一句话标题}` = 不超过 30 字的简写（如 `roleId=0特殊语义`）
- **不带版本号**（Proposal 是"事件"，但每份独立编号；不修改历史）

**示例：**
- `documentStorage.resolve_path(intent="PROPOSAL", storyId="STORY-001-BE", version=1, title="roleId=0特殊语义")`
- `documentStorage.resolve_path(intent="PROPOSAL", storyId="STORY-001-BE", version=2, title="用户列表分页越界")`

---

## 第五步：用 Proposal 走流程（🔴 核心）

> **🔴 关键：** 不分渠道，所有 Proposal 都走相同的"改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test"5 步流程。

### 5.1 5 步流程总览

```
Proposal 文档已生成
    ↓
1. 改 Story（如涉及）→ 触发 story-update-skill.md（携带 Proposal）
    ↓
2. 改 TestCase（如涉及）→ 触发 testcase-generate-skill.md（携带 Proposal）
    ↓
3. 改 Task（如涉及）→ 触发 task-generate-skill.md（携带 Proposal）
    ↓
4. 改 Coding（如涉及）→ 触发 coding-skill.md（携带 Proposal，遵守实时追溯链）
    ↓
5. 改 Test（如涉及）→ 触发测试验证（携带 Proposal，验证修改后是否解决原问题）
    ↓
全部完成 → 回到原评审流程（Code Review / Story Review / Test 验证）确认问题已解决
    ↓
连续 1 轮无新增问题 → Proposal 归档（✅ done）
```

### 5.2 各下游 SKILL 接收 Proposal 的方式

| 下游 SKILL | 接收 Proposal 的方式 | 必读字段 |
|-----------|------------------|---------|
| `story-update-skill.md` | 读取 Proposal §1 §2 §3 §4 改 Story | §2 新需求 + §3 步骤 + §4 Story 部分 |
| `testcase-generate-skill.md` | 读取 Proposal §1 §2 §3 §4 改 TestCase | §2 新 AC + §3 步骤 + §4 TestCase 部分 |
| `task-generate-skill.md` | 读取 Proposal §1 §2 §3 §4 改 Task | §3 步骤 + §4 Task 部分 |
| `coding-skill.md` | 读取 Proposal §1 §2 §3 §4 改 Coding | §3 Coding 步骤 + §4 Coding 部分 + 实时追溯链 |
| 测试验证 | 跑修改后的测试 + 验证 §2 验收标准 | §2 验收标准 + §1 原状对比 |

### 5.3 Proposal 与下游 UpdatePlan 的关系

**🔴 关键：Proposal 替代各 SKILL 的内置 UpdatePlan**

| 旧（被替代） | 新（统一） |
|-------------|----------|
| `code-review-skill.md §第四步 bis` CodeReviewUpdatePlan | **Proposal 替代**（Code Review 评审发现 → 生成 Proposal）|
| `story-review-skill.md §第四步 bis` StoryReviewUpdatePlan | **Proposal 替代**（Story Review 评审发现 → 生成 Proposal）|
| `coding-skill.md §异常路径` A1-A6 实时追溯链 | **Proposal 替代**（异常触发 → 生成 Proposal）|
| `project-assets-update-skill.md §4 动作 2` Update | **Proposal 替代**（漂移触发 → 生成 Proposal）|

**改写规则：** 各 SKILL 的"问题处理"章节改为：
- 旧：详细描述"如何写 UpdatePlan"
- 新：**1 行指针** → "详细见 `proposal-skill.md §第五步`，本 SKILL 不再重复"

### 5.4 下游 SKILL 的"携带 Proposal"机制

每个下游 SKILL 的输入参数中**新增一栏：**

```yaml
input:
  - {proposal_path}    # ← 新增：Proposal 文档路径
  - {Story ID}
  - ...
```

下游 SKILL **第一步**是"读 Proposal"，将 Proposal 的 §3 步骤拆解为本 SKILL 的执行步骤。

### 5.5 Proposal 多源合并（可选）

> **🔴 允许：** 多个相关问题可合并为一份 Proposal 走一次流程。

**合并条件：**
- 涉及同一个 Story
- 涉及同一个文件的同一行附近
- 由同一个根因导致

**合并方法：**
- 同一份 Proposal 中多个 §1/§2/§3 块（按问题 1 / 问题 2 / ... 编号）
- §4 涉及范围合并

**反例：** 不相关的多问题不要合并（否则流程阻塞）。

---

## 第六步：循环判定

```
所有下游修改完成
    ↓
回到原评审流程（Code Review / Story Review / Test 验证）
    ↓
跑原 SKILL 的"循环判定"（如 code-review-skill.md §第六步）
    ↓
判定：
    ├─ 无新增问题 → Proposal 归档（✅ done）
    └─ 有新增问题 → 回到 §第二步 重写 Proposal（修正方案）→ 循环
```

**退出条件：** 连续 1 轮无新增问题。
**循环上限：** 3 轮。3 轮仍有 🔴 → 升级用户。

---

## 第七步：Proposal 闸门全集

| 闸 | 名称 | 等级 | 判定 |
|----|------|------|------|
| 1 | 完整性闸 | 🔴 阻断 | 4 段必填（§1/§2/§3/§4）|
| 2 | 单一权威源闸 | 🔴 阻断 | 不与其他 UpdatePlan 重复（无双重维护）|
| 3 | 可执行性闸 | 🔴 阻断 | §3 步骤可拆解为下游 5 类动作 |
| 4 | 影响范围闸 | 🔴 阻断 | §4 5 类下游逐项判定（无统一 N/A）|
| 5 | 证据闸 | 🔴 阻断 | §1 每个描述必附 grep/行号/test method 证据 |
| 6 | 替代方案闸 | 🟠 严重 | §3 至少 2 个备选 + 推荐理由（如果适用）|
| 7 | 风险评估闸 | 🟠 严重 | §3 必含风险评估 + 缓解方案 |

**任一 🔴 未过 = Proposal 不通过 → 回到 §第二步 重写。**

---

## 第八步：Proposal 生命周期

```
生成（🆕 initial）
    ↓
使用（5 步流程走完）
    ↓
所有下游修改完成 + 评审通过
    ↓
归档（✅ done → 🗑️ archived）
    ↓
永不删除（保留追溯）
```

**归档路径：**
- 单 Story：`documentStorage.resolve_path(intent="PROPOSAL_ARCHIVE", storyId={STORY-ID}, version={date})`
- 跨 Story：`documentStorage.resolve_path(intent="PROPOSAL_ARCHIVE", storyId="cross-story", version={date})`
- 项目级：`documentStorage.resolve_path(intent="PROPOSAL_ARCHIVE", storyId="project", version={date})`

---

## 多渠道接入设计（🆕 2026-06-06 关键设计）

### 渠道 1：Code Review 评审发现 → 自动生成 Proposal

**触发点：** `code-review-skill.md §第四步 bis`（原 CodeReviewUpdatePlan 章节）

**改写规则：**
- 旧：Code Review 评审员填写 CodeReviewUpdatePlan 模板
- 新：Code Review 评审员**调用 `proposal-skill.md §第二步`** 生成 Proposal

**Code Review 报告改造点：**
```
原：
§十 CodeReviewUpdatePlan（🔴 改代码前硬前置）
  - 问题清单
  - 修复路径（改代码 / 改项目资产 / 改 Story / 改 Task）
  - ...

新：
§十 CodeReview 触发的 Proposal（🔴 改代码前硬前置）
  - 渠道：1（Code Review）
  - Proposal 文档路径：`documentStorage.resolve_path(intent="PROPOSAL", storyId={STORY-ID}, version={N}, title={标题})`
  - 详见 proposal-skill.md §第五步
```

### 渠道 2：Story Review 评审发现 → 自动生成 Proposal

**触发点：** `story-review-skill.md §第四步 bis`（原 StoryReviewUpdatePlan 章节）

**改写规则：** 同渠道 1，触发 `proposal-skill.md §第二步`

**Story Review 报告改造点：**
```
原：
§第四步 bis：生成 StoryReviewUpdatePlan
  - 问题清单
  - 更新计划
  - 字段链路影响分析
  - ...

新：
§第四步 bis：触发的 Proposal（🔴 修改 Story 前硬前置）
  - 渠道：2（Story Review）
  - Proposal 文档路径：`documentStorage.resolve_path(intent="PROPOSAL", storyId={STORY-ID}, version={N}, title={标题})`
  - 详见 proposal-skill.md §第五步
```

### 渠道 3：Coding 异常追溯链 → 触发 Proposal

**触发点：** `coding-skill.md §异常路径：Coding 实时追溯链`

**改写规则：**
- 旧：实时追溯链命中 → 直接触发改 Task/Story/DR
- 新：实时追溯链命中 → **先生成 Proposal** → 再走 Proposal 5 步流程

**Coding 异常路径改造点：**
```
原：
### 追溯层 1 命中：Task 文档缺陷
1. 修 Task 文档
2. 重新生成该 Task 的 CodePlan
3. 重新 Coding

新：
### 追溯层 1 命中：Task 文档缺陷
1. 🔴 先生成 Proposal（渠道 3：Coding 异常）
   1.1 §1 原本是怎么样（Task 文档缺陷描述）
   1.2 §2 要做什么（Task 文档修复目标）
   1.3 §3 怎么做（修哪些章节/字段）
   1.4 §4 涉及范围（标记 Task 涉及）
2. 用 Proposal 走 5 步流程（proposal-skill.md §第五步）
   2.1 改 Task（携带 Proposal 走 task-generate-skill.md）
   2.2 改 Story（如涉及）
   2.3 改 TestCase（如涉及）
   2.4 改 Coding（如涉及）
   2.5 改 Test（如涉及）
3. 重新跑实时追溯链验证问题已解决
```

### 渠道 4：Project Assets 漂移 → 触发 Proposal

**触发点：** `project-assets-update-skill.md §4 动作 2` Update 动作

**改写规则：**
- 旧：发现漂移 → 直接改 §X.Y 章节
- 新：发现漂移 → **先生成 Proposal** → 走 5 步流程

**改写点：** `project-assets-update-skill.md §4.2 步骤 3`：

```
原：
#### 步骤 3：增量更新项目资产对应章节
- 最小化修改原则：只改受影响章节，其他章节不动
- 保持版本兼容：在 §1 lastAuditedAt 写本次更新日期
- 不在更新时大改结构：结构调整触发"重新生成"动作

新：
#### 步骤 3：🔴 先生成 Proposal（渠道 4：Project Assets 漂移）
- §1 原本是怎么样：项目资产 §X.Y 章节与代码不一致
- §2 要做什么：项目资产 §X.Y 修复
- §3 怎么做：如何改 §X.Y
- §4 涉及范围：项目资产 + 可能影响 Story/CodePlan
- 详见 proposal-skill.md §第五步
- 不直接改项目资产，先用 Proposal 走 5 步流程
```

### 渠道 5：用户反馈 → 手动写 Proposal

**触发词：** "修一下 XX" / "发现 XX 问题" / "XX 改一下"

**操作：**
1. 识别渠道（5）
2. 收集用户原话 + 复现步骤
3. 走 Proposal 4 段

### 渠道 6：生产事故 → 手动写 Proposal

**触发词：** "生产故障" / "线上 XX" / "监控告警" / "客户投诉"

**操作：**
1. 识别渠道（6）
2. 收集事故描述 + 影响范围 + 监控数据 + 用户反馈
3. §1 写"现状"（含事故时间/影响用户数/根本原因初步判断）
4. §2 写"目标"（修复 + 预防措施）
5. §3 写"方案"（紧急修复 + 长期方案）
6. §4 涉及范围（紧急 → 跨多 Story 触发）

### 渠道 7：Test 发现 → 自动生成 Proposal

**触发点：** `coding-report-skill.md §四 测试结果` 失败用例

**改写规则：**
- 旧：失败用例 → AI 自报"已知问题"
- 新：失败用例 → **自动生成 Proposal（渠道 7：Test 发现）**

**Coding 报告改造点：**
```
原：
§四 测试结果
  失败用例清单
  已知问题

新：
§四 测试结果
  失败用例清单

§四 bis：触发的 Proposal（🔴 失败用例转 Proposal）
  - 渠道：7（Test 发现）
  - 失败用例 ID + 失败原因 → 写入 Proposal §1
  - Proposal 文档路径：`documentStorage.resolve_path(intent="PROPOSAL", storyId={STORY-ID}, version={N}, title={标题})`
  - 详见 proposal-skill.md §第五步
```

---

## 与其他 SKILL 的衔接

| 上下游 SKILL | 衔接点 |
|------------|-------|
| `story-review-skill.md` | §第四步 bis 改为 Proposal 指针 |
| `code-review-skill.md` | §第四步 bis 改为 Proposal 指针 |
| `coding-skill.md` | §异常路径 实时追溯链 改为触发 Proposal |
| `project-assets-update-skill.md` | §4 动作 2 Update 改为生成 Proposal |
| `coding-report-skill.md` | §四 失败用例 改为触发 Proposal |
| `story-update-skill.md` | 接收 Proposal 作为输入 |
| `testcase-generate-skill.md` | 接收 Proposal 作为输入 |
| `task-generate-skill.md` | 接收 Proposal 作为输入 |
| `document-storage-skill.md` | §2 路径模板增补 Proposal 行 |
| `ae-sdd-update-skill.md` | 边界判定表增补 Proposal 行 |

---

## 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止问题描述散落在对话中 | 失追溯 | §第二步 必写 Proposal 文档 |
| 2 | 禁止各 SKILL 内置 UpdatePlan | 重复维护 | 全部统一到 Proposal |
| 3 | 禁止 §3 步骤过于抽象（如"修代码"）| 不可执行 | 拆解为下游 5 类动作 |
| 4 | 禁止 §4 5 类下游统一填 N/A | 漏改 | 逐项判定 |
| 5 | 禁止 §1 描述无证据 | 主观评判 | 必附 grep/行号/test method |
| 6 | 禁止未走完 5 步流程就归档 | 漏下游 | §第六步 循环判定 |
| 7 | 禁止 Proposal 被多源修改（不单一权威）| 漂移 | 一次性写完，多处引用 |
| 8 | 禁止删除 Proposal 历史 | 失追溯 | §第八步 归档而非删除 |

---

## 执行清单

| # | 动作 | 产出 | 门禁 |
|---|------|--------|------|
| 1 | 识别渠道（1-7）| 渠道标识 | 渠道明确 |
| 2 | 填 §1 原本是怎么样 | 现状描述 + 证据 | 必附证据 |
| 3 | 填 §2 要做什么 | 新行为 + AC | Given-When-Then ≥ 1 条 |
| 4 | 填 §3 怎么做 | 实施步骤 + 字段链路 + 风险 | 步骤可拆解 + 风险评估 |
| 5 | 填 §4 涉及范围 | 5 类下游动作表 | 逐项判定 |
| 6 | §第三步 合理性自检 | 自检报告 | 4 维度全 ✅ |
| 7 | §第七步 7 道闸 | 闸门结果 | 5 个 🔴 + 2 个 🟠 全 ✅ |
| 8 | §第四步 写入 Proposal | Proposal 文档 | 用户确认初稿 |
| 8.5 | **落地存储（🔴 强制）** | 落地确认 | `resolve_path + save_doc` 成功，G-DOC-STORAGE ✅ 后才进入下一步 |
| 9 | §第五步 走 5 步流程 | 5 个下游修改 | 全部完成 |
| 10 | §第六步 循环判定 | 评审通过 | 连续 1 轮无新增 |
| 11 | §第八步 归档 | 归档到 _archive/ | 已归档 |

---

## 维护

- **维护人：** 架构组
- **更新频率：** 每次新增"问题渠道"或流程变更时
- **同步对象：**
  - 所有 SKILL 的"问题处理"章节（统一为 Proposal 指针）
  - `document-storage-skill.md` 增补 Proposal 路径
  - `ae-sdd-update-skill.md` 边界判定表增补 Proposal 行
- **关键变化（2026-06-06 新建）：**
  - 🆕 统一"问题描述 + 解决方案"载体（替代各 SKILL 内置 UpdatePlan）
  - 🆕 4 段必填结构（原本/目标/方案/影响）
  - 🆕 5 步走流程（改 Story → TestCase → Task → Coding → Test）
  - 🆕 多渠道接入（7 个：Code Review / Story Review / Coding 异常 / Project Assets 漂移 / 用户反馈 / 生产事故 / Test 发现）
  - 🆕 "携带 Proposal"机制（下游 SKILL 接收 Proposal 作为输入）
