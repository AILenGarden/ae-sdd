---
name: ae-sdd-update
description: 规范各 SKILL 的内容边界与维护规则。ae-sdd-skill 退守"流程编排"（流程怎么走、节点间如何流转），各子 SKILL 负责"环节内具体规则"（每一步具体怎么做、出错怎么处理）。当用户新增/修改任何 AE 相关 SKILL 时，先查阅本 SKILL 确认内容应放在哪个文件，避免在错误位置撰写或重复堆积。
---

# Auto Engineering Update — SKILL 边界维护规范

> **本 SKILL 不是工作流，而是"SKILL 维护的工作流"。** 它定义 auto-engineering 体系内**每个 SKILL 应该承担什么、不应该承担什么**，杜绝内容在错误位置堆积（这是本次重构的核心目标）。

---

## 核心设计哲学

### auto-engineering-skill = 流程编排（不退守就会腐化）

| 性质 | 含义 | 类比 |
|------|------|------|
| ✅ **应当承担的** | 流程怎么走（Phase 1→2→3）、节点间如何流转、门禁是什么、子 SKILL 索引、状态跟踪 | "项目经理" — 只管"下一步该做什么、由谁做" |
| ❌ **不应当承担的** | 每个环节内"具体怎么做"、讲解模板、报告模板、问题排查细节、术语定义 | "工程师" — "这道题怎么解" |

**反例（历史已发生，本次重构已清理）：**
- ❌ AE-skill 中塞了 3 个讲解模板（应归子 SKILL）
- ❌ AE-skill 中塞了 CodeReview 报告 9 章节模板（应归 `templates/coding/be-codereview-template.md`）
- ❌ AE-skill 中塞了 6 维度前端契约审查清单（应归 `story-review-skill.md`）
- ❌ AE-skill 中塞了 CodingPlan 7 章节 + 10 条门禁（应归 `coding-skill.md`）
- ❌ AE-skill 中塞了测试真实性 8 类禁止（应归 `coding-skill.md`）
- ❌ AE-skill 中塞了 Coding 4 层排查流程（应归 `coding-skill.md`）

**为什么会腐化？** 因为"流程编排"与"具体规则"看似紧密耦合（流程的每一步都有规则），容易把规则顺手写在流程 SKILL 里。**这是"路径依赖"陷阱** —— 一开始写 AE-skill 时为了完整，会把所有相关内容都堆进去；时间一长，AE-skill 变成 2540 行的"百科全书"，没人能维护。

### 各子 SKILL = 环节内具体规则

| 子 SKILL | 职责 | 不应承担 |
|---------|------|----------|
| `story-review-skill.md` | Story 缺陷挖掘 / 合理性判定 / 通过标准 / ①bis 前端契约 6 维度 | 流程编排（AE-skill）、CodeReview 规则 |
| `task-generate-skill.md` | Task 文档生成 / Task 0 公共依赖 / 全局 Task Review / Task 修复流程 | 流程编排、Story 规则、Code 规则 |
| `coding-skill.md` | ⑥ 按 Task 生成代码 / 测试真实性 8 类禁止 / ④bis CodingPlan / ⑥bis 一致性闸 / ⑦bis 对称性闸 / 异常路径 A1-A6 | CodeReview 报告模板（应在 templates/）、流程编排 |
| `testcase-generate-skill.md` | 测试用例生成 / AC 映射 / 合规性校验 | 流程编排、Coding 规则 |
| `dr-update-skill.md` | DR 文档更新 / DR 缺陷修复 | 流程编排、其他文档规则 |
| `story-update-skill.md` | Story 文档更新 / Story 缺陷修复 | 流程编排、Task 规则 |
| `templates/coding/be-codereview-template.md` | CodeReview 报告 9 章节空白模板 | 具体 Story 的填充内容 |
| `templates/coding/be-coding-report-template.md` | Coding 报告空白模板 | 具体 Story 的填充内容 |
| `templates/design/*.md` | DR / Story / Task / Story Review 逻辑汇总等空白模板 | 具体 Story 的填充内容 |
| `templates/testcase/*.md` | 测试用例 / 测试报告空白模板 | 具体 Story 的填充内容 |

---

## SKILL 边界判定表（新增/修改内容时使用）

> 当你拿到一段内容，问自己"这段应该写在哪？"——用本表判定。

