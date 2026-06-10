---
name: auto-engineering
description: 端到端自动化工程 SKILL。从 DR 出发，经过 Story 生成、Review、Task 生成、Coding、测试，直到全部通过。当开发者说"启动自动化工程"、"从 DR 开始实现"、"端到端实现"、"继续流程"、"继续上次"时触发。支持流程状态跟踪与中断后恢复。
---

# Auto Engineering — 端到端自动化工程 Skill

---

## 🔴 输出核心原则（最高优先级，贯穿所有 SKILL 和所有阶段）

> **AI 生成任何内容时必须遵守以下三条，违反即视为输出无效：**

| 原则 | 要求 | 违反示例 |
|------|------|---------|
| **基于事实** | 所有输出必须有明确来源（DR / PRD / 项目资产 / 用户告知 / 代码读取结果）。来源必须可引用、可定位。 | "通常情况下会有一个 status 字段" |
| **禁止猜测** | 对于不确定的信息，不得猜测后输出；必须标注 `{待确认}` 并主动向用户或项目资产提问。 | 自行补全未读取过的接口路径、字段类型、配置值 |
| **禁止杜撰** | 不得编造不存在于输入材料中的业务规则、字段、场景、错误码、类名、配置项。即使内容"看起来合理"，没有来源就不能写进文档或代码。 | 凭经验写一个"应该有的"字段到数据模型里 |

> **遇到信息缺失时的标准动作：**
> 1. 停止生成该部分
> 2. 明确说明"缺少什么信息、应从哪里获取（项目资产 / 用户 / DR / PRD）"
> 3. 等待补充后继续，禁止用占位内容跳过

---

## 🔴 实现方案决策基线（最高优先级，贯穿 Story → Task → Coding 全链路）

> **所有实现方案在落笔前必须完成以下四步，缺任意一步视为实现方案无效，禁止进入下一阶段：**

| 步骤 | 要求 | 适用范围 |
|------|------|---------|
| **① 现有能力复用扫描** | 不只是第三方集成——所有实现点（接口、状态机、领域服务、通知、定时任务、缓存、幂等、外部集成等）都必须先扫描：项目资产已有实现 / 依赖 Story / 历史 Task / 公共组件 / 平台能力 / 团队约定。有则复用，不复用必须写明原因。 | 全部实现点 |
| **② 业内成熟方案参考** | 对非平凡实现点，必须列出业内成熟方案或团队既有成熟实现（如状态机→事件驱动/转移表/领域对象 transition；幂等→唯一约束/乐观锁；通知→通知中台/MQ），并说明采用或不采用的理由。不得凭直觉直接选方案。 | 状态机 / 幂等 / 补偿 / 消息 / 外部集成等非平凡实现 |
| **③ 五维代码质量评估** | 实现方案必须同时满足五维：**可用性**（完整覆盖业务场景/AC）、**高效性**（无重复查询/大事务/阻塞）、**可维护性**（复用团队抽象/能力单一归属）、**健壮性**（覆盖失败/幂等/补偿/可观测）、**可读性**（命名/分层/状态语义清晰）。任一维度不达标 → 🔴 阻断。 | 全部实现点 |
| **④ 核心能力归属唯一** | 每个核心业务能力（如"结单"、"推送"、"状态流转"）只能有一个唯一实现点（owner Task / owner 类 / owner 方法）。多个入口只能调用它，不得各自实现一套。 | 跨 Task / 跨流程触发同一业务逻辑时强制 |

> **不复用 / 新建能力时的举证要求：** 必须说明对可维护性（后续修改要改几处？）、健壮性（团队已有实现的坑是否重蹈？）的影响。"我自己写一个更简单"不是有效理由。

---

> **执行声明：本 SKILL 是强制执行的工作流，不是参考指南。AI 必须严格按本 SKILL 规定的顺序和标准执行每个步骤，不得跳过任何步骤，不得自行决定绕过任何门禁。人工审核节点必须等待用户确认后才能继续，禁止自动决策。**

**门禁定义：门禁（Gate）是强制验证点，每一步完成后必须通过门禁检查才能进入下一步。包括但不限于：读取文件确认、用户确认、编译通过、服务启动成功、测试 Pass。跳过任一门禁属于违规行为。**

## 目标

从 DR 设计文档出发，自动驱动整个研发流程直到代码通过所有测试。串联所有子 SKILL，形成完整的自动化闭环。

---

## 🎯 统一入口与智能路由（🆕 2026-06-06 增强 — AE 体系的"AI 智能调度层"）

> **🔴 核心立场：** 本 SKILL 是 AE 体系的**统一入口 + 智能路由**，**所有用户输入都先经过本 SKILL 分析**再路由到对应节点 SKILL 执行。
>
> **🔴 关键能力：**
> 1. **分析用户输入**属于哪个流程节点（Phase 1 ①/②/③、Phase 2 ④/⑤、Phase 3 ⑦、重入、Proposal、其他）
> 2. **路由到对应 SKILL**（见 §智能路由表）
> 3. **支持流程节点内子任务并行**（同节点内子任务可拆给多 Agent 并行，详见 [`agent-orchestration-skill.md`](../cross-cutting/agent-orchestration-skill.md)）
> 4. **支持重入流程**（state.json 读 + 续接）

### 智能路由表（6 大节点 + 4 重入场景 + 🆕 4 类需求智能路由）

#### 基础路由表（流程节点）

| 用户输入关键词 / 场景 | 路由到 SKILL | 节点 |
|---------------------|------------|------|
| "从 DR 开始" / "生成 Story" / "写 Story" / "Story 起草" | `story-generate-skill.md` | Phase 1 ① |
| "Story 评审" / "审 Story" / "Story Review" | `story-review-skill.md` | Phase 1 ② |
| "生成测试用例" / "补测试用例" | `testcase-generate-skill.md` | Phase 1 ③ |
| "生成 Task" / "写 Task 文档" | `task-generate-skill.md` | Phase 2 ④ |
| "开始 Coding" / "写代码" / "实现 Story" | `coding-skill.md` | Phase 2 ⑤ |
| "出 Coding 报告" / "Coding 完成" | `coding-report-skill.md` | Phase 2 ⑤ |
| "Code Review 报告" / "出 CR 报告" / "评审代码" | `code-review-skill.md` | Phase 3 ⑦ |
| "从 X 继续" / "重入 Y 流程" / "续接" | **🔴 先读 state.json** 判定重入点，再路由到对应 SKILL | 任意 |
| "修一下 XX" / "发现 XX 问题" / "生产故障" / "客户反馈" | `proposal-skill.md` | 任意渠道 |
| "代码写错了" / "编译失败" / "测试失败" | `coding-skill.md §异常路径` → 触发 `proposal-skill.md` | 异常渠道 3 |
| "审计项目资产" / "双源一致性" / "每月审计" | `project-assets-update-skill.md §5 审计` | 横向 |
| "修改/补 Story" / "Story Update" | `story-update-skill.md` | 任意（携带 Proposal）|
| "修改/补 Task" / "Task Update" | `task-generate-skill.md §5bis 全局 Task Review` | 任意（携带 Proposal）|
| "修改/补 Coding" / "改代码" | `coding-skill.md` + 携带 `proposal-skill.md` 输出的 Proposal | 任意（携带 Proposal）|
| "放文档哪里" / "命名" / "重入新建还是修改" | `document-storage-skill.md`（横切依赖）| 任意 |

#### 🆕 4 类需求智能路由（2026-06-10 任务规模分级）

> **核心思路：** 把需求分 4 类，不同规模走不同 SKILL 组合。**所有规模 100% 走 CodingModel 11 维决策 + 14 条 CodingPlan 门禁 + TR-1~TR-7**，只是流程深度和文档数量不同。

| 需求类型 | 触发词示例 | 判定方法 | 路由到 | 事务命名 |
|---------|----------|---------|--------|---------|
| **1. 已有 Story** | "审 STORY-001" / "STORY-001 重入" | `state.json` currentStep 判定 | `story-review-skill.md`（按当前步骤）| `{STORY-ID}` |
| **2. 中大任务（重）** | "做个用户管理功能" / "实现融云回调" / "做一个新功能" | **套 Story 7 区模板能套满 4+ 区** | `story-generate-skill.md` → `story-review-skill.md` → ... | `{STORY-ID}` |
| **3. 小任务（轻）** | "加个缓存预热" / "加个重试机制" / "加个 XX" | 套 Story 7 区只能套 2-3 区 | `task-generate-skill.md`（**跳 Story**）| `Task-{服务缩写}-{任务简述}` |
| **4. 微任务** | "改个枚举值" / "改个常量" / "重命名个字段" / "做个微调" | 套 Story 7 区套不出（0-1 区）| `coding-skill.md` 直接调 `CodingSkill.Plan` 出 CodingPlan（**跳 Story + 跳 Task**）| `Plan-{服务缩写}-{任务简述}` |

**事务简称命名规则（2026-06-10 用户确认）：**
- 格式：`{服务名缩写}-{任务简述}`
- 服务名缩写：去掉 `icec-cloud-` 前缀和 `-service`/`-bff` 后缀，保留核心
  - `icec-cloud-life-cs-service` → `cssv`
  - `icec-cloud-boss-user-service` → `usv`
  - `icec-cloud-boss-user-bff` → `ubff`
- 任务简述：业务名 / 功能名（2-3 个单词，尽量简短保留语义）
- 完整例子：`cssv-rongcloud-callback`、`usv-cache-preheat`、`ubff-user-export`

**判定算法（路由决策算法第 2.2 步）：**

```
2.2 【🆕 任务规模判定】（在 2.1 关键词匹配之后）

  显式触发词识别：
  ├─ 出现 "Story-XX" / "STORY-ID" / "审 Story" → 类型 1（重入到 Story Review）
  ├─ 出现 "出 Story" / "做一个新功能" / "实现 XX" → 类型 2
  ├─ 出现 "出个 Task" / "加个 XX" / "做个 XX" → 类型 3
  ├─ 出现 "改个 XX" / "修个 XX" / "重命名 XX" → 类型 4
  └─ 无显式触发词 → 【自动判定】套 Story 7 区模板
      ├─ 套满 4+ 区 → 类型 2（中大任务）
      ├─ 套满 2-3 区 → 类型 3（小任务）
      └─ 套不出或只套 1 区 → 类型 4（微任务）

  套模板判定步骤（自动判定时执行）：
  1. 列出任务涉及的 7 个区：① 业务背景 ② 主流程 ③ AC ④ 接口契约 ⑤ 数据模型 ⑥ 实现任务映射 ⑦ ①bis 前端契约
  2. 对每个区，看任务描述能否给出实质性内容（不只是"无"）
  3. 统计能填满的区数 → 套模板判定
```

**路径差异：**
- **类型 1-2（重任务）：** 文档收束到 `{STORY-ID} Doc/`，6 大顶层分类（Design/Review/Output/Update/TestReport/Runtime），归属 `design/` 相对路径
- **类型 3（小任务）：** 文档收束到 `{工程根}/.ae-task/Task-{事务简称}/`（隐藏目录，如 `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service\.ae-task\Task-cssv-rongcloud-callback\`）
- **类型 4（微任务）：** 文档收束到 `{工程根}/.ae-plan/Plan-{事务简称}/`（隐藏目录，如 `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service\.ae-plan\Plan-cssv-enum-fix\`）

**完整路径模板见 `document-storage-skill.md §2.9`。**

### 路由决策算法（5 步）

```
0. 【🆕 工作区与项目资产检查】（每次 SKILL 启动时执行，任何后续流程的前置）
   ↓
   0.1 判断是否有明确工作区（projectKey / gitPath 已知？）
       ├─ 未知 → 询问用户"请告知工程目录或项目名（projectKey）"
       └─ 已知 → 进入 0.2
   ↓
   0.2 调用 document-storage-skill.get_assets(projectKey) 检查项目资产
       ├─ 资产存在 → 静默加载，进入步骤 1
       └─ 资产不存在 → 进入 0.3
   ↓
   0.3 【资产缺失：明确告知用户并生成资产】
       AI 输出：
       "⚠️ 未找到项目 {projectKey} 的资产文件（{assetsPath}）。
        项目资产包含微服务清单、分层规则、命名约定、工程约束等，
        是后续所有流程的上下文基础。
        正在为您生成项目资产，这需要扫描工程目录……"
       ↓
       调用 project-assets-update-skill.md §3（生成动作）
       → 9 步探查 SOP（读 CLAUDE.md + AGENTS.md + 扫描工程 + 抽典型类）
       → 输出：{gitPath}/.ae-project/assets.md（或 assets/ 目录）
       ↓
       生成完成后 AI 告知用户：
       "✅ 项目资产已生成：{assetsPath}
        包含：{microservices 数量} 个微服务 / {分层} 层架构 / {技术栈}
        请确认资产内容是否准确，确认后继续流程。"
       ↓
       用户确认 → 进入步骤 1
       用户发现问题 → 先走 project-assets-update-skill.md §4（更新动作）
   ↓
