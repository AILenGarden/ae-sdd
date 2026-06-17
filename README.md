# Auto Engineering — 端到端自动化工程（使用指导书 + 功能说明书）

> **定位：** 本目录是 `auto-engineering` 体系（简称 AE）的**母版**，包含 15 个 SKILL + 模板 + 项目资产 + 策略 + 脚本。本 README 是**使用指导书**（如何用 AE 跑项目）和**功能说明书**（每个 SKILL 做什么）。
>
> **版本：** 2026-06-17（**🆕 SKILL 间能力调用规范化**：项目资产 §G 升级为场景化服务 API + 7 个 SKILL 改为调用 assets.forXxx() + ae-sdd-doc/ 路径全面修正 + code-review 多 Agent 编排去 DRY + pending-questions 机制）
>
> **目标用户：** 架构师 / 项目 owner / 开发者 / AI Agent

---

## 🚀 快速安装

**macOS / Linux / Windows Git Bash**
```bash
curl -fsSL https://raw.githubusercontent.com/AILenGarden/ae-sdd/main/scripts/install.sh | bash
```

**Windows PowerShell**
```powershell
irm https://raw.githubusercontent.com/AILenGarden/ae-sdd/main/scripts/install.ps1 | iex
```

**本地安装**（已 clone 仓库）
```bash
bash scripts/install.sh
```

安装完成后在 Claude Code 中输入 `/ae-sdd` 即可使用。

---

## 📖 目录