| 内容类型 | 判定依据 | 应写入位置 | 严禁位置 |
|---------|---------|-----------|---------|
| **流程节点定义**（Phase 1 包含什么子步骤） | 这是"流程怎么走" | `ae-sdd-skill.md` | 各子 SKILL |
| **节点触发条件**（如"② Review 必须在 ① 完成后"） | 编排级门禁 | `ae-sdd-skill.md` | 各子 SKILL |
| **流程状态机**（state.json 结构 / 流程脱离与再启动） | 全局状态 | `ae-sdd-skill.md` | 各子 SKILL |
| **多 Agent 角色库**（story-writer / code-reviewer 等） | 编排层"由谁做" | `ae-sdd-skill.md` | 各子 SKILL |
| **整体执行清单**（10 项节点级 Checklist） | 编排层门禁 | `ae-sdd-skill.md` | 各子 SKILL |
| **某阶段的具体步骤**（如"第零步：Story 准入检查"） | 这是"Story Review 阶段怎么做" | `story-review-skill.md` | `ae-sdd-skill.md` |
| **某阶段的讲解模板**（如"📖 Story 讲解模板"） | 阶段内的"讲故事"细节 | `story-review-skill.md`（对应阶段） | `ae-sdd-skill.md` |
| **某阶段的报告模板**（CodeReview 9 章节） | 阶段内的产出物空白 | `templates/coding/be-codereview-template.md` | `ae-sdd-skill.md` |
| **Code Review 评审流程**（准入/多维评审/闸门/异常路径/多 Agent） | 阶段内的具体规则 | **`code-review-skill.md`（🆕 2026-06-06 新建）** | `ae-sdd-skill.md`（仅保留角色 7 指针） |
| **Code Review CodeReviewUpdatePlan 模板** | 阶段内产出物空白 | `code-review-skill.md §第四步 bis` 模板（内嵌，不独立成文件） | `templates/coding/` |
| **某阶段的闸门规则**（⑥bis 一致性闸 / ⑦bis 对称性闸 / 8 类测试真实性禁止 / 全文档回扫 / 禁裸 ✅ / 报告-代码对账 / 产出物对账 / 真实 DB-HTTP 覆盖） | 阶段内的硬约束 | **`code-review-skill.md`（🆕 2026-06-06 新建独立 SKILL）** | `ae-sdd-skill.md` / `coding-skill.md`（已迁出，coding-skill 改指针） |
| **某阶段的错误排查**（4 层问题排查） | 阶段内的出错处理 | `coding-skill.md` | `ae-sdd-skill.md` |
| **某阶段的术语定义**（如"DR 是什么"） | 阶段内的概念 | **不放任何文件，靠上下文理解**；如必须定义，写在阶段文件顶部"概念说明" | `ae-sdd-skill.md` |
| **某阶段的禁止事项**（如"测试真实性 8 类禁止"） | 阶段内的强约束 | `coding-skill.md` | `ae-sdd-skill.md`（仅保留高层禁止如"跳过 CodeReview"） |
| **跨阶段的回写规则**（如"Story 变更触发 Task 重生成"） | 跨子 SKILL 的联动 | 写入 `ae-sdd-skill.md` 的"跨阶段联动"章节 + 各子 SKILL 用指针引用 | 单个子 SKILL 写完整联动逻辑 |
| **Plan-first 编排门禁**（如"有确认缺陷时先生成 StoryReviewUpdatePlan，再修改 Story"） | 这是"节点之间如何流转" | `ae-sdd-skill.md` 只写门禁与指针 | `story-review-skill.md` 写成全局流程编排 |
| **StoryReviewUpdatePlan 的内容结构**（问题清单/存疑项/更新计划/影响分析） | 阶段产出物模板 | `templates/design/be-story-review-update-plan-template.md` | `ae-sdd-skill.md` |
| **Story Review 如何生成 UpdatePlan**（缺陷如何转计划、待讨论如何处理） | Story Review 阶段内规则 | `story-review-skill.md` | `ae-sdd-skill.md` |
| **Story Update 如何按 Plan 执行**（只改 Plan 覆盖章节、禁止计划外修改） | Story Update 阶段内规则 | `story-update-skill.md` | `ae-sdd-skill.md` |
| **Story 生成 7 阶段挖掘 SOP**（业务/主流程/AC/接口/数据/Task/①bis） | 阶段内具体规则 | **`story-generate-skill.md`（🆕 2026-06-06 新建）** | `ae-sdd-skill.md`（仅角色 1 指针） |
| **Coding 报告产出 9 章节结构 + 7 道闸** | 阶段内具体规则 | **`coding-report-skill.md`（🆕 2026-06-06 新建）** | `ae-sdd-skill.md`（仅角色 6 指针） |
| **文档存放标准**（路径模板/命名规则/重入处理/版本号/状态码） | AE 体系横向基础设施 | **`document-storage-skill.md`（🆕 2026-06-06 新建）** | 各子 SKILL（每 SKILL 必引用本文件） |
| **建议书（Proposal）的内容结构**（4 段必填 / 7 道闸 / 5 步走流程） | 阶段内产出物模板 | **`proposal-skill.md`（🆕 2026-06-06 新建，重量级）** | `ae-sdd-skill.md`（仅流程编排指针） |
| **建议书模板** | 阶段内产出物空白 | `templates/proposal/proposal-template.md` | `ae-sdd-skill.md`（仅指针） |
| **各 SKILL 的"问题处理"路径**（Code Review/Story Review/Coding 异常/Project Assets 漂移） | 触发 Proposal（不直接生成 UpdatePlan） | **`proposal-skill.md` §多渠道接入设计** | 各 SKILL 内置的 UpdatePlan（已废弃） |
| **AE 体系统一入口 + 智能路由**（分析用户输入属于哪个节点 + 路由到对应 SKILL） | AE 编排层调度 | **`ae-sdd-skill.md` §🎯 统一入口与智能路由**（🆕 2026-06-06 增强） | 各子 SKILL 各自判断路由 |
| **任务节点内子任务拆分 + 多 Agent 并行 + 故障补救** | 阶段内并行执行规则 | **`agent-orchestration-skill.md`（🆕 2026-06-06 新建）** | `ae-sdd-skill.md`（仅含角色 1-8 角色库） |
| **Agent 编排的角色库**（8 角色 story-writer / code-reviewer / test-verifier 等） | 阶段内并行执行 | `agent-orchestration-skill.md` §3 角色库（统一） | `ae-sdd-skill.md` §🤖（已迁出） |
| **完整实现代码**（方法体、条件分支、循环、try-catch） | 这是"怎么实现"的执行细节 | `coding-skill.md`（Coding SKILL 按 Task 骨架"填肉"） | `templates/design/be-task-template.md`（Task 只写骨架） |
| **Task 骨架**（类骨架 + 方法签名 + 伪代码 ≤10 行 + 依赖工具包） | Task 设计产出物 | `templates/design/be-task-template.md §实现方案` | `coding-skill.md`（不在 Coding 里定义骨架格式） |
| **🆕 4 类需求智能路由**（已有 Story / 中大任务 / 小任务 / 微任务）| 智能路由层调度 | `ae-sdd-skill.md §智能路由表 + §路由决策算法 2.2` | 各子 SKILL 各自判定 |
| **🆕 任务规模判定规则**（套 Story 7 区模板能否填满） | 智能路由层判定 | `ae-sdd-skill.md §路由决策算法 2.2 套模板判定步骤` | `task-generate-skill.md`（不重复判定）|
| **🆕 工程根目录路径模板**（重任务 `design/` vs 小任务 `Task/` vs 微任务 `Plan/`）| 文档归属 | `document-storage-skill.md §2.6` | 各子 SKILL 自行决定路径 |
| **🆕 无 Story 上下文独立决策**（TaskSkill / CodingSkill 在无 Story 时如何处理）| 阶段内规则 | `task-generate-skill.md §1.B` + `coding-skill.md §4.2 / §6.0` | `ae-sdd-skill.md`（不重写）|
| **Story 数据模型字段链路标准**（字段链路明细表 + 横向流转对照图；来源→入参/上下文→分层对象→DB/外部依赖→出参） | Story 模板与 Review 检查项 | `templates/design/*story-template.md` + `story-review-skill.md` | `ae-sdd-skill.md` |