1. 接收用户输入
   ↓
2. 关键词匹配（智能路由表 §1）
   ├─ 命中 6 大节点之一 → 路由到对应 SKILL
   ├─ 命中重入场景 → 读 state.json → 判定重入点 → 路由
   ├─ 命中问题场景 → 路由到 proposal-skill.md（带渠道标识）
   ├─ 命中其他场景 → 路由到对应 SKILL
   └─ 多个命中 → 询问用户优先级
   ↓
3. 加载对应 SKILL（从 .claude/skills 加载，或 Read 文件）
   ↓
4. 触发对应 SKILL 的"§整体流程"第零步（准入检查）
   ↓
5. 用户确认 → SKILL 执行
```

**步骤 0 的 3 条硬规则：**
- 🔴 **不允许跳过 0.2**：即使用户说"直接开始"，也必须先确认项目资产存在
- 🔴 **0.3 生成过程必须明确告知用户**：不能静默生成，用户必须知道发生了什么
- 🟡 **资产存在时静默加载**：不打扰用户，直接进入步骤 1

### 重入流程判定（state.json）

**state.json 位置：** `.auto-engineering/{STORY-ID}/state.json`（详见 `document-storage-skill.md §2.5`）

**state.json 关键字段：**
```json
{
  "storyId": "STORY-001-BE",
  "currentPhase": "Phase 2",
  "currentStep": "step-4-coding-r2",
  "completedSteps": ["step-1-dr2story", "step-2-story-review", "step-3-testcase", "step-4-coding-r1"],
  "codingRound": 2
}
```

**重入判定算法：**
```
用户输入"从 X 继续" / "重入 Y"
    ↓
读 state.json
    ↓
解析 currentPhase + currentStep
    ↓
根据"上次停在哪个步骤"判定重入点
    ↓
路由到该步骤的对应 SKILL
    ↓
加载 SKILL 并跳过已完成步骤（从 completedSteps 中过滤）
```

### 路由示例

**示例 1：用户说"从 STORY-001 继续"**
- AE-skill 读 `.auto-engineering/STORY-001-BE/state.json` → currentPhase: "Phase 2", currentStep: "step-4-coding-r2"
- 判定：重入到 Phase 2 ④ Coding 第 2 轮
- 路由：`coding-skill.md` + 携带 completedSteps = [step-1/2/3, step-4-r1]（跳过已完成）

**示例 2：用户说"修一下 roleId=0 的特殊语义"**
- AE-skill 关键词匹配 → "修一下" → 路由到 `proposal-skill.md`，渠道标识 = 5（用户反馈）
- AE-skill 加载 `proposal-skill.md` → 引导用户填写 4 段 → 生成 Proposal 文档
- 之后用户说"按 Proposal 走流程" → AE-skill 读 Proposal §4 涉及范围 → 触发 5 步流程

**示例 3：用户说"出 Coding 报告"**
- AE-skill 关键词匹配 → "出 Coding 报告" → 路由到 `coding-report-skill.md`
- AE-skill 加载 `coding-report-skill.md` → 引导用户填 9 章节 → 生成 Coding 报告

### 路由与 SKILL 编排的关系

| 层级 | SKILL | 职责 |
|------|-------|------|
| **第 1 层：统一入口 + 智能路由** | `ae-sdd-skill.md`（本 SKILL）| 分析用户输入，路由到对应节点 SKILL |
| **第 2 层：流程编排** | `ae-sdd-skill.md §整体流程` | 9 步流程怎么走、门禁是什么 |
| **第 3 层：节点 SKILL** | `story-generate / story-review / coding / code-review` 等 | 每个节点怎么执行 |
| **第 4 层：横切依赖** | `proposal-skill / document-storage-skill / agent-orchestration-skill` | 跨节点的统一标准 |

**🔴 关键：** AE-skill 本章节是"统一入口"层，**所有用户输入都先经过这里**再路由。其他 SKILL 仍可被直接调用（不强制走 AE-skill），但推荐走 AE-skill 入口。

---

## 📖 人工审核主动讲解规范（编排级门禁 — 详细模板在各子 SKILL）

> **设计哲学：** 人工审核不是"丢文档给用户自己看"，而是 AI 主动把内容"讲"给用户听。本节只规定**编排级强制门禁**（哪 3 个节点必须讲、怎么确认、未讲怎么办）；具体的讲解模板（讲什么、怎么讲）下沉到各子 SKILL，避免重复维护。

### 强制门禁（AE 编排层关注）

| 审核节点 | 讲解主体 | 子 SKILL 模板位置 | 触发门禁 |
|---------|---------|------------------|---------|
| ① Story Review | 设计决策 | [`story-review-skill.md` §📖 Story 讲解模板](../phase1-design/story-review-skill.md) | 🔍 人工审核点 1 前必须完成 |
| ② Task Generate/Review | 实现拆解 | [`task-generate-skill.md` §📖 Task 讲解模板](../phase2-task/task-generate-skill.md) | 🔍 人工审核点 2 前必须完成 |
| ③ Code Review | 代码实现 | [`coding-skill.md` §📖 Code 讲解模板](../phase2-coding/coding-skill.md) | 🔍 人工审核点 4 前必须完成 |

**反模式（AE 编排层必须拦截的）：**
- ❌ "Story 文档已生成，请审核"（让用户自己读）
- ❌ "请确认进入 Phase 2"（不解释为什么）
- ❌ "Task 文档生成完毕，请确认"（不解释设计意图）
- ❌ "CodeReview 报告已出具，请审阅"（不主动讲重点）
- ❌ 一次抛出一大坨文档后等用户"整体确认"（用户根本没耐心看）

**正确做法：**
- ✅ 进入审核节点前，AI 主动调用对应子 SKILL 的讲解模板，输出 `📖 【XX 讲解 - {STORY-ID}】` 后再询问确认
- ✅ 用户可要求"展开讲某个点" → AI 必须现场补充讲解
- ✅ 用户可要求"快速过" → AI **必须先问**"是否接受降低讲解详细度"，得到明确同意后才能简化（**不得默认简化**）
- ✅ 讲解后用户回复模糊（如"好"/"行"/"可以"） → AI 必须按 ⚠️ 处理，**逐项追问确认**，不得当作 ✅ 通过
- ✅ 讲解 ≠ 一次讲完就结束。每个审核节点都可能多轮往复讲解（用户追问 → AI 补充 → 再确认）

**门禁：**
- 🔴 任一审核节点未做主动讲解 → 视为跳过人工审核 → 禁止进入下一阶段
- 🔴 AI 自行简化讲解 → 视为审核造假，按 [[feedback_report-code-reconciliation]] 整改

---

## 🤖 多 Agent 任务分配机制（单兵作战，多 Agent 并行）

> **场景：一个人负责整个项目，单 Agent 串行做所有事效率太低。** 本节定义如何在 auto-engineering 流程中**把任务拆分给多个子 Agent 并行执行**，让一个人也能驱动一整条生产线。
>
> **核心理念：一个人 = 一支队伍。**
> - **root agent（你正在对话的 AI）** = 项目经理 / 调度员 / 决策者
> - **sub-agent（被派活的子 AI）** = 专项工程师 / 审阅者 / 验证者
> - root agent 负责**拆活、派活、汇总、决策**；sub-agent 负责**按交付标准完成专项任务**
> - 拆分原则：**3+ 独立轨道 / 需独立验证 / 跨多源多工具 / 高错误代价** → 拆；否则单 Agent 串行做

### 何时启用多 Agent

| 触发条件 | 典型场景 | 建议拆分粒度 |
|---------|---------|------------|
| **多个 Story 并行** | 用户有 3+ 个独立 Story 要做 | Story 级别（每个 sub-agent 负责一个 Story 端到端） |
| **单 Story 多 Phase 并行** | 设计/实现/验证可解耦（如 Story 已稳定，只需补全测试用例和 CodeReview） | Phase 级别 |
| **单 Phase 多 Task 并行** | Task 之间无强依赖（如 Task-3 SPI 定义和 Task-5 Controller 实现可并行） | Task 级别 |
| **需独立验证** | 关键决策点（状态机设计、事务边界、错误码）需要"第二意见" | 验证 sub-agent 独立审阅 |
| **跨多源/多工具** | 需要同时跑 DB 测试、HTTP 测试、前端组件测试 | 测试 sub-agent 各自负责一层 |
| **高错误代价** | 涉及资金/数据/权限/线上行为的接口 | 实现 sub-agent + 验证 sub-agent 双跑 |

> **不启用的场景：** 1-2 个 Story、单 Phase 内 Task 强依赖、需要大量上下文连续推理的工作（避免拆分后 sub-agent 拿不到完整上下文反生错）。

### 启动方式

**方式 1：用户主动启用**
> 用户说："启用多 Agent 模式" / "用子 Agent 并行做这几个 Story" / "派个 reviewer 审一下这个设计"

**方式 2：AI 自动提议**
> root agent 检测到当前任务符合"何时启用"表中的任一条件时，**主动向用户提议**：
> ```
> 🤖 【多 Agent 拆分提议】
>
> 检测到当前任务符合多 Agent 启用条件：
> - 触发条件：{具体触发条件}
> - 建议拆分：{具体拆分方案}
> - 预计提速：X 倍（粗略估算）
>
> 是否启用多 Agent 模式？✅ 启用 / ❌ 不启用（继续单 Agent 串行）
> ```

**方式 3：直接调用底层 skill**
> 通过 `mavis-team` skill 创建团队计划（适合复杂多轨道场景），或 `mavis communication send --command spawn` 派单点子任务（适合验证/审阅场景）。

### 子 Agent 角色库（auto-engineering 专用）

> root agent 在派活时，从以下角色库中选择或组合。每个角色都对应一个**专项 prompt 模板**，sub-agent 启动后按模板执行。

#### 角色 1：Story 生成 Agent（`story-writer`）

| 项 | 内容 |
|----|------|
| 输入 | DR 路径 + Story ID + Story 模板（`templates/design/story-template.md`） |
| 输出 | 完整的 Story 主文档（含前端接口契约章节） |
| 标准 | 必须覆盖 ①bis 6 维度 + 模板所有必填章节 |
| 报告格式 | `{STORY-ID}-Story-WriterReport.md`：列出生成的章节 + 关键决策 + 待用户确认点 |
| 适用阶段 | Phase 1 ① |

#### 角色 2：Story Review Agent（`story-reviewer`）

| 项 | 内容 |
|----|------|
| 输入 | Story 文档 + DR 文档 + 约束 + 前端契约要求 |
| 输出 | 缺陷清单（已分类）+ 修复建议 + StoryReviewUpdatePlan 草案（由 root agent 汇总定稿） |
| 标准 | 跑完 Story Review SKILL 完整的 A-E 阶段 + F-Stage 前端契约 Review |
| 报告格式 | `{STORY-ID}-StoryReviewReport.md`：缺陷 ID / 严重度 / 位置 / 修复建议 |
| 适用阶段 | Phase 1 ② |
| 数量建议 | 1-2 个 reviewer 并行（一个看后端，一个看前端契约） |

#### 角色 3：测试用例生成 Agent（`testcase-writer`）

| 项 | 内容 |
|----|------|
| 输入 | Story 文档 + 测试策略模板 + 约束 |
| 输出 | 测试用例文档（含 AC 映射） |
| 标准 | 覆盖 Story 所有 AC + 合规性校验通过 |
| 报告格式 | `{STORY-ID}-TestCase-WriterReport.md`：用例数量 / 覆盖 AC / 跳过的用例 + 原因 |
| 适用阶段 | Phase 1 ③ |

#### 角色 4：Task 生成 Agent（`task-writer`）— 含 CodePlan 汇总职责

| 项 | 内容 |
|----|------|
| 输入 | Story 文档 + 测试用例 + 项目资产 + 约束 |
| 输出 | Task 文档集（含 CodingModel 决策记录 + 任务级 CodePlan）+ Task 实现方案 + **统一版 `{STORY-ID}-CodingPlan.md`** |
| 标准 | 全局 Task Review 通过（TR-1~TR-7）+ 统一版 CodingPlan 14 条门禁全过 + 用户明确确认 |
| 报告格式 | `{STORY-ID}-Task-WriterReport.md`：Task 数量 / 依赖关系图 / 风险 Task 标记 / 统一版 CodePlan 摘要 |
| 适用阶段 | Phase 2 ④ + ④ter（调 `CodingSkill.Plan(task-level)` 生成 CodingModel 决策记录 + 任务级 CodePlan）+ ⑥（汇总统一版 CodePlan）|
| 子流程 | 1. ④ 按 Task 顺序生成文档，每个 Task 撰写时调用 **`CodingSkill.Plan(task-level)`**（不是直接引用章节号），将返回的 CodingModel 决策记录 + 任务级 CodePlan 嵌入 Task 文档<br>2. ④bis 单 Task 一致性校验（TC-1~TC-7）<br>3. ⑤ 生成 Task 0 + 全局 Task Review（TR-1~TR-7）<br>4. ⑥ 汇总所有 Task 的任务级 CodePlan 为统一版 `{STORY-ID}-CodingPlan.md`，套用 Coding-SKILL §④bis 16 节模板 + 14 条门禁<br>5. 用户审核统一版 CodePlan → 通过后触发 `CodingSkill.Execute` |

#### 角色 5：原 `plan-writer` 已合并入角色 4

> **变更（2026-06-05）：** Plan 写在 Task 内（任务级 CodePlan），不再有独立的 plan-writer 角色。统一版 CodePlan 汇总也是 task-writer 的职责（角色 4 的子流程 ④ter + ⑥）。
> 原 ④bis "CodingPlan 输出" 节点降级为 task-writer 的子动作（"⑥ 汇总统一版 CodePlan"）。

#### 角色 6：Coding Agent（`coder`）— 吃统一版 CodePlan

| 项 | 内容 |
|----|------|
| 输入 | **统一版 `{STORY-ID}-CodingPlan.md`**（用户已确认）+ Story 文档 + 项目资产 + 约束 + 工作目录 |
| 输出 | 可编译、可测试的代码 + 单元测试 |
| 标准 | 严格按 CodePlan 实施（临时偏离需用户确认）+ 每步可编译 + 测试真实性（见 `🔴 测试真实性强制规范`） |
| 报告格式 | `{STORY-ID}-Coding-CoderReport-r{M}.md`：本轮变更文件清单 + 编译/测试结果 + 已知问题 |
| 适用阶段 | Phase 2 ⑤ |
| 数量建议 | 1 个 Story = 1 个 coder（避免多 coder 写同一文件冲突） |

#### 角色 7：CodeReview Agent（`code-reviewer`）

| 项 | 内容 |
|----|------|
| 输入 | Coding 报告 + 测试报告 + Story + 实际代码 + 项目资产 |
| 输出 | CodeReview 报告（含 §2 六阶段评审 + §3 合理性判定 + §第四步 bis UpdatePlan + §第七步 7 道闸） |
| 标准 | 见 [`code-review-skill.md` §多 Agent 评审编排](../phase3-review/code-review-skill.md)（含 prompt 模板 + 多 Reviewer 交叉对比模式） |
| 报告格式 | `{STORY-ID}-CodeReview-v{N}-r{M}.md`：架构师级审阅报告 |
| 适用阶段 | Phase 3 ⑦ |
| 数量建议 | 1-2 个 reviewer 并行（一个看业务实现，一个看架构/规范） |
| **🔴 【2026-06-06 重构】** | **6 大闸门（⑥bis 一致性 / ⑦bis 对称性 / 全文档回扫 / 禁裸 ✅ / 报告-代码对账 / 产出物对账 / 真实 DB-HTTP 覆盖）已迁出到 `code-review-skill.md §第七步`，本角色仅作 AE 编排层指针，详见该 SKILL。** |

#### 角色 8：测试验证 Agent（`test-verifier` 🔴 强制）

| 项 | 内容 |
|----|------|
| 输入 | 测试报告 + 测试代码 + Story AC |
| 输出 | 测试真实性核查报告 |
| 标准 | 见 `🔴 测试真实性强制规范` 8 类禁止手段 + 5 条保障要求 |
| 报告格式 | `{STORY-ID}-TestVerification-Report.md`：8 类手段扫描结果 + 关键测试代码摘录 + AC 覆盖率 |
| 适用阶段 | Phase 3 ⑥.10（⑥ 完成判定的硬前置） |
| **关键角色** | **这是 ⑥.10 强制要求的独立验证位——sub-agent 不依赖主 agent 的报告，独立跑一遍测试** |

### 任务分配模式（典型场景）

#### 模式 A：多 Story 并行（最高频的单兵场景）

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ 检测到：3 个 Story 待做（STORY-001/002/003）      │
│ 决策：每个 Story 派一个 sub-agent 端到端负责       │
│                                                 │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│   │ sub-A    │  │ sub-B    │  │ sub-C    │     │
│   │ STORY-001│  │ STORY-002│  │ STORY-003│     │
│   │ 端到端   │  │ 端到端   │  │ 端到端   │     │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘     │
│        │             │             │            │
│        └─────────────┼─────────────┘            │
│                      ▼                          │
│              root agent 汇总决策                 │
│              （接受 / 重试 / 升级用户）           │
└─────────────────────────────────────────────────┘
```

