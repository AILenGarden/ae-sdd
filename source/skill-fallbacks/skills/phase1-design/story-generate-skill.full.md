---
name: story-generate
description: Story 生成 SKILL - Phase 1 ① 节点的环节内具体规则。从 DR 生成完整 Story 文档，按标准输出 StoryGeneratePlan，并触发 Story Review。当用户说"生成 Story"、"写 Story"、"从 DR 生成 Story"、"Story 起草"时触发。
---

# Story Generate - 从 DR 生成 Story Skill

> **🆕 v3.9.3 重大变更：**
> 1. §第零步：输入清单统一指向 `story-input-checklist.md` SSOT，13 项自检表 + CLI 门禁双重保障
> 2. §第一步：每个输入项的"提取什么"改为引用 SSOT 的 13 项定位
> 3. §第二步：新增"7 阶段 → 模板章节映射表"（来自 generation-standard §2.5）
> 4. §第三步 bis：**新增"来源追溯与验证"步骤**，4 个子步骤（字段来源追溯 / 不合理入参检测 / 跨文档字段对齐 / 设计来源标注）
> 5. §第七步：闸门由 8 道 → 10 道（新增来源追溯闸 + 章节映射闸）

## 目标

从 DR、PRD、产品原型和项目资产生成完整 Story，要求内容可执行、可评审、与 DR 一致。

## 输出边界（最高优先级）

Story 生成的用户可见产出只有 Story 主文档。Story 正文只写当前生效的需求、验收、接口、数据、实现任务和风险约束，不写生成过程。

- 禁止把输入读取清单、方案比较过程、门禁日志、Review 循环、来源追溯过程、Agent 对话、执行流水或“本次生成说明”写入 Story 正文。
- 禁止在 Story 正文或本次 Story 任务中创建、更新或附带 CHANGELOG / 变更历史文档。
- DR 是只读上游输入；Story 任务不得创建或更新 DR、DR_SUPPLEMENT、DR Review 或任何 DR 草稿。缺少 DR 时停止并报告阻断原因，不得转而生成 DR。
- `StoryGeneratePlan`、`STORY_SOURCE_TRACE`、`STORY_WRITER_REPORT` 和 Review 对比表若因系统门禁必须落盘，只能作为内部机器产物，不能回写 Story、不能在最终答复展开，也不能替代 Story 主文档。
- 除非用户明确点名，默认不生成 Story Supplement、Proposal、Report 或其他过程文档。

## 依赖标准