---

## 内容回写到正确位置的 5 步流程

> 当你发现某段内容"放错位置了"（或新增内容不知放哪），按此流程操作。

> **母版声明（🆕 v3.0 改造）：** `source/` 是 AE 体系唯一母版（SSOT，git 跟踪）。所有 SKILL、templates、standards、assets、scripts 的日常维护只改 `source/`；`dist/ae-sdd/`（构建产物，git ignored）、本地 Claude skills 目录（`~/.claude/skills/ae-sdd/`）等均视为发布/安装产物，不手工维护。

### 步骤 1：识别内容类型

对照"SKILL 边界判定表"，明确这段属于"流程编排"还是"环节内具体规则"。

- 流程编排 → 候选位置 `ae-sdd-skill.md`
- 环节内具体规则 → 候选位置 **对应阶段的子 SKILL**

### 步骤 2：定位目标 SKILL

按"阶段→子 SKILL"映射（2026-06-10 全面重组后）：

| 阶段 | 目标子 SKILL（物理路径） |
|------|------------|
| Story 生成 / Review / ①bis 前端契约 / Story 缺陷修复 | `../phase1-design/story-review-skill.md` 或 `../phase1-design/story-update-skill.md` |
| Task 生成 / Review / Task 0 / Task 修复 | `../phase2-task/task-generate-skill.md` |
| ④bis CodingPlan / ⑤ Coding / 测试真实性 / 异常路径 / 一致性闸 / 对称性闸 | `../phase2-coding/coding-skill.md` |
| CodeReview 报告（产出物空白） | `../../templates/coding/be-codereview-template.md` |
| Coding 报告（产出物空白） | `../../templates/coding/be-coding-report-template.md` |
| 测试用例 / 测试报告（产出物空白） | `../../templates/testcase/*.md` |
| Story / Task / DR / 逻辑汇总（产出物空白） | `../../templates/design/*.md` |
| StoryReviewUpdatePlan（产出物空白） | `../../templates/design/be-story-review-update-plan-template.md` |
| TestCase 生成 | `../phase1-design/testcase-generate-skill.md` |
| DR Update | `../phase1-design/dr-update-skill.md` |
| 文档存放标准 / 横切依赖 | `../cross-cutting/document-storage-skill.md` |
| 统一问题载体 | `../cross-cutting/proposal-skill.md` |
| Agent 编排 / 跨 AI 工具适配 | `../cross-cutting/agent-orchestration-skill.md` |
| 项目资产管理 | `../cross-cutting/project-assets-update-skill.md` |
| 约束 9 个 | `../../standards/constraints/*.md` |
| 编码思维引擎 | `../../standards/thinking/be-coding-thinking-engine.md` |
| 测试策略 | `../../standards/testing/be-testcase-strategy.md` |
| 项目资产 schema | `../../standards/project-assets/project-assets-schema.md` |
| 项目资产实例 | `../../assets/{projectKey}/*.assets.md` |