**使用：** 直接调用 `mavis-team` skill 创建 3 轨团队计划。

**root agent 职责：**
1. 创建 3 个 sub-agent 任务描述
2. 等待全部完成（或超时）
3. 收集 3 份 `*-WriterReport.md` / `*-ReviewerReport.md` 等
4. 跨 Story 一致性检查（共用 DR 约束、字段基线、错误码体系）
5. 接受 / 重试 / 升级用户

#### 模式 B：单 Story 多阶段并行（设计/实现/验证可解耦时）

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ 检测到：Story 已稳定，需补全测试用例和 CodeReview  │
│ 决策：派 2 个 sub-agent 并行处理                  │
│                                                 │
│   ┌──────────────────┐  ┌──────────────────┐   │
│   │ sub-X            │  │ sub-Y            │   │
│   │ testcase-writer  │  │ code-reviewer    │   │
│   │ 生成测试用例      │  │ 出具 CodeReview  │   │
│   └────────┬─────────┘  └────────┬─────────┘   │
│            │                     │              │
│            └──────────┬──────────┘              │
│                       ▼                         │
│              root agent 汇总                    │
└─────────────────────────────────────────────────┘
```

#### 模式 C：单 Story 双 Reviewer 独立审阅（关键决策点）

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ 检测到：状态机 / 事务边界 / 错误码 等关键决策点   │
│ 决策：派 2 个 reviewer 独立审，最后交叉对比        │
│                                                 │
│   ┌──────────────────┐  ┌──────────────────┐   │
│   │ reviewer-BE      │  │ reviewer-FE      │   │
│   │ 看后端实现        │  │ 看前端契约 / 联调 │   │
│   └────────┬─────────┘  └────────┬─────────┘   │
│            │                     │              │
│            └──────────┬──────────┘              │
│                       ▼                         │
│           root agent 交叉对比                    │
│           （一致 = 接受；不一致 = 升级用户）      │
└─────────────────────────────────────────────────┘
```

#### 模式 D：测试真实性独立验证（🔴 强制，⑥.10）

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ ⑥.10 完成判定前必须派 test-verifier sub-agent    │
│ 独立跑一遍测试，不依赖主 agent 的报告               │
│                                                 │
│   ┌────────────────────────────────────┐       │
│   │ sub-V (test-verifier)              │       │
│   │ 1. 拉取测试代码                     │       │
│   │ 2. 扫描 8 类禁止伪造手段              │       │
│   │ 3. 独立跑 mvn test                  │       │
│   │ 4. 对照 Story AC 验证覆盖率          │       │
│   │ 5. 出具测试真实性核查报告             │       │
│   └────────────┬───────────────────────┘       │
│                │                                │
│                ▼                                │
│      root agent 决策                             │
│      （报告作废 + 返工 / 接受）                   │
└─────────────────────────────────────────────────┘
```

> **🔴 这是 ⑥.10 硬门禁的兜底：即使主 agent 自称"测试通过"，test-verifier 独立验证不通过 = 不通过。** 防止"AI 给自己打钩"。

### 任务分配协议（root agent → sub-agent）

#### 任务描述模板

root agent 派活时，必须用以下结构化 prompt（不要模糊说"帮我做一下"）：

```yaml
# 任务分配卡
agent_role: {角色名}  # 如 story-writer / code-reviewer
story_id: STORY-XXX-BE
task_id: {本次任务唯一 ID}
priority: P0 / P1 / P2

input:
  - {文件路径 1}
  - {文件路径 2}
  - {约束/模板路径}

output:
  deliverable: {产出物文件路径}
  report: {报告文件路径}

standards:
  - {本任务必须满足的标准 1}
  - {本任务必须满足的标准 2}
  - {门禁/红线}

context:
  - {必要的背景信息，sub-agent 没有 root 的全上下文}

deadline: {最长执行时间}

report_back:
  channel: mavis communication
  target: {root session id}
  format: {报告模板路径}
```

#### 报告回传协议

sub-agent 完成后，必须输出**结构化报告**：

```markdown
# Sub-Agent Report - {task_id}

**Agent 角色：** {role}
**Story：** {STORY-ID}
**执行时间：** {start} ~ {end}
**结果：** ✅ 完成 / ⚠️ 完成但有风险 / 🔴 失败

## 完成情况
- [x] {交付物 1}
- [x] {交付物 2}
- [ ] {未完成项}（原因：{...}）

## 关键决策
- {决策 1}：{原因}
- {决策 2}：{原因}

## 风险点
- 风险 1：{...} → 建议：{...}

## 待 root agent 决策
- {事项 1}
- {事项 2}
```

### 状态共享（state.json 多 Agent 版本）

> 多 Agent 模式下，`.auto-engineering/{STORY-ID}/state.json` 需要扩展。

```json
{
  "storyId": "STORY-010-BE",
  "storyVersion": "v1",
  "codingRound": "r1",
  "currentPhase": "Phase 1",
  "currentStep": "step-3-testcase",
  "completedSteps": ["step-1-dr2story", "step-2-story-review"],
  "activeAgents": [
    {
      "agentId": "sub-A-001",
      "role": "story-writer",
      "sessionId": "mvs_xxx",
      "status": "running",
      "startedAt": "2026-06-03T10:00:00",
      "currentSubTask": "生成 §前端接口契约"
    },
    {
      "agentId": "sub-B-001",
      "role": "story-reviewer",
      "sessionId": "mvs_yyy",
      "status": "completed",
      "startedAt": "...",
      "completedAt": "...",
      "report": "design/story/be/STORY-010-BE-StoryReviewReport.md"
    }
  ],
  "agentReports": [
    {
      "agentId": "sub-A-001",
      "reportPath": "...",
      "summary": "..."
    }
  ],
  "pendingOutputs": {...},
  "lastUpdated": "2026-06-03T11:00:00"
}
```

**root agent 职责：**
- 启动 sub-agent 时 → 写入 `activeAgents[]`
- sub-agent 完成时 → 移入 `agentReports[]` + 更新 `activeAgents[].status`
- 任一 sub-agent 超时或失败 → 决定重试 / 升级用户

### 协调与汇总

#### 汇总流程（root agent 必须执行的步骤）

```
1. 收集所有 sub-agent 报告
   ↓
2. 跨报告一致性检查
   - 多个 Story 是否使用一致的错误码体系？
   - 多个 sub-agent 的设计决策是否冲突？
   - 测试数据来源是否与 Story 一致？
   ↓
3. 门禁扫描
   - sub-agent 是否达成任务卡上的 standards？
   - 是否有 sub-agent 标注 "未完成"？
   ↓
4. 决策
   - 全部达成 + 一致 → 接受，进入下一阶段
   - 部分未达成 → 重试该 sub-agent
   - 冲突 / 不一致 → 升级用户
   ↓