1. [什么是 auto-engineering](#1-什么是-auto-engineering)
2. [4 层 SKILL 架构](#2-4-层-skill-架构)
3. [15 个 SKILL 功能清单](#3-15-个-skill-功能清单)
4. [使用指导：从 0 到 1 跑一个 Story](#4-使用指导从-0-到-1-跑一个-story)
5. [核心设计原则（5 大）](#5-核心设计原则5-大)
6. [横切依赖（3 个）](#6-横切依赖3-个)
7. [常见问题 FAQ](#7-常见问题-faq)
8. [维护与扩展](#8-维护与扩展)

---

## 1. 什么是 auto-engineering

**auto-engineering** 是**端到端自动化工程 SKILL 体系**，从 DR（需求文档）出发，自动化驱动整个研发流程（Story 生成 → 评审 → Task → Coding → Code Review），最终输出可上线的代码 + 完整的报告 + 可追溯的审计日志。

### 1.1 核心价值

- **流程自动化**：从 PRD/Issue/对话需求出发，9 步流程到上线的全自动化（人工仅在 5 个审核节点介入：🆕 RA 审核 + Story Review + Task Review + CodingPlan + CodeReview）
- **质量可追溯**：每一步都有节点闸门 + 实时追溯链 + 证据要求（grep / 文件:行号 / 测试方法）
- **跨项目复用**：模板 + 策略 + 项目资产 三层解耦，新项目零成本启动
- **多 Agent 协作**：任务节点内可拆子任务并行（5 Agent 上限 + 4 级故障补救）

### 1.2 9 步流程总览

```
[PRD / Issue / 对话需求]
    ↓
[🆕 Phase 0 需求分析] (requirement-analysis-skill)
  ① 需求分析 (requirement-analysis-skill)         ← 8 维度挖掘 + 5 问自检 + 5 维规模裁定 + 6 类路由
  ①.5 规模=大 → DR 生成 (dr-generate-skill)        ← 18 章节 DR
  ①.6 DR 评审 (dr-review-skill)                    ← 5 阶段评审 + 4 条标尺
    ↓
Phase 1 设计阶段
  ② 生成 Story (story-generate-skill)        ← 7 阶段挖掘 + 实现方案决策基线 + 8 道闸
  ③ Story Review (story-review-skill)          ← 5 阶段并行挖掘 + ①bis 6 维度
  ④ 生成测试用例 (testcase-generate-skill)    ← 3 层覆盖
    ↓
Phase 2 实现阶段
  ⑤ Task 生成 (task-generate-skill)           ← 含任务级 CodePlan
  ⑥ Coding (coding-skill)                     ← 实时追溯链
    ↓
Phase 3 验证阶段
  ⑦ 测试真实性验证 (coding-skill 集成)         ← 8 类禁止 + 5 类必真实
  ⑧ Code Review (code-review-skill)            ← 6 阶段评审 + 7 道闸
    ↓
[可上线的代码 + 完整报告 + 审计日志]
```

### 1.3 5 个审核节点（人工介入）

| 节点 | 触发 | 内容 |
|------|------|------|
| 🔍 人工审核点 0 | **🆕 2026-06-17**：RA 文档生成后 | 用户审阅需求分析（角色矩阵 / 场景清单 / 规模评分表）|
| 🔍 人工审核点 1 | Story Review 完成后 | 用户审阅 Story，确认设计决策 |
| 🔍 人工审核点 2 | Task Review 完成后 | 用户审阅 Task，确认实现拆解 |
| 🔍 人工审核点 2.5 | CodingPlan 评审 | 用户审阅统一版 CodingPlan + 14 条门禁全过 |
| 🔍 人工审核点 4 | CodeReview 阶段完成确认 | 用户审阅 CR 报告 + ⑥bis 一致性 + ⑦bis 对称性 |

> 5 个审核节点：🆕 RA 审核 + Story Review + Task Review + CodingPlan + CodeReview。

---

## 2. 母版目录结构（🆕 2026-06-10 全面重组）

按"**流程节点 + 横切依赖**"分类，不再按"文件类型"分。

```
skills/ae-sdd/                                     # 母版根
├── README.md                                              # 入口文档
├── SKILL.md                                               # 插件入口（= ae-sdd-skill.md 副本）
│
├── skills/                                                # 所有 SKILL .md 文件
│   ├── orchestration/                                     # 第 1 层：编排
│   │   ├── ae-sdd-skill.md                     # 统一入口 + 智能路由 + 流程编排
│   │   └── ae-sdd-update-skill.md               # SKILL 边界维护规范
│   │
│   ├── phase1-design/                                    # 第 2 层：Phase 1 ①+②+③
│   │   ├── requirement-analysis-skill.md     # 🆕 Phase 0 需求分析（1056 行）
│   │   ├── dr-generate-skill.md              # 🆕 DR 生成（1056 行）
│   │   ├── dr-review-skill.md                # 🆕 DR 评审（1082 行）
│   │   ├── story-generate-skill.md
│   │   ├── story-review-skill.md
│   │   ├── story-update-skill.md
│   │   ├── testcase-generate-skill.md
│   │   └── dr-update-skill.md
│   │
│   ├── phase2-task/                                      # 第 2 层：Phase 2 ④
│   │   └── task-generate-skill.md
│   │
│   ├── phase2-coding/                                     # 第 2 层：Phase 2 ⑤
│   │   ├── coding-skill.md
│   │   └── coding-report-skill.md
│   │
│   ├── phase3-review/                                    # 第 2 层：Phase 3 ⑦
│   │   └── code-review-skill.md
│   │
│   └── cross-cutting/                                     # 第 2 层：横切依赖
│       ├── document-storage-skill.md
│       ├── agent-orchestration-skill.md
│       ├── proposal-skill.md
│       └── project-assets-update-skill.md
│
├── standards/                                             # 标准和约束（原 constraints/ + strategies/）
│   ├── constraints/                                       # 9 个约束
│   ├── thinking/                                          # 编码思维引擎
│   ├── testing/                                           # 测试策略
│   └── project-assets/                                    # 项目资产 schema/template
│
├── assets/                                                # 实际项目资产（原 project-assets/）
│   └── icec-cloud-boss/
│       ├── icec-cloud-boss.assets.md
│       └── icec-cloud-boss.update-log.md
│
├── templates/                                             # 模板
│   ├── coding/                                            # CodingPlan / CodeReview / Coding 报告
│   ├── design/                                            # DR / Story / Task
│   │   ├── prd-template.md                    # 🆕 PRD 模板
│   │   ├── issue-template.md                  # 🆕 Issue 模板
│   │   ├── ra-template.md                     # 🆕 RA 需求分析模板
│   │   └── (DR / Story / Task 模板)
│   ├── testcase/                                          # TestCase / 测试报告
│   ├── proposal/                                          # Proposal
│   └── project-assets/                                    # 项目资产更新日志
│
└── scripts/                                               # 辅助脚本
    ├── sync-to-plugin.sh
    ├── test_authenticity_scan.py
    └── migrate-docs.mjs                  # 🆕 存量迁移工具（DRY-RUN）

docs/                                     # 🆕
└── migration-guide.md                 # 存量迁移指南
```

**AE 体系按"层级 + 职责"组织为 **4 层架构**（不变）：**

| 层级 | 职责 | 物理位置 |
|------|------|---------|
| **第 1 层：编排 + 入口** | 智能路由（用户输入分析 + 4 类需求规模判定）+ 流程编排（9 步怎么走）+ §⑦ter `{STORY-ID} Doc/` 同工作流文档收束原则 | `skills/orchestration/` |
| **第 1 层：边界维护** | SKILL 边界判定 / 文档存放标准（含 §0.5 重任务 + §2.6 三类任务规模路径） / Agent 编排（含 §13 跨 AI 工具适配）| `skills/orchestration/ae-sdd-update-skill.md` + `skills/cross-cutting/` |
| **第 2 层：节点 SKILL** | 每个流程节点的具体执行规则 | `skills/phase1-design/` + `skills/phase2-task/` + `skills/phase2-coding/` + `skills/phase3-review/` |
| **第 3 层：模板 + 项目资产** | 空白模板 / 项目特化资产（原位不动）| `templates/` + `standards/` + `assets/` + `scripts/` |

**4 层之间的关系：**
```
用户输入
    ↓
[第 1 层] auto-engineering-skill 智能路由（分析 → 路由到节点 SKILL）
    ↓
[第 2 层] 节点 SKILL 执行（走自己的 §整体流程）
    ├─ 触发时调用 [横切依赖]（document-storage / proposal / agent-orchestration）
    ├─ 完成后触发 [下游 SKILL]
    └─ 异常时触发 [Proposal SKILL] 走问题修复流程
    ↓
[第 3 层] 模板 + 项目资产（落地到具体文件）
```

---

## 3. 18 个 SKILL 功能清单（按新目录分类）

### 3.1 第 1 层：编排层（2 个）→ `skills/orchestration/`

| SKILL | 文件 | 功能 |
|-------|------|------|
| **auto-engineering-skill** | `skills/orchestration/ae-sdd-skill.md` | **AE 体系核心**。统一入口 + 智能路由（**🆕 2026-06-10 4 类需求**：已有 Story / 中大任务 / 小任务 / 微任务）+ 流程编排（9 步流程）+ **🆕 5 审核节点（加 CodingPlan 评审，删 Coding 完成评审）** + 角色库（8 角色）|
| **auto-engineering-update-skill** | `skills/orchestration/ae-sdd-update-skill.md` | SKILL 边界维护规范。判定"内容放哪个文件" + 健康度自检清单；步骤 4 新增：改动 AE-skill 时必须同步更新 README §3/§4.2/§8.5 及"最后更新"日期 |

### 3.2 第 2 层：节点 SKILL（12 个）→ `skills/phase1-design/` + `phase2-task/` + `phase2-coding/` + `phase3-review/`

| Phase | SKILL | 文件 | 功能 |
|------|-------|------|------|
| **0 ①** | **🆕 requirement-analysis-skill** | `skills/phase1-design/requirement-analysis-skill.md` | 需求分析（8 维度挖掘 + 5 问自检 + 4 级缺口 + 5 维规模裁定 + 6 类路由）|
| **0 ①.5** | **🆕 dr-generate-skill** | `skills/phase1-design/dr-generate-skill.md` | DR 生成（18 章节 + 实现方案决策基线 + 8 道闸）|
| **0 ①.6** | **🆕 dr-review-skill** | `skills/phase1-design/dr-review-skill.md` | DR 评审（5 阶段 A-E + 4 标尺 + 漏报升级）|
| 1 ② | **story-generate-skill** | `skills/phase1-design/story-generate-skill.md` | 从 DR 生成 Story（含 ①bis 6 维度前端契约）+ 7 阶段挖掘 + 实现方案决策基线 + 8 道闸 |
| 1 ③ | **story-review-skill** | `skills/phase1-design/story-review-skill.md` | Story 评审（5 阶段挖掘 + F-Stage 前端契约 + 4 标尺 + Plan-first）；退出后控制权交还主流程编排；**🆕 2026-06-10 上下文 7 件套（+PRD+产品原型）+ §A-UI 8 项 + §B-PRD 6 项** |
| 1 ③ | story-update-skill | `skills/phase1-design/story-update-skill.md` | 按 StoryReviewUpdatePlan 改 Story（已简化为 Proposal 指针）|
| 1 ④ | **testcase-generate-skill** | `skills/phase1-design/testcase-generate-skill.md` | 生成测试用例（3 层覆盖 + Story/DR/PRD/原型 6 文件输入）|
| 2 ⑤ | **task-generate-skill** | `skills/phase2-task/task-generate-skill.md` | Task 生成（任务级 CodePlan + 调 coding-skill ④bis SOP）+ 全局 Task Review；**🆕 2026-06-10 加"无 Story 上级文档"分支（小任务场景）** |
| 2 ⑥ | **coding-skill** | `skills/phase2-coding/coding-skill.md` | ⑤ Coding 怎么写（④bis 5 步 SOP + 实时追溯链 Task→Story→DR→AI 犯蠢）；**🆕 2026-06-10 加"无 Story 上下文"独立决策分支（微任务场景）** |
| 2 ⑥ | **coding-report-skill** | `skills/phase2-coding/coding-report-skill.md` | Coding 完成后出 Coding 报告（9 章节 + 7 道闸自检）|
| 3 ⑧ | **code-review-skill** | `skills/phase3-review/code-review-skill.md` | Code Review（6 阶段评审 + 7 道闸 + 多 Agent 编排 + 3 种评审模式 A/B/C）|
| - | dr-update-skill | `skills/phase1-design/dr-update-skill.md` | 按 DR 缺陷更新 DR 文档 |

### 3.3 第 2 层：横切依赖（4 个）→ `skills/cross-cutting/`

| SKILL | 文件 | 功能 |
|-------|------|------|
| **proposal-skill** | `skills/cross-cutting/proposal-skill.md` | **统一问题载体**。4 段必填（原本/目标/方案/涉及）+ 7 道闸 + 5 步走流程（改 Story → TestCase → Task → Coding → Test）+ 7 渠道接入 |
| **🆕 document-storage-skill**（强化） | `skills/cross-cutting/document-storage-skill.md` | **横切依赖标准 + 文档系统统一**。8 类流程目录（PRD/RA/DR/Story/Task/Coding/Test/CR）+ 版本号 v{major}.{minor} + ChangeLog + 关联性分析（业务 0/1 + 逻辑 0/1）+ gitignore 自动生成 + 存量迁移 API（migrate_old_docs）|
| **agent-orchestration-skill** | `skills/cross-cutting/agent-orchestration-skill.md` | 任务节点**内**子任务拆分 + 多 Agent 并行（5 Agent 上限）+ 4 级故障补救 + 负载均衡 + 状态跟踪 + **§13 跨 AI 工具适配抽象层**（SubAgent Adapter Interface）|
| **🆕 project-assets-update-skill**（强化） | `skills/cross-cutting/project-assets-update-skill.md` | **7 层索引**（§A 资产大纲 / §B 模块索引 / §C 字段索引 / §D 组件索引 / §E API 索引 / §F 关键词反向索引 / §G 资产读取 API）；按需加载而非全文加载；🆕 §G 升级为对外服务契约（8 个场景化 API：assets.forStoryGenerate / forStoryReview / forCoding / forCodeReview 等，调用方 SKILL 禁止直接全文读取 assets.md） |

### 3.4 模板与标准（`templates/` + `standards/` + `assets/` + `scripts/`）

| 目录 | 内容 |
|------|------|
| `templates/coding/` | `be-coding-plan-template.md`（16 节 CodePlan）/ `be-codereview-template.md`（10 节 CR 报告）/ `be-coding-report-template.md`（9 节 Coding 报告）|
| `templates/proposal/` | `proposal-template.md`（4 段必填 Proposal）|
| `templates/testcase/` | `be-testcase-template.md` + `be-testcase-report-template.md` |
| `templates/design/` | DR / Story / Task / Story Review UpdatePlan 模板 |
| `templates/project-assets/` | `project-assets-update-log-template.md`（项目资产更新日志）|
| **🆕 `standards/constraints/`** | 9 个约束（原 constraints/）：api / code-style / database / implicit-constraints / layered-arch / project-structure / security / technology-stack / testing |
| **🆕 `standards/thinking/`** | `be-coding-thinking-engine.md`（编码思维引擎，11 维决策 + 7 基准 + 4 问）|
| **🆕 `standards/testing/`** | `be-testcase-strategy.md`（测试用例生成策略）|
| **🆕 `standards/project-assets/`** | `project-assets-schema.md` + `project-assets-template.md`（项目资产 schema + 模板）|
| **🆕 `assets/{projectKey}/`** | 每个项目的资产主体 `.assets.md` + 更新日志 `.update-log.md`（原 project-assets/）|
| `scripts/` | `sync-to-plugin.sh`（母版→plugin 同步）+ `test_authenticity_scan.py`（测试真实性扫描）|

---

## 4. 使用指导：从 0 到 1 跑一个 Story

### 4.1 🆕 需求分析流程（从 PRD / Issue / 对话需求 开始 — 2026-06-17）

**场景：** 用户只有一份 PRD / Issue / 一段对话需求，不知道怎么往下走。

```
Step 1：用户输入 "从 PRD 开始" / "帮我分析 XX 需求"
    ↓
AE-skill 智能路由（关键词"从 PRD 开始"→ Phase 0 ①）
    ↓
加载 requirement-analysis-skill.md
    ↓
按 8 维度挖掘 SOP：
  A 角色矩阵（谁会用）
  B 场景清单（什么场景）
  C 业务流程（怎么走）
  D 数据模型（需要什么数据）
  E 业务规则（约束是什么）
  F 设计方向（技术决策基线）
  G 验收标准（AC）
  H 假设与风险
    ↓
5 问自检 + 4 级缺口管理 + 5 维规模评分
    ↓
按规模结果路由：
  - 大 → 走 dr-generate → dr-review → 续 Phase 1 ②
  - 中 → 直接进 Phase 1 ② story-generate
  - 小 → 跳过 Story 直出 Task + CodingPlan
  - 微 → 直接 CodingPlan + Coding
    ↓
输出：ae-sdd-doc/iterations/{date}/RA/{requirement-id}.md
    ↓
触发 🔍 人工审核点 0（用户审阅需求分析）
```

### 4.2 后续流程（已分析后 — 从 DR 开始）

**场景：** 用户已完成需求分析并拿到 DR，本节描述从 DR 到上线的标准流程。

### 4.3 前置准备（一次性）

**步骤 1：项目资产初始化**
```
用户说：项目名是 icec-cloud-boss，请生成项目资产
    ↓
AE-skill 路由 → project-assets-update-skill.md §3 生成动作
    ↓
按 9 步探查 SOP（读 AGENTS.md / 读 constraints/ / 跑 mvn / 抽典型类 / ...）
    ↓
输出：skills/ae-sdd/project-assets/icec-cloud-boss/icec-cloud-boss.assets.md
```

**步骤 2：依赖读取**
- 读 `document-storage-skill.md §11.1`（横切依赖）
- 读 `agent-orchestration-skill.md §1-3`（任务拆分原则）
- 读 `proposal-skill.md`（统一问题载体）

### 4.4 单个 Story 跑完（典型流程）

**场景：** 用户有 1 个 Story 要从 DR 写到上线。

```
Step 1：用户输入 "从 DR 开始，STORY-001-BE"
    ↓
AE-skill 智能路由（关键词"从 DR 开始"→ Phase 1 ①）
    ↓
加载 story-generate-skill.md
    ↓
按 7 阶段挖掘 SOP：
  A 业务背景
  B 主流程 + 异常流程
  C AC 验收标准
  D 接口契约
  E 数据模型
  F 实现任务映射
  G ①bis 前端接口契约（6 维度）
    ↓
填入 design/story/be/STORY-001-BE.md（按 document-storage-skill §2.1 路径）
    ↓
触发 Phase 1 ② Story Review
```

```
Step 2：用户说 "出 STORY-001-BE 的 Story Review 报告"
    ↓
AE-skill 路由 → story-review-skill.md
    ↓
按 5 阶段挖掘 + F-Stage 前端契约 + 4 标尺
    ↓
出 StoryReviewReport-r1（按 document-storage-skill §2.3 路径）
    ↓
如有 🔴 缺陷 → 触发 proposal-skill.md（渠道 2 Story Review）
```

```
Step 3：用户说 "Story 评审通过，开始生成 Task"
    ↓
AE-skill 路由 → task-generate-skill.md
    ↓
按 6 步流程生成 Task 文档（含任务级 CodePlan + 调 coding-skill §④bis）
    ↓
出 task-N-X.md（按 §2.1 路径）
    ↓
全局 Task Review（TR-1~TR-6）
    ↓
如有 🔴 缺陷 → 触发 proposal-skill.md
```

```
Step 4：用户说 "开始 Coding"
    ↓
AE-skill 路由 → coding-skill.md
    ↓
按 §④bis 5 步 SOP（任务级 CodePlan）
    ↓
复杂任务时调 agent-orchestration-skill.md 拆子任务并行（如拆 Domain/Application/Infrastructure/Tests）
    ↓
Coding 完成 → 触发 coding-report-skill.md 出 Coding 报告（r1）
```

```
Step 5：用户说 "出 Code Review 报告"
    ↓
AE-skill 路由 → code-review-skill.md
    ↓
选评审模式 A/B/C（建议 B 双 reviewer 业务+架构）
    ↓
按 6 阶段评审 + 7 道闸
    ↓
出 CodeReview 报告（r1，按 document-storage-skill §2.2 路径）
    ↓
如有 🔴 缺陷 → 触发 proposal-skill.md（渠道 1 Code Review）
    ↓
走 5 步流程（改 Story → TestCase → Task → Coding → Test）
```

```
Step 6：用户说 "全部通过"
    ↓
回到原评审确认问题已解决
    ↓
归档所有 Proposal
    ↓
Story 完成，可上线
```

### 4.5 重入流程

**场景：** 用户跑了 1-2 轮 Coding 发现问题，需要重入。

```
用户说 "从 STORY-001 继续"
    ↓
AE-skill 读 state.json → 解析 currentPhase + currentStep
    ↓
判定：重入到 Phase 2 ⑤ Coding 第 2 轮
    ↓
路由 → coding-skill.md + 携带 completedSteps（跳过已完成）
    ↓
Coding 完成 → CodingReport-v1-r2.md（r 递增，不修改历史）
```

### 4.6 发现问题

**场景：** 用户反馈 / Test 失败 / Code Review 发现问题。

```
用户说 "修一下 roleId=0 的特殊语义"
    ↓
AE-skill 路由（"修一下"→ 渠道 5 用户反馈）→ proposal-skill.md
    ↓
填写 4 段 Proposal（原本/目标/方案/涉及范围）
    ↓
走 5 步流程（改 Story → TestCase → Task → Coding → Test）
    ↓
所有下游修改完成 → 回到原评审
```

---

## 5. 核心设计原则（5 大）

### 5.1 统一问题载体（Proposal）

**🔴 原则：** 所有问题 → 先生成 Proposal → 走 5 步流程

- 不分渠道（Code Review / Story Review / Coding 异常 / Project Assets 漂移 / 用户反馈 / 生产事故 / Test 发现）
- 不内置 UpdatePlan（替代 CodeReview UpdatePlan / StoryReview UpdatePlan / Coding 异常追溯链 / Project Assets Update 4 处散落）
- 4 段必填（原本/目标/方案/涉及范围）+ 7 道闸 + 5 步走流程

### 5.2 抽象分层规则不变

**🔴 原则：** 4 类抽象分层（请求处理/业务编排/领域逻辑/基础能力）+ 2 可选（跨模块 SPI/BFF 入口）

- 项目特化的具体分层 → 从项目资产 §3 读取
- 模板**不写死**分层（避免跨项目失效）
- 所有 SKILL 引用项目资产 §3/§4/§6 作为事实基线

### 5.3 横切依赖标准

**🔴 原则：** 3 个横切依赖 SKILL 解决"跨节点通用问题"

| 横切依赖 | 解决什么问题 |
|---------|------------|
| `document-storage-skill` | 文档怎么存、怎么命名、重入怎么处理 |
| `proposal-skill` | 问题怎么描述、修改方案怎么统一载体 |
| `agent-orchestration-skill` | 任务节点内子任务怎么拆、Agent 怎么分、故障怎么补救 |

### 5.4 模板 = 通用结构，项目资产 = 工程特化

**🔴 原则：** 模板只定通用结构（什么字段、什么门禁），不写死工程特化（哪些层、什么命名）

- 模板（如 `be-codereview-template.md`）→ 通用结构 + 占位
- 项目资产（如 `icec-cloud-boss.assets.md §3`）→ 工程特化结构
- 模板引用项目资产 → 自适应

### 5.5 实时追溯链

**🔴 原则：** Coding 报错 → 先排除文档缺陷 → 再判定 AI 自己犯蠢

```
发现 Coding 报错
    ↓
追溯层 1️⃣：先读 Task 文档（Code Plan 写在 Task 内）
    ↓ 错则修 Task
追溯层 2️⃣：再读 Story 文档
    ↓ 错则修 Story
追溯层 3️⃣：照例去检查 DR
    ↓ 错则修 DR
追溯层 4️⃣：判定 AI 自己犯蠢（兜底）
    ↓ 纯实现问题直接改代码
```

### 5.6 三语言翻译链（🆕 2026-06-17）

**🔴 原则：** PRD（业务语言）→ DR（技术语言）→ Story（工程语言）

- 业务侧：PRD 用业务语言描述（"用户能..."、"系统应..."）
- 技术侧：DR 用技术语言描述（"领域模型"、"接口契约"、"状态机"）
- 工程侧：Story 用工程语言描述（"实现方法"、"数据模型"、"Task 拆分"）

`requirement-analysis-skill` 负责业务 → 技术翻译
`dr-generate-skill` 负责技术方案落地
`story-generate-skill` 负责工程实现拆分

---

## 6. 横切依赖（3 个）

### 6.1 `document-storage-skill.md`（🆕 2026-06-17 文档系统统一）

**何时调用：** 任何生成/更新文档的 SKILL 在落地前**第一步**调用。

**8 类流程目录（🆕）：**
- PRD / RA / DR / Story / Task / Coding / Test / CR

**统一存放：**
- 主目录：`{工程根}/ae-sdd-doc/`
- 迭代目录：`ae-sdd-doc/iterations/{YYYY-MM-DD}/`
- ChangeLog：`iterations/{date}/{DocType}/ChangeLog/{doc-id}-changelog.md`

**版本号（🆕）：**
- 设计类文档：v{major}.{minor}（v1.0 → v1.1 → v2.0，旧版保留）
- 事件类报告：v{N}-r{M}（r 递增）

**关联性分析（🆕）：**
- 业务关联 0/1（B1 业务域 / B2 业务场景 / B3 业务实体 / B4 业务规则）
- 逻辑关联 0/1（L1 代码调用 / L2 数据流 / L3 状态流转 / L4 组件复用）
- 强关联/中关联：默认入当前迭代；无关联：强制问用户

**存量迁移（🆕）：**
- `scripts/migrate-docs.mjs`（默认 DRY-RUN）
- 旧路径：design/、.ae-task/、.ae-plan/、.spec/iterations/
- 新路径：ae-sdd-doc/iterations/{date}/{DocType}/

**完整 API 见** `document-storage-skill.md §11.5`

### 6.2 `proposal-skill.md`（统一问题载体）

**何时调用：** 任何问题（Code Review 缺陷 / Story Review 缺陷 / Coding 异常 / Project Assets 漂移 / 用户反馈 / 生产事故 / Test 发现）触发时调用。

**核心结构：**
- §1 原本是怎么样（现状 + 证据）
- §2 要做什么（目标 + AC）
- §3 怎么做（方案 + 影响分析 + 5 步拆解）
- §4 涉及范围（下游 5 类动作）

**完整流程见** `proposal-skill.md §第五步`

### 6.3 `agent-orchestration-skill.md`（多 Agent 并行 + 故障补救 + 跨 AI 工具适配）

**何时调用：** 节点 SKILL 复杂任务时，决定"是否拆子任务并行"。

**核心规则：**
- **拆与不拆：** 同一节点内独立子任务才拆（如 Coding 按 DDD 分层拆）
- **5 Agent 硬上限**（避免 token 爆炸）
- **4 级故障补救**（重试 → 重新分配 → 降级 → 升级用户，3 次上限）
- **🆕 跨 AI 工具可移植性（§13）：** SubAgent Adapter Interface 抽象层——节点 SKILL 通过 `adapter.spawn/awaitAll/cancel/status` 调用子 Agent，**不绑定** Claude Code / Cursor / Cline / Copilot 等具体工具的 Workflows / subAgent / Task tool 机制

**完整决策见** `agent-orchestration-skill.md §1-13`

---

## 7. 常见问题 FAQ

### Q1：用户输入"修一下 XX"该走哪个 SKILL？

**A：** AE-skill 智能路由（关键词"修一下"）→ `proposal-skill.md`（渠道 5 用户反馈）。先生成 Proposal → 走 5 步流程。

### Q2：任务节点内子任务可以拆多少个？

**A：** 最多 5 个（`agent-orchestration-skill.md §2.2` 硬上限）。超 5 个必须先合并再考虑。

### Q3：Coding 报错是先改代码还是先改文档？

**A：** 实时追溯链（`coding-skill.md §异常路径`）：先读 Task → Task 错则修 Task → 再读 Story → Story 错则修 Story → 再读 DR → DR 错则修 DR → 都无问题时改代码。**先改文档再改代码**。

### Q4：事件类报告（如 Coding 报告）重入时是新建还是修改？

**A：** **新增**（r 递增，不修改历史）。`document-storage-skill.md §3.2` 明确规定事件类带版本号。

### Q5：项目资产怎么改？

**A：** 永远通过 `proposal-skill.md` 走流程，**不直接改**（`project-assets-update-skill.md §4.2 步骤 3` 2026-06-06 改造）。

### Q6：模板写死"5 层"怎么办？

**A：** 反模式。**模板不写死**分层（哪些层、层数），具体分层从项目资产 §3 读取（`document-storage-skill.md` 横切依赖标准 + `feedback-template-no-hardcode-project-structure` 项目 memory）。

### Q7：如何判断 Proposal 该重写还是修订？

**A：** Proposal **永不修改**（一次性写完）。重写场景 = 新 Proposal（同 N 编号 + 合并 §1/§2/§3 块）。多源合并 = 新 Proposal。

### Q8：什么时候用单 Agent / 多 Agent？

**A：** 简单任务（单一输出）= 1 个 Agent。复杂任务（多输出/多文件/多评审维度）= 多 Agent。**不超过 5 个**。详见 `agent-orchestration-skill.md §2`。

### Q9：用户输入"从 STORY-001 继续"是什么意思？

**A：** AE-skill 读 `state.json` 判定重入点（`ae-sdd-skill.md §🎯 统一入口与智能路由`）。重入到上次停在的步骤。

### Q10：AE-skill 跟 auto-engineering-update-skill 区别？

**A：**
- `ae-sdd-skill.md` = 流程编排 + 智能路由（用户怎么用）
- `ae-sdd-update-skill.md` = SKILL 边界维护规范（开发者怎么维护 SKILL 家族）

### Q11：用户输入"从 PRD 开始"该走哪个 SKILL？（🆕 2026-06-17）

**A：** AE-skill 智能路由 → `requirement-analysis-skill.md`。先生成 RA 文档 + 5 维规模裁定 → 按规模结果路由到大/中/小/微。

### Q12：requirement-analysis 阶段是做什么的？（🆕 2026-06-17）

**A：** 8 维度并行挖掘（角色/场景/流程/数据/规则/设计方向/AC/假设）+ 5 问自检 + 缺口管理。**不容许杜撰、不容许有歧义**。

### Q13：关联性分析是什么？（🆕 2026-06-17）

**A：** 文档存放前的关联度判定。业务关联 + 逻辑关联双维 0/1，命中即关联；全部 0 强制问用户放哪个迭代。详见 `document-storage-skill.md §6`。

### Q14：ae-sdd-doc/ 与 design/、.ae-task/、.ae-plan/ 是什么关系？（🆕 2026-06-17）

**A：** ae-sdd-doc/ 是新统一目录。design/、.ae-task/、.ae-plan/ 是旧路径，存量文档可通过 `scripts/migrate-docs.mjs` 一次性迁移（默认 DRY-RUN）。

---

## 8. 维护与扩展

### 8.1 文件组织

```
skills/ae-sdd/
├── README.md                                ← 本文件
├── *.md (15 个 SKILL)
├── strategies/                              ← 测试策略
├── templates/                               ← 空白模板（按阶段归类）
│   ├── coding/                              (CodePlan / CR / Coding 报告)
│   ├── design/                              (DR / Story / Task)
│   ├── proposal/                            (Proposal 模板)
│   ├── testcase/                            (测试用例 / 测试报告)
│   └── project-assets/                      (项目资产更新日志)
├── project-assets/                          ← 项目特定资产（按 projectKey 分目录）
│   └── icec-cloud-boss/
│       ├── icec-cloud-boss.assets.md
│       └── icec-cloud-boss.update-log.md
└── scripts/                                 ← 辅助脚本
```

### 8.2 健康度自检（每月跑 1 次）

| 检查项 | 阈值 | 工具 |
|--------|------|------|
| AE-skill 行数 | < 1500 | `wc -l ae-sdd-skill.md` |
| 子 SKILL 关键章节齐全 | 必填章节存在 | `grep` 关键章节标题 |
| 项目资产 §1 lastAuditedAt | < 90 天 | 读 `.assets.md` |
| 缺口进度 | 有推进（不允许连续 3 个月不变）| 读 `.update-log.md §4` |
| 知识衰减（包路径/命名/微服务清单）| < 5 处漂移 | 跑 `agent-orchestration-skill.md §5.2` 资源评估 |
| 🆕 同工作流收束 | 同 STORY-ID 文档在 `design/story/be/{STORY-ID}/` 树下 | `grep` STORY-ID 检查所有引用 |
| 🆕 跨 AI 工具适配 | `agent-orchestration-skill.md §13` adapter 抽象接口存在 | `grep "SubAgentAdapter"` |
| 🆕 4 类需求路由 | 智能路由表覆盖 4 类（已有 Story / 中大 / 小 / 微）+ 任务规模判定 | `grep` 路由表 |
| 🆕 3 类任务路径 | `document-storage §2.6` 完整（重/小/微 三类路径模板） | `grep` 路径模板 |

### 8.3 新增 SKILL 流程

1. 读 `ae-sdd-update-skill.md §SKILL 边界判定表` 判定内容放哪
2. 写新 SKILL 顶部加 `📦 文档存放前置调用` 段（引用 `document-storage-skill`）
3. 更新本 README §3 SKILL 功能清单
4. 更新 `ae-sdd-skill.md §智能路由表` 加新场景
5. 更新 `ae-sdd-update-skill.md` 边界判定表
6. 更新 `MEMORY.md` 索引

### 8.4 修改 SKILL 流程

1. 读 `ae-sdd-update-skill.md §SKILL 健康度自检清单`
2. 修改 SKILL 内容（**先记录再修改** — log 写"待更新"条目）
3. 同步更新其他引用本 SKILL 的文档（auto-engineering-skill / auto-engineering-update-skill / 本 README）
4. **若改动了 `ae-sdd-skill.md`，必须同步更新本 README §3/§4.2/§8.5 及末行"最后更新"日期**（见 `ae-sdd-update-skill.md §步骤 4`）
5. 更新 `MEMORY.md`

### 8.5 常见变更场景

| 场景 | 涉及文件 |
|------|---------|
| 新增流程节点 | 1. 新 SKILL 2. auto-engineering-skill §智能路由表 3. auto-engineering-update-skill 边界表 4. **README §3 + §4.2 + §8.5 + 末行日期** |
| 修改横切依赖 | 1. 横切依赖 SKILL 2. 各调用方 SKILL 的"📦 文档存放前置调用"段 3. auto-engineering-update-skill 边界表 4. **README §3 功能清单对应行 + 末行日期**；🆕 修改 project-assets-update-skill §G（场景 API 增减）时，同步更新所有调用方 SKILL 的 assets.forXxx() 调用 |
| 改造节点 SKILL | 1. 节点 SKILL 2. **README §3 功能清单对应行 + 末行日期** 3. auto-engineering-skill §智能路由表（场景）|
| 废弃 SKILL | 1. SKILL 顶部加"DEPRECATED" 2. **README §3 标记"已废弃" + 末行日期** 3. auto-engineering-update-skill 边界表删行 |
| 🆕 新增需求分析 SKILL 链路 | 1. 3 个新 SKILL 2. ae-sdd-skill §智能路由表 3. ae-sdd-update-skill 边界表 4. README §1.2/§3.2/§4.1/§5.6/末行日期 5. CHANGELOG/ |
| 🆕 文档系统统一 | 1. document-storage-skill §1-§8 2. 各调用方 SKILL 的"📦 文档存放前置调用"段 3. README §6.1/§3.3 4. ae-sdd-update-skill 边界表 5. README 末行日期 |
| 🆕 路由 4 维增强 | 1. ae-sdd-skill §1.4/§1.5/§1.6/§1.7 2. ae-sdd-update-skill 边界表 3. README §1.1/§1.2/§1.3/末行日期 |

---

## 📚 完整 SKILL 家族速查表

| # | SKILL | 类别 | 触发词 |
|---|-------|------|-------|
| 1 | auto-engineering-skill | 编排 + 入口 | "从 X 继续" / 任意用户输入 |
| 2 | auto-engineering-update-skill | 边界维护 | "新增/修改 SKILL" |
| 3 | agent-orchestration-skill | 横向基础设施 | 复杂任务时由节点 SKILL 调 |
| 4 | document-storage-skill | 横向基础设施 | 任何生成/更新文档前 |
| 5 | proposal-skill | 横向基础设施 | "修一下 XX" / 任何问题发现 |
| 6 | project-assets-update-skill | 横向基础设施 | "生成/更新/审计/读取项目资产" |
| 7 | requirement-analysis-skill | 节点（Phase 0 ①）| "分析需求" / "从 PRD 开始" / "需求拆解" |
| 8 | dr-generate-skill | 节点（Phase 0 ①.5）| "生成 DR" / "写 DR" |
| 9 | dr-review-skill | 节点（Phase 0 ①.6）| "DR 评审" / "DR Review" |
| 10 | story-generate-skill | 节点（Phase 1 ②）| "从 DR 开始" / "生成 Story" |
| 11 | story-review-skill | 节点（Phase 1 ③）| "Story 评审" |
| 12 | story-update-skill | 节点（Phase 1 ③）| "修改/补 Story" |
| 13 | testcase-generate-skill | 节点（Phase 1 ④）| "生成测试用例" |
| 14 | task-generate-skill | 节点（Phase 2 ⑤）| "生成 Task" |
| 15 | coding-skill | 节点（Phase 2 ⑥）| "开始 Coding" / "代码写错了" |
| 16 | coding-report-skill | 节点（Phase 2 ⑥）| "出 Coding 报告" |
| 17 | code-review-skill | 节点（Phase 3 ⑧）| "Code Review 报告" |
| 18 | dr-update-skill | 节点（-）| "修改/补 DR" |

---

## 🆘 紧急联系

- **架构组：** 维护 SKILL 家族 / 跨 SKILL 协调
- **项目 owner：** 维护本项目的 project-assets/*.assets.md
- **评审员（业务/架构/测试）：** 各自 SKILL 的多 reviewer 模式

---

**最后更新：** 2026-06-17（SKILL 间能力调用规范化（二轮）：项目资产 §G 场景化 API 8 个 + 7 SKILL 调用方式统一 + ae-sdd-doc 路径全面修正 + code-review 多 Agent 去重 DRY -160行 + agent-orchestration 示例路径修正 + pending-questions 机制 + 直接修改边界说明 + 并行执行引用声明）
**下次审查：** 每月 1 号（与项目资产审计同步）

---

Made by [@AILenGarden](https://github.com/AILenGarden)