### 步骤 3：执行回写

- **新增到子 SKILL** → 在子 SKILL 末尾追加 `## 📋 [章节名]` 标题
- **从 AE-skill 移除重复块** → 在 AE-skill 原位置改为指针：
  ```markdown
  > **📍 详细 [内容] 已下沉到 [`[目标 SKILL]` §[章节锚]](./[目标文件])，本 SKILL 不再重复。**
  > **AE 编排层只关注 [N] 个门禁：** ...
  ```
- **从 templates/ 提取报告模板** → 把空白模板移到 `templates/[阶段]/[模板名].md`，AE-skill 改为指针

### 步骤 4：更新交叉引用

- 涉及多个子 SKILL 的联动 → 在 `ae-sdd-skill.md` 增加一行"跨阶段联动"指针
- 各子 SKILL 之间相互引用 → 用相对路径 `./xxx-skill.md#锚` 跳转
- 新增 Plan-first 类规则 → AE-skill 只增加"必须先有 Plan 才能进入下一节点"的门禁；Plan 内容、生成规则、执行规则分别放到模板和对应子 SKILL
- **改动 `ae-sdd-skill.md`**（新增/删除流程节点、改路由场景、改角色库、改门禁数量）→ **必须同步更新 `README.md` 以下章节**：
  - §3 SKILL 功能清单（新增/删除/重命名 SKILL 时）
  - §4.2 典型流程示例（流程步骤变更时）
  - §8.5 常见变更场景表（新增变更类型时）
  - README 末行"最后更新"日期
- **🆕 2026-06-10 未来防御：修改任一 SKILL 时，必须同步更新 `README.md:5` 的版本号**
  - **触发条件：** 任何 SKILL .md 改动（不仅是 `ae-sdd-skill.md`）→ 必须更新 README 第 5 行 `**版本：** YYYY-MM-DD（最新变更：...）`
  - **操作：** `README.md:5` 的 `**版本：**` 行 = 仓库整体"代际标识"；"最新变更"括号内简述本次变更（如 `+ 4 类需求路由` `+ 工程解耦定位器`）
  - **防止：** 出现"内部大量 2026-06-10 改动但 README 还显示 2026-06-06" 的不一致
  - **章节定位：** `README.md:5` 固定格式 `> **版本：** YYYY-MM-DD（最新变更：...）`

### 步骤 4.5：写入 CHANGELOG（🆕 2026-06-10 强制）

> **🔴 强制：** 每次修改 SKILL 母版，必须在 `CHANGELOG/` 目录新建一个 `YYYY-MM-DD-{主题}.md` 文件。

**操作：**
1. **文件命名：** `YYYY-MM-DD-{主题}.md`（如 `2026-06-10-AE-4类需求路由.md`）
2. **位置：** `CHANGELOG/` 目录下（与 SKILL 母版平级）
3. **模板：** 参考 `CHANGELOG/_template.md`
4. **必填内容：** 变更摘要 + 详细变更（文件:行号）+ 触发原因 + 影响范围 + 验证方式 + Reviewer
5. **历史回填：** 历史变更按 MEMORY.md 索引补建 1 个 .md 文件（不丢历史）

**为什么需要：**
- 之前 SKILL 母版无变更日志，"为什么改"信息散落在 SKILL.md frontmatter / 章节内 emoji 标签 / README 末行长段
- git commit 信息是"什么时候改"而不是"为什么改"
- 一个 1 个文件集中"一次大变更"，比 git log 易查阅 100 倍

**禁止：**
- ❌ 修改 SKILL 后不写 CHANGELOG
- ❌ 在 CHANGELOG/ 之外的地方记录 SKILL 变更历史（除 README.md 末行日期 + git commit）
- ❌ 多个变更共用一个文件（每次大变更独立文件）
- ❌ 删 CHANGELOG/ 里的历史文件（永久保留，git 跟踪）