5. 更新 state.json + state.phase
```

#### 冲突处理规则

| 冲突类型 | 处置 |
|---------|------|
| sub-agent 报告间数据不一致 | root agent 必须**自己读双方产出物**判断对错，**不得默认信任何一方** |
| sub-agent 自称完成但门禁未过 | root agent 重新跑门禁，**不轻信 sub-agent 自评** |
| sub-agent 失败 / 超时 | root agent 决定：重试（同一 sub-agent）/ 换 sub-agent / 退回单 Agent / 升级用户 |
| 多 sub-agent 写同一文件 | **禁止！** root agent 派活时必须按文件拆分，**不允许并发写同一文件** |

### 门禁与规则（强制）

| # | 规则 | 违反处置 |
|---|------|---------|
| 1 | sub-agent 必须输出结构化报告，不允许"做完了"这种模糊回复 | 视为任务未完成 |
| 2 | root agent 派活必须用任务卡模板（input/output/standards/deadline 缺一不可） | 视为派活不完整 |
| 3 | root agent 不得默认信任 sub-agent 报告，必须独立交叉验证关键产出物 | 视为汇总失败 |
| 4 | sub-agent 不得直接对话用户（除非任务卡明确授权） | 视为越权 |
| 5 | 多个 sub-agent 不得并发写同一文件 / 同一目录 | 立即终止冲突 sub-agent |
| 6 | sub-agent 失败时不得静默忽略，必须更新 state.json + 通知 root | 视为任务丢失 |
| 7 | root agent 决定"单 Agent 继续"时，必须向用户说明理由（"多 Agent 拆分代价大于收益"） | 视为擅自降级 |
| 8 | ⑥.10 测试真实性必须由 test-verifier sub-agent 独立验证，**主 agent 不得自我验证** | 视为违反 ⑥.10 门禁 |

### 多 Agent 模式 vs 单 Agent 模式对比

| 维度 | 单 Agent 模式（默认） | 多 Agent 模式（启用后） |
|------|---------------------|---------------------|
| 适用场景 | 1-2 Story / 强依赖 / 简单任务 | 3+ Story / 弱依赖 / 高错误代价 / 需独立验证 |
| 速度 | 串行，慢 | 并行，快 2-3 倍 |
| 上下文连续性 | ✅ 强（同一 agent 全程） | ⚠️ 弱（sub-agent 需靠任务卡传递上下文） |
| 决策一致性 | ✅ 强 | ⚠️ 中（root agent 需做交叉验证） |
| 成本 | 低 | 中（每个 sub-agent 独立上下文） |
| 适用阶段 | 全阶段通用 | 各阶段子任务可并行 |
| 推荐使用 | 默认 | **明确场景下启用** |

> **🔴 默认单 Agent，遇到符合"何时启用"表的场景时 AI 必须主动提议。** 但用户可随时说"继续单 Agent 串行做"拒绝。

### 与 mavis-team skill 的衔接

> 本节是 auto-engineering SKILL 内部的"任务分配机制"**规范层**——定义何时派活、派什么活、如何汇总。**具体执行层**通过 `mavis-team` skill（复杂多轨道）或 `mavis communication send --command spawn`（单点验证）实现。
>
> 详细执行流程参考 `mavis-team` skill 文档；本节是 auto-engineering 场景下的"业务规则"层。

### 与现有 SKILL 章节的衔接

| auto-engineering 章节 | 多 Agent 应用 |
|---------------------|-------------|
| Phase 1 ① 生成 Story | 可派 `story-writer` sub-agent |
| Phase 1 ② Story Review | 可派 2 个 `story-reviewer`（BE + FE） |
| Phase 1 ③ 测试用例 | 可派 `testcase-writer` sub-agent |
| Phase 2 ④ Task 生成 | 可派 `task-writer` sub-agent |
| Phase 2 ④bis CodingPlan | 可派 `plan-writer` sub-agent |
| Phase 2 ⑤ Coding | 建议**单 coder**（避免文件冲突） |
| Phase 3 ⑥.10 测试真实性 | **🔴 强制派 `test-verifier` sub-agent 独立验证** |
| Phase 3 ⑦ CodeReview | 派 `code-reviewer` sub-agent（**走 `code-review-skill.md` §多 Agent 评审编排** — 6 阶段 + 7 道闸） |

---

## 流程状态跟踪与再启动

### 状态跟踪（强制）

AI 在本 SKILL 运行期间，**必须持续维护以下状态**，每个 Story 独立存储在 `.auto-engineering/{STORY-ID}/state.json`（工程目录根路径）：

```
.auto-engineering/
├── STORY-010-BE/
│   └── state.json
├── STORY-011-BE/
│   └── state.json
└── ...
```

```json
{
  "storyId": "STORY-010-BE",
  "storyVersion": "v1",
  "codingRound": "r1",
  "currentPhase": "Phase 1",
  "currentStep": "step-3-testcase",
  "completedSteps": ["step-1-dr2story", "step-2-story-review"],
  "pendingOutputs": {
    "storyDoc": "design/story/be/STORY-010-BE.md",
    "testcase": "design/testcase/be/STORY-010-BE/STORY-010-BE-testcase.md"
  },
  "lastUpdated": "2026-05-26T10:00:00"
}
```

> **storyVersion 变更时机：** Story 主文档发生变更（内容修改、补充说明合入）后累加。用于报告文件命名和 DR-Story 一致性追踪。
> **codingRound 变更时机：** 每次开始新一轮 Coding 实现（不论是因为缺陷修复、增量补充还是重构）前累加。每一轮 Coding 独立出具该轮的 CodeReview 报告。

**每次进入新步骤前**：必须读取 `.auto-engineering/{STORY-ID}/state.json`，确认当前步骤和已完成步骤。
**每次完成步骤后**：必须更新 `.auto-engineering/{STORY-ID}/state.json`，记录完成的步骤和产出物路径。
**版本号更新时机**：
  - `storyVersion` 在 Story 主文档发生内容变更后累加
  - `codingRound` 在开始新一轮 Coding（不论任务类型）前累加
**Story ID 来源**：从用户输入或 state.json 中读取，不可混用不同 Story 的状态文件。

---

### 流程脱离与再启动

#### 场景一：用户偏离流程（说其他话题）

**判定：** 用户消息与当前 Story/Task/Coding 无关，且不是流程节点询问。

**AI 动作（强制）：**
1. 简短回应用户话题（不阻塞）
2. 不更新 `.auto-engineering/{STORY-ID}/state.json`（保持当前步骤不变）
3. 下次对话时，若用户未明确说"继续/回到流程"，则询问："当前流程停在 [{当前步骤}]，是否继续？"

#### 场景二：用户明确说要继续/回到流程

**触发条件：** 用户说"继续"、"回到流程"、"继续上次"、"接着来"等。

**AI 动作（强制）：**
1. 读取 `.auto-engineering/{STORY-ID}/state.json`（STORY-ID 从上下文或用户输入获取）
2. 定位当前步骤和已完成步骤
3. 输出状态摘要：
   ```
   【流程已恢复】
   Story ID：{STORY-ID}
   当前阶段：Phase 1 - 设计阶段
   当前步骤：③ 生成测试用例
   已完成：① Story 生成、② Story Review
   待完成：③bis 用例校验 → 🔍 人工审核 → Phase 2...
   ```
4. 从当前步骤继续执行

#### 场景三：用户说要重启/重新开始

**触发条件：** 用户明确说"重新开始"、"从头来过"。

**AI 动作（强制）：**
1. 确认用户意图（询问是否确认放弃当前进度）
2. 用户确认后，删除 `.auto-engineering/{STORY-ID}/state.json`
3. 从 `Phase 1 ①` 重新开始

#### 场景四：用户切换到其他 Story

**触发条件：** 用户提供了新的 Story ID 或 DR 路径。

**AI 动作（强制）：**
1. 读取当前 Story 的 `.auto-engineering/{当前STORY-ID}/state.json`，确认是否有未完成的 Story
2. 询问用户：是否要暂停当前 Story，先处理新的？
3. 用户确认后，切换到新 Story ID，读取或创建 `.auto-engineering/{新STORY-ID}/state.json`
4. 两个 Story 的状态文件互不影响，可随时切换回来

---

### 再启动判定规则

| 用户意图 | AI 动作 |
|---------|---------|
| 继续上次流程（继续/接着做/接着来） | 读取 `.auto-engineering/{STORY-ID}/state.json`，恢复到当前步骤 |
| 从某个步骤继续（从xx开始/重新做xx） | 读取 `.auto-engineering/{STORY-ID}/state.json`，从指定步骤恢复（不可倒退已完成的关键门禁：Phase 1 完成确认、Phase 2 实现方案预确认、Task 文档完成确认、Phase 3 完成判定、⑥bis 全切面一致性核查闸、CodeReview 报告出具、⑦bis 全链路对称性核查闸） |
| 放弃当前，重新开始 | 确认后删除 `.auto-engineering/{STORY-ID}/state.json`，从 Phase 1 ① 重启 |
| 切换到另一个 Story | 保留当前 Story 的 state.json，切换到新 Story 的 `.auto-engineering/{新STORY-ID}/state.json` |
| 偏离流程（其他话题） | 简短回应，不更新 state，保持步骤不变 |

---

## 整体流程

```
【必须严格执行以下顺序，不可跳过任何步骤】

输入：DR 文档路径 + Story ID + 工作目录
                │
Phase 1 ────────┤ 设计阶段（必须完成）
                │
                ├── ① 执行 DRtoStory SKILL → 生成 Story
                │
                ├── ①bis 🔴 前端视角接口审视（强制）→ 补全前端接口契约章节
                │
                ├── ② 执行 Story Review SKILL → 挖掘/判定 → StoryReviewUpdatePlan → 按 Plan 修复循环 → Story 稳定
                │
                ├── ③ 生成测试用例
                │
                ├── ③bis 用例合规性校验（必须通过）
                │
                └── 🔍 人工审核：确认设计阶段完成（必须通过）

Phase 2 ────────┤ 实现阶段（必须完成）
                │
                ├── 🔍 人工审核：实现方案预确认（必须通过）
                │     在生成 Task 文档之前，对核心业务/接口/分层/并发/异常达成共识
                │
                ├── ④ 执行 Task Generate SKILL → 生成 Task 文档 → 全局 Task Review（结合约束+Story+测试用例）→ Task 实现方案
                │
                ├── ④bis 🔴 CodingPlan 输出（可执行层）→ 文件级实现顺序 + 关键代码骨架 + 验证点
                │
                ├── 🔍 人工审核 2.5【🆕 2026-06-10】：CodingPlan 评审
                │     复核 16 章节 + 14 条门禁 + CodingModel 决策 + 风险 Task
                │     用户明确确认后 → ⑤ Coding
                │
                └── 🔍 人工审核：确认 Task 文档 + 实现方案 + CodingPlan 完成（必须通过）

                ├── ⑤ 执行 Coding SKILL
                │     每个 Task 开始前必须呈现实现方案并获用户确认
                │     用户确认后才能开始写代码
                │
                └── ~~🔍 人工审核：确认编码阶段完成~~（🗑️ 2026-06-10 删除，合并到审核点 4）

Phase 3 ────────┤ 验证阶段（必须完成）
                │
                ├── ⑥ 完成判定（全部条件通过）
                │
                ├── ⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 硬前置，每轮强制）
                │
                ├── ⑦ 出具 CodeReview 报告（必须完成）
                │
                ├── ⑦bis 全链路对称性核查闸（🔴 流程收尾强制：DR-Story-Task-实现-测试用例 五层一一对应）
                │
                └── 🔍 人工审核 4：CodeReview 阶段完成确认（🆕 2026-06-10 改名，原"验证阶段"）

                └── ⑧ 完成 ✅