- [Story 输入清单 SSOT（v3.9.3 新增）](../../standards/story/story-input-checklist.md) **← 单一权威源**
- [Story 生成标准](../../standards/story/story-generation-standard.md)
- [Story 前端接口契约标准（①bis）](../../standards/story/story-frontend-contract-standard.md)
- [StoryGeneratePlan 模板](../../templates/design/story-generate-plan-template.md)
- [Story 生成 Agent 任务分配卡](../../templates/design/story-writer-prompt-template.md)
- [Story 模板（主模板，已合并 be-story-template.md）](../../templates/design/story-template.md)
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)
- [`review-loop-skill.md`](../cross-cutting/review-loop-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 输出文档 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| Story 主文档 | `ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md` | 不带版本号（原地更新）|
| Story Supplement | `ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 |
| Story-WriterReport | `ae-sdd doc save --intent STORY_WRITER_REPORT --work-item {W} --story-id {S} --content-file 草稿.md` | 带 r{N} |
| Review 对比表 | `ae-sdd doc save --intent REVIEW_COMPARE --work-item {W} --story-id {S} --version "v1-to-v2" --content-file 草稿.md` | 带 v{N}-to-v{M} |
| StoryGeneratePlan | `ae-sdd doc save --intent STORY_GENERATE_PLAN --work-item {W} --story-id {S} --content-file 草稿.md` | 带 r{N} |
| 🔴 **来源追溯报告（v3.9.3 新增）** | `ae-sdd doc save --intent STORY_SOURCE_TRACE --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |

## 整体流程

```text
触发
  → 【v3.9.3 强化】第零步：加载 13 项输入（SSOT prose 自检 + G-STORY-CTX 门禁）
  → 【v3.9.3 强化】第一步：读取输入（按 SSOT 13 项定位）
  → 完成实现方案决策基线
  → 【v3.9.3 强化】第二步：按"7 阶段 → 模板章节映射表"生成 Story
  → 【v3.9.3 强化】第三步 bis：来源追溯与验证（4 子步骤）
  → 第三步：合理性自检
  → 第四步：写入 Story 文档
  → 第四步 bis：生成 StoryGeneratePlan
  → 第五步：触发 Story Review
  → 第六步：循环判定
  → 【v3.9.3 强化】第七步：闸门校验（10 道闸含来源追溯闸 + 章节映射闸）
```

## 第零步：准入检查（🆕 v3.9.3 强化为 SSOT 化）

**输入清单统一指向 [`story-input-checklist.md`](../../standards/story/story-input-checklist.md) 单一权威源（SSOT）。**

### 0.1 加载 SSOT
读 `source/standards/story/story-input-checklist.md` 全文，按 §3 输入加载 SOP 4 步执行：
1. 定位（13 项输入逐一确定文件路径）
2. 读取（按 SSOT §2 加载 API）
3. 输入完整性自检（13 项 prose checklist）
4. CLI 门禁（机械层）

### 0.2 13 项 prose 自检表（必须全部 ✅）

```
[ ] A1 DR 文档已读取
[ ] A2 PRD 文档已读取
[ ] A3 产品原型已读取（或标注"无原型"）
[ ] A4 历史 RA 已读取（如有）
[ ] B1 项目资产已读取
[ ] B2 项目约束已读取
[ ] B3 前端约束已读取（如涉及前端）
[ ] B4 依赖能力清单已扫描
[ ] C1 依赖 Story 已加载并比对字段
[ ] C2 阻塞下游 Story 已识别（如有）
[ ] D1 Story 模板已加载
[ ] D2 Story 生成标准已加载
[ ] D3 Story 审核标准 + 前端契约标准已加载
[ ] D4 测试策略已加载
```

### 0.3 🔴 机械门禁（保留 v3.9.1 命令，v3.9.3 扩展检查项）

```bash
ae-sdd gates check --only G-STORY-CTX
```

`G-STORY-CTX` 校验 `constraints/assets/DR/PRD/dependsStory/sourceTrace` 6 类上下文已读齐（v3.9.3 在原 4 类基础上新增 `dependsStory` 与 `sourceTrace`，对应 SSOT 中 C1 与 §4 来源追溯）。注册表 `CONTEXT_GATE_REGISTRY`，复用 `document-storage-skill` 的 `get_constraints/get_assets` API + `paths.find_doc`/`_find_prd_files` + 新增的 `_check_depends_story`/`_check_source_trace`。

未过 → **BLOCK，禁止进入生成**。

## 第一步：读取输入（🆕 v3.9.3 强化为引用 SSOT）

**所有 13 项输入的定位路径与加载方式全部以 [`story-input-checklist.md`](../../standards/story/story-input-checklist.md) §2 为准**。本节仅列出本 Skill 特有的提取要求：

- DR：提取业务规则（BR-XX）、验收标准（AC-XX）、接口契约（API/SPI）、数据模型（表/字段）、异常场景、领域模型（聚合根/实体/值对象）。**关键章节号必须记录**用于后续 §三步 bis 来源追溯。
- PRD：提取业务背景、用户场景（含角色）、业务规则、用户旅程图（用于前端契约）。
- 产品原型：提取 UI 流程（页面/按钮/事件）、状态展示（颜色/图标/文案）、边界场景（空态/弱网/超长）。
- 项目资产：按 `assets.md §3/§5/§7` 分别取分层、命名、契约入口。
- 项目约束：按 `constraints/*.md` 全文逐条读，提取硬约束（鉴权/事务/限流/i18n）。
- 依赖 Story：按元信息"复用其他 Story"表列出 ID，**逐个调 `ae-sdd doc resolve --intent STORY --story-id {ID}` 读取**并提取依赖的接口契约/数据模型/错误码。
- Story 模板：按章节顺序逐章节加载，记录章节标题与必填/选填标记。
- Story 生成标准：加载 §2.5 章节映射表（生成时使用）+ §4 10 道闸（自检时使用）。
- Story 审核标准：加载 §A-E + §A.bis + §D.bis + §D.ter（Review 阶段使用，本阶段先预加载）。

## 第一步 bis：实现方案决策基线

所有非平凡实现点都必须先判断：

1. 是否复用现有能力
2. 是否参考成熟方案
3. 是否需要新建能力
4. 候选方案的五维质量如何
5. 最终能力归属在哪里

任何未完成基线判断的实现点，禁止进入接口、伪代码、Task 和第三方服务设计。

## 第二步：Story 章节生成（🆕 v3.9.3 强化为按映射表生成）

按 `story-generation-standard.md` 的 **§2.5 7 阶段 → 模板章节映射表** 生成，禁止跳章节或合并章节。映射表 SSOT 在 `source/standards/story/story-generation-standard.md §2.5`，本 Skill 必须按映射表逐章节填充。

| 阶段 | 对应模板章节（速查） | 关键填写要求 |
|------|---------------------|-------------|
| A. 业务背景与核心价值 | §元信息 / §用户故事 / §业务价值 / §范围 / §前置条件 / §涉及工程 / §触发入口 | 元信息"来源 PRD / 来源 DR"必须填具体文档 ID + 章节号 |
| B. 主流程与异常流程 | §主流程 / §异常流程 / §实现方案决策基线 / §实现伪代码 | 主流程每步标注对应 DR §3.x 规则编号 |
| C. AC 验收标准 | §AC 覆盖策略基线 / §AC 明细 / §用例设计映射 / §测试策略 / §验收记录 | 每条 AC 标注 Given-When-Then + 对应 DR §验收标准 |
| D. 接口契约 | §接口契约（含 ①bis 6 维度） / §错误码 / §🔴前端错误码处理 | 每字段四维（来源/必要性/合理性/可行性）必填 |
| E. 数据模型 | §数据模型（含表变更/字段变更/索引变更/DDL/DB CRUD/字段链路映射/枚举映射/常量与魔法值） / §配置变更 | 字段链路必须闭环 |
| F. 实现任务映射 | §实现方案决策基线（含决策五表） / §实现任务映射 / §第三方服务 / §偏离声明 | 每个实现点必须走完"复用扫描→成熟方案→质量评估→归属" |
| G. ①bis 前端接口契约 | §接口契约（已内聚 6 维度，v3.9.5 合并）/ §🔴状态流转前端展示 / §🔴边界场景前端处理 / §🔴前端错误码处理 | 6 维度全填，任一缺失 = 阻断 |
| 跨阶段通用 | §非功能设计 / §依赖与风险 / §未决问题 | 有写操作必填幂等设计 |

> 🔴 **章节映射闸（v3.9.3 新增）：** 7 阶段生成完成后，必须按本表逐章节核对；任意阶段缺失对应模板章节 → 阻断。

## 第三步：合理性自检

按 `story-generation-standard.md` §4 执行 **10 道闸** 自检（原 8 道闸 + v3.9.3 新增的来源追溯闸 + 章节映射闸）。

## 第三步 bis：来源追溯与验证（🆕 v3.9.3 新增）

> 🔴 **目的：** 解决"Story 字段来源不可追溯"问题。在合理性自检之后、写入 Story 之前，必须执行本步骤。

### 3bis.1 字段来源追溯
对接口契约章节中每个字段的"来源"列，逐一验证：

- [ ] 该来源在 PRD/DR/原型/依赖 Story 中确实存在
- [ ] 字段类型与来源文档声明一致（Long ≠ String）
- [ ] 必填/可选标记与来源文档一致
- [ ] 金额字段标注了单位（分/元）+ 精度
- [ ] 时间字段标注了格式（ISO8601 / `yyyy-MM-dd HH:mm:ss`）+ 时区（UTC+8 / UTC）
- [ ] 枚举字段列出全部枚举值 + 含义

### 3bis.2 不合理入参检测

- [ ] 声称来源为 CurrentUser 的字段，确认不由前端传入（安全风险）
- [ ] 不存在"凭空出现"的字段（来源缺失或来源不可达）
- [ ] 不存在与来源文档矛盾的字段
- [ ] ID 字段类型为 Long/Integer/String（依项目规范），不混用
- [ ] 布尔字段类型为 Boolean，不为 String

### 3bis.3 跨文档字段对齐

- [ ] 提取 PRD 中所有接口字段 → 与 Story Request 逐字段比对（名称/类型/必填/校验规则）
- [ ] 提取 DR 中所有接口字段 → 与 Story Request 逐字段比对
- [ ] 提取依赖 Story 的出参字段 → 与本 Story Request 逐字段比对
- [ ] 不一致项：标注为 🔴 阻断型，禁止继续

### 3bis.4 设计来源标注（章节级）

对以下内容标注上游来源（精确到章节/段落，格式 `📌 来源：{文档类型} {文档ID} §{章节号}`）：

- [ ] 每条 AC → 对应 DR §验收标准 / PRD §业务规则
- [ ] 每个接口字段（Request + Response） → 对应 PRD/DR §字段定义
- [ ] 每个数据模型字段 → 对应 DR §数据模型
- [ ] 每个异常流程行 → 对应 DR §异常场景
- [ ] 每个状态值 → 对应 DR §状态机
- [ ] 每个错误码 → 对应 DR §错误码表
- [ ] 每个复用项 → 对应依赖 Story §接口契约

**输出：来源追溯报告（写入 STORY_SOURCE_TRACE 文档，作为 Review 输入）**

## 第四步：写入 Story 文档

按 `story-template.md` 写入 Story 正文。

写入前执行输出边界检查：正文不得包含过程叙述、变更历史、CHANGELOG、DR 正文/草稿或任何“详见本次生成过程”的段落；发现即删除后再保存。

## 第四步 bis：生成 StoryGeneratePlan

按 `story-generate-plan-template.md` 写入 Plan。

## 第五步：触发 Story Review

写入完成后，触发 `story-review-skill.md`。

## 第六步：循环判定

Story Review 反馈后进入修正循环，规则遵守 `review-loop-skill.md`。

## 第七步：闸门校验（🆕 v3.9.3 由 8 道闸 → 10 道闸）

Story 生成必须通过 `story-generation-standard.md §4` 的 **10 道闸**：

| # | 门禁 | 通过标准 | 来源标准 |
|---|------|----------|----------|
| 1 | 完整性 | 7 阶段 + 模板章节映射全覆盖 | generation-standard §2.5 |
| 2 | DR 一致性 | 业务规则 / AC / 接口 / 数据模型 / 任务映射与 DR 100% 一致 | generation-standard |
| 3 | 可执行性 | 每条 AC 可独立测试 | generation-standard |
| 4 | 语言精确性 | 禁止纯主观词和空泛判断 | generation-standard |
| 5 | 方案基线 | 非平凡实现点完成 5 项判断 | generation-standard §2 |
| 6 | 可评审性 | 命名、路径、结构符合项目资产约束 | generation-standard |
| 7 | 能力归属 | 核心能力只有一个唯一实现点 | generation-standard |
| 8 | 前端契约 | ①bis 6 维度完整 | frontend-contract-standard |
| **9** | **🔴 来源追溯闸（v3.9.3 新增）** | **每个 AC / 字段 / 异常流程 / 数据模型字段都有章节级来源标注；来源缺失 = 阻断** | **input-checklist §4** |
| **10** | **🔴 章节映射闸（v3.9.3 新增）** | **7 阶段生成的每个阶段对应模板章节全部填充；任意阶段缺失对应章节 = 阻断** | **generation-standard §2.5** |

> 🔴 任一门禁未过，禁止写入 Story 正文。

## 禁止事项

- 禁止未读齐输入就开始写 Story（违反 → 触发 G-STORY-CTX 门禁 BLOCK）
- 禁止跳过方案决策直接写接口、伪代码和 Task
- 禁止把示例当正文
- 禁止跳过 ①bis 六维度
- 禁止未过闸门就写入最终文档
- **🆕 v3.9.3：** 禁止在 Story 中填写"无来源"或"详见 PRD"等模糊来源标注
- **🆕 v3.9.3：** 禁止在依赖 Story 字段未对齐前进入下一阶段
- **🆕 v3.9.3：** 禁止跳过 §三步 bis 来源追溯与验证步骤
- **输出边界：** 禁止在 Story 任务中生成 CHANGELOG 或任何 DR 文档；禁止把过程内容写入 Story 正文。