### 步骤 5：验证无重复

执行一次全文 grep：

```bash
# 在 AE-skill 中 grep "已下沉到" — 应能列出所有外链指针
grep -nE "已下沉|已统一存放" ae-sdd-skill.md

# 在子 SKILL 中 grep 关键章节标题 — 应能在目标位置找到
grep -nE "^## 📋 ①bis|^## 📋 ④bis|^## 📋 测试真实性" *.md
```

---

## 母版修改后的同步规则（强制）

> 本节定义"修改完 AE 母版后，如何让本地运行环境拿到最新内容"。它属于 SKILL 维护工作流，不属于 AE 运行流程。

### 适用范围

任一以下内容发生变更，都适用本节：

- `*.md` 子 SKILL
- `templates/`
- `strategies/`
- `scripts/`
- `project-assets/`
- `README.md`

### 默认规则（🆕 v3.0 双目录分层）

| 对象 | 定位 | 维护方式 |
|------|------|---------|
| `source/`（仓库根 `source/`） | **唯一母版（SSOT）** | 直接修改（开发者编辑这里） |
| `dist/ae-sdd/` | **实例化分发包**（构建产物，git ignored） | 不手工改；由 `bash scripts/build-dist.sh` 从 `source/` 构建 |
| `~/.claude/skills/ae-sdd/` | **本地 Claude skills 安装** | 不手工改；由 `bash scripts/install.sh` 从 `dist/ae-sdd/` 装入 |
| **🆕 v3.0 母版根 `SKILL.md`**（`source/SKILL.md`） | ae-sdd 唯一主入口 | **手工编辑（直接修改主入口）；build 时自动包含** |
| `dist/ae-sdd/SKILL.md`（构建产物） | 分发包入口 | 不手工改；由 build-dist.sh 自动从 `source/SKILL.md` 复制 |
| `~/.claude/skills/ae-sdd/SKILL.md`（安装副本） | 本地 Claude 加载入口 | 不手工改；由 install.sh 自动从 `dist/ae-sdd/SKILL.md` 复制 |

> **🆕 v3.0 重大变更（2026-06-18）：**
> 1. **目录结构重组**：仓库根改为 `source/`（母版）+ `dist/ae-sdd/`（分发包）双目录。
> 2. **主入口已就位**：`source/SKILL.md` 即为 ae-sdd 唯一主入口（直接编辑），不再从 `skills/orchestration/ae-sdd-skill.md` 派生（原派生文件已删除）。
> 3. **废弃 `plugins/ae-sdd/`**：v3.0 起 marketplace plugin 副本路径改为 `dist/ae-sdd/`，`plugins/ae-sdd/` 整个废弃。
> 4. **脚本重命名**：`sync-to-plugin.sh` → `build-dist.sh`（构建）+ `install.sh`（安装）+ `dev-sync.sh`（开发者工具）。
> 5. **安装路径简化**：`~/.claude/skills/ae-sdd/skills/ae-sdd/` → `~/.claude/skills/ae-sdd/`（去掉多余中间层）。

### 修改后动作（🆕 v3.0 工作流）

1. 完成母版修改（直接改 `source/SKILL.md`、`source/skills/xxx-skill.md` 等主入口文件）。
2. 执行本文件 §"内容回写到正确位置的 5 步流程" 中的重复性校验。
3. 如本次变更需要在本地 Claude Skill 中立即生效，运行：

   ```bash
   # 开发者推荐：build + install 一步到位
   bash scripts/dev-sync.sh

   # 或显式两步：
   bash scripts/build-dist.sh  # source/ → dist/ae-sdd/
   bash scripts/install.sh     # dist/ae-sdd/ → ~/.claude/skills/ae-sdd/
   ```

4. 确认同步目标目录（**两个**，由 dev-sync 链式调用产出）：

   ```text
   1) <仓库根>/dist/ae-sdd/SKILL.md                      # 实例化分发包（build 产物）
   2) ~/.claude/skills/ae-sdd/SKILL.md                   # 本地 Claude skills 安装（install 产物）
   ```

5. 两个产物下的 `SKILL.md` 应与 `source/SKILL.md` **完全一致**（tar 整树复制保证）。
6. 在最终回复中明确说明：本次是否已执行 dev-sync / build-dist / install；如未执行，说明"仅修改母版，尚未分发/安装"。

### 同步脚本说明（🆕 v3.0 三脚本分工）