```

---

## 输入

```
必须提供以下三项信息，缺一不可：
1. DR 文档路径
2. 要实现的 Story ID（或"全部"）
3. 工作目录（各工程所在的磁盘根路径）
```

---

## Phase 1：设计阶段（必须完成，不可跳过）

### ① 生成 Story

**触发：** [DRtoStory SKILL]（已有）

**输入：** DR 文档路径 + Story ID
**输出：** Story 主文档
**必须参考模板：** `templates/design/be-story-template.md`

**跳过条件：** Story 文档已存在且状态非 Draft

---

### ①bis 前端视角接口审视（🔴 强制 — 6 维度审查清单已下沉）

> **📍 详细 6 维度审查清单已下沉：** 原 AE-skill 中 ①bis 的 6 维度（接口契约完整性/调用流程/状态展示/错误码/边界场景/联调支持）+ 产出物模板（嵌入 Story 文档的"前端接口契约"章节）+ 🔴 门禁（共 6 条），已统一存放在 [`story-review-skill.md` §📋 ①bis 前端视角接口审视 — 6 维度审查清单](../phase1-design/story-review-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 3 个门禁：**
> - ✅ ① 生成 Story 完成后、② Story Review 之前，必须完成 ①bis 6 维度审视
> - ✅ Story 文档"前端接口契约"章节已嵌入（含至少 1 个完整请求+响应示例）
> - ✅ Story Review SKILL 的"零、Story 准入检查"已包含"前端接口契约章节完整性"勾选项
> - 若前端约束规范 `constraints/frontend.md` 不存在，本步骤降级执行但需在 Story 中标注"**前端约束待补充**"

---

### ② Story Review

**触发：** [Story Review SKILL](../phase1-design/story-review-skill.md)

**输入：** Story 文档 + DR 文档 + `templates/design/be-story-template.md` + 约束
**输出：** 稳定的 Story 文档（无新增可修改项）+ `{STORY-ID}-StoryReviewUpdatePlan-r{轮次}.md`（每轮有确认缺陷时必出）

**内部循环：**
```
挖掘 → 判定 → 生成 StoryReviewUpdatePlan → 按 Plan 修复 → 再挖掘 → ... → 退出
```

> **📍 详细 Plan-first 规则已下沉：** `StoryReviewUpdatePlan` 的内容、模板、字段链路修订计划和出闸条件见 [`story-review-skill.md` §Plan-first 更新原则](../phase1-design/story-review-skill.md) 与 [`templates/design/be-story-review-update-plan-template.md`](../../templates/design/be-story-review-update-plan-template.md)。AE 编排层只关注：有确认缺陷时必须先有 Plan；Story Update 必须按 Plan 执行；Plan 外业务语义修改视为无效更新。

**🔴 必须新增 F-Stage（前端契约 Review）：**
> ①bis 已要求 Story 包含"前端接口契约章节"。② Story Review **必须** 把这一章节作为 Review 维度之一，增加 **F-Stage：前端契约 Review**。F-Stage 至少包含以下检查项：
> 1. 6 个维度是否都覆盖了（接口契约完整性 / 调用流程 / 状态展示 / 错误码 / 边界场景 / 联调支持）
> 2. 字段是否同时满足后端实现和前端对接的需求（不偏废任何一方）
> 3. 错误码前端处理建议是否可执行（不是"toast 一下"这种空话）
> 4. 状态流转 UI 展示建议是否可执行（颜色/图标有具体值）
> 5. 时间/金额/ID 字段的格式与项目前端基线是否一致
> 6. 联调信息（mock / 环境 / 时间窗口）是否齐全
>
> F-Stage 未通过 → Story Review 视为不完整，禁止退出循环。

**人工节点：** 每完成 3 轮完整的 A-E 阶段循环（每轮 = 挖掘缺陷 → 判定 → 修复 → 确认无新增），自动暂停并询问用户"是否继续 Story Review？"

---

### ③ 生成测试用例

**触发：** Story Review SKILL 第七步退出后自动触发，或开发者说"生成测试用例"

**触发 SKILL：** [TestCase Generate SKILL](../phase1-design/testcase-generate-skill.md)

**输入：**
- Story 主文档
- DR 文档
- `strategies/be-testcase-strategy.md`（测试策略模板）
- `templates/testcase/be-testcase-template.md`（用例模板）
- `constraints/testing.md`（测试约束）

**输出：** 测试用例文档 `design/testcase/be/{STORY-ID}/2c-im-testcase-{STORY-ID}-{标题}.md`

**SKILL 内部流程：**
1. 读取 Story + 策略模板 + 用例模板 + 约束
2. 识别 Story 类型，选择覆盖策略（状态机/CRUD/回调/定时任务/集成）
3. 按三层模型生成用例（类型策略 + 通用维度 + 测试分层）
4. 合规性校验（全部检查项通过才可退出）
5. AC 完整性检查（如有缺失需反馈到 Supplement）
6. 输出测试用例文档

**禁止：**
- 不参考策略模板直接生成 ❌
- 跳过合规性校验 ❌
- 只覆盖 AC，不做全场景覆盖 ❌

---

### ③bis 业务逻辑汇总输出

**触发时机：** Story Review 循环退出后，测试用例生成完成后，进入人工审核之前

**前置条件：** Story Review 的 C8 数据视角总览已完成（含 C8-4 字段链路映射）；①bis 前端视角接口审视已完成

**输出模板：** `templates/design/be-story-review-logic-summary-template.md`

**产出物：** `{STORY-ID}-业务逻辑汇总.md`

**填写规则：** AI 必须基于 Story 文档和 C8 数据视角总览（C8-1~C8-4），填写模板中的所有章节。禁止留空或填"待补充"（如有不确定项，必须标注"需人工确认"）。

**🔴 必须新增"前端对接维度"章节：**
> 业务逻辑汇总表除业务逻辑外，必须包含"**前端对接一览**"章节，汇总从 ①bis 前端接口契约中提取的要点：
> 1. 对外接口清单（接口名 / Method / URL / 调用方）
> 2. 关键字段约定（时间格式、ID 类型、金额单位）
> 3. 状态枚举与 UI 展示映射
> 4. 错误码前端处理建议汇总
> 5. 联调关键信息（mock 平台 / 测试环境 / 联系人 / 时间窗口）
>
> 此章节是**人工审核点 1** 时业务方 / 前端负责人快速确认前端对接可行性的依据。

**人工审核前必须完成此汇总：** 审核者基于此表快速判断业务逻辑完整性，跳过此步骤不得进入人工审核。

---

### 🔍 人工审核点 1：设计阶段完成确认

**触发时机：** 测试用例生成完成后，进入 Phase 2 之前

#### 📖 AI 主动讲解（Story 故事，🔴 强制）

> **本节点禁止直接问"请确认"。** AI 必须先用讲故事的笔法，主动向用户讲清楚本 Story 的业务背景、核心流程、关键设计决策、AC 故事和已识别风险点（详细规范见上文 `📖 人工审核主动讲解规范` 章节）。讲完后才能进入下方的"审核内容 → 询问用户"环节。

讲解模板参考上文 `① StoryReview 阶段 —— 讲"业务设计故事"` 的输出模板，**必须覆盖**：
1. 业务背景与典型用户故事
2. 核心业务流程（从前台到后端）
3. 关键设计决策（状态机/接口/数据模型）的"为什么"
4. 每个 AC 背后的用户场景
5. 已识别风险点与应对

**讲解结束后的询问：**

**审核内容：**
- [ ] Story 文档已稳定（无新增可修改项）
- [ ] StoryReviewUpdatePlan 已按轮次生成并执行 / 或本轮无确认缺陷不需要生成
- [ ] 测试用例已生成且覆盖所有 AC
- [ ] Story 补充说明文档已完善（如有）
- [ ] 设计阶段产出物齐全
- [ ] **🔴 AI 已完成 Story 故事讲解（业务背景/核心流程/关键决策/AC/风险）**
- [ ] **🔴 ①bis 前端接口契约章节已生成且 6 个维度门禁全部通过**
- [ ] **🔴 业务逻辑汇总的"前端对接一览"章节已生成**

**询问用户：** "Phase 1 设计阶段已完成，产出物包括：Story 文档、Story 补充说明（如有）、StoryReviewUpdatePlan（如有确认缺陷）、测试用例文档。是否确认进入 Phase 2 实现阶段？"

**用户选项：**
- ✅ 确认进入 Phase 2
- ⚠️ 需要修改（说明修改内容，返回对应步骤）
- ❌ 暂停流程

---

## Phase 2：实现阶段（必须完成，不可跳过）

### 🔍 人工审核点 1.5：实现方案预确认（在生成 Task 之前）

**触发时机：** Phase 1 完成后，进入 Phase 2 之前

**目的：** 在写 Task 文档之前，先和用户对齐实现思路，避免 Task 文档写完后返工。

**AI 必须向用户呈现的内容：**

```
【实现方案预确认】

请确认以下实现思路：

一、核心业务理解对齐
- 本次 Story 的核心业务是什么？
- 核心状态机有哪些状态？流转规则是什么？
- 涉及哪些 DB 操作？（增/删/改/查）

二、接口与依赖确认
- 对外暴露的 SPI 接口有哪些？调用方是谁？
- 依赖哪些下游 Story/外部服务？
- 需要复用的现有代码在哪里？

三、分层实现思路
- Domain 层：聚合根/实体/领域服务/Repository 接口
- Application 层：AppService 的核心方法
- Infrastructure 层：Repository 实现/外部服务封装
- Interfaces 层：Controller/SPI 接口

四、并发与事务策略
- 是否有并发场景？并发控制方案？
- 事务边界怎么划？
- 幂等性如何保障？

五、异常处理思路
- 核心异常有哪些？错误码体系？
- 事务外操作失败怎么处理？
```

**用户选项：**
- ✅ 确认实现思路，无误
- ⚠️ 有修改意见（说明修改内容，AI 记录后继续）
- ❌ 暂停流程

> **强制要求：** AI 必须等待用户确认后才能进入生成 Task 文档环节。禁止跳过此节点直接生成 Task。如果用户未回复，最多重试 3 次（每次发送提醒），3 次后仍无回复则暂停，等待用户明确指示。

---

### 🔍 人工审核点 2：Task 文档 + 实现方案完成确认

**触发时机：** Task 文档生成完成 + Task 实现方案输出后，进入 Coding 之前

#### 📖 AI 主动讲解（Task 故事，🔴 强制）

> **本节点禁止"列完 Task 清单就问请确认"。** AI 必须先讲清楚"为什么这样拆 Task"、"依赖链路长什么样"、"每类 Task 风险点在哪"，再走下面的逐文件核对。详细规范见上文 `📖 人工审核主动讲解规范` 章节。

讲解模板参考上文 `② TaskReview 阶段 —— 讲"实现拆解故事"` 的输出模板，**必须覆盖**：
1. **拆 Task 的故事**：为什么拆成 N 个 Task？拆分依据（DDD 聚合根/分层/依赖）？
2. **依赖链路故事**：Task 之间为什么这么排？谁必须先做？
3. **DB 变更故事**：表结构、索引、跨服务一致性
4. **事务边界故事**：每个 Task 的事务范围、事务外操作
5. **风险 Task 标记**：哪些 Task 风险高？为什么？

**讲解 + 逐文件核对的协作流程：**
```
进入本审核点
    │
    ├── 1. AI 先讲一遍"全 Task 拆解故事"（上面的 5 个维度）
    │
    ├── 2. 然后进入逐文件核对循环（见下文）
    │     每核对一个文件，AI 先讲"本 Task 故事"（用上文 ② 的逐 Task 模板）
    │     再读出文件全文，再请用户审阅
    │
    └── 3. 全部文件核对完，最后核对实现方案
```

**审核内容（AI 自检）：**
- [ ] Task 0（环境准备）已生成
- [ ] 所有实现 Task 已生成且包含核心代码示例
- [ ] Task 依赖关系清晰
- [ ] Task 检查项完整
- [ ] **{STORY-ID}-Task实现方案.md 已生成**（汇总所有 Task 的实现要点、依赖关系、DB 变更、事务边界）
- [ ] **🔴 AI 已完成 Task 故事讲解（拆 Task 故事/依赖链路/DB 变更/事务边界/风险 Task）**
- [ ] **🔴 每个 Task 文件核对前 AI 已讲完"本 Task 故事"**

#### 🔴 强制门禁：人工审核必须逐文件自上而下核对（用户原话："我们从上到下一点一点一个文件一个文件的过"）

> **AI 不得把全部 Task 文档一次性抛出后等用户"整体确认"。** 用户作为人工审核者，没有义务把 AI 吐出来的一大坨文档自己通读一遍再给一个总评——这等于把审核责任转嫁给用户，违背"人工审核点"的设计本意。
>
> **正确的做法是：AI 主动带用户一个文件一个文件从上到下过，每个文件单独确认后才进入下一个。**

**核对流程（强制，不可整体确认）：**

```
进入人工审核点 2
    │
    ├── 1. AI 列出 Task 文件清单（按文件名字典序排序，天然自上而下）
    │     例如：task-0-env.md → task-1-xxx.md → task-2-yyy.md → ... → {STORY-ID}-Task实现方案.md
    │     （实现方案放在最后核对，因为它依赖前面所有 Task 的决策）
    │
    ├── 2. 对每个 Task 文件，循环执行（用户必须逐文件走完才算完成）：
    │     │
    │     ├── a. AI 完整读出本文件内容（一次性呈现，不省略不摘要）
    │     │     （用户不需要自己打开文件，AI 必须主动呈现）
    │     │
    │     ├── b. AI 主动指出本文件"请用户重点确认"的位置
    │     │     （不是让用户自己找要点）：
    │     │     - 关键决策点（多方案择一）
    │     │     - 与上一 Task 的依赖点
    │     │     - DB 变更 / 事务边界 / 错误码等高风险设计
    │     │     - 不可逆 / 难回退的设计选择
    │     │     - 与 Story AC 的对应点
    │     │
    │     ├── c. 询问用户对本文件的意见（每个文件独立确认）：
    │     │     - ✅ 通过（进入下一个文件）
    │     │     - ⚠️ 需要修改（AI 记录后本文件重审，不进入下一文件）
    │     │     - ⏸️ 暂停（写入 state.json，下次"继续"从本文件继续）
    │     │     - ❌ 终止（清空本轮审核，回 Task Generate 阶段）
    │     │
    │     └── d. 文件被 ⚠️ 修改后，**重走该文件核对流程**，直到 ✅ 才进入下一文件
    │
    ├── 3. 全部 Task 文件 ✅ 通过后，最后核对 {STORY-ID}-Task实现方案.md
    │     （实现方案是"汇总层"，最后看才能对照前面 Task 的实际决策）
    │
    └── 4. 实现方案 ✅ 通过后，输出最终确认：
          "已逐文件核对完毕：[文件名 ✅/⚠️→✅/⏸️] × N + 实现方案 ✅。
           是否确认开始编码实现？"