| 脚本 | 位置 | 职责 |
|------|------|------|
| `build-dist.sh` | `scripts/build-dist.sh` | 从 `source/` 构建 `dist/ae-sdd/`（注入 VERSION + plugin.json，剥离 CHANGELOG/docs/marketplace.json）|
| `install.sh` / `install.ps1` | `scripts/install.{sh,ps1}` | 从 `dist/ae-sdd/` 装到 `~/.claude/skills/ae-sdd/`（跨平台 + 本地/远程两模式）|
| `dev-sync.sh` | `scripts/dev-sync.sh` | 开发者工具：build + install 组合 + `--watch` 监听模式 + `--uninstall` |

**build-dist.sh 详细职责：**
1. **校验母版 `source/SKILL.md` 存在性**（不存在则终止）。
2. 从 `source/SKILL.md` YAML frontmatter 提取 `version` 字段。
3. `tar` 整树复制 `source/` → `dist/ae-sdd/`，剥离 `CHANGELOG/` `docs/` `.idea/`。
4. 剥离 `.claude-plugin/marketplace.json`（分发包不携带 marketplace 注册表）。
5. 注入 `dist/ae-sdd/VERSION`（含 version + buildDate）。
6. 注入 `dist/ae-sdd/.claude-plugin/plugin.json`（plugin 自描述元数据）。
7. 验证 `dist/ae-sdd/SKILL.md` 存在性。

**install.sh 详细职责：**
1. 检测运行模式（远程 git clone / 远程 zip / 本地 build / 本地 dist）。
2. 自动调 `build-dist.sh`（如果 dist 不存在）。
3. 备份旧版（`${DST}.bak.<时间戳>`）。
4. `cp -r dist/ae-sdd/. ~/.claude/skills/ae-sdd/`。
5. 验证 SKILL.md + VERSION 写入。

### 禁止

| 禁止 | 原因 | 正确做法 |
|------|------|---------|
| 只改 `dist/ae-sdd/` 或本地 Claude skills 目录 | 产物会被下次 build/install 覆盖，母版丢失变更 | 只改 `source/` 母版 |
| 同时手工维护母版和分发包 | 双源漂移，无法判断哪个是权威版本 | 母版单点维护，分发包由 `build-dist.sh` 生成 |
| 修改母版后假设运行环境已自动更新 | 当前没有自动触发同步机制 | 需要即时生效时显式运行 `bash scripts/dev-sync.sh` |
| ~~手工编辑 `SKILL.md`~~ | ❌ v3.0 已废除此规则 | ✅ **v3.0 起，`source/SKILL.md` 是主入口，**直接编辑即可** |
| ~~修改 `ae-sdd-skill.md` 后同步~~ | ❌ v3.0 已废除 | ✅ **v3.0 起，**直接修改 `source/SKILL.md`**，然后跑 `dev-sync.sh`** |
| ~~运行 `sync-to-plugin.sh`~~ | ❌ v3.0 已重命名 | ✅ **v3.0 起，**运行 `build-dist.sh` + `install.sh`（或 `dev-sync.sh` 一步到位）** |
| ~~把构建产物 commit 到 git~~ | ❌ v3.0 已加 gitignore | ✅ **`dist/` 在 .gitignore 内，不应 commit** |

---

## SKILL 健康度自检清单（每月或重大变更后跑一次）

### AE-skill 健康度

- [ ] AE-skill 总行数 < 1500（当前 1362，已达成）
- [ ] AE-skill 中不出现以下关键词的实质内容（出现只能是 1 行指针）：
  - `接口契约完整性` / `调用流程` / `状态展示` / `错误码` / `边界场景` / `联调支持`（6 维度前端契约）
  - `文件顺序` / `类骨架` / `Mapper SQL` / `测试对应` / `验证点` / `调试回滚`（CodingPlan 7 章节）
  - `@Disabled` / `assertTrue(true)` / `catch 吞噬` / `Thread.sleep`（8 类禁止）
  - `全切面一致性` / `对称性`（核查闸）
  - `分层排查` / `4 层排查`（Coding 问题排查）
- [ ] AE-skill 中所有引用子 SKILL 的位置都用相对路径 `./xxx-skill.md`
- [ ] AE-skill 中"📍 已下沉"指针数 ≥ 实际子 SKILL 数 - 1（每个子 SKILL 至少被引用一次）

### 子 SKILL 健康度