```

**AI 与用户对话的强制要求：**

| 场景 | ❌ AI 错误行为 | ✅ AI 正确行为 |
|------|-------------|-------------|
| 进入审核 | "Task 已生成，请审核"（让用户自己找） | "现在开始过 Task 文件。按文件名字典序，第 1 个：`task-0-env.md`。" |
| 呈现单文件 | 只列文件名 / 只给摘要 / 让用户自己打开看 | **完整读出文件全文** |
| 提请确认 | "请确认"（不指明看哪里） | "**请重点确认 §3 事务边界**——这与下游 Task-2 强依赖，如有调整会影响 Task-2" |
| 用户回复模糊 | 用户说"好" / "行" / "OK" 即进入下一文件 | **追问确认**："您对当前文件是 ✅ 通过 / ⚠️ 修改 / ⏸️ 暂停？模糊回复需要明确判定。" |
| 用户中途想跳 | 默认跳到末尾"整体确认" | **拒绝跳过**："按 SKILL 规定必须逐文件核对，您可以 ⏸️ 暂停但不能跳到汇总。如要整体重审，请回 Task Generate 阶段。" |
| 全部完成 | "OK 进入编码" | 输出"已逐文件核对：Task-0 ✅ / Task-1 ✅ / Task-2 ⚠️→修复→✅ / ... / 实现方案 ✅ / CodingPlan ✅。是否确认开始编码？" |

**跳过与暂停规则：**

- ⏸️ **暂停**：AI 必须记录"已审核到第 N 个 / 共 X 个文件"、当前文件路径、当前决策（✅/⚠️/⏸️）→ 写入 `.auto-engineering/{STORY-ID}/state.json` 的 `currentStep` 字段 → 下次用户说"继续"时**从第 N+1 个文件继续**，不重头开始。
- ❌ **终止**：清空本轮审核记录 → 触发 Task Generate SKILL 重新生成（或局部修复）→ 修复后回到本审核点，**从第 1 个文件重新走**。
- 🚫 **不存在"快速模式"** —— SKILL 没有快速模式，必须逐文件。

**门禁（出闸条件）：**

- [ ] 全部 Task 文件均获得用户逐文件 ✅ 确认
- [ ] {STORY-ID}-Task实现方案.md 获得用户 ✅ 确认
- [ ] **{STORY-ID}-CodingPlan.md 获得用户 ✅ 确认（详见 ④bis 步骤）**
- [ ] 任何 ⚠️ 已被修复并重新获得 ✅

> **任一未达成 → 不允许进入 ⑤ Coding。** AI 不得以"用户已口头确认整体"等模糊信号绕过门禁。

**违反本门禁的典型反模式（必须避免）：**

- ❌ "我把所有 Task 文档贴在下面，您看一下，确认了我就开始编码。"
- ❌ "Task 文档生成完毕，共 8 个 Task，是否确认开始编码？"（一锅端问）
- ❌ 用户回"好的" / "可以" / "行" → AI 直接进入 ⑤ Coding。
- ❌ AI 自己说"重点是 X"但用户根本没看 X，AI 就当 ✅ 处理。

---

### 🔍 人工审核点 2.5：CodingPlan 评审（🆕 2026-06-10）

**触发时机：** 统一版 CodingPlan 生成后（TaskSkill 第六步完成），进入 ⑤ Coding 之前

**目的：** 在 Coding 之前把 CodingPlan 确认下来，避免 Coding 完后才发现方案有问题（重做 Coding 的成本远高于修改 CodingPlan）。

#### 📖 AI 主动讲解（CodingPlan 故事，🔴 强制）

> 本节点禁止直接问"请审阅 CodingPlan"。AI 必须先用 walkthrough 的方式，把 CodingPlan 主动讲给用户听。

讲解模板（必须覆盖）：

1. **CodingPlan 结构摘要**：16 章节有哪些、哪几章是核心、哪几章是辅助
2. **14 条门禁通过情况**：每条门禁的当前状态（✅/🔴/🟠），重点标红未通过项
3. **关键决策基线对齐**：与 Story `## 实现方案决策基线` 对齐情况（拆分依据/复用能力/五维质量）
4. **CodingModel 决策摘要**：11 维决策的结论 + 核心链路保护（如涉及）
5. **风险 Task 标记**：哪些 Task 风险较高（高并发/批量/外部依赖/支付等）

**讲解结束后的询问：**

**审核内容：**
- [ ] 16 章节齐全（无空章节，N/A 章节有说明）
- [ ] 14 条门禁全部通过
- [ ] **🔴 CodingModel 决策记录完整**（11 维均有明确结论，无空值/无"不知道"）
- [ ] 核心链路保护已标注（如涉及回调/Webhook/支付/状态更新/消息落库）
- [ ] 资源隔离已证明（如涉及异步/批量/外部通知）
- [ ] 混合压测场景已覆盖（如涉及核心链路 + 批量任务）
- [ ] 关键类骨架覆盖所有层（Domain/Application/Infrastructure/Interfaces）
- [ ] DB 操作 WHERE 明确，乐观锁/状态前置已标
- [ ] 测试映射完整，每个 TestCase 有对应 Task 覆盖点
- [ ] 调试回滚 ≥ 5 类
- [ ] 风险 Task 已标记

**询问用户：** "CodingPlan 已生成（路径：...），14 条门禁全部通过。请审阅 CodingPlan，重点关注：
1. 关键决策基线（拆分/复用）是否符合预期
2. CodingModel 决策（并发/幂等/事务/资源）方案是否可接受
3. 风险 Task 的应对方案是否充分
是否确认开始编码实现？"

**用户选项：**
- ✅ 确认 CodingPlan，无误
- ⚠️ 需要修改（说明修改内容，AI 记录后返回 TaskSkill 修补对应章节，再次过 14 条门禁，再次请用户确认）
- ❌ 暂停流程

> **强制要求：** 必须等待用户确认后才能进入 ⑤ Coding。模糊回复（如"好"/"行"/"可以"）需 AI 追问确认。

---

### ④bis CodingPlan 输出（🔴 强制 — 详细 7 章节已下沉）

> **📍 详细 7 章节 + 14 条门禁已下沉：** 原 AE-skill 中 ④bis 的 7 章节（文件顺序/类骨架/数据/Mapper SQL/测试对应/验证点/调试回滚）+ 14 条门禁 + 节点职责已统一存放在 [`coding-skill.md`](../phase2-coding/coding-skill.md) 和 [`be-coding-plan-template.md`](../../templates/coding/be-coding-plan-template.md)，本 SKILL 不再重复。

**AE 编排层 Phase ④→⑤ 调用协议（7 项前置条件，全部满足后才触发 `CodingSkill.Execute`）：**

| # | 前置条件 | 验证方式 |
|---|---------|---------|
| 1 | Task 0 ~ Task-N 全部已生成 | Task 文档目录中每个 Task 文件存在 |
| 2 | 每个 Task 均包含 `## CodingModel 决策记录`，11 维均有明确结论 | 逐 Task 文档检查该章节无空行 |
| 3 | 每个 Task 均包含 `## 任务级 CodePlan`（含类骨架 + 方法级逻辑 + DB 操作 + 外部依赖 + 测试映射） | 逐 Task 文档检查该章节子节齐全 |
| 4 | 全局 Task Review TR-1~TR-7 全部通过（无新增问题连续一轮） | Review 结论输出中 TR-1~TR-7 均显示 ✅ |
| 5 | 统一版 `{STORY-ID}-CodingPlan.md` 已生成，14 条门禁全部通过 | CodingPlan 门禁自检表全 ✅ |
| 6 | 统一版 CodingPlan 已获用户明确确认（"确认"/"同意"/"可以开始"） | 用户回复记录中有明确确认词 |
| 7 | 统一版 CodingPlan 中的 CodingModel 决策记录已复核（与各 Task 一致） | 无冲突项 |

> **任一前置条件未满足 → 禁止触发 `CodingSkill.Execute`。** AI 不得以"用户整体确认"等模糊信号绕过。

**触发：** `CodingSkill.Execute`（见 [coding-skill.md §CodingSkill 对外调用契约](../phase2-coding/coding-skill.md)）

---

### ⑤ Coding

**触发：** [Coding SKILL](../phase2-coding/coding-skill.md)

**输入：** Story + Task 文档 + **{STORY-ID}-CodingPlan.md** + 测试用例 + 约束 + 工作目录
**输出：** 可编译、可测试的代码 + Coding 报告

**触发：** [Coding SKILL](../phase2-coding/coding-skill.md)

**输入：** Story + Task 文档 + 测试用例 + 约束 + 工作目录
**输出：** 可编译、可测试的代码 + Coding 报告

**一轮 Coding 的定义：**
一轮 Coding = 对一个或多个连续 Task 的完整编码实现（从开始到报告出具）。一轮可能包含多个 Task，也可能只有一个 Task。

**多轮 Coding 的常见场景：**

| 场景 | 说明 | 示例 |
|------|------|------|
| 0-1 实现 | 全新功能实现 | Task 1-3 全部新写 |
| 缺陷修复 | 修复测试/Review 发现的问题 | 第一轮发现 bug，第二轮修复 |
|增量补充 | 补充遗漏的功能点 | 发现漏了通知逻辑，补加 |
| 重构优化 | 不改功能，仅优化结构 | 抽取公共方法、拆解大事务 |

**一轮 Coding 的典型流程：**
```
确认实现方案 → 按 Task 顺序写代码 → 编译 → 测试 → 出 Coding 报告 → 出 CodeReview 报告
    │
    └── 失败 → 异常路径（记录问题 → 分析 → 修复 → 继续）
```

**多轮 Coding 的衔接规则：**
- 每一轮 Coding 完成后，出具该轮的 CodeReview 报告（独立版本）
- 后续轮次的 CodeReview 基于前序轮次累积评估（但每次报告独立存档）
- 多轮之间通过 `state.json` 中的 `codingRound` 字段追踪

**Coding 报告要求：**
编码完成后必须生成 `{story}-CodingReport-v{N}-r{M}.md`（v{N}=Story 版本号，r{M}=第 M 轮 Coding），包含以下内容：

1. **Story 任务概述**
   - Story ID 和标题
   - 核心功能描述
   - 实现的业务价值

2. **分层实现清单**
   - 按调用顺序分层列出，表格列固定为 `类型 / 文件路径 / 变更类型 / 说明`
   - **SPI 层**：跨服务接口、SPI DTO（如有）
   - **Domain 层**：聚合根、实体、值对象、领域服务、Facade 接口、Repository 接口
   - **Application 层**：AppService、Orchestrator、DTO/Command、Converter、事件处理器
   - **Infrastructure 层**：Facade 实现、Repository 实现、PO/DO、Mapper/DAO、外部服务集成
   - **Interfaces/BFF 层**：Controller、Request/Response、JobHandler、BFF 聚合入口
   - **Test 层**：单元测试、集成测试、测试资源
   - **文档/配置**：Nacos、YAML、DDL、Story/Coding 报告等（如有）

3. **关键业务逻辑说明**
   - 每个核心方法的业务逻辑描述
   - 状态机流转逻辑（如有）
   - 事务边界说明
   - 并发控制方案（如有）

4. **数据库变更**
   - 新增/修改的表结构
   - 新增/修改的索引
   - 数据迁移脚本（如有）

5. **外部依赖调用**
   - 调用的外部服务及接口
   - 调用时机和参数说明
   - 异常处理策略

6. **单元测试覆盖**
   - 测试类清单
   - 核心场景覆盖情况
   - Mock 策略说明

7. **开发问题记录**
   - 遇到的问题及解决方案（必须记录）
   - 技术债务说明（必须记录）
   - 待优化项（必须记录）

---

### 🔍 人工审核点 3：~~编码阶段完成确认~~（🗑️ 2026-06-10 删除）

> **删除原因：** 编码完成后的确认已合并到 Phase 3 ⑦ 阶段的 CodeReview 报告中（CodeReview 报告本身包含代码 walkthrough + ⑥bis 一致性核查 + ⑦bis 对称性闸），无需单独的"Coding 完成后"审核节点。重复审核浪费用户时间。
> 替代节点：见下面 `§🔍 人工审核点 4：CodeReview 阶段完成确认`（旧 §3 的内容已合并过去）。
---

## Phase 3：验证阶段（必须完成，不可跳过）

### 🔴 测试真实性强制规范（8 类禁止手段 + 证据链硬门禁 — 已下沉到 coding-skill）