- [ ] `story-review-skill.md` 包含 `## 📖 Story 讲解模板` + `## 📋 ①bis 前端视角接口审视 — 6 维度审查清单`
- [ ] `story-review-skill.md` 包含 `## Plan-first 更新原则`，并引用 `templates/design/be-story-review-update-plan-template.md`
- [ ] `story-update-skill.md` 明确按 `StoryReviewUpdatePlan` 执行，禁止计划外业务语义修改
- [ ] `task-generate-skill.md` 包含 `## 📖 Task 讲解模板`
- [ ] `task-generate-skill.md` 🆕 2026-06-10 包含 `### 1.A 有 Story 上级文档` + `### 1.B 无 Story 上级文档`（小任务场景独立决策分支）
- [ ] `coding-skill.md` 包含 `## 📖 Code 讲解模板` + `## 📋 ④bis CodingPlan` + `## 📋 测试真实性强制规范` + `## 📋 ⑥bis 编码后全切面一致性核查闸` + `## 📋 ⑦bis 全链路对称性核查闸` + `## 📋 Coding 问题分层排查与修改链`
- [ ] `coding-skill.md` 🆕 2026-06-10 包含 `### 6.0 任务规模 × 文档组合` + `§4.2 按任务规模分支读取` + CodingSkill.Plan/Execute 输入参数条件必填 Story 路径
- [ ] `ae-sdd-skill.md` 🆕 2026-06-10 智能路由表包含 4 类需求（已有 Story / 中大任务 / 小任务 / 微任务）+ 路由决策算法 2.2 套 Story 7 区模板判定
- [ ] `document-storage-skill.md` 🆕 2026-06-10 包含 `§2.6 三类任务规模 × 文档路径`（重任务 `design/` / 小任务 `Task/` / 微任务 `Plan/`）
- [ ] `templates/coding/be-codereview-template.md` 9 章节齐全
- [ ] `templates/design/be-story-review-update-plan-template.md` 存在，且包含问题清单、更新计划、字段链路明细表、横向流转对照图、影响分析、执行后验收

### 跨 SKILL 一致性

- [ ] AE-skill 的"子 SKILL 索引"表覆盖目录中所有 `*-skill.md` 文件
- [ ] AE-skill 的"执行清单"中每个步骤都指向唯一一个具体子 SKILL
- [ ] 任何 SKILL 不重复定义"DR 是什么 / Story 是什么"等基础概念（依赖上下文）
- [ ] **README.md §3 SKILL 功能清单与目录中实际 `*-skill.md` 文件数量一致**（不多不少）
- [ ] **README.md 末行"最后更新"日期 ≥ 本次变更日期**（每次修改任一 SKILL 后必须更新）
- [ ] 如本次变更需要即时生效，已运行 `bash scripts/dev-sync.sh`（或显式 `build-dist.sh` + `install.sh`），并确认 `dist/ae-sdd/SKILL.md` 与 `~/.claude/skills/ae-sdd/SKILL.md` 已刷新；如未运行，最终回复已明确说明"仅修改母版"

---

## 禁止的 6 种反模式

| 反模式 | 危害 | 正确做法 |
|--------|------|---------|
| 在 AE-skill 中塞"具体怎么做" | AE-skill 膨胀 → 维护困难 → 与子 SKILL 内容漂移 | 内容下沉到子 SKILL，AE-skill 只保留指针 |
| 同一规则在 AE-skill 和子 SKILL 各写一份 | 两份内容漂移 → 改一处忘改另一处 | 单点维护：要么在 AE-skill 要么在子 SKILL，用指针互引 |
| 子 SKILL 中写"流程编排" | 子 SKILL 承担不该承担的事 → 跨阶段改动要改多个文件 | 流程编排一律在 AE-skill，子 SKILL 只接收"我这一阶段的指针" |
| templates/ 与 SKILL 中并存报告模板 | 模板改动两份不一致 | 报告空白模板只在 `templates/`，SKILL 中只写"按 templates/xxx 模板生成" |
| 手工修改插件副本或本地安装目录 | 产物与母版漂移，下次同步会丢变更 | 只改 `skills/ae-sdd/` 母版，需要生效时运行同步脚本 |
| 修改母版后不说明是否同步 | 使用者不知道当前运行环境是否已拿到新规则 | 最终回复必须说明"已同步"或"仅修改母版，未同步" |

---

## 与其他 SKILL 的关系

- **本 SKILL 不是工作流**，是 SKILL 维护的工作流。仅在新增/修改 SKILL 内容时参考。
- **执行 SKILL 内容时不要加载本 SKILL**（会污染上下文）。判断方式：
  - 用户说"开始 AE 流程"/"从 DR 开始" → 加载 `ae-sdd-skill.md`
  - 用户说"重构 SKILL"/"新增 SKILL"/"SKILL 内容分类" → 加载本 SKILL

---

## 本次重构摘要（2026-06-04）

| 移除项（原 AE-skill 行号） | 大小 | 下沉到 |
|----------------------|------|--------|
| ①bis 6 维度前端契约（L825-1043） | 5.7KB | `story-review-skill.md` |
| ④bis CodingPlan 7 章节（L1327-1502） | 6.3KB | `coding-skill.md` |
| 🔴 测试真实性 8 类禁止（L1604-1766） | 4.4KB | `coding-skill.md` |
| ⑥bis 一致性闸（L1791-1837） | 1.9KB | `coding-skill.md` |
| 七、CodeReview 报告模板（L1859-2383） | 12.5KB | `templates/coding/be-codereview-template.md`（已存在，本次确认无副本） |
| ⑦bis 对称性闸（L2268-2303） | 1.5KB | `coding-skill.md` |
| 📖 讲解规范 L18-222 三个节点模板 | 4.8KB | `story-review-skill.md` / `task-generate-skill.md` / `coding-skill.md` |
| 异常处理中"Coding 4 层排查"（L1268-1325） | 2.6KB | `coding-skill.md` |