> **📍 详细 8 类禁止伪造手段 + 证据链硬门禁已下沉：** 原 AE-skill 中 Phase 3 的"测试真实性强制规范"（包括 8 类禁止手段：@Disabled 隐藏失败 / assertTrue(true) 永真 / catch 吞噬异常 / 全 Mock 替代 / 期望值=实际值 / 无效测试数据 / Thread.sleep 绕过 / 凑覆盖率 + 原始日志 / Surefire-Failsafe XML / `test_authenticity_scan.py` / AC 对账 / 跳测参数扫描）已统一存放在 [`coding-skill.md` §📋 测试真实性强制规范](../phase2-coding/coding-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 1 个门禁：**
> - ✅ ⑥ 完成判定前必须派 `test-verifier` sub-agent 独立跑一遍测试 + 解析 Surefire/Failsafe XML + 执行 `scripts/test_authenticity_scan.py` + 扫描 8 类禁止手段
> - ✅ 详见 coding-skill.md 对应章节
> - 🔴 **主 agent 自称"测试通过"无效，必须 test-verifier 独立验证不通过 = 不通过**

---

### ⑥ 完成判定（逐维量化验证，不可仅凭感觉）

| # | 条件 | 验证方式 | 通过标准 |
|---|------|---------|---------|
| 6.1 | `mvn compile` 通过 | 执行 `mvn compile` | 返回 BUILD SUCCESS，无 error |
| 6.2 | 服务启动成功 | 执行 `mvn spring-boot:run` | 日志含 `Started XxxApplication in X seconds`；端口实际监听（`curl localhost:port/actuator/health` 返回 UP）；无 `BeanCreationException`、`BeanNotOfRequiredTypeException` |
| 6.3 | 主流程接口 Pass | 执行 L2 真实 HTTP 测试（SpringBootTest RANDOM_PORT + TestRestTemplate）或 L3 集成测试 | 主流程接口 100% Pass（至少覆盖 Story 的 AC-001 场景）；返回 HTTP 200；响应结构与 Story 接口契约一致。🔴 能走真实 HTTP 的接口禁止仅用 MockMvc 验证（MockMvc 不走真实端口/网络），仅框架过老无法启动嵌入式容器时降级，并在测试报告注明 |
| 6.4 | 错误码映射正确 | 执行异常场景 L2 测试 | 业务异常 → 对应错误码（如 2104X）；系统异常 → 500 或对应兜底码；HTTP 状态码与 Story 定义一致 |
| 6.5 | DB 写操作落库 | 执行 L3 集成测试（真实 DB） | 验证：INSERT 后 `SELECT` 能查到；UPDATE 后字段值正确；事务提交后数据可见 |
| 6.6 | 事务边界正确 | 执行 L3 集成测试 | 事务内操作失败 → 全部回滚（数据未污染）；事务外操作（通知/消息）异步执行（不等返回） |
| 6.7 | 所有测试用例 Pass | 执行 `mvn test` | L1/L2/L3/L4 全部 Pass；无跳过（除非 Story 在"可跳过的测试"章节中明确标注了可跳过的测试 ID 和原因，且原因合理） |
| 6.8 | 异常路径无 Open 问题 | 检查开发问题记录 | 所有 Open 问题都有修复方案或已修复 |
| 6.9 | 测试报告已出具 | 检查 `{story}-Report.md` 存在 | 文件已生成且与代码仓库内容一致 |
| 6.10 🔴 | **测试真实性校验** | **执行 `🔴 测试真实性强制规范`：独立复跑测试、归档原始日志、解析 Surefire/Failsafe XML、运行 `scripts/test_authenticity_scan.py`、对账 AC × 测试方法** | **扫描 BLOCKER=0；XML failures/errors/skipped=0（skipped 需 Story 明确批准）；测试报告统计与 XML 一致；无跳测/忽略失败参数；关键测试代码已呈现；测试数据可追溯到 Story/Task；无未授权"修复测试"；AC 覆盖率 100%。任一未达成 → 测试报告作废，⑤ Coding 必须返工** |

**门禁说明：**
- 6.1-6.3：任意一项未通过 → 🔴 强制停止，必须修复才能继续
- 6.4-6.6：任意一项未通过 → 🟠 严重型缺陷，需修复后继续
- 6.7-6.10：任意一项未通过 → 修复后才能进入 CodeReview

**全部通过 → 进入 ⑥bis 全切面一致性核查闸（CodeReview 的硬前置，不可跳过）**
**未全部通过 → 回到 Phase 2 继续**

---

### ⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 硬前置 — 已下沉到 coding-skill）

> **📍 详细 4 步核查 + 漂移判定已下沉：** 原 AE-skill 中 ⑥bis 的"全切面一致性核查表"（以代码为锚反向核查 DR / Story / Task / 测试用例 / 代码 五方一致 + 🔴 漂移判定规则）已统一存放在 [`coding-skill.md` §📋 ⑥bis 编码后全切面一致性核查闸](../phase2-coding/coding-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 1 个门禁：**
> - ✅ ⑦ CodeReview 出具前必须完成 ⑥bis 闸
> - ✅ 输出《全切面一致性核查表》并嵌入 CodeReview 报告"零、"章节
> - 🔴 任一漂移项 = 阻断型 CodeReview 问题

---

### ⑦ CodeReview 报告出具

**触发时机：** ⑥ 完成判定全部通过后，人工审核之前

**输入：** Coding 报告 + 测试报告 + Story 文档 + DR 文档 + 约束规范 + 实际代码

**输出：** `{story}-CodeReview-v{N}-r{M}.md` — 架构师级审阅报告（多版本）

**多版本规则：**
- 一个 Story 可以有多轮 Coding，每轮出具一份独立的 CodeReview 报告
- 文件命名：`{STORY-ID}-CodeReview-v{N}-r{M}.md`
  - `v{N}` = Story 版本号（Story 文档变更后累加）
  - `r{M}` = Coding 轮次号（第 M 轮 Coding 的报告）
- 多轮之间，后一轮的 CodeReview 应在"八、本次提交 Git 文件清单"中标注"与 v{N}-r{M-1} 相比的变化"，不要求重复描述未变化的内容
- 每一轮报告都是独立的完整版本，可独立审阅；累积问题清单在最后一轮汇总

> **强制要求：AI 必须扫描实际代码仓库逐文件填写，禁止凭记忆或 Story 文档推断填写。每个章节的内容必须与实际代码一致。**

> **扫描范围：不仅扫描本 Story 新增的文件，还必须扫描被修改的现有文件，并检查其所有直接调用方是否受影响。扫描方式：使用 IDE 搜索或 grep 查找所有引用。使用 `grep -r "类名" --include="*.java"` 确认所有调用点。传递依赖不在强制扫描范围内，但若因传递依赖导致编译错误则必须处理。禁止只扫新增文件就下结论。**

---

## 七、CodeReview 报告模板

> **📍 模板已下沉到 `templates/`：** 完整的 9 章节 CodeReview 报告模板（含"零、全切面一致性核查表" / 一-九节 / 分层职责红线核查 / 产出物对账 / Git 文件清单等）已统一存放在 [`templates/coding/be-codereview-template.md`](../../templates/coding/be-codereview-template.md)，本 SKILL 不再重复维护副本。
>
> **使用规则：**
> 1. **生成 CodeReview 报告时** → 直接复制 `templates/coding/be-codereview-template.md`，按 Story 填充各章节
> 2. **流程门禁** → ⑦ CodeReview 出具前必须确认 ⑥bis 全切面一致性核查闸已通过、"零"章节已嵌入报告
> 3. **模板维护** → 模板本身如有更新（如新增核查维度），直接修改 `templates/coding/be-codereview-template.md`，**禁止在 AE-skill 中维护副本**（这是本次重构的核心原则，杜绝重复堆积）
>
> **AE 编排层只关注 4 个门禁：**
> - ✅ ⑥bis 全切面一致性核查闸已通过（详见第六章对应章节）
> - ✅ CodeReview 报告已按模板生成、"零"章节已嵌入
> - ✅ ⑦bis 全链路对称性核查闸已通过（见下）
> - ✅ 报告路径与产出物对账表 100% 一致

---

### ⑦bis 全链路对称性核查闸（🔴 流程收尾强制 — 已下沉到 coding-skill）

> **📍 详细 5 步核查流程已下沉：** 原 AE-skill 中 ⑦bis 的"全链路对称性追溯矩阵"（5 步核查：DR 章节 ↔ Story 章节 ↔ Task 章节 ↔ 代码文件:行号 ↔ 测试用例 ID）已统一存放在 [`coding-skill.md` §📋 ⑦bis 全链路对称性核查闸](../phase2-coding/coding-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 1 个门禁：**
> - ✅ 人工审核点 4 前必须完成 ⑦bis 闸，5 层双向追溯无 🔴 断链（漏做/多做）
> - ✅ 输出《全链路对称性追溯矩阵》

---

### 🔍 人工审核点 4：CodeReview 阶段完成确认（🆕 2026-06-10 改名，原"验证阶段完成确认"）

**触发时机：** CodeReview 报告生成后，最终完成之前

**审核核心：** 以 CodeReview 报告为介质进行架构师级审阅

#### 📖 AI 主动讲解（Code 故事，🔴 强制）

> **本节点禁止直接问"请审阅 CodeReview 报告"。** AI 必须先用 walkthrough 的方式，把代码实现主动讲给用户听。详细规范见上文 `📖 人工审核主动讲解规范` 章节。

讲解模板参考上文 `③ CodeReview 阶段 —— 讲"代码实现故事"` 的输出模板，**必须覆盖**：
1. **代码实现故事**：本轮 Coding 实现了哪些 Task，调用链怎么走
2. **分层 walkthrough**：Domain/Application/Infrastructure/Interfaces 各层核心类 + 关键方法 + 代码位置（文件:行号）
3. **状态机实现**：canTransition() 实际代码、状态流转的判断条件在哪一行
4. **事务实现**：@Transactional 边界、传播行为、回滚规则
5. **异常处理**：异常类、错误码、HTTP 状态码的映射
6. **测试覆盖**：AC × 测试方法 的对应关系、覆盖率
7. **CodeReview 关键发现**：🔴🟠 问题的整改方案

**讲解形式要求：**
- 必须给具体文件:行号，不能只说"在 XxxService 里"
- 必须用代码片段或伪代码展示关键逻辑，不能只口头描述
- 必须主动指出"如果用户想深挖，建议看 {文件}:{行号}-{行号}"
- 用户追问"这段代码什么意思" → 视为讲解不充分，AI 必须自我反思并补充讲解

**讲解结束后的询问：**

**审核内容：**
- [ ] **⑥bis 全切面一致性核查表已嵌入报告"零、"章节，无 🔴 漂移（多做/漏做/做歪全部闭环）**
- [ ] **核查范围为全章节全文件（非当轮 diff），且以代码为锚反向核查**
- [ ] **核心落库路径有真实 DB 证据（无 Mock 充当）**
- [ ] **⑦bis 全链路对称性追溯矩阵已出具，DR-Story-Task-实现-测试用例 五层一一对应，无 🔴 断链（漏做/多做）**
- [ ] CodeReview 报告已出具且内容完整
- [ ] 核心业务逻辑描述准确、可理解
- [ ] DB 逻辑链清晰，事务边界合理
- [ ] 分层实现清单与实际代码一致
- [ ] 无 🔴阻断型 / 🟠严重型 问题（或已附整改方案）
- [ ] DR-Story-Task 设计一致性已验证
- [ ] `mvn compile` 通过
- [ ] **服务启动成功**（有启动日志，无 Bean 注入失败）
- [ ] **接口测试 Pass**（主流程接口至少 1 个通过）
- [ ] 所有测试用例 Pass
- [ ] 测试报告已出具
- [ ] 所有产出物齐全

**询问用户：** "验证阶段已完成，CodeReview 报告已出具。请审阅报告重点关注：
1. 核心业务逻辑是否符合预期
2. DB 逻辑链是否合理
3. 分层实现是否清晰
4. 服务能否启动、接口能否调通
是否确认工程完成？"

**用户选项：**
- ✅ 确认工程完成
- ⚠️ 需要补充测试或修复问题（说明修改内容）
- 🔄 重新执行某个阶段
- ❌ 暂停流程

---

### ⑧ 完成输出

工程完成后输出（最终版，即最后一轮的产出物路径）：

| 产出物 | 路径 |
|--------|------|
| Story 文档（稳定版） | `design/story/be/{story}.md`（最终版） |
| Story 补充说明 | `design/story/be/{story}-Supplement.md` |
| Task 文档 | `design/story/be/task/{story}/` |
| **Coding 报告** | `design/story/be/coding/{story}/{story}-CodingReport-v{N}-r{M}.md`（最后一轮） |
| **CodeReview 报告** | `design/story/be/coding/{story}/{story}-CodeReview-v{N}-r{M}.md`（最后一轮） |
| 测试用例文档 | `design/testcase/be/{story}/{story}-testcase.md` |
| **测试报告** | `design/testcase/be/{story}/{story}-Report-v{N}-r{M}.md`（最后一轮） |
| 源代码 | 工作目录下对应工程 |
| 开发问题记录 | `design/story/be/coding/{story}/{story}-开发问题记录.md` |

> **多轮存档说明：** 每一轮 Coding 的报告（v{N}-r{1}、v{N}-r{2}...）均独立存档，不覆盖。目录 `design/story/be/coding/{story}/` 下存放该 Story 所有轮次的报告文件。

---

## 异常处理

### 总原则：DR-Story-Task 设计一致性

```
DR（需求文档）→ Story → Task → Coding
     ↑
     └── 一切以 DR 为准。DR 是设计链路的源头基准，Story 是 DR 的细化，
         Task 是 Story 的实现映射。设计链路必须保持一致。
```

**核心原则（强制，不可违背）：**
- DR 是设计的基准，任何层级的修改最终都要与 DR 对齐
- Coding 发现 DR 有缺陷/不合逻辑 → 必须修改 DR，不迁就
- 修改 DR 后 → 必须触发 Story Review，验证 Story 与 DR 的一致性
- 禁止为了绕过 DR 问题而扭曲 Story 或 Task 的描述
- 问题必须在发生的层级解决，不允许用上层弥补下层缺陷

### Coding 问题分层排查与修改链

> **📍 详细 4 层排查流程已下沉到 coding-skill.md：** 原 AE-skill 中"Coding 问题分层排查与修改链"（含 ASCII 流程图 + 判定标准表 + 修改影响范围表 + 关键原则）已统一存放在 [`coding-skill.md` §📋 Coding 问题分层排查与修改链](../phase2-coding/coding-skill.md)，与 coding-skill.md 自身的"异常路径 A1-A4"整合。
>
> **AE 编排层只关注分层原则：**
> - 严格在问题发生的层级解决，禁止跨越层级处理
> - Task 问题与 Story 无关 → 直接改 Task，不动 Story
> - Story 问题与 DR 无关 → 直接改 Story，不动 DR
> - 只有确认 DR 本身有缺陷时，才走完整链路
> - 详细排查步骤 / 判定标准 / 修改影响范围 → 见 coding-skill.md 对应章节

---

### 人工介入节点

| 节点 | 触发条件 | 询问内容 |
|------|---------|---------|
| **Phase 1 → Phase 2** | 测试用例生成完成 | "设计阶段已完成，是否进入实现阶段？" |
| **实现方案预确认** | Phase 1 完成后，生成 Task 文档之前 | "请确认核心业务理解、接口依赖、分层思路、并发策略、异常处理，无误后生成 Task 文档" |
| **Task 生成后** | Task 文档生成完成 | "Task 文档已生成，是否开始编码？" |
| **Task 实现方案确认** | 每个 Task 开始写代码之前 | "请确认本 Task 的类/方法/核心逻辑/DB 操作，实现方案无误后开始编码" |
| **Phase 2 → Phase 3** | ~~所有 Task 编码完成（2026-06-10 删除）~~ | ~~"编码阶段已完成，是否进入验证阶段？"~~ |
| **🆕 CodingPlan 评审**（2026-06-10） | 统一版 CodingPlan 生成后，Coding 之前 | "CodingPlan 已生成，14 条门禁全过，请审阅并确认是否开始编码" |
| **Phase 3 完成** | 所有测试通过 + CodeReview 报告出具 | "CodeReview 阶段完成（含 ⑥bis 一致性核查 + ⑦bis 对称性闸），是否确认工程完成？" |
| Story Review 每 3 轮（每完成 3 轮完整的 A-E 阶段循环） | 自动 | "是否继续 Review？" |
| 需人工裁定的缺陷 | 合理性判定为 ⚠️ | "请人工判定此项是否为缺陷？" |
| DR 变更影响多个 Story | DR Update 评估 | "是否同步修改受影响 Story？" |

---

## 子 SKILL 索引

| SKILL | 文件 | 职责 |
|-------|------|------|
| DRtoStory | （已有，非本目录） | DR → Story 生成 |
| Story Review | [story-review-skill.md](../phase1-design/story-review-skill.md) | Story 缺陷挖掘循环 |
| TestCase Generate | [testcase-generate-skill.md](../phase1-design/testcase-generate-skill.md) | 测试用例生成（全场景覆盖 + 合规性校验） |
| Story Update | [story-update-skill.md](../phase1-design/story-update-skill.md) | Story 文档更新 |
| Task Generate | [task-generate-skill.md](../phase2-task/task-generate-skill.md) | Task 文档生成与更新 + 全局 Task Review（结合约束+Story+测试用例审查所有 Task，闭环修复）|
| Coding | [coding-skill.md](../phase2-coding/coding-skill.md) | 代码实现 + 问题反馈 |
| DR Update | [dr-update-skill.md](../phase1-design/dr-update-skill.md) | DR 文档更新 |

---

## 禁止事项（强制，违者必须整改）

| 禁止（违者） | 正确做法（强制） |
|------|------|
| 跳过 Phase 1 直接写代码 | 必须先完成设计阶段 |
| 跳过测试用例生成 | Story 稳定后必须生成测试用例 |
| 测试未全部通过就报告完成 | 完成标准是全部 Pass |
| 跳过测试报告 | 每次测试都要出报告 |
| **跳过 Coding 报告** | **编码完成后必须生成 Coding 报告** |
| **跳过 CodeReview 报告** | **验证阶段必须出具架构师级 CodeReview 报告** |
| **跳过实现方案预确认** | **Phase 2 开始前必须向用户呈现实现思路并获确认** |
| **跳过 Task 实现方案确认** | **每个 Task 开始前必须向用户呈现实现方案并获确认。用户必须明确说"确认"、"同意"、"可以开始"，模糊回复（如"好"、"行"、"看看"）需要 AI 追问确认，未获明确确认前禁止写代码** |
| 人工节点自动决策 | 待讨论项必须询问用户 |
| **跳过阶段审核** | **每个阶段完成后必须经过人工审核确认** |
| **未经确认进入下一阶段** | **必须等待用户确认后才能继续** |
| **Task 审核一锅端（一次性抛出全部 Task 文档让用户"整体确认"）** | **🔴 必须逐文件自上而下核对，每个文件单独获 ✅ 后才进入下一文件** |
| **🔴 跳过人工审核的主动讲解（只丢文档不讲解）** | **🔴 三个审核节点（① 设计阶段 ② Task 文档 ④ CodeReview）必须先讲"故事"再问确认。详见 `📖 人工审核主动讲解规范` 章节** |
| **🔴 跳过 CodingPlan 直接写代码** | **🔴 ⑤ Coding 前必须有 `{STORY-ID}-CodingPlan.md`，含 7 个章节（文件顺序/类骨架/数据/Mapper SQL/测试对应/验证点/调试回滚）。详见 ④bis 章节** |
| **🔴 测试伪造通过** | **🔴 8 类禁止手段任一命中 = 测试无效：@Disabled 隐藏失败 / assertTrue(true) 永真 / catch 吞噬异常 / 全 Mock 替代 / 期望值=实际值 / 无效测试数据 / Thread.sleep 绕过 / 凑覆盖率。详见 `🔴 测试真实性强制规范` 章节** |
| **🔴 "修复测试"代替"修复代码"** | **🔴 AI 自行修改已审核通过的测试代码 = 伪造测试。必须标注修改原因 + 获得用户确认。未确认的修改视为伪造** |

---

## 执行清单（逐项执行，禁止跳过）

> **强制要求：AI 启动本 SKILL 时，必须用 TodoWrite 1:1 映射此表，即：执行清单的每一行对应一个 TodoWrite 项，动作内容 = 该行的"动作"列，状态 = 进行中/已完成。未满足门禁不得继续，不得自行降级处理。**

| # | 动作 | 产出物 | 门禁 |
|---|------|--------|------|
| **0** | **🆕 工作区与项目资产检查**（SKILL 启动时最先执行）| — | **projectKey 已知 + 项目资产已存在（或已完成生成并用户确认）**；任一未满足禁止进入后续步骤 |
| 0a | 确认工作区（projectKey / gitPath）| — | 用户明确告知或当前 session 已知 |
| 0b | 调用 `get_assets(projectKey)` 检查项目资产 | — | 资产存在 → 静默通过；资产不存在 → 进入 0c |
| 0c | **（资产缺失时）** 明确告知用户，调用 `project-assets-update-skill.md §3 生成动作` | `{gitPath}/.ae-project/assets.md` | 生成完成后 AI 报告摘要（微服务数/分层/技术栈）；用户确认资产内容准确 |
| 1 | 收集输入（DR 路径 + Story ID + 工作目录） | — | 三项信息已确认 |
| 1a | **🤖 多 Agent 模式决策**（在 Step 2 之前，可选） | `state.json` 中 `multiAgentMode: true/false` | 检测到"何时启用"表任一条件时主动提议；用户明确同意后启用；启用后从角色库选 sub-agent；⑥.10 测试真实性强制派 `test-verifier` 独立验证 |
| 2 | 生成 Story（DRtoStory SKILL） | `{story}.md` | 文件已生成或已存在 |
| 2b | **🔴 前端视角接口审视** | Story 文档追加"前端接口契约"章节（含 6 个维度：契约完整性/调用流程/状态展示/错误码/边界场景/联调支持） | 6 个维度门禁全部通过；至少 1 个完整请求+响应示例；不确定项已标注"需前端确认"；未通过禁止进入 Story Review |
| 3 | Story Review（Story Review SKILL，含 F-Stage 前端契约 Review） | `{story}-Supplement.md` + `{STORY-ID}-StoryReviewUpdatePlan-r{轮次}.md`（有确认缺陷时） | Review 循环已退出；每轮确认缺陷均先出 Plan 再修改 Story；F-Stage 6 项检查全部通过 |
| 4 | 生成测试用例（TestCase Generate SKILL） | `{story}-testcase.md` | 文件已生成 + 合规性校验通过 |
| 4-📖 | **🔴 AI 主动讲解 Story 故事**（人工审核点 1 之前） | Story 故事讲解输出 | 已讲清"业务背景/核心流程/关键设计决策/AC 故事/已识别风险"5 个维度；未讲解禁止进入 Phase 1 → Phase 2 的人工审核点 1 |
| 4b | **实现方案预确认** | 用户确认记录 | 用户已确认实现方案（核心业务/接口/分层/并发/异常） |
| 5 | 生成 Task（Task Generate SKILL） | `task/{story}-task-*.md` + `{STORY-ID}-Task实现方案.md` | 全部 Task 文件已生成 |
| 5a | **全局 Task Review（结合约束+Story+测试用例）** | Review 结论 | TR-1~TR-7 全部通过；发现问题 → Task 修复 → 重新 Review；连续一轮无新增问题才退出，然后输出实现方案 |
| 5b | 人工审核：Task 文档 + 实现方案完成确认（🔴 强制逐文件自上而下核对，禁止一锅端确认） | 用户逐文件确认记录（每个 Task 文件 + 实现方案 + CodingPlan 各 1 条） | 每个文件单独获得用户 ✅；模糊回复追问后再判定；跳过/整体确认视为违规 |
| 5b-📖 | **🔴 AI 主动讲解 Task 故事**（在 5b 之前） | Task 故事讲解输出 | 已讲清"拆 Task 故事/依赖链路/DB 变更/事务边界/风险 Task"5 个维度；每 Task 文件核对前已讲"本 Task 故事"；未讲解禁止进入 5b |
| 5c | **🔴 CodingPlan 输出**（④bis，⑤ 之前） | `{STORY-ID}-CodingPlan.md` | 7 章节齐全；**14 条门禁全部通过**（含 CodingModel 决策记录完整 + 核心链路保护 + 资源隔离 + 混合压测）；Phase ④→⑤ 调用协议 7 项前置条件全满足；未通过禁止触发 `CodingSkill.Execute` |
| **5b.5** | **🆕 2026-06-10 人工审核：CodingPlan 评审**（在 5c 之后，⑤ 之前） | 用户确认记录 | 用户已确认 CodingPlan；模糊回复追问后再判定；未确认禁止进入 ⑤ Coding |
| 5c-🚫 | **🔴 测试真实性扫描**（在 6.7 之前，⑥ 完成判定硬前置） | 原始日志 + XML 对账 + `test-authenticity-scan` 报告 | `test_authenticity_scan.py` BLOCKER=0；Surefire/Failsafe XML 与报告统计一致；无跳测/忽略失败参数；关键测试代码已呈现；测试数据可追溯；无未授权"修复测试"；AC 覆盖率 100%。任一未达成 → 测试报告作废，⑤ Coding 返工 |
| 6 | Coding（Coding SKILL） | 源代码 + 开发问题记录 | 编译通过 + 服务启动成功 + 接口测试 Pass + 测试全部 Pass |
| 7 | 出具测试报告 | `{story}-Report.md` | 文件已生成 |
| 8 | 出具 Coding 报告 | `{story}-Coding-Report.md` | 文件已生成 |
| 8a | **⑥bis 编码后全切面一致性核查闸** | 《全切面一致性核查表》（嵌入 CodeReview 报告"零、"章节） | 🔴 以代码为锚反向核查全章节五方一致；无 🔴 漂移；核心落库路径有真实 DB 证据；未达标禁止进入 8b |
| 8b | 出具 CodeReview 报告 | `{story}-CodeReview.md` | 报告结构完整，含"零、全切面一致性核查表"，无阻断型问题 |
| 8c | **⑦bis 全链路对称性核查闸** | 《全链路对称性追溯矩阵》 | 🔴 DR-Story-Task-实现-测试用例 五层双向追溯一一对应；无 🔴 断链（漏做/多做）；未达标禁止进入人工审核 |
| 9 | 完成判定 | — | 全部条件 ✅ |
| 9-📖 | **🔴 AI 主动讲解 Code 故事**（在 10 之前） | Code 故事讲解输出 | 已讲清"调用链 walkthrough/分层实现/状态机/事务/异常/测试覆盖/CodeReview 发现"7 个维度；含具体文件:行号和代码片段；未讲解禁止进入 10 |
| 10 | 人工审核确认 | — | 用户确认工程完成 |