**AE-skill 削减效果：** 134,656 字节 → 75,323 字节（-44%）

---

## 本次重构摘要（2026-06-10 任务规模分级）

| 新增项 | 位置 | 备注 |
|------|------|------|
| 4 类需求智能路由（已有 Story / 中大任务 / 小任务 / 微任务）| `ae-sdd-skill.md §智能路由表 + §路由决策算法 2.2` | 套 Story 7 区模板自动判定 |
| 事务简称命名规则（`{服务缩写}-{任务简述}`）| `ae-sdd-skill.md §4 类需求智能路由` + `document-storage-skill.md §2.6` | 服务名缩写：去前缀 `icec-cloud-` 去后缀 `-service`/`-bff` |
| 三类任务路径模板（重任务 `design/` / 小任务 `Task/` / 微任务 `Plan/`）| `document-storage-skill.md §2.6` | 工程根目录相对路径 |
| TaskSkill 加"无 Story 上级文档"分支 | `task-generate-skill.md §1.A / §1.B` | 小任务场景独立决策 |
| CodingSkill Plan/Execute 输入参数条件必填 Story 路径 | `coding-skill.md §CodingSkill 对外调用契约` | 微任务不传 Story 路径 |
| CodingSkill §4.2 按任务规模分支读取 | `coding-skill.md §4.2` | 微任务跳过 Task 文档读取 |
| CodingSkill §6.0 任务规模 × 文档组合 | `coding-skill.md §6.0` | 三类 100% 全流程，仅文档数量不同 |

**核心原则：**
- **流程深度不减**：3 类规模 100% 走 CodingModel 11 维决策 + 14 条 CodingPlan 门禁 + TR-1~TR-7
- **文档数量递减**：重任务 6 类 → 小任务 5 类 → 微任务 3 类
- **独立决策**：无 Story 时 CodingModel 决策 + 核心链路保护照样走，但**禁止伪造 Story 引用**
- **多 Agent 不变**：由 agent-orchestration-skill 按"任务可拆性"判定，与规模无关

---

## 本次重构摘要（2026-06-10 SKILL 母版目录全面重组 + 3 项配套整改）

| 新增项 | 位置 | 备注 |
|------|------|------|
| **13 SKILL 拆 7 子目录** | `skills/orchestration/` + `skills/phase1-design/` + `skills/phase2-task/` + `skills/phase2-coding/` + `skills/phase3-review/` + `skills/cross-cutting/` | 按"流程节点 + 横切依赖"分类 |
| `constraints/` + `strategies/` 合并为 `standards/` | `standards/constraints/` + `standards/thinking/` + `standards/testing/` + `standards/project-assets/` | 原 9 约束 + 3 策略 + 2 schema/template 都进 standards/ |
| `project-assets/` 改名 `assets/` | `assets/{projectKey}/` | 实际项目资产 |
| 小任务/微任务 `.ae-task/` `.ae-plan/` 隐藏目录 | `document-storage-skill.md §2.6` | 避免污染 IDE 视图 |
| 人工审核点 4 → 5（加 CodingPlan 评审，删 Coding 完成评审）| `ae-sdd-skill.md` 整体流程 + 整体执行清单 + 人工节点表 | 节点编号 1 → 1.5 → 2 → 2.5 → 4 |
| 同步 `sync-to-plugin.sh` 后的新目录 | 母版 → `~/.claude/skills/ae-sdd/skills/ae-sdd/` | ❌ v3.0 已废弃此机制，改为 `source/` → `dist/ae-sdd/` → `~/.claude/skills/ae-sdd/` 三层构建 + 安装 |

**Why：**
- 之前按"文件类型"分（`templates/` `constraints/` `strategies/` `project-assets/` 散落），无法一眼看出"哪个 SKILL 用于哪个流程节点"
- 重组成"流程节点 + 横切依赖"后，AE-skill 编排层 → 节点 SKILL → 横切 SKILL 的调用链一目了然

**关键原则（保持不动）：**
- 单一权威源 = 母版，plugins 副本只读
- 物理目录 + 逻辑分层分离：物理按流程节点，便于维护；逻辑仍是 4 层架构
- 4 类需求路由（已有 Story / 中大 / 小 / 微）继续按 §智能路由表 判定

---

**维护原则：当你不确定一段内容放哪时，先看"SKILL 边界判定表"，再问"这是流程编排还是环节内具体规则"。99% 的情况能立即定位。剩下 1% 在本 SKILL 评论区或 issue 中讨论。**
